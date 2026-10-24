//! The reports of the fixtures on a real headless Chrome. `#[ignore]`d: run
//! them with `cargo test --features testing --test sb_reports_chrome -- --ignored`,
//! one binary at a time. The mock runs of `tests/sb_reports.rs` cover the rest.
#![cfg(feature = "testing")]

use std::sync::Arc;

use seleniumbase_rs::core::report_helper::TestStatus;
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::testing::SbCdp;
use seleniumbase_rs::{SeleniumBaseError, Settings};

const PAGE: &str = "data:text/html,<title>Reports</title><h1>Hello</h1>";

#[tokio::test]
#[ignore = "starts a real Chrome"]
async fn a_failing_cdp_test_is_on_the_dashboard_with_the_evidence_of_a_real_page() {
    let root = tempfile::tempdir().unwrap();
    let run = TestRun::open(
        root.path(),
        ReportSettings {
            dashboard: true,
            html: Some("report.html".into()),
            junit_xml: Some("junit.xml".into()),
            ..ReportSettings::default()
        },
        RunKey::new("sb-reports-chrome"),
    )
    .unwrap();
    let fixture = SbCdp::new(Settings {
        headless: true,
        ..Settings::default()
    })
    .report_to(Arc::clone(&run));
    let id = fixture.test_id().to_owned();
    let live = Arc::clone(&run);

    let result = fixture
        .run(async |page| {
            page.goto(PAGE).await?;
            assert_eq!(
                live.snapshot()?.results[0].status,
                TestStatus::Untested,
                "the dashboard shows the test while its browser runs"
            );
            Err(SeleniumBaseError::AssertionFailed("on purpose".to_owned()))
        })
        .await;
    assert!(result.is_err());

    let results = run.snapshot().unwrap().results;
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].status, TestStatus::Failed);
    let failure = results[0].failure.as_ref().expect("the failure's details");
    assert!(
        failure
            .url
            .as_deref()
            .is_some_and(|url| url.starts_with("data:text/html")),
        "{failure:?}"
    );
    let folder = root.path().join("latest_logs").join(&id);
    let png = std::fs::read(folder.join("screenshot.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"), "a real screenshot");
    assert!(std::fs::read_to_string(folder.join("page_source.html"))
        .unwrap()
        .contains("<h1>Hello</h1>"));
    let info = std::fs::read_to_string(folder.join("basic_test_info.txt")).unwrap();
    assert!(info.contains("Last Page: data:text/html"), "{info}");
    assert!(
        info.contains("\r\n") && info.contains("Traceback:"),
        "{info}"
    );

    let ended = run.finalize().unwrap();
    assert_eq!(ended.summary.failed, 1);
    for file in ["dashboard.html", "report.html", "junit.xml"] {
        let text = std::fs::read_to_string(root.path().join(file)).unwrap();
        assert!(
            text.contains("a_failing_cdp_test_is_on_the_dashboard"),
            "{file}"
        );
    }
    let dashboard = std::fs::read_to_string(root.path().join("dashboard.html")).unwrap();
    assert!(dashboard.contains("Test Run Complete"), "{dashboard}");
    assert!(
        !dashboard.contains("http-equiv=\"refresh\""),
        "a finished page stays still"
    );
}
