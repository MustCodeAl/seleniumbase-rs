//! The run configuration of the environment turns the reports on by itself:
//! with `SB_DASHBOARD` set (what `SB_DASHBOARD=1 cargo test` or `sbrs test
//! --dashboard` does), a test that uses the fixture is started in the run of the
//! process and shows on `dashboard.html` while it runs, with no line of
//! reporting code in the test. The variables are set once for this binary, which
//! is why it is a binary of its own.
#![cfg(all(feature = "testing", feature = "test-util"))]

use std::path::Path;
use std::sync::OnceLock;

use base64::prelude::*;
use seleniumbase_rs::core::report_helper::TestStatus;
use seleniumbase_rs::core::test_run::TestRun;
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::testing::SbCdp;
use seleniumbase_rs::Result;
use serde_json::json;

/// The folder of the run, made (and put in the variables) the first time.
fn root() -> &'static Path {
    static ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();
    ROOT.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("SB_REPORT_ROOT", dir.path());
        std::env::set_var("SB_RUN_ID", "sb-reports-env");
        std::env::set_var("SB_DASHBOARD", "true");
        std::env::set_var("SB_DASH_TITLE", "Env_run");
        dir
    })
    .path()
}

async fn mocked() -> Result<Browser> {
    let (browser, mock) = Browser::new_mocked();
    mock.reply(
        "Runtime.evaluate",
        json!({ "result": { "value": "https://shop.test/cart" } }),
    );
    mock.reply(
        "Page.captureScreenshot",
        json!({ "data": BASE64_STANDARD.encode(b"\x89PNG") }),
    );
    Ok(browser)
}

/// The status the run has for the test whose name has `name` in it.
fn status_of(name: &str) -> TestStatus {
    let results = TestRun::current().unwrap().snapshot().unwrap().results;
    results
        .iter()
        .find(|result| result.name.contains(name))
        .unwrap_or_else(|| panic!("no result for {name}: {results:?}"))
        .status
}

#[tokio::test]
async fn a_fixture_test_reports_by_itself_when_the_dashboard_is_on() {
    let root = root();
    SbCdp::from_env()
        .run_with_browser(
            async || mocked().await,
            async |_page| {
                let page = std::fs::read_to_string(root.join("dashboard.html")).unwrap();
                assert!(
                    page.contains("a_fixture_test_reports_by_itself_when_the_dashboard_is_on"),
                    "the dashboard shows the test while it runs: {page}"
                );
                assert!(
                    page.contains("Env run"),
                    "the title, with its underscores read as spaces"
                );
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(
        status_of("a_fixture_test_reports_by_itself_when_the_dashboard_is_on"),
        TestStatus::Passed
    );
    assert!(root.join(".sbrs_run").join("results.jsonl").is_file());
}

#[cfg(feature = "bdd")]
#[tokio::test(flavor = "multi_thread")]
async fn a_scenario_reports_by_itself_when_the_dashboard_is_on() {
    use seleniumbase_rs::testing::bdd::SbCdpCase;
    use seleniumbase_rs::Settings;

    let root = root();
    let logs = tempfile::tempdir().unwrap();
    {
        let case = SbCdpCase::launch_with_browser(Settings::load().unwrap(), logs.path(), mocked)
            .await
            .unwrap();
        let _ = case.url().await;
        let page = std::fs::read_to_string(root.join("dashboard.html")).unwrap();
        assert!(
            page.contains("a_scenario_reports_by_itself_when_the_dashboard_is_on"),
            "a scenario is on the dashboard while its steps run: {page}"
        );
        // Dropping the case ends the scenario, on the lifecycle's own thread.
    }
    assert_eq!(
        status_of("a_scenario_reports_by_itself_when_the_dashboard_is_on"),
        TestStatus::Passed
    );
}
