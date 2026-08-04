// Multi-driver session management helpers.

impl BaseCase {
    /// Creates a new browser session and switches to it.
    ///
    /// If `config` is `None`, the current `BrowserConfig` is cloned. The new
    /// session is appended to the internal driver list and becomes the active
    /// session. Returns the zero-based index of the new driver.
    pub async fn get_new_driver(
        &mut self,
        config: Option<BrowserConfig>,
    ) -> Result<usize, SeleniumBaseError> {
        let cfg = config.unwrap_or_else(|| self.config.clone());
        let session = BrowserSession::connect(cfg.clone()).await?;
        self.extra_sessions.push(std::mem::replace(&mut self.session, session));
        let index = self.extra_sessions.len() - 1;
        info!(driver_index = index, "created new driver session");
        Ok(index)
    }

    /// Switches the active session to the driver at `index`.
    ///
    /// `index` refers to the position returned by [`BaseCase::get_new_driver`].
    pub async fn switch_to_driver(&mut self, index: usize) -> Result<(), SeleniumBaseError> {
        if index >= self.extra_sessions.len() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "driver index {} out of range ({} extra sessions)",
                index,
                self.extra_sessions.len()
            )));
        }
        self.session = std::mem::replace(&mut self.extra_sessions[index], BrowserSession::disconnected());
        info!(driver_index = index, "switched to driver");
        Ok(())
    }

    /// Quits an extra driver created by [`BaseCase::get_new_driver`].
    ///
    /// The active `session` cannot be quit through this method; use
    /// [`BaseCase::quit`] for that.
    pub async fn quit_extra_driver(&mut self, index: usize) -> Result<(), SeleniumBaseError> {
        if index >= self.extra_sessions.len() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "driver index {} out of range ({} extra sessions)",
                index,
                self.extra_sessions.len()
            )));
        }
        let mut session = std::mem::replace(&mut self.extra_sessions[index], BrowserSession::disconnected());
        session.quit().await?;
        info!(driver_index = index, "quit extra driver session");
        Ok(())
    }

    /// Returns the number of extra driver sessions stored by this `BaseCase`.
    pub fn driver_count(&self) -> usize {
        self.extra_sessions.len()
    }
}
