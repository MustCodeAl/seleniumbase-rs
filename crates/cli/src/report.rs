//! `sbrs report`: ending a test run (`finish`) and reading a results database (`history`).
//!
//! `finish` is for the run that `sbrs test` did not start: plain `cargo test` leaves the live
//! dashboard going, and this ends it (see [`test_session`](crate::test_session)).

use std::path::PathBuf;

use seleniumbase_rs::core::test_run::{ReportSettings, RunFinish, TestRun};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

use crate::args::report::{FinishArgs, ReportArgs, ReportCommand};
use crate::failure::ArgumentError;
use crate::test_session::{self, Closing, ROOT_VARIABLE};

/// Runs the `report` subcommand the command line named.
///
/// # Errors
///
/// Returns the error of the subcommand. A folder with no run is not one: it is said on
/// standard error and the process exits with status 1.
pub(crate) async fn run(args: &ReportArgs) -> Result<(), Box<dyn std::error::Error>> {
    match &args.command {
        ReportCommand::Finish(finish_args) => finish(finish_args).await,
        #[cfg(feature = "turso")]
        ReportCommand::History(history) => {
            crate::history::run(
                history.db.clone(),
                history.run,
                history.failed,
                history.flaky,
                history.limit,
                history.json,
            )
            .await
        }
    }
}

/// The folder of the run: `SB_REPORT_ROOT`, else the current folder.
fn run_root() -> std::io::Result<PathBuf> {
    let root = std::env::var_os(ROOT_VARIABLE)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    match root {
        Some(root) => std::path::absolute(root),
        None => std::env::current_dir(),
    }
}

/// What the run was started with, with the flags of `report finish` on top.
fn overridden(saved: &ReportSettings, args: &FinishArgs) -> ReportSettings {
    let mut settings = saved.clone();
    if args.dashboard {
        settings.dashboard = true;
    }
    if let Some(title) = &args.dash_title {
        settings.dash_title = Some(title.clone());
    }
    if let Some(html) = &args.html {
        settings.html = Some(html.clone());
    }
    if let Some(xml) = &args.junit_xml {
        settings.junit_xml = Some(xml.clone());
    }
    if args.archive_logs {
        settings.archive_logs = true;
    }
    settings
}

/// Says why there is nothing to finish and exits with status 1.
fn nothing_to_finish(error: &SeleniumBaseError) -> ! {
    let message = match error {
        SeleniumBaseError::InvalidConfig(message) => message.clone(),
        other => other.to_string(),
    };
    eprintln!("error: {message}");
    std::process::exit(1)
}

async fn finish(args: &FinishArgs) -> Result<(), Box<dyn std::error::Error>> {
    let root = run_root()?;
    let saved =
        TestRun::resume(root.clone(), None).unwrap_or_else(|error| nothing_to_finish(&error));
    let settings = overridden(saved.settings(), args);
    if args.show_report && settings.html.is_none() && !settings.dashboard {
        return Err(ArgumentError::new(
            "report",
            "--show-report needs --html or --dashboard, or a run that was started with one of them",
        )
        .into());
    }
    let run = TestRun::resume(root, Some(settings.clone()))
        .unwrap_or_else(|error| nothing_to_finish(&error));

    let done = run.finish().await?;
    let closing = Closing::of(&settings, &done);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&finish_json(&done))?);
    } else {
        closing.print();
    }
    if args.show_report {
        if let Some(page) = &closing.page {
            test_session::show_report(page).await?;
        }
    }
    if closing.report_missing {
        std::process::exit(1);
    }
    Ok(())
}

/// What finishing the run did, for `--json`.
fn finish_json(done: &RunFinish) -> Value {
    let outcome = &done.outcome;
    let summary = &outcome.summary;
    #[cfg(feature = "turso")]
    let database_run = done.database_run.map(|run| run.get());
    #[cfg(not(feature = "turso"))]
    let database_run: Option<i64> = None;
    json!({
        "summary": {
            "total": summary.total,
            "passed": summary.passed,
            "failed": summary.failed,
            "skipped": summary.skipped,
            "untested": summary.untested,
            "unreported": summary.unreported,
            "duration_secs": summary.duration_secs,
        },
        "dashboard": outcome.dashboard,
        "html": outcome.html,
        "junit_xml": outcome.junit_xml,
        "archived": outcome.archived,
        "logs_dir": outcome.logs_dir,
        "database_run": database_run,
        "warnings": outcome.warnings,
    })
}
