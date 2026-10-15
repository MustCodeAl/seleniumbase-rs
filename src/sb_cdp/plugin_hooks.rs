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

use seleniumbase_rs_plugin::{
    async_trait, BrowserFamily, BrowserHandle, Engine, LaunchContext, LaunchSettings, PageHandle,
    PluginError, PluginSet, Result as PluginResult,
};
use serde_json::{json, Value};

use super::client::Client;
use super::launch::{LaunchOptions, Proxy};
use super::page::Page;
use crate::error::SeleniumBaseError;

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

/// The start-up settings a plugin may change, taken from `options`.
///
/// The proxy is given as its server only: a password in the options is not
/// shown to plugins, and it stays when they leave the proxy as it is.
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
}

/// Puts the settings the plugins returned back into `options`.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when a plugin set a proxy that cannot
/// be parsed.
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
        ..
    } = settings;
    options.extra_args = args;
    options.prefs = prefs;
    options.extensions = extensions;
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
    let plugins = options.plugins.clone();
    plugins.check(&launch_context(options.is_headless(), false))?;
    let mut settings = settings_of(&options);
    plugins.before_launch(&mut settings).await?;
    apply_settings(&mut options, settings)?;
    plugins.check(&launch_context(options.is_headless(), false))?;
    Ok(options)
}

/// Tells the plugins the browser behind `client` runs.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first hook that fails.
pub(super) async fn on_browser(
    plugins: &PluginSet,
    client: &Client,
) -> Result<(), SeleniumBaseError> {
    if plugins.is_empty() {
        return Ok(());
    }
    Ok(plugins.on_browser(&BrowserLink { client }).await?)
}

/// Adds the plugins' packages to the tab attached as `session` and calls
/// their `on_page_created`.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for the first package or hook that fails.
pub(super) async fn on_page_created(
    plugins: &PluginSet,
    client: &Client,
    session: &str,
) -> Result<(), SeleniumBaseError> {
    if plugins.is_empty() {
        return Ok(());
    }
    Ok(plugins
        .on_page_created(&TabLink { client, session })
        .await?)
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

/// A protocol failure, as a plugin's hook sees it.
fn refused(error: &SeleniumBaseError) -> PluginError {
    PluginError::other(error.to_string())
}

/// The browser, as plugins see it: browser-level commands on its connection.
#[derive(Debug)]
struct BrowserLink<'a> {
    client: &'a Client,
}

#[async_trait]
impl BrowserHandle for BrowserLink<'_> {
    fn engine(&self) -> Engine {
        Engine::Cdp
    }

    fn family(&self) -> BrowserFamily {
        BrowserFamily::Chromium
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        self.client
            .send(method, params, None)
            .await
            .map_err(|error| refused(&error))
    }
}

/// One tab, as plugins see it: commands on the protocol session attached to
/// it.
#[derive(Debug)]
struct TabLink<'a> {
    client: &'a Client,
    session: &'a str,
}

impl TabLink<'_> {
    async fn command(&self, method: &str, params: Value) -> PluginResult<Value> {
        self.client
            .send(method, params, Some(self.session))
            .await
            .map_err(|error| refused(&error))
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
/// protocol session.
///
/// The trait's `evaluate` and `add_init_script` differ from the methods of
/// the same names on [`Page`]: `add_init_script` here also runs the script in
/// the current document, as the plugin API asks, and returns no identifier.
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
}

impl Page {
    /// This tab as the hooks see a tab.
    fn tab_link(&self) -> TabLink<'_> {
        TabLink {
            client: &self.browser().inner().client,
            session: self.session_id(),
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
}
