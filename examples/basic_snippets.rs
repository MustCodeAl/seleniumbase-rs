//! A cookbook of the most common [`BaseCase`] actions.
//!
//! Each numbered snippet is self-contained, so you can copy the pieces you need
//! into your own tests. The snippets drive a page this example serves from
//! `127.0.0.1` (see `local_site/mod.rs`), so every selector below exists and
//! every assertion holds.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example basic_snippets
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network. The browser runs headless; call
//! `.with_headless(false)` on the config to watch it work. The screenshot is
//! written to `latest_logs/`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{tokio, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let site = Site::serve_demo()?;

    // ------------------------------------------------------------------
    // 1. Launch a browser
    // ------------------------------------------------------------------
    // `BrowserConfig::default()` is headless Chrome driven over WebDriver.
    // Switch to `DriverMode::Uc` for undetected-chromium style launch args.
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    // Run the snippets, then close the browser whether or not they passed.
    let outcome = snippets(&mut sb, &site).await;
    let closed = sb.quit().await;
    outcome.and(closed)
}

async fn snippets(sb: &mut BaseCase, site: &Site) -> Result<()> {
    // ------------------------------------------------------------------
    // 2. Open a URL
    // ------------------------------------------------------------------
    sb.open(&site.url(DEMO_PATH)).await?;

    // ------------------------------------------------------------------
    // 3. Assertions
    // ------------------------------------------------------------------
    sb.assert_title("Web Testing Page").await?;
    sb.assert_text("Demo Page", "h1").await?;
    sb.assert_url(&site.url(DEMO_PATH)).await?;

    // ------------------------------------------------------------------
    // 4. Type text into an input and click a button
    // ------------------------------------------------------------------
    // `click` and `type_text` wait for the element to be clickable or
    // visible first, so there is no need to sleep before them.
    sb.type_text("#username", "rustacean").await?;
    sb.type_text("#password", "not-a-real-password").await?;
    sb.click("#submit").await?;
    sb.assert_text("Signed in as rustacean", "#status").await?;

    // ------------------------------------------------------------------
    // 5. Hover and scroll
    // ------------------------------------------------------------------
    sb.hover("nav .menu").await?;
    sb.assert_element_visible("#submenu").await?;
    sb.scroll_to("#footer").await?;
    sb.scroll_to_bottom().await?;

    // ------------------------------------------------------------------
    // 6. Select a dropdown option
    // ------------------------------------------------------------------
    sb.select("#country", "Germany").await?;
    sb.select_option_by_value("#language", "de").await?;
    println!("country now: {}", sb.get_value("#country").await?);

    // ------------------------------------------------------------------
    // 7. Run JavaScript
    // ------------------------------------------------------------------
    let title = sb.execute_script("return document.title;").await?;
    println!("page title via JS: {}", title.as_str().unwrap_or_default());

    // ------------------------------------------------------------------
    // 8. Cookies
    // ------------------------------------------------------------------
    // Cookies need a real origin, which is why the page is served over HTTP.
    sb.add_cookie(&seleniumbase_rs::json::json!({ "name": "session", "value": "abc123" }))
        .await?;
    let session = sb.get_cookie("session").await?;
    println!("session cookie: {}={}", session.name, session.value);

    // ------------------------------------------------------------------
    // 9. Screenshot
    // ------------------------------------------------------------------
    // The name must be a plain file name; the file lands in `latest_logs/`.
    let screenshot = sb.save_screenshot("basic_snippets.png").await?;
    println!("screenshot saved to {}", screenshot.display());

    // ------------------------------------------------------------------
    // 10. Handle an alert
    // ------------------------------------------------------------------
    // The page raises the alert a moment after the click, so wait for it.
    sb.click("#alertButton").await?;
    sb.wait_for_and_accept_alert(5).await?;

    Ok(())
}
