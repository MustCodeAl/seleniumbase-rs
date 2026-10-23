//! A command line that does not parse prints the page of the command its words reached (usage,
//! arguments, flags, examples) and then the error, as Python's `sbase` does, instead of the
//! bare parser error. Each case runs the real `sbrs` binary; nothing here starts a browser.
//!
//! Help and version requests, and the completion request, are not failures and stay as they
//! were: standard output, status 0. Run with
//! `cargo test -p seleniumbase-rs-cli --test cli_failure_pages`.

#[path = "support/sbrs.rs"]
mod sbrs;

use std::process::Output;

use sbrs::{run, stderr, stdout};

/// Runs `sbrs args...` with `env` set, in a folder of its own.
fn sbrs_with(env: &[(&str, &str)], args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("temp dir");
    run(dir.path(), env, args)
}

/// What the process said on standard error, after checking it was refused as clap refused a
/// bad command line: status 2 and nothing on standard output.
fn refused(output: &Output) -> String {
    let error = stderr(output);
    assert_eq!(output.status.code(), Some(2), "{error}");
    assert!(stdout(output).is_empty(), "{}", stdout(output));
    error
}

/// Checks that `error` is a failure page: the `Usage:` line of `command`, its `Examples:`
/// and then the `error:` line, in that order, and nothing of the page after the error.
fn assert_page_then_error(error: &str, command: &str) {
    let usage = error
        .find(&format!("Usage: {command}"))
        .unwrap_or_else(|| panic!("no `Usage: {command}` in:\n{error}"));
    let examples = error
        .find("Examples:")
        .unwrap_or_else(|| panic!("no Examples in:\n{error}"));
    let failure = error
        .find("error: ")
        .unwrap_or_else(|| panic!("no error line in:\n{error}"));
    assert!(usage < examples && examples < failure, "{error}");
    assert!(!error[failure..].contains("Usage:"), "{error}");
    assert!(!error.contains("For more information"), "{error}");
    assert_eq!(error.matches("Usage:").count(), 1, "{error}");
}

#[test]
fn mkrec_with_global_options_and_no_file_prints_the_mkrec_page_then_the_missing_file() {
    // `--uc` and `--url` are global options now, so they parse after `mkrec`; what is wrong is
    // the missing FILE.
    let error = refused(&sbrs_with(&[], &["mkrec", "--uc", "--url=google.com"]));

    assert_page_then_error(&error, "sbrs mkrec");
    assert!(error.contains("required"), "{error}");
}

#[test]
fn an_unknown_flag_prints_the_page_of_the_command_then_the_flag() {
    let error = refused(&sbrs_with(&[], &["mkrec", "--nosuch", "a.rs"]));

    assert_page_then_error(&error, "sbrs mkrec");
    assert!(error.contains("unexpected argument '--nosuch'"), "{error}");
}

#[test]
fn a_missing_argument_prints_the_page_of_the_command() {
    for (args, command) in [
        (&["mkfile"][..], "sbrs mkfile"),
        (&["grid-hub"], "sbrs grid-hub"),
        (&["get-text"], "sbrs get-text"),
        (&["plugin", "info"], "sbrs plugin info"),
    ] {
        let error = refused(&sbrs_with(&[], args));

        assert_page_then_error(&error, command);
        assert!(error.contains("required"), "{args:?}: {error}");
    }
}

#[test]
fn a_command_that_wants_a_subcommand_prints_its_page() {
    let error = refused(&sbrs_with(&[], &["plugin"]));

    assert!(error.contains("Usage: sbrs plugin"), "{error}");
    assert!(error.contains("Examples:"), "{error}");
    assert!(error.contains("sbrs plugin list"), "{error}");
}

#[test]
fn the_page_of_a_command_leaves_out_the_list_of_global_flags() {
    let error = refused(&sbrs_with(&[], &["get-text"]));

    assert!(!error.contains("--proxy-pac-url"), "{error}");
    assert!(error.contains("sbrs --help"), "{error}");
    assert!(error.lines().count() < 25, "{error}");
}

#[test]
fn an_unknown_command_prints_the_commands_by_group_and_the_error() {
    let error = refused(&sbrs_with(&[("COLUMNS", "80")], &["nosuch"]));

    assert!(
        error.starts_with("Usage: sbrs [FLAGS] <COMMAND>"),
        "{error}"
    );
    for heading in [
        "Page commands:",
        "Files and generators:",
        "Record and replay:",
        "Tests:",
        "Drivers, grid and servers:",
        "Import and convert:",
        "Security:",
        "Info and setup:",
    ] {
        assert!(error.contains(heading), "no {heading} in:\n{error}");
    }
    assert!(error.contains("mkfile"), "{error}");
    assert!(error.contains("sbrs help <COMMAND>"), "{error}");
    assert!(
        error
            .trim_end()
            .ends_with("error: unrecognized subcommand 'nosuch'"),
        "{error}"
    );
    assert!(
        error.lines().count() <= 30,
        "{} lines:\n{error}",
        error.lines().count()
    );
    assert!(error.lines().all(|line| line.len() <= 80), "{error}");
}

