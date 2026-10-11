//! HTML/Chart.js chart generation for reports and presentations.
//!
//! The [`Chart`] type supports pie, bar, line, area, and column charts and can
//! emit both a JSON description ([`Chart::to_json`]) and an HTML page
//! ([`Chart::save`]) that draws the chart with Chart.js.
//!
//! The page is a single file, but it is not offline-capable: it loads a pinned
//! version of Chart.js from the jsDelivr CDN, so the browser that opens it
//! needs network access.
//!
//! Series are aligned by label. The first series' labels are the chart's
//! labels, and each further series is matched to them by name. A point a
//! series does not have is left out (`null`), not drawn as zero, and labels
//! only a later series has are added at the end.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::chart::{Chart, ChartSeries, ChartType};
//!
//! let mut chart = Chart::new("Sales", ChartType::Column);
//! chart.add_data_point("Q1", 100);
//! chart.add_data_point("Q2", 140);
//!
//! let mut last_year = ChartSeries::new("Last year");
//! last_year.add_data_point("Q2", 90); // no Q1 figure for last year
//! chart.add_series(last_year);
//!
//! let json: serde_json::Value = serde_json::from_str(&chart.to_json()).unwrap();
//! assert_eq!(json["labels"], serde_json::json!(["Q1", "Q2"]));
//! assert_eq!(json["datasets"][1]["data"], serde_json::json!([null, 90]));
//! ```

use crate::error::SeleniumBaseError;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// The Chart.js build every saved chart page loads. Pinned, so that a new
/// Chart.js release cannot change how a saved chart looks.
const CHART_JS_URL: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.js";

/// Supported chart types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartType {
    /// A pie chart: one slice per point.
    Pie,
    /// Horizontal bars (Highcharts' `bar`).
    Bar,
    /// A line through the points.
    Line,
    /// A line with the area below it filled.
    Area,
    /// Vertical columns (Highcharts' `column`).
    Column,
}

/// A named data series for charts.
#[derive(Clone, Debug)]
pub struct ChartSeries {
    /// Display name for the series (used in the legend).
    pub name: String,
    /// Label/value pairs for this series.
    pub data: Vec<(String, i32)>,
}

impl ChartSeries {
    /// Creates an empty series with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            data: Vec::new(),
        }
    }

    /// Adds a labeled data point to the series.
    pub fn add_data_point(&mut self, label: &str, value: i32) {
        self.data.push((label.to_owned(), value));
    }
}

/// A simple HTML/Chart.js chart generator.
#[derive(Clone, Debug)]
pub struct Chart {
    /// Chart title shown in the page and legend.
    pub title: String,
    /// Visual chart type.
    pub chart_type: ChartType,
    /// Primary label/value data points.
    pub data: Vec<(String, i32)>,
    /// Additional named series overlaid on the same chart.
    pub extra_series: Vec<ChartSeries>,
}

impl Chart {
    /// Creates an empty chart with the given title and type.
    pub fn new(title: &str, chart_type: ChartType) -> Self {
        Self {
            title: title.to_owned(),
            chart_type,
            data: Vec::new(),
            extra_series: Vec::new(),
        }
    }

    /// Adds a labeled data point to the primary series.
    pub fn add_data_point(&mut self, label: &str, value: i32) {
        self.data.push((label.to_owned(), value));
    }

    /// Adds an additional named series to the chart.
    pub fn add_series(&mut self, series: ChartSeries) {
        self.extra_series.push(series);
    }

    /// The chart's labels: the primary series' labels in order, then each
    /// label only an extra series has, in the order first seen.
    fn labels(&self) -> Vec<&str> {
        let mut labels: Vec<&str> = self.data.iter().map(|(label, _)| label.as_str()).collect();
        for series in &self.extra_series {
            for (label, _) in &series.data {
                if !labels.contains(&label.as_str()) {
                    labels.push(label);
                }
            }
        }
        labels
    }

