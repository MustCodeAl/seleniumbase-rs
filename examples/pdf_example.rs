//! Print a page to PDF, then read the text back out of the PDF.
//!
//! `save_as_pdf_to_logs` asks Chrome to print the current page, writes the file
//! into `latest_logs/` and returns its path. `get_pdf_text` and
//! `assert_pdf_text` then read the file back, which is how you check a
//! generated report or a downloaded PDF.
//!
//! Printing uses the Chrome DevTools Protocol, so the browser must be started
//! in `DriverMode::Cdp` (or `Uc`); in plain WebDriver mode the call returns a
//! "CDP is unavailable" error.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example pdf_example
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, tokio, BaseCase, BrowserConfig, DriverMode, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    let config = BrowserConfig::builder()
        .mode(DriverMode::Cdp)
        .build()
        .expect("a valid config");
    run_browser_test(config, async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    let pdf = sb.save_as_pdf_to_logs("demo_page.pdf").await?;
    println!("PDF saved to {}", pdf.display());

    // A path is as good as a string for the file name.
    let text = sb.pdf_text(&pdf)?;
    println!("Extracted {} characters of text", text.len());

    // The heading and the green sentence are both on the printed page.
    sb.assert_pdf_text(&pdf, "Demo Page")?;
    sb.assert_pdf_text(&pdf, "This Text is Green")?;
    Ok(())
}
