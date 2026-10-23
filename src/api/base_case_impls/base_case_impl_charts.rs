// Chart helpers.
use crate::api::chart::{self as chart_lib, Chart, ChartType};

/// How long `display_chart` shows a chart for: `requested`, or the run's
/// `--interval`, and one second where nobody watches (Python: headless,
/// headless2 or xvfb). Zero means that it waits until the window is closed.
fn display_interval(
    requested: Duration,
    run_interval: Option<Duration>,
    nobody_watches: bool,
) -> Duration {
    if nobody_watches {
        Duration::from_secs(1)
    } else {
        crate::api::presentation::resolve_interval(requested, run_interval)
    }
}

impl BaseCase {
    /// Starts a pie chart, as Python's `create_pie_chart(chart_name, title,
    /// subtitle, data_name, unit, libs, labels, legend)`. It returns a call
    /// builder: awaiting it runs the call with the keyword arguments that were
    /// set.
    ///
    /// `.chart_name(..)` names the chart, so that several can exist at the same
    /// time (the default name is `default`); `.title(..)`, `.subtitle(..)`,
    /// `.data_name(..)` (the name of the first series) and `.unit(..)` (the
    /// title of the value axis) describe it; `.libs(false)` leaves the
    /// `<script src>` of Chart.js out of [`extract_chart`](Self::extract_chart);
    /// `.labels(false)` stops the values being drawn on the chart and
    /// `.legend(false)` hides the legend. Creating a chart under a name that is
    /// in use starts it again.
    ///
    /// Python draws its charts with Highcharts; these are drawn with Chart.js,
    /// which is free to use in a commercial product.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.create_pie_chart().title("Browsers").unit("%").await?;
    /// sb.add_data_point("Chrome", 60.0).await?;
    /// sb.add_data_point("Firefox", 25.0).color("orange").await?;
    /// let path = sb.save_chart().filename("browsers.html").await?;
    /// println!("saved to {}", path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_pie_chart(&mut self) -> CreateChart<'_> {
        CreateChart::new(self, ChartType::Pie)
    }

    /// Starts a bar chart, whose bars are horizontal. It takes the keyword
    /// arguments of [`create_pie_chart`](Self::create_pie_chart).
    pub fn create_bar_chart(&mut self) -> CreateChart<'_> {
        CreateChart::new(self, ChartType::Bar)
    }

    /// Starts a column chart, whose columns are vertical. It takes the keyword
    /// arguments of [`create_pie_chart`](Self::create_pie_chart).
    pub fn create_column_chart(&mut self) -> CreateChart<'_> {
        CreateChart::new(self, ChartType::Column)
    }

    /// Starts a line chart. It takes the keyword arguments of
    /// [`create_pie_chart`](Self::create_pie_chart) and `.zero(true)`, which
    /// makes the value axis begin at zero.
    pub fn create_line_chart(&mut self) -> CreateChart<'_> {
        CreateChart::new(self, ChartType::Line)
    }

    /// Starts an area chart, which is a line chart with the area below the line
    /// filled. It takes the keyword arguments of
    /// [`create_line_chart`](Self::create_line_chart).
    pub fn create_area_chart(&mut self) -> CreateChart<'_> {
        CreateChart::new(self, ChartType::Area)
    }

    /// Starts a new series in a chart, as Python's `add_series_to_chart(
    /// data_name, chart_name)`. The data points added after it go to the new
    /// series; the labels stay those of the first one.
    ///
    /// `.data_name(..)` names the series (the default is `Series N`) and
    /// `.chart_name(..)` picks the chart.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Chart {NAME} does not exist!`, if there is no chart of that name.
    pub fn add_series_to_chart(&mut self) -> AddSeries<'_> {
        AddSeries {
            sb: self,
            data_name: None,
            chart_name: None,
        }
    }

    /// Adds a data point to a chart, as Python's `add_data_point(label, value,
    /// color, chart_name)`: to the series that was started last. `.color(..)`
    /// is a CSS colour for the point and `.chart_name(..)` picks the chart.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Chart {NAME} does not exist!`, if there is no chart of that name, and
    /// `Expecting a numeric value for "value"!` if `value` is not a number
    /// (NaN or infinite).
    pub fn add_data_point<'a>(&'a mut self, label: &'a str, value: f64) -> AddDataPoint<'a> {
        AddDataPoint {
            sb: self,
            label,
            value,
            color: None,
            chart_name: None,
        }
    }

    /// Saves a chart as an HTML page, as Python's `save_chart(chart_name,
    /// filename, folder)`, and returns the path of the file.
    ///
    /// The page goes to `saved_charts/my_chart.html` unless `.filename(..)`
    /// names another file, which must end in `.html`, or `.folder(..)` another
    /// folder. The page loads Chart.js from a CDN, so it draws nothing when the
    /// browser is offline.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's texts,
    /// `Chart {NAME} does not exist!` and `Chart file must end in ".html"!`,
    /// and an error if the file cannot be written.
    pub fn save_chart(&self) -> SaveChart<'_> {
        SaveChart {
            sb: self,
            chart_name: None,
            filename: None,
            folder: None,
        }
    }

    /// Shows a chart in the browser, as Python's `display_chart(chart_name,
    /// filename, interval)`: it saves the chart, opens the file, and waits.
    ///
    /// With `.interval(..)` it waits that long; without one, it waits for the
    /// run's `--interval`, and, if there is none, until the window is closed
    /// (it prints `*** Close the browser window to continue ***`). Where
    /// nobody watches (a headless browser or a virtual display) it waits one
    /// second. It returns the path of the file.
    ///
    /// # Errors
    ///
    /// The same as [`save_chart`](Self::save_chart), and the error of opening
    /// the file.
    pub fn display_chart(&mut self) -> DisplayChart<'_> {
        DisplayChart {
            sb: self,
            chart_name: None,
            filename: None,
            interval: Duration::ZERO,
        }
    }

    /// Returns the HTML of a chart, as Python's `extract_chart(chart_name)`, to
    /// put on a page of your own or on a slide: `.chart_name(..)` picks the
    /// chart. It starts with the `<script src>` of Chart.js unless the chart
    /// was created with `.libs(false)`; put the library on the page once, and
    /// charts on one page each need their own name.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Chart {NAME} does not exist!`, if there is no chart of that name.
    pub fn extract_chart(&self) -> ExtractChart<'_> {
        ExtractChart {
            sb: self,
            chart_name: None,
        }
    }

    /// The chart called `chart_name` (the default name if there is none).
    fn chart_named(&self, chart_name: Option<&str>) -> Result<&Chart, SeleniumBaseError> {
        let name = chart_lib::name_or_default(chart_name);
        self.charts
            .get(name)
            .ok_or_else(|| chart_lib::missing_chart(name))
    }
}

