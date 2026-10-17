//! The plugin hooks of the Pure CDP engine (ADR 004, phase 2), against a
//! scripted browser, with no Chrome.
//!
//! A plugin's hooks run at launch (`before_launch`, `on_browser`), for every
//! tab the library attaches to (its packages, then `on_page_created`) and at
//! close (`on_close`). These tests read back the protocol commands the mock
//! received, so they check what reaches the browser and in which order. Run
//! them with `cargo test --features "test-util plugins" --test sb_cdp_plugins`.

#![cfg(all(feature = "plugins", feature = "test-util"))]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::plugin_api::{
    async_trait, BrowserFamily, BrowserHandle, Engine, LaunchSettings, Package, PageHandle, Plugin,
    PluginError, PluginSet, Requirements, Result as PluginResult,
};
use seleniumbase_rs::sb_cdp::{Browser, Call, LaunchOptions, MockCtrl};
use seleniumbase_rs::{Fingerprint, OsType, SeleniumBaseError};
use serde_json::json;

/// What the hooks of the plugins in one test saw, in order.
type Log = Arc<Mutex<Vec<String>>>;

fn note(log: &Log, entry: impl Into<String>) {
    log.lock().unwrap().push(entry.into());
}

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// Launch options with these plugins, headless so a `HEADFUL` check has
/// something to refuse.
fn options_with(plugins: PluginSet) -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .plugins(plugins)
        .build()
        .unwrap()
}

/// Runs the launch steps against `mock` with these plugins.
async fn launch(plugins: PluginSet, mock: &MockCtrl) -> Result<Browser, SeleniumBaseError> {
    Browser::launch_mocked(options_with(plugins), mock).await
}

/// The position of the first call after `from` that `matches`.
fn position(calls: &[Call], from: usize, matches: impl Fn(&Call) -> bool) -> Option<usize> {
    calls
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, call)| matches(call))
        .map(|(index, _)| index)
}

fn adds_script(call: &Call, needle: &str) -> bool {
    call.method == "Page.addScriptToEvaluateOnNewDocument"
        && call.params["source"]
            .as_str()
            .is_some_and(|source| source.contains(needle))
}

fn evaluates(call: &Call, needle: &str) -> bool {
    call.method == "Runtime.evaluate"
        && call.params["expression"]
            .as_str()
            .is_some_and(|expression| expression.contains(needle))
}

/// Loads a package into every tab, then sends a command and adds a script of
/// its own from `on_page_created`.
struct Marker;

#[async_trait]
impl Plugin for Marker {
    fn name(&self) -> &str {
        "marker"
    }

    fn packages(&self) -> Vec<Package> {
        vec![Package::script("window.markedByPackage = 1;").everywhere()]
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        assert_eq!(page.engine(), Engine::Cdp);
        page.send(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Europe/Berlin" }),
        )
        .await?;
        page.add_init_script("window.markedByHook = 2;").await
    }
}

#[tokio::test]
async fn a_plugins_packages_then_its_hook_reach_every_new_tab_in_order() {
    let mock = MockCtrl::new();
    let browser = launch(PluginSet::new().with(Marker).unwrap(), &mock)
        .await
        .unwrap();

    let calls = mock.calls();
    let helpers = position(&calls, 0, |call| adds_script(call, "__sbcdp")).expect("the helpers");
    let package = position(&calls, helpers, |call| adds_script(call, "markedByPackage"))
        .expect("the package is added after the library's own helpers");
    let package_now = position(&calls, package, |call| evaluates(call, "markedByPackage"))
        .expect("and run in the current document");
    let hook_command = position(&calls, package_now, |call| {
        call.method == "Emulation.setTimezoneOverride"
    })
    .expect("then the hook's own command");
    let hook_script = position(&calls, hook_command, |call| {
        adds_script(call, "markedByHook")
    })
    .expect("then the hook's script");
    position(&calls, hook_script, |call| evaluates(call, "markedByHook"))
        .expect("which also runs in the current document");
    for index in [package, package_now, hook_command, hook_script] {
        assert_eq!(
            calls[index].session_id.as_deref(),
            Some("S1"),
            "{:?} goes to the tab",
            calls[index]
        );
    }

    browser.new_page(None::<&str>).await.unwrap();
    let packages = mock
        .calls()
        .iter()
        .filter(|call| adds_script(call, "markedByPackage"))
        .count();
    assert_eq!(packages, 2, "the first tab and the new one");
}

