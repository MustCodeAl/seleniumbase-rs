//! The bridge against a real Chrome.
//!
//! These need a Chrome or Chromium on the machine, so they are ignored by default. Run them
//! one binary at a time, headless:
//!
//! ```text
//! cargo test -p seleniumbase-rs-playwright --test playwright_chrome -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{Fingerprint, SeleniumBaseError};
use seleniumbase_rs_playwright::{PlaywrightSession, StealthyPlaywright};

const PAGE: &str = "<!doctype html><html><head><title>Playwright</title></head>\
                    <body><h1 id=\"title\">hello</h1><input id=\"name\"></body></html>";

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

fn headless() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .build()
        .expect("launch options")
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn the_fingerprint_of_the_browser_reaches_the_attached_client_after_navigating() {
    let port = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(&Fingerprint::windows_desktop())
        .build()
        .expect("launch options");
    let stealthy = StealthyPlaywright::launch(options)
        .await
        .expect("the browser starts and rustwright attaches");

    // The first page was about:blank when Playwright attached. The persona must still be
    // presented once the page has navigated.
    stealthy
        .playwright()
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");
    let platform = stealthy
        .playwright()
        .evaluate("navigator.platform")
        .await
        .expect("the page runs script");
    assert_eq!(platform, json!("Win32"));

    stealthy.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn an_attached_session_sees_the_open_page_and_leaves_the_browser_running() {
    let port = serve();
    let browser = Browser::launch(headless()).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a first tab");
    page.goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    let session = PlaywrightSession::attach(&browser)
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
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn both_apis_work_on_the_same_tab() {
    let port = serve();
    let stealthy = StealthyPlaywright::launch(headless())
        .await
        .expect("the browser starts and rustwright attaches");

    // Playwright navigates; the CDP API reads the same tab.
    stealthy
        .playwright()
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");
    let page = stealthy.cdp().default_page().await.expect("the tab");
    assert_eq!(page.title().await.expect("the CDP title"), "Playwright");

    // The CDP API navigates; Playwright reads the new document.
    page.goto(&format!("http://127.0.0.1:{port}/?again"))
        .await
        .expect("the CDP navigation");
    assert_eq!(
        stealthy
            .playwright()
            .get_text("#title")
            .await
            .expect("the heading"),
        "hello"
    );

    stealthy.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn a_page_opened_through_playwright_is_as_stealthy_as_the_first_one() {
    let port = serve();
    let mut stealthy = StealthyPlaywright::launch(headless())
        .await
        .expect("the browser starts and rustwright attaches");
    let url = format!("http://127.0.0.1:{port}/");

    stealthy.playwright().goto(&url).await.expect("first page");
    let first = stealthy
        .playwright()
        .evaluate("navigator.webdriver === true")
        .await
        .expect("script runs");
    stealthy
        .playwright_mut()
        .new_page()
        .await
        .expect("a new page");
    stealthy.playwright().goto(&url).await.expect("second page");
    let second = stealthy
        .playwright()
        .evaluate("navigator.webdriver === true")
        .await
        .expect("script runs");

    assert_eq!(first, json!(false));
    assert_eq!(second, first);
    stealthy.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn a_missing_element_is_reported_as_not_found() {
    let port = serve();
    let stealthy = StealthyPlaywright::launch(headless())
        .await
        .expect("the browser starts and rustwright attaches");
    stealthy
        .playwright()
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    // rustwright waits for the element before giving up, so this takes a while.
    let error = stealthy
        .playwright()
        .click("#not-on-this-page")
        .await
        .expect_err("there is nothing to click");
    assert!(
        matches!(error, SeleniumBaseError::ElementNotFound { .. }),
        "{error:?}"
    );

    stealthy.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
async fn the_page_actions_fill_read_evaluate_and_capture_the_attached_page() {
    let port = serve();
    let stealthy = StealthyPlaywright::launch(headless())
        .await
        .expect("the browser starts and rustwright attaches");
    let session = stealthy.playwright();
    session
        .goto(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");
    assert!(session.is_connected() && session.browser().is_connected());

    session
        .type_text("#name", "Ada")
        .await
        .expect("the field is filled");
    assert_eq!(
        session
            .evaluate("document.querySelector('#name').value")
            .await
            .expect("script runs"),
        json!("Ada")
    );

    // Everything else rustwright can do is reached through the closure, which gets a clone of
    // the page handle that `page()` lends.
    let _engine_page: seleniumbase_rs_playwright::rustwright::Page = session.page().clone();
    let title = session
        .with_page(|page| page.title(Default::default()))
        .await
        .expect("the title");
    assert_eq!(title, "Playwright");

    let shot = std::env::temp_dir().join(format!("sbrs-playwright-{port}.png"));
    session
        .screenshot(&shot)
        .await
        .expect("the screenshot is written");
    assert!(shot.metadata().expect("the file exists").len() > 0);
    let _ = std::fs::remove_file(&shot);

    // The endpoint for other clients is the one the session attached to.
    assert_eq!(stealthy.endpoint().http_url(), stealthy.cdp().http_url());

    stealthy.close().await.expect("Chrome closes");
}