    /// One value per label for the primary series: its points in order, then
    /// `None` for the labels only other series have.
    fn primary_values(&self, label_count: usize) -> Vec<Option<i32>> {
        let mut values: Vec<Option<i32>> =
            self.data.iter().map(|(_, value)| Some(*value)).collect();
        values.resize(label_count, None);
        values
    }

    /// One value per label for `series`, found by label name.
    fn series_values(series: &ChartSeries, labels: &[&str]) -> Vec<Option<i32>> {
        labels
            .iter()
            .map(|label| {
                series
                    .data
                    .iter()
                    .find(|(name, _)| name == label)
                    .map(|(_, value)| *value)
            })
            .collect()
    }

    /// Every series' values, aligned to [`Chart::labels`], with its name.
    fn aligned_series(&self, labels: &[&str]) -> Vec<(&str, Vec<Option<i32>>)> {
        std::iter::once((self.title.as_str(), self.primary_values(labels.len())))
            .chain(
                self.extra_series
                    .iter()
                    .map(|series| (series.name.as_str(), Self::series_values(series, labels))),
            )
            .collect()
    }

    /// Returns the chart data as a JSON object string.
    ///
    /// The JSON contains the title, type, labels, and datasets required to
    /// render the chart with Chart.js or a custom front-end. Datasets are
    /// aligned to `labels` by label name; a point a series lacks is `null`.
    pub fn to_json(&self) -> String {
        let labels = self.labels();
        let datasets: Vec<Value> = self
            .aligned_series(&labels)
            .into_iter()
            .map(|(name, values)| json!({ "label": name, "data": values }))
            .collect();
        json!({
            "title": self.title,
            "type": format!("{:?}", self.chart_type).to_lowercase(),
            "labels": labels,
            "datasets": datasets,
        })
        .to_string()
    }

    /// The Chart.js configuration object for this chart.
    fn chart_js_config(&self) -> Value {
        let labels = self.labels();
        let (kind, fill, horizontal) = match self.chart_type {
            ChartType::Pie => ("pie", false, false),
            ChartType::Bar => ("bar", false, true),
            ChartType::Column => ("bar", false, false),
            ChartType::Line => ("line", false, false),
            ChartType::Area => ("line", true, false),
        };
        let datasets: Vec<Value> = self
            .aligned_series(&labels)
            .into_iter()
            .enumerate()
            .map(|(index, (name, values))| {
                // The primary series takes a colour per point, which is what
                // pie slices and bars need; the others take one colour each.
                let colors = if index == 0 {
                    json!(labels
                        .iter()
                        .enumerate()
                        .map(|(i, _)| default_color(i))
                        .collect::<Vec<_>>())
                } else {
                    json!(default_color(index))
                };
                json!({ "label": name, "data": values, "backgroundColor": colors, "fill": fill })
            })
            .collect();
        let mut options = json!({ "responsive": true });
        if horizontal {
            options["indexAxis"] = json!("y");
        }
        json!({
            "type": kind,
            "data": { "labels": labels, "datasets": datasets },
            "options": options,
        })
    }

    /// Writes the chart to `path` as an HTML page that draws it with Chart.js.
    ///
    /// The title and labels are placed into the page as data, so any text is
    /// safe in them. The page loads Chart.js from a CDN, so it draws nothing
    /// when the browser is offline.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be written.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        let config = json_for_script(&self.chart_js_config());
        let title = html_escape(&self.title);
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="{CHART_JS_URL}"></script>
</head>
<body>
<h1>{title}</h1>
<canvas id="chart"></canvas>
<script>
new Chart(document.getElementById('chart'), {config});
</script>
</body>
</html>"#
        );

        fs::write(path.as_ref(), html)?;
        Ok(())
    }
}

