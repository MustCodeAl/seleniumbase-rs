//! The test hooks of the plugin API on `SbCdp::run` over a scripted browser, with no Chrome: what a
//! plugin hears when a test starts and ends, what it sees of the page, and how a verdict of
//! `TestVerdict::Fail` turns a passed test into a failed one.
//!
//! Each test reads back a log the plugins write and the result of `run`. Run them with
//! `cargo test --features "testing test-util plugins" --test sb_fixture_plugins`.
#![cfg(all(feature = "testing", feature = "test-util", feature = "plugins"))]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use base64::prelude::*;
use seleniumbase_rs::config::test_settings::TestSettings;
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, PageHandle, Plugin, PluginError, PluginSet,
    Result as PluginResult, TestInfo, TestOutcome, TestVerdict,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use seleniumbase_rs::testing::SbCdp;
use seleniumbase_rs::{Result, SeleniumBaseError, Settings};
use serde_json::json;

const PNG: &[u8] = b"\x89PNG-from-the-mock";

/// What the hooks of the plugins in one test saw, in order.
type Log = Arc<Mutex<Vec<String>>>;

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// What a [`Judge`] makes of a test that ended.
#[derive(Clone, Copy)]
enum Says {
    /// Leaves the result as it is.
    Keep,
    /// Fails the test with this message.
    Fail(&'static str),
    /// Fails the first attempt only, with this message.
    FailOnce(&'static str),
    /// Errors in its hook.
    Breaks,
}

/// A plugin with test hooks: it writes down what it hears and what the page looks like.
struct Judge {
    name: &'static str,
    says: Says,
    log: Log,
    ended: AtomicU32,
}

impl Judge {
    fn new(name: &'static str, says: Says, log: &Log) -> Self {
        Self {
            name,
            says,
            log: log.clone(),
            ended: AtomicU32::new(0),
        }
    }

    fn note(&self, entry: String) {
        self.log.lock().unwrap().push(entry);
    }
}

#[async_trait]
impl Plugin for Judge {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::TestHooks)
    }

    async fn on_test_start(&self, test: &TestInfo) -> PluginResult<()> {
        assert!(!test.name.is_empty(), "the test has a name");
        self.note(format!("{} start", self.name));
        Ok(())
    }

    async fn on_test_end(
        &self,
        _test: &TestInfo,
        outcome: &TestOutcome,
        page: Option<&dyn PageHandle>,
    ) -> PluginResult<TestVerdict> {
        // The page is still open: the plugin can ask it where it is.
        let at = match page {
            Some(page) => page
                .url()
                .await
                .unwrap_or_else(|error| format!("no answer: {error}")),
            None => "no page".to_owned(),
        };
        self.note(format!("{} end {outcome:?} at {at}", self.name));
        let attempt = self.ended.fetch_add(1, Ordering::SeqCst);
        match self.says {
            Says::Keep => Ok(TestVerdict::Keep),
            Says::Fail(message) => Ok(TestVerdict::Fail(message.to_owned())),
            Says::FailOnce(message) if attempt == 0 => Ok(TestVerdict::Fail(message.to_owned())),
            Says::FailOnce(_) => Ok(TestVerdict::Keep),
            Says::Breaks => Err(PluginError::other("the judge broke")),
        }
    }
}

/// Mocked browsers launched with the plugins, one per attempt.
struct Browsers {
    plugins: PluginSet,
    launches: AtomicU32,
}

impl Browsers {
    fn new(judges: Vec<Judge>) -> Self {
        let plugins = judges.into_iter().fold(PluginSet::new(), |set, judge| {
            set.with(judge).expect("the plugin joins the set")
        });
        Self {
            plugins,
            launches: AtomicU32::new(0),
        }
    }

    fn launches(&self) -> u32 {
        self.launches.load(Ordering::SeqCst)
    }

    async fn launch(&self) -> Result<Browser> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        let mock = MockCtrl::new();
        let options = LaunchOptions::builder()
            .headless(true)
            .plugins(self.plugins.clone())
            .build()?;
        let browser = Browser::launch_mocked(options, &mock).await?;
        mock.reply(
            "Runtime.evaluate",
            json!({ "result": { "value": "https://shop.test/cart" } }),
        );
        mock.reply(
            "Page.captureScreenshot",
            json!({ "data": BASE64_STANDARD.encode(PNG) }),
        );
        Ok(browser)
    }
}

