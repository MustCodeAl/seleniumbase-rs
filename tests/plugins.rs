//! Test plugins: what they do when a test fails and when it finishes.
//!
//! None of these tests need a browser. A fake page stands in for one, and the
//! Pure CDP mock stands in for the real `Page`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use seleniumbase_rs::plugins::observer::{PageEvidence, Plugins, TestPlugin};
use seleniumbase_rs::plugins::page_source::PageSourceOnFailurePlugin;
use seleniumbase_rs::plugins::reports::ReportPlugin;
use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
use seleniumbase_rs::SeleniumBaseError;

/// A page that returns canned evidence, or fails to.
struct FakePage {
    png: Option<Vec<u8>>,
    html: Option<String>,
}

impl FakePage {
    fn working() -> Self {
        Self {
            png: Some(b"\x89PNG-bytes".to_vec()),
            html: Some("<html><body>broken page</body></html>".to_owned()),
        }
    }

    fn gone() -> Self {
        Self {
            png: None,
            html: None,
        }
    }
}

#[async_trait]
impl PageEvidence for FakePage {
    async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.png
            .clone()
            .ok_or_else(|| SeleniumBaseError::screenshot("the browser has gone away"))
    }

    async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        self.html
            .clone()
            .ok_or_else(|| SeleniumBaseError::cdp_driver("the browser has gone away"))
    }
}

fn boom() -> SeleniumBaseError {
    SeleniumBaseError::AssertionFailed("expected 'a' but got 'b'".to_owned())
}

fn files_in(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|entries| entries.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    files.sort();
    files
}

/// Logs every hook it receives into a list that the test can read.
struct Recorder {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl TestPlugin for Recorder {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn test_started(&mut self, test: &str) -> Result<(), SeleniumBaseError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:started:{test}", self.name));
        Ok(())
    }

    async fn test_failed(
        &mut self,
        test: &str,
        error: &SeleniumBaseError,
        page: &dyn PageEvidence,
    ) -> Result<(), SeleniumBaseError> {
        let png = page.screenshot_png().await.map(|b| b.len()).unwrap_or(0);
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:failed:{test}:{error}:png={png}", self.name));
        Ok(())
    }

    async fn test_finished(
        &mut self,
        test: &str,
        _elapsed: Duration,
        outcome: &Result<(), SeleniumBaseError>,
    ) -> Result<(), SeleniumBaseError> {
        self.log.lock().unwrap().push(format!(
            "{}:finished:{test}:ok={}",
            self.name,
            outcome.is_ok()
        ));
        Ok(())
    }
}

/// A plugin whose every hook fails.
struct Broken;

#[async_trait]
impl TestPlugin for Broken {
    async fn test_started(&mut self, _: &str) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::Unsupported("broken at start".to_owned()))
    }

    async fn test_failed(
        &mut self,
        _: &str,
        _: &SeleniumBaseError,
        _: &dyn PageEvidence,
    ) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::Unsupported(
            "broken on failure".to_owned(),
        ))
    }

    async fn test_finished(
        &mut self,
        _: &str,
        _: Duration,
        _: &Result<(), SeleniumBaseError>,
    ) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::Unsupported(
            "broken at finish".to_owned(),
        ))
    }
}

// ----------------------------------------------------------------------
// Failure evidence
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_failing_test_gets_a_real_screenshot_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = Plugins::new().with(ScreenshotOnFailurePlugin::new(dir.path()));

    plugins
        .failed("login_works", &boom(), &FakePage::working())
        .await;

    let files = files_in(dir.path());
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("login_works_") && name.ends_with(".png"),
        "{name}"
    );
    assert_eq!(std::fs::read(&files[0]).unwrap(), b"\x89PNG-bytes");
}

#[tokio::test]
async fn a_failing_test_gets_its_page_source_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = Plugins::new().with(PageSourceOnFailurePlugin::new(dir.path()));

    plugins
        .failed("checkout", &boom(), &FakePage::working())
        .await;

    let files = files_in(dir.path());
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].extension().is_some_and(|e| e == "html"));
    assert_eq!(
        std::fs::read_to_string(&files[0]).unwrap(),
        "<html><body>broken page</body></html>"
    );
}

#[tokio::test]
async fn no_file_is_written_when_the_browser_cannot_give_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = Plugins::new()
        .with(ScreenshotOnFailurePlugin::new(dir.path()))
        .with(PageSourceOnFailurePlugin::new(dir.path()));

    plugins.failed("t", &boom(), &FakePage::gone()).await;

    assert!(files_in(dir.path()).is_empty(), "no placeholder files");
}

#[tokio::test]
async fn two_failures_of_the_same_test_keep_both_screenshots() {
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = Plugins::new().with(ScreenshotOnFailurePlugin::new(dir.path()));

    for _ in 0..3 {
        plugins.failed("flaky", &boom(), &FakePage::working()).await;
    }

    assert_eq!(files_in(dir.path()).len(), 3);
}

#[tokio::test]
async fn a_hostile_test_name_cannot_write_outside_the_directory() {
    let root = tempfile::tempdir().unwrap();
    let out = root.path().join("out");
    let mut plugins = Plugins::new().with(ScreenshotOnFailurePlugin::new(&out));

    plugins
        .failed("../../escaped/evil", &boom(), &FakePage::working())
        .await;

    let files = files_in(&out);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].parent().unwrap(), out);
    assert!(
        files_in(root.path()).iter().all(|p| p == &out),
        "nothing beside `out`"
    );
}

