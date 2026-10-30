// Alert helpers.

impl BaseCase {
    /// Waits for an alert, confirm box or prompt and returns a handle on it.
    /// Corresponds to Python's `switch_to_alert`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no alert opens in time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.execute_script("window.prompt(\"My Prompt\", \"text\");").await?;
    /// let alert = sb.switch_to_alert().await?;
    /// sb.assert_equal(alert.text().await?.as_str(), "My Prompt")?;
    /// alert.dismiss().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn switch_to_alert(&self) -> SwitchToAlert<'_> {
        SwitchToAlert::new(self)
    }

    /// Waits up to `timeout_secs` for an alert and returns a handle on it, as
    /// [`switch_to_alert`](Self::switch_to_alert) does with its own timeout.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no alert appears in time.
    pub async fn wait_for_and_switch_to_alert(
        &self,
        timeout_secs: u64,
    ) -> Result<Alert, SeleniumBaseError> {
        self.run_switch_to_alert(Some(Duration::from_secs(timeout_secs)))
            .await
    }
}
