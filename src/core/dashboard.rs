//! The HTML pages made from a run: the dashboard and the report.
//!
//! [`render_dashboard`] follows the layout of Python SeleniumBase's
//! `dashboard.html`: a pie chart of the results, a row of counts
//! (`Failed | Skipped | Passed | Untested | Total`), a table of the tests, the
//! time of the last update and a status line. While the run is not finished
//! the page refreshes itself every [`REFRESH_SECONDS`] and says it is awaiting
//! results; when the run finishes the same file becomes the final report: no
//! refresh, a closing status line, and every test that never reported is shown
//! as `UNREPORTED`.
//!
//! [`render_html_report`] is one self-contained page for a finished run: a
//! summary line, the same table, and the pie when [`PageOptions::pie`] asks for
//! it. [`write_html_report`] also keeps a copy of a finished report in
//! `last_report.html` in the run's folder.
//!
//! The table has the columns of Python's: `Result | Test | Duration | Links`,
//! plus `Details`: the error of a failed test, how to fix it, the page it ended
//! on, links to its evidence and the technical detail folded away. The links
//! lead to `latest_logs/` (`Logs`) and to the test's own folder in it (`Data`),
//! made relative to the folder the page is written to.
//!
//! The pages need no script and no network: the pie is inline SVG
//! ([`pie_chart_svg`]) and the only moving part is the `<meta>` refresh. They
//! follow the reader's light or dark preference, and every status is also a
//! word (`PASS`, `FAIL`, ...), never colour alone.
//!
//! Everything a test printed is escaped before it reaches the HTML, so a
//! failing assertion that contains markup cannot inject any into the page. A
//! link to a run's files is made only from a plain relative path, so a result
//! cannot point a link elsewhere.

use std::borrow::Cow;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::fmt::Write as _;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::artifacts::LATEST_LOGS_DIR;
use crate::core::log_helper::{get_master_time, relative_link};
use crate::core::report_helper::{
    html_escape, write_atomically, write_atomically_async, FailureDetails, ReportSummary,
    RunSnapshot, TestResult, TestStatus,
};

/// How often a live dashboard refreshes itself, in seconds. Python uses 12.
pub const REFRESH_SECONDS: u32 = 12;

/// The name of the copy of the last finished report that
/// [`write_html_report`] keeps in the run's folder.
pub const LAST_REPORT_FILE: &str = "last_report.html";

/// The slice colours of the pie. The first four are Python's.
const PASSED_COLOR: &str = "#84d474";
const UNTESTED_COLOR: &str = "#eaeaea";
const SKIPPED_COLOR: &str = "#efd8b4";
const FAILED_COLOR: &str = "#f17476";
const UNREPORTED_COLOR: &str = "#c9c9c9";

/// The size of the pie's drawing, and where its pie and legend sit in it.
const PIE_WIDTH: u32 = 420;
const PIE_HEIGHT: u32 = 250;
const PIE_CENTER: (f64, f64) = (100.0, 140.0);
const PIE_RADIUS: f64 = 80.0;
const LEGEND_X: u32 = 200;
const LEGEND_TOP: u32 = 80;
const LEGEND_STEP: u32 = 28;

