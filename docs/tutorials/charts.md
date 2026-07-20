# Chart Maker Guide

Generate interactive charts from your test data and save them as standalone HTML files.

## Supported chart types

- Pie
- Bar
- Line
- Area
- Column

## Single-series chart

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, Chart, ChartType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    let chart = Chart {
        title: "Browser Market Share".into(),
        chart_type: ChartType::Pie,
        labels: vec!["Chrome".into(), "Firefox".into(), "Safari".into()],
        data: vec![60.0, 25.0, 15.0],
        ..Default::default()
    };
    sb.create_chart(&chart, "market_share.html").await?;

    sb.quit().await?;
    Ok(())
}
```

## Multi-series chart

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, Chart, ChartType, ChartSeries};

let chart = Chart {
    title: "Monthly Signups".into(),
    chart_type: ChartType::Bar,
    labels: vec!["Jan".into(), "Feb".into(), "Mar".into()],
    series: vec![
        ChartSeries { name: "2024".into(), data: vec![100.0, 150.0, 200.0] },
        ChartSeries { name: "2025".into(), data: vec![120.0, 180.0, 240.0] },
    ],
    ..Default::default()
};
sb.create_chart(&chart, "signups.html").await?;
```

## Output

Each chart is a self-contained HTML file with embedded JavaScript. Open it in any browser or attach it to test reports.
