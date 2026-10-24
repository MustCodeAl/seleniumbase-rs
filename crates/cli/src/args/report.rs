//! The arguments of `sbrs report`: `finish` and `history`.
//!
//! (Plain comments on the structs, not docs: a doc comment on a struct that derives `Args`
//! would be help text.)

use std::path::PathBuf;

// The `report` command: it holds only its own subcommands.
#[derive(Debug, usage::Args)]
pub(crate) struct ReportArgs {
    #[usage(subcommand)]
    pub(crate) command: ReportCommand,
}

#[derive(Debug, usage::Subcommands)]
pub(crate) enum ReportCommand {
    /// Finish the test run of the current folder and write its reports.
    ///
    /// A plain `cargo test` leaves the live dashboard running, because the standard test
    /// harness cannot tell when the last test has ended. This command ends the run: the tests
    /// that started and never ended become unreported, the dashboard, the HTML report and the
    /// JUnit XML are written a last time, the logs are archived when asked, and the results go
    /// to the results database when `with_db_reporting` is on. `sbrs test` does the same by
    /// itself. The run is the one in `SB_REPORT_ROOT`, or in the current folder; the flags
    /// replace the settings the run was started with.
    #[usage(example = "sbrs report finish --html report.html --junit-xml report.xml")]
    Finish(FinishArgs),
    /// Show the test runs recorded in a results database.
    ///
    /// Lists the latest runs; `--run` shows one run's results and `--flaky` the tests that
    /// both pass and fail across recent runs. The database is the one a `ResultStore`
    /// writes: pass its path with `--db` or set SB_REPORT_DB (default: reports/results.db).
    #[cfg(feature = "turso")]
    #[usage(example = "sbrs report history --limit 10")]
    History(HistoryArgs),
}

// The options of `report finish`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct FinishArgs {
    /// Write the live dashboard (dashboard.html) in its final form.
    #[usage(long, bool_value)]
    pub(crate) dashboard: bool,
    /// The dashboard's title; an underscore stands for a space.
    #[usage(long = "dash-title", value_name = "TITLE", value_hint = usage::ValueHint::Other)]
    pub(crate) dash_title: Option<String>,
    /// Write an HTML report to this file.
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::AnyPath)]
    pub(crate) html: Option<PathBuf>,
    /// Write a JUnit XML report to this file.
    #[usage(long = "junit-xml", value_name = "PATH", value_hint = usage::ValueHint::AnyPath)]
    pub(crate) junit_xml: Option<PathBuf>,
    /// Keep the logs of the run in archived_logs/ instead of deleting them later.
    #[usage(long = "archive-logs", bool_value)]
    pub(crate) archive_logs: bool,
    /// Open the HTML report or the dashboard in a browser window and wait until it is closed.
    #[usage(long = "show-report", bool_value)]
    pub(crate) show_report: bool,
    /// Print what was done as JSON instead of the `-----` lines.
    #[usage(long, bool_value)]
    pub(crate) json: bool,
}

// The options of `report history`.
#[cfg(feature = "turso")]
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct HistoryArgs {
    /// Path of the results database.
    #[usage(long, value_hint = usage::ValueHint::AnyPath)]
    pub(crate) db: Option<PathBuf>,
    /// Show the results of this run.
    #[usage(long, conflicts("--flaky"))]
    pub(crate) run: Option<seleniumbase_rs::storage::RunId>,
    /// With --run, show only the tests that failed.
    #[usage(long, bool_value, requires("--run"))]
    pub(crate) failed: bool,
    /// Show tests that both pass and fail across the latest runs.
    #[usage(long, bool_value)]
    pub(crate) flaky: bool,
    /// How many runs to list, or to examine for flaky tests.
    #[usage(long, default = "20")]
    pub(crate) limit: usize,
    /// Print JSON instead of a table.
    #[usage(long, bool_value)]
    pub(crate) json: bool,
}
