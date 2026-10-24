//! The `sb` and `sb_cdp` fixtures and the handles they give a test.
//!
//! `Drop` cannot await and the runtime of a `#[tokio::test]` is gone when a
//! background clean-up would run, so a fixture cannot close a browser after the
//! test. The fixture is a handle instead: [`Sb`] and [`SbCdp`] hold the
//! configuration and the identity of the test, and their `run` owns the whole
//! browser lifecycle ([`lifecycle`](super::lifecycle)). That also gives reruns
//! (the body is an `AsyncFn`, called again) and evidence of an `Err` return,
//! which a guard could not see.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::deadline::Deadline;
#[cfg(feature = "plugins")]
use super::lifecycle::TestPlugins;
use super::lifecycle::{say_on_stderr, Finalizer, Lifecycle, Plan, TerminalPrompter, TestCase};
use super::report::{self, Reporting};
use crate::api::recorder::RecordingTarget;
use crate::api::runner::{close_browser, CdpCase};
use crate::common::debug_prompt::PromptPage;
use crate::config::settings::Settings;
use crate::config::test_settings::TestData;
use crate::core::log_helper::TestId;
use crate::core::test_run::TestRun;
use crate::error::{Result, SeleniumBaseError};
use crate::sb_cdp::{Browser, Page};
use crate::BaseCase;

/// What `Sb` and `SbCdp` share: the settings (or why they could not be read),
/// the id of the test and what is registered to see it end.
struct Config {
    settings: std::result::Result<Settings, String>,
    test: TestId,
    reporting: Reporting,
    logs_dir: Option<PathBuf>,
    finalizers: Vec<Arc<dyn Finalizer>>,
}

impl Config {
    fn from_env() -> Self {
        // The id is read here, on the thread the harness named after the test.
        let test = TestId::current();
        let settings = Settings::load().map_err(|error| match error {
            SeleniumBaseError::InvalidConfig(message) => message,
            other => other.to_string(),
        });
        Self::with(settings, test)
    }

    fn new(settings: Settings) -> Self {
        Self::with(Ok(settings), TestId::current())
    }

    fn with(settings: std::result::Result<Settings, String>, test: TestId) -> Self {
        Self {
            settings,
            test,
            reporting: Reporting::Auto,
            logs_dir: None,
            finalizers: Vec::new(),
        }
    }

    fn configure(mut self, change: impl FnOnce(&mut Settings)) -> Self {
        if let Ok(settings) = &mut self.settings {
            change(settings);
        }
        self
    }

    fn settings(&self) -> Result<&Settings> {
        self.settings
            .as_ref()
            .map_err(|message| SeleniumBaseError::InvalidConfig(message.clone()))
    }

    /// The plan of the test, after the settings are checked.
    fn plan(&self, driver: &'static str) -> Result<Plan> {
        let settings = self.settings()?;
        settings.validate()?;
        let mut plan = Plan::from_settings(settings, &self.test.id, driver)?;
        plan.display.clone_from(&self.test.display);
        if let Some(dir) = &self.logs_dir {
            plan.logs_dir.clone_from(dir);
        }
        Ok(plan)
    }

    async fn run<C, L, B>(&self, mut plan: Plan, launch: L, body: B) -> Result<()>
    where
        C: TestCase,
        L: AsyncFn(u32) -> Result<C>,
        B: AsyncFn(&mut C) -> Result<()>,
    {
        {
            let settings = self.settings()?;
            let run = report::run_for(&self.reporting, settings);
            if let (None, Some(run)) = (&self.logs_dir, &run) {
                plan.logs_dir = run.logs_dir();
            }
            let mut finalizers =
                report::finalizers(run.as_ref(), settings, &self.test, &plan.logs_dir);
            finalizers.extend(self.finalizers.iter().cloned());
            Lifecycle {
                plan: &plan,
                prompter: &TerminalPrompter,
                finalizers: &finalizers,
                say: &say_on_stderr,
            }
            .run(launch, body)
            .await
        }
    }

    fn describe(&self, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("test_id", &self.test.id)
            .field("settings", &self.settings.as_ref().map(|_| "read"))
            .field("finalizers", &self.finalizers.len())
            .finish_non_exhaustive()
    }
}

