// Tests of the `MasterQA` wrapper against the mock WebDriver in
// `webdriver_support` (no browser needed). The files of a run go under
// `SB_REPORT_ROOT`, which is process-wide, so the tests that write them hold
// `ROOT_LOCK`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::{json, Value};

use super::*;
use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
use crate::api::jqc::{self, ANSWER_SCRIPT, OPEN_COUNT_SCRIPT};
use crate::api::run_settings::RunSettings;
use crate::artifacts::REPORT_ROOT_VARIABLE;
use crate::browser::config::BrowserConfig;
use crate::browser::session::BrowserSession;

/// The start of every PNG: enough for the driver to write a file.
const PNG_BASE64: &str = "iVBORw0KGgo=";

static ROOT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A folder for the files of one test, which `logs_root` answers with while
/// it lives.
struct Root {
    dir: tempfile::TempDir,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl Root {
    async fn new() -> Self {
        let lock = ROOT_LOCK.lock().await;
        let dir = tempfile::tempdir().expect("a temporary folder");
        std::env::set_var(REPORT_ROOT_VARIABLE, dir.path());
        Self { dir, _lock: lock }
    }

    fn latest(&self) -> PathBuf {
        self.dir.path().join("latest_report")
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        std::env::remove_var(REPORT_ROOT_VARIABLE);
    }
}

/// A browser that shows one dialog after another: each is closed at the first
/// poll and answered with the next of `answers` (the last one repeats).
/// `jquery_loads` says whether the libraries load. While `alerts` is above
/// zero a `confirm` box is open, and the page then reports `confirmed`.
fn page_driver(
    answers: Vec<&'static str>,
    jquery_loads: bool,
    alerts: usize,
    confirmed: &'static str,
) -> MockWebDriver {
    let answered = AtomicUsize::new(0);
    let alert_polls = AtomicUsize::new(0);
    MockWebDriver::start(json!({}), move |request| {
        let script = request.body["script"].as_str().unwrap_or_default();
        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/execute/async") => Reply::Value(json!(jquery_loads)),
            ("POST", "/execute/sync") if script == "return document.readyState;" => {
                Reply::Value(json!("complete"))
            }
            ("POST", "/execute/sync") if script == OPEN_COUNT_SCRIPT => Reply::Value(json!(0)),
            ("POST", "/execute/sync") if script == ANSWER_SCRIPT => {
                let index = answered.fetch_add(1, Ordering::SeqCst);
                let button = answers[index.min(answers.len() - 1)];
                Reply::Value(json!({ "button": button, "input": "" }))
            }
            ("POST", "/execute/sync") if script == "return window.master_qa_result" => {
                Reply::Value(json!(confirmed))
            }
            ("GET", "/alert/text") => {
                if alert_polls.fetch_add(1, Ordering::SeqCst) < alerts {
                    Reply::Value(json!("Approve?"))
                } else {
                    Reply::Error(404, "no such alert")
                }
            }
            ("GET", "/url") => Reply::Value(json!("https://page.test/a")),
            ("GET", "/screenshot") => Reply::Value(json!(PNG_BASE64)),
            _ => Reply::Value(Value::Null),
        }
    })
}

async fn case(driver: &MockWebDriver, headless: bool) -> BaseCase {
    let mut config = BrowserConfig::default()
        .with_headless(headless)
        .with_webdriver_url(driver.url().to_owned());
    config.auto_start_driver = false;
    let mut sb = BaseCase::new(config)
        .await
        .expect("the mock accepts a session");
    sb.set_run_settings(RunSettings::default().with_verify_delay(Duration::ZERO));
    sb
}

fn scripts(driver: &MockWebDriver) -> Vec<String> {
    driver
        .requests("/execute/sync")
        .iter()
        .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
        .collect()
}

/// `text` as it is written inside a JavaScript string literal of a script (the
/// scripts escape angle brackets, quotes and the like).
fn escaped(text: &str) -> String {
    crate::js_code::quote(text).trim_matches('"').to_owned()
}

fn message_of(error: SeleniumBaseError) -> String {
    error.to_string()
}

