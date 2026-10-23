//! The renderers of a run: the pie chart, the dashboard, the HTML report and
//! the JUnit XML. Golden files hold the exact output for a fixed run (rewrite
//! them with `UPDATE_GOLDEN=1`), and one test drives a real `TestRun` to see
//! that the three outputs agree. No browser is needed.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use seleniumbase_rs::core::dashboard::{
    pie_chart_svg, render_dashboard, render_html_report, PageOptions, LAST_REPORT_FILE,
};
#[cfg(unix)]
use seleniumbase_rs::core::junit::render_junit_xml_for_host;
use seleniumbase_rs::core::log_helper::TestId;
use seleniumbase_rs::core::report_helper::{FailureDetails, RunSnapshot, TestResult, TestStatus};
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::SeleniumBaseError;

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/reports")
        .join(name)
}

fn assert_golden(actual: &str, name: &str) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("golden folder")).expect("create golden folder");
        fs::write(&path, actual).expect("write golden file");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "{name} differs from its golden file; rerun with UPDATE_GOLDEN=1 if the change is intended"
    );
}

/// Three tests of a run in `/run`: one passed, one failed with evidence, one
/// skipped. The time of the run is fixed, so the output is the same on every
/// machine (the time zone only appears in the `Last updated` line, which
/// [`without_the_clock`] removes).
fn three_tests(finished: bool) -> RunSnapshot {
    let at = Local.with_ymd_and_hms(2026, 10, 22, 15, 24, 1).unwrap();
    let folder = "latest_logs/login_chrome.tests.test_bad_password";
    let mut failed = TestResult::new(
        "login_chrome::tests::test_bad_password",
        TestStatus::Failed,
        1.25,
        "assertion failed: the error text was missing\nsecond line",
    );
    failed.log_dir = Some(folder.to_owned());
    failed.failure = Some(FailureDetails {
        kind: "assertion_failed".to_owned(),
        hint: Some("The page did not match what the test expected.".to_owned()),
        url: Some("https://example.test/login".to_owned()),
        title: Some("Login".to_owned()),
        screenshot: Some(format!("{folder}/screenshot.png")),
        page_source: Some(format!("{folder}/page_source.html")),
        detail: Some("assertion failed: the error text was missing".to_owned()),
    });
    RunSnapshot {
        title: "SeleniumBase Dashboard 📊".to_owned(),
        started_at: at,
        updated_at: at,
        finished,
        results: vec![
            TestResult::new(
                "login_chrome::tests::test_login",
                TestStatus::Passed,
                0.5,
                "ok",
            ),
            failed,
            TestResult::new(
                "cart_chrome::tests::test_checkout",
                TestStatus::Skipped,
                0.0,
                "needs a payment sandbox",
            ),
        ],
        root: PathBuf::from("/run"),
    }
}

