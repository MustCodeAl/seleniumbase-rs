//! The plugin hooks of ADR 004 for the Pure CDP engine (feature `plugins`).
//!
//! [`Browser`](super::Browser) calls these at fixed points: [`before_launch`]
//! before the process starts, [`on_browser`] once it answers (launched or
//! attached), [`on_page_created`] for every tab it attaches to, and
//! [`on_close`] when it closes. Plugins see the browser and its tabs through
//! [`BrowserLink`] and [`TabLink`], which send protocol commands over the
//! browser's own connection, so a plugin never names a type of this crate.
//! A [`Page`] is a [`PageHandle`] too, so a program can hand one of its tabs
//! to code written against the plugin API.
//!
//! Since plugin API 0.1.3 the engine also calls [`navigate`] around
//! [`Page::goto`] (`before_navigate`, `after_navigate`), [`before_context`],
//! [`on_context_created`] and [`on_context_closed`] around a context's life,
//! [`before_close`] ahead of `on_close`, and [`deliver_event`] for the protocol
//! events a plugin named. The handles open tabs and contexts for plugins
//! ([`BrowserLink`], [`ContextLink`]), and what they open goes through the same
//! hooks. The network hooks run in the router of `plugin_network`; the test and task hooks are not wired yet.

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

use seleniumbase_rs_plugin::{
    async_trait, BrowserFamily, BrowserHandle, BrowserId, ContextHandle, ContextId,
    ContextSettings, Engine, HostInfo, LaunchContext, LaunchSettings, LoadedExtension, NavOrigin,
    NavOutcome, Navigation, PageHandle, PageId, PluginError, PluginSet, ProfileView, ProtocolEvent,
    Result as PluginResult,
};
use serde_json::{json, Value};

use super::browser::Browser;
use super::client::{CdpEvent, Client};
use super::context::{BrowserContext, ContextOptions};
use super::launch::{LaunchOptions, Proxy};
use super::page::Page;
use super::sync::locked;
use crate::error::SeleniumBaseError;
use crate::plugin_host::{convert, guard};

/// What this engine supports for the plugins it hosts: everything.
pub(super) const HOST: HostInfo = HostInfo::cdp();

/// What the set checks its plugins against: always a Chromium browser on this
/// engine.
pub(super) fn launch_context(headless: bool, connecting: bool) -> LaunchContext {
    LaunchContext {
        engine: Engine::Cdp,
        family: BrowserFamily::Chromium,
        headless,
        connecting,
    }
}

/// The context for a browser attached with `connect`, from its
/// `/json/version` answer: one that calls itself `HeadlessChrome` runs
/// without a window.
pub(super) fn connect_context(version: &Value) -> LaunchContext {
    let headless = version["User-Agent"]
        .as_str()
        .is_some_and(|agent| agent.contains("HeadlessChrome"));
    launch_context(headless, true)
}

/// The plugins of a browser attached with `connect`: fresh instances of the
/// per-launch ones, checked against the browser's `/json/version` answer.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] when a plugin refuses the browser.
pub(super) fn for_attach(
    plugins: &PluginSet,
    version: &Value,
) -> Result<PluginSet, SeleniumBaseError> {
    let plugins = crate::plugin_host::timed(plugins.for_launch());
    plugins.check_with(&connect_context(version), &HOST)?;
    Ok(plugins)
}

/// The start-up settings a plugin may change, taken from `options`.
///
/// The proxy is given as its server only: a password in the options is not
/// shown to plugins, and it stays when they leave the proxy as it is. The
/// profile is read-only to plugins: persistent when the options name a folder
/// for it, and a throwaway one otherwise.
pub(super) fn settings_of(options: &LaunchOptions) -> LaunchSettings {
    let mut settings = LaunchSettings::default();
    settings.args.clone_from(&options.extra_args);
    settings.prefs.clone_from(&options.prefs);
    settings.extensions.clone_from(&options.extensions);
    settings.proxy = options
        .proxy
        .as_ref()
        .map(|proxy| proxy.server().to_owned());
    settings.headless = options.is_headless();
    settings
        .host_resolver_rules
        .clone_from(&options.host_resolver_rules);
    settings
        .disable_features
        .clone_from(&options.disable_features);
    settings
        .enable_features
        .clone_from(&options.enable_features);
    settings.disable_quic = options.disable_quic;
    settings.webrtc_policy = options.webrtc.map(convert::webrtc_to_api);
    settings.set_profile(match &options.user_data_dir {
        Some(folder) => ProfileView::Persistent(folder.clone()),
        None => ProfileView::Ephemeral,
    });
    settings
}

