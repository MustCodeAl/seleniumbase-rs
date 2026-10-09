//! `sbase report`: reading a results database from the command line.

use std::path::PathBuf;

use seleniumbase_rs::core::report_helper::TestResult;
use seleniumbase_rs::storage::{FlakyTest, ResultStore, RunId, RunSummary};
use serde_json::{json, Value};

/// Where results are read from when neither `--db` nor `SB_REPORT_DB` is set.
const DEFAULT_DB: &str = "reports/results.db";

pub async fn run(
    db: Option<PathBuf>,
    run: Option<RunId>,
    failed: bool,
    flaky: bool,
    limit: usize,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = db
        .or_else(|| std::env::var_os("SB_REPORT_DB").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DB));
    // Opening would create an empty database, which would hide a typo in the path.
    if !path.exists() {
        return Err(format!(
            "there is no results database at {}; pass --db or set SB_REPORT_DB",
            path.display()
        )
        .into());
    }
    let store = ResultStore::open(&path).await?;

    let output = if flaky {
        let tests = store.flaky_tests(limit).await?;
        if json {
            pretty(&flaky_json(&tests))
        } else {
            render_flaky(&tests, limit)
        }
    } else if let Some(id) = run {
        let summary = store
            .run(id)
            .await?
            .ok_or_else(|| format!("there is no run {id} in {}", path.display()))?;
        let mut results = store.results(id).await?;
        if failed {
            results.retain(|r| !r.passed);
        }
        if json {
            pretty(&json!({ "run": run_json(&summary), "results": results_json(&results) }))
        } else {
            render_run(&summary, &results)
        }
    } else {
        let runs = store.runs(limit).await?;
        if json {
            pretty(&Value::Array(runs.iter().map(run_json).collect()))
        } else {
            render_runs(&runs)
        }
    };
    println!("{output}");
    Ok(())
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn run_json(run: &RunSummary) -> Value {
    json!({
        "id": run.id.get(),
        "label": run.label,
        "environment": run.environment,
        "started_at": run.started_at.to_rfc3339(),
        "last_result_at": run.last_result_at.map(|t| t.to_rfc3339()),
        "passed": run.passed,
        "failed": run.failed,
    })
}

fn results_json(results: &[TestResult]) -> Value {
    Value::Array(
        results
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "passed": r.passed,
                    "duration_secs": r.duration_secs,
                    "message": r.message,
                })
            })
            .collect(),
    )
}

fn flaky_json(tests: &[FlakyTest]) -> Value {
    Value::Array(
        tests
            .iter()
            .map(|t| json!({ "name": t.name, "attempts": t.attempts, "failures": t.failures }))
            .collect(),
    )
}

fn render_runs(runs: &[RunSummary]) -> String {
    if runs.is_empty() {
        return "No runs recorded yet.".to_owned();
    }
    let label_width = runs
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(0)
        .max(5);
    let env_width = runs
        .iter()
        .map(|r| r.environment.chars().count())
        .max()
        .unwrap_or(0)
        .max(3);
    let mut out = format!(
        "{:>5}  {:<label_width$}  {:<env_width$}  {:<19}  {:>6}  {:>6}",
        "RUN", "LABEL", "ENV", "STARTED (UTC)", "PASSED", "FAILED"
    );
    for run in runs {
        out.push_str(&format!(
            "\n{:>5}  {:<label_width$}  {:<env_width$}  {:<19}  {:>6}  {:>6}",
            run.id,
            run.label,
            run.environment,
            run.started_at.format("%Y-%m-%d %H:%M:%S"),
            run.passed,
            run.failed,
        ));
    }
    out
}

fn render_run(run: &RunSummary, results: &[TestResult]) -> String {
    let mut out = format!(
        "Run {} \"{}\" on {}: {} passed, {} failed",
        run.id, run.label, run.environment, run.passed, run.failed
    );
    if results.is_empty() {
        out.push_str("\n(no results to show)");
    }
    for result in results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "\n{status}  {:>8.3}s  {}",
            result.duration_secs, result.name
        ));
        if !result.passed && !result.message.is_empty() {
            for line in result.message.lines() {
                out.push_str(&format!("\n          {line}"));
            }
        }
    }
    out
}

