//! `save_screenshot_to_logs` and `save_page_source_to_logs` against a real
//! Chrome, while a test run records the test: the files land in the test's own
//! folder, `latest_logs/<test id>/`, as they do in Python SeleniumBase.
//!
//! The test needs Chrome and chromedriver on the machine, so it is ignored by
//! default. Run it with:
//!
//! ```text
//! cargo test --test basecase_logs_chrome -- --ignored
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts itself.

use std::fs;

use seleniumbase_rs::core::log_helper::TestId;
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::{BaseCase, BrowserConfig};

const PAGE: &str =
    "data:text/html,<html><head><title>Logs</title></head><body>hello logs</body></html>";

fn config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_screenshot_and_the_source_land_in_the_folder_of_the_recorded_test() {
    let root = tempfile::tempdir().unwrap();
    let run = TestRun::open(
        root.path(),
        ReportSettings::default(),
        RunKey::new("chrome"),
    )
    .unwrap();
    let recorder = run.start(TestId::named("shop::logs")).unwrap();
    let folder = recorder.log_dir().to_path_buf();
    let mut sb = BaseCase::new(config()).await.expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");

    let screenshot = sb.save_screenshot_to_logs().await.unwrap();
    let source = sb.save_page_source_to_logs().await.unwrap();

    assert_eq!(
        screenshot.parent(),
        Some(folder.as_path()),
        "{screenshot:?}"
    );
    assert_eq!(source.parent(), Some(folder.as_path()), "{source:?}");
    let png = fs::read(&screenshot).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "a real PNG");
    assert!(fs::read_to_string(&source).unwrap().contains("hello logs"));
    sb.quit().await.expect("quit");
    recorder.finish(&Ok(())).unwrap();
}
