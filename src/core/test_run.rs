//! A test run shared by processes, and the recorder each test writes to it.
//!
//! `cargo test` starts one process per test binary, and `cargo nextest` one per
//! test, and the standard test harness has no hook for the start or the end of
//! a session. So a run is a folder of files that every process appends to,
//! under a file lock:
//!
//! ```text
//! <root>/.sbrs_run/run.json       the run: its key, title, settings and warnings
//! <root>/.sbrs_run/results.jsonl  one `start` or `end` line per test event
//! <root>/.sbrs_run/lock           the lock every read-modify-write takes
//! ```
//!
//! The root is `SB_REPORT_ROOT`, or the current folder. Which processes belong
//! to one run is decided by a [`RunKey`]: `SB_RUN_ID` (what `sbrs test` sets),
//! the id of a nextest run, or the process that started the test binary (the
//! `cargo test` process, on Unix). A process with the key of the run joins it; a
//! process with another key starts a new run, which clears `latest_logs` (or
//! archives it, with `--archive-logs`) like Python's session start does.
//!
//! # How a test uses it
//!
//! ```no_run
//! use seleniumbase_rs::core::log_helper::TestId;
//! use seleniumbase_rs::core::test_run::TestRun;
//!
//! # async fn demo(outcome: seleniumbase_rs::Result<()>) -> seleniumbase_rs::Result<()> {
//! let run = TestRun::current()?;
//! let recorder = run.start(TestId::current())?;
//! // ... the test runs; on a failure, `recorder.on_failure(&error, &page).await` ...
//! recorder.finish(&outcome)?;
//! # Ok(())
//! # }
//! ```
//!
//! Every [`finish`](TestRecorder::finish) rewrites the outputs that are turned on
//! (the dashboard, the HTML report, the JUnit XML), so they are complete after
//! each test. [`TestRun::finalize`] ends the run: tests that started and never
//! finished become unreported, the logs are archived when asked, and the
//! outputs are written a last time. It can be called again.
//!
//! A test that panics or whose process is killed leaves a `start` line and no
//! `end` line. A recorder that is dropped without `finish` writes the missing
//! line itself (failed for a panic, unreported otherwise).

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::artifacts::{self, LATEST_LOGS_DIR};
use crate::config::settings::Settings;
use crate::core::dashboard::{write_dashboard, write_html_report, PageOptions};
use crate::core::junit::write_junit_xml;
use crate::core::log_helper::{
    archive_logs_if_set, clear_empty_logs, log_folder_setup, log_page_source, log_screenshot,
    log_skipped_test_data, log_test_failure_data, FailureInfo, TestId, PAGE_SOURCE_FILE,
    SCREENSHOT_FILE,
};
use crate::core::report_helper::{
    write_atomically, FailureDetails, ReportSummary, RunSnapshot, TestResult, TestStatus,
};
use crate::error::{Result, SeleniumBaseError};
use crate::plugins::observer::PageEvidence;

/// The folder under a run's root where its state is kept.
pub const RUN_DIR: &str = ".sbrs_run";
/// The file name of the dashboard, under the run's root.
pub const DASHBOARD_FILE: &str = "dashboard.html";
/// The title of a dashboard that has none of its own.
pub const DEFAULT_TITLE: &str = "SeleniumBase Dashboard \u{1F4CA}";

const RUN_FILE: &str = "run.json";
const RESULTS_FILE: &str = "results.jsonl";
const LOCK_FILE: &str = "lock";
/// The version of the `run.json` and `results.jsonl` formats.
const FORMAT_VERSION: u32 = 1;
/// How recently a run must have changed for a process with no key to join it.
const RECENT_SECONDS: i64 = 120;
/// How long a run keyed by a process id may sit unchanged and still be joined by
/// a process with that id. Process ids are reused; a test run that quiet is over.
const PPID_REUSE_SECONDS: i64 = 6 * 60 * 60;

/// How long a failing page may take to give its URL, title, screenshot or
/// source; a report must not hang on a page that has.
const PAGE_WAIT: Duration = Duration::from_secs(5);

/// Where a recorder prints (the `=> Fail Page: <url>` line).
pub type Console = Arc<dyn Fn(&str) + Send + Sync>;

fn stderr_console() -> Console {
    Arc::new(|text: &str| {
        // A closed stderr is not worth failing a test over.
        let _ = io::stderr().write_all(text.as_bytes());
    })
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// What a run writes and keeps. It is the `[report]` table of the settings
/// file, completed from the `[test]` table and the `SB_*` variables by
/// [`from_settings`](Self::from_settings), and it is saved in `run.json`.
///
/// | Field | Variable |
/// |---|---|
/// | `dashboard` | `SB_DASHBOARD` |
/// | `dash_title` | `SB_DASH_TITLE` |
/// | `html` | `SB_HTML` |
/// | `junit_xml` | `SB_JUNIT_XML` |
/// | `archive_logs` | `SB_ARCHIVE_LOGS` |
/// | `list_fail_pages` | `SB_LIST_FAIL_PAGE` |
/// | `save_screenshot` | `SB_SAVE_SCREENSHOT` |
/// | `no_screenshot` | `SB_NO_SCREENSHOT` |
/// | `allure_dir` | `SB_ALLURE_DIR` |
/// | `with_db_reporting` | `SB_WITH_DB_REPORTING` |
/// | `db` | `SB_REPORT_DB` |
/// | `database_env` | `SB_DATABASE_ENV` |
///
/// The options that are also options of `sbrs test` (everything but `html`,
/// `junit_xml`, `allure_dir` and `db`) are read from the `[test]` table, where
/// the option table of [`config::test_options`](crate::config::test_options)
/// keeps them; a `[report]` table can turn them on too.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportSettings {
    /// Write the live dashboard (`dashboard.html`).
    pub dashboard: bool,
    /// The dashboard's title; `_` stands for a space.
    pub dash_title: Option<String>,
    /// Write an HTML report to this file.
    pub html: Option<PathBuf>,
    /// Write a JUnit XML report to this file.
    pub junit_xml: Option<PathBuf>,
    /// Keep the logs of the run in `archived_logs/` instead of deleting them.
    pub archive_logs: bool,
    /// Print `=> Fail Page: <url>` after a failing test.
    pub list_fail_pages: bool,
    /// Save a screenshot of every test, not only of failures.
    pub save_screenshot: bool,
    /// Save no screenshot of a failure. It wins over `save_screenshot`.
    pub no_screenshot: bool,
    /// Write Allure results to this folder.
    pub allure_dir: Option<PathBuf>,
    /// Record the run in the results database when it is finalized.
    pub with_db_reporting: bool,
    /// The results database; `reports/results.db` when unset.
    pub db: Option<PathBuf>,
    /// Which results database to use.
    pub database_env: Option<String>,
}

