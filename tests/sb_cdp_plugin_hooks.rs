//! The navigation, context, close and event hooks of the plugin API (0.1.3) on
//! the Pure CDP engine, and the browser, context and tab handles plugins get,
//! against a scripted browser, with no Chrome.
//!
//! Each test reads back what the plugins saw (a log they write) and what the
//! mock browser was asked, so it checks both that a hook ran at the right
//! point and what reached the browser. Run them with
//! `cargo test --features "test-util plugins" --test sb_cdp_plugin_hooks`.

#![cfg(all(feature = "plugins", feature = "test-util"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::plugin_api::{
    async_trait, BrowserHandle, Budgets, Capability, Category, ContextHandle, ContextId,
    ContextSettings, Metadata, NavDecision, NavOutcome, Navigation, PageHandle, Plugin,
    PluginError, PluginSet, ProtocolEvent, Result as PluginResult,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// What the hooks of the plugins in one test saw, in order.
type Log = Arc<Mutex<Vec<String>>>;

fn note(log: &Log, entry: impl Into<String>) {
    log.lock().unwrap().push(entry.into());
}

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// Launches a browser against `mock` with these plugins.
async fn launch_with(options: LaunchOptions, mock: &MockCtrl) -> Browser {
    Browser::launch_mocked(options, mock)
        .await
        .expect("the mocked launch")
}

async fn launch(plugins: PluginSet, mock: &MockCtrl) -> Browser {
    let options = LaunchOptions::builder()
        .headless(true)
        .plugins(plugins)
        .build()
        .unwrap();
    launch_with(options, mock).await
}

/// The addresses the mock was told to open, in order.
fn navigations(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Page.navigate")
        .into_iter()
        .filter_map(|call| call.params["url"].as_str().map(str::to_owned))
        .collect()
}

/// The target ids the mock was told to close.
fn closed_targets(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Target.closeTarget")
        .into_iter()
        .filter_map(|call| call.params["targetId"].as_str().map(str::to_owned))
        .collect()
}

/// Waits (for a few seconds at most) until the log has `count` entries.
async fn until(log: &Log, count: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while entries(log).len() < count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the log has {:?}, not {count} entries", entries(log)));
}

fn budget(configure: impl FnOnce(&mut Budgets)) -> Budgets {
    let mut budgets = Budgets::default();
    configure(&mut budgets);
    budgets
}

// ----------------------------------------------------------------------
// Navigation
// ----------------------------------------------------------------------

/// Keeps tests away from production and writes down every navigation.
struct Fence {
    log: Log,
}

#[async_trait]
impl Plugin for Fence {
    fn name(&self) -> &str {
        "fence"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core)
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        navigation: &Navigation,
    ) -> PluginResult<NavDecision> {
        note(
            &self.log,
            format!("before {} {:?}", navigation.url, navigation.origin),
        );
        Ok(if navigation.url.starts_with("https://prod.example/") {
            NavDecision::Cancel("tests must not touch production".into())
        } else {
            NavDecision::Continue
        })
    }

    async fn after_navigate(
        &self,
        _page: &dyn PageHandle,
        outcome: &NavOutcome,
    ) -> PluginResult<()> {
        note(
            &self.log,
            format!(
                "after {} status={:?} error={:?}",
                outcome.url, outcome.status, outcome.error
            ),
        );
        Ok(())
    }
}

#[tokio::test]
async fn a_plugin_that_cancels_a_navigation_stops_it_before_the_browser_hears_of_it() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let browser = launch(
        PluginSet::new().with(Fence { log: log.clone() }).unwrap(),
        &mock,
    )
    .await;
    let page = browser.default_page().await.unwrap();

    let error = page.goto("https://prod.example/pay").await.unwrap_err();

    let SeleniumBaseError::Plugin { plugin, reason } = &error else {
        panic!("expected a plugin error, got {error:?}");
    };
    assert_eq!(plugin, "fence");
    assert!(
        reason.contains("cancelled by plugin fence: tests must not touch production"),
        "{reason}"
    );
    assert!(
        navigations(&mock).is_empty(),
        "nothing was sent to the browser: {:?}",
        navigations(&mock)
    );
    assert_eq!(
        entries(&log),
        ["before https://prod.example/pay Program"],
        "a navigation that never started has no after_navigate"
    );

    // The tab is still usable: the next navigation goes through, and the
    // plugins hear the status of the document.
    mock.reply("Runtime.evaluate", json!({ "result": { "value": 200 } }));
    page.goto("https://ok.example/").await.unwrap();
    assert_eq!(navigations(&mock), ["https://ok.example/"]);
    assert_eq!(
        entries(&log)[1..],
        [
            "before https://ok.example/ Program",
            "after https://ok.example/ status=Some(200) error=None",
        ]
    );
}

