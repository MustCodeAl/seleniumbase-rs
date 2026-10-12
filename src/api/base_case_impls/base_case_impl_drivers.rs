// Multi-driver session management helpers.

/// Swaps `active` with `parked[index]`, so the one that was active is parked
/// in the slot the other came from and neither is dropped.
fn swap_with_parked<T>(
    active: &mut T,
    parked: &mut [T],
    index: usize,
) -> Result<(), SeleniumBaseError> {
    let count = parked.len();
    let slot = parked.get_mut(index).ok_or_else(|| {
        SeleniumBaseError::InvalidConfig(format!(
            "driver index {index} out of range ({count} extra sessions)"
        ))
    })?;
    std::mem::swap(active, slot);
    Ok(())
}

impl BaseCase {
    /// Creates a new browser session and switches to it.
    ///
    /// If `config` is `None`, the current `BrowserConfig` is cloned. The driver
    /// that was active is parked in the internal driver list and the new one
    /// becomes the active session. Returns the index at which the previous
    /// driver was parked; pass it to [`BaseCase::switch_to_driver`] to go back
    /// to that driver.
    ///
    /// The new browser is never one kept for reuse, as in Python: the
    /// configuration's reuse settings are switched off for it.
    pub async fn get_new_driver(
        &mut self,
        config: Option<BrowserConfig>,
    ) -> Result<usize, SeleniumBaseError> {
        let mut cfg = config.unwrap_or_else(|| self.config.clone());
        cfg.reuse_session = false;
        cfg.launch.reuse_class = None;
        let session = BrowserSession::connect(cfg.clone()).await?;
        self.extra_sessions.push(std::mem::replace(&mut self.session, session));
        let index = self.extra_sessions.len() - 1;
        info!(driver_index = index, "created new driver session");
        Ok(index)
    }

    /// Makes the driver parked at `index` the active one, and parks the driver
    /// that was active in its place.
    ///
    /// `index` is the position [`BaseCase::get_new_driver`] returned. The
    /// driver that was active stays open, parked at the same index, so calling
    /// this again with the same index switches back.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `index` is out of range.
    pub async fn switch_to_driver(&mut self, index: usize) -> Result<(), SeleniumBaseError> {
        swap_with_parked(&mut self.session, &mut self.extra_sessions, index)?;
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

#[cfg(test)]
mod tests_drivers {
    use super::*;

    #[test]
    fn switching_parks_the_active_driver_instead_of_dropping_it() {
        // Dropping a `BrowserSession` quits its browser, so a driver that is
        // replaced without being parked is a browser the test loses.
        let (mut active, mut parked) = ("second", ["first"]);
        swap_with_parked(&mut active, &mut parked, 0).unwrap();
        assert_eq!((active, parked), ("first", ["second"]));
        swap_with_parked(&mut active, &mut parked, 0).unwrap();
        assert_eq!((active, parked), ("second", ["first"]), "the same index switches back");
    }

    #[test]
    fn switching_to_a_driver_that_does_not_exist_changes_nothing() {
        let (mut active, mut parked) = ("a", ["b", "c"]);
        let error = swap_with_parked(&mut active, &mut parked, 2).unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(m) if m.contains("out of range (2 extra sessions)")));
        assert_eq!((active, parked), ("a", ["b", "c"]));
    }

    #[tokio::test]
    async fn the_driver_list_starts_empty() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        assert_eq!(sb.driver_count(), 0);
        assert!(sb.switch_to_driver(0).await.is_err());
        assert!(sb.quit_extra_driver(0).await.is_err());
        assert_eq!(sb.driver_count(), 0);
    }
}
