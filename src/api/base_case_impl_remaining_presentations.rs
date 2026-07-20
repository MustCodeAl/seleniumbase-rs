// Presentation helpers.

impl BaseCase {
    /// Creates a presentation, saves it to a temp file, and opens it in the browser.
    pub async fn begin_presentation(&mut self, title: &str) -> Result<PathBuf, SeleniumBaseError> {
        self.create_presentation(title).await?;
        let dir = tempfile::tempdir().map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?;
        let path = dir.path().join("presentation.html");
        self.save_presentation(path.to_str().unwrap_or("presentation.html")).await?;
        let url = url::Url::from_file_path(&path)
            .map_err(|_| SeleniumBaseError::InvalidConfig("invalid presentation path".to_owned()))?;
        self.open(url.as_str()).await?;
        Ok(path)
    }

    /// Adds a slide to the current presentation.
    pub async fn add_slide(&mut self, content: &str) -> Result<(), SeleniumBaseError> {
        self.add_presentation_slide(content).await
    }
}
