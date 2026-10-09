// Screenshot, PDF, and file helpers.

impl BaseCase {
    /// Saves a full-page screenshot to the logs directory.
    ///
    /// `filename` must be a plain file name; one with a directory part or a
    /// `..` is refused, so the file cannot land outside the logs directory.
    /// A file of the same name is replaced.
    pub async fn save_screenshot(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        self.session.screenshot(&path).await?;
        Ok(path)
    }

    /// Saves the current page as a PDF into the logs directory.
    ///
    /// `filename` must be a plain file name, as for [`Self::save_screenshot`].
    pub async fn save_as_pdf_to_logs(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        self.print_to_pdf(path.to_str().unwrap_or("page.pdf")).await?;
        Ok(path)
    }

    /// Saves a screenshot of `css` to the logs directory.
    ///
    /// `filename` must be a plain file name, as for [`Self::save_screenshot`].
    pub async fn save_element_as_image_file(
        &mut self,
        css: &str,
        filename: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        let by = Selector::auto(css).to_by()?;
        let element = self.session.driver().find(by).await?;
        element.screenshot(&path).await.map_err(SeleniumBaseError::WebDriver)?;
        Ok(path)
    }

    /// Saves `data` to `filename` in the logs directory.
    ///
    /// `filename` must be a plain file name, as for [`Self::save_screenshot`].
    pub fn save_file_as(&self, data: &[u8], filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        fs::write(&path, data)?;
        Ok(path)
    }

    /// Reads the contents of a file from the logs directory.
    ///
    /// `filename` must be a plain file name, as for [`Self::save_screenshot`].
    pub fn get_file_data(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        Ok(fs::read_to_string(&path)?)
    }

    /// Creates a folder in the logs directory.
    ///
    /// `name` must be a plain name, so the folder is created directly inside
    /// the logs directory and nowhere else.
    pub fn create_folder(&self, name: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, name)?;
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}
