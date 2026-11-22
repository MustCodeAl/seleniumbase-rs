//! The pure CDP browser of one Gherkin scenario.

use std::path::PathBuf;
use std::sync::Arc;

use super::session::{Launch, Scenario};
use super::{plan_for, Finalizer, Plan};
use crate::api::runner::CdpCase;
use crate::config::settings::Settings;
use crate::config::test_settings::TestData;
use crate::error::Result;
use crate::sb_cdp::{Browser, Page};

/// The browser of a Gherkin scenario run over pure CDP: its first [`Page`],
/// which the steps use, and the browser, closed when the scenario ends.
///
/// Get it from the [`sb_cdp`](super::sb_cdp) fixture, or launch one with
/// [`launch`](Self::launch).
pub struct SbCdpCase {
    scenario: Scenario<CdpCase>,
    test_id: String,
    data: TestData,
}

impl SbCdpCase {
    /// As [`SbCase::launch`](super::SbCase::launch), over CDP.
    ///
    /// # Errors
    ///
    /// Returns the settings' error or the browser's launch error.
    pub async fn launch() -> Result<Self> {
        Self::launch_with(Settings::load()?).await
    }

    /// As [`SbCase::launch_with`](super::SbCase::launch_with), over CDP.
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

    /// The id of the test; see [`SbCase::test_id`](super::SbCase::test_id).
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

    /// The CDP page the steps run on.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.scenario.case().page
    }
}

impl std::fmt::Debug for SbCdpCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SbCdpCase")
            .field("test_id", &self.test_id)
            .finish_non_exhaustive()
    }
}
