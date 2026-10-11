//! Test results and the JSON and HTML reports made from them.
//!
//! A [`TestResult`] is one line of a report. [`render_json_report`] and
//! [`render_html_report`] turn a slice of them into text, and the
//! `write_*_report` functions put that text in a file. The files are written
//! whole to a temporary neighbour and renamed into place, so a reader (a CI
//! step tailing the report, say) never sees half a report and an interrupted
//! run leaves the previous one intact.
//!
//! Everything a test printed is escaped before it reaches the HTML, so a
//! failing assertion that contains markup cannot inject any into the report.
//!
//! A failure can carry [`FailureDetails`]: the kind of error, the hint, the
//! page's URL and title, and the screenshot and page source taken when it
//! failed. The HTML report shows them the way a person debugs: the error,
//! how to fix it, the page, links to the evidence, and the technical detail
//! folded away.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::core::report_helper::{render_html_report, FailureDetails, TestResult};
//! use seleniumbase_rs::SeleniumBaseError;
//!
//! let error = SeleniumBaseError::element_not_found("#buy");
//! let results = [TestResult {
//!     name: "login_works".to_owned(),
//!     passed: false,
//!     duration_secs: 1.5,
//!     message: error.to_string(),
//!     failure: Some(FailureDetails::from_error(&error)),
//! }];
//! let html = render_html_report(&results);
//! assert!(html.contains("1 test: 0 passed, 1 failed"));
//! assert!(html.contains("How to fix:"));
//! ```

use std::fmt::{self, Write as _};
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// Result of a single test.
///
/// The field names are the JSON keys of [`write_json_report`] and of the
/// results `sbase report` reads back, so they are part of the report format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    /// The test's name.
    pub name: String,
    /// Whether the test passed.
    pub passed: bool,
    /// How long the test took, in seconds.
    pub duration_secs: f64,
    /// `"ok"` for a pass, or the error for a failure.
    pub message: String,
    /// What is known about a failure beyond its message. Absent from the
    /// JSON when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureDetails>,
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
    /// The failure screenshot, relative to the report's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// The page source saved at the failure, relative to the report's folder.
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

/// How many tests passed and failed, and how long they took altogether.
///
/// Its [`Display`](fmt::Display) form is the summary line at the top of the
/// HTML report: `3 tests: 2 passed, 1 failed in 1.250s`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportSummary {
    /// The number of tests.
    pub total: usize,
    /// The number of tests that passed.
    pub passed: usize,
    /// The number of tests that failed.
    pub failed: usize,
    /// The sum of the tests' durations, in seconds.
    pub duration_secs: f64,
}

impl ReportSummary {
    /// Counts `results`.
    #[must_use]
    pub fn from_results(results: &[TestResult]) -> Self {
        let passed = results.iter().filter(|result| result.passed).count();
        Self {
            total: results.len(),
            passed,
            failed: results.len() - passed,
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
            "{} {noun}: {} passed, {} failed in {:.3}s",
            self.total, self.passed, self.failed, self.duration_secs
        )
    }
}

