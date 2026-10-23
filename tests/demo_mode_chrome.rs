//! Real-browser checks of the highlight family and of demo mode: the animation
//! blocks for about 0.2 s a loop and puts the box shadow back, `flash` paints a
//! dot and removes it, demo mode animates before an action, and a passed
//! assertion shows a Messenger success message.
//!
//! The browser is headless, which halves the loops and caps the messages, so
//! the case is told that somebody watches (`BaseCase::with_somebody_watching`,
//! in the `test-util` feature) and the timings are those of a headed run.
//!
//! These tests are ignored by default; they need Chrome, and the last one a
//! network that can reach cdnjs, from where Messenger is loaded. Run them
//! locally with:
//!
//! ```bash
//! cargo test --test demo_mode_chrome --features test-util -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

#![cfg(feature = "test-util")]

use std::time::{Duration, Instant};

use seleniumbase_rs::api::run_settings::RunSettings;
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with a paragraph that has a box shadow of its own, three list items
/// and a button that records its click. (No `#` in it: this is a data URL.)
const PAGE: &str = "data:text/html,<html><head><title>demo</title></head><body>\
    <p id=\"p\" style=\"box-shadow: 1px 1px 2px black\">hello</p>\
    <ul><li>one</li><li>two</li><li>three</li></ul>\
    <button id=\"b\" onclick=\"window.clicked = true\">go</button></body></html>";

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

/// A case in a headless browser that behaves as if somebody watches, on the page.
async fn watched_case(settings: RunSettings) -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser")
        .with_somebody_watching();
    sb.set_run_settings(settings);
    sb.open(PAGE).await.expect("open the page");
    sb
}

async fn run_script(sb: &BaseCase, script: &str) -> serde_json::Value {
    sb.execute_script(script).await.expect("run the script")
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn a_highlight_blocks_for_its_loops_and_puts_the_box_shadow_back() {
    let _lock = BROWSER_LOCK.lock().await;
    let mut sb = watched_case(RunSettings::default()).await;
    let before = run_script(&sb, "return document.querySelector('#p').style.boxShadow;").await;

    let started = Instant::now();
    sb.highlight("#p").loops(2).await.expect("highlight");
    let took = started.elapsed();

    // Two loops of about 0.2 s, plus the scroll check and the 65 ms settle.
    assert!(took >= Duration::from_millis(350), "too fast: {took:?}");
    assert!(took < Duration::from_millis(2500), "too slow: {took:?}");
    let after = run_script(&sb, "return document.querySelector('#p').style.boxShadow;").await;
    assert_eq!(before, after, "the page's own box shadow is back");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn highlight_elements_stops_at_the_limit() {
    let _lock = BROWSER_LOCK.lock().await;
    let mut sb = watched_case(RunSettings::default()).await;
    let started = Instant::now();
    sb.highlight_elements("li")
        .loops(1)
        .limit(2)
        .await
        .expect("highlight");
    let two = started.elapsed();
    let started = Instant::now();
    sb.highlight_elements("li")
        .loops(1)
        .await
        .expect("highlight");
    let three = started.elapsed();
    assert!(
        three > two,
        "all three take longer than two: {two:?} {three:?}"
    );
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn a_flash_adds_its_dot_and_takes_it_away() {
    let _lock = BROWSER_LOCK.lock().await;
    let mut sb = watched_case(RunSettings::default()).await;
    sb.flash("#p")
        .duration(Duration::from_millis(600))
        .color("FF8800")
        .await
        .expect("flash");
    let dots = "return document.querySelectorAll('[id^=flash-]').length;";
    assert_eq!(run_script(&sb, dots).await, 1, "the dot is there");
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(run_script(&sb, dots).await, 0, "and gone");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn demo_mode_animates_before_the_click_and_pauses_after_it() {
    let _lock = BROWSER_LOCK.lock().await;
    let mut sb = watched_case(
        RunSettings::default()
            .with_demo_mode(true)
            .with_demo_sleep(Duration::from_millis(340)),
    )
    .await;
    let started = Instant::now();
    sb.click("#b").await.expect("click");
    let took = started.elapsed();
    // Four loops of about 0.2 s, then 1/3.4 of 340 ms (the page did not change).
    assert!(took >= Duration::from_millis(800), "too fast: {took:?}");
    assert_eq!(
        run_script(&sb, "return window.clicked === true;").await,
        true
    );
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "needs Chrome and the network"]
async fn a_passed_assertion_shows_a_messenger_success_message_in_demo_mode() {
    let _lock = BROWSER_LOCK.lock().await;
    let mut sb = watched_case(
        RunSettings::default()
            .with_demo_mode(true)
            .with_message_duration(Duration::from_millis(600)),
    )
    .await;
    let started = Instant::now();
    sb.assert_text("hello", "#p")
        .await
        .expect("the text is there");
    assert!(
        started.elapsed() >= Duration::from_millis(600),
        "it waits for the message: {:?}",
        started.elapsed()
    );
    assert_eq!(
        run_script(&sb, "return typeof window.Messenger;").await,
        "function",
        "Messenger was loaded to show it"
    );
    assert_eq!(
        run_script(
            &sb,
            "return document.querySelector('#p').style.boxShadow !== '';"
        )
        .await,
        true,
        "the paragraph has its own shadow back"
    );
    sb.quit().await.expect("quit");
}
