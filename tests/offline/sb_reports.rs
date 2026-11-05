//! The reports of the `sb_cdp` fixture on a mocked browser: a test is started
//! in the run before its browser launches and ended with its result, so a live
//! dashboard shows it while it runs. No Chrome starts;
//! `tests/sb_reports_chrome.rs` does the same on a real one, and
//! `tests/sb_reports_env.rs` checks that the run configuration of the
//! environment turns the reports on by itself.
#![cfg(all(feature = "testing", feature = "test-util"))]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use base64::prelude::*;
use seleniumbase_rs::config::test_settings::TestSettings;
use seleniumbase_rs::core::report_helper::{TestResult, TestStatus};
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::testing::SbCdp;
use seleniumbase_rs::{Result, SeleniumBaseError, Settings};
use serde_json::json;

const PNG: &[u8] = b"\x89PNG-from-the-mock";

/// A mocked browser whose tab answers like the page `https://shop.test/cart`.
async fn mocked() -> Result<Browser> {
    let (browser, mock) = Browser::new_mocked();
    mock.reply(
        "Runtime.evaluate",
        json!({ "result": { "value": "https://shop.test/cart" } }),
    );
    mock.reply(
        "Page.captureScreenshot",
        json!({ "data": BASE64_STANDARD.encode(PNG) }),
    );
    Ok(browser)
}

/// A new run under `root` with the report settings `change` makes.
fn open(root: &Path, change: impl FnOnce(&mut ReportSettings)) -> Arc<TestRun> {
    let mut settings = ReportSettings::default();
    change(&mut settings);
    TestRun::open(root, settings, RunKey::new("sb-reports")).unwrap()
}

/// A fixture that records its test in `run`.
fn fixture(run: &Arc<TestRun>, change: impl FnOnce(&mut TestSettings)) -> SbCdp {
    let mut test = TestSettings::default();
    change(&mut test);
    SbCdp::new(Settings {
        test,
        ..Settings::default()
    })
    .report_to(Arc::clone(run))
}

/// The one result the run has.
fn only(run: &TestRun) -> TestResult {
    let mut results = run.snapshot().unwrap().results;
    assert_eq!(results.len(), 1, "{results:?}");
    results.remove(0)
}

#[tokio::test]
async fn a_running_test_is_untested_and_a_finished_one_has_its_result() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |settings| settings.dashboard = true);
    let live = Arc::clone(&run);
    fixture(&run, |_| {})
        .run_with_browser(
            async || mocked().await,
            async |_page| {
                let during = live.snapshot()?;
                assert_eq!(during.results.len(), 1, "started before the body ran");
                assert_eq!(during.results[0].status, TestStatus::Untested);
                assert!(!during.finished);
                let page = std::fs::read_to_string(live.root().join("dashboard.html")).unwrap();
                assert!(
                    page.contains("http-equiv=\"refresh\""),
                    "the live page refreshes"
                );
                assert!(
                    page.contains("a_running_test_is_untested_and_a_finished_one_has_its_result"),
                    "the test is on the page while it runs"
                );
                Ok(())
            },
        )
        .await
        .unwrap();
    let result = only(&run);
    assert_eq!(result.status, TestStatus::Passed);
    assert_eq!(result.message, "ok");
    assert!(
        !run.snapshot().unwrap().finished,
        "a plain `cargo test` leaves the run live for `sbrs report finish`"
    );
}

#[tokio::test]
async fn a_failed_test_is_recorded_with_its_evidence_in_pythons_layout() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |settings| {
        settings.junit_xml = Some("junit.xml".into());
    });
    let fixture = fixture(&run, |_| {});
    let id = fixture.test_id().to_owned();
    let result = fixture
        .run_with_browser(
            async || mocked().await,
            async |_page| {
                Err(SeleniumBaseError::AssertionFailed(
                    "the cart is empty".to_owned(),
                ))
            },
        )
        .await;
    assert!(result.is_err());

    let recorded = only(&run);
    assert_eq!(recorded.status, TestStatus::Failed);
    assert!(
        recorded.message.contains("the cart is empty"),
        "{recorded:?}"
    );
    let failure = recorded.failure.expect("a failure has its details");
    assert_eq!(failure.url.as_deref(), Some("https://shop.test/cart"));
    assert_eq!(
        failure.screenshot,
        Some(format!("latest_logs/{id}/screenshot.png"))
    );
    assert_eq!(
        failure.page_source,
        Some(format!("latest_logs/{id}/page_source.html"))
    );

    let folder = root.path().join("latest_logs").join(&id);
    let info = std::fs::read_to_string(folder.join("basic_test_info.txt")).unwrap();
    assert!(
        info.starts_with(
            "sb_reports::a_failed_test_is_recorded_with_its_evidence_in_pythons_layout\r\n"
        ),
        "the first line is the display id: {info}"
    );
    assert!(info.contains("Traceback:\r\n"), "{info}");
    assert!(
        info.contains("Exception: assertion failed: the cart is empty"),
        "{info}"
    );
    assert_eq!(std::fs::read(folder.join("screenshot.png")).unwrap(), PNG);

    let xml = std::fs::read_to_string(root.path().join("junit.xml")).unwrap();
    assert!(xml.contains("failure"), "{xml}");
}

