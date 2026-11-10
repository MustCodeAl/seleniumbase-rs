//! Stealthy Playwright Mode: this project's API on a stealthy browser, and Playwright beside it.
//!
//! The stealthy pure-CDP browser is driven with the same API as every other page of the
//! documentation (`BrowserApi`, `ElementApi`, `ScreenshotApi` and `Page::evaluate` on an
//! `sb_cdp::Page`). Playwright joins only for what that API does not do: `PlaywrightSidecar`
//! attaches rustwright (the Rust port of Playwright) to the same tab over the DevTools
//! Protocol, and `run` hands it a closure with the rustwright page. This is Python
//! SeleniumBase's `p.chromium.connect_over_cdp(sb.get_endpoint_url())`.
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

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{BrowserApi, ElementApi, ScreenshotApi};
use seleniumbase_rs_playwright::PlaywrightSidecar;

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

    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let outcome = drive(&browser, &url).await;
    // Close the browser, whatever the run did.
    let closed = browser.close().await;
    outcome.and(closed)
}

/// Uses this project's API on the browser, then Playwright beside it on the same tab.
async fn drive(browser: &Browser, url: &str) -> seleniumbase_rs::Result<()> {
    let mut page = browser.default_page().await?;

    // This project's API: open the page, read it, check the stealth, take a screenshot.
    page.open(url).await?;
    println!("Page heading: {}", page.text("h1").await?);
    println!(
        "Flagged as automated (navigator.webdriver === true): {}",
        page.evaluate("navigator.webdriver === true").await?
    );
    let screenshot = page.save_screenshot("playwright_example.png").await?;
    println!("Screenshot saved to {}", screenshot.display());

    // Playwright, for what that API does not do, on the same tab. Attach late, detach early.
    let playwright = PlaywrightSidecar::attach(&page).await?;
    // `title` stands in for a call only Playwright has: our own API reads it with `page.title()`.
    let theirs = playwright.run(|pw| pw.title(Default::default())).await;
    let detached = playwright.close().await;
    println!("Page title (our API): {}", page.title().await?);
    println!("Page title (rustwright, same tab): {}", theirs?);
    detached?;

    // The endpoint is what other clients attach to (Playwright MCP, chrome-devtools-mcp).
    println!(
        "DevTools endpoint: {}",
        browser.devtools_endpoint().http_url()
    );
    Ok(())
}
