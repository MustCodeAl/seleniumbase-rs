//! The session of `sbrs test`: the start and the end of a run that libtest has no hooks for.
//!
//! `cargo test` starts one process per test binary, and none of them knows when the run starts
//! or ends. So `sbrs test` is the one process that does: before Cargo it opens the run
//! (`TestRun::open`, which clears or archives `latest_logs` and writes the first dashboard),
//! gives it to the test processes through `SB_RUN_ID` and `SB_REPORT_ROOT`, and after Cargo it
//! finishes it (`TestRun::finish`: unfinished tests become unreported, the reports are written
//! a last time, the logs are archived when asked, and the results go to the database).
//! `sbrs report finish` ends a run the same way when the tests were started by plain
//! `cargo test`.
//!
//! What is printed follows pytest's terminal summary: `Dashboard: file://...` and a line of
//! `*` when the run starts, and the `----- ... -----` lines when it ends.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use seleniumbase_rs::core::test_run::{
    FinalizeOutcome, ReportSettings, RunFinish, RunKey, TestRun, DASHBOARD_FILE,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::SeleniumBaseError;

/// The variable that names the run the test processes join; `sbrs test` gives it to Cargo.
pub(crate) const RUN_ID_VARIABLE: &str = "SB_RUN_ID";
/// The variable that names the folder the run's files are under.
pub(crate) const ROOT_VARIABLE: &str = "SB_REPORT_ROOT";

/// A run id nobody else has: the process, and the time in milliseconds.
pub(crate) fn new_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    format!("sbrs-{}-{millis}", std::process::id())
}

/// `path` as a `file://` URL: a space, `#`, `?` and `%` are written as escapes, and a
/// Windows path gets its slashes.
pub(crate) fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut url = String::from("file://");
    if !text.starts_with('/') && !text.starts_with('\\') {
        url.push('/');
    }
    for ch in text.chars() {
        match ch {
            '\\' => url.push('/'),
            ' ' => url.push_str("%20"),
            '#' => url.push_str("%23"),
            '?' => url.push_str("%3F"),
            '%' => url.push_str("%25"),
            other => url.push(other),
        }
    }
    url
}

/// The width of the terminal, when `COLUMNS` says.
fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|width| *width > 0)
}

/// The run that `sbrs test` opens before Cargo and finishes after it.
pub(crate) struct Session {
    run: Arc<TestRun>,
    root: PathBuf,
    settings: ReportSettings,
}

impl Session {
    /// Opens the run `run_id` under `root`, which clears or archives `latest_logs` there.
    ///
    /// # Errors
    ///
    /// Returns the library's error when the run's folder cannot be used, or when
    /// `latest_logs` is a symbolic link (nothing is deleted then).
    pub(crate) fn start(
        root: PathBuf,
        run_id: String,
        settings: ReportSettings,
    ) -> Result<Self, SeleniumBaseError> {
        // This run records no test itself, so it needs no Allure sink: the processes of the
        // tests write the results, from `SB_ALLURE_DIR`. Leaving the folder out here also
        // keeps the run from warning that *sbrs* was built without the `allure` feature,
        // which says nothing about the crate that is tested.
        let own = ReportSettings {
            allure_dir: None,
            ..settings.clone()
        };
        let run = TestRun::open(root.clone(), own, RunKey::new(run_id))?;
        Ok(Self {
            run,
            root,
            settings,
        })
    }

    /// `Dashboard: file://...` and a line of `*` as long as that text, when the run has a
    /// dashboard.
    pub(crate) fn banner(&self) -> Option<String> {
        if !self.settings.dashboard {
            return None;
        }
        let line = format!("Dashboard: {}", file_url(&self.root.join(DASHBOARD_FILE)));
        let width = line.chars().count();
        let stars = terminal_width().map_or(width, |columns| columns.min(width));
        Some(format!("{line}\n{}", "*".repeat(stars)))
    }

    /// Ends the run: the reports are written a last time and the database gets the results.
    /// What went wrong is in the [`Closing`], never an error: a report that cannot be written
    /// does not change what the tests did.
    pub(crate) async fn finish(self) -> Closing {
        match self.run.finish().await {
            Ok(finish) => Closing::of(&self.settings, &finish),
            Err(error) => Closing::failed(&format!("the run could not be finished: {error}")),
        }
    }
}

/// What ending a run has to say.
#[derive(Debug, Default)]
pub(crate) struct Closing {
    /// The `----- ... -----` lines, for standard output.
    pub(crate) lines: Vec<String>,
    /// What could not be done, for standard error.
    pub(crate) warnings: Vec<String>,
    /// A report that was asked for and could not be written.
    pub(crate) report_missing: bool,
    /// The page `--show-report` opens: the HTML report, else the dashboard.
    pub(crate) page: Option<PathBuf>,
}