/// Puts the settings the plugins returned back into `options`.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when a plugin set a proxy that cannot
/// be parsed, or a WebRTC policy this version cannot apply.
pub(super) fn apply_settings(
    options: &mut LaunchOptions,
    settings: LaunchSettings,
) -> Result<(), SeleniumBaseError> {
    let LaunchSettings {
        args,
        prefs,
        extensions,
        proxy,
        headless,
        host_resolver_rules,
        webrtc_policy,
        disable_features,
        enable_features,
        disable_quic,
        ..
    } = settings;
    options.extra_args = args;
    options.prefs = prefs;
    options.extensions = extensions;
    options.host_resolver_rules = host_resolver_rules;
    options.disable_features = disable_features;
    options.enable_features = enable_features;
    options.disable_quic = disable_quic;
    options.webrtc = webrtc_policy
        .map(convert::webrtc_from_api)
        .transpose()?
        .flatten();
    if proxy.as_deref() != options.proxy.as_ref().map(Proxy::server) {
        options.proxy = proxy
            .as_deref()
            .map(Proxy::parse)
            .transpose()
            .map_err(|error| {
                // The proxy itself is not shown: it may carry a password.
                SeleniumBaseError::invalid_config(format!(
                    "the proxy a plugin set is not valid ({error})"
                ))
            })?;
    }
    // Left alone, an unset choice stays unset rather than becoming this
    // platform's default.
    if headless != options.is_headless() {
        options.headless = Some(headless);
    }
    Ok(())
}

/// Checks the plugins of `options` and lets them change its start-up settings.
///
/// The set is checked again afterwards, because one plugin may change what
/// another was checked against, such as headless mode.
///
/// The options come back with the set of *this* launch: each plugin that
/// offers [`per_launch`](seleniumbase_rs_plugin::Plugin::per_launch) is
/// replaced by a fresh instance, so browsers launched from one set (a pool)
/// do not share a plugin's state, and the hooks that follow reach the same
/// instances.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] when a plugin is refused or its hook fails,
/// and what [`apply_settings`] returns.
pub(super) async fn before_launch(
    mut options: LaunchOptions,
) -> Result<LaunchOptions, SeleniumBaseError> {
    if options.plugins.is_empty() {
        return Ok(options);
    }
    options.plugins = crate::plugin_host::timed(options.plugins.for_launch());
    let plugins = options.plugins.clone();
    plugins.check_with(&launch_context(options.is_headless(), false), &HOST)?;
    let mut settings = settings_of(&options);
    plugins.before_launch(&mut settings).await?;
    apply_settings(&mut options, settings)?;
    plugins.check_with(&launch_context(options.is_headless(), false), &HOST)?;
    Ok(options)
}

/// Tells the plugins the browser runs.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first hook that fails.
pub(super) async fn on_browser(
    plugins: &PluginSet,
    browser: &Browser,
) -> Result<(), SeleniumBaseError> {
    if plugins.is_empty() {
        return Ok(());
    }
    Ok(plugins.on_browser(&BrowserLink { browser }).await?)
}

/// Adds the plugins' packages to the tab `page` and calls their
/// `on_page_created`.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first package or hook that fails.
pub(super) async fn on_page_created(
    plugins: &PluginSet,
    page: &Page,
) -> Result<(), SeleniumBaseError> {
    if plugins.is_empty() {
        return Ok(());
    }
    Ok(plugins.on_page_created(page).await?)
}

/// Tells the plugins the browser closes, and logs the hooks that fail: a
/// browser closes whatever its plugins say.
pub(super) async fn on_close(plugins: &PluginSet) {
    for error in plugins.on_close().await {
        tracing::event!(
            name: "cdp.plugin.close.failed",
            tracing::Level::WARN,
            error = %error,
            "a plugin failed while the browser closed: {{error}}",
        );
    }
}

/// Tells the plugins the browser is about to close, while it still answers
/// commands, and logs the hooks that fail: a browser closes whatever its
/// plugins say.
pub(super) async fn before_close(browser: &Browser) {
    let plugins = &browser.inner().options.plugins;
    if plugins.is_empty() {
        return;
    }
    for error in plugins.before_close(&BrowserLink { browser }).await {
        tracing::event!(
            name: "cdp.plugin.before_close.failed",
            tracing::Level::WARN,
            error = %error,
            "a plugin failed before the browser closed: {{error}}",
        );
    }
}

/// Gives a protocol event to the plugins that named it, and logs the hooks
/// that fail.
pub(super) async fn deliver_event(browser: &Browser, event: CdpEvent) {
    let plugins = &browser.inner().options.plugins;
    let event = ProtocolEvent::new(event.method, event.params);
    for error in plugins
        .on_protocol_event(&BrowserLink { browser }, &event)
        .await
    {
        tracing::event!(
            name: "cdp.plugin.protocol_event.failed",
            tracing::Level::WARN,
            protocol_event = %event.method,
            error = %error,
            "a plugin failed on the protocol event {{protocol_event}}: {{error}}",
        );
    }
}

