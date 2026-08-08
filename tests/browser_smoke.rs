//! Real-browser smoke tests driven through chromedriver.
//!
//! These tests are ignored by default. Run them locally with Chrome and
//! chromedriver installed:
//!
//! ```bash
//! cargo test --test browser_smoke -- --ignored
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to point at a remote grid instead of the default
//! local chromedriver instance (auto-started on port 9515 by `BaseCase`).

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn test_config() -> BrowserConfig {
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

const DEMO_PAGE: &str = "data:text/html,<html lang=\"en\"><head><title>Smoke</title></head>\
    <body><main><h1 id=\"title\">Hello Smoke</h1>\
    <input id=\"field\" type=\"text\" /><button id=\"btn\">Go</button></main></body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn open_page_and_assert_title() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(DEMO_PAGE).await.expect("open demo page");
    sb.assert_title("Smoke").await.expect("title assertion");
    sb.assert_element("h1#title").await.expect("h1 present");
    sb.assert_text("h1#title", "Hello Smoke")
        .await
        .expect("text assertion");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn type_and_click_round_trip() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(DEMO_PAGE).await.expect("open demo page");
    sb.type_text("#field", "hello world")
        .await
        .expect("type text");
    let value = sb.get_value("#field").await.expect("read value");
    assert_eq!(value, "hello world");
    sb.click("#btn").await.expect("click button");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn screenshot_produces_png_bytes() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(DEMO_PAGE).await.expect("open demo page");
    let png = sb.screenshot_as_png().await.expect("screenshot");
    assert!(png.len() > 1000, "expected a real PNG payload");
    assert_eq!(&png[1..4], b"PNG", "expected PNG magic bytes");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn deferred_asserts_collect_failures() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(DEMO_PAGE).await.expect("open demo page");
    sb.deferred_assert_element("h1#title")
        .await
        .expect("record assertion");
    sb.process_deferred_asserts()
        .await
        .expect("all deferred assertions should pass");
    sb.quit().await.expect("quit");
}
