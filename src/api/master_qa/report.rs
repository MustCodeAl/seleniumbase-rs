//! What a [`MasterQA`](super::MasterQA) run writes down: the question of a
//! check, the rows of `results_table.csv`, the text printed at the end and the
//! `report.html` page.
//!
//! These are plain functions over plain values, so the files can be checked
//! without a browser. The texts, the column order and the colours are those of
//! Python's `masterqa/master_qa.py`.

use std::path::Path;

/// The file with one row per check, kept in the archive folder.
pub(crate) const RESULTS_TABLE: &str = "results_table.csv";

/// The page that sums the run up. It is written to `latest_report` and copied
/// to the archive folder.
pub(crate) const RESULTS_PAGE: &str = "report.html";

/// The first line of `results_table.csv`.
pub(crate) const CSV_HEADER: &str = "\"Num\",\"Result\",\"Screenshot\",\"URL\",\"Browser\",\
     \"Epoch Time\",\"Verification Instructions\",\"Additional Info\"";

/// The colour of a count that is as it should be.
const GOOD: &str = "#11BB11";

/// The colour of a count that says something went wrong.
const BAD: &str = "#EE3A3A";

/// The longest address the failure table shows whole; a longer one is cut to
/// [`SHOWN_URL_LEN`] characters and `...`.
const LONGEST_SHOWN_URL: usize = 60;

/// How much of a long address the failure table shows.
const SHOWN_URL_LEN: usize = 58;

/// How a check ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The person answered YES / PASS.
    Success,
    /// The person answered NO / FAIL.
    Failed,
    /// The test raised an error before it finished.
    Error,
}

impl Verdict {
    /// The word in the `Result` column.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Failed => "FAILED!",
            Self::Error => "ERROR!",
        }
    }
}

/// One row of `results_table.csv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    /// The number of the check, or `ERR` for an error.
    pub(crate) number: String,
    pub(crate) verdict: Verdict,
    /// The screenshot file of a failure, `-` for a success.
    pub(crate) screenshot: String,
    pub(crate) url: String,
    pub(crate) browser: String,
    /// Seconds since the epoch.
    pub(crate) epoch: u64,
    pub(crate) instructions: String,
    pub(crate) info: String,
}

impl Row {
    /// The line of this row: every field in double quotes, a double quote
    /// inside a field doubled (Python writes it as it is, which breaks the
    /// line).
    pub(crate) fn csv_line(&self) -> String {
        let epoch = self.epoch.to_string();
        let fields = [
            self.number.as_str(),
            self.verdict.label(),
            &self.screenshot,
            &self.url,
            &self.browser,
            &epoch,
            &self.instructions,
            &self.info,
        ];
        let quoted: Vec<String> = fields
            .iter()
            .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
            .collect();
        quoted.join(",")
    }
}

/// The content of `results_table.csv`: the header, then a line per row.
pub(crate) fn results_table(rows: &[Row]) -> String {
    let mut table = String::from(CSV_HEADER);
    table.push('\n');
    for row in rows {
        table.push_str(&row.csv_line());
        table.push('\n');
    }
    table
}

/// What the run came to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Checks a person answered.
    pub(crate) answered: usize,
    /// Of those, the ones answered YES / PASS.
    pub(crate) passed: usize,
    /// Errors that ended the test.
    pub(crate) incomplete: usize,
    /// Checks skipped because nobody watched.
    pub(crate) skipped: usize,
}

impl Counts {
    /// Checks answered NO / FAIL.
    pub(crate) const fn failed(&self) -> usize {
        self.answered - self.passed
    }
}

/// The question a check shows, and the instructions the table records, for
/// what the test passed to `verify` (`None` for Python's call with no
/// argument).
///
/// No argument means `default`; text with no `?` becomes `TEXT <> Approve?`;
/// text with a `?` is the question as it stands; the empty text asks
/// `Approve?`.
pub(crate) fn question_for(instructions: Option<&str>, default: &str) -> (String, String) {
    let instructions = instructions.unwrap_or(default);
    let question = if instructions.is_empty() {
        "Approve?".to_owned()
    } else if instructions.contains('?') {
        instructions.to_owned()
    } else {
        format!("{instructions} <> Approve?")
    };
    (instructions.to_owned(), question)
}