impl ReportSettings {
    /// The settings of a run under `settings`, with the process environment
    /// on top.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        Self::from_settings_with(settings, |name| std::env::var(name).ok())
    }

    /// Like [`from_settings`](Self::from_settings), reading the variables
    /// `SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR` and `SB_REPORT_DB` from `var`.
    /// A blank value counts as unset.
    pub fn from_settings_with(settings: &Settings, var: impl Fn(&str) -> Option<String>) -> Self {
        let table = &settings.report;
        let test = &settings.test;
        let var = |name: &str| var(name).filter(|value| !value.trim().is_empty());
        let path = |name: &str, file: Option<&Path>| {
            var(name)
                .map(PathBuf::from)
                .or_else(|| file.map(Path::to_path_buf))
        };
        Self {
            dashboard: test.dashboard || table.dashboard,
            dash_title: test.dash_title.clone().or_else(|| table.dash_title.clone()),
            html: path("SB_HTML", table.html.as_deref()),
            junit_xml: path("SB_JUNIT_XML", table.junit_xml.as_deref()),
            archive_logs: test.archive_logs || table.archive_logs,
            list_fail_pages: test.list_fail_page || table.list_fail_pages,
            save_screenshot: test.save_screenshot || table.save_screenshot,
            no_screenshot: test.no_screenshot || table.no_screenshot,
            allure_dir: path("SB_ALLURE_DIR", table.allure_dir.as_deref()),
            with_db_reporting: test.with_db_reporting || table.with_db_reporting,
            db: path("SB_REPORT_DB", table.db.as_deref()).or_else(|| {
                // `--database-env` picks one of the databases of the `[test.databases]` table.
                let name = test.database_env.as_ref()?;
                test.databases.get(name).cloned()
            }),
            database_env: test
                .database_env
                .clone()
                .or_else(|| table.database_env.clone()),
        }
    }

    /// The dashboard's title: `dash_title` with its underscores read as
    /// spaces, or [`DEFAULT_TITLE`].
    #[must_use]
    pub fn title(&self) -> String {
        self.dash_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map_or_else(|| DEFAULT_TITLE.to_owned(), |title| title.replace('_', " "))
    }

    /// The results database: `db`, or `reports/results.db`.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.db
            .clone()
            .unwrap_or_else(|| PathBuf::from("reports/results.db"))
    }
}

// ---------------------------------------------------------------------------
// The run key
// ---------------------------------------------------------------------------

/// Which processes belong to one run.
///
/// A process whose key equals the key of the run on disk joins it. A key made
/// by [`recent`](Self::recent) joins a run that is not finished and changed in
/// the last two minutes, which is the best that can be done on a platform where
/// a process cannot tell which process started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunKey(Option<String>);

impl RunKey {
    /// The key `key`.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }

    /// The key that joins a recent run that is not finished.
    #[must_use]
    pub fn recent() -> Self {
        Self(None)
    }

    /// The key of the calling process, from the first of these that applies:
    /// `SB_RUN_ID`, `NEXTEST_RUN_ID`, the id of the parent process (Unix only:
    /// it is the `cargo test` process for every test binary it starts), or
    /// [`recent`](Self::recent).
    #[must_use]
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        Self::from_sources(
            var("SB_RUN_ID").as_deref(),
            var("NEXTEST_RUN_ID").as_deref(),
            parent_process(),
        )
    }

    /// The key from the three sources, in order. A blank value counts as
    /// missing.
    #[must_use]
    pub(crate) fn from_sources(
        sb_run_id: Option<&str>,
        nextest: Option<&str>,
        parent: Option<u32>,
    ) -> Self {
        fn present(value: Option<&str>) -> Option<&str> {
            value.map(str::trim).filter(|v| !v.is_empty())
        }
        if let Some(id) = present(sb_run_id) {
            Self::new(id)
        } else if let Some(id) = present(nextest) {
            Self::new(format!("nextest-{id}"))
        } else if let Some(pid) = parent {
            Self::new(format!("cargo-ppid-{pid}"))
        } else {
            Self::recent()
        }
    }

    /// The key as text, or `None` for [`recent`](Self::recent).
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

#[cfg(unix)]
fn parent_process() -> Option<u32> {
    Some(std::os::unix::process::parent_id())
}

#[cfg(not(unix))]
fn parent_process() -> Option<u32> {
    None
}

// ---------------------------------------------------------------------------
// What is on disk
// ---------------------------------------------------------------------------

/// `run.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunFile {
    v: u32,
    key: String,
    title: String,
    started_at: String,
    updated_at: String,
    finalized_at: Option<String>,
    archived: bool,
    settings: ReportSettings,
    warnings: Vec<String>,
}

/// One line of `results.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Line {
    v: u32,
    #[serde(flatten)]
    event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Event {
    /// A test started.
    Start {
        id: String,
        display: String,
        suite: String,
        pid: u32,
        at: String,
    },
    /// A test ended.
    End {
        id: String,
        result: Box<TestResult>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        browser: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        driver: Option<String>,
        at: String,
    },
}

/// Just the version of a file or a line, to refuse a newer format before
/// reading the rest.
#[derive(Deserialize)]
struct Version {
    v: u32,
}

fn now_string() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
}

fn parse_time(text: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(text)
        .map_or_else(|_| Local::now(), |time| time.with_timezone(&Local))
}

fn io_error(what: &str, path: &Path, error: &io::Error) -> SeleniumBaseError {
    SeleniumBaseError::Io(io::Error::new(
        error.kind(),
        format!("{what} {}: {error}", path.display()),
    ))
}

fn too_new(path: &Path, version: u32) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!(
        "{} is in format {version}, which this seleniumbase-rs (format {FORMAT_VERSION}) cannot \
         read; upgrade it or delete the folder {}",
        path.display(),
        path.parent().unwrap_or(path).display()
    ))
}

