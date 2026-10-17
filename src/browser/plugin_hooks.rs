//! The plugin hooks of ADR 004 for the WebDriver engine (feature `plugins`).
//!
//! [`BrowserSession::connect`](crate::BrowserSession::connect) calls these
//! when its configuration carries plugins: [`before_launch`] checks the set
//! and lets the plugins change the start-up settings, which go back into the
//! [`BrowserConfig`] before the driver is asked for a browser. Once the
//! session runs, [`SessionPlugins`] calls `on_browser`, gives each window the
//! packages and `on_page_created`, and calls `on_close` when it ends.
//!
//! Plugins see the browser and its windows through [`DriverBrowser`] and
//! [`DriverPage`]. On a Chromium browser they send protocol commands through
//! chromedriver's DevTools channel (`goog/cdp/execute`), which reaches the
//! window the session is on. Firefox and Safari have no such channel: `send`
//! is refused, and a script for every document runs now and again after each
//! navigation of the session, after the page's own scripts.

use std::path::PathBuf;
use std::sync::Mutex;

use seleniumbase_rs_plugin::{
    async_trait, BrowserFamily, BrowserHandle, Engine, LaunchContext, LaunchSettings, PageHandle,
    PluginError, PluginSet, Result as PluginResult,
};
use serde_json::{json, Map, Value};
use thirtyfour::WebDriver;

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::options::{FirefoxOptions, FirefoxPreference};
use crate::error::SeleniumBaseError;
use crate::sb_cdp::sync::locked;
use crate::stealth::cdp::CdpClient;
use crate::stealth::options::without_credentials;

/// The plugin API's family of `browser`.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for Internet Explorer, which is none of the
/// families plugins are written for.
pub(crate) fn family_of(browser: Browser) -> Result<BrowserFamily, SeleniumBaseError> {
    match browser {
        Browser::Chrome
        | Browser::Chromium
        | Browser::Edge
        | Browser::Brave
        | Browser::Comet
        | Browser::Opera
        | Browser::ChromeForTesting
        | Browser::ChromeHeadlessShell => Ok(BrowserFamily::Chromium),
        Browser::Firefox => Ok(BrowserFamily::Firefox),
        Browser::Safari => Ok(BrowserFamily::Safari),
        Browser::InternetExplorer => Err(SeleniumBaseError::plugin(
            "",
            "browser plugins do not run with Internet Explorer, which is neither a Chromium \
             browser nor Firefox nor Safari",
        )),
    }
}

/// Whether the browser of `config` runs without a window. Safari always has
/// one, whatever the setting says.
fn headless_of(config: &BrowserConfig, family: BrowserFamily) -> bool {
    config.headless && family != BrowserFamily::Safari
}

/// What the set checks its plugins against for `config`.
///
/// A session on a WebDriver server other than the local default counts as
/// attaching (`connecting`): this program does not start that browser itself.
pub(crate) fn launch_context(config: &BrowserConfig, family: BrowserFamily) -> LaunchContext {
    LaunchContext {
        engine: Engine::WebDriver,
        family,
        headless: headless_of(config, family),
        connecting: !config.is_default_webdriver_url(),
    }
}

/// The capability that holds the options of a Chromium browser: Edge has its
/// own, the others are driven as Chrome is.
fn options_key(browser: Browser) -> &'static str {
    if browser == Browser::Edge {
        "ms:edgeOptions"
    } else {
        "goog:chromeOptions"
    }
}

