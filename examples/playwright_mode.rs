//! Playwright-backed stealth mode example.
//!
//! Build with:
//!
//! ```bash
//! cargo run --example playwright_mode --features playwright
//! ```
//!
//! This example demonstrates launching Chromium via the optional Playwright
//! integration, navigating to a page, interacting with it, and taking a
//! screenshot. The `playwright` feature is **not** enabled by default so that
//! the main build does not depend on the Playwright crate.

#[cfg(feature = "playwright")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use seleniumbase_rs::{BaseCase, BrowserConfig};
    use std::path::Path;

    let config = BrowserConfig::default();
    let mut sb = BaseCase::new(config).await?;

    sb.activate_playwright_mode().await?;

    if let Some(session) = sb.playwright_session() {
        session.goto("https://example.com").await?;
        let heading = session.get_text("h1").await?;
        println!("Page heading: {heading}");

        let title: String = session
            .evaluate("() => document.title")
            .await?
            .as_str()
            .unwrap_or("")
            .to_owned();
        println!("Page title: {title}");

        let screenshot_path = Path::new("playwright_example.png");
        session.screenshot(screenshot_path).await?;
        println!("Screenshot saved to {}", screenshot_path.display());

        session.close().await?;
    }

    sb.quit().await?;
    Ok(())
}

#[cfg(not(feature = "playwright"))]
fn main() {
    eprintln!("This example requires the `playwright` feature.");
    eprintln!("Run with: cargo run --example playwright_mode --features playwright");
    std::process::exit(1);
}
