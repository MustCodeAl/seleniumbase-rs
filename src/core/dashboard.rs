//! The HTML pages made from a run: the report and the dashboard.
//!
//! [`render_html_report`] is one self-contained page with no script: a summary
//! line, a row per test and, for a failed test, the error, how to fix it, the
//! page it ended on, links to its evidence and the technical detail folded
//! away. [`render_dashboard`] is the same table for a run that may still be
//! going: while it is, the page refreshes itself every
//! [`REFRESH_SECONDS`].
//!
//! Everything a test printed is escaped before it reaches the HTML, so a
//! failing assertion that contains markup cannot inject any into the page. A
//! link to evidence is made only from a plain relative path, so a result cannot
//! point a link elsewhere.
//!
//! This is the first form of these pages. The pie chart, the status line and
//! the Python layout of the dashboard come with the renderers lane.

use std::fmt::Write as _;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::core::log_helper::relative_link;
use crate::core::report_helper::{
    html_escape, write_atomically, write_atomically_async, FailureDetails, RunSnapshot, TestResult,
    TestStatus,
};

/// How often a live dashboard refreshes itself, in seconds. Python uses 12.
pub const REFRESH_SECONDS: u32 = 12;

/// How a page is rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageOptions {
    /// The page's title and heading.
    pub title: String,
    /// Refresh the page every [`REFRESH_SECONDS`]. A dashboard also does so
    /// while its run is not finished.
    pub live: bool,
    /// Draw the pie chart of the results.
    pub pie: bool,
    /// The folder the page is written to. The links to a run's files are made
    /// relative to it.
    pub page_dir: PathBuf,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            title: "SeleniumBase Rust Test Report".to_owned(),
            live: false,
            pie: false,
            page_dir: PathBuf::from("."),
        }
    }
}

/// The page's stylesheet. It follows the reader's light or dark preference and
/// does not rely on colour alone: every row also says `PASS`, `FAIL` and so on.
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
tr.fail td.status,tr.unreported td.status{color:var(--fail)}\
tr.skip td.status,tr.untested td.status{color:var(--muted)}\
tr.fail,tr.unreported{background:var(--fail-bg)}\
@media(max-width:40rem){body{padding:.5rem}th,td{padding:.25rem .4rem}}";

/// An HTML page listing the results of `run`, with a summary line on top.
///
/// The page is self-contained: its stylesheet is inline and it needs no
/// script. Test names and messages are escaped.
#[must_use]
pub fn render_html_report(run: &RunSnapshot, options: &PageOptions) -> String {
    page(run, options, options.live)
}

/// An HTML page for a run that may still be going: the same table as the
/// report, which refreshes itself while the run is not finished.
#[must_use]
pub fn render_dashboard(run: &RunSnapshot, options: &PageOptions) -> String {
    page(run, options, options.live || !run.finished)
}

fn page(run: &RunSnapshot, options: &PageOptions, refresh: bool) -> String {
    let summary = run.summary();
    let verdict = if summary.failed == 0 && summary.unreported == 0 {
        "pass"
    } else {
        "fail"
    };
    let title = html_escape(&options.title);
    let mut html = String::with_capacity(2048 + run.results.len() * 192);
    html.push_str(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
    );
    if refresh {
        let _ = writeln!(
            html,
            "<meta http-equiv=\"refresh\" content=\"{REFRESH_SECONDS}\">"
        );
    }
    let _ = write!(html, "<title>{title}</title>\n<style>");
    html.push_str(STYLE);
    let _ = write!(html, "</style>\n</head>\n<body>\n<h1>{title}</h1>\n");
    // Writing to a `String` cannot fail.
    let _ = writeln!(html, "<p class=\"summary {verdict}\">{summary}</p>");
    if run.results.is_empty() {
        html.push_str("<p>No tests were recorded.</p>\n");
    } else {
        html.push_str(
            "<table>\n<thead><tr><th>Test</th><th>Status</th><th>Duration</th>\
             <th>Message</th></tr></thead>\n<tbody>\n",
        );
        for result in &run.results {
            row(&mut html, run, options, result);
        }
        html.push_str("</tbody>\n</table>\n");
    }
    html.push_str("</body>\n</html>\n");
    html
}

