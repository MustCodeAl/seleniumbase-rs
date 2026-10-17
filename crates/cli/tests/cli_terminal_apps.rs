//! `sbrs caseplans` and `sbrs behave-gui`, run as the real binary.
//!
//! Both are full-screen terminal apps that exist only in a build with the `tui`
//! feature. A build without it must not touch the terminal: each command (and
//! its alias) prints a hint on how to get the feature and exits with status 2,
//! as `sbrs commander` does. That build is the default one, so the status and
//! hint tests run only when `tui` is off. A `tui` build would take over the
//! terminal that runs the tests, so it is not started here; the apps' logic and
//! their drawing are tested in the library.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Every `SB_*` variable `sbrs` reads, cleared so the host's own settings
/// cannot reach a test.
const SETTINGS_VARS: [&str; 14] = [
    "SB_BROWSER",
    "SB_HEADLESS",
    "SB_MODE",
    "SB_USER_AGENT",
    "SB_LOCALE",
    "SB_AD_BLOCK",
    "SB_PROXY",
    "SB_PROXY_PAC_URL",
    "SB_USER_DATA_DIR",
    "SB_EXTENSION_DIR",
    "SB_REUSE_SESSION",
    "SB_MOBILE",
    "SB_THREADS",
    "SB_WEBDRIVER_URL",
];

/// Runs `sbrs` in `dir`, an empty folder, so no `sbase_config.toml` of the
/// host is picked up, and without the user's `SB_*` settings.
fn sbrs(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    for var in SETTINGS_VARS {
        command.env_remove(var);
    }
    command.output().expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `text` with every run of white space a single space, so that a phrase the
/// help wrapped across two lines is still found.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_help_lists_both_commands_with_their_aliases_and_says_they_need_the_tui_feature() {
    let dir = tempfile::tempdir().expect("temp dir");

    let list = sbrs(dir.path(), &["--help"]);
    assert!(list.status.success(), "{}", stderr(&list));
    for command in ["caseplans", "behave-gui"] {
        assert!(stdout(&list).contains(command), "{}", stdout(&list));
    }

    for name in ["caseplans", "case-plans", "behave-gui", "gui-behave"] {
        let help = sbrs(dir.path(), &[name, "--help"]);
        assert!(help.status.success(), "{name}: {}", stderr(&help));
        assert!(
            flat(&stdout(&help)).contains("`tui` feature"),
            "the help of {name} names the feature it needs:\n{}",
            stdout(&help)
        );
    }
}

#[test]
fn the_helps_name_the_terminal_app_not_a_window() {
    let dir = tempfile::tempdir().expect("temp dir");

    let caseplans = flat(&stdout(&sbrs(dir.path(), &["caseplans", "--help"])));
    let behave = flat(&stdout(&sbrs(dir.path(), &["behave-gui", "--help"])));

    assert!(caseplans.contains("terminal app"), "{caseplans}");
    assert!(caseplans.contains("--starter"), "{caseplans}");
    assert!(behave.contains("terminal app"), "{behave}");
    assert!(behave.contains("--new"), "{behave}");
    assert!(
        behave.contains("built-in steps"),
        "the limit of the runner is in the help:\n{behave}"
    );
}

#[test]
fn the_file_writing_options_cannot_be_combined_with_a_path() {
    let dir = tempfile::tempdir().expect("temp dir");

    let caseplans = sbrs(dir.path(), &["caseplans", "--starter", "tests"]);
    let behave = sbrs(dir.path(), &["behave-gui", "--new", "features"]);

    for (name, output) in [("caseplans", caseplans), ("behave-gui", behave)] {
        assert_eq!(output.status.code(), Some(2), "{name}: {}", stderr(&output));
        assert!(
            stderr(&output).contains("cannot be used with"),
            "{name}: {}",
            stderr(&output)
        );
    }
    assert!(
        std::fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing was written"
    );
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_each_command_and_alias_exits_with_status_2_and_a_hint() {
    let dir = tempfile::tempdir().expect("temp dir");

    for name in ["caseplans", "case-plans", "behave-gui", "gui-behave"] {
        let output = sbrs(dir.path(), &[name]);

        assert_eq!(output.status.code(), Some(2), "{name}: {}", stderr(&output));
        let hint = stderr(&output);
        assert!(
            hint.contains("built without the terminal UI"),
            "{name}: {hint}"
        );
        assert!(
            hint.contains(
                "cargo install --git https://github.com/MustCodeAl/seleniumbase-rs \
                 seleniumbase-rs-cli --features tui"
            ),
            "{name}: the hint says how to get the feature, from the package that builds sbrs: {hint}"
        );
    }
}

#[cfg(not(feature = "tui"))]
#[test]
fn a_path_does_not_change_that_and_nothing_is_read_or_written() {
    let dir = tempfile::tempdir().expect("temp dir");

    for args in [["caseplans", "tests"], ["behave-gui", "features"]] {
        let output = sbrs(dir.path(), &args);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
    assert!(
        std::fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing was written"
    );
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_the_commands_leave_the_terminal_alone() {
    let dir = tempfile::tempdir().expect("temp dir");

    for name in ["caseplans", "behave-gui"] {
        let output = sbrs(dir.path(), &[name]);

        // Nothing on stdout, and no escape sequence on either stream: the
        // alternate screen and raw mode of the real UI were never entered.
        assert!(stdout(&output).is_empty(), "{name}: {}", stdout(&output));
        assert!(
            !stderr(&output).contains('\x1b'),
            "{name}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn the_file_writing_options_work_in_every_build() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbrs(dir.path(), &["caseplans", "--starter"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(dir.path().join("CASE_PLANS.md").is_file());
}
