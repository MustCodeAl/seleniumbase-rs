//! `sbrs grid-hub` and `sbrs grid-node`, run as the real binary.
//!
//! No Selenium Server jar or Java is needed: these check that the commands are
//! reachable, what they say when the jar is missing, and that nothing is
//! running when nothing was started. Starting a real Grid is covered, with a
//! stand-in for Java, by the unit tests in `cli::scripts::sb_grid`.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs` with its Grid records kept in `state` and no jar in the
/// environment.
fn sbrs(state: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .env("SB_GRID_DIR", state)
        .env_remove("SB_SELENIUM_SERVER_JAR")
        .output()
        .expect("sbrs starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn both_commands_are_listed_and_describe_their_options() {
    let state = tempfile::tempdir().unwrap();
    for command in ["grid-hub", "grid-node"] {
        let out = sbrs(state.path(), &[command, "--help"]);
        assert!(out.status.success(), "{command}: {}", text(&out.stderr));
        let help = text(&out.stdout);
        for word in ["start", "stop", "restart", "status", "--jar", "--grid-port"] {
            assert!(help.contains(word), "{command} help lacks {word}:\n{help}");
        }
    }
}

#[test]
fn starting_without_a_jar_fails_and_says_nothing_is_downloaded() {
    let state = tempfile::tempdir().unwrap();
    let out = sbrs(state.path(), &["grid-hub", "start"]);
    assert!(!out.status.success());
    let error = text(&out.stderr);
    assert!(error.contains("SB_SELENIUM_SERVER_JAR"), "{error}");
    assert!(
        error.contains("not") && error.contains("download"),
        "{error}"
    );
}

#[test]
fn a_jar_that_does_not_exist_is_refused() {
    let state = tempfile::tempdir().unwrap();
    let out = sbrs(
        state.path(),
        &["grid-hub", "start", "--jar", "/definitely/not/here.jar"],
    );
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("does not exist"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn nothing_is_running_when_nothing_was_started() {
    let state = tempfile::tempdir().unwrap();
    let stop = sbrs(state.path(), &["grid-node", "stop"]);
    assert!(stop.status.success(), "{}", text(&stop.stderr));
    assert!(
        text(&stop.stdout).to_lowercase().contains("not running"),
        "{}",
        text(&stop.stdout)
    );
    let status = sbrs(state.path(), &["grid-hub", "status"]);
    assert!(status.status.success(), "{}", text(&status.stderr));
    assert!(
        text(&status.stdout).to_lowercase().contains("not running"),
        "{}",
        text(&status.stdout)
    );
}

#[test]
fn the_help_uses_the_python_names_for_verbose_and_the_idle_timeout() {
    let state = tempfile::tempdir().unwrap();
    for command in ["grid-hub", "grid-node"] {
        let out = sbrs(state.path(), &[command, "--help"]);
        assert!(out.status.success(), "{command}: {}", text(&out.stderr));
        let help = text(&out.stdout);
        for word in ["-v, --verbose", "--timeout", "<SECONDS>"] {
            assert!(help.contains(word), "{command} help lacks {word}:\n{help}");
        }
        for old in ["--grid-verbose", "--session-timeout"] {
            assert!(
                !help.contains(old),
                "{command} help still has {old}:\n{help}"
            );
        }
    }
}

#[test]
fn the_python_spellings_grid_hub_and_grid_node_are_the_same_commands() {
    let state = tempfile::tempdir().unwrap();
    for command in ["grid_hub", "grid_node"] {
        let status = sbrs(state.path(), &[command, "status"]);
        assert!(
            status.status.success(),
            "{command}: {}",
            text(&status.stderr)
        );
        assert!(
            text(&status.stdout).to_lowercase().contains("not running"),
            "{command}: {}",
            text(&status.stdout)
        );
    }
}

#[test]
fn verbose_and_timeout_are_accepted_before_the_jar_is_looked_for() {
    let state = tempfile::tempdir().unwrap();
    for flags in [
        &["-v", "--timeout", "30"][..],
        &["--verbose", "--timeout=30"],
    ] {
        let mut args = vec!["grid-hub", "start"];
        args.extend_from_slice(flags);
        args.extend_from_slice(&["--jar", "/definitely/not/here.jar"]);
        let out = sbrs(state.path(), &args);
        // The command line parsed (a usage error prints the page); the jar is what is missing.
        assert!(!out.status.success(), "{args:?}: {}", text(&out.stdout));
        assert!(
            !text(&out.stderr).contains("Usage:"),
            "{args:?}: {}",
            text(&out.stderr)
        );
        assert!(
            text(&out.stderr).contains("does not exist"),
            "{args:?}: {}",
            text(&out.stderr)
        );
    }
}

#[test]
fn the_earlier_long_names_and_a_timeout_that_is_not_a_number_are_usage_errors() {
    let state = tempfile::tempdir().unwrap();
    for args in [
        &["grid-hub", "start", "--grid-verbose"][..],
        &["grid-node", "start", "--session-timeout", "30"],
        &["grid-hub", "start", "--timeout", "soon"],
    ] {
        let out = sbrs(state.path(), args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            text(&out.stderr)
        );
    }
}