#[tokio::test]
async fn a_navigation_that_fails_is_reported_to_the_plugins_with_the_browsers_reason() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let browser = launch(
        PluginSet::new().with(Fence { log: log.clone() }).unwrap(),
        &mock,
    )
    .await;
    let page = browser.default_page().await.unwrap();
    mock.reply(
        "Page.navigate",
        json!({ "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
    );

    let error = page.goto("https://gone.example/").await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::Navigation { .. }),
        "{error:?}"
    );
    assert_eq!(
        entries(&log)[1],
        "after https://gone.example/ status=None error=Some(\"net::ERR_NAME_NOT_RESOLVED\")"
    );
}

/// Sends one address to another. Redirecting needs `page.navigate`.
struct Redirector;

#[async_trait]
impl Plugin for Redirector {
    fn name(&self) -> &str {
        "redirector"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::PageNavigate)
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        navigation: &Navigation,
    ) -> PluginResult<NavDecision> {
        Ok(if navigation.url == "http://old.example/" {
            NavDecision::Redirect("https://new.example/".into())
        } else {
            NavDecision::Continue
        })
    }
}

#[tokio::test]
async fn a_redirect_changes_what_is_sent_and_the_later_plugins_see_the_new_address() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let plugins = PluginSet::new()
        .with(Redirector)
        .unwrap()
        .with(Fence { log: log.clone() })
        .unwrap();
    let browser = launch(plugins, &mock).await;
    let page = browser.default_page().await.unwrap();

    page.goto("http://old.example/").await.unwrap();

    assert_eq!(navigations(&mock), ["https://new.example/"]);
    assert_eq!(
        entries(&log),
        [
            "before https://new.example/ Program",
            "after https://new.example/ status=None error=None",
        ]
    );
}

#[tokio::test]
async fn goto_if_not_url_runs_the_hooks_only_when_it_navigates() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let browser = launch(
        PluginSet::new().with(Fence { log: log.clone() }).unwrap(),
        &mock,
    )
    .await;
    let page = browser.default_page().await.unwrap();
    mock.reply(
        "Runtime.evaluate",
        json!({ "result": { "value": "https://ok.example/" } }),
    );

    page.goto_if_not_url("https://ok.example/").await.unwrap();
    assert!(
        entries(&log).is_empty(),
        "already there: {:?}",
        entries(&log)
    );

    let error = page
        .goto_if_not_url("https://prod.example/pay")
        .await
        .unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::Plugin { .. }),
        "{error:?}"
    );
    assert_eq!(entries(&log), ["before https://prod.example/pay Program"]);
    assert!(navigations(&mock).is_empty());
}

/// A navigation hook that never finishes, with a short time limit.
struct Stuck;

#[async_trait]
impl Plugin for Stuck {
    fn name(&self) -> &str {
        "stuck"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).budget(budget(|budgets| {
            budgets.before_navigate = Some(Duration::from_millis(50));
        }))
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        _navigation: &Navigation,
    ) -> PluginResult<NavDecision> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn a_navigation_hook_that_runs_out_of_time_stops_the_navigation() {
    let mock = MockCtrl::new();
    let browser = launch(PluginSet::new().with(Stuck).unwrap(), &mock).await;
    let page = browser.default_page().await.unwrap();

    let error = page.goto("https://ok.example/").await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Plugin { reason, .. } if reason.contains("timed out")),
        "{error:?}"
    );
    assert!(navigations(&mock).is_empty());
}