/// The preferences a Chromium configuration names in its capabilities
/// (`goog:chromeOptions.prefs`, or `ms:edgeOptions.prefs` for Edge).
fn chromium_prefs(config: &BrowserConfig) -> Map<String, Value> {
    config
        .launch
        .capabilities
        .get(options_key(config.browser))
        .and_then(|options| options.get("prefs"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// The folders of `extension_dir`, which Chrome takes comma-separated.
fn extension_dirs(config: &BrowserConfig) -> Vec<PathBuf> {
    config
        .extension_dir
        .as_deref()
        .map(|dirs| {
            dirs.split(',')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The start-up settings plugins may change, taken from `config`.
///
/// A Chromium browser shows its extra arguments, the preferences of its
/// capabilities, its extension folders and its proxy, the last as its server
/// only: a password in the configuration is not shown to plugins, and it stays
/// when they leave the proxy as it is. Firefox shows its own arguments and
/// preferences. Safari takes none of these at launch.
pub(crate) fn settings_of(config: &BrowserConfig, family: BrowserFamily) -> LaunchSettings {
    let mut settings = LaunchSettings::default();
    match family {
        BrowserFamily::Chromium => {
            settings.args.clone_from(&config.extra_args);
            settings.prefs = chromium_prefs(config);
            settings.extensions = extension_dirs(config);
            settings.proxy = config.proxy.as_deref().map(without_credentials);
        }
        BrowserFamily::Firefox => {
            settings.args = config.launch.firefox.args().to_vec();
            settings.prefs = config.launch.firefox.preferences_over(Map::new());
        }
        _ => {}
    }
    settings.headless = headless_of(config, family);
    settings
}

/// The refusal of a change the browser cannot take at launch.
fn cannot(browser: &str, what: &str) -> SeleniumBaseError {
    SeleniumBaseError::plugin(
        "",
        format!("a plugin's before_launch asked for what {browser} cannot do here: {what}"),
    )
}

/// Puts the settings the plugins returned back into `config`.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for a change the browser cannot take, and
/// [`SeleniumBaseError::InvalidConfig`] for a value it cannot be given.
pub(crate) fn apply_settings(
    config: &mut BrowserConfig,
    family: BrowserFamily,
    settings: LaunchSettings,
) -> Result<(), SeleniumBaseError> {
    let before = settings_of(config, family);
    let LaunchSettings {
        args,
        prefs,
        extensions,
        proxy,
        headless,
        ..
    } = settings;
    match family {
        BrowserFamily::Chromium => {
            config.extra_args = args;
            if prefs != before.prefs {
                let prefs = (!prefs.is_empty()).then_some(Value::Object(prefs));
                config
                    .launch
                    .capabilities
                    .set_field(options_key(config.browser), "prefs", prefs);
            }
            if extensions != before.extensions {
                config.extension_dir = extension_list(&extensions)?;
            }
            if proxy != before.proxy {
                config.proxy = proxy;
            }
        }
        BrowserFamily::Firefox => {
            if extensions != before.extensions {
                return Err(cannot(
                    "Firefox",
                    "load extension folders at launch; install add-ons with \
                     BaseCase::install_addon once the session runs",
                ));
            }
            if proxy != before.proxy {
                return Err(cannot("Firefox", "take a proxy from this engine"));
            }
            if args != before.args || prefs != before.prefs {
                config.launch.firefox = firefox_options(&args, prefs)?;
            }
        }
        _ => {
            if args != before.args
                || prefs != before.prefs
                || extensions != before.extensions
                || proxy != before.proxy
            {
                return Err(cannot(
                    "Safari",
                    "take arguments, preferences, extensions or a proxy at launch",
                ));
            }
        }
    }
    if headless != before.headless {
        if family == BrowserFamily::Safari {
            return Err(cannot("Safari", "run headless"));
        }
        config.headless = headless;
    }
    Ok(())
}

/// `extensions` as Chrome's comma-separated `--load-extension` list.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] for a folder whose path is not UTF-8,
/// or holds a comma, which would split it in two.
fn extension_list(extensions: &[PathBuf]) -> Result<Option<String>, SeleniumBaseError> {
    if extensions.is_empty() {
        return Ok(None);
    }
    let mut dirs = Vec::with_capacity(extensions.len());
    for path in extensions {
        let dir = path
            .to_str()
            .filter(|dir| !dir.is_empty() && !dir.contains(','))
            .ok_or_else(|| {
                SeleniumBaseError::invalid_config(format!(
                    "Chrome cannot be given the extension folder {} a plugin added: its path \
                     must be UTF-8 and must not hold a comma",
                    path.display()
                ))
            })?;
        dirs.push(dir);
    }
    Ok(Some(dirs.join(",")))
}

/// Firefox options with `args` and `prefs`.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] for an argument Firefox would split,
/// and for a preference that is not true or false, a whole number or text.
fn firefox_options(
    args: &[String],
    prefs: Map<String, Value>,
) -> Result<FirefoxOptions, SeleniumBaseError> {
    let mut firefox = FirefoxOptions::default();
    for arg in args {
        firefox.add_arg(arg).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "a plugin's Firefox argument {arg:?} cannot be used: {error}"
            ))
        })?;
    }
    for (name, value) in prefs {
        let value = match value {
            Value::Bool(flag) => Some(FirefoxPreference::Bool(flag)),
            Value::Number(number) => number.as_i64().map(FirefoxPreference::Int),
            Value::String(text) => Some(FirefoxPreference::Text(text)),
            _ => None,
        }
        .ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!(
                "the Firefox preference {name} a plugin set is neither true or false, a whole \
                 number, nor text, which is all Firefox takes"
            ))
        })?;
        firefox.set_preference(name, value);
    }
    Ok(firefox)
}

/// Checks the plugins of `config` and lets them change its start-up
/// settings; the set is checked again afterwards.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] when a plugin is refused or a hook fails,
/// [`SeleniumBaseError::InvalidConfig`] for plugins with `reuse_session`, and
/// what [`apply_settings`] returns.
pub(crate) async fn before_launch(
    mut config: BrowserConfig,
) -> Result<BrowserConfig, SeleniumBaseError> {
    if config.plugins().is_empty() {
        return Ok(config);
    }
    if config.reuse_session {
        return Err(SeleniumBaseError::invalid_config(
            "browser plugins cannot be used with reuse_session: a browser kept for reuse would \
             get the plugins' hooks a second time, or not at all. Turn one of them off.",
        ));
    }
    let plugins = config.plugins().clone();
    let family = family_of(config.browser)?;
    plugins.check(&launch_context(&config, family))?;
    let mut settings = settings_of(&config, family);
    plugins.before_launch(&mut settings).await?;
    apply_settings(&mut config, family, settings)?;
    plugins.check(&launch_context(&config, family))?;
    Ok(config)
}

/// Tells the plugins of `set` the browser closes, and logs the hooks that
/// fail: a session ends whatever its plugins say.
pub(crate) async fn close(set: &PluginSet) {
    for error in set.on_close().await {
        tracing::event!(
            name: "webdriver.plugin.close.failed",
            tracing::Level::WARN,
            error = %error,
            "a plugin failed while the browser closed",
        );
    }
}

/// The plugins of a running session, and the scripts they added for every
/// document, which Firefox and Safari run again after each navigation.
#[derive(Debug)]
pub(crate) struct SessionPlugins {
    set: PluginSet,
    family: BrowserFamily,
    scripts: Mutex<Vec<String>>,
}