/// Runs a navigation the program asked for through the plugins: each
/// `before_navigate` may let it go on, send it to another address or cancel
/// it, and every `after_navigate` sees how it ended.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] when a plugin cancels the navigation or its
/// hook fails (nothing is sent to the browser then), and what the navigation
/// itself returns.
pub(super) async fn navigate(
    page: &Page,
    url: String,
    origin: NavOrigin,
) -> Result<(), SeleniumBaseError> {
    let plugins = &page.browser().inner().options.plugins;
    let navigation = plugins
        .before_navigate(page, &Navigation::new(url, origin))
        .await?;
    let result = page.load(navigation.url.clone()).await;
    let outcome = outcome_of(page, navigation.url, &result).await;
    for error in plugins.after_navigate(page, &outcome).await {
        tracing::event!(
            name: "cdp.plugin.after_navigate.failed",
            tracing::Level::WARN,
            error = %error,
            "a plugin failed after a navigation: {{error}}",
        );
    }
    result
}

/// How a navigation to `url` ended, as the plugins are told.
async fn outcome_of(
    page: &Page,
    url: String,
    result: &Result<(), SeleniumBaseError>,
) -> NavOutcome {
    match result {
        Err(SeleniumBaseError::Navigation { reason, .. }) => {
            NavOutcome::new(url).failed(reason.clone())
        }
        Err(error) => NavOutcome::new(url).failed(error.to_string()),
        Ok(()) => match document_status(page).await {
            Some(status) => NavOutcome::new(url).status(status),
            None => NavOutcome::new(url),
        },
    }
}

/// The HTTP status of the document the tab loaded, when the browser reports it
/// (`responseStatus` of the navigation timing entry).
async fn document_status(page: &Page) -> Option<u16> {
    let answer = page
        .execute(
            "Runtime.evaluate",
            json!({
                "expression": "(performance.getEntriesByType('navigation')[0] || {}).responseStatus || 0",
                "returnByValue": true,
            }),
        )
        .await
        .ok()?;
    let status = u16::try_from(answer["result"]["value"].as_u64()?).ok()?;
    (status > 0).then_some(status)
}

/// Lets the plugins change the options of a context about to be made.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first hook that fails or rule that
/// is broken (no context is made then), and
/// [`SeleniumBaseError::InvalidConfig`] when a plugin set a proxy that cannot
/// be parsed.
pub(super) async fn before_context(
    browser: &Browser,
    options: ContextOptions,
) -> Result<ContextOptions, SeleniumBaseError> {
    let plugins = &browser.inner().options.plugins;
    if plugins.is_empty() {
        return Ok(options);
    }
    let mut settings = options.settings();
    plugins.before_context(&mut settings).await?;
    options.with_settings(settings)
}

/// Tells the plugins a context exists. When one of them stops it, the context
/// is disposed of, so it is neither handed out nor left open.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first hook that fails.
pub(super) async fn on_context_created(
    browser: &Browser,
    context: &BrowserContext,
) -> Result<(), SeleniumBaseError> {
    let plugins = &browser.inner().options.plugins;
    if plugins.is_empty() {
        return Ok(());
    }
    let link = ContextLink::new(context.clone());
    if let Err(error) = plugins.on_context_created(&link).await {
        if let Err(failure) = context.dispose().await {
            tracing::event!(
                name: "cdp.plugin.context.dispose_failed",
                tracing::Level::WARN,
                error = %failure,
                "a context a plugin stopped could not be disposed of: {{error}}",
            );
        }
        return Err(error.into());
    }
    Ok(())
}

/// Tells the plugins the context `id` is gone, and logs the hooks that fail.
pub(super) async fn on_context_closed(browser: &Browser, id: &str) {
    let plugins = &browser.inner().options.plugins;
    if plugins.is_empty() {
        return;
    }
    for error in plugins.on_context_closed(&ContextId::new(id)).await {
        tracing::event!(
            name: "cdp.plugin.context.closed_failed",
            tracing::Level::WARN,
            error = %error,
            "a plugin failed when a context closed: {{error}}",
        );
    }
}

/// A protocol failure, as a plugin's hook sees it.
fn refused(error: &SeleniumBaseError) -> PluginError {
    PluginError::other(error.to_string())
}

/// The browser, as plugins see it: browser-level commands on its connection,
/// and the tabs and contexts it opens for them. What a plugin opens goes
/// through the same hooks as what the program opens.
#[derive(Debug)]
struct BrowserLink<'a> {
    browser: &'a Browser,
}