/// What the run prints at the end: how many checks were skipped (where nobody
/// watched), then the verdict, as Python prints it.
pub(crate) fn results_text(counts: &Counts, nobody_watched: bool) -> String {
    let mut text = String::new();
    if nobody_watched && counts.skipped > 0 {
        text.push_str(&format!(
            "WARNING: {} manual checks were skipped! (MasterQA)\n",
            counts.skipped
        ));
    }
    if !nobody_watched {
        text.push('\n');
    }
    text.push_str("\n*** MasterQA Manual Test Results: ***\n");
    let mut perfection = true;
    if counts.failed() > 0 {
        text.push_str("WARNING: Not all tests passed manual inspection!\n");
        perfection = false;
    }
    if counts.incomplete > 0 {
        text.push_str("WARNING: Not all tests finished running!\n");
        perfection = false;
    }
    if perfection {
        text.push_str(if counts.answered > 0 {
            "SUCCESS: Everything checks out OKAY!\n"
        } else {
            "WARNING: No manual checks were performed!\n"
        });
    }
    text
}

/// `text` as HTML text or as a quoted attribute value.
fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// The address shown for `url` in the failure table.
fn shown_url(url: &str) -> String {
    if url.chars().count() > LONGEST_SHOWN_URL {
        let head: String = url.chars().take(SHOWN_URL_LEN).collect();
        format!("{head}...")
    } else {
        url.to_owned()
    }
}

/// The style of the page, in the colours of Python's report style sheet.
const STYLE: &str = "<meta charset=\"utf-8\"><title>Test Report</title><style type=\"text/css\">\
html { background-color: #9988ad; }\
html, body { font-size: 100%; box-sizing: border-box; }\
body { background-color: #c6d6f0; padding: 30; margin: 10; \
font-family: \"Proxima Nova\",\"proxima-nova\",\"Helvetica Neue\",Helvetica,Arial,sans-serif; \
box-shadow: 0px 2px 5px 1px rgba(0, 0, 0, 0.24), 1px 2px 12px 0px rgba(0, 0, 0, 0.18); }\
table { width: 100%; border-collapse: collapse; border-spacing: 0; \
box-shadow: 0px 2px 5px 1px rgba(0, 0, 0, 0.27), 1px 2px 12px 0px rgba(0, 0, 0, 0.21); }\
thead th, thead td { padding: 0.5rem 0.625rem 0.625rem; font-weight: bold; text-align: left; }\
thead { text-align: center; border: 1px solid #e1e1e1; color: #0C8CDF; background-color: #c0f0ff; }\
tbody tr:nth-child(even) { background-color: #f1f1f1; }\
tbody tr:nth-child(odd) { background-color: #ffffff; }\
tbody tr:nth-child(even):hover { background-color: #f8f8d2; }\
tbody tr:nth-child(odd):hover { background-color: #ffffe0; }\
tbody th, tbody td { padding: 0.5rem 0.625rem 0.625rem; }\
tbody { border: 1px solid #e1e1e1; background-color: #fefefe; }\
td { padding: 5px 5px 5px 0; vertical-align: top; }\
h1 table { font-size: 27px; text-align: left; font-weight: bold; padding: 15px 15px 15px 0; }\
h2 table { color: #0C8CDF; font-size: 16px; text-align: left; font-weight: bold; \
padding: 5px 5px 5px 0; padding-right: 10px; padding-left: 20px; }\
</style>";

