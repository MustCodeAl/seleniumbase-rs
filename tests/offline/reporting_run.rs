//! The run model of the reports: runs shared by processes, the recorder of
//! each test, and the logs it leaves. No browser is needed: the page the
//! recorder looks at is a fake.

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::core::log_helper::TestId;
use seleniumbase_rs::core::report_helper::TestStatus;
use seleniumbase_rs::core::test_run::{
    Console, FinishedTest, ReportSettings, RunKey, RunSink, TestRun,
};
use seleniumbase_rs::plugins::observer::PageEvidence;
use seleniumbase_rs::{Result, SeleniumBaseError};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

fn open(root: &Path, key: &str) -> Arc<TestRun> {
    TestRun::open(root, ReportSettings::default(), RunKey::new(key)).unwrap()
}

fn open_with(root: &Path, key: &str, settings: ReportSettings) -> Arc<TestRun> {
    TestRun::open(root, settings, RunKey::new(key)).unwrap()
}

fn boom() -> SeleniumBaseError {
    SeleniumBaseError::AssertionFailed("title".to_owned())
}

/// A console that keeps what is printed.
fn console() -> (Console, Arc<Mutex<String>>) {
    let printed = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&printed);
    let console: Console = Arc::new(move |text: &str| sink.lock().unwrap().push_str(text));
    (console, printed)
}

/// A page with fixed answers, or one that never answers.
struct Page {
    hangs: bool,
}

impl Page {
    fn working() -> Self {
        Self { hangs: false }
    }

    fn hanging() -> Self {
        Self { hangs: true }
    }

    async fn answer<T>(&self, value: T) -> Result<T> {
        if self.hangs {
            std::future::pending::<()>().await;
        }
        Ok(value)
    }
}

#[async_trait]
impl PageEvidence for Page {
    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        self.answer(PNG.to_vec()).await
    }

    async fn page_source(&self) -> Result<String> {
        self.answer("<html><head><title>Login</title></head></html>".to_owned())
            .await
    }

    async fn current_url(&self) -> Result<String> {
        self.answer("https://example.com/login".to_owned()).await
    }

    async fn page_title(&self) -> Result<String> {
        self.answer("Login".to_owned()).await
    }

    async fn browser_label(&self) -> Option<String> {
        self.answer(Some("Chrome 155.0.7000.1 / headless".to_owned()))
            .await
            .ok()
            .flatten()
    }

    async fn driver_label(&self) -> Option<String> {
        self.answer(Some("WebDriver".to_owned()))
            .await
            .ok()
            .flatten()
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

#[test]
fn processes_with_the_key_of_a_run_share_it_and_another_key_starts_a_new_one() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("latest_logs/old");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("screenshot.png"), b"x").unwrap();

    let first = open(root.path(), "A");
    assert!(!old.exists(), "a new run clears the logs of the last one");
    first
        .start(TestId::named("a::one"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    // Another process of the same run.
    let second = open(root.path(), "A");
    second
        .start(TestId::named("b::two"))
        .unwrap()
        .finish(&Err(boom()))
        .unwrap();

    let results = first.snapshot().unwrap().results;
    let seen: Vec<(&str, TestStatus)> = results
        .iter()
        .map(|result| (result.name.as_str(), result.status))
        .collect();
    assert_eq!(
        seen,
        [
            ("a::one", TestStatus::Passed),
            ("b::two", TestStatus::Failed)
        ]
    );
    assert!(root
        .path()
        .join("latest_logs/b.two/basic_test_info.txt")
        .is_file());

    // The next `cargo test` has another key: a new run, and clean logs.
    let third = open(root.path(), "B");
    assert!(third.snapshot().unwrap().results.is_empty());
    assert!(!root.path().join("latest_logs/b.two").exists());
}

#[test]
fn a_new_run_archives_the_logs_of_the_last_one_when_asked() {
    let root = tempfile::tempdir().unwrap();
    let first = open(root.path(), "A");
    first
        .start(TestId::named("a::one"))
        .unwrap()
        .finish(&Err(boom()))
        .unwrap();

    let archiving = ReportSettings {
        archive_logs: true,
        ..ReportSettings::default()
    };
    let _second = open_with(root.path(), "B", archiving);

    let archives: Vec<_> = fs::read_dir(root.path().join("archived_logs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(archives.len(), 1);
    let archive = &archives[0];
    assert!(archive
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("logs_"));
    assert!(archive.join("a.one/basic_test_info.txt").is_file());
    assert!(archive.join("run.json").is_file());
    assert!(read(&archive.join("results.jsonl")).contains("a::one"));
    assert_eq!(
        fs::read_dir(root.path().join("latest_logs"))
            .unwrap()
            .count(),
        0
    );
}

#[cfg(unix)]
#[test]
fn a_latest_logs_that_is_a_symbolic_link_stops_the_run_and_deletes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("precious"), b"keep").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), root.path().join("latest_logs")).unwrap();

    let error = TestRun::open(root.path(), ReportSettings::default(), RunKey::new("A"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("symbolic link"), "{error}");
    assert!(elsewhere.path().join("precious").is_file());
}

