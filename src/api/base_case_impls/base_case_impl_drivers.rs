// Multi-driver session management helpers.

/// Names one of the browsers of a [`BaseCase`]: the one it started with
/// ([`DriverId::DEFAULT`]) or one that [`BaseCase::get_new_driver`] opened.
///
/// An id names the same browser for as long as the case lives, whichever
/// browser is active, and it is never given to another browser after its
/// browser has been quit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DriverId(usize);

impl DriverId {
    /// The browser the case started with. [`BaseCase::switch_to_default_driver`]
    /// goes back to it.
    pub const DEFAULT: Self = Self(0);

    /// The position of the browser in the order they were opened: 0 for the
    /// default one, then 1, 2 and so on.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for DriverId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "driver {}", self.0)
    }
}

/// What became of one of the browsers of a case.
enum DriverSlot<T> {
    /// The browser the case is using now. Its session is the case's `session`
    /// field, not the table's.
    Active,
    /// An open browser waiting to be switched to.
    Parked(T),
    /// A browser that was quit.
    Quit,
}

/// The browsers of a case, by [`DriverId`]. Generic over the session, so the
/// bookkeeping is tested without a browser.
struct DriverTable<T> {
    active: usize,
    slots: Vec<DriverSlot<T>>,
}

impl<T> DriverTable<T> {
    /// A table with the default browser, which is the active one.
    fn new() -> Self {
        Self {
            active: 0,
            slots: vec![DriverSlot::Active],
        }
    }

    fn active_id(&self) -> DriverId {
        DriverId(self.active)
    }

    /// How many browsers are open, the active one included.
    fn open_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| !matches!(slot, DriverSlot::Quit))
            .count()
    }

    fn not_open(id: DriverId, why: &str) -> SeleniumBaseError {
        SeleniumBaseError::InvalidConfig(format!("{id} {why}"))
    }

    /// Parks `active` and puts `new` in its place, under a new id.
    fn add(&mut self, active: &mut T, new: T) -> DriverId {
        let left = std::mem::replace(active, new);
        self.slots[self.active] = DriverSlot::Parked(left);
        self.slots.push(DriverSlot::Active);
        self.active = self.slots.len() - 1;
        DriverId(self.active)
    }

    /// Makes `id` the active browser: its session goes into `active` and the
    /// session that was there is parked, so neither is dropped. Does nothing
    /// if `id` is active already.
    fn switch(&mut self, id: DriverId, active: &mut T) -> Result<(), SeleniumBaseError> {
        if id.0 == self.active {
            return Ok(());
        }
        let count = self.slots.len();
        let slot = self.slots.get_mut(id.0).ok_or_else(|| {
            Self::not_open(id, &format!("does not exist (the case opened {count} browsers)"))
        })?;
        match std::mem::replace(slot, DriverSlot::Active) {
            DriverSlot::Parked(wanted) => {
                let left = std::mem::replace(active, wanted);
                self.slots[self.active] = DriverSlot::Parked(left);
                self.active = id.0;
                Ok(())
            }
            other => {
                *slot = other;
                Err(Self::not_open(id, "has been quit"))
            }
        }
    }

    /// Takes the session of the parked browser `id` so it can be quit.
    fn take_parked(&mut self, id: DriverId) -> Result<T, SeleniumBaseError> {
        let count = self.slots.len();
        let slot = self.slots.get_mut(id.0).ok_or_else(|| {
            Self::not_open(id, &format!("does not exist (the case opened {count} browsers)"))
        })?;
        match std::mem::replace(slot, DriverSlot::Quit) {
            DriverSlot::Parked(session) => Ok(session),
            other => {
                *slot = other;
                Err(Self::not_open(id, "is not a parked browser"))
            }
        }
    }

    /// Retires the active browser: the default browser's session goes into
    /// `active` and the one that was there is returned, to be quit.
    fn retire_active(&mut self, active: &mut T) -> Result<T, SeleniumBaseError> {
        if self.active == DriverId::DEFAULT.0 {
            return Err(Self::not_open(
                DriverId::DEFAULT,
                "is the default browser, which quit() closes",
            ));
        }
        let default = match std::mem::replace(&mut self.slots[0], DriverSlot::Active) {
            DriverSlot::Parked(default) => default,
            other => {
                self.slots[0] = other;
                return Err(Self::not_open(DriverId::DEFAULT, "has been quit"));
            }
        };
        let retired = std::mem::replace(active, default);
        self.slots[self.active] = DriverSlot::Quit;
        self.active = 0;
        Ok(retired)
    }

    /// Takes every parked session, to quit them all.
    fn take_all_parked(&mut self) -> Vec<T> {
        let mut sessions = Vec::new();
        for slot in &mut self.slots {
            if matches!(slot, DriverSlot::Parked(_)) {
                if let DriverSlot::Parked(session) = std::mem::replace(slot, DriverSlot::Quit) {
                    sessions.push(session);
                }
            }
        }
        sessions
    }
}

