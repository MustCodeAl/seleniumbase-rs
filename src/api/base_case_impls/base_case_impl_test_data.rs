// Test data: what `--env`, `--account`, `--data`, `--var1` to `--var3` and
// `--variables` hand to a test, under the names Python SeleniumBase gives them.

impl BaseCase {
    /// The free test data of the run (`--data`, `SB_DATA`), Python's
    /// `self.data`.
    ///
    /// ```
    /// # use seleniumbase_rs::{BaseCase, BrowserConfig};
    /// let sb = BaseCase::without_session(BrowserConfig::default());
    /// assert_eq!(sb.data(), None);
    /// assert_eq!(sb.env(), "test");
    /// ```
    #[must_use]
    pub fn data(&self) -> Option<&str> {
        self.config.launch.test_data.data.as_deref()
    }

    /// The environment the run is against (`--env`, `SB_ENV`): one of the 24
    /// names of [`VALID_ENVS`](crate::config::test_settings::VALID_ENVS) in
    /// lower case, and `test` when none was given. Python's `self.env`.
    #[must_use]
    pub fn env(&self) -> &str {
        self.config.launch.test_data.env()
    }

    /// [`env`](Self::env), under Python's other name for it.
    #[must_use]
    pub fn environment(&self) -> &str {
        self.env()
    }

    /// The account name of the run (`--account`, `SB_ACCOUNT`).
    #[must_use]
    pub fn account(&self) -> Option<&str> {
        self.config.launch.test_data.account.as_deref()
    }

    /// The first free variable of the run (`--var1`, `SB_VAR1`).
    #[must_use]
    pub fn var1(&self) -> Option<&str> {
        self.config.launch.test_data.var1.as_deref()
    }

    /// The second free variable of the run (`--var2`, `SB_VAR2`).
    #[must_use]
    pub fn var2(&self) -> Option<&str> {
        self.config.launch.test_data.var2.as_deref()
    }

    /// The third free variable of the run (`--var3`, `SB_VAR3`).
    #[must_use]
    pub fn var3(&self) -> Option<&str> {
        self.config.launch.test_data.var3.as_deref()
    }

    /// The dictionary of the run (`--variables`, `SB_VARIABLES`); empty when
    /// none was given.
    #[must_use]
    pub fn variables(&self) -> &serde_json::Map<String, Value> {
        &self.config.launch.test_data.variables
    }

    /// The id of the test this case runs: the name given by
    /// [`with_test_name`](Self::with_test_name) (the `sb` fixture gives it the
    /// test's id), else the id of the test running on this thread
    /// ([`current_test_id`](crate::common::test_id::current_test_id)).
    #[must_use]
    pub fn test_id(&self) -> String {
        self.test_name
            .clone()
            .unwrap_or_else(crate::common::test_id::current_test_id)
    }

    /// Gives the case the hard time limit of the lifecycle that runs it.
    #[cfg(feature = "testing")]
    pub(crate) fn set_deadline(&mut self, deadline: Arc<crate::testing::Deadline>) {
        self.deadline = Some(deadline);
    }
}

#[cfg(test)]
mod test_data_tests {
    use super::*;
    use crate::config::test_settings::TestData;

    fn case_with(data: TestData) -> BaseCase {
        let mut config = BrowserConfig::default();
        config.launch.test_data = data;
        BaseCase::without_session(config)
    }

    #[test]
    fn a_run_without_data_has_the_test_environment_and_nothing_else() {
        let sb = case_with(TestData::default());
        assert_eq!(sb.env(), "test");
        assert_eq!(sb.environment(), "test");
        assert_eq!(sb.data(), None);
        assert_eq!(sb.account(), None);
        assert_eq!((sb.var1(), sb.var2(), sb.var3()), (None, None, None));
        assert!(sb.variables().is_empty());
    }

    #[test]
    fn the_data_of_the_run_is_what_the_test_reads() {
        let sb = case_with(TestData {
            env: Some("qa".to_owned()),
            account: Some("alice".to_owned()),
            data: Some("abc".to_owned()),
            var1: Some("x".to_owned()),
            var2: Some("y".to_owned()),
            var3: Some("z".to_owned()),
            variables: TestData::parse_variables("{'k': 1}").unwrap(),
        });
        assert_eq!(sb.env(), "qa");
        assert_eq!(sb.environment(), "qa");
        assert_eq!(sb.account(), Some("alice"));
        assert_eq!(sb.data(), Some("abc"));
        assert_eq!((sb.var1(), sb.var2(), sb.var3()), (Some("x"), Some("y"), Some("z")));
        assert_eq!(sb.variables()["k"], 1);
    }

    #[test]
    fn the_test_id_is_the_given_name_or_the_running_test() {
        let sb = case_with(TestData::default());
        assert!(sb.test_id().ends_with("the_test_id_is_the_given_name_or_the_running_test"));
        let sb = sb.with_test_name("login.valid_login");
        assert_eq!(sb.test_id(), "login.valid_login");
    }
}