#[test]
fn many_threads_and_two_handles_write_one_run_without_losing_a_line() {
    let root = tempfile::tempdir().unwrap();
    let a = open(root.path(), "A");
    let b = open(root.path(), "A");

    std::thread::scope(|scope| {
        for thread in 0..8_u32 {
            let run = if thread % 2 == 0 { &a } else { &b };
            scope.spawn(move || {
                for case in 0..25_u32 {
                    let recorder = run
                        .start(TestId::named(&format!("t{thread}::case{case}")))
                        .unwrap();
                    recorder.finish(&Ok(())).unwrap();
                }
            });
        }
    });

    let results = a.snapshot().unwrap().results;
    assert_eq!(results.len(), 200);
    assert!(results
        .iter()
        .all(|result| result.status == TestStatus::Passed));
    let lines = read(&root.path().join(".sbrs_run/results.jsonl"));
    assert_eq!(lines.lines().count(), 400, "a start and an end for each");
    for line in lines.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["v"], 1);
    }
}

#[test]
fn files_of_a_newer_format_are_refused_with_the_folder_named() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join(".sbrs_run");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("run.json"), r#"{"v":2,"key":"A"}"#).unwrap();

    let error = TestRun::open(root.path(), ReportSettings::default(), RunKey::new("A"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("format 2"), "{error}");
    assert!(error.contains(".sbrs_run"), "{error}");
}

// ---------------------------------------------------------------------------
// The recorder
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_failure_leaves_the_three_files_and_prints_the_fail_page() {
    let root = tempfile::tempdir().unwrap();
    let (console, printed) = console();
    let settings = ReportSettings {
        list_fail_pages: true,
        ..ReportSettings::default()
    };
    let run = TestRun::open_with_console(root.path(), settings, RunKey::new("A"), console).unwrap();
    let mut recorder = run.start(TestId::named("shop::checkout")).unwrap();

    recorder.on_failure(&boom(), &Page::working()).await;
    recorder.finish(&Err(boom())).unwrap();

    let dir = root.path().join("latest_logs/shop.checkout");
    let info = read(&dir.join("basic_test_info.txt"));
    assert!(info.starts_with("shop::checkout\r\n"), "{info}");
    assert!(
        info.contains("Last Page: https://example.com/login\r\n"),
        "{info}"
    );
    assert!(
        info.contains("  Browser: Chrome 155.0.7000.1 / headless\r\n"),
        "{info}"
    );
    assert!(info.contains("   Driver: WebDriver\r\n"), "{info}");
    assert!(
        info.contains("Exception: assertion failed: title\r\n"),
        "{info}"
    );
    assert_eq!(fs::read(dir.join("screenshot.png")).unwrap(), PNG);
    assert!(PNG.starts_with(b"\x89PNG"));
    let source = read(&dir.join("page_source.html"));
    assert!(
        source.contains("<base href=\"https://example.com/login\">"),
        "{source}"
    );
    assert!(printed
        .lock()
        .unwrap()
        .contains("=> Fail Page: https://example.com/login"));

    let result = run.snapshot().unwrap().results.remove(0);
    assert_eq!(result.status, TestStatus::Failed);
    assert_eq!(result.message, "assertion failed: title");
    assert_eq!(result.log_dir.as_deref(), Some("latest_logs/shop.checkout"));
    let failure = result.failure.unwrap();
    assert_eq!(failure.url.as_deref(), Some("https://example.com/login"));
    assert_eq!(failure.title.as_deref(), Some("Login"));
    assert_eq!(
        failure.screenshot.as_deref(),
        Some("latest_logs/shop.checkout/screenshot.png")
    );
    assert_eq!(
        failure.page_source.as_deref(),
        Some("latest_logs/shop.checkout/page_source.html")
    );
}