impl SessionPlugins {
    pub(crate) fn new(set: PluginSet, family: BrowserFamily) -> Self {
        Self {
            set,
            family,
            scripts: Mutex::new(Vec::new()),
        }
    }

    /// Calls `on_browser`, then prepares the window the session is on.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for the first hook that fails.
    pub(crate) async fn start(&self, driver: &WebDriver) -> Result<(), SeleniumBaseError> {
        let browser = DriverBrowser {
            driver,
            family: self.family,
        };
        self.set.on_browser(&browser).await?;
        self.prepare_window(driver).await
    }

    /// Adds the packages to the window the session is on and calls
    /// `on_page_created` for it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for the first package or hook that fails.
    pub(crate) async fn prepare_window(&self, driver: &WebDriver) -> Result<(), SeleniumBaseError> {
        let page = DriverPage {
            driver,
            family: self.family,
            scripts: Mutex::new(Vec::new()),
        };
        let prepared = self.set.on_page_created(&page).await;
        let added = std::mem::take(&mut *locked(&page.scripts));
        let mut scripts = locked(&self.scripts);
        for script in added {
            // Another window gets the same scripts; each runs once per document.
            if !scripts.contains(&script) {
                scripts.push(script);
            }
        }
        Ok(prepared?)
    }

    /// Runs again, in the document the session just opened, the scripts the
    /// plugins added on a browser without the DevTools channel.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] when one of them throws.
    pub(crate) async fn after_navigation(
        &self,
        driver: &WebDriver,
    ) -> Result<(), SeleniumBaseError> {
        // Copied, so the lock is not held while the driver works.
        let scripts = locked(&self.scripts).clone();
        for script in &scripts {
            run_script(driver, script).await.map_err(|error| {
                SeleniumBaseError::plugin(
                    "",
                    format!(
                        "a script a plugin added for every document failed after the \
                         navigation: {error}"
                    ),
                )
            })?;
        }
        Ok(())
    }

    /// Calls `on_close`; see [`close`].
    pub(crate) async fn close(&self) {
        close(&self.set).await;
    }
}

/// The browser of a WebDriver session, as plugins see it.
#[derive(Debug)]
struct DriverBrowser<'a> {
    driver: &'a WebDriver,
    family: BrowserFamily,
}

#[async_trait]
impl BrowserHandle for DriverBrowser<'_> {
    fn engine(&self) -> Engine {
        Engine::WebDriver
    }

    fn family(&self) -> BrowserFamily {
        self.family
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        devtools(self.driver, self.family)?
            .execute_with_params(method, params)
            .await
            .map_err(|error| refused(&error))
    }
}

/// The window a WebDriver session is on, as plugins see it. On Firefox and
/// Safari it keeps the scripts added for every document, for the session to
/// run again after each navigation.
#[derive(Debug)]
struct DriverPage<'a> {
    driver: &'a WebDriver,
    family: BrowserFamily,
    scripts: Mutex<Vec<String>>,
}

#[async_trait]
impl PageHandle for DriverPage<'_> {
    fn engine(&self) -> Engine {
        Engine::WebDriver
    }

    async fn send(&self, method: &str, params: Value) -> PluginResult<Value> {
        devtools(self.driver, self.family)?
            .execute_with_params(method, params)
            .await
            .map_err(|error| refused(&error))
    }

    /// On Chromium, `Page.addScriptToEvaluateOnNewDocument` and
    /// `Runtime.evaluate` now, as on the pure CDP engine. On Firefox and
    /// Safari, a WebDriver script now, and again after each navigation of the
    /// session; it runs as a function body, so its top-level declarations stay
    /// local (assign to `window` to share a value).
    async fn add_init_script(&self, source: &str) -> PluginResult<()> {
        if self.family == BrowserFamily::Chromium {
            let cdp = devtools(self.driver, self.family)?;
            cdp.execute_with_params(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": source }),
            )
            .await
            .map_err(|error| refused(&error))?;
            // The document already loaded gets it too.
            let response = cdp
                .execute_with_params("Runtime.evaluate", json!({ "expression": source }))
                .await
                .map_err(|error| refused(&error))?;
            return match response.get("exceptionDetails") {
                Some(details) => Err(thrown(details)),
                None => Ok(()),
            };
        }
        run_script(self.driver, source)
            .await
            .map_err(|error| PluginError::other(format!("the script threw: {error}")))?;
        locked(&self.scripts).push(source.to_owned());
        Ok(())
    }

    /// A WebDriver script that returns `expression`, in the frame the session
    /// is on; a promise is waited for.
    async fn evaluate(&self, expression: &str) -> PluginResult<Value> {
        let expression = expression.trim().trim_end_matches(';');
        run_script(self.driver, &format!("return (\n{expression}\n);"))
            .await
            .map_err(|error| PluginError::other(format!("the script threw: {error}")))
    }
}

/// The DevTools channel of a Chromium session.
///
/// # Errors
///
/// For Firefox and Safari, which have none through WebDriver.
fn devtools(driver: &WebDriver, family: BrowserFamily) -> PluginResult<CdpClient> {
    if family == BrowserFamily::Chromium {
        return Ok(CdpClient::from_handle(driver.handle().clone()));
    }
    let name = match family {
        BrowserFamily::Firefox => "Firefox",
        BrowserFamily::Safari => "Safari",
        _ => "this browser",
    };
    Err(PluginError::other(format!(
        "{name} has no Chrome DevTools Protocol channel through WebDriver, so a plugin cannot \
         send it protocol commands"
    )))
}