/// Fails in `on_page_created` from the `fail_from`-th tab on.
struct FailsOnTab {
    seen: AtomicUsize,
    fail_from: usize,
}

#[async_trait]
impl Plugin for FailsOnTab {
    fn name(&self) -> &str {
        "fails-on-tab"
    }

    async fn on_page_created(&self, _page: &dyn PageHandle) -> PluginResult<()> {
        let tab = self.seen.fetch_add(1, Ordering::SeqCst) + 1;
        if tab >= self.fail_from {
            Err(PluginError::other("no stealth for this tab"))
        } else {
            Ok(())
        }
    }
}

fn fails_from(fail_from: usize) -> PluginSet {
    PluginSet::new()
        .with(FailsOnTab {
            seen: AtomicUsize::new(0),
            fail_from,
        })
        .unwrap()
}

#[tokio::test]
async fn a_failing_on_page_created_fails_the_new_tab_and_closes_it() {
    let mock = MockCtrl::new();
    let browser = launch(fails_from(2), &mock).await.unwrap();

    let error = browser.new_page(None::<&str>).await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "fails-on-tab"),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("on_page_created") && message.contains("no stealth for this tab"),
        "{message}"
    );
    assert!(error.hint().unwrap().contains("fails-on-tab"));
    let closed = mock.calls_to("Target.closeTarget");
    assert_eq!(closed.len(), 1, "the unprepared tab is closed");
    assert_eq!(closed[0].params["targetId"], "T2");
}

#[tokio::test]
async fn a_plugin_that_fails_on_the_first_tab_fails_the_launch() {
    let mock = MockCtrl::new();

    let error = launch(fails_from(1), &mock).await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::Plugin { .. }),
        "{error:?}"
    );
    assert!(
        mock.calls_to("Target.closeTarget").is_empty(),
        "a tab the library did not open is left alone"
    );
}

/// Records each hook, and asks the browser for its version from `on_browser`.
struct Lifecycle {
    log: Log,
}

#[async_trait]
impl Plugin for Lifecycle {
    fn name(&self) -> &str {
        "lifecycle"
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        assert_eq!(browser.engine(), Engine::Cdp);
        assert_eq!(browser.family(), BrowserFamily::Chromium);
        let version = browser.send("Browser.getVersion", json!({})).await?;
        note(
            &self.log,
            format!("on_browser {}", version["product"].as_str().unwrap_or("?")),
        );
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        let value = page.evaluate("navigator.webdriver").await?;
        note(&self.log, format!("on_page_created {value}"));
        Ok(())
    }

    async fn on_close(&self) -> PluginResult<()> {
        note(&self.log, "on_close");
        Err(PluginError::other("on_close errors are only logged"))
    }
}

#[tokio::test]
async fn on_browser_runs_before_the_first_tab_and_talks_to_the_browser() {
    let mock = MockCtrl::new();
    mock.reply("Browser.getVersion", json!({ "product": "Chrome/155.0" }));
    mock.on("Runtime.evaluate", |params| {
        Ok(if params["expression"] == "navigator.webdriver" {
            assert_eq!(params["returnByValue"], true);
            json!({ "result": { "value": false } })
        } else {
            json!({ "result": {} })
        })
    });
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Lifecycle { log: log.clone() })
        .unwrap();

    let browser = launch(plugins, &mock).await.unwrap();

    assert_eq!(
        entries(&log),
        ["on_browser Chrome/155.0", "on_page_created false"]
    );
    let version = mock.calls_to("Browser.getVersion");
    assert_eq!(version.len(), 1);
    assert_eq!(version[0].session_id, None, "a browser-level command");
    let calls = mock.calls();
    let asked = position(&calls, 0, |call| call.method == "Browser.getVersion").unwrap();
    let first_tab = position(&calls, 0, |call| call.method == "Page.enable").unwrap();
    assert!(
        asked < first_tab,
        "on_browser runs before the first tab is prepared"
    );
    drop(browser);
}

