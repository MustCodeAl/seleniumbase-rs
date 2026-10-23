//! The `sb` and `sb_cdp` fixtures on a real headless Chrome. `#[ignore]`d: run
//! them with `cargo test --features testing --test sb_fixture_chrome -- --ignored`,
//! one binary at a time. The mock runs of `tests/sb_fixture.rs` cover the rest.
#![cfg(feature = "testing")]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use rstest::rstest;
use seleniumbase_rs::config::test_settings::TestData;
use seleniumbase_rs::testing::{sb, sb_cdp, Sb, SbCdp};
use seleniumbase_rs::{Result, SeleniumBaseError, Settings};

const PAGE: &str = "data:text/html,<title>Fixture</title><h1>Hello</h1>";

fn headless(change: impl FnOnce(&mut Settings)) -> Settings {
    let mut settings = Settings {
        headless: true,
        ..Settings::default()
    };
    change(&mut settings);
    settings
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn failure() -> Result<()> {
    Err(SeleniumBaseError::AssertionFailed("on purpose".to_owned()))
}

#[tokio::test]
#[ignore = "starts a real Chrome"]
async fn a_failing_cdp_test_leaves_the_three_files_of_the_page_it_ended_on() {
    let logs = tempfile::tempdir().unwrap();
    let fixture = SbCdp::new(headless(|_| {})).logs_dir(logs.path());
    let id = fixture.test_id().to_owned();
    let result = fixture
        .run(async |page| {
            page.goto(PAGE).await?;
            assert_eq!(page.title().await?, "Fixture");
            failure()
        })
        .await;
    assert!(
        matches!(result, Err(SeleniumBaseError::AssertionFailed(_))),
        "{result:?}"
    );
    let folder = logs.path().join(id);
    assert_eq!(
        names(&folder),
        ["basic_test_info.txt", "page_source.html", "screenshot.png"]
    );
    let png = std::fs::read(folder.join("screenshot.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"), "a real screenshot");
    assert!(std::fs::read_to_string(folder.join("page_source.html"))
        .unwrap()
        .contains("<h1>Hello</h1>"));
    let info = std::fs::read_to_string(folder.join("basic_test_info.txt")).unwrap();
    assert!(info.contains("Last Page: data:text/html"), "{info}");
    assert!(
        info.contains("Browser: Chrome") || info.contains("Browser: Chromium"),
        "{info}"
    );
    assert!(info.contains("/ headless"), "{info}");
}

#[rstest]
#[tokio::test]
#[ignore = "starts a real Chrome"]
async fn a_failing_webdriver_test_leaves_the_three_files(sb: Sb) {
    let logs = tempfile::tempdir().unwrap();
    let sb = sb
        .configure(|settings| settings.headless = true)
        .logs_dir(logs.path());
    let id = sb.test_id().to_owned();
    let result = sb
        .run(async |sb| {
            sb.open(PAGE).await?;
            sb.assert_title("Fixture").await?;
            failure()
        })
        .await;
    assert!(
        matches!(result, Err(SeleniumBaseError::AssertionFailed(_))),
        "{result:?}"
    );
    assert_eq!(
        names(&logs.path().join(id)),
        ["basic_test_info.txt", "page_source.html", "screenshot.png"]
    );
}

#[tokio::test]
#[ignore = "starts a real Chrome"]
async fn the_webdriver_case_reads_the_test_data_and_its_own_id() {
    let fixture = Sb::new(headless(|settings| {
        settings.test.data = TestData {
            env: Some("qa".to_owned()),
            data: Some("abc".to_owned()),
            var1: Some("x".to_owned()),
            variables: TestData::parse_variables("{\"k\": 1}").unwrap(),
            ..TestData::default()
        };
    }));
    let id = fixture.test_id().to_owned();
    fixture
        .run(async |sb| {
            assert_eq!(sb.env(), "qa");
            assert_eq!(sb.data(), Some("abc"));
            assert_eq!(sb.var1(), Some("x"));
            assert_eq!(sb.variables()["k"], 1);
            assert_eq!(sb.test_id(), id);
            Ok(())
        })
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
#[ignore = "starts a real Chrome"]
async fn a_cdp_test_that_fails_once_passes_on_a_rerun(sb_cdp: SbCdp) {
    let logs = tempfile::tempdir().unwrap();
    let sb_cdp = sb_cdp
        .configure(|settings| {
            settings.headless = true;
            settings.test.reruns = 1;
            settings.test.reruns_delay = 0.1;
        })
        .logs_dir(logs.path());
    let runs = AtomicU32::new(0);
    sb_cdp
        .run(async |page| {
            page.goto(PAGE).await?;
            if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                return failure();
            }
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}
