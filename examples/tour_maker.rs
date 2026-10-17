//! Tour maker: build a guided tour of a page, play it in the browser, and export
//! it as a static HTML outline.
//!
//! A tour is a list of steps, each with a message and an optional element to
//! spotlight. `play_tour` injects the default tour into the open page: a dark
//! overlay around the target and a box with the message and Next/End buttons.
//! `export_tour` writes the steps to an HTML file instead, for documentation.
//! The other themes (`create_shepherd_tour`, `create_introjs_tour`,
//! `create_driverjs_tour`) load their libraries from a CDN, so they need the
//! network; the default theme used here does not.
//!
//! The example plays the tour, clicks through the first step, and checks the
//! box each time, then exports the tour to `latest_logs/` and checks the file.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example tour_maker
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use std::fs;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    sb.create_tour("Demo Tour").await?;
    sb.add_tour_step("Welcome to the demo page.", Some("h1"))
        .await?;
    sb.add_tour_step("Type something here.", Some("#myTextInput"))
        .await?;
    sb.add_tour_step("Then click this button.", Some("#myButton"))
        .await?;

    // Play it: the box appears with the first step's message...
    sb.play_tour().await?;
    sb.wait_for_element_visible("#sb-rs-tour-box", 5).await?;
    sb.assert_text("#sb-rs-tour-box", "Welcome to the demo page.")
        .await?;

    // ...and Next moves on to the second.
    sb.click("#sb-rs-tour-next").await?;
    sb.assert_text("#sb-rs-tour-box", "Type something here.")
        .await?;
    println!("the tour played and advanced to step 2");

    // Export the same steps as a static page.
    let outline = ensure_latest_logs_dir()?.join("demo_tour.html");
    sb.export_tour(&outline.to_string_lossy()).await?;
    let html = fs::read_to_string(&outline)?;
    ensure(
        ["Demo Tour", "Welcome to the demo page.", "#myButton"]
            .iter()
            .all(|expected| html.contains(expected)),
        "the exported page lists the tour name, messages and targets",
    )?;
    println!("Tour exported to {}", outline.display());
    Ok(())
}
