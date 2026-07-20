// Screenshot, PDF, and file helpers.

impl BaseCase {
    /// Saves a full-page screenshot to the logs directory.
    pub async fn save_screenshot(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        self.session.screenshot(&path).await?;
        Ok(path)
    }

    /// Saves the current page as a PDF into the logs directory.
    pub async fn save_as_pdf_to_logs(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        self.print_to_pdf(path.to_str().unwrap_or("page.pdf")).await?;
        Ok(path)
    }

    /// Saves a screenshot of `css` to the logs directory.
    pub async fn save_element_as_image_file(
        &mut self,
        css: &str,
        filename: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        let by = Selector::Css(css).to_by()?;
        let element = self.session.driver().find(by).await?;
        element.screenshot(&path).await.map_err(SeleniumBaseError::WebDriver)?;
        Ok(path)
    }

    /// Saves `data` to `filename` in the logs directory.
    pub fn save_file_as(&self, data: &[u8], filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        fs::write(&path, data)?;
        Ok(path)
    }

    /// Reads the contents of a file from the logs directory.
    pub fn get_file_data(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        Ok(fs::read_to_string(&path)?)
    }

    /// Creates a folder in the logs directory.
    pub fn create_folder(&self, name: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(name);
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}
