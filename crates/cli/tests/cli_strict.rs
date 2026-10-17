//! `sbrs` refuses what clap refused and usage 6 lets through (see `src/strict.rs`): a value
//! written after a switch. It is checked against the real binary.
//!
//! Nothing here starts a browser. The commands that would run one do so against a fake
//! WebDriver server on a loopback port, which sees whether a session was asked for.
//! Run with `cargo test -p seleniumbase-rs-cli --test cli_strict`.

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "support/sbrs.rs"]
mod sbrs;

use std::process::Output;

use fake_webdriver::FakeWebDriver;
use sbrs::{run, stderr, stdout};

/// Runs `sbrs args...` in a folder of its own and returns the folder with the result.
fn in_a_new_folder(args: &[&str]) -> (tempfile::TempDir, Output) {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = run(dir.path(), &[], args);
    (dir, output)
}

/// What the process said on standard error, after checking it was refused the way clap
/// refused a bad command line: status 2, nothing on standard output.
fn refused(output: &Output) -> String {
    let error = stderr(output);
    assert_eq!(output.status.code(), Some(2), "{error}");
    assert!(stdout(output).is_empty(), "{}", stdout(output));
    error
}

#[test]
fn a_value_written_after_a_global_switch_is_refused() {
    for value in ["true", "false", "1", "0", "yes", ""] {
        let argument = format!("--headless={value}");
        let (_dir, output) = in_a_new_folder(&[&argument, "version"]);

        let error = refused(&output);
        assert!(error.contains("--headless"), "{argument}: {error}");
        assert!(error.contains("takes no value"), "{argument}: {error}");
    }
}

#[test]
fn a_value_written_after_any_global_switch_is_refused() {
    // The message names the switch by its first spelling, whichever one was typed.
    for (argument, name) in [
        ("--headed=true", "--headed"),
        ("--uc=1", "--uc"),
        ("--cdp=false", "--cdp"),
        ("--rs=true", "--reuse-session"),
        ("--mobile=0", "--mobile"),
    ] {
        let (_dir, output) = in_a_new_folder(&[argument, "version"]);

        let error = refused(&output);
        assert!(
            error.contains(&format!("for '{name}'")),
            "{argument}: {error}"
        );
    }
}

#[test]
fn a_value_written_after_a_switch_of_a_subcommand_is_refused_and_nothing_is_written() {
    for argument in ["--force=true", "--force=false", "--basic=1"] {
        let (dir, output) = in_a_new_folder(&["mkdir", "ui", argument]);

        let error = refused(&output);
        assert!(error.contains("takes no value"), "{argument}: {error}");
        assert!(!dir.path().join("ui").exists(), "{argument}: mkdir ran");
    }
}

#[test]
fn a_switch_without_a_value_still_works() {
    let (_dir, output) = in_a_new_folder(&["--headless", "--uc", "version"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let (dir, output) = in_a_new_folder(&["mkfile", "tests/a.rs", "--basic", "--force"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(dir.path().join("tests/a.rs").exists());
}

#[test]
fn a_value_after_a_switch_asks_for_no_session() {
    let driver = FakeWebDriver::start();
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(
        dir.path(),
        &[],
        &[
            "--webdriver",
            driver.url(),
            "--headless=false",
            "open",
            "https://example.test/",
        ],
    );

    refused(&output);
    assert!(driver.requests().is_empty(), "a browser was asked for");
}

#[test]
fn the_help_flag_still_answers_before_a_mistake_after_it() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(dir.path(), &[], &["mkdir", "--help", "--force=1"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("--force"), "{}", stdout(&output));
}
