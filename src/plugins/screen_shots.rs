//! Saves a screenshot when a test fails.

use std::path::PathBuf;

use async_trait::async_trait;

use super::observer::{write_evidence, PageEvidence, TestPlugin};
use crate::error::{Result, SeleniumBaseError};

/// Saves a PNG screenshot of the page into a directory when a test fails.
///
/// The file is named after the test and the time, for example
/// `login_works_20261009_051500.png`, and an existing file is never replaced.
/// If the browser cannot take the screenshot, no file is written; a file that
/// only pretends to be a screenshot would be worse than none.
#[derive(Debug, Clone)]
pub struct ScreenshotOnFailurePlugin {
    /// Where screenshots are saved.
    pub output_dir: PathBuf,
}

impl ScreenshotOnFailurePlugin {
    /// Saves screenshots into `output_dir`, creating it if needed.
    #[must_use]
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }
}

#[async_trait]
impl TestPlugin for ScreenshotOnFailurePlugin {
    async fn test_failed(
        &mut self,
        test: &str,
        _error: &SeleniumBaseError,
        page: &dyn PageEvidence,
    ) -> Result<()> {
        let png = page.screenshot_png().await?;
        let path = write_evidence(&self.output_dir, test, "png", &png).await?;
        tracing::info!(test, path = %path.display(), "saved a failure screenshot");
        Ok(())
    }
}
