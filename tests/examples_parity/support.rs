//! What the ports share: a headless browser configuration for each engine.
//!
//! The Python examples run headed by default and `--headless` turns that on.
//! The ports always run headless, with a window as large as the Python
//! headless default (1440 x 1880), so a page lays out the way it does there.

use seleniumbase_rs::api::runner::run_cdp_test;
use seleniumbase_rs::browser::options::WindowSize;
use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// The WebDriver engine (`BaseCase`), headless.
pub fn headless() -> BrowserConfig {
    BrowserConfig::default()
        .with_headless(true)
        .with_window_size(WindowSize::new(1440, 1880))
}

/// The pure CDP engine (`sb_cdp`), headless.
pub fn cdp_options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1440, 1880)
        .build()
        .expect("the launch options are valid")
}

/// The steps of a port where this crate does not do what the Python call
/// does. A port tries the Python call as this crate spells it; when that
/// fails it notes the step here and carries on with the nearest composition,
/// so one run lists every difference of the example. The test fails at the
/// end when it noted any, and stays green when the crate has caught up.
#[derive(Debug, Default)]
pub struct Mismatches(Vec<String>);

impl Mismatches {
    /// Notes that `step` of the Python example failed with `error`.
    #[expect(
        dead_code,
        reason = "no port notes a mismatch since frames of another origin work"
    )]

    pub fn note(&mut self, step: &str, error: impl std::fmt::Display) {
        println!("MISMATCH {step}: {error}");
        if !self.knows(step) {
            self.0.push(format!("{step}: {error}"));
        }
    }

    /// Whether `step` was noted already, so a port need not wait for the same
    /// failure again.
    #[expect(
        dead_code,
        reason = "only `note` asks, and no port calls it at the moment"
    )]

    pub fn knows(&self, step: &str) -> bool {
        self.0.iter().any(|known| known.starts_with(step))
    }

    /// Fails when a step was noted.
    pub fn report(&self) -> Option<String> {
        if self.0.is_empty() {
            return None;
        }
        Some(format!(
            "{} step(s) of the Python example differ:\n  {}",
            self.0.len(),
            self.0.join("\n  ")
        ))
    }

    /// Fails when a step was noted, with the report; `outcome` is how the test
    /// body ended, and a failure of its own comes first.
    pub fn finish(self, outcome: Result<()>) -> Result<()> {
        match (outcome, self.report()) {
            (Ok(()), None) => Ok(()),
            (Ok(()), Some(report)) => Err(SeleniumBaseError::AssertionFailed(report)),
            (Err(error), None) => Err(error),
            (Err(error), Some(report)) => Err(SeleniumBaseError::AssertionFailed(format!(
                "{error}\nBefore that, {report}"
            ))),
        }
    }
}

/// [`run_cdp_test`] for a port, as [`run_port`] is for [`run_browser_test`].
pub async fn run_cdp_port<F>(test: F) -> Result<()>
where
    F: AsyncFnOnce(&Page, &mut Mismatches) -> Result<()>,
{
    let mut mismatches = Mismatches::default();
    let outcome = run_cdp_test(cdp_options(), async |page| {
        test(page, &mut mismatches).await
    })
    .await;
    mismatches.finish(outcome)
}

/// [`run_browser_test`] for a port: the test also gets the [`Mismatches`] to
/// note into, and fails at the end when it noted any.
pub async fn run_port<F>(test: F) -> Result<()>
where
    F: AsyncFnOnce(&mut BaseCase, &mut Mismatches) -> Result<()>,
{
    let mut mismatches = Mismatches::default();
    let outcome = run_browser_test(headless(), async |sb| test(sb, &mut mismatches).await).await;
    mismatches.finish(outcome)
}
