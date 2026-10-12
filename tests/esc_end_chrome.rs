//! A real-browser check of `--esc-end` (`BrowserConfig::with_esc_end`).
//!
//! With the setting on, `BaseCase` puts a key listener in the page, and the
//! first call after a person presses ESC in the browser returns
//! `SeleniumBaseError::Skipped` instead of doing its work. The test presses
//! ESC through chromedriver's key actions, which Chrome delivers as a real
//! `keydown` event.
//!
//! This test is ignored by default. Run it locally with Chrome installed:
//!
//! ```bash
//! cargo test --test esc_end_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A button that writes "clicked" in the title when it is clicked.
const PAGE: &str = "data:text/html,<html><head><title>start</title></head><body>\
    <button id=\"go\" onclick=\"document.title='clicked'\">Go</button>\
    <input id=\"box\"></body></html>";

fn test_config(esc_end: bool) -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_esc_end(esc_end);
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// Sets the title to "start" again, so a click shows in it.
async fn reset_title(sb: &BaseCase) {
    sb.execute_script("document.title = 'start'; return 1;")
        .await
        .expect("the script runs");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn pressing_escape_in_the_page_ends_the_test() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(true))
        .await
        .expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");

    // Until ESC is pressed, calls do their work.
    sb.click("#go").await.expect("the first click works");
    assert_eq!(sb.get_title().await.unwrap(), "clicked");
    reset_title(&sb).await;

    // The person presses ESC with the page in front of them.
    sb.click("#box").await.expect("focus the input");
    sb.press_key("escape").await.expect("press ESC");

    let error = sb
        .click("#go")
        .await
        .expect_err("the click is stopped by ESC");
    assert!(
        matches!(&error, SeleniumBaseError::Skipped(message) if message.contains("ESC")),
        "{error:?}"
    );
    assert_eq!(
        sb.get_title().await.unwrap(),
        "start",
        "the stopped click did not click"
    );
    assert!(
        matches!(
            sb.wait_for_ready_state_complete().await,
            Err(SeleniumBaseError::Skipped(_))
        ),
        "every place that looks for ESC keeps reporting it"
    );

    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn without_the_setting_escape_changes_nothing() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config(false))
        .await
        .expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");

    sb.click("#box").await.expect("focus the input");
    sb.press_key("escape").await.expect("press ESC");
    sb.click("#go").await.expect("ESC is ignored");
    assert_eq!(sb.get_title().await.unwrap(), "clicked");
    sb.wait_for_ready_state_complete()
        .await
        .expect("the page is ready");

    sb.quit().await.expect("quit");
}
