// Alert helpers.

impl BaseCase {
    /// Switches focus to the active alert/confirm/prompt.
    ///
    /// # Errors
    ///
    /// Returns a WebDriver error if no alert is open.
    pub async fn switch_to_alert(&self) -> Result<(), SeleniumBaseError> {
        self.session.webdriver()?.get_alert_text().await.map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Waits up to `timeout_secs` for an alert, then switches to it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no alert appears in time.
    pub async fn wait_for_and_switch_to_alert(
        &self,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            "No alert appeared".to_owned(),
            || self.is_alert_present(),
        )
        .await?;
        self.switch_to_alert().await
    }
}
