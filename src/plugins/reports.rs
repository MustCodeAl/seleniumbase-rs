//! Plugins that record how each test went.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;

use super::observer::{write_evidence, PageEvidence, TestPlugin};
use crate::core::report_helper::{
    write_html_report_async, write_json_report_async, FailureDetails, TestResult,
};
use crate::error::{Result, SeleniumBaseError};

/// How long a failing page may take to give its URL, title, screenshot or
/// source; a report must not hang on a page that has.
const PAGE_WAIT: Duration = Duration::from_secs(5);

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
///
/// A failed test is reported with its [`FailureDetails`]: the kind of error,
/// the hint, and the page's URL and title when it failed. With
/// [`with_evidence`](Self::with_evidence) the plugin also saves a screenshot
/// and the page source next to the report and links them from it.
#[derive(Debug, Clone)]
pub struct ReportPlugin {
    path: PathBuf,
    format: Format,
    results: Vec<TestResult>,
    evidence: bool,
    /// What `test_failed` learned, for `test_finished` to report.
    pending: Option<(String, FailureDetails)>,
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
            evidence: false,
            pending: None,
        }
    }

    /// Also saves a screenshot and the page source of each failing test in
    /// the report's folder, and links them from the report.
    #[must_use]
    pub fn with_evidence(mut self) -> Self {
        self.evidence = true;
        self
    }

    /// The folder the report is written to, where evidence goes too.
    fn folder(&self) -> &Path {
        self.path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }

    /// Saves one piece of evidence in the report's folder and returns its
    /// name there, or `None` if the page could not give it in time.
    async fn save(&self, test: &str, extension: &str, bytes: Option<Vec<u8>>) -> Option<String> {
        let path = write_evidence(self.folder(), test, extension, &bytes?)
            .await
            .ok()?;
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    /// The results seen so far, in the order the tests finished.
    #[must_use]
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }
}

#[async_trait]
impl TestPlugin for ReportPlugin {
    async fn test_failed(
        &mut self,
        test: &str,
        error: &SeleniumBaseError,
        page: &dyn PageEvidence,
    ) -> Result<()> {
        let mut details = FailureDetails::from_error(error);
        details.url = within(page.current_url()).await;
        details.title = within(page.page_title()).await;
        if self.evidence {
            let png = within(page.screenshot_png()).await;
            details.screenshot = self.save(test, "png", png).await;
            let source = within(page.page_source()).await.map(String::into_bytes);
            details.page_source = self.save(test, "html", source).await;
        }
        self.pending = Some((test.to_owned(), details));
        Ok(())
    }

    async fn test_finished(
        &mut self,
        test: &str,
        elapsed: Duration,
        outcome: &Result<()>,
    ) -> Result<()> {
        let mut result = to_result(test, elapsed, outcome);
        if let Err(error) = outcome {
            result.failure = Some(match self.pending.take() {
                Some((failed, details)) if failed == test => details,
                _ => FailureDetails::from_error(error),
            });
        }
        self.results.push(result);
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
        failure: None,
    }
}

/// What `ask` gives within [`PAGE_WAIT`], or `None`.
async fn within<T>(ask: impl std::future::Future<Output = Result<T>>) -> Option<T> {
    tokio::time::timeout(PAGE_WAIT, ask).await.ok()?.ok()
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