#[tokio::test]
async fn a_script_error_in_a_plugins_evaluate_fails_the_tab() {
    let mock = MockCtrl::new();
    mock.on("Runtime.evaluate", |params| {
        Ok(if params["expression"] == "navigator.webdriver" {
            json!({ "exceptionDetails": { "exception": { "description": "ReferenceError: boom" } } })
        } else {
            json!({ "result": {} })
        })
    });
    let plugins = PluginSet::new()
        .with(Lifecycle {
            log: Log::default(),
        })
        .unwrap();

    let error = launch(plugins, &mock).await.unwrap_err();

    assert!(
        error.to_string().contains("ReferenceError: boom"),
        "{error}"
    );
}

#[tokio::test]
async fn on_close_runs_once_and_its_errors_do_not_fail_close() {
    let mock = MockCtrl::new();
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Lifecycle { log: log.clone() })
        .unwrap();
    let browser = launch(plugins, &mock).await.unwrap();

    browser.close().await.unwrap();
    browser.close().await.unwrap();
    drop(browser);
    tokio::task::yield_now().await;

    let closes = entries(&log)
        .iter()
        .filter(|entry| *entry == "on_close")
        .count();
    assert_eq!(closes, 1);
}

/// Counts `on_close` calls.
struct CountsCloses(Arc<AtomicUsize>);

#[async_trait]
impl Plugin for CountsCloses {
    fn name(&self) -> &str {
        "counts-closes"
    }

    async fn on_close(&self) -> PluginResult<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn dropping_the_last_handle_runs_on_close() {
    let mock = MockCtrl::new();
    let closes = Arc::new(AtomicUsize::new(0));
    let plugins = PluginSet::new()
        .with(CountsCloses(Arc::clone(&closes)))
        .unwrap();
    let browser = launch(plugins, &mock).await.unwrap();
    let page = browser.default_page().await.unwrap();

    drop(browser);
    tokio::task::yield_now().await;
    assert_eq!(
        closes.load(Ordering::SeqCst),
        0,
        "a page still holds the browser"
    );

    drop(page);
    for _ in 0..100 {
        if closes.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_failed_launch_tells_the_plugins_once_that_it_ended() {
    let mock = MockCtrl::new();
    let closes = Arc::new(AtomicUsize::new(0));
    let plugins = PluginSet::new()
        .with(CountsCloses(Arc::clone(&closes)))
        .unwrap()
        .with(FailsOnTab {
            seen: AtomicUsize::new(0),
            fail_from: 1,
        })
        .unwrap();

    let error = launch(plugins, &mock).await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::Plugin { .. }),
        "{error:?}"
    );
    assert_eq!(
        closes.load(Ordering::SeqCst),
        1,
        "on_close has run by the time the launch returns"
    );
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(closes.load(Ordering::SeqCst), 1, "and only once");
}

/// Loads an extension and adds a switch in `before_launch`.
struct AddsExtension;

#[async_trait]
impl Plugin for AddsExtension {
    fn name(&self) -> &str {
        "adds-extension"
    }

