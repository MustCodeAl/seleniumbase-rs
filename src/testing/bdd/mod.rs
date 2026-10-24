//! Gherkin features as cargo tests, through [rstest-bdd](https://docs.rs/rstest-bdd)
//! (feature `bdd`). This replaces the in-house `behave` module.
//!
//! A scenario is a test function that binds a feature file; its steps are
//! `#[given]`, `#[when]` and `#[then]` functions that ask for the `sb`
//! fixture and use the browser. [`sb`] gives a [`SbCase`], which derefs to a
//! [`BaseCase`]; [`sb_cdp`] gives a [`SbCdpCase`], which derefs to a pure CDP
//! [`Page`]. Both read the run configuration as the
//! [`testing`](crate::testing) fixtures do (the settings file, `SB_SETTINGS_FILE`
//! and the `SB_*` variables that `sbrs test` sets), and when the scenario is
//! over, whatever its end, they close the browser after taking the evidence of
//! a failure (`latest_logs/<test id>/`), running the debug prompts and the
//! finalizers: the module `session` of this one says how.
//!
//! ```text
//! # tests/features/login.feature
//! Feature: Login
//!   Scenario: Valid login
//!     Given the login page is displayed
//!     When I sign in as "demo"
//!     Then the title is "Welcome"
//! ```
//!
//! ```rust,ignore
//! // tests/login.rs (the test crate depends on rstest, rstest-bdd and
//! // rstest-bdd-macros, and on seleniumbase-rs with the features `testing` and `bdd`)
//! use rstest_bdd_macros::{given, scenario, then, when};
//! use seleniumbase_rs::testing::bdd::{sb, SbCase};
//! // `Result` is written out in full: the steps' macros name `std`'s `Result`.
//!
//! #[scenario(path = "tests/features/login.feature", name = "Valid login")]
//! #[tokio::test(flavor = "multi_thread")]
//! async fn valid_login(#[future(awt)] sb: SbCase) {}
//!
//! #[given("the login page is displayed")]
//! async fn login_page(sb: &mut SbCase) -> seleniumbase_rs::Result<()> {
//!     sb.open("https://example.com/login").await?;
//!     sb.breakpoint("Paused at the login page.").await
//! }
//!
//! #[when("I sign in as {user:string}")]
//! async fn sign_in(sb: &mut SbCase, user: String) -> seleniumbase_rs::Result<()> {
//!     sb.type_text("#user", &user).await
//! }
//!
//! #[then("the title is {title:string}")]
//! async fn title_is(sb: &mut SbCase, title: String) -> seleniumbase_rs::Result<()> {
//!     sb.assert_title(&title).await
//! }
//! ```
//!
//! # What is different from a test with `Sb::run`
//!
//! - A scenario's steps run once. `--reruns` does not apply, and `--time-limit`
//!   is the case's own limit (the next open, wait or action fails once it has
//!   run out), not a hard stop: a step cannot be dropped halfway.
//! - A failed step or assertion panics, and that is how the fixture learns
//!   the scenario failed. A skipped scenario (`rstest_bdd::skip!`) is a pass
//!   as far as the evidence goes.
//! - The scenario function must be `async` and take the fixture with
//!   `#[future(awt)]`; rstest-bdd 0.6 cannot combine `harness = ...` with an
//!   `async fn`, and its harness path cannot run a step that waits.
//! - `#[tokio::test(flavor = "multi_thread")]` is the safest flavor: the
//!   default one works too, but a connection a step opens late then lives on
//!   the test's runtime, which cannot move while the fixture closes the
//!   browser.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use super::lifecycle::{Finalizer, Plan};
use super::report::{self, Reporting};
use crate::api::runner::CdpCase;
use crate::config::settings::Settings;
use crate::config::test_settings::TestData;
use crate::core::log_helper::TestId;
use crate::error::{Result, SeleniumBaseError};
use crate::sb_cdp::{Browser, Page};
use crate::BaseCase;

mod session;
pub mod steps;

use session::{Launch, Scenario};