#[tokio::test]
async fn a_tab_a_plugin_stopped_on_its_way_to_its_first_page_is_closed() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let browser = launch(
        PluginSet::new().with(Fence { log: log.clone() }).unwrap(),
        &mock,
    )
    .await;

    let error = browser
        .new_page_at("https://prod.example/pay")
        .await
        .unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::Plugin { .. }),
        "{error:?}"
    );
    assert_eq!(closed_targets(&mock), ["T2"], "the half-made tab is closed");
}

// ----------------------------------------------------------------------
// Contexts
// ----------------------------------------------------------------------

/// What a context hook does after it has noted the context.
#[derive(Clone, Copy)]
enum Fail {
    Never,
    Error,
    Panic,
    Hang,
}

/// Gives new contexts a proxy, and writes down their life.
struct Contexts {
    name: &'static str,
    log: Log,
    proxy: Option<&'static str>,
    fail: Fail,
}

impl Contexts {
    fn new(name: &'static str, log: &Log) -> Self {
        Self {
            name,
            log: log.clone(),
            proxy: None,
            fail: Fail::Never,
        }
    }

    fn proxy(mut self, proxy: &'static str) -> Self {
        self.proxy = Some(proxy);
        self
    }

    fn failing(mut self, fail: Fail) -> Self {
        self.fail = fail;
        self
    }
}

#[async_trait]
impl Plugin for Contexts {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core)
            .capability(Capability::Contexts)
            .capability(Capability::LaunchProxy)
            .budget(budget(|budgets| {
                budgets.other = Some(Duration::from_millis(100));
            }))
    }

    async fn before_context(&self, settings: &mut ContextSettings) -> PluginResult<()> {
        if let Some(proxy) = self.proxy {
            settings.proxy = Some(proxy.to_owned());
            settings.bypass.push("*.internal.example".to_owned());
        }
        Ok(())
    }

    async fn on_context_created(&self, context: &dyn ContextHandle) -> PluginResult<()> {
        note(&self.log, format!("{} created {}", self.name, context.id()));
        match self.fail {
            Fail::Never => Ok(()),
            Fail::Error => Err(PluginError::other("no context for you")),
            Fail::Panic => panic!("a plugin bug"),
            Fail::Hang => std::future::pending::<PluginResult<()>>().await,
        }
    }

    async fn on_context_closed(&self, id: &ContextId) -> PluginResult<()> {
        note(&self.log, format!("{} closed {id}", self.name));
        Ok(())
    }
}

fn mock_with_context() -> MockCtrl {
    let mock = MockCtrl::new();
    mock.reply(
        "Target.createBrowserContext",
        json!({ "browserContextId": "C1" }),
    );
    mock
}

#[tokio::test]
async fn a_context_is_made_with_the_plugins_settings_and_they_hear_when_it_is_gone() {
    let log = Log::default();
    let mock = mock_with_context();
    let plugins = PluginSet::new()
        .with(Contexts::new("anon", &log).proxy("socks5://127.0.0.1:1080"))
        .unwrap();
    let browser = launch(plugins, &mock).await;

    let context = browser.new_context().await.unwrap();

    let made = mock.calls_to("Target.createBrowserContext");
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].params["proxyServer"], "socks5://127.0.0.1:1080");
    assert_eq!(made[0].params["proxyBypassList"], "*.internal.example");
    let route = browser
        .context_route("C1")
        .expect("the proxy is the context's route");
    assert_eq!(route.proxy.server(), "socks5://127.0.0.1:1080");
    assert_eq!(entries(&log), ["anon created C1"]);

    context.dispose().await.unwrap();
    context.dispose().await.unwrap();
    assert_eq!(
        entries(&log),
        ["anon created C1", "anon closed C1"],
        "told once, however often it is disposed of"
    );
}

#[tokio::test]
async fn a_context_without_contexts_plugins_is_made_as_before() {
    let mock = mock_with_context();
    let browser = launch(
        PluginSet::new()
            .with(Fence {
                log: Log::default(),
            })
            .unwrap(),
        &mock,
    )
    .await;

    let context = browser.new_context().await.unwrap();

    let made = mock.calls_to("Target.createBrowserContext");
    assert_eq!(made[0].params, json!({}));
    context.dispose().await.unwrap();
}

