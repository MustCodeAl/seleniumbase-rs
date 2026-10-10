// Chart helpers.

/// The `file://` URL of `path`, which must exist.
fn file_url(path: &Path, what: &str) -> Result<String, SeleniumBaseError> {
    let absolute = std::fs::canonicalize(path)?;
    url::Url::from_file_path(&absolute)
        .map(Into::into)
        .map_err(|()| SeleniumBaseError::InvalidConfig(format!("invalid {what} path")))
}

/// Writes `chart` as a new HTML file in `dir`, and returns the file together
/// with its `file://` URL.
///
/// The file is left in place, so the path stays valid for as long as the
/// caller wants to look at it or reload the page.
fn write_chart_page(chart: &Chart, dir: &Path) -> Result<(PathBuf, String), SeleniumBaseError> {
    let path = artifact_path(dir, "chart", "html");
    chart.save(&path)?;
    let url = file_url(&path, "chart")?;
    Ok((path, url))
}

impl BaseCase {
    /// Adds a named data series to the current chart.
    pub async fn add_series_to_chart(
        &mut self,
        name: &str,
        data: &[(String, i32)],
    ) -> Result<(), SeleniumBaseError> {
        match self.chart.as_mut() {
            Some(chart) => {
                let mut series = ChartSeries::new(name);
                for (label, value) in data {
                    series.add_data_point(label, *value);
                }
                chart.add_series(series);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created. Call create_*_chart first.".to_owned(),
            )),
        }
    }

    /// Saves the current chart as an HTML file in the logs folder and opens it
    /// in the browser.
    ///
    /// Returns the path of the file, which stays in the logs folder.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if no chart was created, and
    /// an I/O error if the file cannot be written. The page loads Chart.js from
    /// a CDN, so it draws nothing when the browser is offline.
    pub async fn display_chart(&mut self) -> Result<PathBuf, SeleniumBaseError> {
        let chart = self.chart.as_ref().ok_or_else(|| {
            SeleniumBaseError::InvalidConfig("No chart created. Call create_*_chart first.".to_owned())
        })?;
        let (path, url) = write_chart_page(chart, &ensure_latest_logs_dir()?)?;
        self.open(&url).await?;
        Ok(path)
    }

    /// Returns the current chart data as a JSON string.
    pub async fn extract_chart(&self) -> Result<String, SeleniumBaseError> {
        match self.chart.as_ref() {
            Some(chart) => Ok(chart.to_json()),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created. Call create_*_chart first.".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests_charts {
    use super::*;

    #[test]
    fn a_displayed_chart_stays_on_disk_for_the_path_it_returns() {
        let logs = tempfile::tempdir().unwrap();
        let mut chart = Chart::new("Votes", ChartType::Pie);
        chart.add_data_point("A", 1);

        let (path, url) = write_chart_page(&chart, logs.path()).unwrap();
        assert!(path.starts_with(logs.path()));
        assert!(path.is_file(), "the returned path must exist: {}", path.display());
        assert!(std::fs::read_to_string(&path).unwrap().contains("Votes"));
        assert!(url.starts_with("file://") && url.ends_with(".html"), "{url}");

        // A second chart gets its own file instead of overwriting the first.
        let (second, _) = write_chart_page(&chart, logs.path()).unwrap();
        assert_ne!(path, second);
        assert!(path.is_file() && second.is_file());
    }

    #[tokio::test]
    async fn the_chart_helpers_say_so_when_there_is_no_chart() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        for error in [
            sb.add_series_to_chart("s", &[]).await.unwrap_err(),
            sb.display_chart().await.unwrap_err(),
            sb.extract_chart().await.unwrap_err(),
        ] {
            assert!(matches!(error, SeleniumBaseError::InvalidConfig(m) if m.contains("No chart created")));
        }
    }

    #[tokio::test]
    async fn a_series_joins_the_chart_that_was_created() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        sb.chart = Some(Chart::new("t", ChartType::Line));
        sb.add_series_to_chart("s", &[("a".to_owned(), 1)]).await.unwrap();
        let json: serde_json::Value = serde_json::from_str(&sb.extract_chart().await.unwrap()).unwrap();
        assert_eq!(json["datasets"][1]["label"], "s");
    }
}
