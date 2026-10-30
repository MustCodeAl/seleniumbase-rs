//! What `sbrs` writes to standard error as logs: one readable line, no colour
//! unless standard error is a terminal (these tests run with it piped, which
//! is what a CI log or `2> file` is), JSON lines with `SB_LOG_FORMAT=json`.
//!
//! A bad `SB_SHUTDOWN_TIMEOUT_SECS` makes `sbrs` log one warning before it
//! reads the command line, so `--version` is enough to see a log line and no
//! browser starts.

use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs --version` with a bad variable, so that it logs a warning.
fn warns(env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .arg("--version")
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env_remove("SB_LOG_LEVEL")
        .env_remove("SB_LOG_FORMAT")
        .env_remove("TERM")
        .env("SB_SHUTDOWN_TIMEOUT_SECS", "soon");
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("sbrs runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The log line of the warning.
fn warning_line(output: &Output) -> String {
    stderr(output)
        .lines()
        .find(|line| line.contains("ignored an invalid SB_* variable"))
        .unwrap_or_else(|| panic!("no warning in: {}", stderr(output)))
        .to_owned()
}

/// Whether `text` starts with a clock time, `12:34:56`, and what follows it.
fn after_clock(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let shape = b"dd:dd:dd";
    if bytes.len() < shape.len() {
        return None;
    }
    let fits = shape.iter().zip(bytes).all(|(want, got)| match want {
        b'd' => got.is_ascii_digit(),
        other => got == other,
    });
    fits.then(|| &text[shape.len()..])
}

#[test]
fn a_piped_log_line_is_readable_and_has_no_escape_sequence() {
    let output = warns(&[]);

    assert!(output.status.success(), "{}", stderr(&output));
    let line = warning_line(&output);
    assert!(!stderr(&output).contains('\x1b'), "{:?}", stderr(&output));
    let rest = after_clock(&line).unwrap_or_else(|| panic!("no clock at the start: {line:?}"));
    // Whole seconds and a level, then the module, then the message and the field.
    assert!(rest.starts_with("  WARN "), "{line:?}");
    assert!(
        rest.contains("ignored an invalid SB_* variable, its default is used problem="),
        "{line:?}"
    );
}

#[test]
fn the_default_line_has_no_span_noise_and_a_chosen_filter_brings_the_detail_back() {
    let plain = warning_line(&warns(&[]));
    // `info` by default: seconds only, and no crate path.
    assert!(
        !plain[..9].contains('.'),
        "no milliseconds by default: {plain:?}"
    );

    let chosen = warning_line(&warns(&[("SB_LOG_LEVEL", "warn")]));
    let rest = after_clock(&chosen).unwrap_or_else(|| panic!("no clock: {chosen:?}"));
    assert!(
        rest.starts_with('.') && rest[1..4].bytes().all(|b| b.is_ascii_digit()),
        "milliseconds when a filter was chosen: {chosen:?}"
    );
}

#[test]
fn no_color_and_a_dumb_terminal_change_nothing_on_a_pipe() {
    for env in [[("NO_COLOR", "1")], [("TERM", "dumb")]] {
        let output = warns(&env);
        assert!(
            !stderr(&output).contains('\x1b'),
            "{env:?}: {:?}",
            stderr(&output)
        );
        assert!(warning_line(&output).contains(" WARN "), "{env:?}");
    }
}

#[test]
fn json_logs_are_one_object_per_line_without_colour() {
    let output = warns(&[("SB_LOG_FORMAT", "json")]);

    let text = stderr(&output);
    assert!(!text.contains('\x1b'), "{text:?}");
    let line = warning_line(&output);
    let object: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|error| {
        panic!("not JSON ({error}): {line:?}");
    });
    assert_eq!(object["level"], "WARN");
    assert_eq!(
        object["fields"]["message"],
        "ignored an invalid SB_* variable, its default is used"
    );
}
