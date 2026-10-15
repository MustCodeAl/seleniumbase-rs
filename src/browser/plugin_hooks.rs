//! The plugin hooks of ADR 004 for the WebDriver engine (feature `plugins`).
//!
//! [`BrowserSession::connect`](crate::BrowserSession::connect) calls these
//! when its configuration carries plugins: [`before_launch`] checks the set
//! and lets the plugins change the start-up settings, which go back into the
//! [`BrowserConfig`] before the driver is asked for a browser.

use std::path::PathBuf;

use seleniumbase_rs_plugin::{BrowserFamily, Engine, LaunchContext, LaunchSettings};
use serde_json::{Map, Value};

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::options::{FirefoxOptions, FirefoxPreference};
use crate::error::SeleniumBaseError;
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