/// The page's stylesheet. It follows the reader's light or dark preference and
/// does not rely on colour alone: every row also says `PASS` or `FAIL`.
const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#fff;--fg:#1b1f23;--muted:#6a737d;--line:#d8dee4;\
--pass:#1a7f37;--fail:#cf222e;--pass-bg:#e6f4ea;--fail-bg:#fdecee}\
@media(prefers-color-scheme:dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#8b949e;\
--line:#30363d;--pass:#3fb950;--fail:#ff7b72;--pass-bg:#12261a;--fail-bg:#2d1618}}\
body{font:15px/1.5 system-ui,sans-serif;margin:0 auto;max-width:72rem;padding:1rem;\
background:var(--bg);color:var(--fg)}\
h1{font-size:1.4rem;margin:0 0 .5rem}\
.summary{margin:0 0 1rem;padding:.5rem .75rem;border-left:4px solid var(--muted);font-weight:600}\
.summary.pass{border-color:var(--pass);background:var(--pass-bg)}\
.summary.fail{border-color:var(--fail);background:var(--fail-bg)}\
table{border-collapse:collapse;width:100%}\
th,td{border:1px solid var(--line);padding:.35rem .6rem;text-align:left;vertical-align:top}\
th{color:var(--muted);font-weight:600}\
td.num{text-align:right;white-space:nowrap}\
td.msg{white-space:pre-wrap;overflow-wrap:anywhere}\
td.msg .kind{font:600 12px/1.6 ui-monospace,monospace;color:var(--muted)}\
td.msg .error{font-weight:600}\
td.msg .hint,td.msg .page,td.msg .evidence{margin-top:.25rem}\
td.msg details{margin-top:.35rem;color:var(--muted)}\
td.msg pre{margin:.25rem 0 0;white-space:pre-wrap;font:12px/1.5 ui-monospace,monospace}\
td.status{font-weight:700;white-space:nowrap}\
tr.pass td.status{color:var(--pass)}\
tr.fail td.status{color:var(--fail)}\
tr.fail{background:var(--fail-bg)}\
@media(max-width:40rem){body{padding:.5rem}th,td{padding:.25rem .4rem}}";

/// An HTML page listing `results`, with a summary line on top.
///
/// The page is self-contained: its stylesheet is inline and it needs no
/// script. Test names and messages are escaped.
#[must_use]
pub fn render_html_report(results: &[TestResult]) -> String {
    let summary = ReportSummary::from_results(results);
    let verdict = if summary.failed == 0 { "pass" } else { "fail" };
    let mut html = String::with_capacity(2048 + results.len() * 192);
    html.push_str(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>SeleniumBase Rust Test Report</title>\n<style>",
    );
    html.push_str(STYLE);
    html.push_str("</style>\n</head>\n<body>\n<h1>SeleniumBase Rust Test Report</h1>\n");
    // Writing to a `String` cannot fail.
    let _ = writeln!(html, "<p class=\"summary {verdict}\">{summary}</p>");
    if results.is_empty() {
        html.push_str("<p>No tests were recorded.</p>\n");
    } else {
        html.push_str(
            "<table>\n<thead><tr><th>Test</th><th>Status</th><th>Duration</th>\
             <th>Message</th></tr></thead>\n<tbody>\n",
        );
        for result in results {
            let (class, status) = if result.passed {
                ("pass", "PASS")
            } else {
                ("fail", "FAIL")
            };
            let message = match (&result.failure, result.passed) {
                (Some(failure), false) => failure_cell(&result.message, failure),
                _ => html_escape(&result.message),
            };
            let _ = writeln!(
                html,
                "<tr class=\"{class}\"><td>{}</td><td class=\"status\">{status}</td>\
                 <td class=\"num\">{:.3}s</td><td class=\"msg\">{message}</td></tr>",
                html_escape(&result.name),
                result.duration_secs,
            );
        }
        html.push_str("</tbody>\n</table>\n");
    }
    html.push_str("</body>\n</html>\n");
    html
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

/// Writes an HTML report of `results` to `path`, replacing any file there.
///
/// The parent directory must exist. See the [module](self) documentation for
/// how the file is written.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub fn write_html_report<P: AsRef<Path>>(path: P, results: &[TestResult]) -> io::Result<()> {
    write_atomically(path.as_ref(), render_html_report(results).as_bytes())
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

/// Like [`write_html_report`], without blocking the async runtime.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub async fn write_html_report_async(
    path: impl AsRef<Path>,
    results: &[TestResult],
) -> io::Result<()> {
    write_atomically_async(path.as_ref(), render_html_report(results).as_bytes()).await
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

fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary_neighbour(path)?;
    let written =
        std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        // Best effort: the error that matters is the one being returned.
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

async fn write_atomically_async(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

/// The message cell of a failed test with details: the error, how to fix it,
/// the page, links to the evidence, and the full detail folded away.
/// Everything in it is escaped.
fn failure_cell(message: &str, failure: &FailureDetails) -> String {
    let mut cell = String::new();
    if !failure.kind.is_empty() {
        let _ = write!(
            cell,
            "<span class=\"kind\">{}</span>",
            html_escape(&failure.kind)
        );
    }
    let summary = message.lines().next().unwrap_or_default();
    let _ = write!(cell, "<div class=\"error\">{}</div>", html_escape(summary));
    if let Some(hint) = &failure.hint {
        let _ = write!(
            cell,
            "<div class=\"hint\"><b>How to fix:</b> {}</div>",
            html_escape(hint)
        );
    }
    if failure.url.is_some() || failure.title.is_some() {
        cell.push_str("<div class=\"page\"><b>Page:</b> ");
        if let Some(url) = &failure.url {
            let lower = url.to_ascii_lowercase();
            if lower.starts_with("http://") || lower.starts_with("https://") {
                let _ = write!(
                    cell,
                    "<a href=\"{0}\" rel=\"noreferrer\">{0}</a>",
                    html_escape(url)
                );
            } else {
                cell.push_str(&html_escape(url));
            }
        }
        if let Some(title) = &failure.title {
            let _ = write!(cell, " &ldquo;{}&rdquo;", html_escape(title));
        }
        cell.push_str("</div>");
    }
    let evidence: Vec<String> = [
        ("Screenshot", &failure.screenshot),
        ("Page source", &failure.page_source),
    ]
    .into_iter()
    .filter_map(|(label, path)| path.as_deref().map(|path| evidence_link(label, path)))
    .collect();
    if !evidence.is_empty() {
        let _ = write!(
            cell,
            "<div class=\"evidence\"><b>Evidence:</b> {}</div>",
            evidence.join(" &middot; ")
        );
    }
    let detail = failure.detail.as_deref().unwrap_or(message);
    let _ = write!(
        cell,
        "<details><summary>Technical detail</summary><pre>{}</pre></details>",
        html_escape(detail)
    );
    cell
}

/// A link to an evidence file next to the report, or its path as text when
/// it is not a plain relative path (a link must not lead elsewhere).
fn evidence_link(label: &str, path: &str) -> String {
    let relative = !path.is_empty()
        && !path.contains(':')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if relative {
        format!("<a href=\"{}\">{label}</a>", html_escape(path))
    } else {
        format!("{label}: {}", html_escape(path))
    }
}

/// `s` safe to put in HTML text or in a quoted attribute.
fn html_escape(s: &str) -> String {
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

    fn result(name: &str, passed: bool, secs: f64, message: &str) -> TestResult {
        TestResult {
            name: name.to_owned(),
            passed,
            duration_secs: secs,
            message: message.to_owned(),
            failure: None,
        }
    }

    fn failed_with(details: FailureDetails) -> TestResult {
        TestResult {
            failure: Some(details),
            ..result("checkout", false, 2.0, "element not found: #buy\nmore")
        }
    }

    #[test]
    fn a_failure_shows_the_error_the_fix_the_page_and_the_evidence() {
        let error = SeleniumBaseError::element_not_found("#buy");
        let html = render_html_report(&[failed_with(FailureDetails {
            url: Some("https://shop.test/cart".to_owned()),
            title: Some("Cart".to_owned()),
            screenshot: Some("checkout_20261010_101500.png".to_owned()),
            page_source: Some("checkout_20261010_101500.html".to_owned()),
            ..FailureDetails::from_error(&error)
        })]);

        assert!(html.contains("<span class=\"kind\">element_not_found</span>"));
        assert!(html.contains("<div class=\"error\">element not found: #buy</div>"));
        assert!(
            html.contains("<b>How to fix:</b> Nothing on the page matched"),
            "{html}"
        );
        assert!(html.contains(
            "<a href=\"https://shop.test/cart\" rel=\"noreferrer\">https://shop.test/cart</a> &ldquo;Cart&rdquo;"
        ));
        assert!(html.contains("<a href=\"checkout_20261010_101500.png\">Screenshot</a>"));
        assert!(html.contains("<a href=\"checkout_20261010_101500.html\">Page source</a>"));
        assert!(html.contains("<details><summary>Technical detail</summary><pre>"));
        assert!(!html.contains("<script"), "the report needs no script");
    }

    #[test]
    fn hostile_failure_details_cannot_inject_markup_or_links() {
        let html = render_html_report(&[failed_with(FailureDetails {
            kind: "<b>kind</b>".to_owned(),
            hint: Some("<script>alert(1)</script>".to_owned()),
            url: Some("javascript:alert(1)".to_owned()),
            title: Some("\"><img src=x onerror=alert(1)>".to_owned()),
            screenshot: Some("../../etc/passwd".to_owned()),
            page_source: Some("\" onmouseover=\"alert(1)".to_owned()),
            detail: Some("</pre><script>alert(2)</script>".to_owned()),
        })]);

        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(!html.contains("<b>kind"), "{html}");
        assert!(!html.contains("href=\"javascript"), "{html}");
        assert!(
            !html.contains("href=\"../"),
            "a path that climbs out is not linked"
        );
        assert!(!html.contains("\" onmouseover"), "{html}");
        assert!(html.contains("Screenshot: ../../etc/passwd"));
    }

    #[test]
    fn a_failure_without_details_keeps_the_plain_message() {
        let html = render_html_report(&[result("t", false, 0.0, "plain <b>")]);
        assert!(html.contains("<td class=\"msg\">plain &lt;b&gt;</td>"));
        assert!(!html.contains("How to fix"));
    }

    #[test]
    fn failure_details_travel_in_the_json_report() {
        let error = SeleniumBaseError::assertion_failed("title");
        let rows = vec![failed_with(FailureDetails {
            url: Some("https://a.test/".to_owned()),
            ..FailureDetails::from_error(&error)
        })];

        let json = render_json_report(&rows).unwrap();
        let back: Vec<TestResult> = serde_json::from_str(&json).unwrap();

        assert_eq!(back, rows);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value[0]["failure"]["kind"], "assertion_failed");
        assert!(
            value[0]["failure"].get("screenshot").is_none(),
            "absent, not null"
        );
    }

    fn mixed() -> Vec<TestResult> {
        vec![
            result("login", true, 0.5, "ok"),
            result("checkout", false, 1.25, "expected 'a' but got \"b\""),
            result("search", true, 0.25, "ok"),
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
    fn the_summary_says_one_test_not_one_tests() {
        let summary = ReportSummary::from_results(&[result("a", true, 0.0, "ok")]);
        assert_eq!(summary.to_string(), "1 test: 1 passed, 0 failed in 0.000s");
        let none = ReportSummary::from_results(&[]);
        assert_eq!(none.to_string(), "0 tests: 0 passed, 0 failed in 0.000s");
    }

    #[test]
    fn the_html_report_has_a_summary_a_row_per_test_and_a_verdict() {
        let html = render_html_report(&mixed());

        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(
            html.contains("3 tests: 2 passed, 1 failed in 2.000s"),
            "{html}"
        );
        assert!(html.contains("class=\"summary fail\""));
        assert_eq!(html.matches("<tr class=\"pass\">").count(), 2);
        assert_eq!(html.matches("<tr class=\"fail\">").count(), 1);
        assert!(html.contains(">PASS<") && html.contains(">FAIL<"));
        assert!(html.contains("<td class=\"num\">1.250s</td>"));
    }

    #[test]
    fn an_all_green_run_gets_the_passing_verdict() {
        let html = render_html_report(&[result("a", true, 0.1, "ok")]);
        assert!(html.contains("class=\"summary pass\""));
        assert!(!html.contains("class=\"summary fail\""));
    }

    #[test]
    fn the_html_report_is_responsive_and_follows_the_color_scheme() {
        let html = render_html_report(&mixed());

        assert!(html.contains("name=\"viewport\""));
        assert!(html.contains("prefers-color-scheme:dark"));
        assert!(html.contains("<html lang=\"en\">"));
    }

    #[test]
    fn the_html_report_needs_no_script_and_escapes_what_a_test_printed() {
        let hostile = [result(
            "<b>name</b>",
            false,
            0.0,
            "<script>alert('x')</script> & \"quotes\"",
        )];

        let html = render_html_report(&hostile);

        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<b>name"), "{html}");
        assert!(html.contains("&lt;b&gt;name&lt;/b&gt;"));
        assert!(html
            .contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;quotes&quot;"));
    }

    #[test]
    fn an_empty_run_says_so_instead_of_showing_an_empty_table() {
        let html = render_html_report(&[]);

        assert!(html.contains("0 tests"));
        assert!(html.contains("No tests were recorded."));
        assert!(!html.contains("<table>"));
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
        assert_eq!(keys, ["duration_secs", "message", "name", "passed"]);
        assert_eq!(rows[1]["name"], "checkout");
        assert_eq!(rows[1]["passed"], false);
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
        let html = dir.path().join("report.html");
        let json = dir.path().join("report.json");
        std::fs::write(&html, "old").unwrap();

        write_html_report(&html, &mixed()).unwrap();
        write_json_report(&json, &mixed()).unwrap();
        write_json_report(&json, &mixed()[..1]).unwrap();

        assert!(std::fs::read_to_string(&html).unwrap().contains("PASS"));
        let rows: Vec<TestResult> =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(rows.len(), 1, "the second write replaced the first");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_report_that_cannot_be_written_is_an_error_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("no-such-dir").join("report.json");

        assert!(write_json_report(&missing_parent, &mixed()).is_err());
        assert!(write_html_report(dir.path().join(".."), &mixed()).is_err());
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
    async fn the_async_writers_match_the_blocking_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (html, json) = (dir.path().join("r.html"), dir.path().join("r.json"));

        write_html_report_async(&html, &mixed()).await.unwrap();
        write_json_report_async(&json, &mixed()).await.unwrap();

        assert_eq!(
            std::fs::read_to_string(&html).unwrap(),
            render_html_report(&mixed())
        );
        assert_eq!(
            std::fs::read_to_string(&json).unwrap(),
            render_json_report(&mixed()).unwrap()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn an_async_report_that_cannot_be_written_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("no-such-dir").join("report.html");

        assert!(write_html_report_async(&missing_parent, &mixed())
            .await
            .is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
