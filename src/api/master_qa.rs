//! MasterQA: a person verifies pages by eye while an automated test drives
//! the browser.
//!
//! [`MasterQA`] wraps a [`BaseCase`]. [`MasterQA::verify`] pops up a
//! jquery-confirm box with a question, the person answers YES / PASS or
//! NO / FAIL, and the answer goes into a results table. A failed check keeps a
//! screenshot of the page and does not fail the test. When the test is over,
//! [`MasterQA::finish`] writes `results_table.csv` and `report.html` and moves
//! the run to `archived_reports/log_<epoch>`:
//!
//! ```no_run
//! # use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};
//! # use seleniumbase_rs::api::master_qa::MasterQA;
//! # async fn demo() -> Result<(), SeleniumBaseError> {
//! let sb = BaseCase::new(BrowserConfig::default().with_headless(false)).await?;
//! let mut qa = MasterQA::new(sb)?;
//! qa.open("https://example.com").await?; // a `BaseCase` method, through `Deref`
//! qa.verify("Is the heading visible?").await?;
//! qa.verify_default().await?; // asks "Does the page look good?"
//! let (mut sb, report) = qa.finish(None).await?;
//! println!("{} of {} checks passed", report.passed, report.checks);
//! sb.quit().await?;
//! # Ok(()) }
//! ```
//!
//! Where nobody watches (a headless browser or a virtual display) no box
//! opens: [`MasterQA::verify`] prints a warning once and returns `Ok(None)`.
//! The delay before each box is the run settings'
//! [`verify_delay`](crate::api::run_settings::RunSettings::verify_delay)
//! (`--verify-delay`).
//!
//! Without a browser, [`MasterQaSession`] asks the same kind of question in the
//! terminal and writes a Markdown report.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::api::base_case::BaseCase;
use crate::artifacts::{archive_latest_report, ensure_latest_report_dir};
use crate::error::SeleniumBaseError;

mod report;
#[cfg(test)]
mod wrapper_tests;

use report::{Counts, Row, Verdict, RESULTS_PAGE, RESULTS_TABLE};

/// The question `verify_default` asks (Python's
/// `MASTERQA_DEFAULT_VALIDATION_MESSAGE`).
pub const DEFAULT_VALIDATION_MESSAGE: &str = "Does the page look good?";

/// The words before the number of a check in the title of its box.
pub const DEFAULT_VALIDATION_TITLE: &str = "Manual Check";

/// How long a `confirm` box (the fallback when jquery-confirm cannot load)
/// waits for a person before the browser is quit.
pub const MAX_IDLE_TIME_BEFORE_QUIT: Duration = Duration::from_secs(600);

/// How long the results page stays open when it closes itself.
const RESULTS_PAGE_SHOWN: Duration = Duration::from_secs(1);

/// How often the open results page is asked whether its window is still there.
const RESULTS_PAGE_POLL: Duration = Duration::from_millis(100);

use std::io::{self, BufRead, Write};
use std::path::Path;

/// A single manual verification step.
#[derive(Clone, Debug)]
pub struct QaStep {
    pub id: usize,
    pub question: String,
    pub passed: bool,
    pub notes: Option<String>,
}

/// Stateful manual QA session that records verification steps.
#[derive(Clone, Debug, Default)]
pub struct MasterQaSession {
    pub title: String,
    pub steps: Vec<QaStep>,
}