/// The call builder of [`BaseCase::create_pie_chart`] and the other
/// `create_*_chart` methods.
#[must_use = "nothing happens until the call is awaited"]
pub struct CreateChart<'a> {
    sb: &'a mut BaseCase,
    chart_type: ChartType,
    chart_name: Option<&'a str>,
    title: &'a str,
    subtitle: &'a str,
    data_name: &'a str,
    unit: &'a str,
    zero: bool,
    libs: bool,
    labels: bool,
    legend: bool,
}

impl<'a> CreateChart<'a> {
    fn new(sb: &'a mut BaseCase, chart_type: ChartType) -> Self {
        Self {
            sb,
            chart_type,
            chart_name: None,
            title: "",
            subtitle: "",
            data_name: "",
            unit: "",
            zero: false,
            libs: true,
            labels: true,
            legend: true,
        }
    }

    /// Names the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }

    /// Sets the title of the chart (Python's `title`).
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Sets the subtitle of the chart (Python's `subtitle`).
    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = subtitle;
        self
    }

    /// Names the first series (Python's `data_name`).
    pub fn data_name(mut self, data_name: &'a str) -> Self {
        self.data_name = data_name;
        self
    }

    /// Sets the unit of the values: the title of the value axis, and a suffix
    /// in the tooltip (Python's `unit`).
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = unit;
        self
    }

    /// Makes the value axis of a line or an area chart begin at zero (Python's
    /// `zero`). Other charts ignore it.
    pub fn zero(mut self, zero: bool) -> Self {
        self.zero = zero;
        self
    }

    /// Chooses whether the extracted HTML starts with the element that loads
    /// Chart.js (Python's `libs`; the default is that it does).
    pub fn libs(mut self, libs: bool) -> Self {
        self.libs = libs;
        self
    }

    /// Chooses whether the value of each point is drawn on the chart (Python's
    /// `labels`; the default is that it is).
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    /// Chooses whether the legend is shown (Python's `legend`; the default is
    /// that it is).
    pub fn legend(mut self, legend: bool) -> Self {
        self.legend = legend;
        self
    }
}

