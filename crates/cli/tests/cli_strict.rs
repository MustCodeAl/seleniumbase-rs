//! `sbrs` refuses what clap refused and usage 6 lets through (see `src/strict.rs`): a flag
//! given twice, a value written after a switch, and `help` with a path that names no command.
//! Each is checked against the real binary.
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

fn assert_cannot_be_used_multiple_times(error: &str, flag: &str) {
    assert!(
        error.contains(&format!("the argument '{flag}")) && error.contains("multiple times"),
        "{error}"
    );
}

// ---------------------------------------------------------------------------
// A scalar flag given twice after a subcommand
// ---------------------------------------------------------------------------

#[test]
fn a_value_flag_given_twice_to_mkfile_is_refused_and_nothing_is_written() {
    let (dir, output) = in_a_new_folder(&[
        "mkfile",
        "tests/login.rs",
        "--url",
        "https://a.test/",
        "--url",
        "https://b.test/",
    ]);

    assert_cannot_be_used_multiple_times(&refused(&output), "--url");
    assert!(!dir.path().join("tests").exists(), "mkfile ran");
}

#[test]
fn a_switch_given_twice_to_mkdir_is_refused_and_nothing_is_written() {
    let (dir, output) = in_a_new_folder(&["mkdir", "ui", "--force", "--force"]);

    assert_cannot_be_used_multiple_times(&refused(&output), "--force");
    assert!(!dir.path().join("ui").exists(), "mkdir ran");
}

#[test]
fn a_short_and_a_long_spelling_of_one_flag_count_as_the_flag_given_twice() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.py"), "def test_a(self):\n    pass\n").unwrap();

    let output = run(
        dir.path(),
        &[],
        &[
            "import-python",
            "login.py",
            "-o",
            "first.rs",
            "--output",
            "second.rs",
        ],
    );

    assert_cannot_be_used_multiple_times(&refused(&output), "--output");
    assert!(!dir.path().join("first.rs").exists());
    assert!(!dir.path().join("second.rs").exists());
}

#[test]
fn a_flag_given_twice_to_a_command_that_drives_a_browser_asks_for_no_session() {
    let dir = tempfile::tempdir().expect("temp dir");
    for command in [
        &["get-text", "--css", "h1", "--css", "p"][..],
        &["wait-for-text", "--css", "h1", "--text", "a", "--text", "b"],
        &["hover", "--css", "h1", "--css", "h1"],
    ] {
        let driver = FakeWebDriver::start();
        let mut args = vec!["--webdriver", driver.url()];
        args.extend(command);

        let output = run(dir.path(), &[], &args);

        assert!(
            refused(&output).contains("multiple times"),
            "{command:?} was accepted"
        );
        assert!(
            driver.requests().is_empty(),
            "{command:?} asked for a browser"
        );
    }
}

#[test]
fn a_flag_of_a_subcommand_that_may_be_repeated_still_may() {
    let driver = FakeWebDriver::start();
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(
        dir.path(),
        &[],
        &[
            "--webdriver",
            driver.url(),
            "deferred",
            "--assert",
            "element=h1",
            "--assert",
            "title=Fake",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!driver.requests().is_empty(), "no browser was asked for");
}

#[cfg(feature = "plugins-builtin")]
#[test]
fn a_flag_of_the_root_that_may_be_repeated_still_may() {
    let (_dir, output) = in_a_new_folder(&[
        "--plugin",
        "shield",
        "--plugin",
        "block-resources",
        "plugin",
        "list",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn the_commands_that_already_refused_a_repeated_flag_still_do() {
    let (_dir, output) = in_a_new_folder(&["grid-hub", "status", "--wait", "1", "--wait", "2"]);
    assert_cannot_be_used_multiple_times(&refused(&output), "--wait");

    // usage names a flag of the root without its dashes when a subcommand follows it.
    let (_dir, output) = in_a_new_folder(&["--browser", "chrome", "--browser", "edge", "version"]);
    let error = refused(&output);
    assert!(
        error.contains("'browser'") && error.contains("multiple times"),
        "{error}"
    );
}

// ---------------------------------------------------------------------------
// A value written after a switch
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// `help` with a path that names no command
// ---------------------------------------------------------------------------

fn assert_unrecognized(output: &Output, word: &str) {
    let error = refused(output);
    assert!(
        error.contains(&format!("unrecognized subcommand '{word}'")),
        "{error}"
    );
}

#[test]
fn help_for_a_command_that_does_not_exist_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");

    assert_unrecognized(&run(dir.path(), &[], &["help", "nosuch"]), "nosuch");
}

#[test]
fn help_with_a_bad_path_is_refused_whichever_word_is_wrong() {
    let dir = tempfile::tempdir().expect("temp dir");

    assert_unrecognized(
        &run(dir.path(), &[], &["help", "plugin", "nosuch"]),
        "nosuch",
    );
    assert_unrecognized(
        &run(dir.path(), &[], &["help", "nosuch", "plugin"]),
        "nosuch",
    );
    assert_unrecognized(
        &run(dir.path(), &[], &["help", "plugin", "info", "extra"]),
        "extra",
    );
}

#[test]
fn help_for_a_command_that_exists_prints_its_page() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (path, text) in [
        (
            &["help", "mkdir"][..],
            "Create a folder with a runnable browser-test suite",
        ),
        (
            &["help", "plugin"],
            "List the built-in plugins, or show what one declares",
        ),
        (&["help", "plugin", "info"], "Show what one plugin declares"),
        (&["help", "new"], "Create a browser-test file"),
    ] {
        let output = run(dir.path(), &[], path);

        assert!(output.status.success(), "{path:?}: {}", stderr(&output));
        assert!(
            stdout(&output).contains(text),
            "{path:?}:\n{}",
            stdout(&output)
        );
    }
}

#[test]
fn help_alone_prints_the_main_page() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(dir.path(), &[], &["help"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Commands:"), "{}", stdout(&output));
}

#[test]
fn help_for_a_command_may_be_asked_for_with_the_help_flag() {
    let dir = tempfile::tempdir().expect("temp dir");
    for path in [
        &["help", "--help"][..],
        &["help", "help"],
        &["help", "mkdir", "--help"],
        &["help", "plugin", "info", "--help"],
    ] {
        let output = run(dir.path(), &[], path);

        assert!(output.status.success(), "{path:?}: {}", stderr(&output));
        assert!(!stdout(&output).is_empty(), "{path:?} printed nothing");
    }
    assert_unrecognized(
        &run(dir.path(), &[], &["help", "nosuch", "--help"]),
        "nosuch",
    );
}

#[test]
fn a_value_written_after_help_or_the_version_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    for args in [
        &["--help=false"][..],
        &["mkdir", "--help=1"],
        &["--version=1"],
    ] {
        let error = refused(&run(dir.path(), &[], args));

        assert!(error.contains("takes no value"), "{args:?}: {error}");
    }
}

#[test]
fn the_help_flag_still_answers_before_a_mistake_after_it() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(dir.path(), &[], &["mkdir", "--help", "--force", "--force"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("--force"), "{}", stdout(&output));
}