impl MasterQaSession {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            steps: Vec::new(),
        }
    }

    /// Prompts the user to verify a step and records the result.
    pub fn verify(&mut self, question: &str) -> bool {
        let passed = Self::prompt(question);
        self.steps.push(QaStep {
            id: self.steps.len() + 1,
            question: question.to_owned(),
            passed,
            notes: None,
        });
        passed
    }

    /// Prompts the user for a verification step with optional notes.
    pub fn verify_with_notes(&mut self, question: &str) -> bool {
        let passed = Self::prompt(question);
        let notes = if passed {
            None
        } else {
            println!("Enter failure notes (optional):");
            Self::read_line(&mut io::stdin().lock()).filter(|notes| !notes.is_empty())
        };
        self.steps.push(QaStep {
            id: self.steps.len() + 1,
            question: question.to_owned(),
            passed,
            notes,
        });
        passed
    }

    pub fn all_passed(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.passed)
    }

    pub fn passed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.passed).count()
    }

    /// Generates a Markdown test-case management report.
    pub fn to_markdown(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        md.push_str("| Step | Question | Result | Notes |\n");
        md.push_str("|------|----------|--------|-------|\n");
        for step in &self.steps {
            let result = if step.passed { "PASS" } else { "FAIL" };
            let notes = step.notes.as_deref().unwrap_or("-");
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                step.id,
                table_cell(&step.question),
                result,
                table_cell(notes)
            ));
        }
        md.push_str(&format!(
            "\n**Summary:** {} / {} steps passed.\n",
            self.passed_count(),
            self.steps.len()
        ));
        md
    }

    /// Writes the Markdown report to a file.
    pub fn save_markdown<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<std::path::PathBuf, crate::error::SeleniumBaseError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_markdown())?;
        Ok(path.to_owned())
    }

    fn prompt(question: &str) -> bool {
        print!("Manual QA verification required: {} [Y/n] ", question);
        let _ = io::stdout().flush();
        let passed = Self::answer(&mut io::stdin().lock());
        if passed.is_none() {
            println!("\nNo answer was given (input is closed); recording the step as failed.");
        }
        passed.unwrap_or(false)
    }

    /// Reads one answer: Enter or anything but `n` / `no` means yes, as the
    /// `[Y/n]` prompt says. `None` when there was no answer at all, because
    /// input is closed, so an unattended run cannot pass its manual checks.
    fn answer(input: &mut impl BufRead) -> Option<bool> {
        let line = Self::read_line(input)?.to_lowercase();
        Some(!(line == "n" || line == "no"))
    }

    /// One trimmed line, or `None` at end of input or on a read error.
    fn read_line(input: &mut impl BufRead) -> Option<String> {
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_owned()),
        }
    }
}

/// `text` made safe for one cell of a Markdown table.
fn table_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// A [`BaseCase`] with a person who verifies what the test shows: Python's
/// `MasterQA` class, as a wrapper instead of a subclass.
///
/// It dereferences to the case, so every `BaseCase` method is called on it
/// directly. See the [module documentation](self) for an example.
pub struct MasterQA {
    sb: BaseCase,
    /// One row per answered check, and one for an error, in order.
    rows: Vec<Row>,
    counts: Counts,
    auto_close_results: bool,
}

/// What a finished [`MasterQA`] run came to, and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasterQaReport {
    /// Checks a person answered (the report's TOTAL VERIFICATIONS).
    pub checks: usize,
    /// Checks answered YES / PASS.
    pub passed: usize,
    /// Checks answered NO / FAIL.
    pub failed: usize,
    /// Checks skipped because nobody watched the browser.
    pub skipped: usize,
    /// Errors that ended the test early (0 or 1).
    pub incomplete_runs: usize,
    /// `report.html` in `latest_report`, which stays there. The archive
    /// folder holds a copy.
    pub results_page: PathBuf,
    /// `results_table.csv`, in the archive folder.
    pub results_table: PathBuf,
    /// `archived_reports/log_<epoch>`, where the run's files were moved: the
    /// table, the page and the screenshots of the failures.
    pub archive_dir: PathBuf,
}

impl MasterQA {
    /// Starts a MasterQA run on `sb`, as Python's `setUp` does: the old results
    /// in `latest_report` (the screenshots of failed checks, `report.html` and
    /// `results_table.csv`) are deleted. Where nobody watches the browser, the
    /// results page closes itself.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the `latest_report` folder cannot be
    /// made or cleaned.
    pub fn new(sb: BaseCase) -> Result<Self, SeleniumBaseError> {
        let latest = ensure_latest_report_dir()?;
        clear_old_results(&latest)?;
        let auto_close_results = sb.nobody_watches();
        Ok(Self {
            sb,
            rows: Vec::new(),
            counts: Counts::default(),
            auto_close_results,
        })
    }

    /// Asks the person `instructions` and records the answer, as Python's
    /// `verify("...")` does.
    ///
    /// A text with a `?` is the question as it stands; one without becomes
    /// `TEXT <> Approve?`; the empty text asks `Approve?`. The call waits for
    /// the run settings' [`verify_delay`](crate::api::run_settings::RunSettings::verify_delay)
    /// (500 ms unless `--verify-delay` says otherwise) so the person can see
    /// the page, shows the box, and waits for the answer for as long as it
    /// takes. A check answered NO / FAIL saves `failed_check_N.png` in
    /// `latest_report`; it does not fail the test.
    ///
    /// Returns `Some(true)` for YES / PASS and `Some(false)` for NO / FAIL, and
    /// `None` where nobody watches (a headless browser or a virtual display):
    /// there the check is skipped, and the first one prints a warning.
    ///
    /// # Errors
    ///
    /// Returns an error if the page cannot be read or captured, or, when the
    /// browser's own `confirm` box has to be used, if nobody answers it within
    /// [`MAX_IDLE_TIME_BEFORE_QUIT`] (the browser is quit then).
    pub async fn verify(&mut self, instructions: &str) -> Result<Option<bool>, SeleniumBaseError> {
        self.check(Some(instructions)).await
    }