impl<'a> std::future::IntoFuture for CreateChart<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = chart_lib::name_or_default(self.chart_name).to_owned();
            let chart = Chart::new(self.title, self.chart_type)
                .with_subtitle(self.subtitle)
                .with_data_name(self.data_name)
                .with_unit(self.unit)
                .with_zero(self.zero)
                .with_libs(self.libs)
                .with_labels(self.labels)
                .with_legend(self.legend)
                // One canvas id per chart name, so that charts that share a
                // slide deck or a page do not draw on each other.
                .with_id(&format!("sb-chart-{name}"));
            self.sb.charts.insert(name, chart);
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::add_series_to_chart`].
#[must_use = "nothing happens until the call is awaited"]
pub struct AddSeries<'a> {
    sb: &'a mut BaseCase,
    data_name: Option<&'a str>,
    chart_name: Option<&'a str>,
}

impl<'a> AddSeries<'a> {
    /// Names the new series (Python's `data_name`; the default is `Series N`).
    pub fn data_name(mut self, data_name: &'a str) -> Self {
        self.data_name = Some(data_name);
        self
    }

    /// Picks the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }
}

impl<'a> std::future::IntoFuture for AddSeries<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = chart_lib::name_or_default(self.chart_name);
            let chart = self
                .sb
                .charts
                .get_mut(name)
                .ok_or_else(|| chart_lib::missing_chart(name))?;
            chart.start_series(self.data_name);
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::add_data_point`].
#[must_use = "nothing happens until the call is awaited"]
pub struct AddDataPoint<'a> {
    sb: &'a mut BaseCase,
    label: &'a str,
    value: f64,
    color: Option<&'a str>,
    chart_name: Option<&'a str>,
}

impl<'a> AddDataPoint<'a> {
    /// Draws the point in the CSS colour `color` (Python's `color`).
    pub fn color(mut self, color: &'a str) -> Self {
        self.color = Some(color);
        self
    }

    /// Picks the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }
}

