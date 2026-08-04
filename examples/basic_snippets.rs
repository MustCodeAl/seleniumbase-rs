//! # Basic SeleniumBase-rs snippets
//!
//! A cookbook-style example showing how to perform the most common browser
//! automation actions with [`BaseCase`]. Each snippet is self-contained so you
//! can copy the pieces you need into your own tests.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example basic_snippets --features playwright
//! ```
//!
//! Some snippets target a fictional login form on `example.com`. Adjust the
//! selectors and URLs to match the site you are testing.

use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ------------------------------------------------------------------
    // 1. Launch a browser
    // ------------------------------------------------------------------
    // `BrowserConfig::default()` already picks Chrome + WebDriver mode.
    // Switch to `DriverMode::Uc` for undetected-chromium style launch args.
    let config = BrowserConfig::default();
    let mut sb = BaseCase::new(config).await?;

    // ------------------------------------------------------------------
    // 2. Open a URL
    // ------------------------------------------------------------------
    sb.open("https://example.com").await?;

    // ------------------------------------------------------------------
    // 3. Find and click an element
    // ------------------------------------------------------------------
    sb.click("button#submit").await?;

    // ------------------------------------------------------------------
    // 4. Type text into an input
    // ------------------------------------------------------------------
    sb.type_text("input#username", "rustacean").await?;
    sb.type_text("input#password", "secret").await?;

    // ------------------------------------------------------------------
    // 5. Assertions
    // ------------------------------------------------------------------
    sb.assert_title("Example Domain").await?;
    sb.assert_text("h1", "Example Domain").await?;
    sb.assert_url("https://example.com/").await?;

    // ------------------------------------------------------------------
    // 6. Hover and scroll
    // ------------------------------------------------------------------
    sb.hover("nav .menu").await?;
    sb.scroll_to("footer").await?;
    sb.scroll_to_bottom().await?;

    // ------------------------------------------------------------------
    // 7. Select a dropdown option
    // ------------------------------------------------------------------
    sb.select("select#country", "United States").await?;
    sb.select_option_by_value("select#language", "en").await?;

    // ------------------------------------------------------------------
    // 8. Run JavaScript
    // ------------------------------------------------------------------
    let result = sb.execute_script("return document.title;").await?;
    println!("page title via JS: {}", result);

    // ------------------------------------------------------------------
    // 9. Cookies and storage
    // ------------------------------------------------------------------
    sb.add_cookie("session", "abc123").await?;
    let session = sb.get_cookie("session").await?;
    println!("session cookie: {:?}", session);
    let cookies = sb.get_cookies().await?;
    println!("all cookies: {:?}", cookies);

    // ------------------------------------------------------------------
    // 10. Screenshot
    // ------------------------------------------------------------------
    sb.save_screenshot_to_path("screenshot.png").await?;

    // ------------------------------------------------------------------
    // 11. Handle a simple alert
    // ------------------------------------------------------------------
    sb.execute_script("alert('hello from rust!');").await?;
    sb.accept_alert().await?;

    // ------------------------------------------------------------------
    // 12. Clean up
    // ------------------------------------------------------------------
    sb.quit().await?;

    Ok(())
}
