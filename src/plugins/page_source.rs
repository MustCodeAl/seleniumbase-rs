//! Saves the page's HTML when a test fails.

use std::path::PathBuf;

use async_trait::async_trait;

use super::observer::{write_evidence, PageEvidence, TestPlugin};
use crate::error::{Result, SeleniumBaseError};

/// Saves the page's HTML into a directory when a test fails.
///
/// The file is named after the test and the time, for example
/// `login_works_20261009_051500.html`, and an existing file is never replaced.
#[derive(Debug, Clone)]
pub struct PageSourceOnFailurePlugin {
    /// Where page sources are saved.
    pub output_dir: PathBuf,
}

impl PageSourceOnFailurePlugin {
    /// Saves page sources into `output_dir`, creating it if needed.
    #[must_use]
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }
}

#[async_trait]
impl TestPlugin for PageSourceOnFailurePlugin {
    async fn test_failed(
        &mut self,
        test: &str,
        _error: &SeleniumBaseError,
        page: &dyn PageEvidence,
    ) -> Result<()> {
        let html = page.page_source().await?;
        let path = write_evidence(&self.output_dir, test, "html", html.as_bytes()).await?;
        tracing::info!(test, path = %path.display(), "saved the failing page's source");
        Ok(())
    }
}