    async fn before_launch(&self, launch: &mut LaunchSettings) -> PluginResult<()> {
        assert!(launch.headless, "the plugin sees the launch's settings");
        launch
            .extensions
            .push(PathBuf::from("/opt/extensions/helper"));
        launch.args.push("--lang=de".to_owned());
        Ok(())
    }
}

#[tokio::test]
async fn an_extension_a_plugin_adds_in_before_launch_is_loaded_before_the_first_tab() {
    let mock = MockCtrl::new();

    launch(PluginSet::new().with(AddsExtension).unwrap(), &mock)
        .await
        .unwrap();

    let calls = mock.calls();
    let loaded = position(&calls, 0, |call| call.method == "Extensions.loadUnpacked")
        .expect("the extension is loaded over the protocol");
    assert_eq!(calls[loaded].params["path"], "/opt/extensions/helper");
    assert_eq!(calls[loaded].session_id, None);
    let first_tab = position(&calls, 0, |call| call.method == "Page.enable").unwrap();
    assert!(loaded < first_tab);
}

/// Needs a browser with a window.
struct NeedsWindow;

#[async_trait]
impl Plugin for NeedsWindow {
    fn name(&self) -> &str {
        "needs-window"
    }

    fn requirements(&self) -> Requirements {
        Requirements::HEADFUL
    }
}

#[tokio::test]
async fn a_requirement_the_launch_does_not_meet_stops_it_before_any_command() {
    let mock = MockCtrl::new();

    let error = launch(PluginSet::new().with(NeedsWindow).unwrap(), &mock)
        .await
        .unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "needs-window"),
        "{error:?}"
    );
    assert!(error.to_string().contains("headless"), "{error}");
    assert!(mock.calls().is_empty(), "{:?}", mock.calls());
    assert_eq!(error.category(), "plugin");
}

/// Needs the plugin named `missing`.
struct NeedsOther;

#[async_trait]
impl Plugin for NeedsOther {
    fn name(&self) -> &str {
        "needs-other"
    }

    fn dependencies(&self) -> &[&str] {
        &["missing"]
    }
}

#[tokio::test]
async fn a_missing_dependency_stops_the_launch_and_says_what_to_add() {
    let mock = MockCtrl::new();

    let error = launch(PluginSet::new().with(NeedsOther).unwrap(), &mock)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("`missing`"), "{error}");
    let hint = error.hint().unwrap();
    assert!(hint.contains("needs-other"), "{hint}");
}

#[tokio::test]
async fn without_plugins_a_launch_sends_no_plugin_commands() {
    let mock = MockCtrl::new();

    let browser = Browser::launch_mocked(LaunchOptions::default(), &mock)
        .await
        .unwrap();
    browser.close().await.unwrap();

    let methods: Vec<String> = mock.calls().into_iter().map(|call| call.method).collect();
    assert!(
        !methods
            .iter()
            .any(|method| method == "Extensions.loadUnpacked"),
        "{methods:?}"
    );
    assert_eq!(
        methods.first().map(String::as_str),
        Some("Target.getTargets"),
        "the first tab is the first thing a plain launch asks for: {methods:?}"
    );
}

