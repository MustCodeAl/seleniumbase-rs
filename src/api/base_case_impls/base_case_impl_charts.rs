// Chart helpers.

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

    /// Saves the current chart to a temporary HTML file and opens it.
    pub async fn display_chart(&mut self) -> Result<PathBuf, SeleniumBaseError> {
        let chart = self.chart.as_ref().ok_or_else(|| {
            SeleniumBaseError::InvalidConfig("No chart created. Call create_*_chart first.".to_owned())
        })?;
        let dir = tempfile::tempdir().map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?;
        let path = dir.path().join("chart.html");
        chart.save(&path)?;
        let url = url::Url::from_file_path(&path)
            .map_err(|_| SeleniumBaseError::InvalidConfig("invalid chart path".to_owned()))?;
        self.open(url.as_str()).await?;
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
