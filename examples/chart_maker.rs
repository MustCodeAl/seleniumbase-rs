//! Build charts with the `BaseCase` chart maker and save them as HTML.
//!
//! The chart maker only collects data points and writes a Chart.js page, so no
//! browser is needed: this example uses `BaseCase::without_session`. It builds
//! a pie chart and a bar chart with a second series, saves both to
//! `latest_logs/`, and checks that the saved pages contain the data.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example chart_maker
//! ```
//!
//! It needs neither Chrome nor the network to run. The saved pages load
//! Chart.js from a CDN, so open them in a browser that is online to see the
//! drawn charts.

use std::fs;

use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::{BaseCase, BrowserConfig, Result, SeleniumBaseError};

#[tokio::main]
async fn main() -> Result<()> {
    let mut sb = BaseCase::without_session(BrowserConfig::default());
    let dir = ensure_latest_logs_dir()?;

    // A pie chart: one value per label.
    sb.create_pie_chart().title("Browser Market Share").await?;
    sb.add_data_point("Chrome", 60.0).await?;
    sb.add_data_point("Firefox", 25.0).color("orange").await?;
    sb.add_data_point("Safari", 15.0).await?;
    let pie = sb
        .save_chart()
        .folder(&dir)
        .filename("market_share.html")
        .await?;

    // A bar chart: the labels come from the first series, and every series
    // started after it is matched to them by label.
    sb.create_bar_chart()
        .title("Releases per Quarter")
        .data_name("Releases")
        .unit("count")
        .await?;
    sb.add_data_point("Q1", 4.0).await?;
    sb.add_data_point("Q2", 6.0).await?;
    sb.add_data_point("Q3", 5.0).await?;
    // A second series: the data points that follow it go into it.
    sb.add_series_to_chart().data_name("Bug fixes").await?;
    sb.add_data_point("Q1", 12.0).await?;
    sb.add_data_point("Q2", 9.0).await?;
    sb.add_data_point("Q3", 15.0).await?;
    let bars = sb
        .save_chart()
        .folder(&dir)
        .filename("releases.html")
        .await?;

    // The chart as HTML, for a page of your own or a slide.
    let html = sb.extract_chart().await?;
    println!("bar chart as HTML: {} bytes", html.len());

    // Check what was written, instead of trusting the call.
    for (path, expected) in [(&pie, "Safari"), (&bars, "Bug fixes")] {
        let html = fs::read_to_string(path)?;
        if !html.contains(expected) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "{} does not contain '{expected}'",
                path.display()
            )));
        }
        println!("saved {}", path.display());
    }
    Ok(())
}
