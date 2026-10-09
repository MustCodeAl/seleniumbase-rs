//! Plugins that record how each test went.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use super::observer::TestPlugin;
use crate::core::report_helper::{write_html_report_async, write_json_report_async, TestResult};
use crate::error::{Result, SeleniumBaseError};

#[derive(Debug, Clone, Copy)]
enum Format {
    Json,
    Html,
}

/// Writes a JSON or HTML report of every test it has seen.
///
/// The file is rewritten after each test, so it is complete and valid even if
/// the run is interrupted later. Results are kept in memory for the life of the
/// plugin, so use one plugin per run.
#[derive(Debug, Clone)]
pub struct ReportPlugin {
    path: PathBuf,
    format: Format,
    results: Vec<TestResult>,
}

impl ReportPlugin {
    /// A JSON report at `path`.
    #[must_use]
    pub fn json(path: impl Into<PathBuf>) -> Self {
        Self::new(path.into(), Format::Json)
    }

    /// An HTML report at `path`.
    #[must_use]
    pub fn html(path: impl Into<PathBuf>) -> Self {
        Self::new(path.into(), Format::Html)
    }

    fn new(path: PathBuf, format: Format) -> Self {
        Self {
            path,
            format,
            results: Vec::new(),
        }
    }

    /// The results seen so far, in the order the tests finished.
    #[must_use]
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }
}

#[async_trait]
impl TestPlugin for ReportPlugin {
    async fn test_finished(
        &mut self,
        test: &str,
        elapsed: Duration,
        outcome: &Result<()>,
    ) -> Result<()> {
        self.results.push(to_result(test, elapsed, outcome));
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        match self.format {
            Format::Json => write_json_report_async(&self.path, &self.results).await,
            Format::Html => write_html_report_async(&self.path, &self.results).await,
        }
        .map_err(SeleniumBaseError::from)
    }
}

fn to_result(test: &str, elapsed: Duration, outcome: &Result<()>) -> TestResult {
    TestResult {
        name: test.to_owned(),
        passed: outcome.is_ok(),
        duration_secs: elapsed.as_secs_f64(),
        message: match outcome {
            Ok(()) => "ok".to_owned(),
            Err(error) => error.to_string(),
        },
    }
}

/// Records every test in a [`ResultStore`](crate::storage::ResultStore), for
/// trends across runs and `sbase report`.
#[cfg(feature = "turso")]
#[derive(Debug, Clone)]
pub struct ResultStorePlugin {
    store: crate::storage::ResultStore,
    run: crate::storage::RunId,
}

#[cfg(feature = "turso")]
impl ResultStorePlugin {
    /// Records into `run`, which must have been started in `store`.
    #[must_use]
    pub fn new(store: crate::storage::ResultStore, run: crate::storage::RunId) -> Self {
        Self { store, run }
    }
}

#[cfg(feature = "turso")]
#[async_trait]
impl TestPlugin for ResultStorePlugin {
    async fn test_finished(
        &mut self,
        test: &str,
        elapsed: Duration,
        outcome: &Result<()>,
    ) -> Result<()> {
        self.store
            .record_outcome(self.run, test, elapsed, outcome)
            .await
    }
}