#[tokio::test]
async fn no_screenshot_leaves_out_the_png_only_and_nothing_is_printed_unasked() {
    let root = tempfile::tempdir().unwrap();
    let (console, printed) = console();
    let settings = ReportSettings {
        no_screenshot: true,
        save_screenshot: true,
        ..ReportSettings::default()
    };
    let run = TestRun::open_with_console(root.path(), settings, RunKey::new("A"), console).unwrap();
    let mut recorder = run.start(TestId::named("t::t")).unwrap();

    recorder.on_failure(&boom(), &Page::working()).await;

    let dir = root.path().join("latest_logs/t.t");
    assert!(!dir.join("screenshot.png").exists());
    assert!(dir.join("page_source.html").is_file());
    assert!(dir.join("basic_test_info.txt").is_file());
    assert!(
        printed.lock().unwrap().is_empty(),
        "--lfp was not asked for"
    );
    drop(recorder);
}

#[tokio::test]
async fn a_passing_test_gets_a_screenshot_only_when_asked() {
    let root = tempfile::tempdir().unwrap();
    let asked = ReportSettings {
        save_screenshot: true,
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "A", asked);
    let mut recorder = run.start(TestId::named("p::one")).unwrap();
    recorder.on_success(&Page::working()).await;
    recorder.finish(&Ok(())).unwrap();
    assert!(root
        .path()
        .join("latest_logs/p.one/screenshot.png")
        .is_file());

    let unasked = tempfile::tempdir().unwrap();
    let run = open(unasked.path(), "A");
    let mut recorder = run.start(TestId::named("p::one")).unwrap();
    recorder.on_success(&Page::working()).await;
    recorder.finish(&Ok(())).unwrap();
    assert!(!unasked.path().join("latest_logs/p.one").exists());
    assert_eq!(run.snapshot().unwrap().results[0].log_dir, None);
}

#[tokio::test(start_paused = true)]
async fn a_page_that_never_answers_is_given_up_after_five_seconds_an_item() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let mut recorder = run.start(TestId::named("h::hang")).unwrap();
    let began = tokio::time::Instant::now();

    recorder.on_failure(&boom(), &Page::hanging()).await;

    assert!(began.elapsed() >= Duration::from_secs(5));
    let dir = root.path().join("latest_logs/h.hang");
    let info = read(&dir.join("basic_test_info.txt"));
    assert!(info.contains("Last Page: (Unknown Page)"), "{info}");
    assert!(info.contains("  Browser: (Unknown Browser)"), "{info}");
    assert!(!dir.join("screenshot.png").exists());
    assert!(!dir.join("page_source.html").exists());
    drop(recorder);
}

#[test]
fn a_skip_is_recorded_with_its_reason_and_is_not_a_failure() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let recorder = run.start(TestId::named("s::later")).unwrap();

    recorder
        .finish(&Err(SeleniumBaseError::Skipped(
            "no browser here".to_owned(),
        )))
        .unwrap();

    let result = run.snapshot().unwrap().results.remove(0);
    assert_eq!(result.status, TestStatus::Skipped);
    assert_eq!(result.message, "no browser here");
    assert!(result.passed());
    assert!(result.failure.is_none());
    let reason = read(&root.path().join("latest_logs/s.later/skip_reason.txt"));
    assert!(
        reason.ends_with("Skip Reason: no browser here\r\n"),
        "{reason}"
    );
}

#[test]
fn a_test_that_panics_is_failed_and_one_never_finished_is_unreported() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _recorder = run.start(TestId::named("p::boom")).unwrap();
        panic!("the test body panicked");
    }));
    assert!(panicked.is_err());
    drop(run.start(TestId::named("p::forgot")).unwrap());

    let results = run.snapshot().unwrap().results;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].status, TestStatus::Failed);
    assert_eq!(results[0].message, "the test panicked");
    assert!(root
        .path()
        .join("latest_logs/p.boom/basic_test_info.txt")
        .is_file());
    assert_eq!(results[1].status, TestStatus::Unreported);
    assert_eq!(results[1].message, "the test did not finish");
}