/// Runs `script` as a WebDriver script and returns its value.
async fn run_script(driver: &WebDriver, script: &str) -> Result<Value, SeleniumBaseError> {
    let value = driver.execute(script, Vec::new()).await?;
    Ok(value.json().clone())
}

/// A protocol failure, as a plugin's hook sees it.
fn refused(error: &SeleniumBaseError) -> PluginError {
    PluginError::other(error.to_string())
}

/// The exception a `Runtime.evaluate` reported, as a plugin's hook sees it.
fn thrown(details: &Value) -> PluginError {
    let description = details["exception"]["description"]
        .as_str()
        .or_else(|| details["text"].as_str())
        .unwrap_or("script error");
    PluginError::other(format!("the script threw: {description}"))
}

#[cfg(test)]
mod conversion_tests {
    use std::sync::{Arc, Mutex};

    use seleniumbase_rs_plugin::{async_trait, Plugin, PluginSet, Requirements, Result};
    use serde_json::json;

    use super::*;
    use crate::browser::options::CapabilityOverrides;

    fn caps(text: &str) -> CapabilityOverrides {
        CapabilityOverrides::from_text(text).unwrap()
    }

    #[test]
    fn every_browser_has_a_family_but_internet_explorer() {
        for browser in [
            Browser::Chrome,
            Browser::Chromium,
            Browser::Edge,
            Browser::Brave,
            Browser::Comet,
            Browser::Opera,
            Browser::ChromeForTesting,
            Browser::ChromeHeadlessShell,
        ] {
            assert_eq!(
                family_of(browser).unwrap(),
                BrowserFamily::Chromium,
                "{browser}"
            );
        }
        assert_eq!(family_of(Browser::Firefox).unwrap(), BrowserFamily::Firefox);
        assert_eq!(family_of(Browser::Safari).unwrap(), BrowserFamily::Safari);
        assert!(matches!(
            family_of(Browser::InternetExplorer),
            Err(SeleniumBaseError::Plugin { .. })
        ));
    }

    #[test]
    fn the_context_names_the_engine_family_window_and_server() {
        let local = BrowserConfig::default();
        assert_eq!(
            launch_context(&local, BrowserFamily::Chromium),
            LaunchContext {
                engine: Engine::WebDriver,
                family: BrowserFamily::Chromium,
                headless: true,
                connecting: false,
            }
        );

        let grid = BrowserConfig::default()
            .with_headless(false)
            .with_webdriver_url("http://grid.example.com:4444/wd/hub");
        let context = launch_context(&grid, BrowserFamily::Chromium);
        assert!(context.connecting, "a remote server starts the browser");
        assert!(!context.headless);

        // Safari always has a window, whatever the headless setting says.
        let safari = BrowserConfig::default().with_browser(Browser::Safari);
        assert!(!launch_context(&safari, BrowserFamily::Safari).headless);
    }

    fn chrome() -> BrowserConfig {
        BrowserConfig::default()
            .with_extra_args(["--mute-audio"])
            .with_capabilities(caps(
                r#"{"goog:chromeOptions": {"prefs": {"intl.accept_languages": "de-DE"}}}"#,
            ))
            .with_extension_dir("/opt/ext/a,/opt/ext/b")
            .with_proxy("alice:s3cret@proxy.example.com:8080")
    }

    #[test]
    fn chromium_settings_come_from_the_config_without_the_proxy_password() {
        let settings = settings_of(&chrome(), BrowserFamily::Chromium);

        assert_eq!(settings.args, ["--mute-audio"]);
        assert_eq!(settings.prefs["intl.accept_languages"], "de-DE");
        assert_eq!(
            settings.extensions,
            [PathBuf::from("/opt/ext/a"), PathBuf::from("/opt/ext/b")]
        );
        assert_eq!(settings.proxy.as_deref(), Some("proxy.example.com:8080"));
        assert!(settings.headless);
    }

    #[test]
    fn settings_left_alone_change_nothing() {
        let firefox = {
            let mut config = BrowserConfig::default().with_browser(Browser::Firefox);
            config.launch.firefox.add_args("width=800").unwrap();
            config
                .launch
                .firefox
                .add_preferences("a.b:1,c.d:true")
                .unwrap();
            config
        };
        for (config, family) in [
            (chrome(), BrowserFamily::Chromium),
            (BrowserConfig::default(), BrowserFamily::Chromium),
            (firefox, BrowserFamily::Firefox),
            (
                BrowserConfig::default().with_browser(Browser::Safari),
                BrowserFamily::Safari,
            ),
        ] {
            let mut changed = config.clone();
            apply_settings(&mut changed, family, settings_of(&config, family)).unwrap();
            assert_eq!(changed, config, "{family:?}");
            assert_eq!(
                changed.proxy, config.proxy,
                "the password stays when the plugins keep the proxy"
            );
        }
    }

    #[test]
    fn chromium_settings_a_plugin_changed_reach_the_config() {
        let mut config = chrome();
        let mut settings = settings_of(&config, BrowserFamily::Chromium);
        settings.args.push("--lang=de".to_owned());
        settings
            .prefs
            .insert("webkit.webprefs.default_font_size".to_owned(), json!(22));
        settings.extensions.push(PathBuf::from("/opt/ext/c"));
        settings.proxy = Some("socks5://10.0.0.1:1080".to_owned());
        settings.headless = false;

        apply_settings(&mut config, BrowserFamily::Chromium, settings).unwrap();

        assert_eq!(config.extra_args, ["--mute-audio", "--lang=de"]);
        let prefs = &config
            .launch
            .capabilities
            .get("goog:chromeOptions")
            .unwrap()["prefs"];
        assert_eq!(prefs["intl.accept_languages"], "de-DE");
        assert_eq!(prefs["webkit.webprefs.default_font_size"], 22);
        assert_eq!(
            config.extension_dir.as_deref(),
            Some("/opt/ext/a,/opt/ext/b,/opt/ext/c")
        );
        assert_eq!(config.proxy.as_deref(), Some("socks5://10.0.0.1:1080"));
        assert!(!config.headless);
    }