/// The handle of a WebDriver test, which the `sb` fixture gives a test.
///
/// It holds the run configuration (the settings file and the `SB_*`
/// variables, as `sbrs test` sets them) and the id of the test;
/// [`run`](Self::run) launches the browser, runs the test on a [`BaseCase`]
/// and does everything the run configuration asks for: reruns, a time limit,
/// evidence of a failure, the debug prompts.
///
/// ```no_run
/// use rstest::rstest;
/// use seleniumbase_rs::testing::{sb, Sb};
/// use seleniumbase_rs::Result;
///
/// #[rstest]
/// #[tokio::test]
/// async fn valid_login(sb: Sb) -> Result<()> {
///     sb.run(async |sb| {
///         sb.open("https://example.com").await?;
///         sb.assert_title("Example Domain").await
///     })
///     .await
/// }
/// ```
pub struct Sb {
    config: Config,
}

impl fmt::Debug for Sb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.config.describe("Sb", f)
    }
}

impl Sb {
    /// The run configuration of the environment: [`Settings::load`] (the
    /// settings file, `SB_SETTINGS_FILE`, then the `SB_*` variables) and the
    /// id of the test running on the calling thread.
    ///
    /// Settings that cannot be read do not fail here; [`run`](Self::run)
    /// returns the error, so the test that has them fails with their message.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            config: Config::from_env(),
        }
    }

    /// A handle with these `settings` instead of the environment's.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            config: Config::new(settings),
        }
    }

    /// Changes the settings for this test, the Rust form of Python's `SB(...)`
    /// arguments: `Sb::from_env().configure(|s| s.test.reruns = 2)`.
    #[must_use]
    pub fn configure(mut self, change: impl FnOnce(&mut Settings)) -> Self {
        self.config = self.config.configure(change);
        self
    }

    /// Records this test in `run` instead of the run of the process, whatever
    /// the run configuration asks for. Its evidence goes to the run's
    /// `latest_logs` unless [`logs_dir`](Self::logs_dir) says otherwise.
    ///
    /// Without this the test is recorded in the run of the process
    /// ([`TestRun::current`]) when the settings turn a report on
    /// (`--dashboard`, `--html`, `--junit-xml`, ...), and in none otherwise.
    #[must_use]
    pub fn report_to(mut self, run: Arc<TestRun>) -> Self {
        self.config.reporting = Reporting::Run(run);
        self
    }

    /// Records this test in no run, whatever the run configuration asks for.
    #[must_use]
    pub fn without_report(mut self) -> Self {
        self.config.reporting = Reporting::Off;
        self
    }

    /// Writes the evidence of a failure under `dir` instead of `latest_logs`.
    #[must_use]
    pub fn logs_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.logs_dir = Some(dir.into());
        self
    }

    /// Has `finalizer` told how every attempt of the test ended, with the page
    /// still open.
    #[must_use]
    pub fn finalizer(mut self, finalizer: Arc<dyn Finalizer>) -> Self {
        self.config.finalizers.push(finalizer);
        self
    }

    /// The id of the test: its binary, module path and function joined with
    /// `.`, such as `login.valid_login`. It names the folder of evidence.
    #[must_use]
    pub fn test_id(&self) -> &str {
        &self.config.test.id
    }

    /// The settings of the run, or the reason they could not be read.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when the settings file or
    /// an `SB_*` variable is wrong.
    pub fn settings(&self) -> Result<&Settings> {
        self.config.settings()
    }

    /// Launches a WebDriver browser, runs `test` on it with every behaviour
    /// of the run configuration, and closes it.
    ///
    /// The test is an async closure, `async |sb| { .. }`; it is called again
    /// when the test fails and `reruns` allow, so it must not consume what it
    /// captures. A test that panics fails like one that returns `Err`, and the
    /// panic of the last attempt is raised again after the browser is closed.
    ///
    /// # Errors
    ///
    /// Returns the settings' error, the browser's launch error, the test's
    /// error (of its last attempt), the time limit's error, or the cleanup's.
    pub async fn run<F>(&self, test: F) -> Result<()>
    where
        F: AsyncFn(&mut BaseCase) -> Result<()>,
    {
        let plan = self.config.plan("WebDriver")?;
        let browser = self.config.settings()?.to_browser_config();
        let test_id = self.config.test.id.clone();

        self.config
            .run(
                plan,
                async |_attempt: u32| {
                    let mut case = BaseCase::new(browser.clone()).await?;
                    if case.recorder_mode() {
                        case.set_recording_output(RecordingTarget::from_test_id(&test_id));
                    }
                    Ok(case)
                },
                test,
            )
            .await
    }
}

