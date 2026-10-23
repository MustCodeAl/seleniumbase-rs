//! A smoke test: open a page, show an on-page message, highlight a text box,
//! and save a screenshot and the page source.
//!
//! This is the quickest way to find out whether your browser, driver and logs
//! folder work together before you debug a test of your own. By default it
//! opens the demo page this example serves from `127.0.0.1`, so it needs no
//! network. Pass a URL to smoke-test a real site instead; the selector below
//! matches the demo page's text box and the search box on Google's home page,
//! so the same run works for both.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example raw_google                              # offline
//! cargo run --example raw_google -- https://google.com/ncr    # a public site
//! ```
//!
//! Against Google, `/ncr` ("no country redirect") asks for the US site, but in
//! some regions Google still shows a consent page first. The example then fails
//! with a message saying so, instead of carrying on with a page that has no
//! search box.
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both); a public URL also needs internet access. The screenshot and
//! the page source are written to `latest_logs/`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};
use std::time::Duration;

/// The demo page's text box, or the search box on Google's home page.
const TEXT_BOX: &str = "#myTextInput, textarea[name='q'], input[name='q']";

#[tokio::main]
async fn main() -> Result<()> {
    let site = Site::serve_demo()?;
    // The first argument, if there is one, is the page to smoke-test.
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| site.url(DEMO_PATH));
    run_browser_test(BrowserConfig::default(), async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // The message disappears by itself after two seconds, and the call returns
    // once it has, so there is nothing to clean up. (A headless browser cuts
    // the time to 0.75 s.)
    sb.post_message("SeleniumBase Rust demo")
        .duration(Duration::from_secs(2))
        .await?;

    sb.wait_for_element_visible(TEXT_BOX, 10)
        .await
        .map_err(|error| {
            SeleniumBaseError::AssertionFailed(format!(
                "no text box appeared at {url} ({error}); a public site may be showing a \
                 consent page"
            ))
        })?;
    sb.highlight(TEXT_BOX).await?;
    println!("Loaded: {}", sb.get_title().await?);

    let screenshot = sb.save_screenshot_to_logs().await?;
    let source = sb.save_page_source_to_logs().await?;
    println!("Saved screenshot: {}", screenshot.display());
    println!("Saved source: {}", source.display());
    Ok(())
}
