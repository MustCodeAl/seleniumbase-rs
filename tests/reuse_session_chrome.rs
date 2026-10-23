//! A real-browser check of `--reuse-session` (`BrowserConfig::with_reuse_session`).
//!
//! Two `#[tokio::test]` functions, each on its own Tokio runtime, share one
//! Chrome started by one chromedriver. The first sets a cookie and an item of
//! local storage and gives the browser back. The second attaches to the same
//! session (the same session id, and no new browser), finds the page at
//! `about:blank` and the cookie and the storage gone, gives the browser back,
//! and then waits for the reaper: once nobody has held the browser for the idle
//! timeout, the browser is closed and the chromedriver stopped (its port no
//! longer answers).
//!
//! It is ignored by default. Run both tests in one process, alone on the
//! machine:
//!
//! ```bash
//! cargo test --test reuse_session_chrome -- --ignored
//! ```
//!
//! Under cargo-nextest each test is a process of its own, so nothing can be
//! shared and the second test fails waiting for the first.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use seleniumbase_rs::browser::reuse::{reused_sessions, set_crumbs, set_idle_timeout};
use seleniumbase_rs::{BaseCase, BrowserConfig};
use serde_json::Value;

/// How long the browser stays open once nobody holds it.
const IDLE: Duration = Duration::from_secs(3);

/// What the first test leaves for the second.
#[derive(Clone, Debug)]
struct First {
    session_id: String,
    page: String,
    driver_url: String,
}

static FIRST: Mutex<Option<First>> = Mutex::new(None);

fn config() -> BrowserConfig {
    // The second test checks that the first one's cookie is cleaned away, which
    // is what crumbs ask for; `crumbs_chrome.rs` checks the default.
    set_crumbs(true);
    BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_reuse_session(true)
}

/// Serves one page on a loopback port, since cookies need an `http` origin,
/// and returns its address.
fn serve_page() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = "<html><head><title>reuse</title></head><body>page</body></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/")
}

/// The `host:port` of a WebDriver server address.
fn host_and_port(url: &str) -> &str {
    url.trim_start_matches("http://").trim_end_matches('/')
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_first_test_leaves_its_browser_open_with_a_cookie() {
    set_idle_timeout(IDLE);
    let page = serve_page();
    let mut sb = BaseCase::new(config()).await.expect("start Chrome");
    sb.open(&page).await.expect("open the page");
    sb.execute_script(
        "document.cookie = 'left=by-the-first-test; path=/'; \
         localStorage.setItem('left', 'by-the-first-test'); return 1;",
    )
    .await
    .expect("set a cookie and a stored item");
    let cookie = sb.get_cookie("left").await.expect("the cookie is set");
    assert_eq!(cookie.value, "by-the-first-test");

    let session_id = sb.session_id().expect("a session");
    let driver_url = reused_sessions()
        .into_iter()
        .find(|kept| kept.session_id == session_id)
        .expect("the browser is kept for reuse")
        .server_url;
    sb.quit().await.expect("give the browser back");
    assert!(
        TcpStream::connect(host_and_port(&driver_url)).is_ok(),
        "quitting left chromedriver running"
    );

    *FIRST.lock().unwrap() = Some(First {
        session_id,
        page,
        driver_url,
    });
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_second_test_gets_the_same_browser_cleaned_and_the_reaper_closes_it() {
    set_idle_timeout(IDLE);
    // The first test runs at the same time on another thread, or before this
    // one with --test-threads=1. Wait until it has given the browser back.
    let deadline = Instant::now() + Duration::from_secs(120);
    let first = loop {
        let left = FIRST.lock().unwrap().clone();
        if let Some(first) = left {
            break first;
        }
        assert!(
            Instant::now() < deadline,
            "the first test never gave its browser back (both tests must run in one process)"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    let mut sb = BaseCase::new(config())
        .await
        .expect("attach to the kept browser");
    assert_eq!(
        sb.session_id().as_deref(),
        Some(first.session_id.as_str()),
        "the second test attached to the first test's browser"
    );
    assert_eq!(reused_sessions().len(), 1, "no second browser was started");
    assert_eq!(sb.get_current_url().await.unwrap(), "about:blank");

    sb.open(&first.page)
        .await
        .expect("open the same page again");
    let cookies = sb.execute_script("return document.cookie;").await.unwrap();
    assert_eq!(cookies, Value::from(""), "the first test's cookie is gone");
    let stored = sb
        .execute_script("return localStorage.getItem('left');")
        .await
        .unwrap();
    assert_eq!(stored, Value::Null, "the first test's storage is gone");
    sb.quit().await.expect("give the browser back");

    // Nobody holds the browser now: the reaper closes it after the idle
    // timeout, and stops chromedriver.
    let deadline = Instant::now() + IDLE + Duration::from_secs(20);
    while !reused_sessions().is_empty() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        reused_sessions().is_empty(),
        "the reaper did not close the browser"
    );
    let address = host_and_port(&first.driver_url).to_owned();
    while TcpStream::connect(&address).is_ok() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        TcpStream::connect(&address).is_err(),
        "chromedriver still answers at {address}"
    );
}
