//! Test results and the JSON report made from them.
//!
//! A [`TestResult`] is one line of a report: a test, its [`TestStatus`], how
//! long it took and what it said. A [`RunSnapshot`] is a whole run at one
//! moment, the input of the renderers in [`dashboard`](super::dashboard) and
//! [`junit`](super::junit). [`render_json_report`] turns a slice of results
//! into JSON, and the `write_*_report` functions put the text in a file. The
//! files are written whole to a temporary neighbour and renamed into place, so
//! a reader (a CI step tailing the report, say) never sees half a report and an
//! interrupted run leaves the previous one intact.
//!
//! A failure can carry [`FailureDetails`]: the kind of error, the hint, the
//! page's URL and title, and the screenshot and page source taken when it
//! failed. The paths in them are relative to the run's folder.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::core::dashboard::{render_html_report, PageOptions};
//! use seleniumbase_rs::core::report_helper::{
//!     FailureDetails, RunSnapshot, TestResult, TestStatus,
//! };
//! use seleniumbase_rs::SeleniumBaseError;
//!
//! let error = SeleniumBaseError::element_not_found("#buy");
//! let mut login = TestResult::new("login_works", TestStatus::Failed, 1.5, error.to_string());
//! login.failure = Some(FailureDetails::from_error(&error));
//! let run = RunSnapshot::from_results("Report", vec![login]);
//! let html = render_html_report(&run, &PageOptions::default());
//! assert!(html.contains("1 test: 0 passed, 1 failed"));
//! assert!(html.contains("How to fix:"));
//! ```

use std::fmt::{self, Write as _};
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// How a test ended, or has not ended yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    /// The test passed.
    Passed,
    /// The test failed: an error, a panic, or a time limit that ran out.
    Failed,
    /// The test skipped itself; its message is the reason.
    Skipped,
    /// The test started and has not finished.
    Untested,
    /// The test started and its process ended without reporting a result: the
    /// run was killed, or the test never reached its end.
    Unreported,
}

impl TestStatus {
    /// The lower-case word, as it is written in the JSON report.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Untested => "untested",
            Self::Unreported => "unreported",
        }
    }

    /// The upper-case word a report prints, so a status never rests on colour
    /// alone.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "PASS",
            Self::Failed => "FAIL",
            Self::Skipped => "SKIP",
            Self::Untested => "UNTESTED",
            Self::Unreported => "UNREPORTED",
        }
    }
}

impl fmt::Display for TestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Result of a single test.
///
/// The field names are the JSON keys of [`write_json_report`] and of the
/// results `sbrs report` reads back, so they are part of the report format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    /// The test's display id, such as `login_chrome::tests::test_login`.
    pub name: String,
    /// How the test ended.
    pub status: TestStatus,
    /// How long the test took, in seconds.
    pub duration_secs: f64,
    /// `"ok"` for a pass, the error for a failure, the reason for a skip.
    pub message: String,
    /// What is known about a failure beyond its message. Absent from the
    /// JSON when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureDetails>,
    /// The test's folder of logs, such as `latest_logs/login.valid`, relative
    /// to the run's folder. Absent when the test left no logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_dir: Option<String>,
}

impl TestResult {
    /// A result with no failure details and no logs.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        status: TestStatus,
        duration_secs: f64,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status,
            duration_secs,
            message: message.into(),
            failure: None,
            log_dir: None,
        }
    }

    /// Whether the test counts as a success: it passed or it was skipped.
    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self.status, TestStatus::Passed | TestStatus::Skipped)
    }
}

/// What a person needs to debug a failed test.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureDetails {
    /// The error's category, such as `element_not_found`; see
    /// [`SeleniumBaseError::category`].
    pub kind: String,
    /// The likely cause and the fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// The page's URL when the test failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The page's title when the test failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The failure screenshot, relative to the run's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// The page source saved at the failure, relative to the run's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_source: Option<String>,
    /// The full error with each of its causes, one per line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl FailureDetails {
    /// The kind, hint and detail of `error`; the page facts are left for the
    /// caller to fill in.
    #[must_use]
    pub fn from_error(error: &SeleniumBaseError) -> Self {
        let mut detail = error.to_string();
        let mut cause = std::error::Error::source(error);
        while let Some(inner) = cause {
            let text = inner.to_string();
            if !detail.contains(&text) {
                let _ = write!(detail, "\ncaused by: {text}");
            }
            cause = inner.source();
        }
        Self {
            kind: error.category().to_owned(),
            hint: error.hint(),
            detail: Some(detail),
            ..Self::default()
        }
    }
}