impl<'a> std::future::IntoFuture for AddDataPoint<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = chart_lib::name_or_default(self.chart_name);
            let chart = self
                .sb
                .charts
                .get_mut(name)
                .ok_or_else(|| chart_lib::missing_chart(name))?;
            if !self.value.is_finite() {
                return Err(SeleniumBaseError::InvalidConfig(
                    "Expecting a numeric value for \"value\"!".to_owned(),
                ));
            }
            match self.color.filter(|color| !color.is_empty()) {
                Some(color) => chart.add_data_point_with_color(self.label, self.value, color),
                None => chart.add_data_point(self.label, self.value),
            }
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::save_chart`].
#[must_use = "nothing happens until the call is awaited"]
pub struct SaveChart<'a> {
    sb: &'a BaseCase,
    chart_name: Option<&'a str>,
    filename: Option<&'a str>,
    folder: Option<&'a Path>,
}

impl<'a> SaveChart<'a> {
    /// Picks the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }

    /// Names the file, which must end in `.html` (Python's `filename`; the
    /// default is `my_chart.html`).
    pub fn filename(mut self, filename: &'a str) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Saves into this folder instead of `saved_charts` (Python's `folder`).
    /// The folder is made if it is missing.
    pub fn folder<P: AsRef<Path> + ?Sized>(mut self, folder: &'a P) -> Self {
        self.folder = Some(folder.as_ref());
        self
    }
}

impl<'a> std::future::IntoFuture for SaveChart<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let chart = self.sb.chart_named(self.chart_name)?;
            let folder = self.folder.unwrap_or_else(|| Path::new(chart_lib::SAVED_FOLDER));
            let path = chart.save_in(folder, self.filename.unwrap_or(chart_lib::DEFAULT_FILENAME))?;
            tracing::info!(path = %path.display(), "chart saved");
            Ok(path)
        })
    }
}

/// The call builder of [`BaseCase::display_chart`].
#[must_use = "nothing happens until the call is awaited"]
pub struct DisplayChart<'a> {
    sb: &'a mut BaseCase,
    chart_name: Option<&'a str>,
    filename: Option<&'a str>,
    interval: Duration,
}

impl<'a> DisplayChart<'a> {
    /// Picks the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }

    /// Names the file, which must end in `.html` (Python's `filename`; the
    /// default is `my_chart.html`).
    pub fn filename(mut self, filename: &'a str) -> Self {
        self.filename = Some(filename);
        self
    }

    /// How long to show the chart for (Python's `interval`). Zero, the
    /// default, uses the run's `--interval`, and if there is none waits until
    /// the window is closed.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}

impl<'a> std::future::IntoFuture for DisplayChart<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let sb = self.sb;
            let interval = display_interval(
                self.interval,
                sb.run_settings.interval(),
                sb.nobody_watches(),
            );
            let chart = sb.chart_named(self.chart_name)?;
            let path = chart.save_in(
                Path::new(chart_lib::SAVED_FOLDER),
                self.filename.unwrap_or(chart_lib::DEFAULT_FILENAME),
            )?;
            let file = path.to_str().ok_or_else(|| {
                SeleniumBaseError::InvalidConfig("the chart path is not UTF-8".to_owned())
            })?;
            sb.open_html_file(file).await?;
            if interval.is_zero() {
                println!("\n*** Close the browser window to continue ***");
                // Also ends when the page is navigated away from the chart.
                sb.wait_while_shown(chart_lib::SAVED_FOLDER, None).await;
            } else {
                tokio::time::sleep(interval).await;
            }
            Ok(path)
        })
    }
}

/// The call builder of [`BaseCase::extract_chart`].
#[must_use = "nothing happens until the call is awaited"]
pub struct ExtractChart<'a> {
    sb: &'a BaseCase,
    chart_name: Option<&'a str>,
}

impl<'a> ExtractChart<'a> {
    /// Picks the chart (Python's `chart_name`; the default is `default`).
    pub fn chart_name(mut self, chart_name: &'a str) -> Self {
        self.chart_name = Some(chart_name);
        self
    }
}

impl<'a> std::future::IntoFuture for ExtractChart<'a> {
    type Output = Result<String, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, String>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { Ok(self.sb.chart_named(self.chart_name)?.extract()) })
    }
}

#[cfg(test)]
mod tests_charts {
    use super::*;
    use std::future::IntoFuture;

    fn case() -> BaseCase {
        BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected())
    }

    fn assert_send<T: Send>(_: &T) {}

    #[tokio::test]
    async fn two_named_charts_exist_side_by_side_and_fill_separately() {
        let mut sb = case();
        sb.create_pie_chart().title("Default").await.unwrap();
        sb.create_bar_chart().chart_name("bars").title("Bars").await.unwrap();
        sb.add_data_point("a", 1.0).await.unwrap();
        sb.add_data_point("b", 2.0).chart_name("bars").await.unwrap();
        sb.add_data_point("c", 3.0).chart_name("bars").await.unwrap();

        assert_eq!(sb.charts["default"].series()[0].data.len(), 1);
        assert_eq!(sb.charts["default"].chart_type(), ChartType::Pie);
        assert_eq!(sb.charts["bars"].series()[0].data.len(), 2);
        assert_eq!(sb.charts["bars"].chart_type(), ChartType::Bar);
        assert_eq!(sb.charts["bars"].title(), "Bars");
        assert_ne!(sb.charts["default"].id(), sb.charts["bars"].id());
    }

    #[tokio::test]
    async fn every_chart_type_has_its_create_method() {
        let mut sb = case();
        sb.create_pie_chart().chart_name("pie").await.unwrap();
        sb.create_bar_chart().chart_name("bar").await.unwrap();
        sb.create_column_chart().chart_name("column").await.unwrap();
        sb.create_line_chart().chart_name("line").zero(true).await.unwrap();
        sb.create_area_chart().chart_name("area").await.unwrap();
        for (name, expected) in [
            ("pie", ChartType::Pie),
            ("bar", ChartType::Bar),
            ("column", ChartType::Column),
            ("line", ChartType::Line),
            ("area", ChartType::Area),
        ] {
            assert_eq!(sb.charts[name].chart_type(), expected, "{name}");
        }
        let html = sb.extract_chart().chart_name("line").await.unwrap();
        assert!(html.contains("\"beginAtZero\":true"), "{html}");
    }

    #[tokio::test]
    async fn the_keyword_arguments_of_a_chart_reach_the_html() {
        let mut sb = case();
        sb.create_column_chart()
            .title("Sales")
            .subtitle("2026")
            .data_name("North")
            .unit("USD")
            .legend(false)
            .labels(false)
            .await
            .unwrap();
        sb.add_data_point("Q1", 10.0).await.unwrap();
        let html = sb.extract_chart().await.unwrap();
        for part in [
            "\"text\":\"Sales\"",
            "\"text\":\"2026\"",
            "\"label\":\"North\"",
            "\"text\":\"USD\"",
            "\"legend\":{\"display\":false}",
            "\"sbValueLabels\":{\"display\":false,\"unit\":\"USD\"}",
        ] {
            assert!(html.contains(part), "{part} missing in {html}");
        }
    }

    #[tokio::test]
    async fn extract_chart_returns_the_html_and_libs_decides_about_the_library() {
        let mut sb = case();
        sb.create_pie_chart().await.unwrap();
        sb.create_pie_chart().chart_name("bare").libs(false).await.unwrap();
        let with = sb.extract_chart().await.unwrap();
        assert!(with.starts_with("<script src="), "{with}");
        assert!(with.contains("<canvas id=\"sb-chart-default\">"));
        let without = sb.extract_chart().chart_name("bare").await.unwrap();
        assert!(!without.contains("<script src"), "{without}");
        assert!(without.contains("<canvas id=\"sb-chart-bare\">"));
    }

    #[tokio::test]
    async fn a_series_started_later_takes_the_points_that_follow() {
        let mut sb = case();
        sb.create_line_chart().await.unwrap();
        sb.add_data_point("Q1", 1.0).await.unwrap();
        sb.add_series_to_chart().data_name("Second").await.unwrap();
        sb.add_data_point("Q1", 5.0).await.unwrap();
        sb.add_series_to_chart().await.unwrap();

        let names: Vec<&str> = sb.charts["default"]
            .series()
            .iter()
            .map(|series| series.name.as_str())
            .collect();
        assert_eq!(names, ["Series 1", "Second", "Series 3"]);
        assert_eq!(sb.charts["default"].series()[1].data[0].value, 5.0);
    }

    #[tokio::test]
    async fn a_data_point_can_have_a_colour_and_must_be_a_number() {
        let mut sb = case();
        sb.create_pie_chart().await.unwrap();
        sb.add_data_point("a", 1.0).color("red").await.unwrap();
        sb.add_data_point("b", 2.0).color("").await.unwrap();
        let data = &sb.charts["default"].series()[0].data;
        assert_eq!(data[0].color.as_deref(), Some("red"));
        assert_eq!(data[1].color, None, "an empty colour is no colour");

        for bad in [f64::NAN, f64::INFINITY] {
            let error = sb.add_data_point("x", bad).await.unwrap_err();
            assert!(
                error.to_string().contains("Expecting a numeric value for \"value\"!"),
                "{error}"
            );
        }
        assert_eq!(sb.charts["default"].series()[0].data.len(), 2);
    }

    #[tokio::test]
    async fn a_chart_that_does_not_exist_is_reported_with_python_s_text() {
        let mut sb = case();
        let expected = "Chart {nope} does not exist!";
        let errors = [
            sb.add_series_to_chart().chart_name("nope").await.unwrap_err(),
            sb.add_data_point("a", 1.0).chart_name("nope").await.unwrap_err(),
        ];
        for error in errors {
            assert!(error.to_string().contains(expected), "{error}");
        }
        for error in [
            sb.save_chart().chart_name("nope").await.unwrap_err(),
            sb.extract_chart().chart_name("nope").await.unwrap_err(),
            sb.display_chart().chart_name("nope").await.unwrap_err(),
        ] {
            assert!(error.to_string().contains(expected), "{error}");
        }
        // The default name is checked too.
        let error = sb.add_data_point("a", 1.0).await.unwrap_err();
        assert!(error.to_string().contains("Chart {default} does not exist!"));
    }

    #[tokio::test]
    async fn save_chart_writes_the_page_to_the_folder_and_name_given() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("out");
        let mut sb = case();
        sb.create_pie_chart().title("Votes").await.unwrap();
        sb.add_data_point("A", 1.0).await.unwrap();

        let path = sb
            .save_chart()
            .folder(&folder)
            .filename("votes.html")
            .await
            .unwrap();
        assert_eq!(path, folder.join("votes.html"));
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("<title>Votes</title>") && html.contains("chart.js"));

        let default = sb.save_chart().folder(dir.path()).await.unwrap();
        assert_eq!(default, dir.path().join("my_chart.html"));
    }

    #[tokio::test]
    async fn a_chart_file_must_end_in_html() {
        let dir = tempfile::tempdir().unwrap();
        let mut sb = case();
        sb.create_pie_chart().await.unwrap();
        let error = sb
            .save_chart()
            .folder(dir.path())
            .filename("chart.png")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Chart file must end in \".html\"!"));
    }

    #[test]
    fn a_chart_is_shown_for_the_interval_given_else_the_run_s_else_until_closed() {
        let two = Duration::from_secs(2);
        let nine = Duration::from_secs(9);
        assert_eq!(display_interval(two, Some(nine), false), two);
        assert_eq!(display_interval(Duration::ZERO, Some(nine), false), nine);
        assert_eq!(display_interval(Duration::ZERO, None, false), Duration::ZERO);
        // Where nobody watches it is one second, whatever else was asked for.
        assert_eq!(display_interval(two, Some(nine), true), Duration::from_secs(1));
        assert_eq!(display_interval(Duration::ZERO, None, true), Duration::from_secs(1));
    }

    #[test]
    fn the_futures_of_the_chart_builders_are_send() {
        let mut sb = case();
        assert_send(&sb.create_pie_chart().into_future());
        assert_send(&sb.add_series_to_chart().into_future());
        assert_send(&sb.add_data_point("a", 1.0).into_future());
        assert_send(&sb.display_chart().into_future());
        assert_send(&sb.save_chart().into_future());
        assert_send(&sb.extract_chart().into_future());
    }
}
