//! `sbrs mkrec` (also `codegen` and `record`): the Recorder from the command line, run as the
//! real binary against a fake WebDriver server (no browser).
//!
//! The fake holds a recording in its page, as the Recorder's page script would after a
//! person clicked and typed, so the whole path runs: the browser starts in Recorder Mode, the
//! breakpoint (or `--ee`) ends the wait, the recording is written under `recordings/` and
//! copied over the file asked for. `tests/cli_mkrec_chrome.rs` does it in a real Chrome.

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "support/sbrs.rs"]
mod support;

use std::io::Write;
use std::path::Path;
use std::process::{Output, Stdio};

use fake_webdriver::FakeWebDriver;
use support::{command_in, stderr, stdout, SBRS};

/// What the page script keeps after a person opened a page, clicked `#go` and typed `hello`
/// into `#q`. The times are well after the browser starts, so none of the actions is older
/// than the recording.
fn recorded_actions() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock")
        .as_millis();
    let at = |after: u128| now + 600_000 + after;
    format!(
        r##"[["begin","https://example.test","https://example.test/",{}],["click","#go","",{}],["input","#q","hello",{}]]"##,
        at(0),
        at(500),
        at(1_000)
    )
}

/// Runs `sbrs --webdriver <fake> args...` in `dir`, with `input` on standard input (none:
/// a closed one).
fn mkrec(dir: &Path, driver: &FakeWebDriver, args: &[&str], input: Option<&str>) -> Output {
    let mut command = command_in(SBRS, dir, &[]);
    command
        .arg("--webdriver")
        .arg(driver.url())
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("sbrs runs");
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input.as_bytes())
            .expect("write the answer");
    }
    child.wait_with_output().expect("sbrs ends")
}

