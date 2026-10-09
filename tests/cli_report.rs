//! `sbase report`, run as the real binary against a database made with the
//! library. Needs the `turso` feature: `cargo test --features turso`.

#![cfg(feature = "turso")]

use std::path::Path;
use std::process::{Command, Output};

use seleniumbase_rs::core::report_helper::TestResult;
use seleniumbase_rs::storage::{ResultStore, RunInfo};
use serde_json::Value;

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

fn result(name: &str, passed: bool) -> TestResult {
    TestResult {
        name: name.to_owned(),
        passed,
        duration_secs: 0.75,
        message: if passed { "ok" } else { "timed out waiting" }.to_owned(),
    }
}

/// A database with two runs: `login` flips between them, `checkout` always fails.
async fn database(path: &Path) {
    let store = ResultStore::open(path).await.unwrap();
    for (label, login_passes) in [("nightly", true), ("hotfix", false)] {
        let run = store
            .start_run(&RunInfo::new(label).environment("staging"))
            .await
            .unwrap();
        store
            .record(run, &result("login", login_passes))
            .await
            .unwrap();
        store.record(run, &result("checkout", false)).await.unwrap();
    }
}

fn sbase(args: &[&str], db: Option<&Path>) -> Output {
    let mut command = Command::new(SBASE);
    command
        .args(["report"])
        .args(args)
        .env_remove("SB_REPORT_DB");
    if let Some(db) = db {
        command.arg("--db").arg(db);
    }
    command.output().expect("sbase starts")
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[tokio::test]
async fn it_lists_the_runs_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let output = sbase(&[], Some(&db));

    assert!(output.status.success(), "{}", err(&output));
    let table = out(&output);
    let (hotfix, nightly) = (
        table.find("hotfix").unwrap(),
        table.find("nightly").unwrap(),
    );
    assert!(hotfix < nightly, "newest first:\n{table}");
    assert!(table.contains("staging"), "{table}");
}

#[tokio::test]
async fn it_shows_one_run_and_can_show_only_its_failures() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let all = out(&sbase(&["--run", "1"], Some(&db)));
    let failed = out(&sbase(&["--run", "1", "--failed"], Some(&db)));

    assert!(all.contains("PASS") && all.contains("login"), "{all}");
    assert!(all.contains("FAIL") && all.contains("checkout"), "{all}");
    assert!(
        failed.contains("checkout") && failed.contains("timed out waiting"),
        "{failed}"
    );
    assert!(!failed.contains("PASS"), "only failures:\n{failed}");
}

#[tokio::test]
async fn it_finds_the_flaky_test_and_not_the_broken_one() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let output = sbase(&["--flaky"], Some(&db));

    let text = out(&output);
    assert!(text.contains("login"), "{text}");
    assert!(
        !text.contains("checkout"),
        "always failing is broken, not flaky:\n{text}"
    );
}

#[tokio::test]
async fn json_output_parses() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let runs: Value = serde_json::from_str(&out(&sbase(&["--json"], Some(&db)))).unwrap();
    let run: Value =
        serde_json::from_str(&out(&sbase(&["--run", "2", "--json"], Some(&db)))).unwrap();

    assert_eq!(runs.as_array().unwrap().len(), 2);
    assert_eq!(runs[0]["label"], "hotfix");
    assert_eq!(run["run"]["id"], 2);
    assert_eq!(run["results"][0]["name"], "login");
    assert_eq!(run["results"][0]["passed"], false);
}

#[tokio::test]
async fn the_database_can_come_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let output = Command::new(SBASE)
        .arg("report")
        .env("SB_REPORT_DB", &db)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", err(&output));
    assert!(out(&output).contains("nightly"));
}

#[test]
fn a_missing_database_is_an_error_and_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("typo.db");

    let output = sbase(&[], Some(&db));

    assert!(!output.status.success());
    assert!(
        err(&output).contains("no results database"),
        "{}",
        err(&output)
    );
    assert!(!db.exists(), "reading must not create a database");
}

#[tokio::test]
async fn an_unknown_run_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    let output = sbase(&["--run", "99"], Some(&db));

    assert!(!output.status.success());
    assert!(err(&output).contains("no run 99"), "{}", err(&output));
}

#[tokio::test]
async fn options_that_make_no_sense_together_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("results.db");
    database(&db).await;

    assert!(
        !sbase(&["--failed"], Some(&db)).status.success(),
        "--failed needs --run"
    );
    assert!(
        !sbase(&["--run", "1", "--flaky"], Some(&db))
            .status
            .success(),
        "--run and --flaky conflict"
    );
    assert!(
        !sbase(&["--run", "0"], Some(&db)).status.success(),
        "run numbers start at 1"
    );
}
