//! `--crumbs` against a real Chrome: a reused browser keeps its cookies unless
//! crumbs are asked for.
//!
//! One test, because the setting is the process's, and the three steps need
//! one browser in a row. A first `BaseCase` sets a cookie and gives the browser
//! back. A second one, with crumbs off (Python's default), attaches to the same
//! session and finds the cookie still there. With
//! [`set_crumbs(true)`](seleniumbase_rs::browser::reuse::set_crumbs) a third one
//! attaches to the same session again and finds no cookie.
//!
//! It is ignored by default (it needs Chrome and chromedriver). Run it alone:
//!
//! ```bash
//! cargo test --test crumbs_chrome -- --ignored
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use seleniumbase_rs::browser::reuse::{reused_sessions, set_crumbs, set_idle_timeout};
use seleniumbase_rs::{shutdown_reused_sessions, BaseCase, BrowserConfig};
use serde_json::Value;

fn config() -> BrowserConfig {
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
            let body = "<html><head><title>crumbs</title></head><body>page</body></html>";
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

/// The cookies the page at `page` sees in a case that attached to the kept
/// browser, and the session it attached to.
async fn cookies_seen(page: &str) -> (Value, Option<String>) {
    let mut sb = BaseCase::new(config())
        .await
        .expect("attach to the browser");
    let session = sb.session_id();
    sb.open(page).await.expect("open the page");
    let cookies = sb.execute_script("return document.cookie;").await.unwrap();
    sb.quit().await.expect("give the browser back");
    (cookies, session)
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_reused_browser_keeps_its_cookies_unless_crumbs_are_asked_for() {
    set_idle_timeout(Duration::from_secs(60));
    set_crumbs(false);
    let page = serve_page();

    let mut first = BaseCase::new(config()).await.expect("start Chrome");
    first.open(&page).await.expect("open the page");
    first
        .execute_script("document.cookie = 'left=by-the-first-test; path=/'; return 1;")
        .await
        .expect("set a cookie");
    let session = first.session_id();
    first.quit().await.expect("give the browser back");
    assert_eq!(reused_sessions().len(), 1, "the browser is kept");

    // Crumbs off: the same browser, the cookie still there.
    let (cookies, second_session) = cookies_seen(&page).await;
    assert_eq!(
        second_session, session,
        "the second case reuses the browser"
    );
    assert_eq!(
        cookies,
        Value::from("left=by-the-first-test"),
        "without crumbs the cookie of the first test stays"
    );

    // Crumbs on: the same browser again, the cookie gone.
    set_crumbs(true);
    let (cookies, third_session) = cookies_seen(&page).await;
    assert_eq!(third_session, session, "the third case reuses the browser");
    assert_eq!(
        cookies,
        Value::from(""),
        "with crumbs the cookie is deleted"
    );

    assert_eq!(shutdown_reused_sessions(), 1, "the kept browser is closed");
}