fn read_run_file(dir: &Path) -> Result<Option<RunFile>> {
    let path = dir.join(RUN_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("cannot read", &path, &error)),
    };
    let broken = |error: serde_json::Error| {
        SeleniumBaseError::InvalidConfig(format!(
            "{} is not a run file ({error}); delete the folder {} to start a new run",
            path.display(),
            dir.display()
        ))
    };
    let version: Version = serde_json::from_str(&text).map_err(broken)?;
    if version.v > FORMAT_VERSION {
        return Err(too_new(&path, version.v));
    }
    serde_json::from_str(&text).map(Some).map_err(broken)
}

fn write_run_file(dir: &Path, run: &RunFile) -> Result<()> {
    let path = dir.join(RUN_FILE);
    let text = serde_json::to_string(run).map_err(|error| {
        SeleniumBaseError::InvalidConfig(format!("cannot write {}: {error}", path.display()))
    })?;
    write_atomically(&path, text.as_bytes())
        .map_err(|error| io_error("cannot write", &path, &error))
}

fn read_events(dir: &Path) -> Result<Vec<Line>> {
    let path = dir.join(RESULTS_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("cannot read", &path, &error)),
    };
    let mut events = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(Version { v }) = serde_json::from_str(line) {
            if v > FORMAT_VERSION {
                return Err(too_new(&path, v));
            }
        }
        match serde_json::from_str::<Line>(line) {
            Ok(event) => events.push(event),
            // A process killed in the middle of a write leaves half a line.
            Err(error) => {
                tracing::warn!(%error, line = number + 1, path = %path.display(), "a result line was skipped");
            }
        }
    }
    Ok(events)
}

fn append_event(dir: &Path, event: Event) -> Result<()> {
    let path = dir.join(RESULTS_FILE);
    let mut text = serde_json::to_string(&Line {
        v: FORMAT_VERSION,
        event,
    })
    .map_err(|error| {
        SeleniumBaseError::InvalidConfig(format!("cannot write {}: {error}", path.display()))
    })?;
    text.push('\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| io_error("cannot open", &path, &error))?;
    // One write, so a line is never split between two processes.
    file.write_all(text.as_bytes())
        .map_err(|error| io_error("cannot write", &path, &error))
}

/// Takes the exclusive lock of the run in `dir`; it is released when the
/// returned file is dropped.
fn lock(dir: &Path) -> Result<fs::File> {
    fs::create_dir_all(dir).map_err(|error| io_error("cannot create", dir, &error))?;
    let path = dir.join(LOCK_FILE);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| io_error("cannot open", &path, &error))?;
    file.lock()
        .map_err(|error| io_error("cannot lock", &path, &error))?;
    Ok(file)
}