#[async_trait]
impl BrowserHandle for BrowserLink<'_> {
    fn engine(&self) -> Engine {
        Engine::Cdp
    }

    fn family(&self) -> BrowserFamily {
        BrowserFamily::Chromium
    }

    fn id(&self) -> BrowserId {
        BrowserId::new(format!(
            "cdp:{}:{}",
            self.browser.debugging_host(),
            self.browser.debugging_port()
        ))
    }

    fn extensions(&self) -> Vec<LoadedExtension> {
        self.browser
            .loaded_extensions()
            .into_iter()
            .map(|(id, path)| LoadedExtension::new(id, path))
            .collect()
    }

    async fn new_context<'a>(
        &'a self,
        settings: ContextSettings,
    ) -> PluginResult<Box<dyn ContextHandle + 'a>> {
        let options = ContextOptions::from_settings(settings).map_err(|error| refused(&error))?;
        let context = self
            .browser
            .new_context_with(options)
            .await
            .map_err(|error| refused(&error))?;
        Ok(Box::new(ContextLink::new(context)))
    }

    async fn new_page<'a>(&'a self, url: Option<&str>) -> PluginResult<Box<dyn PageHandle + 'a>> {
        let page = match url {
            Some(url) => self.browser.new_page_at(url).await,
            None => self.browser.new_page().await,
        }
        .map_err(|error| refused(&error))?;
        Ok(Box::new(page))
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        guard::refuse_interception(method)?;
        self.browser
            .execute(method, params)
            .await
            .map_err(|error| refused(&error))
    }
}

/// A browser context, as plugins see it.
#[derive(Debug)]
struct ContextLink {
    context: BrowserContext,
    id: ContextId,
}

impl ContextLink {
    fn new(context: BrowserContext) -> Self {
        let id = ContextId::new(context.id());
        Self { context, id }
    }
}

#[async_trait]
impl ContextHandle for ContextLink {
    fn id(&self) -> &ContextId {
        &self.id
    }

    async fn new_page<'a>(&'a self, url: Option<&str>) -> PluginResult<Box<dyn PageHandle + 'a>> {
        let page = match url {
            Some(url) => self.context.new_page_at(url).await,
            None => self.context.new_page().await,
        }
        .map_err(|error| refused(&error))?;
        Ok(Box::new(page))
    }

    async fn dispose(&self) -> PluginResult<()> {
        self.context
            .dispose()
            .await
            .map_err(|error| refused(&error))
    }
}

/// The blocklist of each tab session: the union of every pattern the library
/// and its plugins asked it to block.
///
/// `Network.setBlockedURLs` replaces a tab's list, so plugins that each send
/// theirs, or one that follows the library's ad-block list, would undo each
/// other. Every raw send goes through [`TabLink`], which adds its patterns
/// here and sends the union.
#[derive(Debug, Default)]
pub(super) struct BlockedUrls(StdMutex<HashMap<String, Vec<String>>>);

impl BlockedUrls {
    /// Starts the list of `session` with patterns the library blocked itself.
    pub(super) fn seed(&self, session: &str, urls: &[&str]) {
        let mut lists = locked(&self.0);
        let list = lists.entry(session.to_owned()).or_default();
        for url in urls {
            if !list.iter().any(|seen| seen == url) {
                list.push((*url).to_owned());
            }
        }
    }

    /// Forgets the list of a tab that went away.
    pub(super) fn forget(&self, session: &str) {
        locked(&self.0).remove(session);
    }

    /// Adds the patterns of a `Network.setBlockedURLs` command to the list of
    /// `session` and returns the command to send.
    fn merge(&self, session: &str, params: &Value) -> PluginResult<Value> {
        let mut lists = locked(&self.0);
        let list = lists.entry(session.to_owned()).or_default();
        guard::merge_blocked_urls(list, params)
    }
}

/// One tab, as plugins see it: commands on the protocol session attached to
/// it.
#[derive(Debug)]
struct TabLink<'a> {
    client: &'a Client,
    session: &'a str,
    blocked: &'a BlockedUrls,
}