#[tokio::test]
async fn a_page_is_a_page_handle_that_works_on_its_own_tab() {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    let handle: &dyn PageHandle = &page;
    assert_eq!(handle.engine(), Engine::Cdp);

    let sent = handle
        .send(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Asia/Tokyo" }),
        )
        .await
        .unwrap();
    assert_eq!(sent, json!({}));
    let timezone = mock.calls_to("Emulation.setTimezoneOverride");
    assert_eq!(timezone.len(), 1);
    assert_eq!(timezone[0].session_id.as_deref(), Some("S1"), "to the tab");

    handle
        .add_init_script("window.viaHandle = 1;")
        .await
        .unwrap();
    let calls = mock.calls();
    let added = position(&calls, 0, |call| {
        adds_script(call, "viaHandle") && call.session_id.as_deref() == Some("S1")
    })
    .expect("a script for the tab's next documents");
    position(&calls, added, |call| evaluates(call, "viaHandle"))
        .expect("and run in the current one, as the plugin API says");

    mock.reply("Runtime.evaluate", json!({ "result": { "value": 42 } }));
    assert_eq!(handle.evaluate("6 * 7").await.unwrap(), json!(42));
    let evaluation = mock.calls_to("Runtime.evaluate").pop().unwrap();
    assert_eq!(evaluation.params["expression"], "6 * 7");
    assert_eq!(evaluation.params["returnByValue"], true);
    assert_eq!(evaluation.params["awaitPromise"], true);

    mock.reply(
        "Runtime.evaluate",
        json!({ "exceptionDetails": { "exception": { "description": "TypeError: nope" } } }),
    );
    let error = handle.evaluate("nope()").await.unwrap_err();
    assert!(error.to_string().contains("TypeError: nope"), "{error}");
    mock.fail("Emulation.setTimezoneOverride", "not allowed");
    let error = handle
        .send("Emulation.setTimezoneOverride", json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, PluginError::Other(message) if message.contains("not allowed")),
        "{error:?}"
    );
}

/// A plugin whose state belongs to one browser: `per_launch` hands out a
/// numbered instance, and the template it is made from is number 0.
struct Numbered {
    number: usize,
    issued: Arc<AtomicUsize>,
    log: Log,
}

#[async_trait]
impl Plugin for Numbered {
    fn name(&self) -> &str {
        "numbered"
    }

    fn per_launch(&self) -> Option<Arc<dyn Plugin>> {
        let number = self.issued.fetch_add(1, Ordering::SeqCst) + 1;
        Some(Arc::new(Self {
            number,
            issued: Arc::clone(&self.issued),
            log: Arc::clone(&self.log),
        }))
    }

    async fn before_launch(&self, _launch: &mut LaunchSettings) -> PluginResult<()> {
        note(&self.log, format!("before_launch #{}", self.number));
        Ok(())
    }

    async fn on_page_created(&self, _page: &dyn PageHandle) -> PluginResult<()> {
        note(&self.log, format!("on_page_created #{}", self.number));
        Ok(())
    }

    async fn on_close(&self) -> PluginResult<()> {
        note(&self.log, format!("on_close #{}", self.number));
        Ok(())
    }
}

#[tokio::test]
async fn each_launch_of_a_shared_set_gets_its_own_instance_of_a_per_launch_plugin() {
    let log = Log::default();
    let shared = PluginSet::new()
        .with(Numbered {
            number: 0,
            issued: Arc::new(AtomicUsize::new(0)),
            log: log.clone(),
        })
        .unwrap();

    // Two browsers from one set, as a pool launches them.
    let first = launch(shared.clone(), &MockCtrl::new()).await.unwrap();
    let second = launch(shared.clone(), &MockCtrl::new()).await.unwrap();
    first.close().await.unwrap();
    second.close().await.unwrap();

    assert_eq!(
        entries(&log),
        [
            "before_launch #1",
            "on_page_created #1",
            "before_launch #2",
            "on_page_created #2",
            "on_close #1",
            "on_close #2",
        ],
        "the shared template (#0) never runs, and each browser closes its own"
    );
}

/// Blocks `urls` on every tab with a raw `Network.setBlockedURLs`, as the
/// `block-resources` and `shield` plugins do; `per_tab` adds one URL that names
/// the tab (`*://tab<N>.example/*`).
struct Blocks {
    name: &'static str,
    urls: Vec<&'static str>,
    per_tab: Option<AtomicUsize>,
}

impl Blocks {
    fn of(name: &'static str, urls: &[&'static str]) -> Self {
        Self {
            name,
            urls: urls.to_vec(),
            per_tab: None,
        }
    }
}

