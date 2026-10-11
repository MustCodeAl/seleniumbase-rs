//! `sbase commander`, run as the real binary.
//!
//! The Commander is a full-screen terminal menu that exists only in a build
//! with the `tui` feature. A build without it must not touch the terminal: the
//! command prints a hint on how to get the feature and exits with status 2.
//! That build is the default one, so the status and hint tests run only when
//! `tui` is off. A `tui` build would take over the terminal that runs the
//! tests, so it is not started here.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// Every `SB_*` variable `sbase` reads, cleared so the host's own settings
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

/// Runs `sbase` in `dir`, an empty folder, so no `sbase_config.toml` of the
/// host is picked up, and without the user's `SB_*` settings.
fn sbase(dir: &Path, args: &[&str]) -> Output {
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
    command.output().expect("sbase runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_help_lists_commander_and_says_it_needs_the_tui_feature() {
    let dir = tempfile::tempdir().expect("temp dir");

    let list = sbase(dir.path(), &["--help"]);
    assert!(list.status.success(), "{}", stderr(&list));
    assert!(stdout(&list).contains("commander"), "{}", stdout(&list));

    let help = sbase(dir.path(), &["commander", "--help"]);
    assert!(help.status.success(), "{}", stderr(&help));
    assert!(
        stdout(&help).contains("`tui` feature"),
        "the command's help names the feature it needs:\n{}",
        stdout(&help)
    );
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_commander_exits_with_status_2_and_a_hint() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbase(dir.path(), &["commander"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let hint = stderr(&output);
    assert!(hint.contains("built without the terminal UI"), "{hint}");
    assert!(
        hint.contains("cargo install seleniumbase-rs --features tui"),
        "the hint says how to get the feature: {hint}"
    );
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_commander_leaves_the_terminal_alone() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbase(dir.path(), &["commander"]);

    // Nothing on stdout, and no escape sequence on either stream: the
    // alternate screen and raw mode of the real UI were never entered.
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(!stderr(&output).contains('\x1b'), "{}", stderr(&output));
    assert!(!stdout(&output).contains('\x1b'));
}
