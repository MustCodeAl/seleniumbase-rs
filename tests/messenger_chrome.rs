//! Real-browser checks of the Messenger messages: `post_message`,
//! `post_success_message`, `post_error_message`, `activate_messenger` and
//! `set_messenger_theme`.
//!
//! The browser is headless, which would cut a message to 0.75 s, so the case is
//! told that somebody watches (`BaseCase::with_somebody_watching`, in the
//! `test-util` feature) and the messages keep the time they are given.
//!
//! These tests are ignored by default; they need Chrome and a network that can
//! reach cdnjs, from where the page loads jQuery, Underscore and Messenger.
//! Run them locally with:
//!
//! ```bash
//! cargo test --test messenger_chrome --features test-util -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

#![cfg(feature = "test-util")]

use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{MessengerLocation, MessengerSettings, MessengerTheme};
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with nothing in it but a paragraph.
const PAGE: &str = "data:text/html,<html><head><title>messages</title></head><body>\
    <p id=\"here\">hello</p></body></html>";

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

/// A case in a headless browser that behaves as if somebody watches, on a page.
async fn watched_case() -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser")
        .with_somebody_watching();
    sb.set_time_limit(120).expect("set the limit");
    sb.open(PAGE).await.expect("open the page");
    sb
}

/// Runs `expression` in the page and returns what it answers.
async fn ask(sb: &BaseCase, expression: &str) -> serde_json::Value {
    sb.execute_script(&format!("return {expression};"))
        .await
        .expect("the page answers")
}

/// Waits (up to ten seconds) until `expression` is true in the page.
async fn wait_until(sb: &BaseCase, expression: &str) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if ask(sb, expression).await == serde_json::Value::Bool(true) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("this never became true in the page: {expression}");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_message_is_shown_by_messenger_in_the_future_theme() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;

    sb.post_message("Hello <b>world</b>")
        .duration(Duration::from_secs(4))
        .pause(false)
        .await
        .expect("post the message");

    wait_until(&sb, "!!document.querySelector('.messenger-message')").await;
    // The theme is the future one, in the bottom right corner.
    wait_until(
        &sb,
        "!!document.querySelector('ul.messenger.messenger-theme-future')",
    )
    .await;
    let classes = ask(&sb, "document.querySelector('ul.messenger').className").await;
    let classes = classes.as_str().expect("class names");
    assert!(classes.contains("messenger-on-bottom"), "{classes}");
    assert!(classes.contains("messenger-on-right"), "{classes}");
    // The message is HTML: the bold tag became an element.
    let bold = ask(
        &sb,
        "document.querySelector('.messenger-message-inner b').textContent",
    )
    .await;
    assert_eq!(bold, "world");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn the_style_of_a_message_is_its_type() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;

    sb.post_success_message("Saved")
        .duration(Duration::from_secs(4))
        .pause(false)
        .await
        .expect("post the success");
    wait_until(
        &sb,
        "!!document.querySelector('.messenger-message.message-success')",
    )
    .await;

    sb.post_error_message("Not saved")
        .duration(Duration::from_secs(4))
        .pause(false)
        .await
        .expect("post the error");
    wait_until(
        &sb,
        "!!document.querySelector('.messenger-message.message-error')",
    )
    .await;
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn the_theme_and_the_place_can_be_chosen() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;

    sb.set_messenger_theme(MessengerSettings {
        theme: MessengerTheme::Ice,
        location: MessengerLocation::TopLeft,
        max_messages: 3,
    })
    .await
    .expect("set the theme");
    sb.post_message("In ice")
        .duration(Duration::from_secs(4))
        .pause(false)
        .await
        .expect("post the message");

    wait_until(
        &sb,
        "!!document.querySelector('ul.messenger.messenger-theme-ice')",
    )
    .await;
    let classes = ask(&sb, "document.querySelector('ul.messenger').className").await;
    let classes = classes.as_str().expect("class names");
    assert!(classes.contains("messenger-on-top"), "{classes}");
    assert!(classes.contains("messenger-on-left"), "{classes}");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_message_waits_for_itself_and_a_second_activation_is_harmless() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;

    sb.activate_messenger().await.expect("activate Messenger");
    sb.activate_messenger().await.expect("activate it again");
    // Every theme's style sheet is in the page once.
    let sheets = ask(
        &sb,
        "document.querySelectorAll('link[href*=\"messenger-theme-\"]').length",
    )
    .await;
    assert_eq!(sheets, 5);

    let started = Instant::now();
    sb.post_message("Wait for me")
        .duration(Duration::from_millis(600))
        .await
        .expect("post the message");
    // The duration and 0.15 s more.
    assert!(
        started.elapsed() >= Duration::from_millis(740),
        "{:?}",
        started.elapsed()
    );
    sb.quit().await.expect("quit");
}
