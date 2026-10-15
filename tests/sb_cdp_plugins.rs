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
use seleniumbase_rs::SeleniumBaseError;
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