/// The events folded into one result per test, in the order the tests first
/// started. A test that started and has not ended is untested, or unreported
/// once the run is finalized.
fn fold(run: &RunFile, events: &[Line], root: &Path) -> RunSnapshot {
    struct Row {
        display: String,
        result: Option<TestResult>,
    }
    let finished = run.finalized_at.is_some();
    let mut order: Vec<String> = Vec::new();
    let mut rows: HashMap<String, Row> = HashMap::new();
    for line in events {
        match &line.event {
            Event::Start { id, display, .. } => {
                if !rows.contains_key(id) {
                    order.push(id.clone());
                }
                let row = rows.entry(id.clone()).or_insert_with(|| Row {
                    display: display.clone(),
                    result: None,
                });
                // A rerun starts again: the earlier result stands aside.
                row.display.clone_from(display);
                row.result = None;
            }
            Event::End { id, result, .. } => {
                if !rows.contains_key(id) {
                    order.push(id.clone());
                }
                let row = rows.entry(id.clone()).or_insert_with(|| Row {
                    display: result.name.clone(),
                    result: None,
                });
                row.result = Some(TestResult::clone(result));
            }
        }
    }
    let results = order
        .iter()
        .filter_map(|id| rows.remove(id))
        .map(|row| match row.result {
            Some(result) => result,
            None if finished => TestResult::new(
                row.display,
                TestStatus::Unreported,
                0.0,
                "the test did not finish",
            ),
            None => TestResult::new(row.display, TestStatus::Untested, 0.0, ""),
        })
        .collect();
    RunSnapshot {
        title: run.title.clone(),
        started_at: parse_time(&run.started_at),
        updated_at: parse_time(&run.updated_at),
        finished,
        results,
        root: root.to_path_buf(),
    }
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Hears about every test of a run, to write them somewhere else (Allure
/// results, say). Add one with [`TestRun::add_sink`].
///
/// A sink never changes how a test ends and has no error to return: log what
/// goes wrong.
pub trait RunSink: Send + Sync {
    /// A test started.
    fn started(&self, test: &TestId);

    /// A test ended.
    fn finished(&self, finished: &FinishedTest<'_>);
}

/// A test that ended, as a [`RunSink`] sees it.
#[derive(Debug)]
#[non_exhaustive]
pub struct FinishedTest<'a> {
    /// Which test.
    pub test: &'a TestId,
    /// What was recorded for it.
    pub result: &'a TestResult,
    /// The error that ended it, if one did.
    pub error: Option<&'a SeleniumBaseError>,
    /// The browser it ran in, when the page could tell.
    pub browser: Option<&'a str>,
    /// What drove the browser, when the page could tell.
    pub driver: Option<&'a str>,
    /// The folder with the test's logs; it may not exist.
    pub log_dir: &'a Path,
}

/// What [`TestRun::finalize`] did.
#[derive(Debug, Clone)]
pub struct FinalizeOutcome {
    /// The counts of the finished run.
    pub summary: ReportSummary,
    /// The dashboard that was written, if the run has one.
    pub dashboard: Option<PathBuf>,
    /// The HTML report that was written, if the run has one.
    pub html: Option<PathBuf>,
    /// The JUnit XML that was written, if the run has one.
    pub junit_xml: Option<PathBuf>,
    /// The folder the logs were archived to, if they were.
    pub archived: Option<PathBuf>,
    /// The folder with the logs of the run.
    pub logs_dir: PathBuf,
    /// What could not be done, in words for a person. The run's own result
    /// does not change.
    pub warnings: Vec<String>,
}

/// The files one rewrite of the outputs produced.
#[derive(Default)]
struct Written {
    dashboard: Option<PathBuf>,
    html: Option<PathBuf>,
    junit_xml: Option<PathBuf>,
    warnings: Vec<String>,
}

/// A run of tests, possibly across processes. See the [module](self)
/// documentation.
pub struct TestRun {
    root: PathBuf,
    settings: ReportSettings,
    key: RunKey,
    console: Console,
    sinks: Mutex<Vec<Arc<dyn RunSink>>>,
}

impl fmt::Debug for TestRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestRun")
            .field("root", &self.root)
            .field("key", &self.key)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// The run of this process, or why it could not be made.
static CURRENT: OnceLock<std::result::Result<Arc<TestRun>, String>> = OnceLock::new();

impl TestRun {
    /// The run of the calling process, made the first time it is asked for:
    /// from the settings file and the `SB_*` variables, the root in
    /// `SB_REPORT_ROOT` (the current folder when unset) and [`RunKey::detect`].
    ///
    /// # Errors
    ///
    /// Returns the error that made the run, every time: the settings could not
    /// be read, or the run folder could not be used.
    pub fn current() -> Result<Arc<Self>> {
        CURRENT
            .get_or_init(|| {
                let settings = Settings::load().map_err(|error| error.to_string())?;
                let root = absolute(artifacts::logs_root())
                    .map_err(|error| format!("cannot find the folder of the run: {error}"))?;
                Self::open(
                    root,
                    ReportSettings::from_settings(&settings),
                    RunKey::detect(),
                )
                .map_err(|error| error.to_string())
            })
            .as_ref()
            .map(Arc::clone)
            .map_err(|message| SeleniumBaseError::InvalidConfig(message.clone()))
    }

    /// Opens the run of `key` under `root`: joins it when one with that key
    /// is there, otherwise starts a new run.
    ///
    /// Starting a run clears `latest_logs` of `root`, or archives it when the
    /// new run or the previous one asked for it. `root` should be an absolute
    /// path.
    ///
    /// # Errors
    ///
    /// Returns an error if the run folder cannot be used, if `latest_logs` is a
    /// symbolic link (nothing is deleted then), or if the files of the run were
    /// written by a newer version.
    pub fn open(
        root: impl Into<PathBuf>,
        settings: ReportSettings,
        key: RunKey,
    ) -> Result<Arc<Self>> {
        Self::open_with_console(root, settings, key, stderr_console())
    }

    /// Like [`open`](Self::open), printing through `console` instead of
    /// stderr.
    ///
    /// # Errors
    ///
    /// As [`open`](Self::open).
    pub fn open_with_console(
        root: impl Into<PathBuf>,
        settings: ReportSettings,
        key: RunKey,
        console: Console,
    ) -> Result<Arc<Self>> {
        let run = Arc::new(Self {
            root: root.into(),
            settings,
            key,
            console,
            sinks: Mutex::new(Vec::new()),
        });
        run.begin()?;
        Ok(run)
    }

    /// Opens the run that is already under `root`, whatever its key, to finish it
    /// afterwards (`sbrs report finish`). The settings are the ones saved in the
    /// run, or `settings` when given. Nothing is cleared or started.
    ///
    /// # Errors
    ///
    /// Returns an error naming the folder if there is no run in `root`, or if
    /// its files cannot be read.
    pub fn resume(root: impl Into<PathBuf>, settings: Option<ReportSettings>) -> Result<Arc<Self>> {
        let root = root.into();
        let state = root.join(RUN_DIR);
        let Some(run) = read_run_file(&state)? else {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "there is no test run in {} (looked for {}); run tests that use the sb fixture first",
                root.display(),
                state.join(RUN_FILE).display()
            )));
        };
        Ok(Arc::new(Self {
            root,
            settings: settings.unwrap_or(run.settings),
            key: RunKey::new(run.key),
            console: stderr_console(),
            sinks: Mutex::new(Vec::new()),
        }))
    }

    /// The folder the run's files are under.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The settings of this process's side of the run.
    #[must_use]
    pub fn settings(&self) -> &ReportSettings {
        &self.settings
    }

    /// The folder with the logs of the run.
    #[must_use]
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join(LATEST_LOGS_DIR)
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join(RUN_DIR)
    }

    /// Hears about every test that starts or ends from now on.
    pub fn add_sink(&self, sink: Arc<dyn RunSink>) {
        self.sinks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(sink);
    }

    fn sinks(&self) -> Vec<Arc<dyn RunSink>> {
        self.sinks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn joins(&self, run: &RunFile) -> bool {
        match self.key.as_str() {
            Some(key) => {
                // A process id comes round again after a while: an old run is not this one.
                let age = Local::now() - parse_time(&run.updated_at);
                run.key == key
                    && (!key.starts_with("cargo-ppid-") || age.num_seconds() < PPID_REUSE_SECONDS)
            }
            None => {
                let age = Local::now() - parse_time(&run.updated_at);
                run.finalized_at.is_none() && age.num_seconds() < RECENT_SECONDS
            }
        }
    }

    /// Joins the run on disk or starts a new one.
    fn begin(&self) -> Result<()> {
        let dir = self.state_dir();
        let _lock = lock(&dir)?;
        let existing = read_run_file(&dir)?;
        if let Some(mut run) = existing.clone().filter(|run| self.joins(run)) {
            // A finalized run that gets another test is a run that went on.
            run.finalized_at = None;
            run.updated_at = now_string();
            return write_run_file(&dir, &run);
        }
        let asked_before = existing
            .as_ref()
            .is_some_and(|previous| previous.settings.archive_logs && !previous.archived);
        let archive = self.settings.archive_logs || asked_before;
        let archived = log_folder_setup(&self.root, archive)
            .map_err(|error| io_error("cannot prepare", &self.logs_dir(), &error))?;
        if let Some(target) = &archived {
            // The state of the run those logs belong to goes with them.
            for name in [RUN_FILE, RESULTS_FILE] {
                let _ = fs::copy(dir.join(name), target.join(name));
            }
        }
        let results = dir.join(RESULTS_FILE);
        fs::write(&results, b"").map_err(|error| io_error("cannot write", &results, &error))?;
        let now = now_string();
        let mut run = RunFile {
            v: FORMAT_VERSION,
            key: self.key.as_str().map_or_else(
                || format!("recent-{}", Local::now().timestamp_millis()),
                str::to_owned,
            ),
            title: self.settings.title(),
            started_at: now.clone(),
            updated_at: now,
            finalized_at: None,
            archived: false,
            settings: self.settings.clone(),
            warnings: Vec::new(),
        };
        if self.settings.allure_dir.is_some() && !cfg!(feature = "allure") {
            run.warnings.push(
                "SB_ALLURE_DIR is set, but seleniumbase-rs was built without the \"allure\" \
                 feature; add features = [\"allure\"] to its dependency"
                    .to_owned(),
            );
        }
        let snapshot = fold(&run, &[], &self.root);
        let written = self.write_outputs(&snapshot);
        merge_warnings(&mut run.warnings, written.warnings);
        write_run_file(&dir, &run)
    }

    /// Writes every output that is turned on from `snapshot`. A failure is a
    /// warning: reports never change a test's result.
    fn write_outputs(&self, snapshot: &RunSnapshot) -> Written {
        let mut written = Written::default();
        let options = |path: &Path| PageOptions {
            title: snapshot.title.clone(),
            live: false,
            pie: false,
            page_dir: path
                .parent()
                .map_or_else(|| self.root.clone(), Path::to_path_buf),
        };
        let settings = &self.settings;
        let mut put = |name: &str,
                       path: PathBuf,
                       write: &dyn Fn(&Path) -> io::Result<()>,
                       slot: &mut Option<PathBuf>| {
            let made = path
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| write(&path));
            match made {
                Ok(()) => *slot = Some(path),
                Err(error) => written.warnings.push(format!("{name}: {error}")),
            }
        };
        let mut dashboard = None;
        let mut html = None;
        let mut junit = None;
        if settings.dashboard {
            let path = self.root.join(DASHBOARD_FILE);
            let page = options(&path);
            put(
                "dashboard",
                path,
                &|path| write_dashboard(path, snapshot, &page),
                &mut dashboard,
            );
        }
        if let Some(file) = &settings.html {
            let path = self.root.join(file);
            let page = options(&path);
            put(
                "HTML report",
                path,
                &|path| write_html_report(path, snapshot, &page),
                &mut html,
            );
        }
        if let Some(file) = &settings.junit_xml {
            let path = self.root.join(file);
            put(
                "JUnit XML",
                path,
                &|path| write_junit_xml(path, snapshot),
                &mut junit,
            );
        }
        written.dashboard = dashboard;
        written.html = html;
        written.junit_xml = junit;
        written
    }

    /// Appends `event` to the run, then rewrites the outputs. Everything
    /// happens under the lock of the run.
    fn record(&self, event: Event) -> Result<()> {
        let dir = self.state_dir();
        let _lock = lock(&dir)?;
        let Some(mut run) = read_run_file(&dir)? else {
            return Err(self.no_run());
        };
        append_event(&dir, event)?;
        run.updated_at = now_string();
        let snapshot = fold(&run, &read_events(&dir)?, &self.root);
        let written = self.write_outputs(&snapshot);
        merge_warnings(&mut run.warnings, written.warnings);
        write_run_file(&dir, &run)
    }

    fn no_run(&self) -> SeleniumBaseError {
        SeleniumBaseError::InvalidConfig(format!(
            "there is no test run in {} (looked for {}); the folder was removed during the run",
            self.root.display(),
            self.state_dir().join(RUN_FILE).display()
        ))
    }

    /// Records that `test` started and returns the recorder to finish it with.
    ///
    /// # Errors
    ///
    /// Returns an error if the run's files cannot be written.
    pub fn start(self: &Arc<Self>, test: TestId) -> Result<TestRecorder> {
        self.record(Event::Start {
            id: test.id.clone(),
            display: test.display.clone(),
            suite: test.suite.clone(),
            pid: std::process::id(),
            at: now_string(),
        })?;
        for sink in self.sinks() {
            sink.started(&test);
        }
        let log_dir = self.logs_dir().join(&test.id);
        let thread = artifacts::activate_test_logs(&log_dir);
        Ok(TestRecorder {
            run: Arc::clone(self),
            test,
            started: Instant::now(),
            finished: false,
            log_dir,
            thread,
            failure: None,
            browser: None,
            driver: None,
        })
    }

    /// The run as it stands.
    ///
    /// # Errors
    ///
    /// Returns an error if the run's files cannot be read.
    pub fn snapshot(&self) -> Result<RunSnapshot> {
        let dir = self.state_dir();
        let _lock = lock(&dir)?;
        let Some(run) = read_run_file(&dir)? else {
            return Err(self.no_run());
        };
        Ok(fold(&run, &read_events(&dir)?, &self.root))
    }

    /// Records the results of the run in the results database
    /// ([`ResultStore`](crate::storage::ResultStore)) as one run, when
    /// `with_db_reporting` is on, and returns its number. Tests that have not
    /// finished are left out; a test that never reported is stored as failed, a
    /// skip as a pass whose message starts with `skipped: `.
    ///
    /// Call it once per run, after [`finalize`](Self::finalize), from one
    /// process: the database is never written by several processes at once.
    ///
    /// # Errors
    ///
    /// Returns an error if the run's files cannot be read or the database cannot
    /// be written.
    #[cfg(feature = "turso")]
    pub async fn record_to_database(&self) -> Result<Option<crate::storage::RunId>> {
        use crate::storage::{ResultStore, RunInfo};

        if !self.settings.with_db_reporting {
            return Ok(None);
        }
        let snapshot = self.snapshot()?;
        let path = self.root.join(self.settings.db_path());
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let store = ResultStore::open(&path).await?;
        let mut info = RunInfo::new(snapshot.title.clone());
        if let Some(environment) = &self.settings.database_env {
            info = info.environment(environment.clone());
        }
        let run = store.start_run(&info).await?;
        for result in &snapshot.results {
            if result.status != TestStatus::Untested {
                store.record(run, result).await?;
            }
        }
        Ok(Some(run))
    }

    /// Ends the run: tests that started and never ended become unreported, the
    /// outputs are written a last time, and the logs are archived when asked
    /// and empty folders removed.
    ///
    /// It can be called again, and a test that starts after it re-opens the
    /// run.
    ///
    /// # Errors
    ///
    /// Returns an error if the run's files cannot be read or written. A
    /// report that cannot be written is a warning in the outcome instead.
    pub fn finalize(&self) -> Result<FinalizeOutcome> {
        let dir = self.state_dir();
        let _lock = lock(&dir)?;
        let Some(mut run) = read_run_file(&dir)? else {
            return Err(self.no_run());
        };
        run.finalized_at.get_or_insert_with(now_string);
        run.updated_at = now_string();
        let snapshot = fold(&run, &read_events(&dir)?, &self.root);
        let written = self.write_outputs(&snapshot);
        merge_warnings(&mut run.warnings, written.warnings);
        let mut archived = None;
        if (run.settings.archive_logs || self.settings.archive_logs) && !run.archived {
            match archive_logs_if_set(&self.root, true) {
                Ok(Some(target)) => {
                    for name in [RUN_FILE, RESULTS_FILE] {
                        let _ = fs::copy(dir.join(name), target.join(name));
                    }
                    run.archived = true;
                    archived = Some(target);
                }
                Ok(None) => {}
                Err(error) => {
                    let warning = format!("archived_logs: {error}");
                    merge_warnings(&mut run.warnings, vec![warning]);
                }
            }
        }
        if let Err(error) = clear_empty_logs(&self.root) {
            let warning = format!("latest_logs: {error}");
            merge_warnings(&mut run.warnings, vec![warning]);
        }
        write_run_file(&dir, &run)?;
        Ok(FinalizeOutcome {
            summary: snapshot.summary(),
            dashboard: written.dashboard,
            html: written.html,
            junit_xml: written.junit_xml,
            archived,
            logs_dir: self.logs_dir(),
            warnings: run.warnings,
        })
    }
}