/// Backwards-compatible pie chart constructor.
///
/// Prefer [`Chart`] for new code; `PieChart` is kept for existing callers and
/// simply delegates to a `Chart` of type [`ChartType::Pie`].
#[derive(Clone, Debug, Default)]
pub struct PieChart {
    /// Chart title shown in the page.
    pub title: String,
    /// Label/value pairs, one slice each.
    pub data: Vec<(String, i32)>,
}

impl PieChart {
    /// Creates an empty pie chart with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            data: Vec::new(),
        }
    }

    /// Adds a labeled slice to the pie chart.
    pub fn add_data_point(&mut self, label: &str, value: i32) {
        self.data.push((label.to_owned(), value));
    }

    /// Saves the pie chart as an HTML page; see [`Chart::save`].
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        let mut chart = Chart::new(&self.title, ChartType::Pie);
        for (label, value) in &self.data {
            chart.add_data_point(label, *value);
        }
        chart.save(path)
    }
}

fn default_color(index: usize) -> String {
    let palette = [
        "#3366cc", "#dc3912", "#ff9900", "#109618", "#990099", "#0099c6", "#dd4477", "#66aa00",
        "#b82e2e", "#316395",
    ];
    palette[index % palette.len()].to_owned()
}

/// `value` as JSON that is safe inside a `<script>` element: nothing in it can
/// end the element, open an HTML comment or start another tag.
///
/// A `<` can only occur inside a JSON string, where `<` stands for it, so
/// every one is replaced.
fn json_for_script(value: &Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// `text` made safe for HTML text and attribute values.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn saved(chart: &Chart) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chart.html");
        chart.save(&path).unwrap();
        fs::read_to_string(&path).unwrap()
    }

    /// The Chart.js configuration a saved page passes to `new Chart(...)`.
    fn embedded_config(html: &str) -> Value {
        let start = html.find("getElementById('chart'), ").expect("config call")
            + "getElementById('chart'), ".len();
        let end = html[start..]
            .find(");\n</script>")
            .expect("end of the call")
            + start;
        serde_json::from_str(&html[start..end]).expect("the embedded configuration is valid JSON")
    }

    #[test]
    fn save_pie_chart_contains_data() {
        let mut chart = Chart::new("Votes", ChartType::Pie);
        chart.add_data_point("A", 10);
        chart.add_data_point("B", 20);

        let html = saved(&chart);
        assert!(html.contains("<title>Votes</title>"));
        assert!(html.contains("chart.js"));
        let config = embedded_config(&html);
        assert_eq!(config["type"], "pie");
        assert_eq!(config["data"]["labels"], json!(["A", "B"]));
        assert_eq!(config["data"]["datasets"][0]["data"], json!([10, 20]));
        assert_eq!(
            config["data"]["datasets"][0]["backgroundColor"],
            json!(["#3366cc", "#dc3912"])
        );
    }

    #[test]
    fn save_bar_chart_contains_data() {
        let mut chart = Chart::new("Sales", ChartType::Bar);
        chart.add_data_point("Q1", 100);

        let config = embedded_config(&saved(&chart));
        assert_eq!(config["type"], "bar");
        assert_eq!(config["data"]["datasets"][0]["data"], json!([100]));
    }

    #[test]
    fn a_bar_chart_is_horizontal_and_a_column_chart_is_vertical() {
        // Chart.js draws vertical bars by default and horizontal ones with
        // `indexAxis: 'y'`; Highcharts' `bar` is horizontal, its `column` vertical.
        let bar = embedded_config(&saved(&Chart::new("t", ChartType::Bar)));
        assert_eq!(bar["options"]["indexAxis"], "y");
        let column = embedded_config(&saved(&Chart::new("t", ChartType::Column)));
        assert_eq!(column["type"], "bar");
        assert!(column["options"].get("indexAxis").is_none(), "{column}");
    }

    #[test]
    fn only_an_area_chart_fills_below_its_line() {
        for (chart_type, filled) in [
            (ChartType::Line, false),
            (ChartType::Area, true),
            (ChartType::Pie, false),
        ] {
            let config = embedded_config(&saved(&Chart::new("t", chart_type)));
            assert_eq!(
                config["data"]["datasets"][0]["fill"], filled,
                "{chart_type:?}"
            );
        }
        assert_eq!(
            embedded_config(&saved(&Chart::new("t", ChartType::Area)))["type"],
            "line"
        );
    }

    #[test]
    fn a_title_and_labels_cannot_break_out_of_the_page() {
        let mut chart = Chart::new("</title><script>alert(1)</script>", ChartType::Pie);
        chart.add_data_point("back\\slash", 1);
        chart.add_data_point("it's \"quoted\"\nline", 2);
        chart.add_data_point("</script><script>alert(2)</script>", 3);
        chart.add_data_point("<!-- x", 4);

        let html = saved(&chart);
        assert!(!html.contains("<script>alert"), "{html}");
        assert!(html.contains("<title>&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;</title>"));
        assert_eq!(
            html.matches("<script").count(),
            2,
            "only the CDN script and the chart script"
        );
        assert_eq!(html.matches("</script>").count(), 2);
        assert!(!html.contains("<!-- x"));

        // And the labels arrive exactly as given.
        let config = embedded_config(&html);
        assert_eq!(
            config["data"]["labels"],
            json!([
                "back\\slash",
                "it's \"quoted\"\nline",
                "</script><script>alert(2)</script>",
                "<!-- x"
            ])
        );
    }

    #[test]
    fn the_saved_page_pins_the_chart_js_version() {
        let html = saved(&Chart::new("t", ChartType::Pie));
        assert!(html.contains(
            r#"<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.js">"#
        ));
    }

    fn two_series_chart() -> Chart {
        let mut chart = Chart::new("t", ChartType::Line);
        chart.add_data_point("a", 1);
        chart.add_data_point("b", 2);
        let mut series = ChartSeries::new("s");
        series.add_data_point("b", 20);
        series.add_data_point("c", 30); // a label the primary series lacks
        chart.add_series(series);
        chart
    }

    #[test]
    fn series_are_matched_to_labels_by_name_and_gaps_are_not_zero() {
        let json: Value = serde_json::from_str(&two_series_chart().to_json()).unwrap();
        assert_eq!(json["labels"], json!(["a", "b", "c"]));
        assert_eq!(json["datasets"][0]["data"], json!([1, 2, null]));
        assert_eq!(json["datasets"][1]["data"], json!([null, 20, 30]));
    }

    #[test]
    fn the_json_and_the_page_agree_on_every_value() {
        let chart = two_series_chart();
        let json: Value = serde_json::from_str(&chart.to_json()).unwrap();
        let config = embedded_config(&saved(&chart));
        assert_eq!(config["data"]["labels"], json["labels"]);
        for index in 0..2 {
            assert_eq!(
                config["data"]["datasets"][index]["data"],
                json["datasets"][index]["data"]
            );
            assert_eq!(
                config["data"]["datasets"][index]["label"],
                json["datasets"][index]["label"]
            );
        }
    }

    #[test]
    fn a_chart_with_no_data_is_still_a_valid_page() {
        let config = embedded_config(&saved(&Chart::new("empty", ChartType::Column)));
        assert_eq!(config["data"]["labels"], json!([]));
        assert_eq!(config["data"]["datasets"][0]["data"], json!([]));
    }

    #[test]
    fn a_pie_chart_saves_through_the_chart_it_delegates_to() {
        let mut pie = PieChart::new("Share");
        pie.add_data_point("x", 3);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pie.html");
        pie.save(&path).unwrap();
        let config = embedded_config(&fs::read_to_string(path).unwrap());
        assert_eq!(config["type"], "pie");
        assert_eq!(config["data"]["datasets"][0]["data"], json!([3]));
    }

    #[test]
    fn the_palette_repeats_after_its_last_colour() {
        assert_eq!(default_color(0), default_color(10));
        assert_ne!(default_color(0), default_color(1));
    }
}