/// Makes a context that a plugin stops with `fail`, after another plugin
/// has already heard of it.
async fn a_stopped_context_is_disposed_of(fail: Fail, expected: &str) {
    let log = Log::default();
    let mock = mock_with_context();
    let plugins = PluginSet::new()
        .with(Contexts::new("watcher", &log))
        .unwrap()
        .with(Contexts::new("bad", &log).failing(fail))
        .unwrap();
    let browser = launch(plugins, &mock).await;

    let error = browser.new_context().await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Plugin { reason, .. } if reason.contains(expected)),
        "expected `{expected}` in {error:?}"
    );
    let disposed = mock.calls_to("Target.disposeBrowserContext");
    assert_eq!(disposed.len(), 1, "the half-made context is disposed of");
    assert_eq!(disposed[0].params["browserContextId"], "C1");
    assert!(browser.context_route("C1").is_none());
    assert_eq!(
        entries(&log),
        [
            "watcher created C1",
            "bad created C1",
            "watcher closed C1",
            "bad closed C1",
        ],
        "every plugin that heard of the context hears it is gone"
    );
}

#[tokio::test]
async fn a_context_hook_that_fails_does_not_leave_the_context_open() {
    a_stopped_context_is_disposed_of(Fail::Error, "no context for you").await;
}

#[tokio::test]
async fn a_context_hook_that_panics_does_not_leave_the_context_open() {
    a_stopped_context_is_disposed_of(Fail::Panic, "panicked in `on_context_created`").await;
}

#[tokio::test]
async fn a_context_hook_that_runs_out_of_time_does_not_leave_the_context_open() {
    a_stopped_context_is_disposed_of(Fail::Hang, "timed out").await;
}

struct BadBeforeContext;

#[async_trait]
impl Plugin for BadBeforeContext {
    fn name(&self) -> &str {
        "bad-before"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Contexts)
    }

    async fn before_context(&self, _settings: &mut ContextSettings) -> PluginResult<()> {
        Err(PluginError::other("not now"))
    }
}

#[tokio::test]
async fn a_plugin_that_refuses_in_before_context_makes_no_context() {
    let mock = mock_with_context();
    let browser = launch(PluginSet::new().with(BadBeforeContext).unwrap(), &mock).await;

    let error = browser.new_context().await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::Plugin { .. }),
        "{error:?}"
    );
    assert!(mock.calls_to("Target.createBrowserContext").is_empty());
}

// ----------------------------------------------------------------------
// The handles a plugin gets
// ----------------------------------------------------------------------

/// Opens a context and a tab in it from `on_browser`, drives the tab through
/// its handle, and closes both.
struct Spawner {
    log: Log,
}

#[async_trait]
impl Plugin for Spawner {
    fn name(&self) -> &str {
        "spawner"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capabilities([
            Capability::Contexts,
            Capability::PageNavigate,
            Capability::Cdp("Network".into()),
        ])
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        note(
            &self.log,
            format!("browser {} {}", browser.id(), browser.extensions().len()),
        );
        let context = browser.new_context(ContextSettings::new()).await?;
        let page = context.new_page(Some("https://app.example/")).await?;
        note(
            &self.log,
            format!(
                "page in {}",
                page.context()
                    .map_or("none".to_owned(), |id| id.to_string())
            ),
        );
        page.navigate("https://app.example/next").await?;
        page.block_urls(&["*.ads.example".to_owned()]).await?;
        page.close().await?;
        context.dispose().await
    }
}

/// Blocks a tracker in every new tab.
struct Blocker;

#[async_trait]
impl Plugin for Blocker {
    fn name(&self) -> &str {
        "blocker"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Cdp("Network".into()))
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        page.block_urls(&["*.tracker.example".to_owned()]).await
    }
}

