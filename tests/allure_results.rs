//! The Allure results of a run (`SB_ALLURE_DIR`, feature `allure`): one
//! `*-result.json` per test with its status, labels and failure details, and
//! the evidence of a failure as attachments. No browser is needed: the page the
//! recorder looks at is a fake.

#![cfg(feature = "allure")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use seleniumbase_rs::core::allure::{AllureSink, ENVIRONMENT_FILE};
use seleniumbase_rs::core::log_helper::TestId;
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, RunSink, TestRun};
use seleniumbase_rs::plugins::observer::PageEvidence;
use seleniumbase_rs::{Result, SeleniumBaseError};
use serde_json::{json, Value};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";
const NOW: i64 = 1_700_000_000_000;

/// A page with fixed answers.
struct Page;

#[async_trait]
impl PageEvidence for Page {
    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        Ok(PNG.to_vec())
    }

    async fn page_source(&self) -> Result<String> {
        Ok("<html><head><title>Login</title></head></html>".to_owned())
    }

    async fn current_url(&self) -> Result<String> {
        Ok("https://example.com/login".to_owned())
    }

    async fn page_title(&self) -> Result<String> {
        Ok("Login".to_owned())
    }

    async fn browser_label(&self) -> Option<String> {
        Some("Chrome 155.0.7000.1 / headless".to_owned())
    }

    async fn driver_label(&self) -> Option<String> {
        Some("WebDriver".to_owned())
    }
}

fn run_with_allure(root: &Path, folder: impl Into<PathBuf>) -> Arc<TestRun> {
    let settings = ReportSettings {
        allure_dir: Some(folder.into()),
        ..ReportSettings::default()
    };
    TestRun::open(root, settings, RunKey::new("allure")).unwrap()
}

/// Every `*-result.json` in `dir`, by `fullName`.
fn results(dir: &Path) -> Vec<(String, Value)> {
    let mut found: Vec<(String, Value)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with("-result.json"))
        .map(|path| {
            let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            (value["fullName"].as_str().unwrap().to_owned(), value)
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn result_of<'a>(all: &'a [(String, Value)], full_name: &str) -> &'a Value {
    &all.iter()
        .find(|(name, _)| name == full_name)
        .unwrap_or_else(|| panic!("no result for {full_name}"))
        .1
}