/// How a page is rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageOptions {
    /// The page's title and heading.
    pub title: String,
    /// Refresh the page every [`REFRESH_SECONDS`]. A dashboard also does so
    /// while its run is not finished.
    pub live: bool,
    /// Draw the pie chart of the results in the report. The dashboard always
    /// draws it.
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
--pass:#1a7f37;--fail:#cf222e;--skip:#9a6700;--total:#575792;\
--pass-bg:#e6f4ea;--fail-bg:#fdecee;--skip-bg:#fff8e5}\
@media(prefers-color-scheme:dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#8b949e;\
--line:#30363d;--pass:#3fb950;--fail:#ff7b72;--skip:#d29922;--total:#a5a5ff;\
--pass-bg:#12261a;--fail-bg:#2d1618;--skip-bg:#2a2412}}\
body{font:15px/1.5 system-ui,sans-serif;margin:0 auto;max-width:72rem;padding:1rem;\
background:var(--bg);color:var(--fg)}\
h1{font-size:1.4rem;margin:0 0 .5rem}\
.sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);\
white-space:nowrap}\
.summary{margin:0 0 .5rem;padding:.5rem .75rem;border-left:4px solid var(--muted);font-weight:600}\
.summary.pass{border-color:var(--pass);background:var(--pass-bg)}\
.summary.fail{border-color:var(--fail);background:var(--fail-bg)}\
.run-count{margin:0 0 1rem;color:var(--muted)}\
.pie{margin:0 auto 1rem;text-align:center;color:var(--fg)}\
.pie svg{width:100%;max-width:30rem;height:auto}\
.pie text{fill:currentColor;font:14px system-ui,sans-serif}\
.pie .pie-title{font-size:16px;font-weight:600}\
.pie path,.pie circle{stroke:var(--bg);stroke-width:1.5}\
.pie circle.empty{fill:none;stroke:var(--muted);stroke-dasharray:4 4}\
table{border-collapse:collapse;width:100%}\
table.counts{width:auto;margin:0 auto 1rem}\
table.counts th{text-align:center;white-space:nowrap}\
th.c-fail{color:var(--fail)}th.c-skip{color:var(--skip)}th.c-pass{color:var(--pass)}\
th.c-total{color:var(--total)}th.c-unreported{border-left:4px solid #c9c9c9}\
th,td{border:1px solid var(--line);padding:.35rem .6rem;text-align:left;vertical-align:top}\
th{color:var(--muted);font-weight:600}\
td.num{text-align:right;white-space:nowrap}\
td.links{white-space:nowrap}\
.scroll{overflow-x:auto}\
td.test{min-width:10rem;overflow-wrap:break-word}\
td.msg{min-width:16rem;white-space:pre-wrap;overflow-wrap:anywhere}\
td.msg .kind{font:600 12px/1.6 ui-monospace,monospace;color:var(--muted)}\
td.msg .error{font-weight:600}\
td.msg .hint,td.msg .page,td.msg .evidence{margin-top:.25rem}\
td.msg details{margin-top:.35rem;color:var(--muted)}\
td.msg pre{margin:.25rem 0 0;white-space:pre-wrap;font:12px/1.5 ui-monospace,monospace}\
td.status{font-weight:700;white-space:nowrap}\
tr.pass{background:var(--pass-bg)}tr.skip{background:var(--skip-bg)}\
tr.fail,tr.unreported{background:var(--fail-bg)}\
tr.pass td.status{color:var(--pass)}\
tr.fail td.status,tr.unreported td.status{color:var(--fail)}\
tr.skip td.status{color:var(--skip)}tr.untested td.status{color:var(--muted)}\
tr.unreported td.status{border-left:4px solid #c9c9c9}\
.updated,.status-line,.generated{margin:.75rem 0 0}\
.status-line.pass{color:var(--pass)}.status-line.fail{color:var(--fail)}\
@media(max-width:40rem){body{padding:.5rem}th,td{padding:.25rem .4rem}}";

// ---------------------------------------------------------------------------
// The pages
// ---------------------------------------------------------------------------

/// An HTML page listing the results of `run`, with a summary line on top.
///
/// The page is self-contained: its stylesheet is inline and it needs no
/// script. Test names and messages are escaped. The tests are listed in the
/// order they started. With [`PageOptions::pie`] the pie chart of the results
/// is drawn above the table.
///
/// The test of a finished run that never finished shows as `UNREPORTED`.
#[must_use]
pub fn render_html_report(run: &RunSnapshot, options: &PageOptions) -> String {
    let run = settled(run);
    let summary = run.summary();
    let verdict = if summary.failed == 0 && summary.unreported == 0 {
        "pass"
    } else {
        "fail"
    };
    let mut html = start_page(&options.title, options.live);
    heading(&mut html, &options.title, options.pie);
    // Writing to a `String` cannot fail.
    let _ = writeln!(html, "<p class=\"summary {verdict}\">{summary}</p>");
    let noun = if summary.total == 1 { "test" } else { "tests" };
    let _ = writeln!(
        html,
        "<p class=\"run-count\">{} {noun} ran in {:.2} seconds.</p>",
        summary.total, summary.duration_secs
    );
    if options.pie {
        pie_figure(&mut html, &summary, &options.title);
    }
    let rows: Vec<&TestResult> = run.results.iter().collect();
    results_table(&mut html, &rows, &run, options);
    html.push_str("</body>\n</html>\n");
    html
}

/// An HTML page for a run that may still be going, in the layout of Python's
/// `dashboard.html`.
///
/// While the run is not finished the page refreshes itself and its status line
/// reads `Awaiting results...`. A finished run gives the final page: no
/// refresh, `Test Run Complete` (or `ENDED` when tests never reported) and
/// the tests that never reported as `UNREPORTED`. The rows are ordered failed,
/// skipped, passed with logs, passed without logs, untested, unreported.
#[must_use]
pub fn render_dashboard(run: &RunSnapshot, options: &PageOptions) -> String {
    let run = settled(run);
    let summary = run.summary();
    let mut html = start_page(&options.title, options.live || !run.finished);
    heading(&mut html, &options.title, true);
    pie_figure(&mut html, &summary, &options.title);
    counts_table(&mut html, &summary);
    let rows = dashboard_order(&run.results);
    results_table(&mut html, &rows, &run, options);
    let when = get_master_time(&run.updated_at);
    let _ = writeln!(
        html,
        "<p class=\"updated\"><b>Last updated:</b> {} at {}</p>",
        html_escape(&when.date),
        html_escape(&when.time)
    );
    html.push_str(&status_paragraph(&run, &summary, options));
    html.push_str("<p class=\"generated\">Generated by: <b>seleniumbase-rs</b></p>\n");
    html.push_str("</body>\n</html>\n");
    html
}

/// `run`, with every untested test of a finished run turned into an
/// unreported one: nothing is going to finish it now. Python does the same
/// when it ends the dashboard.
fn settled(run: &RunSnapshot) -> Cow<'_, RunSnapshot> {
    if !run.finished
        || run
            .results
            .iter()
            .all(|result| result.status != TestStatus::Untested)
    {
        return Cow::Borrowed(run);
    }
    let mut settled = run.clone();
    for result in &mut settled.results {
        if result.status == TestStatus::Untested {
            result.status = TestStatus::Unreported;
            if result.message.is_empty() {
                "the test did not finish".clone_into(&mut result.message);
            }
        }
    }
    Cow::Owned(settled)
}

/// The start of a page up to the opening `<body>`.
fn start_page(title: &str, refresh: bool) -> String {
    let mut html = String::with_capacity(8192);
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
    let _ = write!(
        html,
        "<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n",
        html_escape(title)
    );
    html
}

/// The page's `<h1>`. When the pie chart carries the title, the heading stays
/// for screen readers only.
fn heading(html: &mut String, title: &str, hidden: bool) {
    let class = if hidden { " class=\"sr-only\"" } else { "" };
    let _ = writeln!(html, "<h1{class}>{}</h1>", html_escape(title));
}

fn pie_figure(html: &mut String, summary: &ReportSummary, title: &str) {
    let _ = writeln!(
        html,
        "<div class=\"pie\">{}</div>",
        pie_chart_svg(summary, title)
    );
}

/// The row of counts under the pie: `Failed | Skipped | Passed | Untested |
/// Total`, with `Unreported` before the total when some tests never reported.
fn counts_table(html: &mut String, summary: &ReportSummary) {
    html.push_str("<table class=\"counts\"><thead><tr>");
    let mut cell = |class: &str, label: &str, count: usize| {
        let _ = write!(html, "<th class=\"{class}\">{label}:&nbsp;{count}</th>");
    };
    cell("c-fail", "Failed", summary.failed);
    cell("c-skip", "Skipped", summary.skipped);
    cell("c-pass", "Passed", summary.passed);
    cell("c-untested", "Untested", summary.untested);
    if summary.unreported > 0 {
        cell("c-unreported", "Unreported", summary.unreported);
    }
    cell("c-total", "Total", summary.total);
    html.push_str("</tr></thead></table>\n");
}

/// The line at the bottom of the dashboard, with Python's wording.
fn status_paragraph(run: &RunSnapshot, summary: &ReportSummary, options: &PageOptions) -> String {
    let logs = format!(
        "{}/",
        relative_link(&options.page_dir, &run.root.join(LATEST_LOGS_DIR))
    );
    let failed = match summary.failed {
        0 => String::new(),
        count => format!(
            " <b>{count} {} failed!</b> --- (<b><a href=\"{}\">See latest logs for details</a></b>)",
            if count == 1 { "test" } else { "tests" },
            html_escape(&logs)
        ),
    };
    let (class, text) = if !run.finished {
        (
            "wait",
            " Awaiting results... (Refresh the page for updates)".to_owned(),
        )
    } else if summary.unreported > 0 {
        (
            "fail",
            format!(" Test Run ENDED: Some results UNREPORTED due to skipped tearDown(){failed}"),
        )
    } else if summary.failed > 0 {
        ("fail", format!(" Test Run Complete:{failed}"))
    } else if summary.total == 0 {
        ("wait", " Test Run Complete: No tests were run.".to_owned())
    } else if summary.passed == 0 {
        (
            "pass",
            " Test Run Complete: All tests were skipped!".to_owned(),
        )
    } else if summary.skipped == 0 {
        (
            "pass",
            " Test Run Complete: <b>Success!</b> (All tests passed)".to_owned(),
        )
    } else {
        (
            "pass",
            " Test Run Complete: <b>Success!</b> (No failing tests)".to_owned(),
        )
    };
    format!("<p class=\"status-line {class}\"><b>Status:</b>{text}</p>\n")
}

/// The tests in the order the dashboard lists them: failed, skipped, passed
/// with logs, passed without logs, untested, unreported. Tests of one kind
/// keep the order they started in.
fn dashboard_order(results: &[TestResult]) -> Vec<&TestResult> {
    let rank = |result: &TestResult| match result.status {
        TestStatus::Failed => 0,
        TestStatus::Skipped => 1,
        TestStatus::Passed if result.log_dir.is_some() => 2,
        TestStatus::Passed => 3,
        TestStatus::Untested => 4,
        TestStatus::Unreported => 5,
    };
    let mut rows: Vec<&TestResult> = results.iter().collect();
    rows.sort_by_key(|result| rank(result));
    rows
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

fn results_table(
    html: &mut String,
    rows: &[&TestResult],
    run: &RunSnapshot,
    options: &PageOptions,
) {
    if rows.is_empty() {
        html.push_str("<p>No tests were recorded.</p>\n");
        return;
    }
    html.push_str(
        "<div class=\"scroll\">\n<table class=\"results\">\n<thead><tr><th>Result</th><th>Test</th><th>Duration</th>\
         <th>Links</th><th>Details</th></tr></thead>\n<tbody>\n",
    );
    for result in rows {
        row(html, run, options, result);
    }
    html.push_str("</tbody>\n</table>\n</div>\n");
}

fn row(html: &mut String, run: &RunSnapshot, options: &PageOptions, result: &TestResult) {
    let class = match result.status {
        TestStatus::Passed => "pass",
        TestStatus::Failed => "fail",
        TestStatus::Skipped => "skip",
        TestStatus::Untested => "untested",
        TestStatus::Unreported => "unreported",
    };
    let details = match (&result.failure, result.status) {
        (Some(failure), TestStatus::Failed) => failure_cell(&result.message, failure, run, options),
        _ => html_escape(&result.message),
    };
    let _ = writeln!(
        html,
        "<tr class=\"{class}\"><td class=\"status\">{}</td><td class=\"test\">{}</td>\
         <td class=\"num\">{:.2}s</td><td class=\"links\">{}</td>\
         <td class=\"msg\">{details}</td></tr>",
        result.status.label(),
        html_escape(&result.name),
        result.duration_secs,
        links_cell(result, run, options),
    );
}

/// `Logs / Data` for a test that left a folder of logs, `-` for one that did
/// not. `Logs` leads to the folder that holds them all and `Data` to the
/// test's own.
fn links_cell(result: &TestResult, run: &RunSnapshot, options: &PageOptions) -> String {
    let Some(data) = result
        .log_dir
        .as_deref()
        .filter(|dir| is_plain_relative(dir))
    else {
        return "-".to_owned();
    };
    let folder_link = |path: &Path| {
        let href = relative_link(&options.page_dir, &run.root.join(path));
        html_escape(&format!("{href}/"))
    };
    let data_link = format!("<a href=\"{}\">Data</a>", folder_link(Path::new(data)));
    match Path::new(data)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(logs) => format!("<a href=\"{}\">Logs</a> / {data_link}", folder_link(logs)),
        None => data_link,
    }
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
    if is_plain_relative(path) {
        let href = relative_link(&options.page_dir, &run.root.join(path));
        format!("<a href=\"{}\">{label}</a>", html_escape(&href))
    } else {
        format!("{label}: {}", html_escape(path))
    }
}

/// Whether `path` is a relative path that stays inside the folder it is
/// relative to: no root, no drive, no `..` and no scheme such as
/// `javascript:`. Only such a path may become a link or an attachment.
pub(crate) fn is_plain_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains([':', '\\', '\0'])
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

// ---------------------------------------------------------------------------
// The pie chart
// ---------------------------------------------------------------------------

/// One kind of result in the pie.
struct Part {
    status: &'static str,
    label: &'static str,
    color: &'static str,
    count: usize,
}

/// The kinds of result in Python's order, with `Unreported` added when some
/// tests never reported.
fn parts(summary: &ReportSummary) -> Vec<Part> {
    let mut parts = vec![
        Part {
            status: "passed",
            label: "Passed",
            color: PASSED_COLOR,
            count: summary.passed,
        },
        Part {
            status: "untested",
            label: "Untested",
            color: UNTESTED_COLOR,
            count: summary.untested,
        },
        Part {
            status: "skipped",
            label: "Skipped",
            color: SKIPPED_COLOR,
            count: summary.skipped,
        },
        Part {
            status: "failed",
            label: "Failed",
            color: FAILED_COLOR,
            count: summary.failed,
        },
    ];
    if summary.unreported > 0 {
        parts.push(Part {
            status: "unreported",
            label: "Unreported",
            color: UNREPORTED_COLOR,
            count: summary.unreported,
        });
    }
    parts
}

/// A pie chart of the results as an inline `<svg>`, with a legend that names
/// every kind of result and its count, so the chart never rests on colour.
///
/// - The slices are Python's: passed `#84d474`, untested `#eaeaea`, skipped
///   `#efd8b4`, failed `#f17476`, and unreported `#c9c9c9` when there are
///   some. A kind with no tests has no slice but keeps its legend entry.
/// - One kind alone is drawn as a full circle; no tests at all as an empty
///   dashed one saying `No tests`.
/// - `title` is drawn above the pie and is the chart's accessible name; the
///   summary line is its description.
///
/// The drawing needs no script and takes its text colour from the page.
#[must_use]
pub fn pie_chart_svg(summary: &ReportSummary, title: &str) -> String {
    let parts = parts(summary);
    let total: usize = parts.iter().map(|part| part.count).sum();
    let (cx, cy) = PIE_CENTER;
    let title = html_escape(title);
    let mut svg = String::with_capacity(2048);
    let _ = write!(
        svg,
        "<svg class=\"pie-chart\" xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {PIE_WIDTH} {PIE_HEIGHT}\" role=\"img\" \
         aria-labelledby=\"pie-title pie-desc\">\
         <title id=\"pie-title\">{title}</title><desc id=\"pie-desc\">{}</desc>\
         <text class=\"pie-title\" x=\"{}\" y=\"26\" text-anchor=\"middle\">{title}</text>",
        html_escape(&summary.to_string()),
        PIE_WIDTH / 2
    );
    if total == 0 {
        let _ = write!(
            svg,
            "<circle class=\"empty\" cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{PIE_RADIUS:.2}\"/>\
             <text x=\"{cx:.2}\" y=\"{:.2}\" text-anchor=\"middle\">No tests</text>",
            cy + 5.0
        );
    } else {
        let mut start = -FRAC_PI_2;
        for part in parts.iter().filter(|part| part.count > 0) {
            let tooltip = format!("{}: {}", part.label, part.count);
            if part.count == total {
                let _ = write!(
                    svg,
                    "<circle data-status=\"{}\" cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{PIE_RADIUS:.2}\" \
                     fill=\"{}\"><title>{tooltip}</title></circle>",
                    part.status, part.color
                );
                continue;
            }
            let sweep = TAU * part.count as f64 / total as f64;
            let end = start + sweep;
            let point = |angle: f64| (cx + PIE_RADIUS * angle.cos(), cy + PIE_RADIUS * angle.sin());
            let ((x0, y0), (x1, y1)) = (point(start), point(end));
            let large = u8::from(sweep > PI);
            let _ = write!(
                svg,
                "<path data-status=\"{}\" d=\"M{cx:.2} {cy:.2} L{x0:.2} {y0:.2} \
                 A{PIE_RADIUS:.2} {PIE_RADIUS:.2} 0 {large} 1 {x1:.2} {y1:.2} Z\" \
                 fill=\"{}\"><title>{tooltip}</title></path>",
                part.status, part.color
            );
            start = end;
        }
    }
    for (index, part) in parts.iter().enumerate() {
        let top = LEGEND_TOP + LEGEND_STEP * u32::try_from(index).unwrap_or(u32::MAX / 64);
        let percent = if total == 0 {
            0.0
        } else {
            part.count as f64 * 100.0 / total as f64
        };
        let _ = write!(
            svg,
            "<rect x=\"{LEGEND_X}\" y=\"{top}\" width=\"16\" height=\"16\" rx=\"3\" \
             fill=\"{}\"/><text x=\"{}\" y=\"{}\">{}: {} ({percent:.0}%)</text>",
            part.color,
            LEGEND_X + 24,
            top + 13,
            part.label,
            part.count
        );
    }
    svg.push_str("</svg>");
    svg
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Writes the HTML report of `run` to `path`, replacing any file there. The
/// parent directory must exist, and `options.page_dir` should be it.
///
/// The file is written to a temporary neighbour and renamed into place, so a
/// reader never sees half a page.
///
/// When the run is finished the report is also kept in [`LAST_REPORT_FILE`]
/// in the run's folder (`run.root`), with its links made from there. A failure
/// to write that copy is logged and does not fail the call: the report itself
/// is written.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub fn write_html_report<P: AsRef<Path>>(
    path: P,
    run: &RunSnapshot,
    options: &PageOptions,
) -> io::Result<()> {
    let path = path.as_ref();
    write_atomically(path, render_html_report(run, options).as_bytes())?;
    if run.finished {
        let copy = run.root.join(LAST_REPORT_FILE);
        if !same_file(path, &copy) {
            let from_root = PageOptions {
                page_dir: run.root.clone(),
                ..options.clone()
            };
            if let Err(error) =
                write_atomically(&copy, render_html_report(run, &from_root).as_bytes())
            {
                tracing::warn!(%error, copy = %copy.display(), "the last report was not written");
            }
        }
    }
    Ok(())
}

/// Whether `a` and `b` name the same file, going by their path or, when both
/// exist, by where they lead.
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

/// Like [`write_html_report`], without blocking the async runtime. It does not
/// write [`LAST_REPORT_FILE`].
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
/// [`write_html_report`] does (and without a copy in [`LAST_REPORT_FILE`]: the
/// finished dashboard is the final page).
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

    fn with_logs(mut result: TestResult, log_dir: &str) -> TestResult {
        result.log_dir = Some(log_dir.to_owned());
        result
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

    fn dashboard_of(results: Vec<TestResult>, finished: bool) -> String {
        let mut run = RunSnapshot::from_results("Dash", results);
        run.finished = finished;
        render_dashboard(&run, &PageOptions::default())
    }

    fn summary_of(results: &[TestResult]) -> ReportSummary {
        ReportSummary::from_results(results)
    }

    // -- the report ---------------------------------------------------------

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
        assert!(
            html.contains("<p class=\"run-count\">3 tests ran in 2.00 seconds.</p>"),
            "{html}"
        );
        assert!(html.contains("class=\"summary fail\""));
        assert_eq!(html.matches("<tr class=\"pass\">").count(), 2);
        assert_eq!(html.matches("<tr class=\"fail\">").count(), 1);
        assert!(html.contains(">PASS<") && html.contains(">FAIL<"));
        assert!(html.contains("<td class=\"num\">1.25s</td>"), "{html}");
        assert!(html.contains(
            "<th>Result</th><th>Test</th><th>Duration</th><th>Links</th><th>Details</th>"
        ));
        assert!(!html.contains("class=\"pie-chart\""), "no pie unless asked");
    }

    #[test]
    fn the_summary_line_names_the_skipped_tests() {
        let html = html_of(vec![
            result("a", TestStatus::Passed, 0.5, "ok"),
            result("b", TestStatus::Failed, 0.5, "x"),
            result("c", TestStatus::Skipped, 0.0, "no browser"),
        ]);
        assert!(
            html.contains("3 tests: 1 passed, 1 failed, 1 skipped in 1.000s"),
            "{html}"
        );
        assert!(html.contains("<p class=\"run-count\">3 tests ran in 1.00 seconds.</p>"));
        assert!(html_of(vec![result("a", TestStatus::Passed, 0.0, "ok")])
            .contains("1 test ran in 0.00 seconds."));
    }

    #[test]
    fn a_skipped_untested_and_unreported_row_each_say_so() {
        let mut run = RunSnapshot::from_results(
            "R",
            vec![
                result("a", TestStatus::Skipped, 0.0, "no browser"),
                result("b", TestStatus::Untested, 0.0, ""),
                result("c", TestStatus::Unreported, 0.0, ""),
            ],
        );
        run.finished = false;
        let html = render_html_report(&run, &PageOptions::default());
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
    fn the_untested_tests_of_a_finished_run_are_unreported() {
        let html = html_of(vec![
            result("a", TestStatus::Passed, 0.1, "ok"),
            result("b", TestStatus::Untested, 0.0, ""),
        ]);

        assert!(!html.contains(">UNTESTED<"), "{html}");
        assert!(html.contains(">UNREPORTED<"));
        assert!(html.contains("1 unreported"));
        assert!(html.contains("the test did not finish"));
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
        assert!(!html.contains("<table"));
    }

    #[test]
    fn the_report_draws_the_pie_when_asked() {
        let run = RunSnapshot::from_results("Report", mixed());
        let options = PageOptions {
            pie: true,
            title: "Pie".to_owned(),
            ..PageOptions::default()
        };
        let html = render_html_report(&run, &options);

        assert!(html.contains("<svg class=\"pie-chart\""));
        assert!(html.contains("Passed: 2 (67%)"));
        assert!(
            html.contains("<h1 class=\"sr-only\">Pie</h1>"),
            "the pie carries the visible title"
        );
        assert!(!html.contains("<script"));
    }

    // -- the dashboard ------------------------------------------------------

    #[test]
    fn a_live_dashboard_refreshes_and_awaits_results_and_a_final_one_does_neither() {
        let live = dashboard_of(mixed(), false);
        assert!(live.contains("<meta http-equiv=\"refresh\" content=\"12\">"));
        assert!(live.contains("Awaiting results... (Refresh the page for updates)"));
        assert!(!live.contains("Test Run Complete"));

        let done = dashboard_of(mixed(), true);
        assert!(!done.contains("http-equiv=\"refresh\""));
        assert!(!done.contains("Awaiting results"));
        assert!(done.contains("Test Run Complete"));
        assert!(!html_of(mixed()).contains("http-equiv=\"refresh\""));
        assert!(
            render_html_report(
                &RunSnapshot::from_results("R", mixed()),
                &PageOptions {
                    live: true,
                    ..PageOptions::default()
                }
            )
            .contains("http-equiv=\"refresh\""),
            "a live report refreshes"
        );
    }

    #[test]
    fn the_dashboard_has_python_s_layout() {
        let html = dashboard_of(mixed(), true);

        assert!(html.contains("<svg class=\"pie-chart\""));
        assert!(html.contains("Failed:&nbsp;1</th>"));
        assert!(html.contains("Skipped:&nbsp;0</th>"));
        assert!(html.contains("Passed:&nbsp;2</th>"));
        assert!(html.contains("Untested:&nbsp;0</th>"));
        assert!(html.contains("Total:&nbsp;3</th>"));
        assert!(
            !html.contains("Unreported:&nbsp;"),
            "only when there are some"
        );
        assert!(html.contains(
            "<th>Result</th><th>Test</th><th>Duration</th><th>Links</th><th>Details</th>"
        ));
        assert!(html.contains("<b>Last updated:</b> "), "{html}");
        assert!(html.contains("Generated by: <b>seleniumbase-rs</b>"));
        assert!(html.contains("<h1 class=\"sr-only\">SeleniumBase Rust Test Report</h1>"));
        let order = [
            "Failed:&nbsp;",
            "Skipped:&nbsp;",
            "Passed:&nbsp;",
            "Untested:&nbsp;",
            "Total:&nbsp;",
        ];
        let at: Vec<usize> = order.iter().map(|word| html.find(word).unwrap()).collect();
        assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{at:?}");
        assert!(html.contains("<td class=\"num\">1.25s</td>"));
    }

    #[test]
    fn the_counts_row_names_the_unreported_tests_when_there_are_some() {
        let html = dashboard_of(
            vec![
                result("a", TestStatus::Passed, 0.0, "ok"),
                result("b", TestStatus::Unreported, 0.0, ""),
            ],
            true,
        );
        assert!(html.contains("Unreported:&nbsp;1</th>"), "{html}");
        assert!(html.contains("Total:&nbsp;2</th>"));
    }

    #[test]
    fn the_rows_are_ordered_failed_skipped_passed_with_logs_passed_untested() {
        let html = dashboard_of(
            vec![
                result("t_untested", TestStatus::Untested, 0.0, ""),
                result("t_plain", TestStatus::Passed, 0.1, "ok"),
                with_logs(
                    result("t_logged", TestStatus::Passed, 0.1, "ok"),
                    "latest_logs/t_logged",
                ),
                result("t_skipped", TestStatus::Skipped, 0.0, "why"),
                result("t_failed", TestStatus::Failed, 0.1, "bad"),
                result("t_unreported", TestStatus::Unreported, 0.0, ""),
            ],
            false,
        );

        let at: Vec<usize> = [
            "t_failed",
            "t_skipped",
            "t_logged",
            "t_plain",
            "t_untested",
            "t_unreported",
        ]
        .iter()
        .map(|name| html.find(&format!(">{name}</td>")).unwrap())
        .collect();
        assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{at:?}");
    }

    #[test]
    fn the_status_line_uses_python_s_words() {
        let passed = result("a", TestStatus::Passed, 0.1, "ok");
        let skipped = result("b", TestStatus::Skipped, 0.0, "why");
        let failed = |name: &str| result(name, TestStatus::Failed, 0.1, "bad");

        assert!(dashboard_of(vec![passed.clone()], true)
            .contains("Test Run Complete: <b>Success!</b> (All tests passed)"));
        assert!(dashboard_of(vec![passed.clone(), skipped.clone()], true)
            .contains("<b>Success!</b> (No failing tests)"));
        assert!(dashboard_of(vec![skipped.clone()], true)
            .contains("Test Run Complete: All tests were skipped!"));
        assert!(
            dashboard_of(vec![passed.clone(), failed("c")], true).contains(
                "Test Run Complete: <b>1 test failed!</b> --- \
             (<b><a href=\"latest_logs/\">See latest logs for details</a></b>)"
            )
        );
        assert!(dashboard_of(vec![failed("c"), failed("d")], true)
            .contains("<b>2 tests failed!</b> --- "));
        assert!(dashboard_of(Vec::new(), false).contains("Awaiting results..."));
        assert!(dashboard_of(Vec::new(), true).contains("No tests were run."));

        let ended = dashboard_of(
            vec![failed("c"), result("u", TestStatus::Unreported, 0.0, "")],
            true,
        );
        assert!(ended.contains("Test Run ENDED: Some results UNREPORTED due to skipped tearDown()"));
        assert!(
            ended.contains("<b>1 test failed!</b>"),
            "failures are not hidden"
        );
    }

    #[test]
    fn the_untested_tests_of_a_finished_dashboard_are_unreported() {
        let html = dashboard_of(
            vec![
                result("a", TestStatus::Passed, 0.1, "ok"),
                result("b", TestStatus::Untested, 0.0, ""),
            ],
            true,
        );
        assert!(html.contains(">UNREPORTED<"), "{html}");
        assert!(!html.contains(">UNTESTED<"));
        assert!(html.contains("Test Run ENDED"));
        assert!(html.contains("Untested:&nbsp;0</th>"));
        assert!(html.contains("Unreported:&nbsp;1</th>"));
    }

    #[test]
    fn hostile_names_and_links_cannot_inject_markup_or_leave_the_run() {
        let html = dashboard_of(
            vec![
                with_logs(
                    result(
                        "<script>alert(1)</script>\"x",
                        TestStatus::Failed,
                        0.0,
                        "<img src=x onerror=alert(1)>",
                    ),
                    "javascript:alert(1)",
                ),
                with_logs(result("climbs", TestStatus::Passed, 0.0, "ok"), "../../etc"),
                with_logs(
                    result("quotes", TestStatus::Passed, 0.0, "ok"),
                    "latest_logs/a\"b",
                ),
            ],
            true,
        );

        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(!html.contains("href=\"../"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;&quot;x"));
        assert!(
            html.contains("<td class=\"links\">-</td>"),
            "a result with a hostile folder gets no links"
        );
        assert!(
            html.contains("href=\"latest_logs/a%22b/\""),
            "a quote in a folder name is encoded: {html}"
        );
    }

    #[test]
    fn logs_and_data_links_lead_from_the_folder_the_page_is_in() {
        let mut run = RunSnapshot::from_results(
            "R",
            vec![with_logs(
                result("a", TestStatus::Failed, 0.0, "bad"),
                "latest_logs/a.b",
            )],
        );
        run.root = PathBuf::from("/run");

        let in_root = render_dashboard(
            &run,
            &PageOptions {
                page_dir: PathBuf::from("/run"),
                ..PageOptions::default()
            },
        );
        assert!(
            in_root.contains(
                "<a href=\"latest_logs/\">Logs</a> / <a href=\"latest_logs/a.b/\">Data</a>"
            ),
            "{in_root}"
        );

        let in_reports = render_html_report(
            &run,
            &PageOptions {
                page_dir: PathBuf::from("/run/reports"),
                ..PageOptions::default()
            },
        );
        assert!(
            in_reports.contains(
                "<a href=\"../latest_logs/\">Logs</a> / <a href=\"../latest_logs/a.b/\">Data</a>"
            ),
            "{in_reports}"
        );
    }

    #[test]
    fn a_test_without_logs_has_a_dash_for_links() {
        let html = html_of(vec![result("a", TestStatus::Passed, 0.0, "ok")]);
        assert!(html.contains("<td class=\"links\">-</td>"));
    }

    #[test]
    fn plain_relative_paths_are_the_only_ones_that_become_links() {
        for good in ["a", "a/b.png", "./a", "latest_logs/x.y-z"] {
            assert!(is_plain_relative(good), "{good}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../../x",
            "javascript:alert(1)",
            "C:\\x",
            "a\\..\\b",
            "a\0b",
        ] {
            assert!(!is_plain_relative(bad), "{bad:?}");
        }
    }

    // -- the pie ------------------------------------------------------------

    #[test]
    fn a_pie_of_no_tests_is_an_empty_circle_with_a_full_legend() {
        let svg = pie_chart_svg(&summary_of(&[]), "Nothing");

        assert!(svg.contains("class=\"empty\""));
        assert!(svg.contains(">No tests</text>"));
        assert_eq!(svg.matches("<path").count(), 0);
        assert_eq!(svg.matches("<rect").count(), 4, "{svg}");
        for label in [
            "Passed: 0 (0%)",
            "Untested: 0 (0%)",
            "Skipped: 0 (0%)",
            "Failed: 0 (0%)",
        ] {
            assert!(svg.contains(label), "{label}");
        }
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    }

    #[test]
    fn a_pie_of_one_kind_is_a_full_circle() {
        let svg = pie_chart_svg(
            &summary_of(&[
                result("a", TestStatus::Passed, 0.0, "ok"),
                result("b", TestStatus::Passed, 0.0, "ok"),
                result("c", TestStatus::Passed, 0.0, "ok"),
            ]),
            "All green",
        );

        assert_eq!(svg.matches("<circle").count(), 1, "{svg}");
        assert!(svg.contains("data-status=\"passed\"") && svg.contains("fill=\"#84d474\""));
        assert_eq!(svg.matches("<path").count(), 0);
        assert!(svg.contains("Passed: 3 (100%)") && svg.contains("Failed: 0 (0%)"));
    }

    #[test]
    fn a_pie_of_every_kind_has_a_slice_and_a_legend_entry_for_each() {
        let svg = pie_chart_svg(
            &summary_of(&[
                result("a", TestStatus::Passed, 0.0, "ok"),
                result("b", TestStatus::Untested, 0.0, ""),
                result("c", TestStatus::Skipped, 0.0, ""),
                result("d", TestStatus::Failed, 0.0, "x"),
                result("e", TestStatus::Unreported, 0.0, ""),
            ]),
            "Everything",
        );

        assert_eq!(svg.matches("<path").count(), 5, "{svg}");
        assert_eq!(svg.matches("<rect").count(), 5);
        for color in ["#84d474", "#eaeaea", "#efd8b4", "#f17476", "#c9c9c9"] {
            assert!(svg.contains(&format!("fill=\"{color}\"")), "{color}");
        }
        assert!(svg.contains("Unreported: 1 (20%)"));
        assert_eq!(svg.matches("(20%)").count(), 5);
    }

    #[test]
    fn the_big_slice_uses_the_large_arc_flag_and_the_small_one_does_not() {
        let svg = pie_chart_svg(
            &summary_of(&[
                result("a", TestStatus::Passed, 0.0, "ok"),
                result("b", TestStatus::Passed, 0.0, "ok"),
                result("c", TestStatus::Failed, 0.0, "x"),
            ]),
            "Mixed",
        );

        assert!(svg.contains("A80.00 80.00 0 1 1"), "two thirds: {svg}");
        assert!(svg.contains("A80.00 80.00 0 0 1"), "one third: {svg}");
        // Passed starts at twelve o'clock.
        assert!(svg.contains("M100.00 140.00 L100.00 60.00 "), "{svg}");
    }

    #[test]
    fn the_pie_is_accessible_and_escapes_its_title() {
        let svg = pie_chart_svg(&summary_of(&mixed()), "<b>\"T\"</b>");

        assert!(svg.contains("role=\"img\""));
        assert!(svg.contains("<title id=\"pie-title\">&lt;b&gt;&quot;T&quot;&lt;/b&gt;</title>"));
        assert!(svg.contains("<desc id=\"pie-desc\">3 tests: 2 passed, 1 failed in 2.000s</desc>"));
        assert!(!svg.contains("<b>"));
        assert!(!svg.contains("<script"));
    }

    // -- writing ------------------------------------------------------------

    #[test]
    fn written_pages_replace_the_old_file_and_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let html = dir.path().join("report.html");
        let dash = dir.path().join("dashboard.html");
        std::fs::write(&html, "old").unwrap();
        let mut run = RunSnapshot::from_results("R", mixed());
        run.finished = false;
        run.root = dir.path().to_path_buf();

        write_html_report(&html, &run, &PageOptions::default()).unwrap();
        write_dashboard(&dash, &run, &PageOptions::default()).unwrap();

        assert!(std::fs::read_to_string(&html).unwrap().contains("PASS"));
        assert!(std::fs::read_to_string(&dash).unwrap().contains("PASS"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        assert!(write_html_report(dir.path().join(".."), &run, &PageOptions::default()).is_err());
    }

    #[test]
    fn a_finished_report_is_also_kept_as_last_report_with_links_from_the_run_folder() {
        let dir = tempfile::tempdir().unwrap();
        let reports = dir.path().join("reports");
        std::fs::create_dir(&reports).unwrap();
        let report = reports.join("report.html");
        let mut run = RunSnapshot::from_results(
            "R",
            vec![failed_with(FailureDetails {
                screenshot: Some("latest_logs/a/screenshot.png".to_owned()),
                ..FailureDetails::default()
            })],
        );
        run.root = dir.path().to_path_buf();
        let options = PageOptions {
            page_dir: reports.clone(),
            ..PageOptions::default()
        };

        // A run that goes on has no last report yet.
        run.finished = false;
        write_html_report(&report, &run, &options).unwrap();
        assert!(!dir.path().join(LAST_REPORT_FILE).exists());

        run.finished = true;
        write_html_report(&report, &run, &options).unwrap();
        let last = std::fs::read_to_string(dir.path().join(LAST_REPORT_FILE)).unwrap();
        assert!(
            last.contains("<a href=\"latest_logs/a/screenshot.png\">Screenshot</a>"),
            "{last}"
        );
        assert!(std::fs::read_to_string(&report)
            .unwrap()
            .contains("<a href=\"../latest_logs/a/screenshot.png\">Screenshot</a>"));
    }

    #[test]
    fn a_report_that_is_the_last_report_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut run = RunSnapshot::from_results("R", mixed());
        run.root = dir.path().to_path_buf();
        let options = PageOptions {
            page_dir: dir.path().to_path_buf(),
            ..PageOptions::default()
        };

        write_html_report(dir.path().join(LAST_REPORT_FILE), &run, &options).unwrap();

        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn the_async_writer_matches_the_blocking_one() {
        let dir = tempfile::tempdir().unwrap();
        let (html, missing) = (
            dir.path().join("r.html"),
            dir.path().join("no-such-dir").join("r.html"),
        );
        let mut run = RunSnapshot::from_results("R", mixed());
        run.root = dir.path().to_path_buf();
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
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "the async writer keeps no last report"
        );
    }
}