#[tokio::test]
async fn a_plugin_opens_a_context_and_a_tab_through_the_handles_and_the_hooks_see_them() {
    let log = Log::default();
    let mock = mock_with_context();
    let plugins = PluginSet::new()
        .with(Blocker)
        .unwrap()
        .with(Fence { log: log.clone() })
        .unwrap()
        .with(Spawner { log: log.clone() })
        .unwrap();

    let _browser = launch(plugins, &mock).await;

    let log = entries(&log);
    assert_eq!(log[0], "browser cdp:127.0.0.1:0 0");
    assert_eq!(log[1], "before https://app.example/ Program");
    assert_eq!(log[2], "after https://app.example/ status=None error=None");
    assert_eq!(log[3], "page in C1", "the tab names the context it is in");
    assert_eq!(log[4], "before https://app.example/next Program");

    let created = mock.calls_to("Target.createTarget");
    assert_eq!(created[0].params["browserContextId"], "C1");
    assert_eq!(
        navigations(&mock),
        ["https://app.example/", "https://app.example/next"],
        "the handle's navigations went through the hooks"
    );
    let blocklists: Vec<Value> = mock
        .calls_to("Network.setBlockedURLs")
        .into_iter()
        .map(|call| call.params["urls"].clone())
        .collect();
    assert!(
        blocklists.contains(&json!(["*.tracker.example", "*.ads.example"])),
        "the tab blocks the union of what both plugins asked for: {blocklists:?}"
    );
    assert_eq!(closed_targets(&mock), ["T2"]);
    let disposed = mock.calls_to("Target.disposeBrowserContext");
    assert_eq!(disposed[0].params["browserContextId"], "C1");
}

/// Opens a tab at production from `on_browser` and writes down what happens.
struct Wanderer {
    log: Log,
}

#[async_trait]
impl Plugin for Wanderer {
    fn name(&self) -> &str {
        "wanderer"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::PageNavigate)
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        let outcome = browser.new_page(Some("https://prod.example/pay")).await;
        note(
            &self.log,
            match outcome {
                Ok(_) => "opened".to_owned(),
                Err(error) => format!("refused: {error}"),
            },
        );
        Ok(())
    }
}

#[tokio::test]
async fn another_plugins_cancel_reaches_the_plugin_that_opened_the_tab_and_the_tab_is_closed() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let plugins = PluginSet::new()
        .with(Fence {
            log: Log::default(),
        })
        .unwrap()
        .with(Wanderer { log: log.clone() })
        .unwrap();

    let _browser = launch(plugins, &mock).await;

    let log = entries(&log);
    assert_eq!(log.len(), 1);
    assert!(log[0].starts_with("refused: "), "{log:?}");
    assert!(
        log[0].contains("cancelled by plugin fence: tests must not touch production"),
        "{log:?}"
    );
    assert_eq!(closed_targets(&mock), ["T2"]);
}

// ----------------------------------------------------------------------
// Close, extensions and protocol events
// ----------------------------------------------------------------------

/// Saves state while the browser answers, then cleans up.
struct Closer {
    log: Log,
}

#[async_trait]
impl Plugin for Closer {
    fn name(&self) -> &str {
        "closer"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Cdp("Network".into()))
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        let extensions: Vec<String> = browser
            .extensions()
            .iter()
            .map(|extension| format!("{}={}", extension.id, extension.path.display()))
            .collect();
        note(&self.log, format!("extensions {extensions:?}"));
        Ok(())
    }

    async fn before_close(&self, browser: &dyn BrowserHandle) -> PluginResult<()> {
        browser.send("Network.enable", json!({})).await?;
        note(&self.log, "before_close");
        Ok(())
    }

    async fn on_close(&self) -> PluginResult<()> {
        note(&self.log, "on_close");
        Ok(())
    }
}

#[tokio::test]
async fn before_close_runs_while_the_browser_answers_and_before_on_close() {
    let log = Log::default();
    let mock = MockCtrl::new();
    mock.reply("Extensions.loadUnpacked", json!({ "id": "abcdef" }));
    let options = LaunchOptions::builder()
        .headless(true)
        .extension("/opt/extensions/a")
        .plugins(PluginSet::new().with(Closer { log: log.clone() }).unwrap())
        .build()
        .unwrap();
    let browser = launch_with(options, &mock).await;
    assert_eq!(
        entries(&log),
        ["extensions [\"abcdef=/opt/extensions/a\"]"],
        "the handle names the extensions the launch loaded"
    );
    assert!(mock.calls_to("Network.enable").is_empty());

    browser.close().await.unwrap();
    browser.close().await.unwrap();

    assert_eq!(
        entries(&log)[1..],
        ["before_close", "on_close"],
        "once, in this order"
    );
    assert_eq!(
        mock.calls_to("Network.enable").len(),
        1,
        "the plugin's command reached the browser before it closed"
    );
}

/// Wants the log entries of the browser.
struct Listener {
    log: Log,
}

