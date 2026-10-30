//! `BaseCase::solve_captcha` (the WebDriver engine, with trusted input sent over
//! the DevTools Protocol) against the same imitation widgets as the Pure CDP
//! tests, in a real headless Chrome.
//!
//! `BaseCase` and `Page` share one solver (`sb_cdp::captcha`): the locating, the
//! waiting for the widget to stand still, the human pointer path, the checking
//! of the result and the retries are the same code run against a different tab.
//! These tests show the WebDriver side of that seam works on a real browser: a
//! cross-origin frame, a shadow root, a pointer judge, a widget that needs a
//! second check, and an interstitial that sets a clearance cookie.
//!
//! They start Chrome through chromedriver, so they are `#[ignore]`d. Run them one
//! binary at a time:
//!
//! ```text
//! cargo test --test captcha_basecase_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/captcha_site.rs"]
mod captcha_site;

use std::time::{Duration, Instant};

use captcha_site::Site;
use seleniumbase_rs::sb_cdp::Captcha;
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
use serde_json::Value;

/// How long a test waits for the page to show it cleared.
const CLEAR_WITHIN: Duration = Duration::from_secs(8);

fn config() -> BrowserConfig {
    // The solver sends its pointer events over the DevTools Protocol, so the
    // session needs the CDP (or UC) mode.
    BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_mode(DriverMode::Cdp)
}

/// Whether `script` (which returns a boolean) is true within `within`.
async fn becomes_true(sb: &BaseCase, script: &str, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if sb.execute_script(script).await.ok() == Some(Value::Bool(true)) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

const CLEARED: &str = "return !!(document.body && document.body.dataset.cleared === '1');";

/// Opens `path`, solves the widget and asserts the page clears.
async fn solves(path: &str) {
    let site = Site::start();
    let mut sb = BaseCase::new(config()).await.expect("start Chrome");
    sb.open(&site.url(path)).await.expect("open the fixture");

    let attempted = sb.solve_captcha().await;
    assert!(
        matches!(attempted, Ok(Some(Captcha::Turnstile))),
        "{path}: solve_captcha returned {attempted:?}"
    );
    assert!(
        becomes_true(&sb, CLEARED, CLEAR_WITHIN).await,
        "{path}: the page did not clear"
    );
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_turnstile_in_a_cross_origin_frame_clears() {
    solves("/iframe").await;
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_turnstile_inside_an_open_shadow_root_clears() {
    solves("/shadow").await;
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_widget_that_needs_a_second_verified_check_clears() {
    solves("/iframe?widget=fail=1").await;
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_widget_that_judges_the_pointer_path_clears_for_a_human_path() {
    solves("/human").await;
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_form_checkbox_beside_the_widget_is_left_alone() {
    let site = Site::start();
    let mut sb = BaseCase::new(config()).await.expect("start Chrome");

    sb.open(&site.url("/decoy?widget=none"))
        .await
        .expect("open the fixture");
    assert_eq!(sb.solve_captcha().await.expect("solve_captcha"), None);
    let presses = sb.execute_script("return window.__mouse;").await.unwrap();
    assert_eq!(presses, Value::from(0), "something was pressed");

    sb.open(&site.url("/decoy"))
        .await
        .expect("open the fixture");
    sb.solve_captcha().await.expect("solve_captcha");
    assert!(
        becomes_true(&sb, CLEARED, CLEAR_WITHIN).await,
        "did not clear"
    );
    let boxes = sb
        .execute_script(
            "return [document.getElementById('newsletter').checked, \
             document.getElementById('terms').checked];",
        )
        .await
        .unwrap();
    assert_eq!(boxes, serde_json::json!([false, false]));
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn an_interstitial_clears_to_the_real_page_with_a_clearance_cookie() {
    let site = Site::start();
    let mut sb = BaseCase::new(config()).await.expect("start Chrome");
    sb.open(&site.url("/interstitial"))
        .await
        .expect("open the fixture");

    let solved = sb
        .solve_captcha_with(&seleniumbase_rs::sb_cdp::SolveOptions::default())
        .await
        .expect("solve_captcha_with")
        .expect("a widget");
    assert!(solved.evidence.is_some(), "{solved}");

    assert!(
        becomes_true(
            &sb,
            "return !!document.getElementById('real-page');",
            CLEAR_WITHIN
        )
        .await,
        "the real page did not load"
    );
    let cookies = sb.get_cookies().await.expect("cookies").to_string();
    assert!(cookies.contains("cf_clearance"), "{cookies}");
    sb.quit().await.expect("quit");
}