#[test]
fn the_dialog_script_is_pythons_box() {
    let script = jqc::masterqa_dialog_script(3, "Is <b>it</b> \"blue\"?");
    assert!(script.contains("boxWidth: '32.5%'"), "{script}");
    assert!(script.contains("theme: 'bootstrap'"));
    assert!(script.contains("Manual Check #3:"));
    assert!(script.contains("YES / PASS") && script.contains("keys: ['y', 'p', '1']"));
    assert!(script.contains("NO / FAIL") && script.contains("keys: ['n', 'f', '2']"));
    assert!(script.contains("btn-green") && script.contains("btn-red"));
    // The question stays HTML, and a quote in it cannot end the string.
    assert!(script.contains(&escaped("Is <b>it</b>")), "{script}");
    assert!(script.contains(r#"\"blue\""#), "{script}");
    // The buttons store the answer that is read afterwards.
    assert!(script.contains("window.__sbJqcAnswer = { button: button"));
    assert!(script.contains("\"Success!\"") && script.contains("\"Failure!\""));
}

#[test]
fn the_futures_of_the_calls_are_send() {
    fn assert_send<T: Send>(_: &T) {}
    let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
    // `MasterQA::new` needs the report folder, so build the value by hand.
    let mut qa = MasterQA {
        sb,
        rows: Vec::new(),
        counts: Counts::default(),
        auto_close_results: true,
    };
    assert_send(&qa.verify("x"));
    assert_send(&qa.verify_default());
}

#[tokio::test]
async fn new_deletes_what_an_earlier_run_left_and_nothing_else() {
    let root = Root::new().await;
    let latest = root.latest();
    std::fs::create_dir_all(&latest).unwrap();
    for name in [
        "failed_check_1.png",
        "automation_failure_1.png",
        "report.html",
        "results_table.csv",
        "keep.txt",
        "other.png",
    ] {
        std::fs::write(latest.join(name), "x").unwrap();
    }
    let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());

    MasterQA::new(sb).unwrap();

    let mut left: Vec<String> = std::fs::read_dir(&latest)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["keep.txt", "other.png"]);
}

#[tokio::test]
async fn where_nobody_watches_a_check_is_skipped_and_asks_nothing() {
    let _root = Root::new().await;
    let driver = page_driver(vec!["Success!"], true, 0, "");
    let mut qa = MasterQA::new(case(&driver, true).await).unwrap();

    assert_eq!(qa.verify("Is it blue?").await.unwrap(), None);
    assert_eq!(qa.verify_default().await.unwrap(), None);

    assert_eq!(qa.counts.skipped, 2);
    assert_eq!(qa.counts.answered, 0);
    assert!(qa.rows.is_empty());
    assert!(!scripts(&driver).iter().any(|s| s.contains("jconfirm({")));
    // A run nobody watches closes its results page by itself.
    assert!(qa.auto_close_results);
}