#[test]
fn a_near_miss_of_a_command_name_gets_usages_suggestion_after_the_page() {
    let error = refused(&sbrs_with(&[], &["mkfil"]));

    assert!(
        error.starts_with("Usage: sbrs [FLAGS] <COMMAND>"),
        "{error}"
    );
    assert!(
        error.contains("error: unrecognized subcommand 'mkfil'"),
        "{error}"
    );
    assert!(error.contains("mkfile"), "{error}");
    let tip = error.find("tip:").expect("a tip");
    assert!(tip > error.find("error: ").expect("the error"), "{error}");
}

#[test]
fn no_words_print_the_command_list_and_exit_with_status_2() {
    let error = refused(&sbrs_with(&[], &[]));

    assert!(
        error.starts_with("Usage: sbrs [FLAGS] <COMMAND>"),
        "{error}"
    );
    assert!(error.contains("Page commands:"), "{error}");
    assert!(!error.contains("error: "), "{error}");
}

#[test]
fn a_refusal_after_parsing_prints_the_page_of_the_command_then_the_reason() {
    let error = refused(&sbrs_with(&[], &["get-text", "--css", "h1"]));

    assert_page_then_error(&error, "sbrs get-text");
    assert!(
        error
            .trim_end()
            .lines()
            .last()
            .is_some_and(|line| { line.starts_with("error: get-text acts on a page") }),
        "{error}"
    );
}

#[test]
fn a_command_that_opens_its_own_page_refuses_a_start_page_with_its_page_above() {
    let error = refused(&sbrs_with(
        &[],
        &["open", "https://a.test/", "--url", "https://b.test/"],
    ));

    assert_page_then_error(&error, "sbrs open");
    assert!(error.contains("error: open opens its own URL"), "{error}");
}

#[test]
fn a_repeated_global_flag_is_refused_with_the_page() {
    let error = refused(&sbrs_with(&[], &["--headless", "get-title", "--headless"]));

    assert_page_then_error(&error, "sbrs get-title");
    assert!(error.contains("multiple times"), "{error}");
}

#[test]
fn help_and_version_are_unchanged() {
    for args in [
        &["--help"][..],
        &["mkfile", "--help"],
        &["help", "mkfile"],
        &["-h"],
    ] {
        let output = sbrs_with(&[], args);

        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(stderr(&output).is_empty(), "{args:?}: {}", stderr(&output));
        assert!(stdout(&output).contains("Usage:"), "{args:?}");
    }
    let page = stdout(&sbrs_with(&[], &["mkfile", "--help"]));
    assert!(page.contains("Examples:"), "{page}");
    assert!(page.contains("sbrs mkfile tests/login.rs"), "{page}");

    let version = sbrs_with(&[], &["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert!(
        stdout(&version).starts_with("sbrs "),
        "{}",
        stdout(&version)
    );
    assert!(stderr(&version).is_empty());
}

#[test]
fn every_command_help_has_examples() {
    let root = stdout(&sbrs_with(&[], &["--help"]));
    assert!(root.contains("Page commands"), "{root}");
    for command in [
        "get-text",
        "open",
        "test",
        "install",
        "mkdir",
        "mkrec",
        "plugin",
        "plugin list",
    ] {
        let words: Vec<&str> = command.split(' ').chain(["--help"]).collect();
        let page = stdout(&sbrs_with(&[], &words));
        assert!(page.contains("Examples:"), "{command}:\n{page}");
        assert!(page.contains("$ sbrs "), "{command}:\n{page}");
    }
}

#[test]
fn the_completion_request_is_answered_and_not_paged() {
    let output = sbrs_with(
        &[],
        &["__complete_word__", "--shell", "bash", "--line", "sbrs pl"],
    );

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("plugin"), "{}", stdout(&output));
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
}

#[test]
fn no_color_keeps_escapes_out_of_the_page_and_the_error() {
    let error = refused(&sbrs_with(&[("NO_COLOR", "1")], &["mkfile"]));

    assert!(!error.contains('\u{1b}'), "{error:?}");
    assert_page_then_error(&error, "sbrs mkfile");
}

#[test]
fn a_forced_colour_reaches_the_page_and_the_error_is_still_last() {
    let error = refused(&sbrs_with(&[("CLICOLOR_FORCE", "1")], &["mkfile"]));

    assert!(error.contains("\u{1b}["), "{error:?}");
    let plain = {
        let mut text = String::new();
        let mut chars = error.chars();
        while let Some(letter) = chars.next() {
            if letter == '\u{1b}' {
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                text.push(letter);
            }
        }
        text
    };
    assert_page_then_error(&plain, "sbrs mkfile");
}