impl BaseCase {
    /// Opens a new browser with this case's configuration and makes it the
    /// active one. Returns its [`DriverId`]; the browser that was active stays
    /// open, and [`switch_to_driver`](Self::switch_to_driver) or
    /// [`switch_to_default_driver`](Self::switch_to_default_driver) goes back
    /// to it. Use [`get_new_driver_with`](Self::get_new_driver_with) for
    /// another configuration.
    ///
    /// The new browser is never one kept for reuse, as in Python: the
    /// configuration's reuse settings are switched off for it.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be started.
    pub async fn get_new_driver(&mut self) -> Result<DriverId, SeleniumBaseError> {
        self.get_new_driver_with(self.config.clone()).await
    }

    /// Opens a new browser as `config` describes and makes it the active one.
    /// See [`get_new_driver`](Self::get_new_driver).
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be started.
    pub async fn get_new_driver_with(
        &mut self,
        mut config: BrowserConfig,
    ) -> Result<DriverId, SeleniumBaseError> {
        config.reuse_session = false;
        config.launch.reuse_class = None;
        let session = BrowserSession::connect(config).await?;
        let id = self.drivers.add(&mut self.session, session);
        info!(driver = %id, "created new driver session");
        Ok(id)
    }

    /// The browser the case is using now.
    #[must_use]
    pub fn active_driver(&self) -> DriverId {
        self.drivers.active_id()
    }

    /// Makes the browser `id` the active one. The browser that was active
    /// stays open. Switching to the browser that is active already does
    /// nothing, so the call can be repeated.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the case never opened a
    /// browser by that id or it has been quit; nothing changes.
    pub async fn switch_to_driver(&mut self, id: DriverId) -> Result<(), SeleniumBaseError> {
        self.drivers.switch(id, &mut self.session)?;
        info!(driver = %id, "switched to driver");
        Ok(())
    }

    /// Quits the browser `id`, which [`get_new_driver`](Self::get_new_driver)
    /// opened. If it is the active one, the case switches to the default
    /// browser, as Python's `quit_extra_driver` does.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for [`DriverId::DEFAULT`],
    /// which [`quit`](Self::quit) closes, and for a browser that does not exist
    /// or was quit already.
    pub async fn quit_extra_driver(&mut self, id: DriverId) -> Result<(), SeleniumBaseError> {
        if id == DriverId::DEFAULT {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "{id} is the default browser, which quit() closes"
            )));
        }
        let mut session = if id == self.drivers.active_id() {
            self.drivers.retire_active(&mut self.session)?
        } else {
            self.drivers.take_parked(id)?
        };
        session.quit().await?;
        info!(driver = %id, "quit extra driver session");
        Ok(())
    }

    /// How many browsers this case has open: the active one and those it can
    /// switch to.
    #[must_use]
    pub fn driver_count(&self) -> usize {
        self.drivers.open_count()
    }
}

#[cfg(test)]
mod tests_drivers {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A table of the sessions "a" (the default), "b" and "c", with "c" active.
    fn three() -> (DriverTable<&'static str>, &'static str) {
        let mut table = DriverTable::new();
        let mut active = "a";
        table.add(&mut active, "b");
        table.add(&mut active, "c");
        (table, active)
    }

    #[test]
    fn a_new_driver_gets_the_next_id_and_is_the_active_one() {
        let mut table = DriverTable::new();
        let mut active = "a";
        assert_eq!(table.active_id(), DriverId::DEFAULT);
        assert_eq!(table.add(&mut active, "b"), DriverId(1));
        assert_eq!((active, table.active_id()), ("b", DriverId(1)));
        assert_eq!(table.add(&mut active, "c"), DriverId(2));
        assert_eq!((active, table.open_count()), ("c", 3));
    }

    #[test]
    fn switching_to_the_default_twice_stays_on_the_default() {
        // It used to swap with slot 0, so the second call went back.
        let (mut table, mut active) = three();
        table.switch(DriverId::DEFAULT, &mut active).unwrap();
        assert_eq!(active, "a");
        table.switch(DriverId::DEFAULT, &mut active).unwrap();
        assert_eq!(active, "a");
    }

    #[test]
    fn the_default_is_the_first_browser_after_any_number_of_switches() {
        // It used to be whichever browser was left most recently.
        let (mut table, mut active) = three();
        table.switch(DriverId(1), &mut active).unwrap();
        assert_eq!(active, "b");
        table.switch(DriverId::DEFAULT, &mut active).unwrap();
        assert_eq!(active, "a");
        table.switch(DriverId(2), &mut active).unwrap();
        assert_eq!(active, "c");
        table.switch(DriverId::DEFAULT, &mut active).unwrap();
        assert_eq!(active, "a");
        assert_eq!(table.active_id(), DriverId::DEFAULT);
        assert_eq!(table.open_count(), 3, "no browser was dropped");
    }