fn row(html: &mut String, run: &RunSnapshot, options: &PageOptions, result: &TestResult) {
    let class = match result.status {
        TestStatus::Passed => "pass",
        TestStatus::Failed => "fail",
        TestStatus::Skipped => "skip",
        TestStatus::Untested => "untested",
        TestStatus::Unreported => "unreported",
    };
    let message = match (&result.failure, result.status) {
        (Some(failure), TestStatus::Failed) => failure_cell(&result.message, failure, run, options),
        _ => html_escape(&result.message),
    };
    let _ = writeln!(
        html,
        "<tr class=\"{class}\"><td>{}</td><td class=\"status\">{}</td>\
         <td class=\"num\">{:.3}s</td><td class=\"msg\">{message}</td></tr>",
        html_escape(&result.name),
        result.status.label(),
        result.duration_secs,
    );
}

/// The message cell of a failed test with details: the error, how to fix it,
/// the page, links to the evidence, and the full detail folded away.
/// Everything in it is escaped.
fn failure_cell(
    message: &str,
    failure: &FailureDetails,
    run: &RunSnapshot,
    options: &PageOptions,
) -> String {
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
    .filter_map(|(label, path)| {
        path.as_deref()
            .map(|path| evidence_link(label, path, run, options))
    })
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

/// A link to an evidence file of the run, or its path as text when it is not
/// a plain relative path (a link must not lead elsewhere).
fn evidence_link(label: &str, path: &str, run: &RunSnapshot, options: &PageOptions) -> String {
    let relative = !path.is_empty()
        && !path.contains(':')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if relative {
        let href = relative_link(&options.page_dir, &run.root.join(path));
        format!("<a href=\"{}\">{label}</a>", html_escape(&href))
    } else {
        format!("{label}: {}", html_escape(path))
    }
}

/// Writes the HTML report of `run` to `path`, replacing any file there. The
/// parent directory must exist, and `options.page_dir` should be it.
///
/// The file is written to a temporary neighbour and renamed into place, so a
/// reader never sees half a page.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub fn write_html_report<P: AsRef<Path>>(
    path: P,
    run: &RunSnapshot,
    options: &PageOptions,
) -> io::Result<()> {
    write_atomically(path.as_ref(), render_html_report(run, options).as_bytes())
}

/// Like [`write_html_report`], without blocking the async runtime.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub async fn write_html_report_async(
    path: impl AsRef<Path>,
    run: &RunSnapshot,
    options: &PageOptions,
) -> io::Result<()> {
    write_atomically_async(path.as_ref(), render_html_report(run, options).as_bytes()).await
}

