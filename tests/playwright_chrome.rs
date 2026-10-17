//! The Playwright-compatible session against a real Chromium.
//!
//! These need the `playwright` feature and a Chrome or Chromium, so they are
//! ignored by default. Set `SB_CHROME_BIN` to the browser if rustwright does
//! not find it:
//!
//! ```text
//! cargo test --features playwright --test playwright_chrome -- --ignored
//! ```
//!
//! The feature links Python through pyo3; see the README for the library path
//! the test binary needs on macOS.
#![cfg(feature = "playwright")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::browser::config::BrowserConfig;
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::stealth::fingerprint::Fingerprint;
use serde_json::json;

const PAGE: &str = "<!doctype html><html><head><title>Playwright</title></head>\
                    <body><h1 id=\"title\">hello</h1></body></html>";

/// Serves one page on a loopback port.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

fn config() -> BrowserConfig {
    let config = BrowserConfig::default().with_headless(true);
    match std::env::var_os("SB_CHROME_BIN") {
        Some(path) => config.with_browser_binary_path(path),
        None => config,
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome through rustwright"]
async fn the_fingerprint_is_still_presented_after_navigating() {
    let port = serve();
    let session = PlaywrightSession::launch_with_config(
        &config().with_fingerprint(Fingerprint::windows_desktop()),
    )
    .await
    .expect("the session starts");

    // Before the first navigation the page is about:blank. The fingerprint
    // used to be applied only there, and lost as soon as the page changed.
    session
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");
    let platform = session
        .evaluate("navigator.platform")
        .await
        .expect("the page runs script");
    assert_eq!(platform, json!("Win32"));
    assert_eq!(
        session.get_text("#title").await.expect("the heading"),
        "hello"
    );

    session.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn an_attached_session_sees_the_open_page_and_leaves_the_browser_running() {
    use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

    let port = serve();
    let browser = Browser::launch(
        LaunchOptions::builder()
            .headless(true)
            .build()
            .expect("launch options"),
    )
    .await
    .expect("Chrome starts");
    let page = browser.default_page().await.expect("a first tab");
    page.goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    let session = PlaywrightSession::connect_over_cdp(&browser.http_url())
        .await
        .expect("rustwright attaches");
    // It works on the tab that was already open, not a blank new one.
    assert_eq!(
        session.get_text("#title").await.expect("the heading"),
        "hello"
    );
    // Closing detaches; the browser it did not start keeps running.
    session.close().await.expect("detaches");
    assert_eq!(page.title().await.expect("still alive"), "Playwright");
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome through rustwright"]
async fn a_missing_element_is_reported_as_not_found() {
    let port = serve();
    let session = PlaywrightSession::launch_with_config(&config())
        .await
        .expect("the session starts");
    session
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    // rustwright waits for the element before giving up, so this takes a while.
    let error = session
        .click("#not-on-this-page")
        .await
        .expect_err("there is nothing to click");
    assert!(
        matches!(error, SeleniumBaseError::ElementNotFound { .. }),
        "{error:?}"
    );

    session.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches Chrome through chromedriver and a second one through rustwright"]
async fn the_playwright_mode_of_a_case_uses_its_config_and_quit_closes_it() {
    use seleniumbase_rs::BaseCase;

    let port = serve();
    let mut sb = BaseCase::new(config().with_user_agent("Agent/1.0"))
        .await
        .expect("the case starts");
    sb.activate_playwright_mode()
        .await
        .expect("the Playwright browser starts");
    {
        let playwright = sb.playwright_session().expect("the session is active");
        playwright
            .goto(&format!("http://127.0.0.1:{port}/"))
            .await
            .expect("the page loads");
        // The case's user agent reached the second browser; it used to start
        // as a default headed one.
        assert_eq!(
            playwright
                .evaluate("navigator.userAgent")
                .await
                .expect("the page runs script"),
            json!("Agent/1.0")
        );
    }

    sb.quit().await.expect("both browsers close");
    assert!(
        sb.playwright_session().is_none(),
        "quit hands the session back and closes it"
    );
}