impl TabLink<'_> {
    /// Sends `method` to the tab. The `Fetch` domain is refused, and a
    /// blocklist is merged with the tab's others first (see [`BlockedUrls`]).
    async fn command(&self, method: &str, params: Value) -> PluginResult<Value> {
        guard::refuse_interception(method)?;
        let params = if method == guard::SET_BLOCKED_URLS {
            self.blocked.merge(self.session, &params)?
        } else {
            params
        };
        self.client
            .send(method, params, Some(self.session))
            .await
            .map_err(|error| refused(&error))
    }

    /// Blocks the requests that match `patterns`, in addition to what the
    /// library and the other plugins block in this tab: the tab's list is the
    /// union of them all (see [`BlockedUrls`]).
    async fn block_urls(&self, patterns: &[String]) -> PluginResult<()> {
        // The list only takes effect while the Network domain is on.
        self.command("Network.enable", json!({})).await?;
        self.command(guard::SET_BLOCKED_URLS, json!({ "urls": patterns }))
            .await?;
        Ok(())
    }

    /// Runs `Runtime.evaluate` with `params` and returns the value, or the
    /// exception the script threw as an error.
    async fn run(&self, params: Value) -> PluginResult<Value> {
        let response = self.command("Runtime.evaluate", params).await?;
        if let Some(details) = response.get("exceptionDetails") {
            let description = details["exception"]["description"]
                .as_str()
                .or_else(|| details["text"].as_str())
                .unwrap_or("script error");
            return Err(PluginError::other(format!(
                "the script threw: {description}"
            )));
        }
        Ok(response["result"]["value"].clone())
    }
}

#[async_trait]
impl PageHandle for TabLink<'_> {
    fn engine(&self) -> Engine {
        Engine::Cdp
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        self.command(method, params).await
    }

    async fn add_init_script(&self, source: &str) -> PluginResult<()> {
        self.command(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": source }),
        )
        .await?;
        // The document already loaded gets it too, as the page helpers do.
        self.run(json!({ "expression": source })).await?;
        Ok(())
    }

    async fn evaluate(&self, expression: &str) -> PluginResult<Value> {
        self.run(json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true,
        }))
        .await
    }
}

/// A tab, as the plugin API sees it: what [`TabLink`] does, on the tab's own
/// protocol session, and the typed methods of the handle.
///
/// The trait's `evaluate` and `add_init_script` differ from the methods of
/// the same names on [`Page`]: `add_init_script` here also runs the script in
/// the current document, as the plugin API asks, and returns no identifier.
/// `navigate` is [`Page::goto`], so the navigation hooks run for it as for
/// the program's own navigations (the plugin API refuses it inside
/// `before_navigate`, where it would run them again).
#[async_trait]
impl PageHandle for Page {
    fn engine(&self) -> Engine {
        Engine::Cdp
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        self.tab_link().send(method, params).await
    }

    async fn add_init_script(&self, source: &str) -> PluginResult<()> {
        self.tab_link().add_init_script(source).await
    }

    async fn evaluate(&self, expression: &str) -> PluginResult<Value> {
        PageHandle::evaluate(&self.tab_link(), expression).await
    }

    fn id(&self) -> PageId {
        PageId::new(Page::id(self))
    }

    fn context(&self) -> Option<ContextId> {
        self.browser()
            .tab_context(Page::id(self))
            .map(ContextId::new)
    }

    async fn url(&self) -> PluginResult<String> {
        Page::url(self).await.map_err(|error| refused(&error))
    }

    async fn navigate(&self, url: &str) -> PluginResult<()> {
        self.goto(url).await.map_err(|error| refused(&error))
    }

    async fn block_urls(&self, patterns: &[String]) -> PluginResult<()> {
        self.tab_link().block_urls(patterns).await
    }

    async fn close(&self) -> PluginResult<()> {
        Page::close(self).await.map_err(|error| refused(&error))
    }
}

impl Page {
    /// This tab as the hooks see a tab.
    fn tab_link(&self) -> TabLink<'_> {
        let inner = self.browser().inner();
        TabLink {
            client: &inner.client,
            session: self.session_id(),
            blocked: &inner.plugin_blocked,
        }
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs_plugin::{async_trait, Plugin, Requirements};

    use super::*;

    fn options(builder: crate::sb_cdp::LaunchOptionsBuilder) -> LaunchOptions {
        builder.build().unwrap()
    }

    #[test]
    fn the_settings_plugins_see_come_from_the_options() {
        let options = options(
            LaunchOptions::builder()
                .headless(true)
                .arg("--mute-audio")
                .extension("/opt/extensions/a")
                .preference("intl.accept_languages", "de-DE")
                .proxy("alice:s3cret@proxy.example.com:8080"),
        );

        let settings = settings_of(&options);

        assert_eq!(settings.args, ["--mute-audio"]);
        assert_eq!(
            settings.extensions,
            [std::path::PathBuf::from("/opt/extensions/a")]
        );
        assert_eq!(settings.prefs["intl.accept_languages"], "de-DE");
        assert_eq!(
            settings.proxy.as_deref(),
            Some("proxy.example.com:8080"),
            "the password is not shown to plugins"
        );
        assert!(settings.headless);
    }

