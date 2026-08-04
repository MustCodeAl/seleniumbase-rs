# Chart Maker Guide

Generate interactive charts from your test data and save them as standalone HTML
files. Charts are useful for visualizing benchmark results, test metrics, or any
numeric data produced during a run.

## What you will learn

- Which chart types are supported.
- How to create single-series and multi-series charts.
- How to export charts to HTML.

## Supported chart types

- Pie
- Bar
- Line
- Area
- Column

## Single-series chart

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.create_pie_chart("Browser Market Share").await?;
    sb.add_data_point("Chrome", 60).await?;
    sb.add_data_point("Firefox", 25).await?;
    sb.add_data_point("Safari", 15).await?;
    sb.save_chart("market_share.html").await?;

    sb.quit().await?;
    Ok(())
}
```

## Multi-series chart

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.create_bar_chart("Monthly Signups").await?;
    sb.add_data_point("Jan", 100).await?;
    sb.add_data_point("Feb", 150).await?;
    sb.add_data_point("Mar", 200).await?;
    sb.add_series_to_chart("Last Year", &[
        ("Jan".into(), 80),
        ("Feb".into(), 120),
        ("Mar".into(), 160),
    ]).await?;
    sb.save_chart("signups.html").await?;

    sb.quit().await?;
    Ok(())
}
```

## Output

Each chart is a self-contained HTML file with embedded JavaScript. Open it in
any browser or attach it to test reports.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Chart file is empty | No data points added | Call `add_data_point` before `save_chart`. |
| Multi-series labels misaligned | Series lengths differ | Ensure every series has a value for each primary label. |
| Interactive features broken | Offline environment | Open the file in a browser with internet access or use a bundled renderer. |
