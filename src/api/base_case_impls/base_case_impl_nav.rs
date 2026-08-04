// Navigation helpers.

impl BaseCase {
    /// Opens the configured start page if one exists, otherwise `about:blank`.
    pub async fn goto_start_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.open_start_page().await
    }

    /// Opens the configured start page if one exists, otherwise `about:blank`.
    pub async fn open_start_page(&mut self) -> Result<(), SeleniumBaseError> {
        let url = self.config.start_page.clone().unwrap_or_else(|| "about:blank".to_owned());
        self.open(&url).await
    }
}