    #[test]
    fn settings_left_alone_change_nothing() {
        let original = options(
            LaunchOptions::builder()
                .arg("--mute-audio")
                .proxy("alice:s3cret@proxy.example.com:8080"),
        );
        let mut options = original.clone();

        apply_settings(&mut options, settings_of(&original)).unwrap();

        assert_eq!(options.extra_args, original.extra_args);
        assert_eq!(
            options.proxy, original.proxy,
            "the password stays when the plugins keep the proxy"
        );
        assert_eq!(options.headless, None, "an unset choice stays unset");
    }

    #[test]
    fn settings_a_plugin_changed_reach_the_options() {
        let mut options = options(
            LaunchOptions::builder()
                .headless(true)
                .proxy("alice:s3cret@proxy.example.com:8080"),
        );
        let mut settings = settings_of(&options);
        settings.args.push("--lang=de".to_owned());
        settings
            .extensions
            .push(std::path::PathBuf::from("/opt/extensions/b"));
        settings
            .prefs
            .insert("webkit.webprefs.default_font_size".to_owned(), json!(22));
        settings.proxy = Some("socks5://bob:pw@10.0.0.1:1080".to_owned());
        settings.headless = false;

        apply_settings(&mut options, settings).unwrap();

        assert_eq!(options.extra_args, ["--lang=de"]);
        assert_eq!(
            options.extensions,
            [std::path::PathBuf::from("/opt/extensions/b")]
        );
        assert_eq!(options.prefs["webkit.webprefs.default_font_size"], 22);
        let proxy = options.proxy.as_ref().unwrap();
        assert_eq!(proxy.server(), "socks5://10.0.0.1:1080");
        assert_eq!(proxy.credentials(), Some(("bob", "pw")));
        assert!(!options.is_headless());
    }

    #[test]
    fn a_plugin_can_remove_the_proxy_but_not_set_a_broken_one() {
        let mut options = options(LaunchOptions::builder().proxy("proxy.example.com:8080"));
        let mut settings = settings_of(&options);
        settings.proxy = None;
        apply_settings(&mut options, settings).unwrap();
        assert!(options.proxy.is_none());

        let mut settings = settings_of(&options);
        settings.proxy = Some("user:pass@".to_owned());
        let error = apply_settings(&mut options, settings).unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
    }

    struct LaunchOnly;

    #[async_trait]
    impl Plugin for LaunchOnly {
        fn name(&self) -> &'static str {
            "launch-only"
        }

