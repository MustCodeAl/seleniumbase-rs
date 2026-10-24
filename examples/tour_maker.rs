//! Tour maker: build a guided tour of a page, play it in the browser, and export
//! it as a JavaScript file.
//!
//! A tour is a list of steps, each with a message (HTML) and an optional
//! element to point at. `play_tour` shows it with a JavaScript library (here
//! Shepherd, in its dark look) and waits until it ends. With an `interval` it
//! moves on by itself, a new step every few seconds; without one, a person
//! clicks Next. `export_tour` writes the same tour to `tours_exported/` as a
//! script that plays it in any browser console.
//!
//! The libraries come from CDNs, so playing needs the network, and the tour is
//! skipped in a headless browser (nobody watches). The example runs headed
//! and checks the exported file either way.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example tour_maker
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both). The page is served from `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use std::fs;
use std::time::Duration;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::api::tour::{ShepherdTheme, TourAlignment};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    sb.create_shepherd_tour().theme(ShepherdTheme::Dark).await?;
    sb.add_tour_step("Welcome to the demo page.")
        .selector("h1")
        .title("Hello")
        .await?;
    sb.add_tour_step("Type something here.")
        .selector("#myTextInput")
        .alignment(TourAlignment::Bottom)
        .await?;
    sb.add_tour_step("Then click this button.")
        .selector("#myButton")
        .await?;

    // Play it: the tour moves on by itself and the call returns at its end.
    sb.play_tour().interval(Duration::from_secs(2)).await?;
    println!("the tour played to its end");

    // Export the same steps as a script for the console of any browser.
    let script_path = sb.export_tour().filename("demo_tour.js").await?;
    let script = fs::read_to_string(&script_path)?;
    ensure(
        ["Welcome to the demo page.", "#myButton", "Shepherd"]
            .iter()
            .all(|expected| script.contains(expected)),
        "the exported script has the messages, the selectors and the library",
    )?;
    println!("Tour exported to {}", script_path.display());
    Ok(())
}