/// The `report.html` page of a finished run.
///
/// It sums the checks up (passed, failed, total, incomplete runs), links the
/// archive folder and `results_table.csv`, and lists each failed check and
/// error with its screenshot and the address it happened at. `archive_dir` is
/// the absolute folder the run was moved to.
pub(crate) fn results_page(counts: &Counts, archive_dir: &Path, rows: &[Row]) -> String {
    let failed = counts.failed();
    let failed_color = if failed > 0 { BAD } else { GOOD };
    let incomplete_color = if counts.incomplete > 0 { BAD } else { GOOD };
    let summary = format!(
        "<h1 id=\"ContextHeader\" class=\"sectionHeader\" title=\"\"><div><table><thead><tr>\
         <th>TESTING SUMMARY</th><th>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;</th>\
         </tr></thead><tbody>\
         <tr style=\"color:#00BB00\"><td>CHECKS PASSED: <td>{passed}</tr>\
         <tr style=\"color:{failed_color}\"><td>CHECKS FAILED: <td>{failed}</tr>\
         <tr style=\"color:#4D4DDD\"><td>TOTAL VERIFICATIONS: <td>{answered}</tr>\
         <tr style=\"color:{incomplete_color}\"><td>INCOMPLETE TEST RUNS: <td>{incomplete}</tr>\
         </tbody></table></h1>",
        passed = counts.passed,
        answered = counts.answered,
        incomplete = counts.incomplete,
    );

    let folder = archive_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let archive_url = format!("file://{}", archive_dir.display());
    let links = format!(
        "<p><p><p><p><h2><table><tbody>\
         <tr><td>LOG FILES LINK:&nbsp;&nbsp;<td><a href=\"{archive}\">../archived_reports/{folder}</a></tr>\
         <tr><td>RESULTS TABLE:&nbsp;&nbsp;<td><a href=\"{archive}/{table}\">{table}</a></tr>\
         </tbody></table></h2><p><p><p><p>",
        archive = escape_html(&archive_url),
        folder = escape_html(&folder),
        table = RESULTS_TABLE,
    );

    let mut failures = String::from("<h2><table>");
    let mut any = false;
    for row in rows.iter().filter(|row| row.verdict != Verdict::Success) {
        if !any {
            any = true;
            failures.push_str(
                "<thead><tr><th>SCREENSHOT FILE&nbsp;&nbsp;&nbsp;&nbsp;</th>\
                 <th>LOCATION OF FAILURE</th></tr></thead><tbody>",
            );
        }
        failures.push_str(&format!(
            "<tr><td><a href=\"{archive}/{shot}\">{shot}</a>&nbsp;&nbsp;&nbsp;&nbsp;\
             <td><a href=\"{url}\">{shown}</a></tr>\n",
            archive = escape_html(&archive_url),
            shot = escape_html(&row.screenshot),
            url = escape_html(&row.url),
            shown = escape_html(&shown_url(&row.url)),
        ));
    }
    if !any {
        failures.push_str("<tbody>");
    }
    failures.push_str("</tbody></table></h2>");

    format!("<html><head>{STYLE}</head><body>{summary}{links}{failures}</body></html>")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "Does the page look good?";

    fn row(number: &str, verdict: Verdict, shot: &str, url: &str, info: &str) -> Row {
        Row {
            number: number.to_owned(),
            verdict,
            screenshot: shot.to_owned(),
            url: url.to_owned(),
            browser: "chrome".to_owned(),
            epoch: 1_700_000_000,
            instructions: "Is it blue?".to_owned(),
            info: info.to_owned(),
        }
    }

    #[test]
    fn the_question_follows_python_s_rules() {
        let question = |instructions| question_for(instructions, DEFAULT);
        // No argument: the default message, which has a question mark.
        assert_eq!(question(None), (DEFAULT.to_owned(), DEFAULT.to_owned()));
        // Text without a question mark is asked to be approved.
        assert_eq!(
            question(Some("The logo is blue")),
            (
                "The logo is blue".to_owned(),
                "The logo is blue <> Approve?".to_owned()
            )
        );
        // Text with one is the question as it is.
        assert_eq!(
            question(Some("Is the logo blue?")),
            (
                "Is the logo blue?".to_owned(),
                "Is the logo blue?".to_owned()
            )
        );
        // The empty text asks only to approve.
        assert_eq!(question(Some("")), (String::new(), "Approve?".to_owned()));
    }

    #[test]
    fn the_header_is_python_s_byte_for_byte() {
        assert_eq!(
            CSV_HEADER,
            r#""Num","Result","Screenshot","URL","Browser","Epoch Time","Verification Instructions","Additional Info""#
        );
    }

    #[test]
    fn the_rows_are_python_s_lines() {
        let success = row("1", Verdict::Success, "-", "https://a.test/", "*");
        assert_eq!(
            success.csv_line(),
            r#""1","Success","-","https://a.test/","chrome","1700000000","Is it blue?","*""#
        );
        let failure = row(
            "2",
            Verdict::Failed,
            "failed_check_2.png",
            "https://a.test/b",
            "*",
        );
        assert_eq!(
            failure.csv_line(),
            r#""2","FAILED!","failed_check_2.png","https://a.test/b","chrome","1700000000","Is it blue?","*""#
        );
        let error = Row {
            instructions: "-".to_owned(),
            ..row(
                "ERR",
                Verdict::Error,
                "automation_failure_1.png",
                "https://a.test/",
                "boom",
            )
        };
        assert_eq!(
            error.csv_line(),
            r#""ERR","ERROR!","automation_failure_1.png","https://a.test/","chrome","1700000000","-","boom""#
        );
    }

    #[test]
    fn a_double_quote_in_a_field_cannot_break_the_line() {
        let mut quoted = row("1", Verdict::Success, "-", "https://a.test/", "*");
        quoted.instructions = r#"Is "it" blue?"#.to_owned();
        assert!(
            quoted.csv_line().contains(r#","Is ""it"" blue?","*""#),
            "{}",
            quoted.csv_line()
        );
    }

    #[test]
    fn the_table_is_the_header_and_a_line_per_row() {
        let rows = [
            row("1", Verdict::Success, "-", "https://a.test/", "*"),
            row(
                "2",
                Verdict::Failed,
                "failed_check_2.png",
                "https://a.test/",
                "*",
            ),
        ];
        let table = results_table(&rows);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], CSV_HEADER);
        assert!(lines[2].starts_with(r#""2","FAILED!""#));
        assert!(table.ends_with('\n'));
        assert_eq!(results_table(&[]), format!("{CSV_HEADER}\n"));
    }

    #[test]
    fn the_run_prints_python_s_verdicts() {
        let ok = Counts {
            answered: 2,
            passed: 2,
            ..Counts::default()
        };
        assert_eq!(
            results_text(&ok, false),
            "\n\n*** MasterQA Manual Test Results: ***\nSUCCESS: Everything checks out OKAY!\n"
        );
        let none = Counts::default();
        assert!(results_text(&none, false).ends_with("WARNING: No manual checks were performed!\n"));

        let bad = Counts {
            answered: 2,
            passed: 1,
            incomplete: 1,
            skipped: 0,
        };
        let text = results_text(&bad, false);
        assert!(text.contains("WARNING: Not all tests passed manual inspection!\n"));
        assert!(text.contains("WARNING: Not all tests finished running!\n"));
        assert!(!text.contains("SUCCESS"));
    }

    #[test]
    fn a_run_nobody_watched_says_how_many_checks_were_skipped() {
        let skipped = Counts {
            skipped: 3,
            ..Counts::default()
        };
        let text = results_text(&skipped, true);
        assert!(text.starts_with("WARNING: 3 manual checks were skipped! (MasterQA)\n"));
        assert!(text.contains("*** MasterQA Manual Test Results: ***"));
        // A watched run has no such line.
        assert!(!results_text(&skipped, false).contains("were skipped"));
    }

    #[test]
    fn the_page_sums_the_run_up_and_lists_only_the_failures() {
        let rows = [
            row("1", Verdict::Success, "-", "https://a.test/one", "*"),
            row(
                "2",
                Verdict::Failed,
                "failed_check_2.png",
                "https://a.test/two",
                "*",
            ),
        ];
        let counts = Counts {
            answered: 2,
            passed: 1,
            incomplete: 0,
            skipped: 0,
        };
        let page = results_page(&counts, Path::new("/runs/archived_reports/log_17"), &rows);
        assert!(page.contains("CHECKS PASSED: <td>1</tr>"), "{page}");
        assert!(
            page.contains(r##"<tr style="color:#EE3A3A"><td>CHECKS FAILED: <td>1</tr>"##),
            "{page}"
        );
        assert!(page.contains("TOTAL VERIFICATIONS: <td>2</tr>"));
        assert!(
            page.contains(r##"<tr style="color:#11BB11"><td>INCOMPLETE TEST RUNS: <td>0</tr>"##),
            "{page}"
        );
        assert!(page.contains("../archived_reports/log_17"));
        assert!(page.contains(r#"href="file:///runs/archived_reports/log_17/results_table.csv""#));
        assert!(page.contains(r#"href="file:///runs/archived_reports/log_17/failed_check_2.png""#));
        assert!(page.contains(r#"<a href="https://a.test/two">https://a.test/two</a>"#));
        assert!(!page.contains("https://a.test/one"));
    }

    #[test]
    fn a_page_without_failures_has_no_failure_table_and_an_error_is_one() {
        let ok = Counts {
            answered: 1,
            passed: 1,
            ..Counts::default()
        };
        let rows = [row("1", Verdict::Success, "-", "https://a.test/", "*")];
        let page = results_page(&ok, Path::new("/x/log_1"), &rows);
        assert!(!page.contains("SCREENSHOT FILE"));

        let errored = Counts {
            incomplete: 1,
            ..Counts::default()
        };
        let rows = [row(
            "ERR",
            Verdict::Error,
            "automation_failure_1.png",
            "https://a.test/",
            "boom",
        )];
        let page = results_page(&errored, Path::new("/x/log_1"), &rows);
        assert!(page.contains("SCREENSHOT FILE"));
        assert!(page.contains("automation_failure_1.png"));
        assert!(
            page.contains(r##"<tr style="color:#EE3A3A"><td>INCOMPLETE TEST RUNS: <td>1</tr>"##)
        );
    }

    #[test]
    fn a_long_address_is_cut_and_nothing_breaks_out_of_the_page() {
        let long = format!("https://a.test/{}\"><script>", "x".repeat(70));
        let rows = [row("1", Verdict::Failed, "failed_check_1.png", &long, "*")];
        let counts = Counts {
            answered: 1,
            ..Counts::default()
        };
        let page = results_page(&counts, Path::new("/x/log_1"), &rows);
        assert!(!page.contains("<script>"), "{page}");
        assert!(page.contains("&lt;script&gt;") || page.contains("&quot;&gt;&lt;script"));
        let shown = shown_url(&long);
        assert_eq!(shown.chars().count(), SHOWN_URL_LEN + 3);
        assert!(shown.ends_with("..."));
        assert_eq!(shown_url("https://a.test/"), "https://a.test/");
    }
}
