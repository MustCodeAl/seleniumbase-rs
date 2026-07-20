// Download helpers.

impl BaseCase {
    /// Returns the path to the browser downloads folder.
    pub fn get_downloads_folder(&self) -> PathBuf {
        default_download_dir()
    }

    /// Returns the full path of `filename` inside the downloads folder.
    pub fn get_path_of_downloaded_file(&self, filename: &str) -> PathBuf {
        default_download_dir().join(filename)
    }

    /// Returns true if `filename` exists in the downloads folder.
    pub fn is_downloaded_file_present(&self, filename: &str) -> bool {
        self.get_path_of_downloaded_file(filename).exists()
    }

    /// Returns true if any downloaded file matches `regex`.
    pub fn is_downloaded_file_regex_present(&self, regex: &str) -> Result<bool, SeleniumBaseError> {
        let re = regex_or_err(regex)?;
        for entry in self.get_downloaded_files()? {
            if let Some(name) = entry.file_name().and_then(|n| n.to_str()) {
                if re.is_match(name) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Reads the contents of `filename` from the downloads folder.
    pub fn get_data_from_downloaded_file(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        let path = self.get_path_of_downloaded_file(filename);
        Ok(fs::read_to_string(&path)?)
    }

    /// Asserts that `filename` exists and matches `regex`.
    pub fn assert_downloaded_file_regex(
        &self,
        regex: &str,
    ) -> Result<(), SeleniumBaseError> {
        if !self.is_downloaded_file_regex_present(regex)? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "No downloaded file matched regex '{}'",
                regex
            )));
        }
        Ok(())
    }

    /// Asserts that `filename` contains `text`.
    pub fn assert_data_in_downloaded_file(
        &self,
        filename: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let data = self.get_data_from_downloaded_file(filename)?;
        if !data.contains(text) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Downloaded file '{}' did not contain '{}'",
                filename, text
            )));
        }
        Ok(())
    }

    /// Deletes `filename` from the downloads folder if it exists.
    pub fn delete_downloaded_file_if_present(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = self.get_path_of_downloaded_file(filename);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