#[async_trait]
impl Plugin for Listener {
    fn name(&self) -> &str {
        "listener"
    }

    fn protocol_events(&self) -> &[&str] {
        &["Log.entryAdded"]
    }

    async fn on_protocol_event(
        &self,
        browser: &dyn BrowserHandle,
        event: &ProtocolEvent,
    ) -> PluginResult<()> {
        note(
            &self.log,
            format!(
                "{} {} on {}",
                event.method,
                event.params["entry"]["text"],
                browser.id()
            ),
        );
        Ok(())
    }
}

#[tokio::test]
async fn the_protocol_events_a_plugin_named_reach_it_and_no_others() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let browser = launch(
        PluginSet::new()
            .with(Listener { log: log.clone() })
            .unwrap(),
        &mock,
    )
    .await;

    mock.emit("Page.frameNavigated", json!({}), Some("S1"));
    mock.emit(
        "Log.entryAdded",
        json!({ "entry": { "text": "hello" } }),
        Some("S1"),
    );
    until(&log, 1).await;
    mock.emit("Page.frameNavigated", json!({}), Some("S1"));
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        entries(&log),
        ["Log.entryAdded \"hello\" on cdp:127.0.0.1:0"]
    );
    browser.close().await.unwrap();
}

// ----------------------------------------------------------------------
// Order, and what a redirect may not do
// ----------------------------------------------------------------------

/// Sends every navigation to a local file, which a redirect may not do.
struct FileRedirector;

#[async_trait]
impl Plugin for FileRedirector {
    fn name(&self) -> &str {
        "file-redirector"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::PageNavigate)
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        _navigation: &Navigation,
    ) -> PluginResult<NavDecision> {
        Ok(NavDecision::Redirect("file:///etc/passwd".into()))
    }
}

#[tokio::test]
async fn a_redirect_to_a_file_is_refused_and_nothing_is_sent() {
    let mock = MockCtrl::new();
    let browser = launch(PluginSet::new().with(FileRedirector).unwrap(), &mock).await;
    let page = browser.default_page().await.unwrap();

    let error = page.goto("https://ok.example/").await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Plugin { plugin, reason }
            if plugin == "file-redirector" && reason.contains("refused")),
        "{error:?}"
    );
    assert!(navigations(&mock).is_empty());
}

/// Writes down where in a context's life each of its hooks runs.
struct Ordered {
    name: &'static str,
    log: Log,
}

#[async_trait]
impl Plugin for Ordered {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Contexts)
    }

    async fn before_context(&self, _settings: &mut ContextSettings) -> PluginResult<()> {
        note(&self.log, format!("{} before_context", self.name));
        Ok(())
    }

    async fn on_context_created(&self, _context: &dyn ContextHandle) -> PluginResult<()> {
        note(&self.log, format!("{} on_context_created", self.name));
        Ok(())
    }

    async fn on_context_closed(&self, _id: &ContextId) -> PluginResult<()> {
        note(&self.log, format!("{} on_context_closed", self.name));
        Ok(())
    }
}

#[tokio::test]
async fn the_context_hooks_run_in_plugin_order_around_the_browsers_own_commands() {
    let log = Log::default();
    let mock = MockCtrl::new();
    let creating = log.clone();
    mock.on("Target.createBrowserContext", move |_| {
        note(&creating, "browser creates the context");
        Ok(json!({ "browserContextId": "C1" }))
    });
    let disposing = log.clone();
    mock.on("Target.disposeBrowserContext", move |_| {
        note(&disposing, "browser disposes of the context");
        Ok(json!({}))
    });
    let plugins = PluginSet::new()
        .with(Ordered {
            name: "first",
            log: log.clone(),
        })
        .unwrap()
        .with(Ordered {
            name: "second",
            log: log.clone(),
        })
        .unwrap();
    let browser = launch(plugins, &mock).await;

    let context = browser.new_context().await.unwrap();
    context.dispose().await.unwrap();

    assert_eq!(
        entries(&log),
        [
            "first before_context",
            "second before_context",
            "browser creates the context",
            "first on_context_created",
            "second on_context_created",
            "browser disposes of the context",
            "first on_context_closed",
            "second on_context_closed",
        ]
    );
}