#[async_trait]
impl Plugin for Blocks {
    fn name(&self) -> &str {
        self.name
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        let mut urls: Vec<String> = self.urls.iter().map(|url| (*url).to_owned()).collect();
        if let Some(tabs) = &self.per_tab {
            let tab = tabs.fetch_add(1, Ordering::SeqCst) + 1;
            urls.push(format!("*://tab{tab}.example/*"));
        }
        page.send("Network.enable", json!({})).await?;
        page.send("Network.setBlockedURLs", json!({ "urls": urls }))
            .await?;
        Ok(())
    }
}

/// The URLs of the last `Network.setBlockedURLs` the tab `session` received.
fn blocked_on(mock: &MockCtrl, session: &str) -> Vec<String> {
    let call = mock
        .calls_to("Network.setBlockedURLs")
        .into_iter()
        .rfind(|call| call.session_id.as_deref() == Some(session))
        .unwrap_or_else(|| panic!("no blocklist was sent to {session}"));
    serde_json::from_value(call.params["urls"].clone()).expect("a list of patterns")
}

#[tokio::test]
async fn raw_blocklists_of_several_plugins_are_merged_into_one_per_tab() {
    let mock = MockCtrl::new();
    let plugins = PluginSet::new()
        .with(Blocks::of("images", &["*://a.example/*", "*.png"]))
        .unwrap()
        .with(Blocks::of("hosts", &["*://b.example/*", "*.png"]))
        .unwrap();

    let _browser = launch(plugins, &mock).await.unwrap();

    assert_eq!(
        blocked_on(&mock, "S1"),
        ["*://a.example/*", "*.png", "*://b.example/*"],
        "the second plugin's list does not replace the first's"
    );
}

#[tokio::test]
async fn each_tab_keeps_its_own_blocklist() {
    let mock = MockCtrl::new();
    let tabs = Blocks {
        per_tab: Some(AtomicUsize::new(0)),
        ..Blocks::of("tabs", &["*.png"])
    };
    let plugins = PluginSet::new()
        .with(tabs)
        .unwrap()
        .with(Blocks::of("hosts", &["*://b.example/*"]))
        .unwrap();
    let browser = launch(plugins, &mock).await.unwrap();
    assert_eq!(
        blocked_on(&mock, "S1"),
        ["*.png", "*://tab1.example/*", "*://b.example/*"]
    );

    mock.reply("Target.attachToTarget", json!({ "sessionId": "S2" }));
    browser.new_page(None::<&str>).await.unwrap();

    assert_eq!(
        blocked_on(&mock, "S2"),
        ["*.png", "*://tab2.example/*", "*://b.example/*"],
        "the first tab's pattern is not carried over"
    );
    assert_eq!(
        blocked_on(&mock, "S1"),
        ["*.png", "*://tab1.example/*", "*://b.example/*"],
        "and the first tab's list is as it was"
    );
}

#[tokio::test]
async fn the_ad_block_list_of_the_launch_stays_when_a_plugin_blocks_urls() {
    let mock = MockCtrl::new();
    let options = LaunchOptions::builder()
        .headless(true)
        .ad_block(true)
        .plugins(
            PluginSet::new()
                .with(Blocks::of("hosts", &["*://b.example/*"]))
                .unwrap(),
        )
        .build()
        .unwrap();

    let _browser = Browser::launch_mocked(options, &mock).await.unwrap();

    let blocked = blocked_on(&mock, "S1");
    assert!(
        blocked.contains(&"*://*.doubleclick.net/*".to_owned()),
        "{blocked:?}"
    );
    assert_eq!(blocked.last().map(String::as_str), Some("*://b.example/*"));
}

