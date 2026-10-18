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

<Visibility for="agents">

Summary for AI exports: `Chart::new(title, ChartType::Pie)` needs no browser; call `add_data_point(label, value)` (values are `i32`) and `save("file.html")`. On `BaseCase`, `create_pie_chart`, `create_bar_chart`, `create_line_chart`, `create_area_chart` or `create_column_chart` start a chart, then `add_data_point`, `add_series_to_chart` and `save_chart`. The saved HTML file draws the chart with Chart.js loaded from `cdn.jsdelivr.net`, so opening it needs internet access.

</Visibility>

## Supported chart types

- Pie
- Bar (vertical bars)
- Line
- Area (a filled line)
- Column (horizontal bars)

Values are `i32`.

## Single-series chart

<Tabs>

<Tab title="Chart (no browser)" icon="chart-pie">

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

</Tab>

<Tab title="BaseCase" icon="window-maximize">

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

</Tab>

</Tabs>

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

<Tip>

Open it in any browser or attach it to test reports.

</Tip>

Related methods on `BaseCase`:

- `display_chart()` saves the chart to a temporary `chart.html` and opens it in
  the test's browser, returning the path.
- `extract_chart()` returns the chart as a JSON string with the title, type,
  labels and datasets.

`sbrs mkchart --file results.html` writes a small fixed example chart (a
Passed/Failed/Skipped bar chart) to edit by hand.

## Troubleshooting

<AccordionGroup>

<Accordion title="Chart file is empty" icon="file">

**Likely cause:** No data points added.

**Fix:** Call `add_data_point` before `save_chart`.

</Accordion>

<Accordion title="No chart created error" icon="circle-exclamation">

**Likely cause:** `add_data_point` or `save_chart` before any `create_*_chart`.

**Fix:** Create the chart first.

</Accordion>

<Accordion title="Extra series shows zeros" icon="chart-line">

**Likely cause:** Its labels differ from the primary series.

**Fix:** Use the same labels as the primary series.

</Accordion>

<Accordion title="Chart does not render" icon="wifi">

**Likely cause:** Offline: Chart.js is loaded from a CDN.

**Fix:** Open the file on a machine with internet access, or edit the `<script src>` to point at a local copy.

</Accordion>

</AccordionGroup>