#[tokio::test]
async fn an_attempt_that_will_be_run_again_is_not_the_result() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    let live = Arc::clone(&run);
    let attempts = AtomicU32::new(0);
    fixture(&run, |test| {
        test.reruns = 1;
        test.reruns_delay = 0.0;
    })
    .run_with_browser(
        async || mocked().await,
        async |_page| {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(SeleniumBaseError::AssertionFailed("flaky".to_owned()));
            }
            let summary = live.snapshot()?.summary();
            assert_eq!(
                (summary.untested, summary.failed),
                (1, 0),
                "the first attempt records nothing"
            );
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(only(&run).status, TestStatus::Passed);
}

#[tokio::test]
async fn a_test_that_fails_every_attempt_is_one_failed_row() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    let result = fixture(&run, |test| {
        test.reruns = 2;
        test.reruns_delay = 0.0;
    })
    .run_with_browser(
        async || mocked().await,
        async |_page| Err(SeleniumBaseError::AssertionFailed("always".to_owned())),
    )
    .await;
    assert!(result.is_err());
    let recorded = only(&run);
    assert_eq!(recorded.status, TestStatus::Failed);
    assert!(recorded.message.contains("always"));
}

#[tokio::test]
async fn a_skip_is_a_skipped_row_with_its_reason() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    let _ = fixture(&run, |_| {})
        .run_with_browser(
            async || mocked().await,
            async |_page| Err(SeleniumBaseError::Skipped("no network".to_owned())),
        )
        .await;
    let recorded = only(&run);
    assert_eq!(recorded.status, TestStatus::Skipped);
    assert_eq!(recorded.message, "no network");
}

#[tokio::test]
async fn a_browser_that_does_not_start_is_a_failed_test() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    let result = fixture(&run, |_| {})
        .run_with_browser(
            async || -> Result<Browser> {
                Err(SeleniumBaseError::BrowserLaunch {
                    binary: "chrome".to_owned(),
                    reason: "no display".to_owned(),
                })
            },
            async |_page| Ok(()),
        )
        .await;
    assert!(result.is_err());
    let recorded = only(&run);
    assert_eq!(recorded.status, TestStatus::Failed);
    assert!(recorded.message.contains("no display"), "{recorded:?}");
}

#[tokio::test]
async fn the_dashboard_the_html_report_and_the_xml_follow_the_test() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |settings| {
        settings.dashboard = true;
        settings.html = Some("report.html".into());
        settings.junit_xml = Some("junit.xml".into());
    });
    fixture(&run, |_| {})
        .run_with_browser(async || mocked().await, async |_page| Ok(()))
        .await
        .unwrap();
    for file in ["dashboard.html", "report.html", "junit.xml"] {
        let text = std::fs::read_to_string(root.path().join(file)).unwrap();
        assert!(
            text.contains("the_dashboard_the_html_report_and_the_xml_follow_the_test"),
            "{file}: {text}"
        );
    }
}

#[tokio::test]
async fn a_test_that_starts_after_the_run_was_finalized_takes_it_up_again() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    fixture(&run, |_| {})
        .run_with_browser(async || mocked().await, async |_page| Ok(()))
        .await
        .unwrap();
    run.finalize().unwrap();
    assert!(run.snapshot().unwrap().finished);

    let live = Arc::clone(&run);
    fixture(&run, |_| {})
        .run_with_browser(
            async || mocked().await,
            async |_page| {
                assert!(
                    !live.snapshot()?.finished,
                    "the next binary's tests are live"
                );
                Ok(())
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn a_test_without_a_report_leaves_the_run_alone() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |_| {});
    SbCdp::new(Settings::default())
        .report_to(Arc::clone(&run))
        .without_report()
        .run_with_browser(async || mocked().await, async |_page| Ok(()))
        .await
        .unwrap();
    assert!(run.snapshot().unwrap().results.is_empty());
}

#[tokio::test]
async fn a_report_in_the_dashboards_file_is_reported_as_that_file() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |settings| {
        settings.dashboard = true;
        settings.html = Some("dashboard.html".into());
    });
    fixture(&run, |_| {})
        .run_with_browser(async || mocked().await, async |_page| Ok(()))
        .await
        .unwrap();
    let ended = run.finalize().unwrap();
    let dashboard = root.path().join("dashboard.html");
    assert_eq!(ended.dashboard.as_deref(), Some(dashboard.as_path()));
    assert_eq!(
        ended.html.as_deref(),
        Some(dashboard.as_path()),
        "the report is the dashboard's final page"
    );
}

#[tokio::test]
async fn finishing_a_run_ends_it_and_hands_the_results_to_the_database() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), |settings| {
        settings.with_db_reporting = true;
        settings.db = Some("results.db".into());
    });
    fixture(&run, |_| {})
        .run_with_browser(async || mocked().await, async |_page| Ok(()))
        .await
        .unwrap();

    let finished = run.finish().await.unwrap();

    assert!(run.snapshot().unwrap().finished);
    assert_eq!(finished.outcome.summary.passed, 1);
    #[cfg(feature = "turso")]
    {
        assert!(
            finished.database_run.is_some(),
            "{:?}",
            finished.outcome.warnings
        );
        assert!(root.path().join("results.db").is_file());
    }
    #[cfg(not(feature = "turso"))]
    assert!(
        finished
            .outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("turso")),
        "{:?}",
        finished.outcome.warnings
    );
}