/// The handle of a pure CDP test, which the `sb_cdp` fixture gives a test.
///
/// As [`Sb`], with the browser driven over CDP and the test run on its first
/// [`Page`].
///
/// ```no_run
/// use rstest::rstest;
/// use seleniumbase_rs::testing::{sb_cdp, SbCdp};
/// use seleniumbase_rs::Result;
///
/// #[rstest]
/// #[tokio::test]
/// async fn heading(sb_cdp: SbCdp) -> Result<()> {
///     sb_cdp
///         .run(async |page| {
///             page.goto("https://example.com").await?;
///             page.locator("h1").click().await
///         })
///         .await
/// }
/// ```
pub struct SbCdp {
    config: Config,
}

impl fmt::Debug for SbCdp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.config.describe("SbCdp", f)
    }
}

impl SbCdp {
    /// As [`Sb::from_env`].
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            config: Config::from_env(),
        }
    }

    /// A handle with these `settings` instead of the environment's.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            config: Config::new(settings),
        }
    }

    /// As [`Sb::configure`].
    #[must_use]
    pub fn configure(mut self, change: impl FnOnce(&mut Settings)) -> Self {
        self.config = self.config.configure(change);
        self
    }

    /// As [`Sb::report_to`].
    #[must_use]
    pub fn report_to(mut self, run: Arc<TestRun>) -> Self {
        self.config.reporting = Reporting::Run(run);
        self
    }

    /// As [`Sb::without_report`].
    #[must_use]
    pub fn without_report(mut self) -> Self {
        self.config.reporting = Reporting::Off;
        self
    }

    /// As [`Sb::logs_dir`].
    #[must_use]
    pub fn logs_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.logs_dir = Some(dir.into());
        self
    }

    /// As [`Sb::finalizer`].
    #[must_use]
    pub fn finalizer(mut self, finalizer: Arc<dyn Finalizer>) -> Self {
        self.config.finalizers.push(finalizer);
        self
    }

    /// The id of the test; see [`Sb::test_id`].
    #[must_use]
    pub fn test_id(&self) -> &str {
        &self.config.test.id
    }

    /// The test data of the run (`--env`, `--data`, `--var1`, ...). A [`Page`]
    /// has none of its own, so the test reads it here. It is empty when the
    /// settings could not be read.
    #[must_use]
    pub fn data(&self) -> &TestData {
        static NONE: OnceLock<TestData> = OnceLock::new();
        match &self.config.settings {
            Ok(settings) => &settings.test.data,
            Err(_) => NONE.get_or_init(TestData::default),
        }
    }

    /// The settings of the run, or the reason they could not be read.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when the settings file or
    /// an `SB_*` variable is wrong.
    pub fn settings(&self) -> Result<&Settings> {
        self.config.settings()
    }

    /// Launches a Chrome over CDP, runs `test` on its first tab with every
    /// behaviour of the run configuration, and closes it. See [`Sb::run`].
    ///
    /// # Errors
    ///
    /// As [`Sb::run`].
    pub async fn run<F>(&self, test: F) -> Result<()>
    where
        F: AsyncFn(&Page) -> Result<()>,
    {
        let settings = self.config.settings()?;
        self.run_on(
            async || Browser::launch(settings.to_launch_options()?).await,
            test,
        )
        .await
    }

    /// [`run`](Self::run) on the browser `launch` makes (once per attempt),
    /// such as `Browser::new_mocked()`'s, instead of a Chrome started from the
    /// settings.
    ///
    /// # Errors
    ///
    /// As [`run`](Self::run).
    #[cfg(any(test, feature = "test-util"))]
    pub async fn run_with_browser<L, F>(&self, launch: L, test: F) -> Result<()>
    where
        L: AsyncFn() -> Result<Browser>,
        F: AsyncFn(&Page) -> Result<()>,
    {
        self.run_on(launch, test).await
    }

    async fn run_on<L, F>(&self, launch: L, test: F) -> Result<()>
    where
        L: AsyncFn() -> Result<Browser>,
        F: AsyncFn(&Page) -> Result<()>,
    {
        let plan = self.config.plan("CDP (no driver)")?;
        self.config
            .run(
                plan,
                async |_attempt: u32| CdpCase::open(launch().await?).await,
                async |case: &mut CdpCase| test(&case.page).await,
            )
            .await
    }
}

