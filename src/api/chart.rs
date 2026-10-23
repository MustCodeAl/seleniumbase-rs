//! HTML/Chart.js chart generation for reports and presentations.
//!
//! A [`Chart`] is what Python's `create_pie_chart`, `create_bar_chart`,
//! `create_column_chart`, `create_line_chart` and `create_area_chart` make: a
//! [`ChartType`], a title and subtitle, a unit for the values, the options
//! `zero` (line and area charts start the value axis at zero), `libs`, `labels`
//! and `legend`, and one or more series of labelled values. Python draws the
//! chart with Highcharts, which is not free for commercial use; this one draws
//! it with Chart.js, which is MIT-licensed, and maps the options:
//!
//! | Option | In the page |
//! |--------|-------------|
//! | title, subtitle | the chart's title and subtitle |
//! | unit | the title of the value axis, and a suffix in the tooltip |
//! | zero | the value axis begins at zero (line and area charts) |
//! | legend | the legend is shown |
//! | labels | the value of each point is drawn on it |
//! | libs | whether [`Chart::to_html`] starts with the `<script src>` of Chart.js |
//!
//! [`Chart::to_html`] is the chart as a piece of HTML that can go on any page,
//! on a slide of a [`Presentation`](crate::api::presentation::Presentation)
//! too, and [`Chart::save`] writes it as a page of its own.
//!
//! The page is a single file, but it is not offline-capable: it loads a pinned
//! version of Chart.js from the jsDelivr CDN, so the browser that opens it
//! needs network access.
//!
//! The first series' labels are the chart's labels, and each further series is
//! matched to them by name. A point a series does not have is left out
//! (`null`), not drawn as zero, and labels only a later series has are added at
//! the end.
//!
//! [`BaseCase`](crate::api::base_case::BaseCase) keeps any number of named
//! charts (`create_pie_chart`, `add_data_point`, `save_chart`, `display_chart`,
//! `extract_chart`); this module is the part that needs no browser.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::chart::{Chart, ChartSeries, ChartType};
//!
//! let mut chart = Chart::new("Sales", ChartType::Column);
//! chart.add_data_point("Q1", 100.0);
//! chart.add_data_point("Q2", 140.5);
//!
//! let mut last_year = ChartSeries::new("Last year");
//! last_year.add_data_point("Q2", 90.0); // no Q1 figure for last year
//! chart.add_series(last_year);
//!
//! let json: serde_json::Value = serde_json::from_str(&chart.to_json()).unwrap();
//! assert_eq!(json["labels"], serde_json::json!(["Q1", "Q2"]));
//! assert_eq!(json["datasets"][0]["data"], serde_json::json!([100, 140.5]));
//! assert_eq!(json["datasets"][1]["data"], serde_json::json!([null, 90]));
//! ```

use crate::api::presentation::DEFAULT_NAME;
use crate::artifacts::bare_file_name;
use crate::error::SeleniumBaseError;
use serde_json::{json, Map, Value};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The folder, in the working directory, where charts are saved: Python's
/// `Charts.SAVED_FOLDER`.
pub const SAVED_FOLDER: &str = "saved_charts";

/// The file a chart is saved to when no name is given.
pub const DEFAULT_FILENAME: &str = "my_chart.html";

/// The element that loads the Chart.js build every chart page uses. Pinned, so
/// that a new Chart.js release cannot change how a saved chart looks.
pub(crate) const CHART_JS_TAG: &str =
    r#"<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.js"></script>"#;

/// What a chart's script does before it draws: the unit as a tooltip suffix,
/// and the plugin that draws the value of each point.
const CHART_SCRIPT: &str = r"const labelOptions = (config.options.plugins || {}).sbValueLabels || {};
if (labelOptions.unit) {
  config.options.plugins.tooltip = { callbacks: { label: function (item) {
    return (item.dataset.label ? item.dataset.label + ': ' : '') + item.formatted + ' ' + labelOptions.unit;
  } } };
}
config.plugins = [{
  id: 'sbValueLabels',
  afterDatasetsDraw: function (chart, args, options) {
    if (!options || !options.display) { return; }
    const ctx = chart.ctx;
    ctx.save();
    ctx.font = '12px sans-serif';
    ctx.fillStyle = '#222';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    chart.data.datasets.forEach(function (dataset, index) {
      chart.getDatasetMeta(index).data.forEach(function (element, position) {
        const value = dataset.data[position];
        if (value === null || value === undefined) { return; }
        const at = element.tooltipPosition();
        ctx.fillText(String(value), at.x, at.y);
      });
    });
    ctx.restore();
  }
}];";

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

