//! Build charts with the `BaseCase` chart maker and save them as HTML.
//!
//! The chart maker only collects data points and writes a Chart.js page, so no
//! browser is needed: this example uses `BaseCase::without_session`. It builds
//! a pie chart and a bar chart with a second series, saves both to
//! `latest_logs/`, and checks that the saved pages and the JSON export contain
//! the data.
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
    sb.create_pie_chart("Browser Market Share").await?;
    sb.add_data_point("Chrome", 60).await?;
    sb.add_data_point("Firefox", 25).await?;
    sb.add_data_point("Safari", 15).await?;
    let pie = dir.join("market_share.html");
    sb.save_chart(&pie.to_string_lossy()).await?;

    // A bar chart: the labels come from the primary series, and every extra
    // series is matched to them by label.
    sb.create_bar_chart("Releases per Quarter").await?;
    sb.add_data_point("Q1", 4).await?;
    sb.add_data_point("Q2", 6).await?;
    sb.add_data_point("Q3", 5).await?;
    sb.add_series_to_chart(
        "Bug fixes",
        &[
            ("Q1".to_owned(), 12),
            ("Q2".to_owned(), 9),
            ("Q3".to_owned(), 15),
        ],
    )
    .await?;
    let bars = dir.join("releases.html");
    sb.save_chart(&bars.to_string_lossy()).await?;

    // The same data is available as JSON, for a custom front end.
    let json = sb.extract_chart().await?;
    println!("bar chart as JSON: {json}");

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
