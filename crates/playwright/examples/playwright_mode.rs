//! Stealthy Playwright Mode: Playwright attached to a stealthy CDP browser.
//!
//! `StealthyPlaywright::launch` starts a pure-CDP browser the way `seleniumbase_rs::sb_cdp`
//! does, with its stealth launch switches, and attaches rustwright (the Rust port of
//! Playwright) to it over the DevTools Protocol. Both APIs then work on the same tab: the
//! example loads a page with Playwright, reads it back with the CDP API, and takes a
//! screenshot. This is Python SeleniumBase's
//! `p.chromium.connect_over_cdp(sb.get_endpoint_url())`.
//!
//! Run it with:
//!
//! ```bash
//! cargo run -p seleniumbase-rs-playwright --example playwright_mode
//! ```
//!
//! It needs Chrome or Chromium on the machine and nothing else: the page is served from
//! `127.0.0.1` by this file, so the example makes no other requests. The browser is headless
//! (call `.headless(false)` on the launch builder for a window). The screenshot is written to
//! `latest_logs/`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::StealthyPlaywright;

const PAGE: &str = "<!doctype html><html><head><title>Stealthy Playwright</title></head>\
                    <body><h1 id=\"title\">Hello from a stealthy browser</h1></body></html>";

/// Serves `PAGE` on a free loopback port and returns the port.
fn serve() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    Ok(port)
}

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let url = format!("http://127.0.0.1:{}/", serve()?);

    let stealthy =
        StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let outcome = drive(&stealthy, &url).await;
    // Detach Playwright and close the browser, whatever the run did.
    let closed = stealthy.close().await;
    outcome.and(closed)
}

/// Uses the Playwright API and the CDP API on the one browser.
async fn drive(stealthy: &StealthyPlaywright, url: &str) -> seleniumbase_rs::Result<()> {
    let playwright = stealthy.playwright();

    // Playwright loads the page and reads it ...
    playwright.goto(url).await?;
    println!("Page heading: {}", playwright.get_text("h1").await?);
    println!(
        "Flagged as automated (navigator.webdriver === true): {}",
        playwright.evaluate("navigator.webdriver === true").await?
    );

    // ... and the CDP API sees the same tab.
    let page = stealthy.cdp().default_page().await?;
    println!("Page title (CDP): {}", page.title().await?);

    let screenshot = ensure_latest_logs_dir()?.join("playwright_example.png");
    playwright.screenshot(&screenshot).await?;
    println!("Screenshot saved to {}", screenshot.display());

    // The endpoint is what other clients attach to (Playwright MCP, chrome-devtools-mcp).
    println!("DevTools endpoint: {}", stealthy.endpoint().http_url());
    Ok(())
}