    #[test]
    fn chromium_settings_a_plugin_removed_leave_the_config() {
        let mut config = chrome();
        let mut settings = settings_of(&config, BrowserFamily::Chromium);
        settings.prefs.clear();
        settings.extensions.clear();
        settings.proxy = None;

        apply_settings(&mut config, BrowserFamily::Chromium, settings).unwrap();

        assert!(
            config
                .launch
                .capabilities
                .get("goog:chromeOptions")
                .is_none(),
            "{:?}",
            config.launch.capabilities
        );
        assert_eq!(config.extension_dir, None);
        assert_eq!(config.proxy, None);
    }

    #[test]
    fn edge_preferences_go_to_its_own_options() {
        let mut config = BrowserConfig::default().with_browser(Browser::Edge);
        let mut settings = settings_of(&config, BrowserFamily::Chromium);
        settings
            .prefs
            .insert("intl.accept_languages".to_owned(), json!("fr"));

        apply_settings(&mut config, BrowserFamily::Chromium, settings).unwrap();

        assert_eq!(
            config.launch.capabilities.get("ms:edgeOptions").unwrap()["prefs"]
                ["intl.accept_languages"],
            "fr"
        );
        assert!(config
            .launch
            .capabilities
            .get("goog:chromeOptions")
            .is_none());
        assert_eq!(
            settings_of(&config, BrowserFamily::Chromium).prefs["intl.accept_languages"],
            "fr",
            "and are read back from there"
        );
    }

    #[test]
    fn an_extension_folder_chrome_cannot_be_given_is_refused() {
        let mut config = BrowserConfig::default();
        let mut settings = settings_of(&config, BrowserFamily::Chromium);
        settings.extensions.push(PathBuf::from("/opt/a,b"));

        let error = apply_settings(&mut config, BrowserFamily::Chromium, settings).unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(message) if message.contains("/opt/a,b")),
            "{error:?}"
        );
    }

    #[test]
    fn firefox_takes_arguments_and_preferences_as_its_own_options() {
        let mut config = BrowserConfig::default().with_browser(Browser::Firefox);
        config.launch.firefox.add_preferences("a.b:1").unwrap();
        let mut settings = settings_of(&config, BrowserFamily::Firefox);
        assert_eq!(settings.prefs["a.b"], 1);
        settings.args.push("--width=800".to_owned());
        settings
            .prefs
            .insert("intl.accept_languages".to_owned(), json!("de"));
        settings
            .prefs
            .insert("dom.webdriver.enabled".to_owned(), json!(false));

        apply_settings(&mut config, BrowserFamily::Firefox, settings).unwrap();

        assert_eq!(config.launch.firefox.args(), ["--width=800"]);
        let prefs = config.launch.firefox.preferences();
        assert_eq!(prefs["a.b"], FirefoxPreference::Int(1));
        assert_eq!(
            prefs["intl.accept_languages"],
            FirefoxPreference::Text("de".to_owned())
        );
        assert_eq!(
            prefs["dom.webdriver.enabled"],
            FirefoxPreference::Bool(false)
        );
    }

    #[test]
    fn a_firefox_preference_that_is_not_a_flag_number_or_text_is_refused() {
        let mut config = BrowserConfig::default().with_browser(Browser::Firefox);
        let mut settings = settings_of(&config, BrowserFamily::Firefox);
        settings
            .prefs
            .insert("layout.css.devPixelsPerPx".to_owned(), json!(1.5));

        let error = apply_settings(&mut config, BrowserFamily::Firefox, settings).unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(message) if message.contains("layout.css.devPixelsPerPx")),
            "{error:?}"
        );
    }

    #[test]
    fn what_firefox_and_safari_cannot_take_at_launch_is_refused() {
        let refused = |browser: Browser, family: BrowserFamily, change: fn(&mut LaunchSettings)| {
            let mut config = BrowserConfig::default().with_browser(browser);
            let mut settings = settings_of(&config, family);
            change(&mut settings);
            let error = apply_settings(&mut config, family, settings).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::Plugin { .. }),
                "{error:?}"
            );
            error.to_string()
        };

        let firefox = |change| refused(Browser::Firefox, BrowserFamily::Firefox, change);
        assert!(firefox(|s| s.extensions.push(PathBuf::from("/x"))).contains("install_addon"));
        assert!(firefox(|s| s.proxy = Some("proxy:8080".to_owned())).contains("proxy"));

        let safari = |change| refused(Browser::Safari, BrowserFamily::Safari, change);
        assert!(safari(|s| s.args.push("--x".to_owned())).contains("Safari"));
        assert!(safari(|s| {
            s.prefs.insert("a".to_owned(), json!(1));
        })
        .contains("Safari"));
        assert!(safari(|s| s.headless = true).contains("headless"));
    }

    /// Records the hooks that ran; adds `--lang=de` and may switch headless.
    struct Recorder {
        log: Arc<Mutex<Vec<String>>>,
        requirements: Requirements,
        headful: bool,
    }

    #[async_trait]
    impl Plugin for Recorder {
        fn name(&self) -> &str {
            "recorder"
        }

        fn requirements(&self) -> Requirements {
            self.requirements
        }

        async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
            self.log.lock().unwrap().push("before_launch".to_owned());
            launch.args.push("--lang=de".to_owned());
            if self.headful {
                launch.headless = false;
            }
            Ok(())
        }
    }

    fn recorder(requirements: Requirements, headful: bool) -> (PluginSet, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let plugin = Recorder {
            log: Arc::clone(&log),
            requirements,
            headful,
        };
        (PluginSet::new().with(plugin).unwrap(), log)
    }

    #[tokio::test]
    async fn before_launch_changes_the_config_the_session_starts_with() {
        let (plugins, log) = recorder(Requirements::empty(), true);
        let config = BrowserConfig::default().with_plugins(plugins);

        let config = before_launch(config).await.unwrap();

        assert_eq!(*log.lock().unwrap(), ["before_launch"]);
        assert_eq!(config.extra_args, ["--lang=de"]);
        assert!(!config.headless);
        assert_eq!(config.plugins().names(), ["recorder"], "the plugins stay");
    }

    #[tokio::test]
    async fn a_refused_plugin_runs_no_hook() {
        let (plugins, log) = recorder(Requirements::HEADFUL, false);
        let config = BrowserConfig::default().with_plugins(plugins);

        let error = before_launch(config).await.unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "recorder"),
            "{error:?}"
        );
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_remote_server_is_refused_to_a_launch_only_plugin() {
        let (plugins, _) = recorder(Requirements::LAUNCH_ONLY, false);
        let config = BrowserConfig::default()
            .with_webdriver_url("http://grid.example.com:4444")
            .with_plugins(plugins);

        let error = before_launch(config).await.unwrap_err();

        assert!(error.to_string().contains("attaches"), "{error}");
    }

    #[tokio::test]
    async fn plugins_and_reuse_session_are_refused_together() {
        let (plugins, log) = recorder(Requirements::empty(), false);
        let config = BrowserConfig::default()
            .with_reuse_session(true)
            .with_plugins(plugins);

        let error = before_launch(config).await.unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(message) if message.contains("reuse_session")),
            "{error:?}"
        );
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_config_without_plugins_is_returned_as_it_is() {
        let config = BrowserConfig::default()
            .with_reuse_session(true)
            .with_browser(Browser::InternetExplorer);

        assert_eq!(before_launch(config.clone()).await.unwrap(), config);
    }
}

