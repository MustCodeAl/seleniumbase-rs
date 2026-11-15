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
//! // tests/login.rs. The test crate depends on seleniumbase-rs with the features
//! // `testing` and `bdd`, and on rstest-bdd, which the macros read from the manifest
//! // (no file names it).

//! use seleniumbase_rs::testing::bdd::{given, sb, scenario, then, when, SbCase};
//! use seleniumbase_rs::testing::rstest; // the `scenario` attribute expands to `rstest::` paths
//! use seleniumbase_rs::tokio;

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
//!   the scenario failed. A skipped scenario (the `skip!` macro) is a pass
//!   as far as the evidence goes.
//! - The scenario function must be `async` and take the fixture with
//!   `#[future(awt)]`; rstest-bdd 0.6 cannot combine `harness = ...` with an
//!   `async fn`, and its harness path cannot run a step that waits.
//! - `#[tokio::test(flavor = "multi_thread")]` is the safest flavor: the
//!   default one works too, but a connection a step opens late then lives on
//!   the test's runtime, which cannot move while the fixture closes the
//!   browser.

use std::path::PathBuf;
use std::sync::Arc;

use super::lifecycle::{Finalizer, Plan};
use super::report::{self, Reporting};
use crate::config::settings::Settings;
use crate::core::log_helper::TestId;
use crate::error::Result;

mod sb_case;
mod sb_cdp_case;
mod session;
pub mod steps;

pub use ::rstest_bdd::skip;
pub use ::rstest_bdd_macros::{given, scenario, scenarios, then, when};
pub use sb_case::SbCase;
pub use sb_cdp_case::SbCdpCase;

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