/// One labelled value of a series.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartPoint {
    /// The label of the point: a slice, a bar, a place on the horizontal axis.
    pub label: String,
    /// The value of the point.
    pub value: f64,
    /// A CSS colour for this point, instead of the series' own.
    pub color: Option<String>,
}

/// A named data series for charts.
#[derive(Clone, Debug)]
pub struct ChartSeries {
    /// Display name for the series (used in the legend).
    pub name: String,
    /// The points of this series, in order.
    pub data: Vec<ChartPoint>,
}

impl ChartSeries {
    /// Creates an empty series with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            data: Vec::new(),
        }
    }

    /// Adds a labeled data point to the series.
    pub fn add_data_point(&mut self, label: &str, value: f64) {
        self.data.push(ChartPoint {
            label: label.to_owned(),
            value,
            color: None,
        });
    }

    /// Adds a labeled data point drawn in the CSS colour `color`.
    pub fn add_data_point_with_color(&mut self, label: &str, value: f64, color: &str) {
        self.data.push(ChartPoint {
            label: label.to_owned(),
            value,
            color: Some(color.to_owned()),
        });
    }
}

/// A chart that is drawn with Chart.js.
#[derive(Clone, Debug)]
pub struct Chart {
    chart_type: ChartType,
    title: String,
    subtitle: String,
    unit: String,
    zero: bool,
    libs: bool,
    labels: bool,
    legend: bool,
    id: String,
    /// The series. Never empty: the first one is where
    /// [`add_data_point`](Chart::add_data_point) starts, and where the labels
    /// come from.
    series: Vec<ChartSeries>,
}

impl Chart {
    /// Creates an empty chart with the given title and type. It shows its
    /// legend and the values of its points, and has no subtitle or unit.
    #[must_use]
    pub fn new(title: &str, chart_type: ChartType) -> Self {
        Self {
            chart_type,
            title: title.to_owned(),
            subtitle: String::new(),
            unit: String::new(),
            zero: false,
            libs: true,
            labels: true,
            legend: true,
            id: "sb-chart".to_owned(),
            series: vec![ChartSeries::new("Series 1")],
        }
    }

    /// Sets the subtitle shown under the title.
    #[must_use]
    pub fn with_subtitle(mut self, subtitle: &str) -> Self {
        self.subtitle = subtitle.to_owned();
        self
    }

    /// Names the first series (Python's `data_name`), which the legend and
    /// the tooltip show. An empty name keeps `Series 1`.
    #[must_use]
    pub fn with_data_name(mut self, data_name: &str) -> Self {
        if !data_name.trim().is_empty() {
            data_name.clone_into(&mut self.series[0].name);
        }
        self
    }

