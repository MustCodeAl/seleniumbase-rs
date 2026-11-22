//! The WebDriver browser of one Gherkin scenario.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use super::session::{Launch, Scenario};
use super::{plan_for, soft_limit, steps};
use crate::config::settings::Settings;
use crate::error::{Result, SeleniumBaseError};
use crate::BaseCase;

/// The browser of a Gherkin scenario run over WebDriver: a [`BaseCase`] that
/// the steps use, and that is closed when the scenario ends.
///
/// Get it from the [`sb`](super::sb) fixture, or launch one with [`launch`](Self::launch).
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

    /// The case the steps run on.
    #[must_use]
    pub fn case(&self) -> &BaseCase {
        self.scenario.case()
    }

    /// The mutable case the steps run on.
    pub fn case_mut(&mut self) -> &mut BaseCase {
        self.scenario.case_mut()
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
