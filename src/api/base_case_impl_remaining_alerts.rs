// Alert helpers.

impl BaseCase {
    /// Switches focus to the active alert/confirm/prompt.
    pub async fn switch_to_alert(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().get_alert_text().await.map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Waits up to `timeout_secs` for an alert, then switches to it.
    pub async fn wait_for_and_switch_to_alert(
        &self,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_alert_present().await? {
                return self.switch_to_alert().await;
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(
                    "No alert appeared".to_owned(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
}