    /// Sets the unit of the values: the title of the value axis, and a suffix
    /// in the tooltip.
    #[must_use]
    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_owned();
        self
    }

    /// Makes the value axis of a line or an area chart begin at zero (Python's
    /// `zero`). Other charts ignore it.
    #[must_use]
    pub fn with_zero(mut self, zero: bool) -> Self {
        self.zero = zero;
        self
    }

    /// Chooses whether [`to_html`](Self::to_html) starts with the element that
    /// loads Chart.js (Python's `libs`; the default is that it does).
    #[must_use]
    pub fn with_libs(mut self, libs: bool) -> Self {
        self.libs = libs;
        self
    }

    /// Chooses whether the value of each point is drawn on the chart (Python's
    /// `labels`; the default is that it is).
    #[must_use]
    pub fn with_labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    /// Chooses whether the legend is shown (Python's `legend`; the default is
    /// that it is).
    #[must_use]
    pub fn with_legend(mut self, legend: bool) -> Self {
        self.legend = legend;
        self
    }

    /// Sets the `id` of the chart's `<canvas>`. Two charts on one page need
    /// two ids. Every character other than a letter, a digit or `-` is
    /// replaced by `_` and its code in hexadecimal, so that different names
    /// give different ids, and the id is safe in HTML and in a script.
    #[must_use]
    pub fn with_id(mut self, id: &str) -> Self {
        let mut safe = String::with_capacity(id.len());
        for character in id.chars() {
            if character.is_ascii_alphanumeric() || character == '-' {
                safe.push(character);
            } else {
                let _ = write!(safe, "_{:x}", u32::from(character));
            }
        }
        self.id = if safe.is_empty() {
            "sb-chart".to_owned()
        } else {
            safe
        };
        self
    }

    /// The title of the chart.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The type of the chart.
    #[must_use]
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// The `id` of the chart's `<canvas>`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The series of the chart, the first one first.
    #[must_use]
    pub fn series(&self) -> &[ChartSeries] {
        &self.series
    }

    /// The series the next data point goes to: the last one.
    fn current_series(&mut self) -> &mut ChartSeries {
        let last = self.series.len() - 1;
        &mut self.series[last]
    }

    /// Adds a labeled data point to the current series: the first one, or the
    /// one [`start_series`](Self::start_series) made last.
    pub fn add_data_point(&mut self, label: &str, value: f64) {
        self.current_series().add_data_point(label, value);
    }

    /// [`add_data_point`](Self::add_data_point) with a CSS colour for the point.
    pub fn add_data_point_with_color(&mut self, label: &str, value: f64, color: &str) {
        self.current_series()
            .add_data_point_with_color(label, value, color);
    }

    /// Starts a new series, which the next data points fill (Python's
    /// `add_series_to_chart`). Without a `name` it is called `Series N`.
    pub fn start_series(&mut self, name: Option<&str>) {
        let name = match name.filter(|name| !name.trim().is_empty()) {
            Some(name) => name.to_owned(),
            None => format!("Series {}", self.series.len() + 1),
        };
        self.series.push(ChartSeries {
            name,
            data: Vec::new(),
        });
    }

    /// Adds a series with its points already in it, after the ones there are.
    pub fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }

    /// The chart's labels: the first series' labels in order, then each label
    /// only a later series has, in the order first seen.
    fn labels(&self) -> Vec<&str> {
        let mut labels: Vec<&str> = self.series[0]
            .data
            .iter()
            .map(|point| point.label.as_str())
            .collect();
        for series in &self.series[1..] {
            for point in &series.data {
                if !labels.contains(&point.label.as_str()) {
                    labels.push(&point.label);
                }
            }
        }
        labels
    }

    /// One entry per label for series number `index`: the first series' points
    /// in order, then `None` for the labels only other series have; any other
    /// series' point of that label, or `None`.
    fn points_for(&self, index: usize, labels: &[&str]) -> Vec<Option<&ChartPoint>> {
        let series = &self.series[index];
        if index == 0 {
            let mut points: Vec<Option<&ChartPoint>> = series.data.iter().map(Some).collect();
            points.resize(labels.len(), None);
            points
        } else {
            labels
                .iter()
                .map(|label| series.data.iter().find(|point| point.label == *label))
                .collect()
        }
    }

    /// Returns the chart data as a JSON object string.
    ///
    /// The JSON contains the title, type, labels, and datasets required to
    /// render the chart with Chart.js or a custom front-end. Datasets are
    /// aligned to `labels` by label name; a point a series lacks is `null`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let labels = self.labels();
        let datasets: Vec<Value> = (0..self.series.len())
            .map(|index| {
                let values: Vec<Value> = self
                    .points_for(index, &labels)
                    .into_iter()
                    .map(|point| point.map_or(Value::Null, |point| number(point.value)))
                    .collect();
                json!({ "label": self.series[index].name, "data": values })
            })
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
        let (kind, horizontal) = match self.chart_type {
            ChartType::Pie => ("pie", false),
            ChartType::Bar => ("bar", true),
            ChartType::Column => ("bar", false),
            ChartType::Line | ChartType::Area => ("line", false),
        };
        let fill = self.chart_type == ChartType::Area;
        let datasets: Vec<Value> = (0..self.series.len())
            .map(|index| {
                let series_color = default_color(index);
                let points = self.points_for(index, &labels);
                let values: Vec<Value> = points
                    .iter()
                    .map(|point| point.map_or(Value::Null, |point| number(point.value)))
                    .collect();
                // A pie takes a colour per slice; the other charts one per
                // series, and a point only has its own colour if it was given.
                let point_colors: Vec<String> = points
                    .iter()
                    .enumerate()
                    .map(|(position, point)| {
                        point
                            .and_then(|point| point.color.clone())
                            .unwrap_or_else(|| {
                                if self.chart_type == ChartType::Pie {
                                    default_color(position)
                                } else {
                                    series_color.clone()
                                }
                            })
                    })
                    .collect();
                let any_given = points.iter().flatten().any(|point| point.color.is_some());
                let mut dataset = json!({
                    "label": self.series[index].name,
                    "data": values,
                    "fill": fill,
                });
                match self.chart_type {
                    ChartType::Pie => dataset["backgroundColor"] = json!(point_colors),
                    ChartType::Bar | ChartType::Column => {
                        dataset["backgroundColor"] = if any_given {
                            json!(point_colors)
                        } else {
                            json!(series_color)
                        };
                    }
                    ChartType::Line | ChartType::Area => {
                        dataset["borderColor"] = json!(series_color);
                        dataset["backgroundColor"] = if fill {
                            // The area below the line, a little see-through.
                            json!(format!("{series_color}66"))
                        } else {
                            json!(series_color)
                        };
                        if any_given {
                            dataset["pointBackgroundColor"] = json!(point_colors);
                        }
                    }
                }
                dataset
            })
            .collect();

        let mut plugins = json!({
            "legend": { "display": self.legend },
            "sbValueLabels": { "display": self.labels, "unit": self.unit },
        });
        if !self.title.is_empty() {
            plugins["title"] = json!({ "display": true, "text": self.title });
        }
        if !self.subtitle.is_empty() {
            plugins["subtitle"] = json!({ "display": true, "text": self.subtitle });
        }
        let mut options = json!({
            "responsive": true,
            "maintainAspectRatio": false,
            "plugins": plugins,
        });
        if horizontal {
            options["indexAxis"] = json!("y");
        }
        if self.chart_type != ChartType::Pie {
            let mut axis = Map::new();
            if matches!(self.chart_type, ChartType::Line | ChartType::Area) {
                axis.insert("beginAtZero".to_owned(), json!(self.zero));
            }
            if !self.unit.is_empty() {
                axis.insert(
                    "title".to_owned(),
                    json!({ "display": true, "text": self.unit }),
                );
            }
            // The values run along the vertical axis, except in a bar chart.
            let value_axis = if horizontal { "x" } else { "y" };
            let mut scales = Map::new();
            scales.insert(value_axis.to_owned(), Value::Object(axis));
            options["scales"] = Value::Object(scales);
        }
        json!({
            "type": kind,
            "data": { "labels": labels, "datasets": datasets },
            "options": options,
        })
    }

    /// The chart as HTML that goes on any page: a `<canvas>` in a box, and the
    /// script that draws it, and, when `include_libs` is true, the `<script
    /// src>` that loads Chart.js before them. Python's `extract_chart` is this
    /// with the chart's own `libs` option.
    ///
    /// The title, the labels and the series' names are placed into the page
    /// as data, so any text is safe in them. A page with several charts needs
    /// Chart.js once, and each chart needs its own [`id`](Self::with_id).
    #[must_use]
    pub fn to_html(&self, include_libs: bool) -> String {
        let config = json_for_script(&self.chart_js_config());
        let id = &self.id;
        let mut html = String::new();
        if include_libs {
            html.push_str(CHART_JS_TAG);
            html.push('\n');
        }
        let _ = write!(
            html,
            "<div class=\"sb-chart\" style=\"position:relative;width:92%;max-width:960px;\
             height:70vh;margin:0 auto;\">\n<canvas id=\"{id}\"></canvas>\n</div>\n\
             <script>\n(function () {{\nconst config = {config};\n{CHART_SCRIPT}\n\
             new Chart(document.getElementById('{id}'), config);\n}})();\n</script>\n"
        );
        html
    }

    /// [`to_html`](Self::to_html) with the chart's own `libs` option.
    #[must_use]
    pub fn extract(&self) -> String {
        self.to_html(self.libs)
    }

    /// The chart as a page of its own, which always loads Chart.js.
    #[must_use]
    pub fn to_page(&self) -> String {
        let title = if self.title.is_empty() {
            "Chart".to_owned()
        } else {
            html_escape(&self.title)
        };
        format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
             <meta http-equiv=\"Content-Type\" content=\"text/html\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>{title}</title>\n{CHART_JS_TAG}\n</head>\n<body>\n{}</body>\n</html>\n",
            self.to_html(false)
        )
    }

    /// Writes the chart to `path` as a page that draws it with Chart.js.
    ///
    /// The page loads Chart.js from a CDN, so it draws nothing when the
    /// browser is offline.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be written.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        fs::write(path.as_ref(), self.to_page())?;
        Ok(())
    }

    /// Writes the chart to `filename` in `folder`, which is made if it is
    /// missing, and returns the path. The name of the folder is
    /// [`SAVED_FOLDER`] and the name of the file [`DEFAULT_FILENAME`], unless
    /// the caller gives others.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `filename` does not end
    /// in `.html` (Python's text, `Chart file must end in ".html"!`) or has a
    /// directory part, or if the folder or the file cannot be written.
    pub fn save_in(&self, folder: &Path, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        check_filename(filename)?;
        let path = folder.join(filename);
        fs::create_dir_all(folder).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to create the folder '{}' for the chart: {error}",
                folder.display()
            ))
        })?;
        fs::write(&path, self.to_page()).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write chart '{}': {error}",
                path.display()
            ))
        })?;
        Ok(path)
    }
}

