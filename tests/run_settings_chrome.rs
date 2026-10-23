//! Real-browser checks of the run settings that hook into a page load or a
//! wait: `--check-js`, `--disable-beforeunload` and `--timeout-multiplier`
//! (`RunSettings::with_check_js`, `with_disable_beforeunload`,
//! `with_timeout_multiplier`). The same settings are checked against a mock
//! WebDriver in `src/api/base_case_impls/base_case_impl_driver_tests.rs`.
//!
//! These tests are ignored by default. Run them locally with Chrome installed:
//!
//! ```bash
//! cargo test --test run_settings_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::time::{Duration, Instant};

use seleniumbase_rs::api::run_settings::RunSettings;
use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page whose script throws, which Chrome logs as a SEVERE console entry.
const BROKEN_PAGE: &str = "data:text/html,<html><head><title>broken</title></head><body>\
    <script>throw new Error('boom from the page');</script></body></html>";

/// A page with nothing wrong in it.
const CLEAN_PAGE: &str = "data:text/html,<html><head><title>clean</title></head><body>\
    <p id=\"here\">hello</p></body></html>";

/// A page that asks for the `beforeunload` dialog.
const LEAVING_PAGE: &str = "data:text/html,<html><head><title>leaving</title></head><body>\
    <script>window.onbeforeunload = function () { return 'leave?'; };</script></body></html>";

fn test_config(settings: RunSettings) -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_run_settings(settings);
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn check_js_fails_the_open_of_a_page_that_throws() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(RunSettings::default().with_check_js(true)))
        .await
        .expect("connect to browser");

    let error = sb
        .open(BROKEN_PAGE)
        .await
        .expect_err("the open finds the script error");
    let SeleniumBaseError::AssertionFailed(message) = &error else {
        panic!("expected an assertion failure, got {error:?}");
    };
    assert!(message.contains("boom from the page"), "{message}");

    sb.open(CLEAN_PAGE)
        .await
        .expect("a page without errors opens");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn without_check_js_a_page_that_throws_opens() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(RunSettings::default()))
        .await
        .expect("connect to browser");
    sb.open(BROKEN_PAGE)
        .await
        .expect("the error is not looked for");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn disable_beforeunload_clears_the_handler_of_the_page_that_was_opened() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(
        RunSettings::default().with_disable_beforeunload(true),
    ))
    .await
    .expect("connect to browser");
    sb.open(LEAVING_PAGE).await.expect("open the page");
    let cleared = sb
        .execute_script("return window.onbeforeunload === null;")
        .await
        .expect("the script runs");
    assert_eq!(cleared, serde_json::json!(true));
    sb.quit().await.expect("quit");

    let mut plain = BaseCase::new(test_config(RunSettings::default()))
        .await
        .expect("connect to browser");
    plain.open(LEAVING_PAGE).await.expect("open the page");
    let kept = plain
        .execute_script("return typeof window.onbeforeunload;")
        .await
        .expect("the script runs");
    assert_eq!(kept, serde_json::json!("function"));
    plain.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_timeout_multiplier_stretches_the_wait_for_an_element_that_never_comes() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(
        RunSettings::default().with_timeout_multiplier(2.0),
    ))
    .await
    .expect("connect to browser");
    sb.open(CLEAN_PAGE).await.expect("open the page");
    sb.set_timeout(1).await.expect("set the default wait");

    let started = Instant::now();
    let error = sb.click("#absent").await;
    let waited = started.elapsed();

    assert!(error.is_err(), "there is nothing to click");
    assert!(
        waited >= Duration::from_secs(2),
        "the default wait of 1 s doubled lasted only {waited:?}"
    );
    assert!(waited < Duration::from_secs(8), "waited {waited:?}");
    sb.quit().await.expect("quit");
}