fn render_flaky(tests: &[FlakyTest], runs: usize) -> String {
    if tests.is_empty() {
        return format!("No flaky tests in the latest {runs} runs.");
    }
    let width = tests
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let mut out = format!("{:<width$}  {:>8}  {:>8}", "TEST", "ATTEMPTS", "FAILURES");
    for test in tests {
        out.push_str(&format!(
            "\n{:<width$}  {:>8}  {:>8}",
            test.name, test.attempts, test.failures
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs::storage::RunInfo;

    use super::*;

    async fn two_runs() -> (ResultStore, RunId, RunId) {
        let store = ResultStore::in_memory().await.unwrap();
        let first = store
            .start_run(&RunInfo::new("nightly").environment("staging"))
            .await
            .unwrap();
        let second = store.start_run(&RunInfo::new("hotfix")).await.unwrap();
        for (run, name, passed) in [
            (first, "login", true),
            (first, "checkout", false),
            (second, "login", true),
        ] {
            store
                .record(
                    run,
                    &TestResult {
                        name: name.to_owned(),
                        passed,
                        duration_secs: 1.25,
                        message: if passed {
                            "ok"
                        } else {
                            "no such element\nsecond line"
                        }
                        .to_owned(),
                    },
                )
                .await
                .unwrap();
        }
        (store, first, second)
    }

    #[tokio::test]
    async fn the_run_table_lists_every_run_with_its_counts() {
        let (store, _, _) = two_runs().await;

        let table = render_runs(&store.runs(10).await.unwrap());

        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3, "a header and two runs:\n{table}");
        assert!(lines[0].contains("LABEL") && lines[0].contains("FAILED"));
        assert!(
            lines[1].contains("hotfix") && lines[1].contains("local"),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].contains("nightly") && lines[2].contains("staging"),
            "{}",
            lines[2]
        );
    }

    #[test]
    fn an_empty_database_says_so_instead_of_printing_a_bare_header() {
        assert_eq!(render_runs(&[]), "No runs recorded yet.");
        assert_eq!(
            render_flaky(&[], 20),
            "No flaky tests in the latest 20 runs."
        );
    }

    #[tokio::test]
    async fn a_run_shows_failures_with_their_message_indented() {
        let (store, first, _) = two_runs().await;
        let summary = store.run(first).await.unwrap().unwrap();
        let results = store.results(first).await.unwrap();

        let text = render_run(&summary, &results);

        assert!(
            text.starts_with("Run 1 \"nightly\" on staging: 1 passed, 1 failed"),
            "{text}"
        );
        assert!(text.contains("PASS") && text.contains("login"));
        assert!(text.contains("FAIL") && text.contains("checkout"));
        assert!(
            text.contains("\n          no such element\n          second line"),
            "{text}"
        );
        assert!(
            !text.contains("ok"),
            "a passing test's message is not repeated"
        );
    }

    #[tokio::test]
    async fn json_output_is_machine_readable() {
        let (store, first, _) = two_runs().await;
        let summary = store.run(first).await.unwrap().unwrap();

        let value = run_json(&summary);

        assert_eq!(value["id"], 1);
        assert_eq!(value["label"], "nightly");
        assert_eq!(value["environment"], "staging");
        assert_eq!(
            (value["passed"].clone(), value["failed"].clone()),
            (json!(1), json!(1))
        );
        assert!(
            value["started_at"].as_str().unwrap().contains('T'),
            "RFC 3339"
        );
        assert_eq!(
            results_json(&store.results(first).await.unwrap())[1]["passed"],
            false
        );
    }

    #[tokio::test]
    async fn a_start_time_is_shown_in_utc_to_the_second() {
        let (store, _, _) = two_runs().await;

        let table = render_runs(&store.runs(1).await.unwrap());

        let shape = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}").unwrap();
        assert!(shape.is_match(&table), "{table}");
    }
}