/// How many tests passed, failed and so on, and how long they took altogether.
///
/// Its [`Display`](fmt::Display) form is the summary line at the top of the
/// HTML report: `3 tests: 2 passed, 1 failed in 1.250s`. Skipped, untested and
/// unreported tests are named only when there are some.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportSummary {
    /// The number of tests.
    pub total: usize,
    /// The number of tests that passed.
    pub passed: usize,
    /// The number of tests that failed.
    pub failed: usize,
    /// The number of tests that skipped themselves.
    pub skipped: usize,
    /// The number of tests that have not finished.
    pub untested: usize,
    /// The number of tests that never reported a result.
    pub unreported: usize,
    /// The sum of the tests' durations, in seconds.
    pub duration_secs: f64,
}

impl ReportSummary {
    /// Counts `results`.
    #[must_use]
    pub fn from_results(results: &[TestResult]) -> Self {
        let count = |status: TestStatus| results.iter().filter(|r| r.status == status).count();
        Self {
            total: results.len(),
            passed: count(TestStatus::Passed),
            failed: count(TestStatus::Failed),
            skipped: count(TestStatus::Skipped),
            untested: count(TestStatus::Untested),
            unreported: count(TestStatus::Unreported),
            // Not `sum()`: an empty float sum is -0.0, which would print as "-0.000s".
            duration_secs: results
                .iter()
                .fold(0.0, |total, result| total + result.duration_secs),
        }
    }
}

impl fmt::Display for ReportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noun = if self.total == 1 { "test" } else { "tests" };
        write!(
            f,
            "{} {noun}: {} passed, {} failed",
            self.total, self.passed, self.failed
        )?;
        for (count, word) in [
            (self.skipped, "skipped"),
            (self.untested, "untested"),
            (self.unreported, "unreported"),
        ] {
            if count > 0 {
                write!(f, ", {count} {word}")?;
            }
        }
        write!(f, " in {:.3}s", self.duration_secs)
    }
}

/// A run at one moment: its results in the order the tests started, and where
/// they were taken from.
///
/// [`TestRun::snapshot`](super::test_run::TestRun::snapshot) makes one from the
/// files of a run; [`from_results`](Self::from_results) makes one from results
/// you already hold.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSnapshot {
    /// The title of the report or dashboard.
    pub title: String,
    /// When the run started.
    pub started_at: DateTime<Local>,
    /// When the run last changed.
    pub updated_at: DateTime<Local>,
    /// Whether the run is over: no result is going to be added.
    pub finished: bool,
    /// One result per test.
    pub results: Vec<TestResult>,
    /// The run's folder, which the paths in the results are relative to.
    pub root: PathBuf,
}

impl RunSnapshot {
    /// A finished run of `results`, taken now, whose folder is the current one.
    #[must_use]
    pub fn from_results(title: impl Into<String>, results: Vec<TestResult>) -> Self {
        let now = Local::now();
        Self {
            title: title.into(),
            started_at: now,
            updated_at: now,
            finished: true,
            results,
            root: PathBuf::from("."),
        }
    }

    /// The counts and the total duration of the results.
    #[must_use]
    pub fn summary(&self) -> ReportSummary {
        ReportSummary::from_results(&self.results)
    }
}

/// `results` as pretty-printed JSON: an array with one object per test, keyed
/// by [`TestResult`]'s field names.
///
/// # Errors
///
/// Returns an error if serialization fails, which for these types means a
/// duration that is not a finite number.
pub fn render_json_report(results: &[TestResult]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(results)
}