/// Adds the warnings of `new` that `warnings` does not have yet.
fn merge_warnings(warnings: &mut Vec<String>, new: Vec<String>) {
    for warning in new {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
}

fn absolute(root: PathBuf) -> io::Result<PathBuf> {
    if root.as_os_str().is_empty() {
        std::env::current_dir()
    } else {
        std::path::absolute(root)
    }
}

// ---------------------------------------------------------------------------
// The recorder
// ---------------------------------------------------------------------------

/// What one test writes to its [`TestRun`]: its evidence, and its result.
///
/// Make one with [`TestRun::start`]. Call [`on_failure`](Self::on_failure) or
/// [`on_success`](Self::on_success) while the browser is still open, then
/// [`finish`](Self::finish). A recorder that is dropped without `finish`
/// records the test as failed (when the thread is panicking) or unreported.
pub struct TestRecorder {
    run: Arc<TestRun>,
    test: TestId,
    started: Instant,
    finished: bool,
    log_dir: PathBuf,
    thread: ThreadId,
    failure: Option<FailureDetails>,
    browser: Option<String>,
    driver: Option<String>,
}

impl fmt::Debug for TestRecorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestRecorder")
            .field("test", &self.test.display)
            .field("log_dir", &self.log_dir)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

/// How a test ended, borrowed from whoever ran it.
enum Ended<'a> {
    Passed,
    Skipped(&'a str),
    Failed(&'a SeleniumBaseError),
}

/// What `ask` gives within [`PAGE_WAIT`], or `None`.
async fn within<T>(ask: impl std::future::Future<Output = Result<T>>) -> Option<T> {
    tokio::time::timeout(PAGE_WAIT, ask).await.ok()?.ok()
}

impl TestRecorder {
    /// Which test this records.
    #[must_use]
    pub fn test_id(&self) -> &TestId {
        &self.test
    }

    /// The folder with this test's logs, `<root>/latest_logs/<id>`. It is made
    /// by the first file written to it.
    #[must_use]
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// `latest_logs/<id>/<file>`, relative to the run's root.
    fn relative(&self, file: &str) -> String {
        format!("{LATEST_LOGS_DIR}/{}/{file}", self.test.id)
    }

    /// Takes the evidence of a failed test from `page`, while its browser is
    /// still open: `basic_test_info.txt`, `screenshot.png` (unless
    /// `no_screenshot`) and `page_source.html` in the test's folder. Each item
    /// is given at most five seconds, and one that fails is left out. With
    /// `list_fail_pages` it prints `=> Fail Page: <url>`.
    ///
    /// A skip is not a failure; this does nothing for one.
    pub async fn on_failure(&mut self, error: &SeleniumBaseError, page: &dyn PageEvidence) {
        if error.is_skipped() {
            return;
        }
        let settings = self.run.settings.clone();
        let url = within(page.current_url()).await;
        let title = within(page.page_title()).await;
        self.browser = within_label(page.browser_label()).await;
        self.driver = within_label(page.driver_label()).await;
        let mut details = FailureDetails::from_error(error);
        details.url.clone_from(&url);
        details.title = title;
        if !settings.no_screenshot {
            if let Some(png) = within(page.screenshot_png()).await {
                match log_screenshot(&self.log_dir, &png) {
                    Ok(_) => details.screenshot = Some(self.relative(SCREENSHOT_FILE)),
                    Err(error) => tracing::warn!(%error, "the screenshot was not written"),
                }
            }
        }
        if let Some(source) = within(page.page_source()).await {
            match log_page_source(&self.log_dir, url.as_deref(), &source) {
                Ok(_) => details.page_source = Some(self.relative(PAGE_SOURCE_FILE)),
                Err(error) => tracing::warn!(%error, "the page source was not written"),
            }
        }
        self.write_info(error, &details);
        if settings.list_fail_pages {
            if let Some(url) = &url {
                (self.run.console)(&format!("\n=> Fail Page: {url}\n"));
            }
        }
        self.failure = Some(details);
    }

    /// Records the facts of a failure whose evidence the caller took itself:
    /// the screenshot and the page source are linked when they are already in
    /// the test's folder, and nothing is written or printed.
    pub fn note_failure(
        &mut self,
        error: &SeleniumBaseError,
        url: Option<String>,
        title: Option<String>,
    ) {
        if error.is_skipped() {
            return;
        }
        let mut details = FailureDetails::from_error(error);
        details.url = url;
        details.title = title;
        let linked = |file: &str| {
            self.log_dir
                .join(file)
                .is_file()
                .then(|| self.relative(file))
        };
        details.screenshot = linked(SCREENSHOT_FILE);
        details.page_source = linked(PAGE_SOURCE_FILE);
        self.failure = Some(details);
    }

    /// Takes the screenshot of a passing test when `save_screenshot` is on and
    /// `no_screenshot` is not.
    pub async fn on_success(&mut self, page: &dyn PageEvidence) {
        let settings = &self.run.settings;
        if !settings.save_screenshot || settings.no_screenshot {
            return;
        }
        if let Some(png) = within(page.screenshot_png()).await {
            if let Err(error) = log_screenshot(&self.log_dir, &png) {
                tracing::warn!(%error, "the screenshot was not written");
            }
        }
    }

    /// Writes `basic_test_info.txt` for a failure.
    fn write_info(&self, error: &SeleniumBaseError, details: &FailureDetails) {
        let info = FailureInfo {
            test_id: self.test.display.clone(),
            last_page: details.url.clone(),
            duration: self.started.elapsed(),
            browser: self.browser.clone(),
            driver: self.driver.clone(),
            traceback: details.detail.clone(),
            exception: error.to_string(),
        };
        if let Err(error) = log_test_failure_data(&self.log_dir, &info) {
            tracing::warn!(%error, "basic_test_info.txt was not written");
        }
    }

    /// Records how the test ended and rewrites the outputs of the run.
    ///
    /// An error with [`is_skipped`](SeleniumBaseError::is_skipped) is recorded
    /// as skipped, with its reason, and writes `skip_reason.txt`. Any other
    /// error is a failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the run's files cannot be written. That never
    /// changes the test's own result: log it and go on.
    pub fn finish(self, outcome: &Result<()>) -> Result<()> {
        match outcome {
            Ok(()) => self.finish_with(Ended::Passed),
            Err(SeleniumBaseError::Skipped(reason)) => self.finish_with(Ended::Skipped(reason)),
            Err(error) => self.finish_with(Ended::Failed(error)),
        }
    }

    /// Like [`finish`](Self::finish), for an outcome that is only borrowed.
    fn finish_with(mut self, ended: Ended<'_>) -> Result<()> {
        let (result, error) = match ended {
            Ended::Passed => (self.result(TestStatus::Passed, "ok".to_owned(), None), None),
            Ended::Skipped(reason) => {
                self.write_skip(reason);
                (
                    self.result(TestStatus::Skipped, reason.to_owned(), None),
                    None,
                )
            }
            Ended::Failed(error) => {
                let details = self.failure.take().unwrap_or_else(|| {
                    // The caller took no evidence: the facts still belong in the folder.
                    let details = FailureDetails::from_error(error);
                    self.write_info(error, &details);
                    details
                });
                (
                    self.result(TestStatus::Failed, error.to_string(), Some(details)),
                    Some(error),
                )
            }
        };
        self.record(result, error)
    }

    fn write_skip(&self, reason: &str) {
        let info = FailureInfo {
            test_id: self.test.display.clone(),
            duration: self.started.elapsed(),
            exception: reason.to_owned(),
            ..FailureInfo::default()
        };
        if let Err(error) = log_skipped_test_data(&self.log_dir, &info, reason) {
            tracing::warn!(%error, "skip_reason.txt was not written");
        }
    }

    fn result(
        &self,
        status: TestStatus,
        message: String,
        failure: Option<FailureDetails>,
    ) -> TestResult {
        TestResult {
            name: self.test.display.clone(),
            status,
            duration_secs: self.started.elapsed().as_secs_f64(),
            message,
            failure,
            log_dir: self
                .log_dir
                .is_dir()
                .then(|| format!("{LATEST_LOGS_DIR}/{}", self.test.id)),
        }
    }

    /// Appends the end event and tells the sinks.
    fn record(&mut self, result: TestResult, error: Option<&SeleniumBaseError>) -> Result<()> {
        self.finished = true;
        artifacts::deactivate_test_logs(self.thread, &self.log_dir);
        let recorded = self.run.record(Event::End {
            id: self.test.id.clone(),
            result: Box::new(result.clone()),
            browser: self.browser.clone(),
            driver: self.driver.clone(),
            at: now_string(),
        });
        let finished = FinishedTest {
            test: &self.test,
            result: &result,
            error,
            browser: self.browser.as_deref(),
            driver: self.driver.as_deref(),
            log_dir: &self.log_dir,
        };
        for sink in self.run.sinks() {
            sink.finished(&finished);
        }
        recorded
    }
}

/// What a label call gives within [`PAGE_WAIT`], or `None`.
async fn within_label(ask: impl std::future::Future<Output = Option<String>>) -> Option<String> {
    tokio::time::timeout(PAGE_WAIT, ask).await.ok().flatten()
}

impl Drop for TestRecorder {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let panicked = std::thread::panicking();
        let (status, message) = if panicked {
            (TestStatus::Failed, "the test panicked")
        } else {
            (TestStatus::Unreported, "the test did not finish")
        };
        let failure = panicked.then(|| {
            let error = SeleniumBaseError::TestLifecycle(message.to_owned());
            let details = FailureDetails::from_error(&error);
            self.write_info(&error, &details);
            details
        });
        let result = self.result(status, message.to_owned(), failure);
        if let Err(error) = self.record(result, None) {
            tracing::warn!(%error, test = %self.test.display, "the end of a test was not recorded");
        }
    }
}

// ---------------------------------------------------------------------------
// The `sb` fixture
// ---------------------------------------------------------------------------

/// Records every attempt the `sb` fixture makes, when registered with
/// `Sb::finalizer` (or `SbCdp::finalizer`).
///
/// The fixture's lifecycle takes the evidence of a failure itself, so this only
/// records the outcome: it links the evidence that is in the test's folder and
/// finishes the recorder. A run shows a test only once its attempt has ended.
#[cfg(feature = "testing")]
#[derive(Debug, Clone)]
pub struct RunFinalizer {
    run: Arc<TestRun>,
    test: TestId,
}

#[cfg(feature = "testing")]
impl RunFinalizer {
    /// A finalizer that records the attempts of `test` in `run`. Make it where
    /// the test's id is read ([`TestId::current`] on the test's own thread).
    #[must_use]
    pub fn new(run: Arc<TestRun>, test: TestId) -> Self {
        Self { run, test }
    }
}

#[cfg(feature = "testing")]
#[async_trait::async_trait]
impl crate::testing::Finalizer for RunFinalizer {
    async fn finish(
        &self,
        end: &crate::testing::AttemptEnd<'_>,
        page: &dyn crate::common::debug_prompt::PromptPage,
    ) {
        use crate::testing::Outcome;

        let mut recorder = match self.run.start(self.test.clone()) {
            Ok(recorder) => recorder,
            Err(error) => {
                tracing::warn!(%error, test = %self.test.display, "the test was not recorded");
                return;
            }
        };
        // The attempt took `end.duration`; the recorder only just started.
        recorder.started = Instant::now()
            .checked_sub(end.duration)
            .unwrap_or_else(Instant::now);
        let finished = match &end.outcome {
            Outcome::Failed(error) => {
                let url = within(page.url()).await;
                let title = within(page.title()).await;
                recorder.note_failure(error, url, title);
                recorder.finish_with(Ended::Failed(error))
            }
            Outcome::Skipped(error) => {
                let reason = match error {
                    SeleniumBaseError::Skipped(reason) => reason.clone(),
                    other => other.to_string(),
                };
                recorder.finish_with(Ended::Skipped(&reason))
            }
            _ => recorder.finish_with(Ended::Passed),
        };
        if let Err(error) = finished {
            tracing::warn!(%error, test = %self.test.display, "the test was not recorded");
        }
    }
}

#[cfg(all(test, feature = "testing"))]
mod finalizer_tests {
    use async_trait::async_trait;
    use serde_json::Value;

    use super::*;
    use crate::common::debug_prompt::PromptPage;
    use crate::testing::{AttemptEnd, Finalizer, Outcome};

    struct Fake;

    #[async_trait]
    impl PromptPage for Fake {
        async fn url(&self) -> Result<String> {
            Ok("https://a.test/".to_owned())
        }
        async fn title(&self) -> Result<String> {
            Ok("A".to_owned())
        }
        async fn screenshot_png(&self) -> Result<Vec<u8>> {
            Err(SeleniumBaseError::unsupported(
                "the finalizer takes no screenshot",
            ))
        }
        async fn page_source(&self) -> Result<String> {
            Err(SeleniumBaseError::unsupported(
                "the finalizer takes no page source",
            ))
        }
        async fn evaluate(&self, _expression: &str) -> Result<Value> {
            Ok(Value::Null)
        }
    }

    fn run(root: &Path) -> Arc<TestRun> {
        TestRun::open(root, ReportSettings::default(), RunKey::new("finalizer")).unwrap()
    }

    fn end(outcome: Outcome<'_>) -> AttemptEnd<'_> {
        AttemptEnd {
            test_id: "f.test",
            attempt: 1,
            attempts: 1,
            outcome,
            duration: Duration::from_millis(1500),
        }
    }

    #[tokio::test]
    async fn the_finalizer_records_how_an_attempt_ended() {
        let root = tempfile::tempdir().unwrap();
        let run = run(root.path());
        let error = SeleniumBaseError::AssertionFailed("title".to_owned());
        let skip = SeleniumBaseError::Skipped("no browser".to_owned());
        let cases = [
            ("f::passes", Outcome::Passed),
            ("f::fails", Outcome::Failed(&error)),
            ("f::skips", Outcome::Skipped(&skip)),
        ];
        for (name, outcome) in cases {
            let finalizer = RunFinalizer::new(Arc::clone(&run), TestId::named(name));
            finalizer.finish(&end(outcome), &Fake).await;
        }

        let results = run.snapshot().unwrap().results;
        let seen: Vec<(&str, TestStatus)> = results
            .iter()
            .map(|result| (result.name.as_str(), result.status))
            .collect();
        assert_eq!(
            seen,
            [
                ("f::passes", TestStatus::Passed),
                ("f::fails", TestStatus::Failed),
                ("f::skips", TestStatus::Skipped)
            ]
        );
        assert!(
            results[0].duration_secs >= 1.5,
            "{}",
            results[0].duration_secs
        );
        let failure = results[1].failure.as_ref().unwrap();
        assert_eq!(failure.url.as_deref(), Some("https://a.test/"));
        assert_eq!(failure.title.as_deref(), Some("A"));
        assert_eq!(results[2].message, "no browser");
    }

    #[tokio::test]
    async fn the_evidence_the_lifecycle_took_is_linked_not_taken_again() {
        let root = tempfile::tempdir().unwrap();
        let run = run(root.path());
        let folder = root.path().join("latest_logs/f.fails");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("screenshot.png"), b"taken by the lifecycle").unwrap();
        let error = SeleniumBaseError::AssertionFailed("title".to_owned());

        RunFinalizer::new(Arc::clone(&run), TestId::named("f::fails"))
            .finish(&end(Outcome::Failed(&error)), &Fake)
            .await;

        let failure = run.snapshot().unwrap().results.remove(0).failure.unwrap();
        assert_eq!(
            failure.screenshot.as_deref(),
            Some("latest_logs/f.fails/screenshot.png")
        );
        assert_eq!(failure.page_source, None, "there was none");
        assert_eq!(
            fs::read(folder.join("screenshot.png")).unwrap(),
            b"taken by the lifecycle"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_key_takes_the_first_source_that_has_a_value() {
        assert_eq!(
            RunKey::from_sources(Some("sbrs-1"), Some("n"), Some(7)).as_str(),
            Some("sbrs-1")
        );
        assert_eq!(
            RunKey::from_sources(None, Some("abc"), Some(7)).as_str(),
            Some("nextest-abc")
        );
        assert_eq!(
            RunKey::from_sources(None, None, Some(4242)).as_str(),
            Some("cargo-ppid-4242")
        );
        assert_eq!(RunKey::from_sources(None, None, None), RunKey::recent());
    }

    #[test]
    fn a_blank_source_counts_as_missing() {
        assert_eq!(
            RunKey::from_sources(Some("  "), Some(""), Some(9)).as_str(),
            Some("cargo-ppid-9")
        );
    }

    #[test]
    fn the_title_reads_underscores_as_spaces_and_has_a_default() {
        let settings = ReportSettings {
            dash_title: Some("Nightly_checkout_run".to_owned()),
            ..ReportSettings::default()
        };
        assert_eq!(settings.title(), "Nightly checkout run");
        assert_eq!(ReportSettings::default().title(), DEFAULT_TITLE);
        let blank = ReportSettings {
            dash_title: Some("  ".to_owned()),
            ..ReportSettings::default()
        };
        assert_eq!(blank.title(), DEFAULT_TITLE);
    }
}
