//! `sbrs fingerprint capture` against a real Chrome, and a browser that presents a file.
//!
//! These tests start Chrome (and chromedriver), so they are ignored by default. Run them one
//! test binary at a time:
//!
//! ```text
//! cargo test -p seleniumbase-rs-cli --test cli_fingerprint_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/sbrs.rs"]
mod sbrs;

use sbrs::{run, stderr, stdout};

const PAGE: &str = "https://example.com";

#[test]
#[ignore = "starts a real Chrome"]
fn a_capture_is_a_fingerprint_file_that_checks_out_and_a_summary_on_standard_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("captured.json");
    let readings = dir.path().join("readings.json");

    let output = run(
        dir.path(),
        &[],
        &[
            "--headless",
            "fingerprint",
            "capture",
            PAGE,
            "-o",
            file.to_str().unwrap(),
            "--readings",
            readings.to_str().unwrap(),
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).is_empty(),
        "the fingerprint went to the file"
    );
    assert!(
        stderr(&output).contains("Captured Chrome"),
        "{}",
        stderr(&output)
    );

    let check = run(
        dir.path(),
        &[],
        &["fingerprint", "check", file.to_str().unwrap()],
    );
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));

    let readings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&readings).unwrap()).unwrap();
    assert_eq!(readings["probe"]["url"], "https://example.com/");
    assert!(readings["unmapped"].is_object());
}

#[test]
#[ignore = "starts a real Chrome"]
fn a_browser_started_with_a_fingerprint_presents_it() {
    let dir = tempfile::tempdir().unwrap();
    let expected = dir.path().join("expected.json");
    let seen = dir.path().join("seen.json");
    // Not this machine's own system, so a pass cannot be the browser's natural answer.
    let export = run(
        dir.path(),
        &[],
        &[
            "fingerprint",
            "export",
            "--random",
            "--seed",
            "11",
            "--os",
            "windows",
            "-o",
            expected.to_str().unwrap(),
        ],
    );
    assert!(export.status.success(), "{}", stderr(&export));

    let capture = run(
        dir.path(),
        &[],
        &[
            "--headless",
            "--fingerprint",
            expected.to_str().unwrap(),
            "fingerprint",
            "capture",
            PAGE,
            "-o",
            seen.to_str().unwrap(),
        ],
    );
    assert!(capture.status.success(), "{}", stderr(&capture));

    // What a page cannot be made to show in a headless Chrome that chromedriver started
    // (WebGL), what moves between readings (the connection numbers) and what no page can read
    // (the position) may differ; no other setting may.
    let diff = run(
        dir.path(),
        &[],
        &[
            "fingerprint",
            "diff",
            expected.to_str().unwrap(),
            seen.to_str().unwrap(),
        ],
    );
    let report = stdout(&diff);
    for line in report.lines().filter(|line| line.contains(": expected ")) {
        let setting = line.split(':').next().unwrap_or_default();
        assert!(
            ["webgl", "connection", "geolocation"].contains(&setting),
            "the browser does not present {setting}:\n{report}"
        );
    }
}