        fn requirements(&self) -> Requirements {
            Requirements::LAUNCH_ONLY | Requirements::HEADFUL
        }
    }

    #[test]
    fn an_attached_browser_is_checked_as_attached_and_by_its_user_agent() {
        let headless = connect_context(&json!({
            "User-Agent": "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 HeadlessChrome/155.0 Safari/537.36",
        }));
        assert!(headless.connecting && headless.headless);
        assert_eq!(headless.engine, Engine::Cdp);
        assert_eq!(headless.family, BrowserFamily::Chromium);
        let headed = connect_context(&json!({ "User-Agent": "Mozilla/5.0 Chrome/155.0" }));
        assert!(headed.connecting && !headed.headless);
        assert!(!connect_context(&json!({})).headless);

        let set = PluginSet::new().with(LaunchOnly).unwrap();
        let error = SeleniumBaseError::from(set.check(&headed).unwrap_err());
        assert!(error.to_string().contains("attaches"), "{error}");
        assert!(set.check(&launch_context(false, false)).is_ok());
    }

    struct Headless;

    #[async_trait]
    impl Plugin for Headless {
        fn name(&self) -> &'static str {
            "headless"
        }

        async fn before_launch(&self, launch: &mut LaunchSettings) -> PluginResult<()> {
            launch.headless = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_plugin_cannot_make_a_launch_break_another_plugins_requirement() {
        let plugins = PluginSet::new()
            .with(Headless)
            .unwrap()
            .with(LaunchOnly)
            .unwrap();
        let options = options(LaunchOptions::builder().headless(false).plugins(plugins));

        let error = before_launch(options).await.unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "launch-only"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn without_plugins_the_options_are_kept_as_they_are() {
        let original = options(LaunchOptions::builder().arg("--mute-audio"));

        let options = before_launch(original.clone()).await.unwrap();

        assert_eq!(options.extra_args, original.extra_args);
        assert_eq!(options.headless, None);
    }

    /// The command-line switches that start with `prefix`.
    fn switches<'a>(args: &'a [String], prefix: &str) -> Vec<&'a str> {
        args.iter()
            .map(String::as_str)
            .filter(|arg| arg.starts_with(prefix))
            .collect()
    }

    fn args_of(options: &LaunchOptions) -> Vec<String> {
        options.browser_args(std::path::Path::new("/tmp/profile"))
    }

    #[test]
    fn plugin_settings_become_one_switch_each() {
        let mut options = options(LaunchOptions::builder());
        let mut settings = settings_of(&options);
        settings.host_resolver_rules = vec![
            "MAP a.example ^NOTFOUND".to_owned(),
            "MAP b.example ^NOTFOUND".to_owned(),
        ];
        settings.disable_features = vec!["OptimizationHints".to_owned(), "Translate".to_owned()];
        settings.enable_features = vec!["Foo".to_owned(), "Bar".to_owned(), "Foo".to_owned()];
        settings.disable_quic = true;

        apply_settings(&mut options, settings).unwrap();
        let args = args_of(&options);

        assert_eq!(
            switches(&args, "--host-resolver-rules"),
            ["--host-resolver-rules=MAP a.example ^NOTFOUND,MAP b.example ^NOTFOUND"]
        );
        assert_eq!(
            switches(&args, "--disable-features"),
            ["--disable-features=Translate,OptimizationHints"],
            "the library's own Translate and the plugin's features, once each, in one switch"
        );
        assert_eq!(
            switches(&args, "--enable-features"),
            ["--enable-features=Foo,Bar"]
        );
        assert_eq!(switches(&args, "--disable-quic"), ["--disable-quic"]);
    }

    #[test]
    fn a_switch_in_extra_args_joins_the_merged_one_instead_of_replacing_it() {
        let mut options = options(
            LaunchOptions::builder()
                .arg("--disable-features=UserFeature,Translate")
                .arg("--enable-features=UserOn")
                .arg("--host-resolver-rules=MAP user.example ^NOTFOUND")
                .arg("--force-webrtc-ip-handling-policy=default")
                .arg("--mute-audio"),
        );
        let mut settings = settings_of(&options);
        settings.host_resolver_rules = vec!["MAP a.example ^NOTFOUND".to_owned()];
        settings.disable_features = vec!["OptimizationHints".to_owned()];
        settings.enable_features = vec!["Foo".to_owned()];
        settings.webrtc_policy = Some(seleniumbase_rs_plugin::WebRtcPolicy::DisableNonProxiedUdp);

        apply_settings(&mut options, settings).unwrap();
        let args = args_of(&options);

        assert_eq!(
            switches(&args, "--disable-features"),
            ["--disable-features=Translate,OptimizationHints,UserFeature"]
        );
        assert_eq!(
            switches(&args, "--enable-features"),
            ["--enable-features=Foo,UserOn"]
        );
        assert_eq!(
            switches(&args, "--host-resolver-rules"),
            ["--host-resolver-rules=MAP a.example ^NOTFOUND,MAP user.example ^NOTFOUND"]
        );
        assert_eq!(
            switches(&args, "--force-webrtc-ip-handling-policy"),
            ["--force-webrtc-ip-handling-policy=disable_non_proxied_udp"],
            "the policy the options hold wins over a stray argument"
        );
        assert!(args.contains(&"--mute-audio".to_owned()), "{args:?}");
    }

    #[test]
    fn a_launch_no_plugin_touched_keeps_its_switches() {
        let args = args_of(&options(LaunchOptions::builder()));

        assert_eq!(
            switches(&args, "--disable-features"),
            ["--disable-features=Translate"]
        );
        for absent in [
            "--enable-features",
            "--host-resolver-rules",
            "--disable-quic",
            "--force-webrtc-ip-handling-policy",
        ] {
            assert!(switches(&args, absent).is_empty(), "{absent}: {args:?}");
        }
    }

    #[test]
    fn a_webrtc_policy_a_plugin_sets_reaches_its_chrome_flag() {
        use seleniumbase_rs_plugin::WebRtcPolicy as Api;

        let flag = |policy: Api| {
            let mut options = options(LaunchOptions::builder());
            let mut settings = settings_of(&options);
            settings.webrtc_policy = Some(policy);
            apply_settings(&mut options, settings).unwrap();
            switches(&args_of(&options), "--force-webrtc-ip-handling-policy")
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            flag(Api::DisableNonProxiedUdp),
            ["--force-webrtc-ip-handling-policy=disable_non_proxied_udp"]
        );
        assert_eq!(
            flag(Api::DefaultPublicInterfaceOnly),
            ["--force-webrtc-ip-handling-policy=default_public_interface_only"]
        );
        assert_eq!(
            flag(Api::DefaultPublicAndPrivateInterfaces),
            ["--force-webrtc-ip-handling-policy=default_public_and_private_interfaces"]
        );
        assert!(
            flag(Api::Default).is_empty(),
            "Chrome's own default is no switch"
        );
    }

    #[test]
    fn the_policy_the_options_already_have_is_shown_to_plugins_and_kept() {
        let mut options = options(
            LaunchOptions::builder()
                .webrtc_policy(crate::stealth::fingerprint::WebRtcPolicy::PublicInterfaceOnly),
        );

        let settings = settings_of(&options);
        assert_eq!(
            settings.webrtc_policy,
            Some(seleniumbase_rs_plugin::WebRtcPolicy::DefaultPublicInterfaceOnly)
        );
        apply_settings(&mut options, settings).unwrap();

        assert_eq!(
            options.webrtc,
            Some(crate::stealth::fingerprint::WebRtcPolicy::PublicInterfaceOnly)
        );
    }

    #[test]
    fn the_profile_plugins_see_is_persistent_when_the_options_name_a_folder() {
        let kept = options(LaunchOptions::builder().user_data_dir("/var/profile"));
        assert_eq!(
            settings_of(&kept).profile(),
            &ProfileView::Persistent(std::path::PathBuf::from("/var/profile"))
        );
        let fresh = options(LaunchOptions::builder());
        assert_eq!(settings_of(&fresh).profile(), &ProfileView::Ephemeral);
    }

    #[cfg(feature = "plugin-shield")]
    mod with_shield {
        use super::*;
        use crate::builtin_plugins::shield::{Level, Shield, ShieldConfig};

        fn shielded(config: ShieldConfig) -> PluginSet {
            PluginSet::strict()
                .with(Shield::new(config).unwrap())
                .unwrap()
        }

        #[tokio::test]
        async fn strict_shield_reaches_the_command_line_as_single_switches() {
            let plugins = shielded(ShieldConfig::new().level(Level::Strict));
            let launched = before_launch(options(
                LaunchOptions::builder().headless(true).plugins(plugins),
            ))
            .await
            .unwrap();

            let args = args_of(&launched);

            let rules = switches(&args, "--host-resolver-rules");
            assert_eq!(rules.len(), 1, "{rules:?}");
            assert!(
                rules[0].contains("MAP update.googleapis.com ^NOTFOUND,MAP clientservices"),
                "{rules:?}"
            );
            let disabled = switches(&args, "--disable-features");
            assert_eq!(
                disabled,
                ["--disable-features=Translate,OptimizationHints,MediaRouter"]
            );
            assert_eq!(
                switches(&args, "--force-webrtc-ip-handling-policy"),
                ["--force-webrtc-ip-handling-policy=disable_non_proxied_udp"]
            );
            assert!(args.contains(&"--disable-background-networking".to_owned()));
            assert_eq!(launched.prefs["net.network_prediction_options"], 2);
        }

        #[tokio::test]
        async fn shield_refuses_a_persistent_profile_because_the_options_show_it() {
            let kept = options(
                LaunchOptions::builder()
                    .headless(true)
                    .user_data_dir("/var/profile")
                    .plugins(shielded(ShieldConfig::new())),
            );

            let error = before_launch(kept).await.unwrap_err();

            assert!(
                matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "shield"),
                "{error:?}"
            );
            assert!(error.to_string().contains("persistent profile"), "{error}");

            let allowed = options(
                LaunchOptions::builder()
                    .headless(true)
                    .user_data_dir("/var/profile")
                    .plugins(shielded(ShieldConfig::new().ephemeral_profile(false))),
            );
            before_launch(allowed).await.unwrap();
        }
    }
}