#[tokio::test]
async fn the_tracker_hosts_of_a_fingerprint_stay_when_a_plugin_blocks_urls() {
    let mut fingerprint = Fingerprint::randomized(OsType::Linux, 7);
    fingerprint.flags.block_trackers = true;
    let tracker = format!("*{}*", fingerprint.tracker_hosts()[0]);

    for ad_block in [false, true] {
        let mock = MockCtrl::new();
        let options = LaunchOptions::builder()
            .headless(true)
            .ad_block(ad_block)
            .fingerprint(&fingerprint)
            .plugins(
                PluginSet::new()
                    .with(Blocks::of("hosts", &["*://b.example/*"]))
                    .unwrap(),
            )
            .build()
            .unwrap();

        let _browser = Browser::launch_mocked(options, &mock).await.unwrap();

        let blocked = blocked_on(&mock, "S1");
        assert!(
            blocked.contains(&tracker),
            "ad_block {ad_block}: the fingerprint's list survives the plugin's: {blocked:?}"
        );
        assert_eq!(blocked.last().map(String::as_str), Some("*://b.example/*"));
        assert_eq!(
            blocked.contains(&"*://*.doubleclick.net/*".to_owned()),
            ad_block,
            "{blocked:?}"
        );
    }
}

#[tokio::test]
async fn ad_block_does_not_replace_the_trackers_of_a_fingerprint() {
    let mut fingerprint = Fingerprint::randomized(OsType::Linux, 7);
    fingerprint.flags.block_trackers = true;
    let tracker = format!("*{}*", fingerprint.tracker_hosts()[0]);
    let mock = MockCtrl::new();
    let options = LaunchOptions::builder()
        .headless(true)
        .ad_block(true)
        .fingerprint(&fingerprint)
        .build()
        .unwrap();

    let _browser = Browser::launch_mocked(options, &mock).await.unwrap();

    let blocked = blocked_on(&mock, "S1");
    assert!(blocked.contains(&tracker), "{blocked:?}");
    assert!(
        blocked.contains(&"*://*.doubleclick.net/*".to_owned()),
        "{blocked:?}"
    );
}

/// Sends the protocol command `method` from `on_browser` (`on_tab` false) or
/// from `on_page_created`.
struct SendsRaw {
    method: &'static str,
    on_tab: bool,
}

#[async_trait]
impl Plugin for SendsRaw {
    fn name(&self) -> &str {
        "sends-raw"
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        if !self.on_tab {
            browser.send(self.method, json!({})).await?;
        }
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        if self.on_tab {
            page.send(self.method, json!({})).await?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn a_plugin_cannot_take_over_request_interception() {
    for method in [
        "Fetch.enable",
        "Fetch.disable",
        "Fetch.fulfillRequest",
        "Fetch.continueRequest",
        "Network.setRequestInterception",
    ] {
        for on_tab in [true, false] {
            let mock = MockCtrl::new();
            let plugins = PluginSet::new().with(SendsRaw { method, on_tab }).unwrap();

            let error = launch(plugins, &mock).await.unwrap_err();

            assert!(
                matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "sends-raw"),
                "{method} (tab: {on_tab}): {error:?}"
            );
            assert!(error.to_string().contains(method), "{error}");
            assert!(
                mock.calls_to(method).is_empty(),
                "{method} never reached the browser"
            );
        }
    }
}

#[tokio::test]
async fn the_librarys_own_fetch_for_proxy_passwords_is_not_a_plugins() {
    let mock = MockCtrl::new();
    let options = LaunchOptions::builder()
        .headless(true)
        .proxy("alice:s3cret@proxy.example.com:8080")
        .plugins(PluginSet::new().with(Marker).unwrap())
        .build()
        .unwrap();

    let _browser = Browser::launch_mocked(options, &mock).await.unwrap();

    assert_eq!(
        mock.calls_to("Fetch.enable").len(),
        1,
        "the launch's own interception stays"
    );
}

#[tokio::test]
async fn a_non_fetch_domain_with_a_similar_name_is_not_refused() {
    let mock = MockCtrl::new();
    let plugins = PluginSet::new()
        .with(SendsRaw {
            method: "FetchLikeDomain.enable",
            on_tab: true,
        })
        .unwrap();

    launch(plugins, &mock).await.unwrap();

    assert_eq!(mock.calls_to("FetchLikeDomain.enable").len(), 1);
}