/// Writes a JSON report of `results` to `path`, replacing any file there.
///
/// The parent directory must exist. See the [module](self) documentation for
/// how the file is written.
///
/// # Errors
///
/// Returns the I/O error if the results cannot be serialized or the file
/// cannot be written.
pub fn write_json_report<P: AsRef<Path>>(path: P, results: &[TestResult]) -> io::Result<()> {
    write_atomically(path.as_ref(), render_json_report(results)?.as_bytes())
}

/// Like [`write_json_report`], without blocking the async runtime.
///
/// # Errors
///
/// Returns the I/O error if the results cannot be serialized or the file
/// cannot be written.
pub async fn write_json_report_async(
    path: impl AsRef<Path>,
    results: &[TestResult],
) -> io::Result<()> {
    write_atomically_async(path.as_ref(), render_json_report(results)?.as_bytes()).await
}

/// A path beside `path` to write to before renaming over it.
fn temporary_neighbour(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} does not name a file", path.display()),
        )
    })?;
    let mut temporary = std::ffi::OsString::from(".");
    temporary.push(name);
    temporary.push(format!(".{}.tmp", std::process::id()));
    Ok(path.with_file_name(temporary))
}

/// Writes `bytes` to `path` through a temporary neighbour and a rename.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary_neighbour(path)?;
    let written =
        std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        // Best effort: the error that matters is the one being returned.
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// Like [`write_atomically`], without blocking the async runtime.
pub(crate) async fn write_atomically_async(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary_neighbour(path)?;
    let written = match tokio::fs::write(&temporary, bytes).await {
        Ok(()) => tokio::fs::rename(&temporary, path).await,
        Err(error) => Err(error),
    };
    if written.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    written
}