/// A folder to run in and a fake browser whose pages hold a recording.
fn setup() -> (tempfile::TempDir, FakeWebDriver) {
    (
        tempfile::tempdir().expect("temp dir"),
        FakeWebDriver::start_with_recording(&recorded_actions()),
    )
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

#[test]
fn mkrec_records_to_the_recordings_folder_and_copies_the_test_to_the_file() {
    let (dir, driver) = setup();

    let output = mkrec(
        dir.path(),
        &driver,
        &["mkrec", "new_test.rs", "--url", "https://example.test/"],
        Some("c\n"),
    );

    let out = stdout(&output);
    assert!(output.status.success(), "{out}\n{}", stderr(&output));
    assert!(
        out.contains("* RECORDING initialized: new_test.rs"),
        "{out}"
    );
    assert!(
        out.contains(">>> RECORDING SAVED as: recordings/new_test_rec.rs"),
        "{out}"
    );
    assert!(
        out.contains("*** RECORDING COPIED to: new_test.rs"),
        "{out}"
    );
    // The code is shown with line numbers before the copy is announced.
    assert!(out.contains("   1  "), "{out}");
    assert!(
        stderr(&output).contains("RECORDER BREAKPOINT"),
        "{}",
        stderr(&output)
    );

    let recorded = read(dir.path(), "recordings/new_test_rec.rs");
    for call in [
        "sb.goto(\"https://example.test/\")",
        "sb.click(\"#go\")",
        "sb.type_text(\"#q\", \"hello\")",
    ] {
        assert!(recorded.contains(call), "{call} in:\n{recorded}");
    }
    assert_eq!(read(dir.path(), "new_test.rs"), recorded);
    let json = read(dir.path(), "recordings/new_test_rec.json");
    assert!(json.contains("\"click\""), "{json}");
    // The browser was asked to record, and the start page was opened.
    let sent = driver.requests();
    assert!(sent.iter().any(|seen| seen.path == "/goog/cdp/execute"));
    assert!(sent
        .iter()
        .any(|seen| seen.path == "/url" && seen.method == "POST"));
}

#[test]
fn codegen_and_record_do_what_mkrec_does() {
    for name in ["codegen", "record"] {
        let (dir, driver) = setup();

        let output = mkrec(dir.path(), &driver, &[name, "x.rs"], Some("c\n"));

        assert!(output.status.success(), "{name}: {}", stderr(&output));
        assert!(
            stdout(&output).contains("*** RECORDING COPIED to: x.rs"),
            "{name}: {}",
            stdout(&output)
        );
        assert_eq!(
            read(dir.path(), "x.rs"),
            read(dir.path(), "recordings/x_rec.rs"),
            "{name}"
        );
    }
}

#[test]
fn a_file_below_the_current_folder_is_made_where_it_is_asked_for() {
    let (dir, driver) = setup();

    let output = mkrec(
        dir.path(),
        &driver,
        &["mkrec", "tests/login.rs"],
        Some("c\n"),
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(read(dir.path(), "tests/login.rs").contains("sb.click(\"#go\")"));
    // The recording keeps the name of the file, not its folder.
    assert!(dir.path().join("recordings/login_rec.rs").is_file());
}

#[test]
fn a_bad_file_name_is_refused_with_status_1_before_a_browser_starts() {
    for (name, message) in [
        ("new_test.py", "File name must end with \".rs\"!"),
        ("https://example.test/", "File name must end with \".rs\"!"),
        ("*.rs", "Invalid file name!"),
        ("../x.rs", "File must be created in the current directory!"),
        (
            "/tmp/x.rs",
            "File must be created in the current directory!",
        ),
    ] {
        let (dir, driver) = setup();

        let output = mkrec(dir.path(), &driver, &["mkrec", name], Some("c\n"));

        assert_eq!(output.status.code(), Some(1), "{name}: {}", stderr(&output));
        assert!(
            stderr(&output).contains(&format!("ERROR: {message}")),
            "{name}: {}",
            stderr(&output)
        );
        assert!(
            driver.requests().is_empty(),
            "{name}: a browser was asked for"
        );
        assert!(!dir.path().join("recordings").exists(), "{name}");
    }
}

#[test]
fn the_old_record_arguments_are_a_file_name_error_now() {
    let (dir, driver) = setup();

    let output = mkrec(
        dir.path(),
        &driver,
        &["record", "https://example.test/"],
        None,
    );

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("File name must end with"));
    // And the options of the old command are not accepted.
    let output = mkrec(
        dir.path(),
        &driver,
        &["record", "x.rs", "-o", "a.json"],
        None,
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

#[test]
fn export_recording_is_not_a_command_any_more() {
    let (dir, driver) = setup();

    let output = mkrec(dir.path(), &driver, &["export-recording", "a.json"], None);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("export-recording"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_existing_file_is_refused_and_overwrite_replaces_it_after_a_recording() {
    let (dir, driver) = setup();
    std::fs::write(dir.path().join("taken.rs"), "my own work").expect("write");

    let refused = mkrec(dir.path(), &driver, &["mkrec", "taken.rs"], Some("c\n"));

    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert!(
        stderr(&refused).contains("ERROR: File \"taken.rs\" already exists in this directory!"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(read(dir.path(), "taken.rs"), "my own work");
    assert!(driver.requests().is_empty());

    let replaced = mkrec(
        dir.path(),
        &driver,
        &["mkrec", "taken.rs", "--overwrite"],
        Some("c\n"),
    );

    assert!(replaced.status.success(), "{}", stderr(&replaced));
    assert!(read(dir.path(), "taken.rs").contains("sb.click(\"#go\")"));
}

#[test]
fn behave_also_writes_the_feature_and_says_where_it_is() {
    for flag in ["--behave", "--gherkin", "--rec-behave"] {
        let (dir, driver) = setup();

        let output = mkrec(dir.path(), &driver, &["mkrec", "b.rs", flag], Some("c\n"));

        let out = stdout(&output);
        assert!(
            output.status.success(),
            "{flag}: {out}\n{}",
            stderr(&output)
        );
        assert!(
            out.contains("*** BEHAVE RECORDING at: recordings/features/b_rec.feature"),
            "{flag}: {out}"
        );
        let feature = read(dir.path(), "recordings/features/b_rec.feature");
        assert!(
            feature.contains("Feature: RecorderTest"),
            "{flag}: {feature}"
        );
        assert!(
            feature.contains("Scenario: test_recording"),
            "{flag}: {feature}"
        );
        assert!(
            feature.contains("Given Open \"https://example.test/\""),
            "{flag}: {feature}"
        );
        assert!(feature.contains("Click \"#go\""), "{flag}: {feature}");
    }
}

#[test]
fn the_recording_can_be_a_script_or_pure_cdp_code() {
    let (dir, driver) = setup();
    let script = mkrec(
        dir.path(),
        &driver,
        &["mkrec", "s.rs", "--rec-sb-mgr"],
        Some("c\n"),
    );
    assert!(script.status.success(), "{}", stderr(&script));
    let code = read(dir.path(), "s.rs");
    assert!(code.contains("#[tokio::main]"), "{code}");
    assert!(code.contains("BaseCase"), "{code}");

    let (dir, driver) = setup();
    let cdp = mkrec(
        dir.path(),
        &driver,
        &["mkrec", "c.rs", "--rec-sb-cdp"],
        Some("c\n"),
    );
    assert!(cdp.status.success(), "{}", stderr(&cdp));
    let code = read(dir.path(), "c.rs");
    assert!(code.contains("sb_cdp"), "{code}");
    assert!(code.contains("page.locator(\"#go\").click()"), "{code}");
}

#[test]
fn ee_waits_for_shift_esc_and_never_asks_at_a_prompt() {
    let (dir, driver) = setup();

    // No input at all: a prompt would end at once, but this must not be one.
    let output = mkrec(dir.path(), &driver, &["mkrec", "e.rs", "--ee"], None);

    let out = stdout(&output);
    assert!(output.status.success(), "{out}\n{}", stderr(&output));
    assert!(
        out.contains(">>> Use [SHIFT + ESC] in the browser to end recording!"),
        "{out}"
    );
    assert!(
        !stderr(&output).contains("RECORDER BREAKPOINT"),
        "{}",
        stderr(&output)
    );
    assert!(read(dir.path(), "e.rs").contains("sb.click(\"#go\")"));
}

#[test]
fn a_browser_that_is_not_chromium_is_refused() {
    let (dir, driver) = setup();

    let output = mkrec(
        dir.path(),
        &driver,
        &["--browser", "firefox", "mkrec", "f.rs"],
        Some("c\n"),
    );

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Recorder Mode ONLY supports Chrome and Edge"),
        "{}",
        stderr(&output)
    );
    assert!(!dir.path().join("f.rs").exists());
}

#[test]
fn nothing_recorded_writes_no_file_and_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    // A page that holds no actions.
    let driver = FakeWebDriver::start_with_recording("[]");

    let output = mkrec(dir.path(), &driver, &["mkrec", "n.rs"], Some("c\n"));

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("nothing was recorded"),
        "{}",
        stderr(&output)
    );
    assert!(!dir.path().join("n.rs").exists());
}

#[test]
fn a_recording_replays_with_sbrs_replay() {
    let (dir, driver) = setup();
    let made = mkrec(dir.path(), &driver, &["mkrec", "r.rs"], Some("c\n"));
    assert!(made.status.success(), "{}", stderr(&made));

    let replayed = mkrec(
        dir.path(),
        &driver,
        &["replay", "recordings/r_rec.json"],
        None,
    );

    let out = stdout(&replayed);
    assert!(replayed.status.success(), "{out}\n{}", stderr(&replayed));
    assert!(out.contains("0 failed, 0 skipped"), "{out}");
    assert!(out.contains("3 passed") || out.contains("passed"), "{out}");
}
