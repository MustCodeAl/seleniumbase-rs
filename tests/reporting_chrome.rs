//! The run reporter against a real Chrome: a failing test on a `data:` page
//! leaves `latest_logs/<test>/` with a real screenshot, the page source and the
//! info file, and the run records it as failed.
//!
//! The test needs a Chrome or Chromium on the machine, so it is ignored by
//! default. Run it with:
//!
//! ```text
//! cargo test --test reporting_chrome -- --ignored
//! ```

use std::fs;
use std::sync::Arc;

use seleniumbase_rs::core::report_helper::TestStatus;
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::plugins::observer::Plugins;
use seleniumbase_rs::plugins::reports::RunReporterPlugin;
use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};
use seleniumbase_rs::SeleniumBaseError;

const PAGE: &str = "data:text/html,<html><head><title>Shop</title></head><body>hello</body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_failing_test_leaves_a_real_screenshot_the_source_and_the_info() {
    let root = tempfile::tempdir().unwrap();
    let run = TestRun::open(
        root.path(),
        ReportSettings::default(),
        RunKey::new("chrome"),
    )
    .unwrap();
    let mut plugins = Plugins::new().with(RunReporterPlugin::with_run(Arc::clone(&run)));
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .unwrap();

    let outcome = plugins
        .run_cdp_test("shop::checkout", options, async |page: &Page| {
            page.goto(PAGE).await?;
            Err(SeleniumBaseError::AssertionFailed(
                "expected a cart".to_owned(),
            ))
        })
        .await;

    assert!(outcome.is_err());
    let dir = root.path().join("latest_logs/shop.checkout");
    let png = fs::read(dir.join("screenshot.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "a real PNG");
    let source = fs::read_to_string(dir.join("page_source.html")).unwrap();
    assert!(source.contains("<base href="), "{source}");
    assert!(source.contains("hello"), "{source}");
    let info = fs::read_to_string(dir.join("basic_test_info.txt")).unwrap();
    assert!(info.starts_with("shop::checkout\r\n"), "{info}");
    assert!(info.contains("  Browser: Chrome"), "{info}");
    assert!(info.contains("   Driver: Pure CDP (no driver)"), "{info}");
    assert!(
        info.contains("Exception: assertion failed: expected a cart"),
        "{info}"
    );

    let result = run.snapshot().unwrap().results.remove(0);
    assert_eq!(result.status, TestStatus::Failed);
    assert_eq!(result.log_dir.as_deref(), Some("latest_logs/shop.checkout"));
}
