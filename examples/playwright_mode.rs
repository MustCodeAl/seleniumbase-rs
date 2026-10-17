//! Playwright-backed stealth mode: drive Chromium through the `rustwright`
//! engine instead of WebDriver.
//!
//! `BaseCase::activate_playwright_mode` starts a Chromium session with
//! anti-detection launch arguments and stores it in the test case. The example
//! then navigates to a page, reads text, evaluates a script, takes a
//! screenshot, and closes the session. The `playwright` feature is **not**
//! enabled by default, so the main build does not pull in the Playwright engine.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example playwright_mode --features playwright
//! ```
//!
//! The first launch may need to download a Chromium build, so allow it network
//! access, or point `BrowserConfig::browser_binary_path` at a local Chromium.
//! The page itself is served from `127.0.0.1` by `local_site/mod.rs`, so the
//! example makes no other requests. The browser opens a visible window, because
//! `PlaywrightSession::launch` is headed on purpose (use `launch_with_config` with
//! `BrowserConfig::with_headless(true)` for CI). The screenshot is written to `latest_logs/`.

#[cfg(feature = "playwright")]
mod local_site;

#[cfg(feature = "playwright")]
#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    use local_site::{Site, DEMO_PATH};
    use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
    use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

    let url = Site::serve_demo()?.url(DEMO_PATH);

    // Playwright mode brings its own browser, so no WebDriver session is needed.
    let mut sb = BaseCase::without_session(BrowserConfig::default());
    sb.activate_playwright_mode().await?;
    let session = sb.playwright_session().ok_or_else(|| {
        SeleniumBaseError::Playwright("the Playwright session did not start".to_owned())
    })?;

    session.goto(&url).await?;
    let heading = session.get_text("h1").await?;
    println!("Page heading: {heading}");

    let title = session.evaluate("document.title").await?;
    println!("Page title: {}", title.as_str().unwrap_or_default());

    let screenshot = ensure_latest_logs_dir()?.join("playwright_example.png");
    session.screenshot(&screenshot).await?;
    println!("Screenshot saved to {}", screenshot.display());

    session.close().await?;
    sb.quit().await
}

#[cfg(not(feature = "playwright"))]
fn main() {
    eprintln!("This example requires the `playwright` feature.");
    eprintln!("Run with: cargo run --example playwright_mode --features playwright");
    std::process::exit(1);
}
