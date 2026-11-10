//! The sidecar against a real Chrome.
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
use seleniumbase_rs::{BaseCase, BrowserConfig, ElementApi, Fingerprint};
use seleniumbase_rs_playwright::rustwright::{ActionOptions, GotoOptions};
use seleniumbase_rs_playwright::PlaywrightSidecar;

/// Serves a page on a loopback port for every path: its title and heading are the path
/// (`/second` is the page titled `second`), and it has one input, `#name`.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split(' ').nth(1))
                .unwrap_or("/");
            let title = path.trim_matches('/');
            let page = format!(
                "<!doctype html><html><head><title>{title}</title></head>\
                 <body><h1>{title}</h1><input id=\"name\"></body></html>"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{page}",
                page.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

#[tokio::test]
#[ignore = "launches a real Chrome and attaches rustwright to it"]
#[serial_test::serial(chrome)]
async fn the_sidecar_is_on_the_tab_of_its_target_and_leaves_the_browser_running() {
    let base = format!("http://127.0.0.1:{}", serve());
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(&Fingerprint::windows_desktop())
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");

    // Two tabs. Our API types into the second one, so reading the typed value back through
    // rustwright proves the sidecar took the second tab and not the first.
    let first = browser.default_page().await.expect("a first tab");
    first
        .goto(&format!("{base}/first"))
        .await
        .expect("the first page loads");
    let mut second = browser
        .new_page_at(format!("{base}/second"))
        .await
        .expect("a second tab");
    second
        .type_text("#name", "Ada")
        .await
        .expect("our API types into the second tab");

    let sidecar = PlaywrightSidecar::attach(&second)
        .await
        .expect("rustwright attaches to the tab of `second`");
    assert!(sidecar.is_connected() && sidecar.browser().is_connected());
    let typed = sidecar
        .run(|pw| {
            pw.evaluate(
                "document.querySelector('#name').value",
                None,
                ActionOptions::default(),
            )
        })
        .await
        .expect("script runs in the second tab");
    assert_eq!(typed, json!("Ada"));

    // rustwright navigates; our handle follows the same tab. The browser's persona is still
    // presented after the navigation, and the tab is not flagged as automated.
    let third = format!("{base}/third");
    sidecar
        .run({
            let third = third.clone();
            move |pw| pw.goto(&third, GotoOptions::default()).map(drop)
        })
        .await
        .expect("rustwright navigates");
    assert_eq!(second.url().await.expect("our URL"), third);
    let platform = sidecar
        .run(|pw| pw.evaluate("navigator.platform", None, ActionOptions::default()))
        .await
        .expect("script runs");
    assert_eq!(platform, json!("Win32"));
    let flagged = sidecar
        .run(|pw| {
            pw.evaluate(
                "navigator.webdriver === true",
                None,
                ActionOptions::default(),
            )
        })
        .await
        .expect("script runs");
    assert_eq!(flagged, json!(false));

    // A `BaseCase` on the same browser, through a direct CDP attachment, has a tab of its
    // own: a sidecar beside it is on that tab, whichever one the case took.
    let mut sb = BaseCase::new(BrowserConfig::default().with_cdp_url(browser.http_url()))
        .await
        .expect("a case attaches to the running browser");
    let case_url = sb.current_url().await.expect("the case's URL");
    let beside_case = PlaywrightSidecar::attach(&sb)
        .await
        .expect("rustwright attaches to the case's tab");
    let seen = beside_case
        .run(|pw| Ok(pw.url()))
        .await
        .expect("rustwright reads the URL");
    assert_eq!(seen, case_url);
    beside_case.close().await.expect("detaches from the case");
    sb.quit().await.expect("the case detaches");

    // Closing detaches: the browser and the tab keep running and answering.
    sidecar.close().await.expect("detaches");
    assert_eq!(second.title().await.expect("the tab is alive"), "third");

    // Closing again, once the browser itself is gone, is not an error.
    browser.close().await.expect("Chrome closes");
    sidecar
        .close()
        .await
        .expect("a sidecar whose browser is gone closes quietly");
}