#[tokio::test]
async fn a_passed_check_waits_the_verify_delay_and_is_recorded() {
    let _root = Root::new().await;
    let driver = page_driver(vec!["Success!"], true, 0, "");
    let mut qa = MasterQA::new(case(&driver, false).await).unwrap();
    qa.set_run_settings(RunSettings::default().with_verify_delay(Duration::from_millis(150)));

    let started = Instant::now();
    let verdict = qa.verify("Is it blue?").await.unwrap();

    assert_eq!(verdict, Some(true));
    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(qa.rows.len(), 1);
    let line = qa.rows[0].csv_line();
    assert!(
        line.starts_with(r#""1","Success","-","https://page.test/a","chrome","#),
        "{line}"
    );
    assert!(line.ends_with(r#","Is it blue?","*""#), "{line}");
    let opened = scripts(&driver)
        .into_iter()
        .find(|script| script.contains("jconfirm({"))
        .expect("the box was opened");
    assert!(opened.contains("Manual Check #1:"), "{opened}");
    // "Is it blue?" has its question mark, so it is asked as it is.
    assert!(!opened.contains(&escaped("<> Approve?")));
}

#[tokio::test]
async fn a_failed_check_keeps_a_screenshot_and_does_not_fail_the_test() {
    let root = Root::new().await;
    let driver = page_driver(vec!["Success!", "Failure!"], true, 0, "");
    let mut qa = MasterQA::new(case(&driver, false).await).unwrap();

    assert_eq!(qa.verify("The logo is blue").await.unwrap(), Some(true));
    assert_eq!(qa.verify("The footer is there").await.unwrap(), Some(false));

    assert_eq!(
        qa.counts,
        Counts {
            answered: 2,
            passed: 1,
            incomplete: 0,
            skipped: 0
        }
    );
    let failed = &qa.rows[1];
    assert_eq!(failed.verdict, Verdict::Failed);
    assert_eq!(failed.screenshot, "failed_check_2.png");
    assert!(root.latest().join("failed_check_2.png").is_file());
    assert!(!root.latest().join("failed_check_1.png").exists());
    // Text without a question mark is asked to be approved, and numbered.
    let second = scripts(&driver)
        .into_iter()
        .filter(|script| script.contains("jconfirm({"))
        .nth(1)
        .unwrap();
    assert!(second.contains("Manual Check #2:"));
    assert!(
        second.contains(&escaped("The footer is there <> Approve?")),
        "{second}"
    );
}

#[tokio::test]
async fn finish_writes_the_table_and_the_page_and_archives_the_run() {
    let root = Root::new().await;
    let driver = page_driver(vec!["Success!", "Failure!"], true, 0, "");
    let mut qa = MasterQA::new(case(&driver, false).await).unwrap();
    qa.verify("Is it blue?").await.unwrap();
    qa.verify("Is it round?").await.unwrap();
    qa.auto_close_results();

    let (mut sb, report) = qa.finish(None).await.unwrap();

    assert_eq!(
        (
            report.checks,
            report.passed,
            report.failed,
            report.skipped,
            report.incomplete_runs
        ),
        (2, 1, 1, 0, 0)
    );
    // The run moved to `archived_reports/log_<epoch>` with its table, page and
    // screenshots, and the folder it came from is empty again but for the page.
    let archived = root.dir.path().join("archived_reports");
    assert_eq!(
        report.archive_dir.parent().map(|p| p.file_name()),
        Some(archived.file_name())
    );
    let table = std::fs::read_to_string(&report.results_table).unwrap();
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 3, "{table}");
    assert_eq!(lines[0], report::CSV_HEADER);
    assert!(
        lines[2].starts_with(r#""2","FAILED!","failed_check_2.png""#),
        "{table}"
    );
    assert!(report.archive_dir.join("failed_check_2.png").is_file());
    assert!(report.archive_dir.join(RESULTS_PAGE).is_file());
    assert!(!root.latest().join(RESULTS_TABLE).exists());
    assert!(!root.latest().join("failed_check_2.png").exists());
    assert!(report
        .results_page
        .starts_with(std::path::absolute(root.latest()).unwrap()));
    let page = std::fs::read_to_string(&report.results_page).unwrap();
    assert!(page.contains("CHECKS PASSED: <td>1</tr>"), "{page}");
    assert!(page.contains("failed_check_2.png"));
    assert_eq!(
        page,
        std::fs::read_to_string(report.archive_dir.join(RESULTS_PAGE)).unwrap()
    );
    // The archived page was opened in the browser.
    let opened: Vec<String> = driver
        .requests("/url")
        .iter()
        .filter(|request| request.method == "POST")
        .filter_map(|request| request.body["url"].as_str().map(str::to_owned))
        .collect();
    let archived_page = format!("file://{}", report.archive_dir.join(RESULTS_PAGE).display());
    assert!(
        opened.contains(&archived_page),
        "{opened:?} {archived_page}"
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
async fn an_error_in_the_test_adds_an_err_row_and_a_screenshot() {
    let root = Root::new().await;
    let driver = page_driver(vec!["Success!"], true, 0, "");
    let mut qa = MasterQA::new(case(&driver, false).await).unwrap();
    qa.verify("Is it blue?").await.unwrap();
    qa.auto_close_results();
    let error = SeleniumBaseError::WaitTimeout("the page never loaded".to_owned());

    let (_sb, report) = qa.finish(Some(&error)).await.unwrap();

    assert_eq!((report.checks, report.incomplete_runs), (1, 1));
    let table = std::fs::read_to_string(&report.results_table).unwrap();
    let err_line = table.lines().last().unwrap();
    assert!(
        err_line.starts_with(
            r#""ERR","ERROR!","automation_failure_1.png","https://page.test/a","chrome","#
        ),
        "{err_line}"
    );
    assert!(
        err_line.ends_with(r#","-","wait timeout: the page never loaded""#),
        "{err_line}"
    );
    assert!(report
        .archive_dir
        .join("automation_failure_1.png")
        .is_file());
    drop(root);
}

#[tokio::test]
async fn a_page_that_cannot_load_jquery_confirm_is_asked_with_confirm() {
    let _root = Root::new().await;
    // The libraries do not load; the box is open for two polls; it was
    // answered "Failure!".
    let driver = page_driver(vec!["Success!"], false, 2, "Failure!");
    let mut qa = MasterQA::new(case(&driver, false).await).unwrap();

    let verdict = qa.verify("Is it blue?").await.unwrap();

    assert_eq!(verdict, Some(false));
    let sent = scripts(&driver);
    let asked = sent
        .iter()
        .find(|script| script.contains("confirm("))
        .expect("the question was asked with confirm");
    assert!(!asked.contains("jconfirm({"), "{asked}");
    assert!(asked.contains("Is it blue?"));
    assert!(!sent.iter().any(|script| script.contains("jconfirm({")));
}

#[tokio::test]
async fn nobody_answering_the_confirm_box_quits_the_browser_and_fails() {
    let driver = page_driver(vec!["Success!"], false, usize::MAX, "");
    let mut sb = case(&driver, false).await;

    let error = sb
        .masterqa_confirm("Is it blue?", Duration::from_millis(200))
        .await
        .unwrap_err();

    assert_eq!(
        message_of(error),
        "wait timeout: 0.2 seconds passed without human action! Stopping..."
    );
    let quit = driver
        .all_requests()
        .into_iter()
        .any(|request| request.method == "DELETE");
    assert!(quit, "the session was not deleted");
}

#[tokio::test]
async fn an_alert_that_is_not_the_question_is_not_waited_for() {
    let driver = MockWebDriver::start(json!({}), |request| {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/alert/text") => Reply::Value(json!("Leave this page")),
            ("POST", "/execute/sync") => Reply::Value(json!("Success!")),
            _ => Reply::Value(Value::Null),
        }
    });
    let mut sb = case(&driver, false).await;
    assert!(sb
        .masterqa_confirm("Is it blue?", Duration::from_secs(5))
        .await
        .unwrap());
}
