//! A tour of the smaller `BaseCase` helpers: window introspection, page
//! artifacts, on-page messages, referral URLs, console-log capture and charts.
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
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`. The screenshot, page source and chart
//! are written to `latest_logs/`.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
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
    sb.assert_exact_text("#sb-toast-message", "All good!")
        .await?;

    // Referral helper: adds a `ref` query parameter.
    let referral = sb.generate_referral(&url, "rust_docs")?;
    println!("Referral URL: {referral}");
    ensure(
        referral.ends_with("?ref=rust_docs"),
        "the referral URL carries the ref",
    )?;

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
