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
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, DriverMode, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    let config = BrowserConfig::default().with_mode(DriverMode::Cdp);
    run_browser_test(config, |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    let pdf = sb.save_as_pdf_to_logs("demo_page.pdf").await?;
    println!("PDF saved to {}", pdf.display());

    let path = pdf.to_string_lossy();
    let text = sb.get_pdf_text(&path)?;
    println!("Extracted {} characters of text", text.len());

    // The heading and the green sentence are both on the printed page.
    sb.assert_pdf_text(&path, "Demo Page")?;
    sb.assert_pdf_text(&path, "This Text is Green")?;
    Ok(())
}
