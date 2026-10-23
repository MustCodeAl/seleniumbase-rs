//! The `sb_cdp` fixture and `SbCdp::run` on a mocked browser: test data, the
//! evidence of a failure, reruns, the time limit and the closed browser. No
//! Chrome starts; `tests/sb_fixture_chrome.rs` runs the same on a real one.
#![cfg(all(feature = "testing", feature = "test-util"))]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::prelude::*;
use rstest::rstest;
use seleniumbase_rs::config::test_settings::{TestData, TestSettings};
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::testing::{sb_cdp, SbCdp};
use seleniumbase_rs::{Result, SeleniumBaseError, Settings};
use serde_json::json;

const PNG: &[u8] = b"\x89PNG-from-the-mock";

/// A mocked browser whose tab answers like the page `https://shop.test/cart`,
/// and the mocks it made, in order.
#[derive(Clone, Default)]
struct Browsers {
    made: Arc<Mutex<Vec<Browser>>>,
}

impl Browsers {
    async fn launch(&self) -> Result<Browser> {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Runtime.evaluate",
            json!({ "result": { "value": "https://shop.test/cart" } }),
        );
        mock.reply(
            "Page.captureScreenshot",
            json!({ "data": BASE64_STANDARD.encode(PNG) }),
        );
        self.made.lock().unwrap().push(browser.clone());
        Ok(browser)
    }

    fn launches(&self) -> usize {
        self.made.lock().unwrap().len()
    }

    /// How many browsers were told to close.
    async fn closed(&self) -> usize {
        let browsers: Vec<Browser> = self.made.lock().unwrap().clone();
        let mut closed = 0;
        for browser in browsers {
            // A closed mock answers nothing.
            let gone = match browser.default_page().await {
                Ok(page) => page.url().await.is_err(),
                Err(_) => true,
            };
            closed += usize::from(gone);
        }
        closed
    }
}

fn settings(change: impl FnOnce(&mut TestSettings)) -> Settings {
    let mut test = TestSettings::default();
    change(&mut test);
    Settings {
        test,
        ..Settings::default()
    }
}

fn fixture_for(logs: &Path, change: impl FnOnce(&mut TestSettings)) -> SbCdp {
    SbCdp::new(settings(change)).logs_dir(logs)
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[rstest]
#[tokio::test]
async fn the_fixture_runs_a_test_on_the_first_tab_and_closes_the_browser(sb_cdp: SbCdp) {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let result = sb_cdp
        .logs_dir(logs.path())
        .run_with_browser(
            async || browsers.launch().await,
            async |page| {
                assert_eq!(page.url().await?, "https://shop.test/cart");
                Ok(())
            },
        )
        .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(browsers.launches(), 1);
    assert_eq!(browsers.closed().await, 1);
    assert!(names(logs.path()).is_empty(), "a pass leaves nothing");
}

#[rstest]
#[tokio::test]
async fn the_test_id_is_the_binary_the_module_and_the_function(sb_cdp: SbCdp) {
    assert_eq!(
        sb_cdp.test_id().rsplit('.').next(),
        Some("the_test_id_is_the_binary_the_module_and_the_function")
    );
    assert!(
        sb_cdp.test_id().starts_with("sb_fixture."),
        "{}",
        sb_cdp.test_id()
    );
}

#[tokio::test]
async fn the_test_data_of_the_run_reaches_the_test() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |test| {
        test.data = TestData {
            env: Some("qa".to_owned()),
            data: Some("abc".to_owned()),
            var1: Some("x".to_owned()),
            variables: TestData::parse_variables("{'k': 1}").unwrap(),
            ..TestData::default()
        };
    });
    let data = fixture.data().clone();
    assert_eq!(data.env(), "qa");
    let result = fixture
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| {
                assert_eq!(data.data.as_deref(), Some("abc"));
                assert_eq!(data.var1.as_deref(), Some("x"));
                assert_eq!(data.variables["k"], 1);
                Ok(())
            },
        )
        .await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn a_failing_test_leaves_the_three_files_and_closes_the_browser() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |_| {});
    let id = fixture.test_id().to_owned();
    let result = fixture
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| {
                Err(SeleniumBaseError::AssertionFailed(
                    "the cart is empty".to_owned(),
                ))
            },
        )
        .await;
    assert!(
        matches!(result, Err(SeleniumBaseError::AssertionFailed(_))),
        "{result:?}"
    );
    let folder = logs.path().join(&id);
    assert_eq!(
        names(&folder),
        ["basic_test_info.txt", "page_source.html", "screenshot.png"]
    );
    assert_eq!(std::fs::read(folder.join("screenshot.png")).unwrap(), PNG);
    let info = std::fs::read_to_string(folder.join("basic_test_info.txt")).unwrap();
    assert!(info.starts_with(&id), "{info}");
    assert!(info.contains("Last Page: https://shop.test/cart"), "{info}");
    assert!(
        info.contains("Exception: assertion failed: the cart is empty"),
        "{info}"
    );
    assert_eq!(browsers.closed().await, 1, "closed after the evidence");
}

