//! Real-browser checks of `MasterQA`: the box a check shows, the answers, and
//! the files `finish` writes.
//!
//! A check waits for a person, so each test plays one: a script that runs in
//! the page, waits for each box to appear and then clicks its YES / PASS or
//! NO / FAIL button. The browser is headless, which would make `verify` skip
//! the checks, so the case is told that somebody watches
//! (`BaseCase::with_somebody_watching`, in the `test-util` feature). The
//! report folders go under a temporary folder (`SB_REPORT_ROOT`).
//!
//! These tests are ignored by default; they need Chrome and, for the first,
//! a network that can reach cdnjs, from where the page loads jQuery and
//! jquery-confirm. Run them locally with:
//!
//! ```bash
//! cargo test --test masterqa_chrome --features test-util -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

#![cfg(feature = "test-util")]

use seleniumbase_rs::api::master_qa::MasterQA;
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port, and the tests that set `SB_REPORT_ROOT`.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with nothing in it but a paragraph.
const PAGE: &str = "data:text/html,<html><head><title>qa</title></head><body>\
    <p id=\"here\">hello</p></body></html>";

/// A page whose policy forbids every script, so jquery-confirm cannot load.
const LOCKED_PAGE: &str = "data:text/html,<html><head><title>qa</title>\
    <meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'none'\">\
    </head><body><p>locked</p></body></html>";

/// The person: for each box that opens, wait a moment and click the button the
/// next of `window.__answers` names (0 is YES / PASS, 1 is NO / FAIL).
const THE_PERSON: &str = "window.__answers = [0, 1]; \
    setInterval(function () { \
        var box = document.querySelector('.jconfirm-box'); \
        var buttons = document.querySelectorAll('.jconfirm-buttons button'); \
        if (!box || !buttons.length || box.dataset.handled || !window.__answers.length) { return; } \
        box.dataset.handled = '1'; \
        var pick = window.__answers.shift(); \
        setTimeout(function () { buttons[pick].click(); }, 700); \
    }, 50); \
    return true;";

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// A case in a headless browser that behaves as if somebody watches, on `page`.
async fn watched_case(page: &str) -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser")
        .with_somebody_watching();
    // A box nobody answers waits for ever; the time limit ends it.
    sb.set_time_limit(120).expect("set the limit");
    sb.open(page).await.expect("open the page");
    sb
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_yes_and_a_no_are_recorded_and_the_run_is_archived() {
    let _guard = BROWSER_LOCK.lock().await;
    let root = tempfile::tempdir().expect("a temporary folder");
    std::env::set_var("SB_REPORT_ROOT", root.path());
    let sb = watched_case(PAGE).await;
    sb.execute_script(THE_PERSON)
        .await
        .expect("the person is ready");

    let mut qa = MasterQA::new(sb).expect("start the run");
    let first = qa
        .verify("The paragraph says hello")
        .await
        .expect("check 1");
    let second = qa.verify("The page is blue?").await.expect("check 2");
    qa.auto_close_results();
    let (mut sb, report) = qa.finish(None).await.expect("finish the run");

    assert_eq!((first, second), (Some(true), Some(false)));
    assert_eq!(
        (report.checks, report.passed, report.failed, report.skipped),
        (2, 1, 1, 0)
    );
    let table = std::fs::read_to_string(&report.results_table).expect("the table");
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(
        lines[0],
        r#""Num","Result","Screenshot","URL","Browser","Epoch Time","Verification Instructions","Additional Info""#
    );
    assert!(lines[1].starts_with(r#""1","Success","-","#), "{table}");
    assert!(lines[1].contains(r#","chrome","#), "{table}");
    assert!(
        lines[2].starts_with(r#""2","FAILED!","failed_check_2.png","#),
        "{table}"
    );
    // The failure kept a real screenshot, and the run moved to the archive.
    let png = std::fs::read(report.archive_dir.join("failed_check_2.png")).expect("the PNG");
    assert!(png.starts_with(b"\x89PNG"), "not a PNG");
    assert!(report.archive_dir.join("report.html").is_file());
    assert!(report.results_page.is_file());
    assert!(report
        .archive_dir
        .starts_with(root.path().join("archived_reports")));
    assert!(!root
        .path()
        .join("latest_report")
        .join("results_table.csv")
        .exists());

    sb.quit().await.expect("quit");
    std::env::remove_var("SB_REPORT_ROOT");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_page_that_forbids_scripts_is_asked_with_the_browsers_own_confirm() {
    let _guard = BROWSER_LOCK.lock().await;
    let root = tempfile::tempdir().expect("a temporary folder");
    std::env::set_var("SB_REPORT_ROOT", root.path());
    let sb = watched_case(LOCKED_PAGE).await;
    // A native box cannot be clicked from the page, so `confirm` is answered
    // by the page itself: it says no.
    sb.execute_script("window.confirm = function () { return false; }; return true;")
        .await
        .expect("replace confirm");

    let mut qa = MasterQA::new(sb).expect("start the run");
    let verdict = qa.verify("Is it locked?").await.expect("the check");
    qa.auto_close_results();
    let (mut sb, report) = qa.finish(None).await.expect("finish the run");

    assert_eq!(verdict, Some(false));
    assert_eq!((report.checks, report.passed, report.failed), (1, 0, 1));
    sb.quit().await.expect("quit");
    std::env::remove_var("SB_REPORT_ROOT");
}