fn fixture(logs: &Path, change: impl FnOnce(&mut TestSettings)) -> SbCdp {
    let mut test = TestSettings::default();
    change(&mut test);
    SbCdp::new(Settings {
        test,
        ..Settings::default()
    })
    .logs_dir(logs)
    .without_report()
}

fn names(folder: &Path) -> Vec<String> {
    std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn plugins_hear_the_test_start_and_end_and_see_the_open_page() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![Judge::new("judge", Says::Keep, &log)]);

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        entries(&log),
        ["judge start", "judge end Passed at https://shop.test/cart"]
    );
    assert!(names(logs.path()).is_empty(), "a pass leaves nothing");
}

#[tokio::test]
async fn a_plugin_that_fails_a_passed_test_fails_it_with_its_message_and_leaves_evidence() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![
        Judge::new("links", Says::Fail("2 broken links"), &log),
        Judge::new("speed", Says::Fail("the page took 9 s"), &log),
    ]);

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;

    let error = result.unwrap_err();
    let text = error.to_string();
    assert!(
        matches!(error, SeleniumBaseError::AssertionFailed(_)),
        "{error:?}"
    );
    assert!(text.contains("a plugin failed the test"), "{text}");
    assert!(
        text.contains("2 broken links") && text.contains("the page took 9 s"),
        "both plugins are heard: {text}"
    );
    assert!(text.contains("; "), "the messages are joined: {text}");
    assert!(
        !names(logs.path()).is_empty(),
        "a failed test leaves its evidence"
    );
}

#[tokio::test]
async fn a_test_that_failed_keeps_its_own_error_and_the_plugins_still_hear_it() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![Judge::new("judge", Says::Fail("also bad"), &log)]);

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| {
                Err(SeleniumBaseError::AssertionFailed(
                    "the cart is empty".into(),
                ))
            },
        )
        .await;

    let text = result.unwrap_err().to_string();
    assert!(text.contains("the cart is empty"), "{text}");
    assert!(!text.contains("a plugin failed"), "{text}");
    let heard = entries(&log);
    assert_eq!(heard[0], "judge start");
    assert!(
        heard[1].starts_with("judge end Failed(") && heard[1].contains("the cart is empty"),
        "{heard:?}"
    );
}

#[tokio::test]
async fn a_skipped_test_is_reported_to_the_plugins_as_skipped_and_stays_a_skip() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![Judge::new("judge", Says::Fail("no"), &log)]);

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(
            async || browsers.launch().await,
            async |_page| Err(SeleniumBaseError::Skipped("not today".into())),
        )
        .await;

    assert!(result.is_ok(), "a skip is not a failure: {result:?}");
    assert!(
        entries(&log)[1].starts_with("judge end Skipped"),
        "{:?}",
        entries(&log)
    );
}

#[tokio::test]
async fn a_plugin_whose_hook_fails_does_not_change_the_result() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![Judge::new("judge", Says::Breaks, &log)]);

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(entries(&log).len(), 2, "{:?}", entries(&log));
}

#[tokio::test]
async fn every_attempt_of_a_rerun_test_is_told_and_a_later_pass_wins() {
    let log = Log::default();
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(vec![Judge::new("judge", Says::FailOnce("flaky"), &log)]);

    let result = fixture(logs.path(), |test| test.reruns = 1)
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(browsers.launches(), 2, "the failed attempt was run again");
    assert_eq!(
        entries(&log)
            .iter()
            .filter(|entry| entry.ends_with("start"))
            .count(),
        2
    );
    assert_eq!(
        entries(&log)
            .iter()
            .filter(|entry| entry.contains(" end Passed"))
            .count(),
        2
    );
}

#[tokio::test]
async fn a_test_without_plugins_runs_as_before() {
    let logs = tempfile::tempdir().unwrap();
    let browsers = Browsers::new(Vec::new());

    let result = fixture(logs.path(), |_| {})
        .run_with_browser(async || browsers.launch().await, async |_page| Ok(()))
        .await;

    assert!(result.is_ok(), "{result:?}");
}