/// The hooks of a running session, against the mock WebDriver of
/// `webdriver_support`: what reaches the driver, and in which order.
#[cfg(test)]
mod session_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use seleniumbase_rs_plugin::{
        async_trait, BrowserHandle, LaunchSettings, Package, PageHandle, Plugin, PluginError,
        PluginSet, Requirements, Result,
    };
    use serde_json::{json, Value};

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::session::BrowserSession;
    use crate::config::ad_block_list::{AdBlockList, MAX_URL_PATTERNS};
    use crate::BaseCase;

    type Log = Arc<Mutex<Vec<String>>>;

    fn entries(log: &Log) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    fn count(log: &Log, entry: &str) -> usize {
        entries(log).iter().filter(|seen| *seen == entry).count()
    }

    /// Notes each hook and what the handles answered; fails in the hook named
    /// by `fail_in`.
    struct Probe {
        log: Log,
        fail_in: Option<&'static str>,
    }

    impl Probe {
        fn note(&self, entry: impl Into<String>) {
            self.log.lock().unwrap().push(entry.into());
        }

        fn fail_if(&self, hook: &str) -> Result<()> {
            if self.fail_in == Some(hook) {
                Err(PluginError::other(format!("{hook} failed on purpose")))
            } else {
                Ok(())
            }
        }
    }

    #[async_trait]
    impl Plugin for Probe {
        fn name(&self) -> &str {
            "probe"
        }

        fn packages(&self) -> Vec<Package> {
            vec![Package::script("window.packaged = 1;").everywhere()]
        }

        async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
            self.note("before_launch");
            launch.args.push("--lang=de".to_owned());
            Ok(())
        }

        async fn on_browser(&self, browser: &dyn BrowserHandle) -> Result<()> {
            self.note(format!(
                "on_browser {:?} {:?}",
                browser.engine(),
                browser.family()
            ));
            match browser.send("Browser.getVersion", json!({})).await {
                Ok(_) => self.note("browser.send ok"),
                Err(error) => self.note(format!("browser.send refused: {error}")),
            }
            self.fail_if("on_browser")
        }

        async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
            self.note(format!("on_page_created {:?}", page.engine()));
            let timezone = json!({ "timezoneId": "Europe/Berlin" });
            match page.send("Emulation.setTimezoneOverride", timezone).await {
                Ok(_) => self.note("page.send ok"),
                Err(error) => self.note(format!("page.send refused: {error}")),
            }
            page.add_init_script("window.planet = 'Erde';").await?;
            let value = page.evaluate("6 * 7;").await?;
            self.note(format!("evaluate {value}"));
            self.fail_if("on_page_created")
        }

        async fn on_close(&self) -> Result<()> {
            self.note("on_close");
            self.fail_if("on_close")
        }
    }

    fn probe(fail_in: Option<&'static str>) -> (PluginSet, Log) {
        let log = Log::default();
        let plugin = Probe {
            log: Arc::clone(&log),
            fail_in,
        };
        (PluginSet::new().with(plugin).unwrap(), log)
    }

    /// A WebDriver that answers scripts with 42, opens a second window, and
    /// answers everything else, DevTools commands included, with `null`.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!(42)),
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "w2", "type": "tab" })),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn config_for(driver: &MockWebDriver, browser: Browser, plugins: PluginSet) -> BrowserConfig {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url())
            .with_browser(browser)
            .with_plugins(plugins);
        config.auto_start_driver = false;
        config
    }

    /// The DevTools commands the driver was sent, as `(method, params)`.
    fn devtools(driver: &MockWebDriver) -> Vec<(String, Value)> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| {
                let method = request.body["cmd"].as_str().unwrap_or_default().to_owned();
                (method, request.body["params"].clone())
            })
            .collect()
    }

    /// The scripts the driver was asked to run.
    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .into_iter()
            .map(|request| {
                request.body["script"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }

    fn mentioning(scripts: &[String], needle: &str) -> usize {
        scripts
            .iter()
            .filter(|script| script.contains(needle))
            .count()
    }

    #[tokio::test]
    async fn a_chromium_session_runs_every_hook_in_order_through_the_devtools_channel() {
        let driver = driver();
        let (plugins, log) = probe(None);

        let mut session = BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins))
            .await
            .unwrap();

        assert_eq!(
            entries(&log),
            [
                "before_launch",
                "on_browser WebDriver Chromium",
                "browser.send ok",
                "on_page_created WebDriver",
                "page.send ok",
                "evaluate 42",
            ]
        );
        let new_session = driver.requests("/session");
        assert!(
            new_session[0].body.to_string().contains("--lang=de"),
            "before_launch reached the capabilities: {}",
            new_session[0].body
        );
        let sent = devtools(&driver);
        let methods: Vec<&str> = sent.iter().map(|(method, _)| method.as_str()).collect();
        assert_eq!(
            methods,
            [
                "Browser.getVersion",
                "Page.addScriptToEvaluateOnNewDocument",
                "Runtime.evaluate",
                "Emulation.setTimezoneOverride",
                "Page.addScriptToEvaluateOnNewDocument",
                "Runtime.evaluate",
            ]
        );
        assert!(sent[1].1["source"]
            .as_str()
            .unwrap()
            .contains("window.packaged"));
        assert!(sent[2].1["expression"]
            .as_str()
            .unwrap()
            .contains("window.packaged"));
        assert_eq!(sent[4].1["source"], "window.planet = 'Erde';");
        assert_eq!(sent[5].1["expression"], "window.planet = 'Erde';");
        assert_eq!(
            scripts(&driver),
            ["return (\n6 * 7\n);"],
            "evaluate runs as a WebDriver script"
        );

        session.quit().await.unwrap();
        assert_eq!(entries(&log).last().map(String::as_str), Some("on_close"));
        assert_eq!(driver.requests("").len(), 1, "the browser was closed");
    }

    #[tokio::test]
    async fn firefox_runs_page_scripts_now_and_again_after_each_navigation() {
        let driver = driver();
        let (plugins, log) = probe(None);
        let mut session = BrowserSession::connect(config_for(&driver, Browser::Firefox, plugins))
            .await
            .unwrap();

        let seen = entries(&log);
        assert_eq!(seen[1], "on_browser WebDriver Firefox");
        assert!(
            seen[2].starts_with("browser.send refused:") && seen[2].contains("DevTools"),
            "{seen:?}"
        );
        assert!(seen[4].starts_with("page.send refused:"), "{seen:?}");
        assert!(
            devtools(&driver).is_empty(),
            "Firefox has no DevTools channel"
        );
        let ran = scripts(&driver);
        assert_eq!(mentioning(&ran, "window.packaged"), 1, "{ran:?}");
        assert_eq!(mentioning(&ran, "window.planet"), 1, "{ran:?}");

        session.goto("https://example.com/").await.unwrap();

        let ran = scripts(&driver);
        assert_eq!(
            mentioning(&ran, "window.packaged"),
            2,
            "again after goto: {ran:?}"
        );
        assert_eq!(mentioning(&ran, "window.planet"), 2, "{ran:?}");
        session.quit().await.unwrap();
        assert_eq!(count(&log, "on_close"), 1);
    }

    #[tokio::test]
    async fn base_case_runs_the_hooks_and_its_open_runs_the_scripts_again() {
        let driver = driver();
        let (plugins, log) = probe(None);

        let mut sb = BaseCase::new(config_for(&driver, Browser::Firefox, plugins))
            .await
            .unwrap();
        sb.open("https://example.com/").await.unwrap();
        sb.quit().await.unwrap();

        assert_eq!(entries(&log)[1], "on_browser WebDriver Firefox");
        assert_eq!(mentioning(&scripts(&driver), "window.planet"), 2);
        assert_eq!(count(&log, "on_close"), 1);
    }

    #[tokio::test]
    async fn a_failing_hook_stops_the_session_and_closes_the_browser() {
        for hook in ["on_browser", "on_page_created"] {
            let driver = driver();
            let (plugins, log) = probe(Some(hook));

            let error = BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins))
                .await
                .err()
                .unwrap();

            assert!(
                matches!(&error, SeleniumBaseError::Plugin { plugin, reason } if plugin == "probe" && reason.contains(hook)),
                "{error:?}"
            );
            assert_eq!(
                driver.requests("").len(),
                1,
                "{hook}: the browser was closed"
            );
            assert_eq!(count(&log, "on_close"), 1, "{hook}: {:?}", entries(&log));
        }
    }

    #[tokio::test]
    async fn a_failure_after_before_launch_tells_the_plugins_the_launch_ended() {
        let driver = driver();
        let (plugins, log) = probe(None);
        let hosts: Vec<String> = (0..=MAX_URL_PATTERNS / 2)
            .map(|i| format!("host{i}.com"))
            .collect();
        let config = config_for(&driver, Browser::Chrome, plugins)
            .with_ad_block_list(AdBlockList::from_hosts(hosts).unwrap());

        let connected = BrowserSession::connect(config).await;

        assert!(matches!(
            connected,
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert_eq!(entries(&log), ["before_launch", "on_close"]);
    }

    /// Refuses a headless browser.
    struct Headful;

    #[async_trait]
    impl Plugin for Headful {
        fn name(&self) -> &str {
            "headful"
        }

        fn requirements(&self) -> Requirements {
            Requirements::HEADFUL
        }
    }

    #[tokio::test]
    async fn a_refused_plugin_starts_no_browser() {
        let driver = driver();
        let plugins = PluginSet::new().with(Headful).unwrap();

        let connected =
            BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins)).await;

        assert!(matches!(connected, Err(SeleniumBaseError::Plugin { .. })));
        assert!(
            driver.requests("/session").is_empty(),
            "no browser was asked for"
        );
    }

    #[tokio::test]
    async fn every_window_the_session_opens_gets_the_page_hooks() {
        let driver = driver();
        let (plugins, log) = probe(None);
        let mut session = BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins))
            .await
            .unwrap();

        session.switch_to_new_window().await.unwrap();
        assert_eq!(count(&log, "on_page_created WebDriver"), 2);

        session.apply_plugins_to_window().await.unwrap();
        assert_eq!(
            count(&log, "on_page_created WebDriver"),
            3,
            "a window reached some other way gets them on request"
        );
        session.quit().await.unwrap();
    }

    #[tokio::test]
    async fn base_case_gives_the_windows_it_opens_the_page_hooks() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!(42)),
                ("GET", "/window/handles") => Reply::Value(json!(["w1", "w2"])),
                _ => Reply::Value(Value::Null),
            }
        });
        let (plugins, log) = probe(None);
        let mut sb = BaseCase::new(config_for(&driver, Browser::Chrome, plugins))
            .await
            .unwrap();
        let created = |log: &Log| count(log, "on_page_created WebDriver");
        assert_eq!(created(&log), 1, "the first window");

        sb.open_new_window().await.unwrap();
        assert_eq!(created(&log), 2, "open_new_window: {:?}", entries(&log));
        sb.open_new_tab_url("https://example.com/").await.unwrap();
        assert_eq!(created(&log), 3, "open_new_tab_url: {:?}", entries(&log));

        // A window the page opened: switching to it runs no hook, asking does.
        sb.switch_to_newest_window().await.unwrap();
        assert_eq!(created(&log), 3);
        sb.apply_plugins_to_window().await.unwrap();
        assert_eq!(created(&log), 4);
        sb.quit().await.unwrap();
    }

    #[tokio::test]
    async fn on_close_failures_are_logged_and_the_session_still_quits() {
        let driver = driver();
        let (plugins, log) = probe(Some("on_close"));
        let mut session = BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins))
            .await
            .unwrap();

        session.quit().await.unwrap();

        assert_eq!(count(&log, "on_close"), 1);
        assert_eq!(driver.requests("").len(), 1);
    }

    #[tokio::test]
    async fn a_reconnected_session_runs_the_hooks_for_its_new_browser() {
        let driver = driver();
        let (plugins, log) = probe(None);
        let config = config_for(&driver, Browser::Chrome, plugins);
        let mut session = BrowserSession::connect(config.clone()).await.unwrap();

        session.reconnect(&config).await.unwrap();

        let seen = entries(&log);
        assert_eq!(count(&log, "before_launch"), 2, "{seen:?}");
        assert_eq!(count(&log, "on_browser WebDriver Chromium"), 2, "{seen:?}");
        assert_eq!(count(&log, "on_close"), 1, "{seen:?}");
        let close = seen.iter().position(|entry| entry == "on_close").unwrap();
        let second_launch = seen
            .iter()
            .rposition(|entry| entry == "before_launch")
            .unwrap();
        let second_browser = seen
            .iter()
            .rposition(|entry| entry == "on_browser WebDriver Chromium")
            .unwrap();
        assert!(
            second_launch < close && close < second_browser,
            "the plugins are checked for the new browser first, so a refusal changes nothing; \
             then the old browser's plugins hear it close, then the new one starts: {seen:?}"
        );
        assert_eq!(
            driver.requests("/session")[1]
                .body
                .to_string()
                .matches("--lang=de")
                .count(),
            driver.requests("/session")[0]
                .body
                .to_string()
                .matches("--lang=de")
                .count(),
            "before_launch starts again from the configuration, not from its last result"
        );
        session.quit().await.unwrap();
        assert_eq!(count(&log, "on_close"), 2);
    }

    #[tokio::test]
    async fn a_session_dropped_without_quit_still_tells_the_plugins() {
        let driver = driver();
        let (plugins, log) = probe(None);
        let session = BrowserSession::connect(config_for(&driver, Browser::Chrome, plugins))
            .await
            .unwrap();

        drop(session);

        for _ in 0..100 {
            if count(&log, "on_close") == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(count(&log, "on_close"), 1, "{:?}", entries(&log));
    }
}
