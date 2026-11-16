// Window / screen helpers.

impl BaseCase {
    /// Maximizes the browser window.
    pub async fn maximize(&self) -> Result<(), SeleniumBaseError> {
            self.session.maximize_window().await?;
            self.demo_pause(PauseKind::Short).await;
            Ok(())
        }

    /// Minimizes the browser window.
    pub async fn minimize(&self) -> Result<(), SeleniumBaseError> {
            self.session.minimize_window().await?;
            self.demo_pause(PauseKind::Short).await;
            Ok(())
        }

    /// Returns the browser window rectangle as `(x, y, width, height)`.
    pub async fn window_rect(&self) -> Result<(i64, i64, i64, i64), SeleniumBaseError> {
            self.session.window_rect().await
        }
    

    

    

    /// Returns the screen rectangle as `(width, height)`.
    pub async fn screen_rect(&self) -> Result<(i64, i64), SeleniumBaseError> {
        let w = self.screen_width().await?;
        let h = self.screen_height().await?;
        Ok((w, h))
    }

    /// Brings the active browser window to the front. To raise one element
    /// above the others, see [`bring_to_front`](Self::bring_to_front).
    pub async fn bring_active_window_to_front(&self) -> Result<(), SeleniumBaseError> {
        self.focus_window().await
    }
}
