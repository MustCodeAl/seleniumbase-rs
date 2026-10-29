// Screenshot, PDF, and file helpers.

use crate::api::misc_calls::SaveFileAs;

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

    /// Saves the current page as a PDF into the log folder of the running test.
    /// The folder is the one [`save_screenshot_to_logs`](Self::save_screenshot_to_logs)
    /// writes to. A file of the same name is replaced.
    ///
    /// `filename` must be a plain file name, as for [`Self::save_screenshot`].
    pub async fn save_as_pdf_to_logs(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = self.test_logs_dir()?;
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
        let element = self.session.webdriver()?.find(by).await?;
        element.screenshot(&path).await.map_err(SeleniumBaseError::WebDriver)?;
        Ok(path)
    }

    /// Fetches `file_url` and saves it as `new_file_name`, as Python's
    /// `save_file_as(file_url, new_file_name, destination_folder=None)` does:
    /// [`download_file`](Self::download_file) under a name of your choosing.
    /// Returns the path of the file. The keyword is
    /// [`SaveFileAs::destination_folder`].
    ///
    /// `new_file_name` must be a plain file name, as for [`Self::save_screenshot`].
    pub fn save_file_as<'a>(
        &'a self,
        file_url: &'a str,
        new_file_name: &'a str,
    ) -> SaveFileAs<'a> {
        SaveFileAs::new(self, file_url, new_file_name)
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