fn label<'a>(result: &'a Value, name: &str) -> Vec<&'a str> {
    result["labels"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|label| label["name"] == name)
        .map(|label| label["value"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_passing_a_failing_and_a_skipped_test_each_get_a_result() {
    let root = tempfile::tempdir().unwrap();
    let run = run_with_allure(root.path(), "allure-results");

    let recorder = run.start(TestId::named("shop::cart_adds_an_item")).unwrap();
    recorder.finish(&Ok(())).unwrap();

    let mut recorder = run.start(TestId::named("shop::checkout")).unwrap();
    let failed: Result<()> = Err(SeleniumBaseError::AssertionFailed(
        "the title is not Home\nsecond line".to_owned(),
    ));
    recorder
        .on_failure(failed.as_ref().unwrap_err(), &Page)
        .await;
    recorder.finish(&failed).unwrap();

    let recorder = run.start(TestId::named("shop::coupon")).unwrap();
    recorder
        .finish(&Err(SeleniumBaseError::Skipped(
            "no coupon today".to_owned(),
        )))
        .unwrap();

    let dir = root.path().join("allure-results");
    let all = results(&dir);
    assert_eq!(all.len(), 3, "one result per test: {all:#?}");

    let passed = result_of(&all, "shop::cart_adds_an_item");
    assert_eq!(passed["status"], "passed");
    assert_eq!(passed["name"], "cart_adds_an_item");
    assert!(passed.get("statusDetails").is_none());
    assert_eq!(passed["attachments"], json!([]));
    assert_eq!(label(passed, "suite"), ["shop"]);
    assert_eq!(label(passed, "framework"), ["seleniumbase-rs"]);
    assert_eq!(label(passed, "language"), ["rust"]);
    assert_eq!(label(passed, "testMethod"), ["cart_adds_an_item"]);
    assert_eq!(label(passed, "testId"), ["shop.cart_adds_an_item"]);
    assert!(passed["start"].as_i64().unwrap() <= passed["stop"].as_i64().unwrap());

    let failed = result_of(&all, "shop::checkout");
    assert_eq!(failed["status"], "failed");
    assert_eq!(
        failed["statusDetails"]["message"],
        "assertion failed: the title is not Home"
    );
    let trace = failed["statusDetails"]["trace"].as_str().unwrap();
    assert!(trace.contains("the title is not Home"), "trace: {trace}");
    assert!(failed["description"]
        .as_str()
        .unwrap()
        .contains("https://example.com/login"));
    assert_eq!(
        failed["parameters"],
        json!([
            { "name": "browser", "value": "Chrome 155.0.7000.1 / headless" },
            { "name": "driver", "value": "WebDriver" },
        ])
    );
    let attachments = failed["attachments"].as_array().unwrap();
    let described: Vec<(&str, &str)> = attachments
        .iter()
        .map(|a| (a["name"].as_str().unwrap(), a["type"].as_str().unwrap()))
        .collect();
    assert_eq!(
        described,
        [
            ("screenshot.png", "image/png"),
            ("page_source.html", "text/html"),
            ("basic_test_info.txt", "text/plain"),
        ]
    );
    for attachment in attachments {
        let copy = dir.join(attachment["source"].as_str().unwrap());
        assert!(copy.is_file(), "{} is missing", copy.display());
    }
    let screenshot = dir.join(attachments[0]["source"].as_str().unwrap());
    assert_eq!(fs::read(screenshot).unwrap(), PNG);

    let skipped = result_of(&all, "shop::coupon");
    assert_eq!(skipped["status"], "skipped");
    assert_eq!(
        skipped["statusDetails"],
        json!({ "message": "no coupon today" })
    );

    let environment = fs::read_to_string(dir.join(ENVIRONMENT_FILE)).unwrap();
    assert!(environment.starts_with("framework=seleniumbase-rs\n"));
}

#[tokio::test]
async fn a_test_that_never_reports_is_broken() {
    let root = tempfile::tempdir().unwrap();
    let run = run_with_allure(root.path(), "allure-results");

    let recorder = run.start(TestId::named("shop::killed")).unwrap();
    drop(recorder);

    let all = results(&root.path().join("allure-results"));
    let broken = result_of(&all, "shop::killed");
    assert_eq!(broken["status"], "broken");
    assert_eq!(
        broken["statusDetails"]["message"],
        "the test did not finish"
    );
}

#[tokio::test]
async fn a_test_that_ends_on_another_error_is_broken_not_failed() {
    let root = tempfile::tempdir().unwrap();
    let run = run_with_allure(root.path(), "allure-results");

    let recorder = run.start(TestId::named("shop::config")).unwrap();
    recorder
        .finish(&Err(SeleniumBaseError::InvalidConfig(
            "no such file".to_owned(),
        )))
        .unwrap();

    let all = results(&root.path().join("allure-results"));
    assert_eq!(result_of(&all, "shop::config")["status"], "broken");
}

#[tokio::test]
async fn a_second_sink_on_the_same_folder_does_not_record_a_test_twice() {
    let root = tempfile::tempdir().unwrap();
    let run = run_with_allure(root.path(), "allure-results");
    let dir = root.path().join("allure-results");
    run.add_sink(Arc::new(AllureSink::new(&dir).unwrap()) as Arc<dyn RunSink>);

    let recorder = run.start(TestId::named("shop::once")).unwrap();
    recorder.finish(&Ok(())).unwrap();

    assert_eq!(results(&dir).len(), 1, "two sinks, one folder, one result");
}

#[tokio::test]
async fn a_run_without_a_results_folder_writes_no_allure_files() {
    let root = tempfile::tempdir().unwrap();
    let run = TestRun::open(root.path(), ReportSettings::default(), RunKey::new("none")).unwrap();

    let recorder = run.start(TestId::named("shop::plain")).unwrap();
    recorder.finish(&Ok(())).unwrap();

    assert!(!root.path().join("allure-results").exists());
}

#[tokio::test]
async fn an_absolute_results_folder_is_used_as_it_is() {
    let (root, elsewhere) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = elsewhere.path().join("results");
    let run = run_with_allure(root.path(), &dir);

    let recorder = run.start(TestId::named("shop::far")).unwrap();
    recorder.finish(&Ok(())).unwrap();

    assert_eq!(results(&dir).len(), 1);
}

/// The whole file, with the time and the uuid fixed by the injected sources.
#[tokio::test]
async fn a_result_matches_its_golden_json() {
    let root = tempfile::tempdir().unwrap();
    let run = TestRun::open(
        root.path(),
        ReportSettings::default(),
        RunKey::new("golden"),
    )
    .unwrap();
    let dir = root.path().join("results");
    let sink = AllureSink::with_sources(
        &dir,
        Arc::new(|| NOW),
        Arc::new(|| "00000000-0000-4000-8000-00000000000a".to_owned()),
    )
    .unwrap();
    run.add_sink(Arc::new(sink) as Arc<dyn RunSink>);

    let recorder = run.start(TestId::named("shop::checkout")).unwrap();
    recorder.finish(&Ok(())).unwrap();

    let all = results(&dir);
    let mut result = result_of(&all, "shop::checkout").clone();
    // The duration is the recorder's own, so the start moves; the stop is fixed.
    let start = result.as_object_mut().unwrap().remove("start").unwrap();
    assert!((NOW - 60_000..=NOW).contains(&start.as_i64().unwrap()));
    assert_eq!(
        result,
        json!({
            "uuid": "00000000-0000-4000-8000-00000000000a",
            "historyId": "0c566b9619aa195832390c12f35a318f",
            "testCaseId": "0c566b9619aa195832390c12f35a318f",
            "fullName": "shop::checkout",
            "name": "checkout",
            "status": "passed",
            "stage": "finished",
            "stop": NOW,
            "labels": [
                { "name": "framework", "value": "seleniumbase-rs" },
                { "name": "language", "value": "rust" },
                { "name": "suite", "value": "shop" },
                { "name": "package", "value": "shop" },
                { "name": "testClass", "value": "shop" },
                { "name": "testMethod", "value": "checkout" },
                { "name": "testId", "value": "shop.checkout" },
            ],
            "parameters": [],
            "attachments": [],
            "steps": [],
            "links": [],
        })
    );
}