    #[test]
    fn switching_to_the_active_driver_does_nothing() {
        let (mut table, mut active) = three();
        table.switch(DriverId(2), &mut active).unwrap();
        assert_eq!((active, table.active_id()), ("c", DriverId(2)));
    }

    #[test]
    fn switching_to_a_driver_that_does_not_exist_or_was_quit_changes_nothing() {
        let (mut table, mut active) = three();
        let error = table.switch(DriverId(7), &mut active).unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(m) if m.contains("driver 7") && m.contains("3 browsers")),
        );
        assert_eq!((active, table.active_id()), ("c", DriverId(2)));

        assert_eq!(table.take_parked(DriverId(1)).unwrap(), "b");
        let error = table.switch(DriverId(1), &mut active).unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(m) if m.contains("has been quit")));
        assert_eq!((active, table.active_id(), table.open_count()), ("c", DriverId(2), 2));
    }

    #[test]
    fn a_quit_driver_keeps_its_id_and_the_next_one_gets_a_new_id() {
        let (mut table, mut active) = three();
        table.take_parked(DriverId(1)).unwrap();
        assert_eq!(table.add(&mut active, "d"), DriverId(3));
        assert!(table.take_parked(DriverId(1)).is_err(), "it cannot be quit twice");
    }

    #[test]
    fn quitting_the_active_driver_brings_the_default_back() {
        let (mut table, mut active) = three();
        let retired = table.retire_active(&mut active).unwrap();
        assert_eq!((retired, active, table.active_id()), ("c", "a", DriverId::DEFAULT));
        assert_eq!(table.open_count(), 2);
        // "b" is still parked and can be switched to.
        table.switch(DriverId(1), &mut active).unwrap();
        assert_eq!(active, "b");
        // The default is not an extra driver.
        table.switch(DriverId::DEFAULT, &mut active).unwrap();
        assert!(table.retire_active(&mut active).is_err());
    }

    #[test]
    fn quitting_everything_takes_every_parked_session() {
        let (mut table, _active) = three();
        let mut parked = table.take_all_parked();
        parked.sort_unstable();
        assert_eq!(parked, ["a", "b"]);
        assert_eq!(table.open_count(), 1, "only the active one is left");
        assert!(table.take_all_parked().is_empty());
    }

    /// A WebDriver server whose page title is `title`.
    fn titled(title: &'static str) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/title") => Reply::Value(json!(title)),
                _ => Reply::Value(serde_json::Value::Null),
            }
        })
    }

    /// The configuration of a case that uses `driver`.
    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        config
    }

    #[tokio::test]
    async fn the_commands_follow_the_active_browser() {
        let (first, second) = (titled("first"), titled("second"));
        let mut sb = first.base_case().await;
        assert_eq!((sb.driver_count(), sb.active_driver()), (1, DriverId::DEFAULT));
        assert_eq!(sb.get_title().await.unwrap(), "first");

        let other = sb.get_new_driver_with(config_for(&second)).await.unwrap();
        assert_eq!((other, sb.active_driver(), sb.driver_count()), (DriverId(1), DriverId(1), 2));
        assert_eq!(sb.get_title().await.unwrap(), "second");

        // However often it is asked, the default is the first browser.
        for _ in 0..3 {
            sb.switch_to_default_driver().await.unwrap();
            assert_eq!(sb.get_title().await.unwrap(), "first");
        }
        sb.switch_to_driver(other).await.unwrap();
        sb.switch_to_driver(other).await.unwrap();
        assert_eq!(sb.get_title().await.unwrap(), "second");

        // Quitting the active extra browser falls back to the default one.
        sb.quit_extra_driver(other).await.unwrap();
        assert_eq!((sb.active_driver(), sb.driver_count()), (DriverId::DEFAULT, 1));
        assert_eq!(sb.get_title().await.unwrap(), "first");
        assert_eq!(second.requests("").len(), 1, "the second browser got a delete-session");
        assert!(sb.switch_to_driver(other).await.is_err());
        assert!(sb.quit_extra_driver(other).await.is_err());
        assert!(
            sb.quit_extra_driver(DriverId::DEFAULT).await.is_err(),
            "the default browser is quit with quit()"
        );
    }

    #[tokio::test]
    async fn quit_closes_every_browser() {
        let (first, second) = (titled("first"), titled("second"));
        let mut sb = first.base_case().await;
        sb.get_new_driver_with(config_for(&second)).await.unwrap();
        sb.quit().await.unwrap();
        for driver in [&first, &second] {
            let deleted = driver
                .all_requests()
                .iter()
                .filter(|request| request.method == "DELETE" && request.path.is_empty())
                .count();
            assert_eq!(deleted, 1);
        }
    }

    #[tokio::test]
    async fn a_case_without_a_second_browser_has_one() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        assert_eq!(sb.driver_count(), 1);
        assert!(sb.switch_to_driver(DriverId(3)).await.is_err());
        assert!(sb.quit_extra_driver(DriverId(1)).await.is_err());
        assert_eq!(sb.driver_count(), 1);
    }
}