/// Checks the name of a chart file: it must end in `.html` and be one plain
/// file name, so it cannot leave the folder it is saved to.
pub(crate) fn check_filename(filename: &str) -> Result<(), SeleniumBaseError> {
    if !filename.ends_with(".html") {
        return Err(SeleniumBaseError::InvalidConfig(
            "Chart file must end in \".html\"!".to_owned(),
        ));
    }
    bare_file_name(filename).map(|_| ())
}

/// The error for a chart name that nothing was created under.
pub(crate) fn missing_chart(chart_name: &str) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!("Chart {{{chart_name}}} does not exist!"))
}

/// `name`, or the default name if there is none.
pub(crate) fn name_or_default(name: Option<&str>) -> &str {
    name.filter(|name| !name.is_empty()).unwrap_or(DEFAULT_NAME)
}

/// A value as JSON: a whole number as an integer, so that `90` is not `90.0`,
/// and `null` for a value that is not a number.
fn number(value: f64) -> Value {
    // The cast is exact: the value is whole and below 2^53.
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        json!(value as i64)
    } else if value.is_finite() {
        json!(value)
    } else {
        Value::Null
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
pub(crate) fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Chart.js configuration a chart's HTML passes to `new Chart(...)`.
    fn embedded_config(html: &str) -> Value {
        let start = html.find("const config = ").expect("config line") + "const config = ".len();
        let end = html[start..].find(";\n").expect("end of the line") + start;
        serde_json::from_str(&html[start..end]).expect("the embedded configuration is valid JSON")
    }

    fn config_of(chart: &Chart) -> Value {
        embedded_config(&chart.to_page())
    }

    #[test]
    fn a_pie_chart_page_contains_the_data_with_a_colour_per_slice() {
        let mut chart = Chart::new("Votes", ChartType::Pie);
        chart.add_data_point("A", 10.0);
        chart.add_data_point("B", 20.0);

        let html = chart.to_page();
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
        assert!(
            config["options"].get("scales").is_none(),
            "a pie has no axes"
        );
    }

    #[test]
    fn values_are_floats_and_whole_ones_are_written_as_integers() {
        let mut chart = Chart::new("t", ChartType::Line);
        chart.add_data_point("a", 2.5);
        chart.add_data_point("b", 3.0);
        chart.add_data_point("c", f64::NAN);
        let json: Value = serde_json::from_str(&chart.to_json()).unwrap();
        assert_eq!(json["datasets"][0]["data"], json!([2.5, 3, null]));
    }

    #[test]
    fn a_bar_chart_is_horizontal_and_a_column_chart_is_vertical() {
        // Chart.js draws vertical bars by default and horizontal ones with
        // `indexAxis: 'y'`; Highcharts' `bar` is horizontal, its `column` vertical.
        let bar = config_of(&Chart::new("t", ChartType::Bar));
        assert_eq!(bar["type"], "bar");
        assert_eq!(bar["options"]["indexAxis"], "y");
        let column = config_of(&Chart::new("t", ChartType::Column));
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
            let config = config_of(&Chart::new("t", chart_type));
            assert_eq!(
                config["data"]["datasets"][0]["fill"], filled,
                "{chart_type:?}"
            );
        }
        assert_eq!(config_of(&Chart::new("t", ChartType::Area))["type"], "line");
    }

    #[test]
    fn the_options_reach_the_chart_js_configuration() {
        let chart = Chart::new("Sales", ChartType::Line)
            .with_subtitle("2026")
            .with_unit("USD")
            .with_zero(true)
            .with_legend(false)
            .with_labels(false);
        let options = &config_of(&chart)["options"];
        assert_eq!(
            options["plugins"]["title"],
            json!({"display": true, "text": "Sales"})
        );
        assert_eq!(
            options["plugins"]["subtitle"],
            json!({"display": true, "text": "2026"})
        );
        assert_eq!(options["plugins"]["legend"]["display"], false);
        assert_eq!(
            options["plugins"]["sbValueLabels"],
            json!({"display": false, "unit": "USD"})
        );
        assert_eq!(options["scales"]["y"]["beginAtZero"], true);
        assert_eq!(
            options["scales"]["y"]["title"],
            json!({"display": true, "text": "USD"})
        );
    }

    #[test]
    fn the_defaults_show_the_legend_and_the_values_and_start_at_nothing_special() {
        let options = &config_of(&Chart::new("", ChartType::Column))["options"];
        assert_eq!(options["plugins"]["legend"]["display"], true);
        assert_eq!(options["plugins"]["sbValueLabels"]["display"], true);
        assert!(options["plugins"].get("title").is_none());
        assert!(options["plugins"].get("subtitle").is_none());
        assert!(options["scales"]["y"].get("title").is_none());
        assert!(options["scales"]["y"].get("beginAtZero").is_none());
        let line = &config_of(&Chart::new("t", ChartType::Line))["options"];
        assert_eq!(line["scales"]["y"]["beginAtZero"], false);
    }

    #[test]
    fn a_bar_chart_puts_its_unit_on_the_horizontal_axis() {
        let options = &config_of(&Chart::new("t", ChartType::Bar).with_unit("ms"))["options"];
        assert_eq!(options["scales"]["x"]["title"]["text"], "ms");
        assert!(options["scales"].get("y").is_none());
    }

    #[test]
    fn the_libs_option_decides_whether_the_extracted_html_loads_chart_js() {
        let chart = Chart::new("t", ChartType::Pie);
        let with = chart.extract();
        assert!(with.starts_with(CHART_JS_TAG), "{with}");
        assert_eq!(with.matches("<script src").count(), 1);

        let without = Chart::new("t", ChartType::Pie).with_libs(false).extract();
        assert!(!without.contains("<script src"), "{without}");
        assert!(without.contains("new Chart("));
        // `to_html` takes the choice directly; a saved page always loads it.
        assert!(chart.to_html(false).find(CHART_JS_TAG).is_none());
        assert!(Chart::new("t", ChartType::Pie)
            .with_libs(false)
            .to_page()
            .contains(CHART_JS_TAG));
    }

    #[test]
    fn a_title_and_labels_cannot_break_out_of_the_page() {
        let mut chart = Chart::new("</title><script>alert(1)</script>", ChartType::Pie)
            .with_subtitle("</script><b>")
            .with_unit("<!-- u");
        chart.add_data_point("back\\slash", 1.0);
        chart.add_data_point("it's \"quoted\"\nline", 2.0);
        chart.add_data_point("</script><script>alert(2)</script>", 3.0);
        chart.add_data_point("<!-- x", 4.0);

        let html = chart.to_page();
        assert!(!html.contains("<script>alert"), "{html}");
        assert!(html.contains("<title>&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;</title>"));
        assert_eq!(
            html.matches("<script").count(),
            2,
            "only the CDN script and the chart script"
        );
        assert_eq!(html.matches("</script>").count(), 2);
        assert!(!html.contains("<!-- x"));
        assert!(!html.contains("<!-- u"));

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
        assert_eq!(
            config["options"]["plugins"]["sbValueLabels"]["unit"],
            "<!-- u"
        );
    }

    #[test]
    fn the_page_pins_the_chart_js_version() {
        let html = Chart::new("t", ChartType::Pie).to_page();
        assert!(html.contains(
            r#"<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.js">"#
        ));
    }

    fn two_series_chart() -> Chart {
        let mut chart = Chart::new("t", ChartType::Line).with_data_name("first");
        chart.add_data_point("a", 1.0);
        chart.add_data_point("b", 2.0);
        let mut series = ChartSeries::new("s");
        series.add_data_point("b", 20.0);
        series.add_data_point("c", 30.0); // a label the first series lacks
        chart.add_series(series);
        chart
    }

    #[test]
    fn series_are_matched_to_labels_by_name_and_gaps_are_not_zero() {
        let json: Value = serde_json::from_str(&two_series_chart().to_json()).unwrap();
        assert_eq!(json["labels"], json!(["a", "b", "c"]));
        assert_eq!(json["datasets"][0]["label"], "first");
        assert_eq!(json["datasets"][0]["data"], json!([1, 2, null]));
        assert_eq!(json["datasets"][1]["data"], json!([null, 20, 30]));
    }

    #[test]
    fn the_json_and_the_page_agree_on_every_value() {
        let chart = two_series_chart();
        let json: Value = serde_json::from_str(&chart.to_json()).unwrap();
        let config = config_of(&chart);
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
    fn a_started_series_takes_the_data_points_that_follow() {
        let mut chart = Chart::new("t", ChartType::Column);
        chart.add_data_point("Q1", 1.0);
        chart.start_series(None);
        chart.add_data_point("Q1", 5.0);
        chart.start_series(Some("Named"));
        chart.add_data_point("Q1", 9.0);

        let names: Vec<&str> = chart.series().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Series 1", "Series 2", "Named"]);
        let json: Value = serde_json::from_str(&chart.to_json()).unwrap();
        assert_eq!(
            json["labels"],
            json!(["Q1"]),
            "the labels come from the first series"
        );
        for (index, expected) in [1, 5, 9].into_iter().enumerate() {
            assert_eq!(json["datasets"][index]["data"], json!([expected]));
        }
    }

    #[test]
    fn the_first_series_is_named_by_data_name_unless_that_is_empty() {
        assert_eq!(Chart::new("t", ChartType::Pie).series()[0].name, "Series 1");
        let named = Chart::new("t", ChartType::Pie).with_data_name("Share");
        assert_eq!(named.series()[0].name, "Share");
        let empty = Chart::new("t", ChartType::Pie).with_data_name("  ");
        assert_eq!(empty.series()[0].name, "Series 1");
    }

    #[test]
    fn a_point_can_have_its_own_colour() {
        let mut pie = Chart::new("t", ChartType::Pie);
        pie.add_data_point("a", 1.0);
        pie.add_data_point_with_color("b", 2.0, "red");
        assert_eq!(
            config_of(&pie)["data"]["datasets"][0]["backgroundColor"],
            json!(["#3366cc", "red"])
        );

        let mut column = Chart::new("t", ChartType::Column);
        column.add_data_point("a", 1.0);
        assert_eq!(
            config_of(&column)["data"]["datasets"][0]["backgroundColor"],
            "#3366cc",
            "one colour for the series unless a point has its own"
        );
        column.add_data_point_with_color("b", 2.0, "red");
        assert_eq!(
            config_of(&column)["data"]["datasets"][0]["backgroundColor"],
            json!(["#3366cc", "red"])
        );

        let mut line = Chart::new("t", ChartType::Line);
        line.add_data_point_with_color("a", 1.0, "red");
        let dataset = &config_of(&line)["data"]["datasets"][0];
        assert_eq!(dataset["borderColor"], "#3366cc");
        assert_eq!(dataset["pointBackgroundColor"], json!(["red"]));
    }

    #[test]
    fn each_series_of_a_bar_chart_has_a_colour_of_its_own() {
        let config = config_of(&two_series_chart_of(ChartType::Column));
        assert_eq!(config["data"]["datasets"][0]["backgroundColor"], "#3366cc");
        assert_eq!(config["data"]["datasets"][1]["backgroundColor"], "#dc3912");
    }

    fn two_series_chart_of(chart_type: ChartType) -> Chart {
        let mut chart = Chart::new("t", chart_type);
        chart.add_data_point("a", 1.0);
        chart.start_series(None);
        chart.add_data_point("a", 2.0);
        chart
    }

    #[test]
    fn a_chart_with_no_data_is_still_a_valid_page() {
        let config = config_of(&Chart::new("empty", ChartType::Column));
        assert_eq!(config["data"]["labels"], json!([]));
        assert_eq!(config["data"]["datasets"][0]["data"], json!([]));
    }

    #[test]
    fn chart_ids_are_safe_and_different_names_give_different_ids() {
        let id = |name: &str| {
            Chart::new("t", ChartType::Pie)
                .with_id(name)
                .id()
                .to_owned()
        };
        assert_eq!(id("sales-2026"), "sales-2026");
        assert_eq!(id("a b"), "a_20b");
        assert_ne!(id("a b"), id("a_20b"), "the underscore is escaped too");
        assert_ne!(id("a'b"), id("a\"b"));
        assert_eq!(id(""), "sb-chart");
        let html = Chart::new("t", ChartType::Pie)
            .with_id("x'); alert(1); ('")
            .to_html(true);
        assert!(!html.contains("alert(1); ("), "{html}");
        for character in id("x'); <b> é").chars() {
            assert!(character.is_ascii_alphanumeric() || "-_".contains(character));
        }
    }

    #[test]
    fn two_charts_with_two_ids_can_share_a_page() {
        let first = Chart::new("a", ChartType::Pie).with_id("one").to_html(true);
        let second = Chart::new("b", ChartType::Bar)
            .with_id("two")
            .to_html(false);
        assert!(first.contains("<canvas id=\"one\">") && first.contains("getElementById('one')"));
        assert!(second.contains("<canvas id=\"two\">") && second.contains("getElementById('two')"));
    }

    #[test]
    fn a_chart_is_saved_under_the_name_given_in_the_folder_given() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("charts");
        let mut chart = Chart::new("Votes", ChartType::Pie);
        chart.add_data_point("A", 1.0);

        let path = chart.save_in(&folder, DEFAULT_FILENAME).unwrap();
        assert_eq!(path, folder.join("my_chart.html"));
        assert!(fs::read_to_string(&path).unwrap().contains("Votes"));

        let plain = dir.path().join("plain.html");
        chart.save(&plain).unwrap();
        assert!(fs::read_to_string(plain).unwrap().contains("new Chart("));
    }

    #[test]
    fn a_file_name_must_end_in_html_and_stay_in_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        let chart = Chart::new("t", ChartType::Pie);
        let error = chart
            .save_in(dir.path(), "chart.png")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Chart file must end in \".html\"!"),
            "{error}"
        );
        for bad in ["../chart.html", "a/chart.html", "/tmp/chart.html"] {
            assert!(chart.save_in(dir.path(), bad).is_err(), "{bad}");
        }
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn a_missing_chart_is_reported_with_python_s_text() {
        assert_eq!(
            missing_chart("sales").to_string(),
            "invalid configuration: Chart {sales} does not exist!"
        );
        assert_eq!(name_or_default(None), "default");
        assert_eq!(name_or_default(Some("")), "default");
        assert_eq!(name_or_default(Some("sales")), "sales");
    }

    #[test]
    fn the_palette_repeats_after_its_last_colour() {
        assert_eq!(default_color(0), default_color(10));
        assert_ne!(default_color(0), default_color(1));
    }
}