impl Closing {
    fn failed(message: &str) -> Self {
        Self {
            warnings: vec![message.to_owned()],
            report_missing: true,
            ..Self::default()
        }
    }

    /// The summary of a finished run, whose reports `settings` asked for.
    pub(crate) fn of(settings: &ReportSettings, finish: &RunFinish) -> Self {
        Self::from_outcome(settings, &finish.outcome)
    }

    pub(crate) fn from_outcome(settings: &ReportSettings, outcome: &FinalizeOutcome) -> Self {
        let summary = &outcome.summary;
        let failed = summary.failed > 0 || summary.unreported > 0;
        let mut lines = Vec::new();
        if failed {
            if let Some(dashboard) = &outcome.dashboard {
                lines.push(format!("----- Dashboard = {} -----", file_url(dashboard)));
            }
        }
        if let Some(html) = &outcome.html {
            lines.push(format!(
                "----- Generated html report: {} -----",
                file_url(html)
            ));
        }
        if let Some(xml) = &outcome.junit_xml {
            lines.push(format!("----- generated xml file: {} -----", xml.display()));
        }
        if failed || outcome.logs_dir.is_dir() {
            lines.push(format!(
                "----- Latest Logs dir: {}/ -----",
                outcome.logs_dir.display()
            ));
        }
        let report_missing = (settings.dashboard && outcome.dashboard.is_none())
            || (settings.html.is_some() && outcome.html.is_none())
            || (settings.junit_xml.is_some() && outcome.junit_xml.is_none());
        Self {
            lines,
            warnings: outcome.warnings.clone(),
            report_missing,
            page: outcome.html.clone().or_else(|| outcome.dashboard.clone()),
        }
    }

    /// Prints the lines on standard output and the warnings on standard error.
    pub(crate) fn print(&self) {
        for line in &self.lines {
            println!("{line}");
        }
        for warning in &self.warnings {
            eprintln!("warning: {warning}");
        }
    }
}

/// Opens `page` in a headed browser and waits until its window is closed
/// (`--show-report`).
///
/// # Errors
///
/// Returns the library's error when no browser can be started.
pub(crate) async fn show_report(page: &Path) -> Result<(), SeleniumBaseError> {
    let options = LaunchOptions::builder()
        .headless(false)
        .url(file_url(page))
        .build()?;
    let browser = Browser::launch(options).await?;
    println!("*** Close the html report window to continue. ***");
    while browser.is_connected() {
        match browser.list_pages().await {
            Ok(pages) if !pages.is_empty() => {}
            _ => break,
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // The window is gone, or the browser; a failure to close what is closed is no news.
    let _ = browser.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_url_escapes_what_a_url_cannot_hold() {
        assert_eq!(
            file_url(Path::new("/tmp/my reports/a#b?c%d.html")),
            "file:///tmp/my%20reports/a%23b%3Fc%25d.html"
        );
        assert_eq!(
            file_url(Path::new("C:\\runs\\report.html")),
            "file:///C:/runs/report.html"
        );
    }

    #[test]
    fn a_run_id_names_the_process_and_the_time() {
        let id = new_run_id();
        let mut parts = id.split('-');
        assert_eq!(parts.next(), Some("sbrs"));
        assert_eq!(
            parts.next().and_then(|pid| pid.parse::<u32>().ok()),
            Some(std::process::id())
        );
        assert!(parts.next().is_some_and(|ms| ms.parse::<u128>().is_ok()));
    }

    #[test]
    fn the_banner_is_only_for_a_run_with_a_dashboard() {
        let root = tempfile::tempdir().expect("temp dir");
        let with = ReportSettings {
            dashboard: true,
            ..ReportSettings::default()
        };
        let session =
            Session::start(root.path().to_path_buf(), new_run_id(), with).expect("the run opens");

        let banner = session.banner().expect("a dashboard has a banner");
        let (text, stars) = banner.split_once('\n').expect("two lines");
        assert!(text.starts_with("Dashboard: file://"), "{text}");
        assert!(text.ends_with("/dashboard.html"), "{text}");
        assert!(stars.chars().all(|ch| ch == '*'), "{stars}");
        assert!(stars.len() <= text.len());
        assert!(
            root.path().join("dashboard.html").is_file(),
            "the first dashboard is written when the run opens"
        );

        let without = Session::start(
            tempfile::tempdir().expect("temp dir").path().to_path_buf(),
            new_run_id(),
            ReportSettings::default(),
        )
        .expect("the run opens");
        assert!(without.banner().is_none());
    }
}