/// The plan of a scenario's test, after the settings are checked.
fn plan_for(
    settings: &Settings,
    driver: &'static str,
    logs_dir: Option<PathBuf>,
) -> Result<(Plan, Vec<Arc<dyn Finalizer>>)> {
    settings.validate()?;
    // Read on the thread the harness named after the test.
    let test = TestId::current();
    let mut plan = Plan::from_settings(settings, &test.id, driver)?;
    plan.display.clone_from(&test.display);
    // The steps of a scenario run once, and a hard stop would have to drop
    // them halfway.
    plan.reruns = 0;
    plan.time_limit = None;
    let own_logs = logs_dir.is_some();
    if let Some(dir) = logs_dir {
        plan.logs_dir = dir;
    }
    // A scenario is recorded in the run of the process when reports are on,
    // like a test of the `sb` fixture.
    let run = report::run_for(&Reporting::Auto, settings);
    if let (false, Some(run)) = (own_logs, &run) {
        plan.logs_dir = run.logs_dir();
    }
    let finalizers = report::finalizers(run.as_ref(), settings, &test, &plan.logs_dir);
    Ok((plan, finalizers))
}

/// The test's time limit in whole seconds, for the case's own (soft) limit.
fn soft_limit(settings: &Settings) -> Option<u64> {
    let seconds = settings.test.time_limit?;
    let limit = std::time::Duration::try_from_secs_f64(seconds).ok()?;
    let whole = limit.as_secs() + u64::from(limit.subsec_nanos() > 0);
    (whole > 0).then_some(whole)
}

/// The browser of a Gherkin scenario run over WebDriver: a [`BaseCase`] that
/// the steps use, and that is closed when the scenario ends.
///
/// Get it from the [`sb`] fixture, or launch one with [`launch`](Self::launch).
pub struct SbCase {
    scenario: Scenario<BaseCase>,
    test_id: String,
}

impl SbCase {
    /// Launches the browser of the test running on this thread, with the run
    /// configuration of the environment: [`Settings::load`].
    ///
    /// # Errors
    ///
    /// Returns the settings' error or the browser's launch error.
    pub async fn launch() -> Result<Self> {
        Self::launch_with(Settings::load()?).await
    }

    /// As [`launch`](Self::launch), with these `settings` instead of the
    /// environment's.
    ///
    /// # Errors
    ///
    /// As [`launch`](Self::launch).
    pub async fn launch_with(settings: Settings) -> Result<Self> {
        Self::start(&settings, None).await
    }

    async fn start(settings: &Settings, logs_dir: Option<PathBuf>) -> Result<Self> {
        // The built-in steps register through `inventory`: referencing them
        // from here keeps their object files in every test binary that
        // launches an `SbCase`.
        steps::link();

        let (plan, finalizers) = plan_for(settings, "WebDriver", logs_dir)?;
        let test_id = plan.test_id.clone();
        let browser = settings.to_browser_config();
        let soft = soft_limit(settings);
        let launch: Launch<BaseCase> = Box::new(move |_attempt| {
            let browser = browser.clone();
            Box::pin(async move {
                let mut case = BaseCase::new(browser).await?;
                if let Some(seconds) = soft {
                    case.set_time_limit(seconds)?;
                }
                Ok::<_, SeleniumBaseError>(case)
            })
        });
        Ok(Self {
            scenario: Scenario::start(plan, finalizers, launch).await?,
            test_id,
        })
    }

    /// The id of the test: its binary, module path and function joined with
    /// `.`. It names the folder of evidence.
    #[must_use]
    pub fn test_id(&self) -> &str {
        &self.test_id
    }
}

impl std::fmt::Debug for SbCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SbCase")
            .field("test_id", &self.test_id)
            .finish_non_exhaustive()
    }
}

impl Deref for SbCase {
    type Target = BaseCase;

    fn deref(&self) -> &BaseCase {
        self.scenario.case()
    }
}

impl DerefMut for SbCase {
    fn deref_mut(&mut self) -> &mut BaseCase {
        self.scenario.case_mut()
    }
}

/// The browser of a Gherkin scenario run over pure CDP: its first [`Page`],
/// which the steps use, and the browser, closed when the scenario ends.
///
/// Get it from the [`sb_cdp`] fixture, or launch one with
/// [`launch`](Self::launch).
pub struct SbCdpCase {
    scenario: Scenario<CdpCase>,
    test_id: String,
    data: TestData,
}

