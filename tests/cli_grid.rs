//! `sbase grid-hub` and `sbase grid-node`, run as the real binary.
//!
//! No Selenium Server jar or Java is needed: these check that the commands are
//! reachable, what they say when the jar is missing, and that nothing is
//! running when nothing was started. Starting a real Grid is covered, with a
//! stand-in for Java, by the unit tests in `cli::scripts::sb_grid`.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// Runs `sbase` with its Grid records kept in `state` and no jar in the
/// environment.
fn sbase(state: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .env("SB_GRID_DIR", state)
        .env_remove("SB_SELENIUM_SERVER_JAR")
        .output()
        .expect("sbase starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn both_commands_are_listed_and_describe_their_options() {
    let state = tempfile::tempdir().unwrap();
    for command in ["grid-hub", "grid-node"] {
        let out = sbase(state.path(), &[command, "--help"]);
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
    let out = sbase(state.path(), &["grid-hub", "start"]);
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
    let out = sbase(
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
    let stop = sbase(state.path(), &["grid-node", "stop"]);
    assert!(stop.status.success(), "{}", text(&stop.stderr));
    assert!(
        text(&stop.stdout).to_lowercase().contains("not running"),
        "{}",
        text(&stop.stdout)
    );
    let status = sbase(state.path(), &["grid-hub", "status"]);
    assert!(status.status.success(), "{}", text(&status.stderr));
    assert!(
        text(&status.stdout).to_lowercase().contains("not running"),
        "{}",
        text(&status.stdout)
    );
}