#[test]
fn a_rerun_replaces_the_result_and_keeps_its_place() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let first = run.start(TestId::named("r::flaky")).unwrap();
    first.finish(&Err(boom())).unwrap();
    run.start(TestId::named("r::other"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    run.start(TestId::named("r::flaky"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();

    let results = run.snapshot().unwrap().results;
    let seen: Vec<(&str, TestStatus)> = results
        .iter()
        .map(|r| (r.name.as_str(), r.status))
        .collect();
    assert_eq!(
        seen,
        [
            ("r::flaky", TestStatus::Passed),
            ("r::other", TestStatus::Passed)
        ]
    );
}

#[test]
fn a_test_that_has_started_is_untested_until_it_ends() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let recorder = run.start(TestId::named("u::running")).unwrap();

    assert_eq!(
        run.snapshot().unwrap().results[0].status,
        TestStatus::Untested
    );
    assert!(!run.snapshot().unwrap().finished);
    recorder.finish(&Ok(())).unwrap();
    assert_eq!(
        run.snapshot().unwrap().results[0].status,
        TestStatus::Passed
    );
}

#[test]
fn the_folder_of_the_running_test_is_where_the_basecase_helpers_write() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let recorder = run.start(TestId::named("q::logs")).unwrap();

    let dir = seleniumbase_rs::artifacts::ensure_test_logs_dir().unwrap();

    assert_eq!(dir, recorder.log_dir());
    assert!(dir.is_dir());
    recorder.finish(&Ok(())).unwrap();
}

#[test]
fn a_sink_hears_about_every_test() {
    struct Notes(Mutex<Vec<String>>);
    impl RunSink for Notes {
        fn started(&self, test: &TestId) {
            self.0
                .lock()
                .unwrap()
                .push(format!("start {}", test.display));
        }
        fn finished(&self, finished: &FinishedTest<'_>) {
            self.0.lock().unwrap().push(format!(
                "end {} {} error={}",
                finished.result.name,
                finished.result.status,
                finished.error.is_some()
            ));
        }
    }

    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    let notes = Arc::new(Notes(Mutex::new(Vec::new())));
    run.add_sink(Arc::clone(&notes) as Arc<dyn RunSink>);

    run.start(TestId::named("n::pass"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    run.start(TestId::named("n::fail"))
        .unwrap()
        .finish(&Err(boom()))
        .unwrap();

    assert_eq!(
        *notes.0.lock().unwrap(),
        [
            "start n::pass",
            "end n::pass passed error=false",
            "start n::fail",
            "end n::fail failed error=true"
        ]
    );
}

// ---------------------------------------------------------------------------
// Outputs and finalize
// ---------------------------------------------------------------------------

#[test]
fn finalizing_turns_unfinished_tests_into_unreported_and_can_be_repeated() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        dashboard: true,
        html: Some("report.html".into()),
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "A", settings);
    std::mem::forget(run.start(TestId::named("k::killed")).unwrap());
    run.start(TestId::named("k::ok"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    fs::create_dir_all(root.path().join("latest_logs/empty")).unwrap();

    let live = read(&root.path().join("dashboard.html"));
    assert!(
        live.contains("http-equiv=\"refresh\""),
        "a dashboard refreshes while tests run"
    );
    assert!(live.contains("UNTESTED"));

    let first = run.finalize().unwrap();

    assert_eq!(first.summary.total, 2);
    assert_eq!((first.summary.passed, first.summary.unreported), (1, 1));
    assert_eq!(
        first.dashboard.as_deref(),
        Some(root.path().join("dashboard.html").as_path())
    );
    assert_eq!(
        first.html.as_deref(),
        Some(root.path().join("report.html").as_path())
    );
    let done = read(&root.path().join("dashboard.html"));
    assert!(
        !done.contains("http-equiv=\"refresh\""),
        "a finished dashboard stays still"
    );
    assert!(read(&root.path().join("report.html")).contains("UNREPORTED"));
    assert!(
        !root.path().join("latest_logs/empty").exists(),
        "empty folders are removed"
    );
    assert!(run.snapshot().unwrap().finished);

    let second = run.finalize().unwrap();
    assert_eq!(second.summary, first.summary);
}

#[test]
fn a_test_after_finalize_re_opens_the_run() {
    let root = tempfile::tempdir().unwrap();
    let run = open(root.path(), "A");
    run.start(TestId::named("a::one"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    run.finalize().unwrap();
    assert!(run.snapshot().unwrap().finished);

    let joined = open(root.path(), "A");
    joined
        .start(TestId::named("a::two"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();

    let snapshot = run.snapshot().unwrap();
    assert!(!snapshot.finished);
    assert_eq!(snapshot.results.len(), 2);
}

#[test]
fn finalizing_archives_the_logs_when_asked() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        archive_logs: true,
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "A", settings);
    run.start(TestId::named("a::one"))
        .unwrap()
        .finish(&Err(boom()))
        .unwrap();

    let outcome = run.finalize().unwrap();
    let archive = outcome.archived.expect("the logs were archived");
    assert!(archive.join("a.one/basic_test_info.txt").is_file());
    assert!(archive.join("results.jsonl").is_file());
    assert!(
        root.path().join("latest_logs/a.one").is_dir(),
        "the logs stay for reading"
    );

    // Once is enough.
    assert_eq!(run.finalize().unwrap().archived, None);
    assert_eq!(
        fs::read_dir(root.path().join("archived_logs"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn a_report_that_cannot_be_written_is_a_warning_not_an_error() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("blocker"), b"x").unwrap();
    let settings = ReportSettings {
        html: Some("blocker/report.html".into()),
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "A", settings);

    run.start(TestId::named("w::one"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    let outcome = run.finalize().unwrap();

    assert_eq!(outcome.html, None);
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.starts_with("HTML report:")),
        "{:?}",
        outcome.warnings
    );
    assert_eq!(
        outcome
            .warnings
            .iter()
            .filter(|w| w.starts_with("HTML report:"))
            .count(),
        1,
        "a warning is kept once, not once per test"
    );
}

#[cfg(not(feature = "allure"))]
#[test]
fn an_allure_folder_without_the_feature_is_a_warning() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        allure_dir: Some("allure-results".into()),
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "A", settings);

    let outcome = run.finalize().unwrap();

    assert!(
        outcome
            .warnings
            .iter()
            .any(|w| w.contains("built without the \"allure\" feature")),
        "{:?}",
        outcome.warnings
    );
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[test]
fn the_settings_come_from_the_test_table_the_report_table_and_the_environment() {
    let mut settings = Settings::default();
    settings.test.dashboard = true;
    settings.test.dash_title = Some("My_title".to_owned());
    settings.test.list_fail_page = true;
    settings.test.no_screenshot = true;
    settings.report.junit_xml = Some("table.xml".into());
    settings.report.html = Some("table.html".into());
    let var = |name: &str| match name {
        "SB_HTML" => Some("env.html".to_owned()),
        "SB_REPORT_DB" => Some("  ".to_owned()),
        "SB_ALLURE_DIR" => Some("allure".to_owned()),
        _ => None,
    };

    let report = ReportSettings::from_settings_with(&settings, var);

    assert!(report.dashboard && report.list_fail_pages && report.no_screenshot);
    assert!(!report.archive_logs && !report.save_screenshot);
    assert_eq!(report.title(), "My title");
    assert_eq!(
        report.html.as_deref(),
        Some(Path::new("env.html")),
        "the variable wins"
    );
    assert_eq!(report.junit_xml.as_deref(), Some(Path::new("table.xml")));
    assert_eq!(report.allure_dir.as_deref(), Some(Path::new("allure")));
    assert_eq!(report.db, None, "a blank variable is unset");
    assert_eq!(report.db_path(), Path::new("reports/results.db"));
}

#[test]
fn the_report_table_of_the_settings_file_is_read() {
    let settings: Settings = toml::from_str(
        r#"
        [report]
        html = "out/report.html"
        junit_xml = "out/report.xml"
        dashboard = true
        "#,
    )
    .unwrap();

    let report = ReportSettings::from_settings_with(&settings, |_| None);

    assert_eq!(report.html.as_deref(), Some(Path::new("out/report.html")));
    assert_eq!(
        report.junit_xml.as_deref(),
        Some(Path::new("out/report.xml"))
    );
    assert!(report.dashboard);
}

#[test]
fn a_run_key_is_its_text_or_the_recent_run() {
    assert_eq!(RunKey::new("a").as_str(), Some("a"));
    assert_eq!(RunKey::recent().as_str(), None);
}

#[cfg(feature = "turso")]
#[tokio::test]
async fn the_finished_run_is_recorded_in_the_results_database_once_asked_for() {
    use seleniumbase_rs::storage::ResultStore;

    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        with_db_reporting: true,
        db: Some("reports/results.db".into()),
        database_env: Some("staging".to_owned()),
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "D", settings);
    run.start(TestId::named("d::passes"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    run.start(TestId::named("d::fails"))
        .unwrap()
        .finish(&Err(boom()))
        .unwrap();
    run.start(TestId::named("d::skips"))
        .unwrap()
        .finish(&Err(SeleniumBaseError::Skipped("no browser".to_owned())))
        .unwrap();
    std::mem::forget(run.start(TestId::named("d::killed")).unwrap());
    run.finalize().unwrap();

    let id = run
        .record_to_database()
        .await
        .unwrap()
        .expect("reporting is on");

    let store = ResultStore::open(root.path().join("reports/results.db"))
        .await
        .unwrap();
    let stored = store.results(id).await.unwrap();
    let seen: Vec<(&str, TestStatus)> =
        stored.iter().map(|r| (r.name.as_str(), r.status)).collect();
    assert_eq!(
        seen,
        [
            ("d::passes", TestStatus::Passed),
            ("d::fails", TestStatus::Failed),
            ("d::skips", TestStatus::Skipped),
            ("d::killed", TestStatus::Failed)
        ]
    );
    assert_eq!(store.run(id).await.unwrap().unwrap().environment, "staging");

    let elsewhere = tempfile::tempdir().unwrap();
    let off = open(elsewhere.path(), "E");
    assert!(
        off.record_to_database().await.unwrap().is_none(),
        "off unless asked"
    );
}

#[test]
fn a_process_id_that_comes_round_again_does_not_join_an_old_run() {
    let root = tempfile::tempdir().unwrap();
    let first = open(root.path(), "cargo-ppid-4242");
    first
        .start(TestId::named("a::one"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    let joined = open(root.path(), "cargo-ppid-4242");
    assert_eq!(
        joined.snapshot().unwrap().results.len(),
        1,
        "a recent run is joined"
    );

    // The run is days old.
    let file = root.path().join(".sbrs_run/run.json");
    let mut saved: serde_json::Value = serde_json::from_str(&read(&file)).unwrap();
    saved["updated_at"] = "2020-01-01T00:00:00.000+00:00".into();
    fs::write(&file, saved.to_string()).unwrap();

    let second = open(root.path(), "cargo-ppid-4242");
    assert!(second.snapshot().unwrap().results.is_empty());
}

#[test]
fn a_run_can_be_resumed_with_its_saved_settings_or_new_ones() {
    let root = tempfile::tempdir().unwrap();
    let saved = ReportSettings {
        html: Some("report.html".into()),
        ..ReportSettings::default()
    };
    let run = open_with(root.path(), "K", saved.clone());
    run.start(TestId::named("r::one"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();

    let resumed = TestRun::resume(root.path(), None).unwrap();
    assert_eq!(resumed.settings(), &saved);
    assert_eq!(resumed.snapshot().unwrap().results.len(), 1);
    let outcome = resumed.finalize().unwrap();
    assert_eq!(outcome.summary.total, 1);
    assert!(root.path().join("report.html").is_file());

    let changed = ReportSettings {
        dashboard: true,
        ..ReportSettings::default()
    };
    let again = TestRun::resume(root.path(), Some(changed.clone())).unwrap();
    assert_eq!(again.settings(), &changed);

    let empty = tempfile::tempdir().unwrap();
    let error = TestRun::resume(empty.path(), None).unwrap_err().to_string();
    assert!(error.contains("there is no test run in"), "{error}");
    assert!(error.contains(".sbrs_run"), "{error}");
    assert!(!empty.path().join(".sbrs_run").exists(), "nothing was made");
}