impl SbCdpCase {
    /// As [`SbCase::launch`], over CDP.
    ///
    /// # Errors
    ///
    /// Returns the settings' error or the browser's launch error.
    pub async fn launch() -> Result<Self> {
        Self::launch_with(Settings::load()?).await
    }

    /// As [`SbCase::launch_with`], over CDP.
    ///
    /// # Errors
    ///
    /// As [`launch`](Self::launch).
    pub async fn launch_with(settings: Settings) -> Result<Self> {
        let (plan, finalizers) = plan_for(&settings, "CDP (no driver)", None)?;
        // Checked here, so a setting that cannot start a browser fails the
        // fixture with its own message.
        settings.to_launch_options()?;
        let launch: Launch<CdpCase> = Box::new({
            let settings = settings.clone();
            move |_attempt| {
                let settings = settings.clone();
                Box::pin(async move {
                    CdpCase::open(Browser::launch(settings.to_launch_options()?).await?).await
                })
            }
        });
        Self::from_plan(plan, finalizers, settings.test.data, launch).await
    }

    /// [`launch_with`](Self::launch_with) on the browser `launch` makes, such
    /// as `Browser::new_mocked()`'s, with the evidence under `logs_dir`.
    ///
    /// # Errors
    ///
    /// As [`launch`](Self::launch).
    #[cfg(any(test, feature = "test-util"))]
    pub async fn launch_with_browser<F, Fut>(
        settings: Settings,
        logs_dir: impl Into<PathBuf>,
        launch: F,
    ) -> Result<Self>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Browser>> + 'static,
    {
        let (plan, finalizers) = plan_for(&settings, "CDP (no driver)", Some(logs_dir.into()))?;
        let launch: Launch<CdpCase> = Box::new(move |_attempt| {
            let opening = launch();
            Box::pin(async move { CdpCase::open(opening.await?).await })
        });
        Self::from_plan(plan, finalizers, settings.test.data, launch).await
    }

    async fn from_plan(
        plan: Plan,
        finalizers: Vec<Arc<dyn Finalizer>>,
        data: TestData,
        launch: Launch<CdpCase>,
    ) -> Result<Self> {
        let test_id = plan.test_id.clone();
        Ok(Self {
            scenario: Scenario::start(plan, finalizers, launch).await?,
            test_id,
            data,
        })
    }

    /// The id of the test; see [`SbCase::test_id`].
    #[must_use]
    pub fn test_id(&self) -> &str {
        &self.test_id
    }

    /// The test data of the run (`--env`, `--data`, `--var1`, ...). A [`Page`]
    /// has none of its own, so the steps read it here.
    #[must_use]
    pub fn data(&self) -> &TestData {
        &self.data
    }

    /// The browser the page belongs to.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.scenario.case().browser
    }
}

impl std::fmt::Debug for SbCdpCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SbCdpCase")
            .field("test_id", &self.test_id)
            .finish_non_exhaustive()
    }
}

impl Deref for SbCdpCase {
    type Target = Page;

    fn deref(&self) -> &Page {
        &self.scenario.case().page
    }
}

/// The fixture of a WebDriver scenario: `#[future(awt)] sb: SbCase` in the
/// signature of a scenario function.
///
/// A browser that cannot start fails the scenario with the reason.
///
/// # Panics
///
/// Panics when the settings are wrong or the browser does not start.
#[rstest::fixture]
pub async fn sb() -> SbCase {
    SbCase::launch()
        .await
        .unwrap_or_else(|error| panic!("the sb fixture could not start the browser: {error}"))
}

/// The fixture of a pure CDP scenario: `#[future(awt)] sb_cdp: SbCdpCase` in
/// the signature of a scenario function.
///
/// # Panics
///
/// Panics when the settings are wrong or the browser does not start.
#[rstest::fixture]
pub async fn sb_cdp() -> SbCdpCase {
    SbCdpCase::launch()
        .await
        .unwrap_or_else(|error| panic!("the sb_cdp fixture could not start the browser: {error}"))
}
