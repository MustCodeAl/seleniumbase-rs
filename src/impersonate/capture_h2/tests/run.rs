//! Writing a run's result into the measured file, one pass or both, without a browser.

use std::path::Path;

use super::{all_h2, entry, evidence, version, write_base, BASE_JSON};
use crate::impersonate::capture_h2::browser::{all_probes_seen, launch_options};
use crate::impersonate::capture_h2::{same_browser, write_result, CaptureH2Error};
use crate::scrape::runtime::capture::args::ToolArgs;
use crate::scrape::runtime::capture::chrome::ChromeOptions;

const FULL: &str = "155.0.1.2";

fn args(dir: &Path) -> ToolArgs {
    let mut args = ToolArgs::parse(Vec::<String>::new(), dir).expect("default arguments");
    args.out_dir = dir.to_path_buf();
    args.fixtures_dir = dir.join("fixtures");
    args
}

fn saved(dir: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(dir.join(entry().file_name())).expect("the file");
    serde_json::from_str(&text).expect("valid JSON")
}

#[test]
fn both_passes_write_the_evidence_the_specs_and_the_fixtures() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    write_base(dir.path(), BASE_JSON);
    let (h2, h1) = evidence();

    let report = write_result(
        &args(dir.path()),
        version(FULL),
        Some(h2),
        Some(h1),
        "capture_h2 9.9.9",
        chrono::Utc::now(),
    )
    .expect("writes");

    assert!(report.derived.is_some());
    assert_eq!(report.fixtures.len(), 5);
    assert_eq!(report.path, dir.path().join(entry().file_name()));
    let file = saved(dir.path());
    assert!(file["http2"].is_object() && file["headers"].is_object());
    assert_eq!(
        file["capture"]["h2"]["probes"].as_array().map(Vec::len),
        Some(5)
    );
    assert_eq!(
        file["capture"]["h1"]["probes"].as_array().map(Vec::len),
        Some(5)
    );
}

#[test]
fn a_single_pass_keeps_the_stored_evidence_of_the_other() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    write_base(dir.path(), BASE_JSON);
    let (h2, h1) = evidence();

    let first = write_result(
        &args(dir.path()),
        version(FULL),
        Some(h2.clone()),
        None,
        "capture_h2 9.9.9",
        chrono::Utc::now(),
    )
    .expect("writes the HTTP/2 pass");
    assert!(
        first.derived.is_none(),
        "one pass cannot derive the header spec"
    );
    assert!(first.h1.is_none());
    assert_eq!(first.fixtures.len(), 5);
    assert!(saved(dir.path()).get("http2").is_none());

    let second = write_result(
        &args(dir.path()),
        version(FULL),
        None,
        Some(h1),
        "capture_h2 9.9.9",
        chrono::Utc::now(),
    )
    .expect("writes the HTTP/1.1 pass");
    assert!(
        second.derived.is_some(),
        "the stored HTTP/2 evidence is used"
    );
    assert_eq!(second.h2, Some(h2));
    assert!(
        second.fixtures.is_empty(),
        "the HTTP/2 pass did not run again"
    );
    assert!(saved(dir.path())["http2"].is_object());
}

#[test]
fn a_run_without_a_base_file_or_for_another_version_writes_nothing() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let (h2, h1) = evidence();
    let run = |full: &str| {
        write_result(
            &args(dir.path()),
            version(full),
            Some(h2.clone()),
            Some(h1.clone()),
            "capture_h2 9.9.9",
            chrono::Utc::now(),
        )
    };

    assert!(matches!(run(FULL), Err(CaptureH2Error::NoBase { .. })));
    write_base(dir.path(), BASE_JSON);
    assert!(matches!(
        run("155.0.9.9"),
        Err(CaptureH2Error::VersionMismatch { .. })
    ));
    assert!(saved(dir.path()).get("http2").is_none());
    assert!(!dir.path().join("fixtures").exists());
}

#[test]
fn two_passes_must_have_seen_the_same_browser() {
    assert!(same_browser(&version(FULL), &version(FULL)).is_ok());

    let error = same_browser(&version(FULL), &version("155.0.9.9")).expect_err("two builds");

    assert!(
        matches!(&error, CaptureH2Error::DifferentBrowsers { first, second }
            if first == FULL && second == "155.0.9.9"),
        "{error}"
    );
}

#[test]
fn a_pass_is_over_when_every_probe_has_been_seen() {
    let mut observed = all_h2();
    assert!(all_probes_seen(&observed));

    observed.pop();
    assert!(!all_probes_seen(&observed));
    assert!(!all_probes_seen(&[]));
}

#[test]
fn an_http1_request_with_another_destination_is_refused() {
    let mut headers = super::h2_headers(super::IMAGE, 4433);
    headers.retain(|(name, _)| name != "sec-fetch-dest");
    let mut observed = super::all_h1();
    observed.retain(|one| one.probe != super::IMAGE);
    observed.push(super::h1_observed(super::IMAGE, &headers));

    let error = crate::impersonate::capture_h2::h1_evidence(&observed).expect_err("no destination");

    assert!(
        matches!(&error, CaptureH2Error::WrongDestination { probe: "image", found } if found == "none"),
        "{error}"
    );
}

#[test]
fn a_measured_file_over_the_size_limit_is_not_read() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join(entry().file_name());
    let file = std::fs::File::create(&path).expect("creates");
    file.set_len(16 * 1024 * 1024 + 1).expect("sets the length");

    let outcome =
        crate::impersonate::capture_h2::MeasuredDoc::open(dir.path(), entry(), &version(FULL));

    assert!(
        matches!(outcome, Err(CaptureH2Error::Store { .. })),
        "{outcome:?}"
    );
}

#[test]
fn the_browser_is_started_with_the_extra_arguments() {
    let options = ChromeOptions::default();

    assert!(launch_options(&options, &["--mute-audio".to_owned()]).is_ok());
    let with_binary = ChromeOptions {
        executable: Some("/nonexistent/chrome".into()),
        no_sandbox: true,
        ..options
    };
    assert!(launch_options(&with_binary, &[]).is_ok());
}
