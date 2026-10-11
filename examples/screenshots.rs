//! Save screenshots and page source as test artifacts.
//!
//! All four artifact calls write into the `latest_logs/` folder and return the
//! path they used:
//!
//! * `save_screenshot(name)` and `save_page_source(name)` use a name you choose
//!   and replace an earlier file of that name, which is what a re-run wants.
//! * `save_screenshot_to_logs()` and `save_page_source_to_logs()` choose a
//!   fresh name every time, so they never overwrite anything.
//! * `save_element_as_image_file(css, name)` captures one element.
//!
//! The example then checks what was written: the screenshots start with the PNG
//! signature and the page source contains the page's heading.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example screenshots
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use std::fs;
use std::path::Path;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    let named = sb.save_screenshot("demo_page.png").await?;
    let unique = sb.save_screenshot_to_logs().await?;
    let heading = sb
        .save_element_as_image_file("h1", "demo_heading.png")
        .await?;
    for path in [&named, &unique, &heading] {
        check_png(path)?;
    }

    let source = sb.save_page_source("demo_page.html").await?;
    let source_in_logs = sb.save_page_source_to_logs().await?;
    for path in [&source, &source_in_logs] {
        let html = fs::read_to_string(path)?;
        if !html.contains("Demo Page") {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "{} does not contain the page heading",
                path.display()
            )));
        }
        println!("saved {} ({} bytes)", path.display(), html.len());
    }
    Ok(())
}

/// Fails unless `path` is a non-empty PNG file.
fn check_png(path: &Path) -> Result<()> {
    let bytes = fs::read(path)?;
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(SeleniumBaseError::AssertionFailed(format!(
            "{} is not a PNG file",
            path.display()
        )));
    }
    println!("saved {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}
