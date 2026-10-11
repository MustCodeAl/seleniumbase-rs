//! The smallest complete browser test: open a page, use it, assert on the
//! result, and close the browser.
//!
//! Copy this file as the starting point for your own test. `BaseCase::new`
//! starts a real browser, so the test closes it before reporting, even when an
//! assertion fails. `browser_test_runner` shows the one-call version of that
//! pattern.
//!
//! Run it with either of:
//!
//! ```bash
//! cargo run --example basic_test
//! sbase test --example basic_test
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`. The browser runs headless.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let site = Site::serve_demo()?;
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    let outcome = home_page_works(&mut sb, &site).await;

    // Close the browser before reporting, so a failed assertion cannot leak it.
    let closed = sb.quit().await;
    outcome.and(closed)
}

async fn home_page_works(sb: &mut BaseCase, site: &Site) -> Result<()> {
    sb.open(&site.url(DEMO_PATH)).await?;
    sb.assert_title("Web Testing Page").await?;

    sb.type_text("#myTextInput", "hello").await?;
    sb.assert_exact_text("#pText", "This Text is Green").await?;

    sb.click("#myButton").await?;
    sb.assert_exact_text("#pText", "This Text is Purple")
        .await?;

    println!("home page works");
    Ok(())
}
