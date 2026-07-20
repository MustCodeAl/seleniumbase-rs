// Window / screen helpers.

impl BaseCase {
    /// Maximizes the browser window.
    pub async fn maximize(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().maximize_window().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Minimizes the browser window.
    pub async fn minimize(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().minimize_window().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns the browser window rectangle as `(x, y, width, height)`.
    pub async fn get_window_rect(&self) -> Result<(i64, i64, i64, i64), SeleniumBaseError> {
        let rect = self
            .session
            .driver()
            .get_window_rect()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok((rect.x, rect.y, rect.width, rect.height))
    }

    /// Returns the screen rectangle as `(width, height)`.
    pub async fn get_screen_rect(&self) -> Result<(i64, i64), SeleniumBaseError> {
        let w = self.get_screen_width().await?;
        let h = self.get_screen_height().await?;
        Ok((w, h))
    }

    /// Brings the active browser window to the front.
    pub async fn bring_active_window_to_front(&self) -> Result<(), SeleniumBaseError> {
        self.bring_to_front().await
    }
}
