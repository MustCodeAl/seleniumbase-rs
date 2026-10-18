---
title: "Chart Maker Guide"
---

Generate charts from your test data and save them as standalone HTML files.
Charts are useful for visualizing benchmark results, test metrics, or any numeric
data produced during a run.

<Info>

**What you will learn**

- Which chart types are supported.
- How to create single-series and multi-series charts.
- How to export charts to HTML.

</Info>

## Supported chart types

- Pie
- Bar (vertical bars)
- Line
- Area (a filled line)
- Column (horizontal bars)

Values are `i32`.

## Single-series chart

The `Chart` type needs no browser. Build it, add data points, and save it:

```rust
use seleniumbase_rs::{Chart, ChartType};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let mut chart = Chart::new("Browser Market Share", ChartType::Pie);
chart.add_data_point("Chrome", 60);
chart.add_data_point("Firefox", 25);
chart.add_data_point("Safari", 15);
chart.save("market_share.html")?;
# let _ = std::fs::remove_file("market_share.html");
# Ok(())
# }
```

`BaseCase` has the same operations as async methods, for tests that already hold
one (`create_pie_chart`, `create_bar_chart`, `create_line_chart`,
`create_area_chart`, `create_column_chart`, then `add_data_point` and
`save_chart`). Each `create_*_chart` call replaces the current chart:

```rust,no_run
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

```rust,no_run
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

The labels come from the primary series. An extra series is matched to them by
label, and a label it lacks is drawn as `0`.

## Output

`save_chart` (and `Chart::save`) writes one HTML file that draws the chart with
[Chart.js](https://www.chartjs.org/). The file embeds the data but loads the
Chart.js library from `cdn.jsdelivr.net`, so opening it needs internet access.

<Info>

Open it in any browser or attach it to test reports.

</Info>

Related methods on `BaseCase`:

- `display_chart()` saves the chart to a temporary `chart.html` and opens it in
  the test's browser, returning the path.
- `extract_chart()` returns the chart as a JSON string with the title, type,
  labels and datasets.

`sbrs mkchart --file results.html` writes a small fixed example chart (a
Passed/Failed/Skipped bar chart) to edit by hand.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Chart file is empty | No data points added | Call `add_data_point` before `save_chart`. |
| `No chart created` error | `add_data_point` or `save_chart` before any `create_*_chart` | Create the chart first. |
| Extra series shows zeros | Its labels differ from the primary series | Use the same labels as the primary series. |
| Chart does not render | Offline: Chart.js is loaded from a CDN | Open the file on a machine with internet access, or edit the `<script src>` to point at a local copy. |
