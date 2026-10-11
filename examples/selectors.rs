//! The selector syntaxes every `BaseCase` method understands.
//!
//! A selector is a plain string, and `BaseCase` works out what kind it is the
//! way Python SeleniumBase does:
//!
//! | You write                | It is        |
//! |--------------------------|--------------|
//! | `#myButton`, `main h1`   | CSS          |
//! | `//h1`, `./p`, `(//a)[1]`| XPath        |
//! | `link=Read the docs`     | link text (also `link_text=`, `text=`) |
//! | `partial_link=More info` | partial link text (also `partial_link_text=`, `partial_text=`, `p_link=`, `p_text=`) |
//!
//! The example uses each kind to read or click something on the page and checks
//! the result.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example selectors
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // CSS selector.
    sb.click("#myButton").await?;
    sb.assert_text("#pText", "Purple").await?;

    // XPath selector: it starts with `/`, `./` or `(`.
    let heading = sb.get_text("//h1").await?;
    println!("h1 text: {heading}");
    sb.assert_exact_text("//h1", "Demo Page").await?;

    // Link text selector.
    sb.click("link=Read the docs").await?;
    sb.assert_url_contains("#docs-section").await?;

    // Partial link text selector.
    sb.click("partial_link=More info").await?;
    sb.assert_url_contains("#more").await?;

    println!("every selector kind found its element");
    Ok(())
}