#[tokio::test]
async fn no_screenshot_leaves_out_the_png() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |test| test.no_screenshot = true);
    let id = fixture.test_id().to_owned();
    let _ = fixture
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| Err(SeleniumBaseError::AssertionFailed("x".to_owned())),
        )
        .await;
    assert_eq!(
        names(&logs.path().join(id)),
        ["basic_test_info.txt", "page_source.html"]
    );
}

#[tokio::test]
async fn a_screenshot_of_a_passing_test_is_taken_on_request() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |test| test.save_screenshot = true);
    let id = fixture.test_id().to_owned();
    fixture
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await
        .unwrap();
    assert_eq!(
        names(&logs.path().join(id)),
        ["page_source.html", "screenshot.png"]
    );
}

#[tokio::test]
async fn a_test_that_fails_once_passes_on_a_rerun() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |test| {
        test.reruns = 2;
        test.reruns_delay = 0.05;
    });
    let runs = AtomicU32::new(0);
    let started = Instant::now();
    let result = fixture
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(SeleniumBaseError::AssertionFailed("flaky".to_owned()))
                } else {
                    Ok(())
                }
            },
        )
        .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(runs.load(Ordering::SeqCst), 2);
    assert_eq!(browsers.launches(), 2, "a browser per attempt");
    assert_eq!(browsers.closed().await, 2);
    assert!(started.elapsed() >= Duration::from_millis(50), "the delay");
}

#[tokio::test]
async fn a_test_that_runs_past_the_time_limit_is_stopped_and_its_browser_closed() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::default();
    let fixture = fixture_for(logs.path(), |test| test.time_limit = Some(1.0));
    let started = Instant::now();
    let result = fixture
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| {
                tokio::time::sleep(Duration::from_secs(3)).await;
                Ok(())
            },
        )
        .await;
    let error = result.unwrap_err().to_string();
    assert!(
        error.ends_with("This test has exceeded the time limit of 1 second!"),
        "{error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(browsers.closed().await, 1);
}

#[tokio::test]
async fn settings_that_do_not_hold_up_fail_the_test_before_a_browser_starts() {
    let browsers = Browsers::default();
    let fixture = SbCdp::new(Settings {
        browser: "netscape".to_owned(),
        ..Settings::default()
    });
    let result = fixture
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;
    assert!(result.unwrap_err().to_string().contains("netscape"));
    assert_eq!(browsers.launches(), 0);
}

#[tokio::test]
async fn a_launch_that_fails_is_reported() {
    let fixture = SbCdp::new(Settings::default());
    let result = fixture
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
    assert!(matches!(
        result,
        Err(SeleniumBaseError::BrowserLaunch { .. })
    ));
}