    /// Asks the default question, "Does the page look good?" (Python's
    /// `verify()` with no argument). See [`verify`](Self::verify).
    ///
    /// # Errors
    ///
    /// As for [`verify`](Self::verify).
    pub async fn verify_default(&mut self) -> Result<Option<bool>, SeleniumBaseError> {
        self.check(None).await
    }

    /// Makes the results page close by itself at the end of the run, one
    /// second after it opens, instead of waiting for the person to close it.
    pub fn auto_close_results(&mut self) {
        self.auto_close_results = true;
    }

    /// Ends the run, as Python's `tearDown` does, minus quitting the browser.
    ///
    /// With an `error` (the one that stopped the test) an `ERR` row and
    /// `automation_failure_N.png` are added. The results are printed;
    /// `results_table.csv` is written to `latest_report`, which is then moved
    /// to `archived_reports/log_<epoch>`; `report.html` is written to the new,
    /// empty `latest_report` and copied to the archive; and the archived page
    /// is opened. Unless [`auto_close_results`](Self::auto_close_results) was
    /// called (or nobody watches), the call returns when the person has closed
    /// the browser window.
    ///
    /// Returns the case, so the caller can quit it, and the report.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if a file or folder of the report
    /// cannot be written or moved.
    pub async fn finish(
        mut self,
        error: Option<&SeleniumBaseError>,
    ) -> Result<(BaseCase, MasterQaReport), SeleniumBaseError> {
        if let Some(error) = error {
            self.add_failure(error).await?;
        }
        print!(
            "{}",
            report::results_text(&self.counts, self.sb.nobody_watches())
        );

        let latest = ensure_latest_report_dir()?;
        std::fs::write(
            latest.join(RESULTS_TABLE),
            report::results_table(&self.rows),
        )?;
        let archive_dir = std::path::absolute(archive_latest_report()?)?;

        let page = report::results_page(&self.counts, &archive_dir, &self.rows);
        let results_page = std::path::absolute(ensure_latest_report_dir()?)?.join(RESULTS_PAGE);
        std::fs::write(&results_page, &page)?;
        let archived_page = archive_dir.join(RESULTS_PAGE);
        std::fs::copy(&results_page, &archived_page)?;
        if self.counts.answered > 0 {
            println!(
                "\n*** The manual test report is located at:\n{}",
                results_page.display()
            );
        }
        self.show_results(&archived_page).await;

        let report = MasterQaReport {
            checks: self.counts.answered,
            passed: self.counts.passed,
            failed: self.counts.failed(),
            skipped: self.counts.skipped,
            incomplete_runs: self.counts.incomplete,
            results_page,
            results_table: archive_dir.join(RESULTS_TABLE),
            archive_dir,
        };
        Ok((self.sb, report))
    }

    /// One call of `verify`.
    async fn check(
        &mut self,
        instructions: Option<&str>,
    ) -> Result<Option<bool>, SeleniumBaseError> {
        if self.sb.nobody_watches() {
            self.counts.skipped += 1;
            if self.counts.skipped == 1 {
                println!("\nWARNING: MasterQA skips manual checks in headless mode!");
            }
            return Ok(None);
        }
        let (instructions, question) =
            report::question_for(instructions, DEFAULT_VALIDATION_MESSAGE);
        // Let the person see the whole page before the box covers it.
        tokio::time::sleep(self.sb.run_settings().verify_delay()).await;

        let number = self.counts.answered + 1;
        let passed = self.sb.masterqa_ask(number, &question).await?;
        self.counts.answered += 1;
        let mut row = Row {
            number: number.to_string(),
            verdict: Verdict::Success,
            screenshot: "-".to_owned(),
            url: self.sb.masterqa_page_url().await,
            browser: self.sb.masterqa_browser().to_owned(),
            epoch: unix_seconds(),
            instructions,
            info: "*".to_owned(),
        };
        if passed {
            self.counts.passed += 1;
            self.rows.push(row);
        } else {
            let screenshot = format!("failed_check_{number}.png");
            row.verdict = Verdict::Failed;
            row.screenshot.clone_from(&screenshot);
            self.rows.push(row);
            let path = ensure_latest_report_dir()?.join(screenshot);
            self.sb.save_screenshot_to_path(path).await?;
        }
        Ok(Some(passed))
    }