/// `html` with the `Last updated` paragraph blanked: it names the time zone of
/// the machine.
fn without_the_clock(html: &str) -> String {
    html.lines()
        .map(|line| {
            if line.starts_with("<p class=\"updated\">") {
                "<p class=\"updated\">(the time of the last update)</p>"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn options(page_dir: &str, pie: bool) -> PageOptions {
    PageOptions {
        title: "SeleniumBase Dashboard 📊".to_owned(),
        live: false,
        pie,
        page_dir: PathBuf::from(page_dir),
    }
}

#[test]
fn golden_pie_chart_of_three_kinds() {
    let run = three_tests(true);
    assert_golden(
        &pie_chart_svg(&run.summary(), &run.title),
        "pie_three_kinds.svg.golden",
    );
}

#[test]
fn golden_live_dashboard() {
    let html = render_dashboard(&three_tests(false), &options("/run", true));
    assert_golden(&without_the_clock(&html), "dashboard_live.html.golden");
}

#[test]
fn golden_final_dashboard() {
    let html = render_dashboard(&three_tests(true), &options("/run", true));
    assert_golden(&without_the_clock(&html), "dashboard_final.html.golden");
}

#[test]
fn golden_html_report_from_a_reports_folder() {
    let html = render_html_report(&three_tests(true), &options("/run/reports", true));
    assert_golden(&html, "report_final.html.golden");
}

#[cfg(unix)]
#[test]
fn golden_junit_xml_of_three_tests() {
    let xml = render_junit_xml_for_host(&three_tests(true), "ci-box");
    assert_golden(&xml, "junit_three_tests.xml.golden");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The number after `label:&nbsp;` in the dashboard's row of counts.
fn count_in(dashboard: &str, label: &str) -> usize {
    let key = format!("{label}:&nbsp;");
    let start = dashboard
        .find(&key)
        .unwrap_or_else(|| panic!("no {label} in {dashboard}"))
        + key.len();
    dashboard[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

fn run_three(run: &std::sync::Arc<TestRun>) {
    run.start(TestId::named("suite_a::tests::passes"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();
    run.start(TestId::named("suite_a::tests::fails"))
        .unwrap()
        .finish(&Err(SeleniumBaseError::AssertionFailed("title".to_owned())))
        .unwrap();
    run.start(TestId::named("suite_b::skips"))
        .unwrap()
        .finish(&Err(SeleniumBaseError::Skipped("no browser".to_owned())))
        .unwrap();
}

#[test]
fn the_dashboard_the_report_and_the_junit_xml_of_a_run_agree() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        dashboard: true,
        html: Some("reports/report.html".into()),
        junit_xml: Some("reports/junit.xml".into()),
        ..ReportSettings::default()
    };
    let run = TestRun::open(root.path(), settings, RunKey::new("renderers")).unwrap();
    run_three(&run);

    let live = read(&root.path().join("dashboard.html"));
    assert!(
        live.contains("http-equiv=\"refresh\" content=\"12\""),
        "{live}"
    );
    assert!(live.contains("Awaiting results... (Refresh the page for updates)"));
    assert!(
        !root.path().join(LAST_REPORT_FILE).exists(),
        "no last report while the run goes on"
    );

    run.finalize().unwrap();

    let dashboard = read(&root.path().join("dashboard.html"));
    assert!(!dashboard.contains("http-equiv=\"refresh\""));
    assert!(
        dashboard.contains("Test Run Complete: <b>1 test failed!</b>"),
        "{dashboard}"
    );
    assert!(dashboard.contains("<svg class=\"pie-chart\""));
    assert!(dashboard.contains(
        "<a href=\"latest_logs/\">Logs</a> / <a href=\"latest_logs/suite_a.tests.fails/\">Data</a>"
    ));

    let report = read(&root.path().join("reports/report.html"));
    assert!(
        report.contains("../latest_logs/suite_a.tests.fails/"),
        "the report links from its own folder: {report}"
    );
    assert!(
        report.contains("<svg class=\"pie-chart\""),
        "with the dashboard on, the report has the pie"
    );
    let last = read(&root.path().join(LAST_REPORT_FILE));
    assert!(
        last.contains("href=\"latest_logs/suite_a.tests.fails/\""),
        "{last}"
    );

    let xml = read(&root.path().join("reports/junit.xml"));
    let doc = roxmltree::Document::parse(&xml).unwrap_or_else(|e| panic!("{e}\n{xml}"));
    let top = doc.root_element();
    let attr = |name: &str| -> usize { top.attribute(name).unwrap().parse().unwrap() };
    assert_eq!(attr("tests"), count_in(&dashboard, "Total"));
    assert_eq!(attr("failures"), count_in(&dashboard, "Failed"));
    assert_eq!(attr("skipped"), count_in(&dashboard, "Skipped"));
    assert_eq!(attr("errors"), 0);
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name("testcase"))
            .count(),
        3
    );
    let suites: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("testsuite"))
        .filter_map(|n| n.attribute("name"))
        .collect();
    assert_eq!(suites, ["suite_a", "suite_b"]);
}

#[test]
fn a_report_in_the_dashboards_file_leaves_one_final_dashboard() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        dashboard: true,
        html: Some("dashboard.html".into()),
        ..ReportSettings::default()
    };
    let run = TestRun::open(root.path(), settings, RunKey::new("same-file")).unwrap();
    run_three(&run);
    assert!(read(&root.path().join("dashboard.html")).contains("Awaiting results"));

    run.finalize().unwrap();

    let page = read(&root.path().join("dashboard.html"));
    assert!(page.contains("Test Run Complete"), "{page}");
    assert!(page.contains("Last updated"));
    assert!(!page.contains("http-equiv=\"refresh\""));
    assert!(page.contains("<svg class=\"pie-chart\""));
}

#[test]
fn a_test_that_never_finished_is_unreported_everywhere() {
    let root = tempfile::tempdir().unwrap();
    let settings = ReportSettings {
        dashboard: true,
        junit_xml: Some("junit.xml".into()),
        ..ReportSettings::default()
    };
    let run = TestRun::open(root.path(), settings, RunKey::new("killed")).unwrap();
    std::mem::forget(run.start(TestId::named("k::killed")).unwrap());
    run.start(TestId::named("k::ok"))
        .unwrap()
        .finish(&Ok(()))
        .unwrap();

    let live_xml = read(&root.path().join("junit.xml"));
    assert!(
        !live_xml.contains("killed"),
        "an untested test is left out while the run is live: {live_xml}"
    );

    run.finalize().unwrap();

    let dashboard = read(&root.path().join("dashboard.html"));
    assert!(dashboard.contains("Test Run ENDED: Some results UNREPORTED due to skipped tearDown()"));
    assert!(dashboard.contains(">UNREPORTED<"));
    let xml = read(&root.path().join("junit.xml"));
    assert!(xml.contains("<error message=\"the test did not finish\" type=\"unreported\"/>"));
    let doc = roxmltree::Document::parse(&xml).unwrap();
    assert_eq!(doc.root_element().attribute("errors"), Some("1"));
    assert_eq!(doc.root_element().attribute("tests"), Some("2"));
}
