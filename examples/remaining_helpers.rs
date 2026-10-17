//! A tour of the smaller `BaseCase` helpers: window introspection, page
//! artifacts, on-page messages, referral click-throughs, console-log capture
//! and charts.
//!
//! Each helper is followed by a check on its effect, so a run that finishes has
//! shown that the helper did what its name says. The written-up version is
//! `docs/tutorials/remaining_helpers.md`.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example remaining_helpers
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`. The screenshot, page source and chart
//! are written to `latest_logs/`.

mod local_site;

use local_site::{ensure, Site, DEMO_PAGE, DEMO_PATH};
use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

/// Where the referral helper below leads, and what that page says.
const LANDING_PATH: &str = "/landing";
const LANDING_PAGE: &str = "<!doctype html><title>Landing</title><p id=\"landed\">You arrived.</p>";

#[tokio::main]
async fn main() -> Result<()> {
    let site = Site::serve(&[(DEMO_PATH, DEMO_PAGE), (LANDING_PATH, LANDING_PAGE)])?;
    let (url, landing) = (site.url(DEMO_PATH), site.url(LANDING_PATH));
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(demo(sb, url, landing))
    })
    .await
}

async fn demo(sb: &mut BaseCase, url: String, landing: String) -> Result<()> {
    sb.open(&url).await?;

    // Window introspection.
    let (x, y, width, height) = sb.get_window_rect().await?;
    println!("Window: x={x}, y={y}, width={width}, height={height}");

    // Page artifacts land in `latest_logs/`; the names must be plain file names.
    let screenshot = sb.save_screenshot("remaining_helpers.png").await?;
    println!("Screenshot saved to {}", screenshot.display());
    let source = sb.save_page_source("remaining_helpers.html").await?;
    println!("Page source saved to {}", source.display());

    // On-page messaging: the toast is an element, so wait for it rather than
    // sleeping.
    sb.post_success_message("All good!").await?;
    sb.wait_for_element_visible("#sb-toast-message", 5).await?;
    sb.assert_exact_text("All good!", "#sb-toast-message")
        .await?;

    // Referral helper: opens the start page, builds a link to the destination in
    // it and clicks the link, so the destination is reached by a real
    // click-through and receives the start page as its `Referer`.
    sb.generate_referral(&url, &landing, None).await?;
    println!("Referral led to {}", sb.get_current_url().await?);
    ensure(
        sb.get_current_url().await? == landing,
        "the click led to the landing page",
    )?;
    sb.assert_exact_text("You arrived.", "#landed").await?;
    sb.open(&url).await?;

    // Console log capture: once recording starts, every `console.log` call is
    // kept, whether the page makes it or you write it from Rust.
    sb.start_recording_console_logs().await?;
    sb.execute_script("console.log('from the page')").await?;
    sb.console_log_script("document.title").await?; // logs the value of a JS expression
    sb.console_log_string("Manual log entry").await?; // logs a string
    let logs = sb.get_recorded_console_logs().await?;
    println!("Console logs: {logs:?}");
    ensure(
        logs == ["from the page", "Web Testing Page", "Manual log entry"],
        "the page's line, the title and the manual line were captured",
    )?;

    // A chart with a second series, saved as a Chart.js page. Open it online
    // to see it drawn, because the page loads Chart.js from a CDN.
    sb.create_bar_chart("Sample Chart").await?;
    sb.add_data_point("Jan", 100).await?;
    sb.add_data_point("Feb", 150).await?;
    sb.add_series_to_chart(
        "Revenue",
        &[("Jan".to_owned(), 80), ("Feb".to_owned(), 120)],
    )
    .await?;
    let chart = ensure_latest_logs_dir()?.join("remaining_helpers_chart.html");
    sb.save_chart(&chart.to_string_lossy()).await?;
    println!("Chart saved to {}", chart.display());
    Ok(())
}