/// The fixture of a WebDriver test: `sb: Sb` in the signature of a test
/// function made with `#[rstest]`.
///
/// ```no_run
/// use rstest::rstest;
/// use seleniumbase_rs::testing::{sb, Sb};
/// use seleniumbase_rs::Result;
///
/// #[rstest]
/// #[tokio::test]
/// async fn title(sb: Sb) -> Result<()> {
///     sb.run(async |sb| sb.assert_title("Example Domain").await).await
/// }
/// ```
#[rstest::fixture]
#[must_use]
pub fn sb() -> Sb {
    Sb::from_env()
}

/// The fixture of a pure CDP test: `sb_cdp: SbCdp` in the signature of a test
/// function made with `#[rstest]`.
#[rstest::fixture]
#[must_use]
pub fn sb_cdp() -> SbCdp {
    SbCdp::from_env()
}

#[async_trait]
impl PromptPage for CdpCase {
    async fn url(&self) -> Result<String> {
        Page::url(&self.page).await
    }

    async fn title(&self) -> Result<String> {
        Page::title(&self.page).await
    }

    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        Page::screenshot(&self.page).await
    }

    async fn page_source(&self) -> Result<String> {
        Page::content(&self.page).await
    }

    async fn evaluate(&self, expression: &str) -> Result<Value> {
        Page::evaluate(&self.page, expression).await
    }
}

impl TestCase for CdpCase {
    async fn close(&mut self) -> Result<()> {
        CdpCase::close(self).await
    }

    #[cfg(feature = "plugins")]
    fn test_plugins(&self) -> Option<TestPlugins<'_>> {
        let set = &self.browser.inner().options.plugins;
        (!set.is_empty()).then_some(TestPlugins {
            set,
            page: &self.page,
        })
    }
}

impl TestCase for BaseCase {
    async fn close(&mut self) -> Result<()> {
        close_browser(self).await
    }

    fn name_test(&mut self, test_id: &str) {
        self.set_test_name(test_id);
    }

    fn attach_deadline(&mut self, deadline: Arc<Deadline>) {
        self.set_deadline(deadline);
    }

    fn extend_clock(&mut self, paused: Duration) {
        self.extend_time_limit(paused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_settings::TestSettings;

    #[test]
    fn the_handle_knows_its_test() {
        let sb = Sb::new(Settings::default());
        assert!(
            sb.test_id().ends_with("the_handle_knows_its_test"),
            "{}",
            sb.test_id()
        );
    }

    #[test]
    fn configure_changes_the_settings_of_this_test() {
        let sb = Sb::new(Settings::default()).configure(|settings| settings.test.reruns = 2);
        assert_eq!(sb.settings().unwrap().test.reruns, 2);
        let cdp = SbCdp::new(Settings::default()).configure(|settings| {
            settings.test = TestSettings {
                data: TestData {
                    data: Some("abc".to_owned()),
                    ..TestData::default()
                },
                ..TestSettings::default()
            };
        });
        assert_eq!(cdp.data().data.as_deref(), Some("abc"));
    }

    #[tokio::test]
    async fn settings_that_cannot_be_read_fail_the_run_not_the_fixture() {
        let sb = Sb {
            config: Config::with(
                Err("SB_THREADS: not a number".to_owned()),
                TestId::named("t"),
            ),
        };
        let error = sb.run(async |_: &mut BaseCase| Ok(())).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid configuration: SB_THREADS: not a number"
        );
        assert!(sb.settings().is_err());
    }

    #[tokio::test]
    async fn checked_settings_fail_the_run_before_a_browser_starts() {
        let sb = Sb::new(Settings {
            browser: "netscape".to_owned(),
            ..Settings::default()
        });
        let error = sb.run(async |_: &mut BaseCase| Ok(())).await.unwrap_err();
        assert!(error.to_string().contains("netscape"), "{error}");
    }

    #[test]
    fn a_handle_without_readable_settings_has_no_test_data() {
        let cdp = SbCdp {
            config: Config::with(Err("bad".to_owned()), TestId::named("t")),
        };
        assert_eq!(cdp.data(), &TestData::default());
    }

    #[test]
    fn a_handle_is_debug_without_its_secrets() {
        let sb = Sb::new(Settings::default());
        let shown = format!("{sb:?}");
        assert!(shown.starts_with("Sb {"), "{shown}");
        assert!(shown.contains("test_id"), "{shown}");
    }
}