#[cfg(feature = "test-util")]
#[tokio::test]
async fn evidence_comes_through_the_pure_cdp_page_too() {
    use base64::Engine;
    use seleniumbase_rs::sb_cdp::Browser;
    use serde_json::json;

    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    let png = b"\x89PNG\r\n real screenshot bytes";
    mock.reply(
        "Page.captureScreenshot",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(png) }),
    );
    mock.reply(
        "Runtime.evaluate",
        json!({ "result": { "value": "<html><body>cdp page</body></html>" } }),
    );
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = Plugins::new()
        .with(ScreenshotOnFailurePlugin::new(dir.path()))
        .with(PageSourceOnFailurePlugin::new(dir.path()));

    plugins.failed("cdp_test", &boom(), &page).await;

    let files = files_in(dir.path());
    let png_file = files
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "png"))
        .unwrap();
    let html_file = files
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "html"))
        .unwrap();
    assert_eq!(std::fs::read(png_file).unwrap(), png);
    assert_eq!(
        std::fs::read_to_string(html_file).unwrap(),
        "<html><body>cdp page</body></html>"
    );
}

// ----------------------------------------------------------------------
// Dispatch
// ----------------------------------------------------------------------

#[tokio::test]
async fn plugins_hear_about_a_test_in_order_and_in_the_order_they_were_added() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut plugins = Plugins::new()
        .with(Recorder {
            name: "first",
            log: log.clone(),
        })
        .with(Recorder {
            name: "second",
            log: log.clone(),
        });
    let outcome = Err(boom());

    plugins.started("t").await;
    plugins.failed("t", &boom(), &FakePage::working()).await;
    plugins
        .finished("t", Duration::from_millis(5), &outcome)
        .await;

    assert_eq!(
        *log.lock().unwrap(),
        [
            "first:started:t",
            "second:started:t",
            "first:failed:t:assertion failed: expected 'a' but got 'b':png=10",
            "second:failed:t:assertion failed: expected 'a' but got 'b':png=10",
            "first:finished:t:ok=false",
            "second:finished:t:ok=false",
        ]
    );
}

#[tokio::test]
async fn a_plugin_that_fails_does_not_stop_the_others() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut plugins = Plugins::new().with(Broken).with(Recorder {
        name: "after",
        log: log.clone(),
    });

    plugins.started("t").await;
    plugins.failed("t", &boom(), &FakePage::working()).await;
    plugins.finished("t", Duration::ZERO, &Ok(())).await;

    assert_eq!(
        log.lock().unwrap().len(),
        3,
        "the plugin after the broken one still ran"
    );
}

#[tokio::test]
async fn a_plugin_set_describes_itself() {
    let plugins = Plugins::new()
        .with(ScreenshotOnFailurePlugin::new("x"))
        .with(ReportPlugin::json("r.json"));

    assert_eq!(plugins.len(), 2);
    assert!(!plugins.is_empty());
    assert!(Plugins::new().is_empty());
    let shown = format!("{plugins:?}");
    assert!(
        shown.contains("ScreenshotOnFailurePlugin") && shown.contains("ReportPlugin"),
        "{shown}"
    );
}

// ----------------------------------------------------------------------
// Reports
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_json_report_is_complete_after_every_test() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out").join("report.json");
    let mut plugins = Plugins::new().with(ReportPlugin::json(&path));

    plugins
        .finished("login", Duration::from_millis(1500), &Ok(()))
        .await;
    let after_one: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    plugins
        .finished("checkout", Duration::from_millis(250), &Err(boom()))
        .await;
    let after_two: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    assert_eq!(
        after_one.as_array().unwrap().len(),
        1,
        "valid after the first test"
    );
    let rows = after_two.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0]["name"].clone(), rows[0]["passed"].clone()),
        ("login".into(), true.into())
    );
    assert!((rows[0]["duration_secs"].as_f64().unwrap() - 1.5).abs() < 1e-9);
    assert_eq!(rows[1]["passed"], false);
    assert!(rows[1]["message"]
        .as_str()
        .unwrap()
        .contains("expected 'a'"));
}

#[tokio::test]
async fn the_html_report_escapes_what_a_test_printed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.html");
    let mut plugins = Plugins::new().with(ReportPlugin::html(&path));
    let hostile = Err(SeleniumBaseError::AssertionFailed(
        "<script>alert(1)</script>".to_owned(),
    ));

    plugins
        .finished("<b>name</b>", Duration::ZERO, &hostile)
        .await;

    let html = std::fs::read_to_string(&path).unwrap();
    assert!(!html.contains("<script>alert(1)</script>"), "{html}");
    assert!(
        html.contains("&lt;script&gt;") && html.contains("&lt;b&gt;name"),
        "{html}"
    );
}

#[tokio::test]
async fn an_unwritable_report_is_logged_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    // The report's "directory" is a file, so it can never be written.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let mut plugins = Plugins::new().with(ReportPlugin::json(blocker.join("report.json")));

    // Returns normally: the test result is not the plugin's to change.
    plugins.finished("t", Duration::ZERO, &Ok(())).await;
}

#[cfg(feature = "turso")]
#[tokio::test]
async fn the_result_store_plugin_records_each_test() {
    use seleniumbase_rs::plugins::reports::ResultStorePlugin;
    use seleniumbase_rs::storage::{ResultStore, RunInfo};

    let store = ResultStore::in_memory().await.unwrap();
    let run = store.start_run(&RunInfo::new("ci")).await.unwrap();
    let mut plugins = Plugins::new().with(ResultStorePlugin::new(store.clone(), run));

    plugins
        .finished("passes", Duration::from_millis(10), &Ok(()))
        .await;
    plugins
        .finished("fails", Duration::from_millis(20), &Err(boom()))
        .await;

    let summary = store.run(run).await.unwrap().unwrap();
    assert_eq!((summary.passed, summary.failed), (1, 1));
    let results = store.results(run).await.unwrap();
    assert_eq!(results[1].name, "fails");
    assert!(results[1].message.contains("expected 'a'"));
}