/// Writes the dashboard of `run` to `path`, replacing any file there, as
/// [`write_html_report`] does.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub fn write_dashboard<P: AsRef<Path>>(
    path: P,
    run: &RunSnapshot,
    options: &PageOptions,
) -> io::Result<()> {
    write_atomically(path.as_ref(), render_dashboard(run, options).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SeleniumBaseError;

    fn result(name: &str, status: TestStatus, secs: f64, message: &str) -> TestResult {
        TestResult::new(name, status, secs, message)
    }

    fn failed_with(details: FailureDetails) -> TestResult {
        TestResult {
            failure: Some(details),
            ..result(
                "checkout",
                TestStatus::Failed,
                2.0,
                "element not found: #buy\nmore",
            )
        }
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

    fn html_of(results: Vec<TestResult>) -> String {
        render_html_report(
            &RunSnapshot::from_results("Report", results),
            &PageOptions::default(),
        )
    }

    #[test]
    fn a_failure_shows_the_error_the_fix_the_page_and_the_evidence() {
        let error = SeleniumBaseError::element_not_found("#buy");
        let html = html_of(vec![failed_with(FailureDetails {
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
    fn evidence_is_linked_from_the_folder_the_page_is_in() {
        let mut run = RunSnapshot::from_results(
            "Report",
            vec![failed_with(FailureDetails {
                screenshot: Some("latest_logs/a/screenshot.png".to_owned()),
                ..FailureDetails::default()
            })],
        );
        run.root = PathBuf::from("/run");
        let options = PageOptions {
            page_dir: PathBuf::from("/run/reports"),
            ..PageOptions::default()
        };
        let html = render_html_report(&run, &options);
        assert!(
            html.contains("<a href=\"../latest_logs/a/screenshot.png\">Screenshot</a>"),
            "{html}"
        );
    }

    #[test]
    fn hostile_failure_details_cannot_inject_markup_or_links() {
        let html = html_of(vec![failed_with(FailureDetails {
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
        let html = html_of(vec![result("t", TestStatus::Failed, 0.0, "plain <b>")]);
        assert!(html.contains("<td class=\"msg\">plain &lt;b&gt;</td>"));
        assert!(!html.contains("How to fix"));
    }

    #[test]
    fn the_html_report_has_a_summary_a_row_per_test_and_a_verdict() {
        let html = html_of(mixed());

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
    fn a_skipped_untested_and_unreported_row_each_say_so() {
        let html = html_of(vec![
            result("a", TestStatus::Skipped, 0.0, "no browser"),
            result("b", TestStatus::Untested, 0.0, ""),
            result("c", TestStatus::Unreported, 0.0, ""),
        ]);
        for class in ["skip", "untested", "unreported"] {
            assert!(html.contains(&format!("<tr class=\"{class}\">")), "{class}");
        }
        assert!(
            html.contains(">SKIP<") && html.contains(">UNTESTED<") && html.contains(">UNREPORTED<")
        );
        assert!(
            html.contains("class=\"summary fail\""),
            "an unreported test is not a pass"
        );
    }

    #[test]
    fn an_all_green_run_gets_the_passing_verdict() {
        let html = html_of(vec![result("a", TestStatus::Passed, 0.1, "ok")]);
        assert!(html.contains("class=\"summary pass\""));
        assert!(!html.contains("class=\"summary fail\""));
    }

    #[test]
    fn the_html_report_is_responsive_and_follows_the_color_scheme() {
        let html = html_of(mixed());

        assert!(html.contains("name=\"viewport\""));
        assert!(html.contains("prefers-color-scheme:dark"));
        assert!(html.contains("<html lang=\"en\">"));
    }

    #[test]
    fn the_html_report_needs_no_script_and_escapes_what_a_test_printed() {
        let html = html_of(vec![result(
            "<b>name</b>",
            TestStatus::Failed,
            0.0,
            "<script>alert('x')</script> & \"quotes\"",
        )]);

        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<b>name"), "{html}");
        assert!(html.contains("&lt;b&gt;name&lt;/b&gt;"));
        assert!(html
            .contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;quotes&quot;"));
    }

    #[test]
    fn the_title_is_escaped_too() {
        let run = RunSnapshot::from_results("x", Vec::new());
        let options = PageOptions {
            title: "<i>T</i>".to_owned(),
            ..PageOptions::default()
        };
        let html = render_html_report(&run, &options);
        assert!(html.contains("<h1>&lt;i&gt;T&lt;/i&gt;</h1>"));
        assert!(!html.contains("<i>T"));
    }

    #[test]
    fn an_empty_run_says_so_instead_of_showing_an_empty_table() {
        let html = html_of(Vec::new());

        assert!(html.contains("0 tests"));
        assert!(html.contains("No tests were recorded."));
        assert!(!html.contains("<table>"));
    }

    #[test]
    fn a_dashboard_refreshes_itself_while_its_run_is_not_finished() {
        let mut run = RunSnapshot::from_results("Dash", mixed());
        run.finished = false;
        let live = render_dashboard(&run, &PageOptions::default());
        assert!(live.contains("<meta http-equiv=\"refresh\" content=\"12\">"));
        run.finished = true;
        let done = render_dashboard(&run, &PageOptions::default());
        assert!(!done.contains("http-equiv=\"refresh\""));
        assert!(!html_of(mixed()).contains("http-equiv=\"refresh\""));
    }

    #[test]
    fn written_pages_replace_the_old_file_and_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let html = dir.path().join("report.html");
        let dash = dir.path().join("dashboard.html");
        std::fs::write(&html, "old").unwrap();
        let run = RunSnapshot::from_results("R", mixed());

        write_html_report(&html, &run, &PageOptions::default()).unwrap();
        write_dashboard(&dash, &run, &PageOptions::default()).unwrap();

        assert!(std::fs::read_to_string(&html).unwrap().contains("PASS"));
        assert!(std::fs::read_to_string(&dash).unwrap().contains("PASS"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        assert!(write_html_report(dir.path().join(".."), &run, &PageOptions::default()).is_err());
    }

    #[tokio::test]
    async fn the_async_writer_matches_the_blocking_one() {
        let dir = tempfile::tempdir().unwrap();
        let (html, missing) = (
            dir.path().join("r.html"),
            dir.path().join("no-such-dir").join("r.html"),
        );
        let run = RunSnapshot::from_results("R", mixed());
        let options = PageOptions::default();

        write_html_report_async(&html, &run, &options)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&html).unwrap(),
            render_html_report(&run, &options)
        );
        assert!(write_html_report_async(&missing, &run, &options)
            .await
            .is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