/// `s` safe to put in HTML text or in a quoted attribute.
pub(crate) fn html_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(name: &str, status: TestStatus, secs: f64, message: &str) -> TestResult {
        TestResult::new(name, status, secs, message)
    }

    fn mixed() -> Vec<TestResult> {
        vec![
            result("login", TestStatus::Passed, 0.5, "ok"),
            result(
                "checkout",
                TestStatus::Failed,
                1.25,
                "expected 'a' but got \"b\"",
            ),
            result("search", TestStatus::Passed, 0.25, "ok"),
        ]
    }

    #[test]
    fn markup_characters_are_escaped_including_both_quotes() {
        assert_eq!(
            html_escape(r#"<a href="x" onclick='y'>&"#),
            "&lt;a href=&quot;x&quot; onclick=&#39;y&#39;&gt;&amp;"
        );
        assert_eq!(
            html_escape("plain, ünïcode, 日本語"),
            "plain, ünïcode, 日本語"
        );
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn the_summary_counts_and_adds_up_durations() {
        let summary = ReportSummary::from_results(&mixed());

        assert_eq!((summary.total, summary.passed, summary.failed), (3, 2, 1));
        assert!((summary.duration_secs - 2.0).abs() < 1e-9);
        assert_eq!(summary.to_string(), "3 tests: 2 passed, 1 failed in 2.000s");
    }

    #[test]
    fn the_summary_names_skipped_untested_and_unreported_tests_only_when_there_are_some() {
        let mut results = mixed();
        results.push(result("later", TestStatus::Skipped, 0.0, "no browser"));
        assert_eq!(
            ReportSummary::from_results(&results).to_string(),
            "4 tests: 2 passed, 1 failed, 1 skipped in 2.000s"
        );
        results.push(result("running", TestStatus::Untested, 0.0, ""));
        results.push(result("killed", TestStatus::Unreported, 0.0, ""));
        let summary = ReportSummary::from_results(&results);
        assert_eq!(
            (summary.skipped, summary.untested, summary.unreported),
            (1, 1, 1)
        );
        assert_eq!(
            summary.to_string(),
            "6 tests: 2 passed, 1 failed, 1 skipped, 1 untested, 1 unreported in 2.000s"
        );
    }

    #[test]
    fn the_summary_says_one_test_not_one_tests() {
        let summary = ReportSummary::from_results(&[result("a", TestStatus::Passed, 0.0, "ok")]);
        assert_eq!(summary.to_string(), "1 test: 1 passed, 0 failed in 0.000s");
        let none = ReportSummary::from_results(&[]);
        assert_eq!(none.to_string(), "0 tests: 0 passed, 0 failed in 0.000s");
    }

    #[test]
    fn a_skipped_test_counts_as_a_success_and_an_untested_one_does_not() {
        assert!(result("a", TestStatus::Passed, 0.0, "").passed());
        assert!(result("a", TestStatus::Skipped, 0.0, "").passed());
        for status in [
            TestStatus::Failed,
            TestStatus::Untested,
            TestStatus::Unreported,
        ] {
            assert!(!result("a", status, 0.0, "").passed(), "{status}");
        }
    }

    #[test]
    fn a_status_is_a_lower_case_word_in_json_and_an_upper_case_one_on_a_page() {
        assert_eq!(
            serde_json::to_string(&TestStatus::Unreported).unwrap(),
            "\"unreported\""
        );
        assert_eq!(TestStatus::Passed.to_string(), "passed");
        assert_eq!(TestStatus::Passed.label(), "PASS");
        assert_eq!(TestStatus::Failed.label(), "FAIL");
    }

    #[test]
    fn a_snapshot_of_results_is_finished_and_counts_them() {
        let run = RunSnapshot::from_results("Report", mixed());
        assert!(run.finished);
        assert_eq!(run.title, "Report");
        assert_eq!(run.summary().total, 3);
    }

    #[test]
    fn failure_details_travel_in_the_json_report() {
        let error = SeleniumBaseError::assertion_failed("title");
        let rows = vec![TestResult {
            failure: Some(FailureDetails {
                url: Some("https://a.test/".to_owned()),
                ..FailureDetails::from_error(&error)
            }),
            log_dir: Some("latest_logs/checkout".to_owned()),
            ..result(
                "checkout",
                TestStatus::Failed,
                2.0,
                "assertion failed: title",
            )
        }];

        let json = render_json_report(&rows).unwrap();
        let back: Vec<TestResult> = serde_json::from_str(&json).unwrap();

        assert_eq!(back, rows);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value[0]["failure"]["kind"], "assertion_failed");
        assert_eq!(value[0]["log_dir"], "latest_logs/checkout");
        assert!(
            value[0]["failure"].get("screenshot").is_none(),
            "absent, not null"
        );
    }

    #[test]
    fn the_json_report_keeps_its_shape() {
        let json = render_json_report(&mixed()).unwrap();

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = value.as_array().unwrap();
        assert_eq!(rows.len(), 3);
        let mut keys: Vec<&str> = rows[1]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["duration_secs", "message", "name", "status"]);
        assert_eq!(rows[1]["name"], "checkout");
        assert_eq!(rows[1]["status"], "failed");
        let back: Vec<TestResult> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, mixed());
    }

    #[test]
    fn an_empty_json_report_is_an_empty_array() {
        assert_eq!(render_json_report(&[]).unwrap(), "[]");
    }

    #[test]
    fn written_reports_replace_the_old_file_and_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join("report.json");
        std::fs::write(&json, "old").unwrap();

        write_json_report(&json, &mixed()).unwrap();
        write_json_report(&json, &mixed()[..1]).unwrap();

        let rows: Vec<TestResult> =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(rows.len(), 1, "the second write replaced the first");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_report_that_cannot_be_written_is_an_error_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("no-such-dir").join("report.json");

        assert!(write_json_report(&missing_parent, &mixed()).is_err());
        assert!(write_atomically(&dir.path().join(".."), b"x").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_path_with_no_file_name_is_refused() {
        let error = write_json_report("/", &mixed()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("does not name a file"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn the_async_writer_matches_the_blocking_one() {
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join("r.json");

        write_json_report_async(&json, &mixed()).await.unwrap();

        assert_eq!(
            std::fs::read_to_string(&json).unwrap(),
            render_json_report(&mixed()).unwrap()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn an_async_report_that_cannot_be_written_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("no-such-dir").join("report.json");

        assert!(write_json_report_async(&missing_parent, &mixed())
            .await
            .is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