    /// Records that the test ended with `error`: a screenshot and an `ERR` row.
    async fn add_failure(&mut self, error: &SeleniumBaseError) -> Result<(), SeleniumBaseError> {
        self.counts.incomplete += 1;
        let screenshot = format!("automation_failure_{}.png", self.counts.incomplete);
        // The test failed already; a page that cannot be captured is not news.
        let _ = self
            .sb
            .save_screenshot_to_path(ensure_latest_report_dir()?.join(&screenshot))
            .await;
        self.rows.push(Row {
            number: "ERR".to_owned(),
            verdict: Verdict::Error,
            screenshot,
            url: self.sb.masterqa_page_url().await,
            browser: self.sb.masterqa_browser().to_owned(),
            epoch: unix_seconds(),
            instructions: "-".to_owned(),
            info: error.to_string(),
        });
        // Back to the first window if the test opened another.
        self.sb.masterqa_close_extra_window().await;
        Ok(())
    }

    /// Opens the results page, and leaves it open for a second or until the
    /// person has closed the browser window.
    async fn show_results(&mut self, page: &Path) {
        let url = format!("file://{}", page.display());
        if let Err(error) = self.sb.open(&url).await {
            eprintln!("MasterQA could not show the results page: {error}");
            return;
        }
        if self.auto_close_results {
            // Long enough to notice the results before closing the page.
            tokio::time::sleep(RESULTS_PAGE_SHOWN).await;
            return;
        }
        println!("\n*** Close the html report window to continue ***");
        while matches!(self.sb.get_all_window_handles().await, Ok(handles) if !handles.is_empty()) {
            tokio::time::sleep(RESULTS_PAGE_POLL).await;
        }
    }
}

impl Deref for MasterQA {
    type Target = BaseCase;

    /// The case the checks are made on, so every `BaseCase` method works on a
    /// `MasterQA`.
    fn deref(&self) -> &BaseCase {
        &self.sb
    }
}

impl DerefMut for MasterQA {
    fn deref_mut(&mut self) -> &mut BaseCase {
        &mut self.sb
    }
}

impl std::fmt::Debug for MasterQA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasterQA")
            .field("answered", &self.counts.answered)
            .field("passed", &self.counts.passed)
            .field("skipped", &self.counts.skipped)
            .field("incomplete_runs", &self.counts.incomplete)
            .field("auto_close_results", &self.auto_close_results)
            .finish_non_exhaustive()
    }
}

/// Seconds since the epoch.
fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Deletes what an earlier run left in `latest`: the screenshots of failed
/// checks and of errors, the results page and the results table.
fn clear_old_results(latest: &Path) -> Result<(), SeleniumBaseError> {
    for entry in std::fs::read_dir(latest)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let old = name.starts_with("failed_")
            || name.starts_with("automation_failure")
            || name == RESULTS_PAGE
            || name == RESULTS_TABLE;
        if old && entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn answer(text: &str) -> Option<bool> {
        MasterQaSession::answer(&mut Cursor::new(text.as_bytes()))
    }

    #[test]
    fn enter_and_yes_pass_and_no_fails() {
        assert_eq!(answer("\n"), Some(true));
        assert_eq!(answer("y\n"), Some(true));
        assert_eq!(answer("yes\n"), Some(true));
        assert_eq!(answer("n\n"), Some(false));
        assert_eq!(answer(" No \n"), Some(false));
    }

    #[test]
    fn closed_input_is_no_answer_and_not_a_pass() {
        assert_eq!(answer(""), None);
    }

    #[test]
    fn a_table_cell_cannot_break_the_table() {
        assert_eq!(table_cell("a | b\nc"), "a \\| b c");
        let mut session = MasterQaSession::new("T");
        session.steps.push(QaStep {
            id: 1,
            question: "is a|b shown?".into(),
            passed: false,
            notes: Some("line 1\nline 2".into()),
        });
        let md = session.to_markdown();
        let row = md.lines().find(|l| l.starts_with("| 1 ")).unwrap();
        assert_eq!(row, "| 1 | is a\\|b shown? | FAIL | line 1 line 2 |");
    }
}