#[cfg(test)]
mod navigation_origin_tests {
    use std::sync::{Arc, Mutex};

    use seleniumbase_rs_plugin::{async_trait, NavDecision, Plugin};

    use super::*;

    /// Writes down who asked for each navigation.
    struct Origins(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl Plugin for Origins {
        fn name(&self) -> &'static str {
            "origins"
        }

        async fn before_navigate(
            &self,
            _page: &dyn PageHandle,
            navigation: &Navigation,
        ) -> PluginResult<NavDecision> {
            self.0
                .lock()
                .unwrap()
                .push(format!("{:?} {}", navigation.origin, navigation.url));
            Ok(NavDecision::Continue)
        }
    }

    #[tokio::test]
    async fn the_origin_of_a_navigation_is_who_asked_for_it() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mock = crate::sb_cdp::mock::MockCtrl::new();
        let options = LaunchOptions::builder()
            .headless(true)
            .plugins(PluginSet::new().with(Origins(Arc::clone(&seen))).unwrap())
            .build()
            .unwrap();
        let browser = Browser::launch_mocked(options, &mock).await.unwrap();
        let page = browser.default_page().await.unwrap();

        page.goto("https://a.example/").await.unwrap();
        page.goto_from("https://b.example/", NavOrigin::Mcp)
            .await
            .unwrap();
        PageHandle::navigate(&page, "https://c.example/")
            .await
            .unwrap();

        assert_eq!(
            *seen.lock().unwrap(),
            [
                "Program https://a.example/",
                "Mcp https://b.example/",
                "Program https://c.example/",
            ]
        );
    }
}
