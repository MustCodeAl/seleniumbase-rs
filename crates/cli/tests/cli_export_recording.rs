//! `sbrs export-recording`: an action file written by `sbrs record` becomes a
//! Rust test. It starts no browser, so the real binary runs here as it is.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

const ACTIONS: &str = r##"[
  {"name": "goto", "target": "https://example.test/", "value": null, "timestamp_ms": 1},
  {"name": "type", "target": "#q", "value": "rust\n", "timestamp_ms": 2},
  {"name": "goto_if_not_url", "target": "https://example.test/search?q=rust", "value": null, "timestamp_ms": 3},
  {"name": "click", "target": "#first", "value": null, "timestamp_ms": 4}
]"##;

fn sbrs(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .current_dir(dir)
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
        .output()
        .expect("sbrs runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn an_action_file_becomes_a_rust_test_beside_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.json"), ACTIONS).expect("write actions");

    let output = sbrs(dir.path(), &["export-recording", "login.json"]);

    let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("login.rs"), "{stdout}");
    let test = std::fs::read_to_string(dir.path().join("login.rs")).expect("login.rs");
    assert!(test.contains("#[tokio::test]\nasync fn login()"), "{test}");
    for line in [
        "sb.open(\"https://example.test/\").await?;",
        "sb.type_text(\"#q\", \"rust\\n\").await?;",
        "sb.goto_if_not_url(\"https://example.test/search?q=rust\").await?;",
        "sb.click(\"#first\").await?;",
    ] {
        assert!(test.contains(line), "{line:?} in\n{test}");
    }
    assert!(!test.contains("skipped"), "{test}");
}

#[test]
fn the_output_can_be_named_and_goes_into_a_new_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("a.json"), ACTIONS).expect("write actions");

    let output = sbrs(
        dir.path(),
        &["export-recording", "a.json", "-o", "tests/search_flow.rs"],
    );

    assert!(output.status.success(), "{}", text(&output.stderr));
    let test = std::fs::read_to_string(dir.path().join("tests/search_flow.rs")).expect("the test");
    assert!(test.contains("async fn search_flow()"), "{test}");
}

#[test]
fn an_empty_or_missing_action_file_is_an_error_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("empty.json"), "[]").expect("write actions");

    for file in ["empty.json", "missing.json"] {
        let output = sbrs(dir.path(), &["export-recording", file]);
        assert!(!output.status.success(), "{file}: {}", text(&output.stdout));
    }
    assert!(!dir.path().join("empty.rs").exists());
    assert!(!dir.path().join("missing.rs").exists());
}

#[test]
fn the_action_file_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("a.json"), ACTIONS).expect("write actions");

    let output = sbrs(dir.path(), &["export-recording", "a.json", "-o", "a.json"]);

    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.json")).unwrap(),
        ACTIONS
    );
}
