use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::api::run_settings::RunSettings;
#[cfg(feature = "plugins")]
use crate::browser::options::BrowserPlugins;
use crate::browser::options::{
    BrowserSettings, LaunchSwitch, SessionSettings, DEFAULT_WEBDRIVER_URL,
};
use crate::config::test_settings::TestData;

/// The folders of a comma-separated list, as Chrome takes them (`--load-extension`,
/// `SB_EXTENSION_DIR`): trimmed, without the empty items.
pub(crate) fn split_extension_dirs(text: &str) -> Vec<PathBuf> {
    text.split(',')
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The browser settings of a default [`BrowserConfig`]: the defaults of
/// [`BrowserSettings`], and headless.
fn default_browser_settings() -> BrowserSettings {
    BrowserSettings {
        headless: true,
        ..BrowserSettings::default()
    }
}

use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::Fingerprint;

mod builder;

pub use builder::BrowserConfigBuilder;

/// Supported browser types.
///
/// The names Python SeleniumBase uses for them (`--brave`, `--cft`, `--chs`,
/// `--ie`, ...) are read by [`FromStr`] and written by [`Display`].
///
/// [`FromStr`]: std::str::FromStr
/// [`Display`]: std::fmt::Display
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Hash)]
#[non_exhaustive]
pub enum Browser {
    #[default]
    Chrome,
    Chromium,
    Edge,
    Firefox,
    /// Brave, a Chromium browser run with chromedriver.
    Brave,
    /// Perplexity's Comet, a Chromium browser run with chromedriver.
    Comet,
    /// Opera, a Chromium browser run with operadriver.
    Opera,
    /// Safari, run with `safaridriver` (macOS only).
    Safari,
    /// Internet Explorer, run with `IEDriverServer` (Windows only).
    InternetExplorer,
    /// Chrome for Testing: Google's build of Chrome made for automation, run
    /// with chromedriver (`--cft`).
    ChromeForTesting,
    /// `chrome-headless-shell`, the headless-only Chrome, run with chromedriver
    /// (`--chs`). It cannot run the undetected (UC) mode.
    ChromeHeadlessShell,
    /// A WebDriver server that already runs the browser, such as a Selenium
    /// Grid or a cloud provider (`--browser=remote`). The session asks for
    /// nothing but the capabilities of
    /// [`capabilities`](BrowserConfigBuilder::capabilities) (`--cap-file`,
    /// `--cap-string`), so the other browser options (mobile mode, switches,
    /// window geometry) do not apply to it. It needs a server that is not the
    /// local default: see [`webdriver_url`](BrowserConfigBuilder::webdriver_url)
    /// and [`grid`](BrowserConfigBuilder::grid).
    Remote,
}

impl Browser {
    /// Whether the browser is Chromium-based, which is what the CDP and UC modes
    /// and the Chrome launch arguments need.
    #[must_use]
    pub const fn is_chromium_based(self) -> bool {
        !matches!(
            self,
            Self::Firefox | Self::Safari | Self::InternetExplorer | Self::Remote
        )
    }

    /// The name Python SeleniumBase gives the browser (`--browser=NAME`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
            Self::Firefox => "firefox",
            Self::Brave => "brave",
            Self::Comet => "comet",
            Self::Opera => "opera",
            Self::Safari => "safari",
            Self::InternetExplorer => "ie",
            Self::ChromeForTesting => "cft",
            Self::ChromeHeadlessShell => "chs",
            Self::Remote => "remote",
        }
    }
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Browser {
    type Err = SeleniumBaseError;

    /// Reads a Python `--browser` name, in any letter case. `ie` also answers
    /// to `internet explorer`, `cft` to `chrome-for-testing`, and `chs` to
    /// `chrome-headless-shell`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(match text.trim().to_ascii_lowercase().as_str() {
            "chrome" => Self::Chrome,
            "chromium" => Self::Chromium,
            "edge" => Self::Edge,
            "firefox" => Self::Firefox,
            "brave" => Self::Brave,
            "comet" => Self::Comet,
            "opera" => Self::Opera,
            "safari" => Self::Safari,
            "ie" | "internet explorer" | "internet-explorer" => Self::InternetExplorer,
            "cft" | "chrome-for-testing" => Self::ChromeForTesting,
            "chs" | "chrome-headless-shell" => Self::ChromeHeadlessShell,
            "remote" => Self::Remote,
            _ => {
                return Err(SeleniumBaseError::parse(
                    text,
                    "a browser: chrome, chromium, edge, firefox, brave, comet, opera, safari, ie, \
                     cft, chs or remote",
                ))
            }
        })
    }
}

/// Driver execution mode.
///
/// The names `webdriver`, `cdp` and `uc` are read by [`FromStr`] and written by
/// [`Display`].
///
/// [`FromStr`]: std::str::FromStr
/// [`Display`]: std::fmt::Display
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Hash)]
#[non_exhaustive]
pub enum DriverMode {
    // The names in a file are lower case; the capitalized names of an older file are read too.
    #[default]
    #[serde(alias = "WebDriver")]
    WebDriver,
    #[serde(alias = "Cdp")]
    Cdp,
    #[serde(alias = "Uc")]
    Uc,
}

impl DriverMode {
    /// The name `Settings::mode`, `SB_MODE` and `sbrs` use for the mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebDriver => "webdriver",
            Self::Cdp => "cdp",
            Self::Uc => "uc",
        }
    }
}

impl std::fmt::Display for DriverMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for DriverMode {
    type Err = SeleniumBaseError;

    /// Reads `webdriver`, `cdp` or `uc`, in any letter case.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "webdriver" => Ok(Self::WebDriver),
            "cdp" => Ok(Self::Cdp),
            "uc" => Ok(Self::Uc),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a driver mode: webdriver, cdp or uc",
            )),
        }
    }
}

/// Configuration for a browser session.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserConfig {
    /// What both engines start: the browser and its window, proxy, profile and switches.
    /// Set it with the [`BrowserConfigBuilder`] setters; a default configuration is headless.
    #[serde(default = "default_browser_settings")]
    pub browser: BrowserSettings,
    /// What only a WebDriver session reads: the driver mode, the WebDriver server, reuse,
    /// capabilities, the driver version and more.
    #[serde(default)]
    pub session: SessionSettings,
    /// The run-wide demo, slow, message and timeout settings a test starts with; see
    /// [`run_settings`](BrowserConfigBuilder::run_settings). They do not change the browser, so they
    /// are not part of a reuse key, and they are not written to, or read from, a configuration
    /// file.
    #[serde(skip)]
    pub run: RunSettings,
    /// The data a run hands to its tests (`--env`, `--account`, `--data`, `--var1..3`,
    /// `--variables`); see [`TestData`]. A `BaseCase` made from the configuration reads it.
    /// It does not change the browser and is not written to, or read from, a configuration
    /// file.
    #[serde(skip)]
    pub test_data: TestData,
    /// Optional anti-detection fingerprint profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,
    /// Attach the pure CDP engine to this HTTP or browser WebSocket URL.
    /// An authenticated Tauri plugin URL discovers its debugger via `/cdp`.
    /// `BaseCase` uses its `cdp_*` methods; no local browser or driver is started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdp_url: Option<String>,
    /// Discover CDP through the session's WebDriver URL (the authenticated Tauri plugin URL).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cdp_discovery: bool,
    /// The window `BaseCase` drives on a direct CDP attachment: a target id,
    /// or part of the window's URL. An app can own several windows and only
    /// the one that is picked is prepared (fingerprint, plugins); without a
    /// target it is the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdp_target: Option<String>,
    /// The browser plugins whose hooks run when a session starts (feature
    /// `plugins`, ADR 004); see [`plugins`](BrowserConfigBuilder::plugins). They are not written to,
    /// or read from, a configuration file.
    #[cfg(feature = "plugins")]
    #[serde(skip)]
    pub plugins: BrowserPlugins,
}

/// What Firefox is told when its proxy has a login: only Chromium browsers can
/// answer the proxy's challenge.
pub(crate) const FIREFOX_PROXY_LOGIN: &str =
    "Chrome or Edge is required when using a proxy server that has authentication! \
     (If using a proxy server without auth, Chrome, Edge, or Firefox may be used.)";

/// `timeout` in whole seconds, rounded up: the WebDriver client and
/// `BaseCase` take seconds.
pub(crate) fn whole_seconds(timeout: std::time::Duration) -> u64 {
    timeout.as_secs() + u64::from(timeout.subsec_nanos() > 0)
}

// Written by hand: the proxy, the PAC URL and the WebDriver server URL may carry
// a password or a cloud provider's access key, and `{config:?}` must not print
// them.
impl std::fmt::Debug for BrowserConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The discovery URL holds a token in its path, which `without_credentials` keeps.
        let mut session = self.session.clone();
        if self.cdp_discovery && session.webdriver_url.is_some() {
            session.webdriver_url = Some("<configured>".to_owned());
        }
        f.debug_struct("BrowserConfig")
            .field("browser", &self.browser)
            .field("session", &session)
            .field("run", &self.run)
            .field("test_data", &self.test_data)
            .field("fingerprint", &self.fingerprint)
            .field("cdp_url", &self.cdp_url.as_ref().map(|_| "<configured>"))
            .field("cdp_discovery", &self.cdp_discovery)
            .field("cdp_target", &self.cdp_target)
            .finish()
    }
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self::base()
    }
}

impl BrowserConfig {
    /// A builder that starts from the default configuration (headless, every other setting at
    /// its default); [`BrowserConfigBuilder::build`] returns the configuration.
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let config = BrowserConfig::builder().headless(false).build()?;
    /// assert!(!config.browser.headless);
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn builder() -> BrowserConfigBuilder {
        BrowserConfigBuilder::new(Self::default())
    }

    /// A builder that starts from this configuration, such as one that
    /// [`Settings::to_browser_config`](crate::Settings::to_browser_config) or
    /// [`from_env`](Self::from_env) returned: only what a setter is called for changes.
    ///
    /// ```
    /// use seleniumbase_rs::{BrowserConfig, DriverMode};
    ///
    /// let loaded = BrowserConfig::from_env()?;
    /// let config = loaded.into_builder().mode(DriverMode::Uc).build()?;
    /// assert_eq!(config.session.mode, Some(DriverMode::Uc));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn into_builder(self) -> BrowserConfigBuilder {
        BrowserConfigBuilder::new(self)
    }

    /// Build a `BrowserConfig` from the current process environment.
    ///
    /// This is the preferred constructor for deployable applications because
    /// it honors `SB_WEBDRIVER_URL`, `SB_CHROME_BIN` and `SB_REMOTE_WEBDRIVER_TIMEOUT`, which it
    /// reads as `Settings` reads them (a blank value is unset), without code changes per environment.
    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// [`from_env`](Self::from_env) reading the variables from `var`, which looks one up by
    /// name, instead of the process environment.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] for an `SB_REMOTE_WEBDRIVER_TIMEOUT` that is not a
    /// number of seconds.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, SeleniumBaseError> {
        let env = crate::config::env_vars::EnvVars::new("environment variable", var);
        let mut built = Self::base();
        built.session.webdriver_url = env
            .text("SB_WEBDRIVER_URL")
            .filter(|url| url != DEFAULT_WEBDRIVER_URL);
        built.browser.executable_path = env.text("SB_CHROME_BIN").map(std::path::PathBuf::from);
        built.session.remote_webdriver_timeout_seconds =
            env.parse::<u64>("SB_REMOTE_WEBDRIVER_TIMEOUT")?;
        Ok(built)
    }

    fn base() -> Self {
        Self {
            browser: default_browser_settings(),
            session: SessionSettings::default(),
            run: RunSettings::default(),
            test_data: TestData::default(),
            fingerprint: None,
            cdp_url: None,
            cdp_discovery: false,
            cdp_target: None,
            #[cfg(feature = "plugins")]
            plugins: BrowserPlugins::default(),
        }
    }
}

impl BrowserConfig {
    /// Checks the settings that can be refused before anything starts: the
    /// proxy and the PAC URL, a login on a Firefox proxy, and `--browser=remote`
    /// without capabilities or a server. A session checks them itself when it
    /// connects; call this to fail early. [`BrowserConfigBuilder::build`] does not
    /// call it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] naming the setting. The
    /// message hides a proxy's password.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let signed_in = BrowserConfig::builder().proxy("user:pw@proxy.test:8080").build()?;
    /// assert!(signed_in.validate().is_ok());
    /// let no_port = BrowserConfig::builder().proxy("user:pw@proxy.test").build()?;
    /// assert!(no_port.validate().is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        use crate::browser::options::{PacUrl, ProxyAddress};
        let proxy = self
            .browser
            .proxy
            .server
            .as_deref()
            .map(str::parse::<ProxyAddress>)
            .transpose()?;
        let pac = self
            .browser
            .proxy
            .pac_url
            .as_deref()
            .map(str::parse::<PacUrl>)
            .transpose()?;
        let has_login = proxy.is_some_and(|proxy| proxy.credentials().is_some())
            || pac.is_some_and(|pac| pac.credentials().is_some());
        if has_login && self.browser.kind == Browser::Firefox {
            return Err(SeleniumBaseError::invalid_config(FIREFOX_PROXY_LOGIN));
        }
        if self.browser.kind == Browser::Remote {
            if self.session.capabilities.is_empty() {
                return Err(SeleniumBaseError::invalid_config(
                    "--browser=remote needs --cap-file or --cap-string",
                ));
            }
            if self.is_default_webdriver_url() {
                return Err(SeleniumBaseError::invalid_config(
                    "--browser=remote needs a WebDriver server: --server, --webdriver or \
                     SB_WEBDRIVER_URL",
                ));
            }
        }
        Ok(())
    }

    pub fn is_cdp_enabled(&self) -> bool {
        self.cdp_url.is_some()
            || self.cdp_discovery
            || self.session.driver_mode() == DriverMode::Cdp
            || self.session.driver_mode() == DriverMode::Uc
    }

    pub fn is_uc_enabled(&self) -> bool {
        self.session.driver_mode() == DriverMode::Uc
    }

    pub fn is_default_webdriver_url(&self) -> bool {
        self.session.is_default_webdriver_url()
    }

    /// Whether pressing ESC in the browser ends the test
    /// ([`BrowserConfigBuilder::esc_end`]).
    #[must_use]
    pub fn esc_end(&self) -> bool {
        self.browser.has(LaunchSwitch::EscEnd)
    }

    /// Whether recordings are written as pure CDP code
    /// ([`BrowserConfigBuilder::recording_sb_cdp`]).
    #[must_use]
    pub fn recording_sb_cdp(&self) -> bool {
        self.browser.has(LaunchSwitch::RecSbCdp)
    }

    /// Whether Recorder Mode is on: `recorder_mode`, or any option that
    /// implies it (`rec_print`, `rec_behave`, `rec_sb_mgr`, `rec_sb_cdp`), as in
    /// Python.
    #[must_use]
    pub fn recorder_mode(&self) -> bool {
        [
            LaunchSwitch::RecorderMode,
            LaunchSwitch::RecPrint,
            LaunchSwitch::RecBehave,
            LaunchSwitch::RecSbMgr,
            LaunchSwitch::RecSbCdp,
        ]
        .into_iter()
        .any(|switch| self.browser.has(switch))
    }

    /// Whether the recording is printed ([`BrowserConfigBuilder::recording_print`]).
    #[must_use]
    pub fn rec_print(&self) -> bool {
        self.browser.has(LaunchSwitch::RecPrint)
    }

    /// Whether the recording is also written as a Gherkin feature
    /// ([`BrowserConfigBuilder::recording_behave`]).
    #[must_use]
    pub fn rec_behave(&self) -> bool {
        self.browser.has(LaunchSwitch::RecBehave)
    }

    /// Whether the recording is written as a standalone script
    /// ([`BrowserConfigBuilder::recording_sb_mgr`]).
    #[must_use]
    pub fn rec_sb_mgr(&self) -> bool {
        self.browser.has(LaunchSwitch::RecSbMgr)
    }

    /// The run-wide settings; see [`BrowserConfigBuilder::run_settings`].
    #[must_use]
    pub fn run_settings(&self) -> &RunSettings {
        &self.run
    }

    /// The browser plugins set with [`BrowserConfigBuilder::plugins`].
    #[cfg(feature = "plugins")]
    #[must_use]
    pub fn plugins(&self) -> &seleniumbase_rs_plugin::PluginSet {
        self.plugins.set()
    }

    /// Returns true when the configured fingerprint requests native-level
    /// (binary + CDP) spoofing.
    pub fn native_spoofing_enabled(&self) -> bool {
        self.fingerprint
            .as_ref()
            .is_some_and(|fp| fp.policy().native_spoofing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::options::{
        CapabilityOverrides, DeviceMetrics, PageLoadStrategy, VirtualDisplay, WindowPosition,
        WindowSize,
    };
    use crate::browser::reuse::ReuseClass;

    fn built(builder: BrowserConfigBuilder) -> BrowserConfig {
        builder.build().expect("a valid config")
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_unknown_plugin_name_is_refused_when_it_is_added() {
        let error = BrowserConfig::builder()
            .plugins_named(["no-such-plugin"])
            .build()
            .unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "no-such-plugin"),
            "{error:?}"
        );
        assert!(error.to_string().contains("known:"), "{error}");
    }

    #[cfg(feature = "plugin-shield")]
    #[test]
    fn named_plugins_join_the_plugins_of_the_configuration() {
        use crate::config::plugins::PluginSelection;
        use seleniumbase_rs_plugin::{async_trait, Plugin, PluginSet};

        struct Mine;

        #[async_trait]
        impl Plugin for Mine {
            fn name(&self) -> &str {
                "mine"
            }
        }

        let config = BrowserConfig::builder()
            .plugins(PluginSet::new().with(Mine).unwrap())
            .plugins_named(["shield"])
            .build()
            .unwrap();
        assert_eq!(config.plugins().names(), ["mine", "shield"]);

        let again = config
            .into_builder()
            .plugin_selection(&PluginSelection::new().enable("shield"))
            .build()
            .unwrap_err();
        assert!(again.to_string().contains("already in the set"), "{again}");
    }

    #[test]
    fn the_builders_write_into_the_group_that_owns_the_setting() {
        let config = BrowserConfig::builder()
            .proxy("proxy.test:8080")
            .proxy_pac_url("http://pac.test/p.pac")
            .user_agent("Agent/1")
            .locale("fr_FR")
            .start_page("https://example.com/")
            .user_data_dir("/tmp/profile")
            .extension_dir("/ext/a, /ext/b")
            .browser_binary_path("/opt/chrome")
            .mode(DriverMode::Uc)
            .reuse_session(true)
            .webdriver_url("http://hub.test:4444/wd/hub")
            .build()
            .expect("a valid config");
        let browser = &config.browser;
        assert_eq!(browser.proxy.server.as_deref(), Some("proxy.test:8080"));
        assert_eq!(
            browser.proxy.pac_url.as_deref(),
            Some("http://pac.test/p.pac")
        );
        assert_eq!(browser.user_agent.as_deref(), Some("Agent/1"));
        assert_eq!(browser.lang.as_deref(), Some("fr_FR"));
        assert_eq!(browser.url.as_deref(), Some("https://example.com/"));
        assert_eq!(browser.user_data_dir, Some(PathBuf::from("/tmp/profile")));
        assert_eq!(
            browser.extensions,
            [PathBuf::from("/ext/a"), PathBuf::from("/ext/b")]
        );
        assert_eq!(browser.executable_path, Some(PathBuf::from("/opt/chrome")));
        assert_eq!(config.session.mode, Some(DriverMode::Uc));
        assert!(config.session.reuse);
        assert_eq!(
            config.session.webdriver_url.as_deref(),
            Some("http://hub.test:4444/wd/hub")
        );
        assert!(!config.is_default_webdriver_url());
    }

    #[test]
    fn each_group_has_one_default_and_the_config_is_headless() {
        assert!(!BrowserSettings::default().headless, "Python's default");
        assert!(BrowserConfig::default().browser.headless);
        assert!(SessionSettings::default().auto_start_driver);
        assert_eq!(
            BrowserConfig::default().session.webdriver_url(),
            "http://localhost:4444"
        );
        let from_nothing: BrowserConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(from_nothing, BrowserConfig::default());
    }

    #[test]
    fn a_config_reads_its_groups_from_json() {
        let config: BrowserConfig = serde_json::from_str(
            r#"{
                "browser": {"kind": "Firefox", "headless": false,
                            "proxy": {"server": "p.test:1"},
                            "switches": {"dark-mode": true}},
                "session": {"mode": "Cdp", "grid": {"server": "hub.test", "port": 4445}},
                "cdp_target": "/settings"
            }"#,
        )
        .unwrap();
        assert_eq!(config.browser.kind, Browser::Firefox);
        assert!(!config.browser.headless);
        assert_eq!(config.browser.proxy.server.as_deref(), Some("p.test:1"));
        assert!(config.browser.has(LaunchSwitch::DarkMode));
        assert_eq!(config.session.mode, Some(DriverMode::Cdp));
        assert_eq!(
            config.session.webdriver_url(),
            "http://hub.test:4445/wd/hub"
        );
        assert_eq!(config.cdp_target.as_deref(), Some("/settings"));
    }

    #[test]
    fn a_config_in_the_old_flat_shape_is_refused_and_not_silently_defaulted() {
        let error = serde_json::from_str::<BrowserConfig>(
            r#"{"webdriver_url": "http://hub.test/wd/hub", "headless": false}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("unknown field"), "{error}");
    }

    #[test]

    fn the_debug_text_hides_every_login_and_the_discovery_url() {
        let config = BrowserConfig::builder()
            .proxy("user:PROXYPW@proxy.test:8080")
            .webdriver_url("https://USER:GRIDKEY@hub.test/wd/hub")
            .build()
            .expect("a valid config");
        let text = format!("{config:?}");
        assert!(!text.contains("PROXYPW"), "{text}");
        assert!(!text.contains("GRIDKEY"), "{text}");
        let discovery = BrowserConfig::builder()
            .webdriver_url("http://127.0.0.1:4445/TOKEN123/")
            .cdp_discovery(true)
            .build()
            .expect("a valid config");
        assert!(!format!("{discovery:?}").contains("TOKEN123"));
    }

    #[test]

    fn run_settings_are_set_by_a_builder_and_stay_out_of_config_files() {
        let demo = RunSettings::default().with_demo_mode(true);
        let config = BrowserConfig::builder()
            .run_settings(demo.clone())
            .build()
            .expect("a valid config");
        assert_eq!(config.run_settings(), &demo);
        assert_eq!(
            BrowserConfig::default().run_settings(),
            &RunSettings::default()
        );
        let text = serde_json::to_string(&config).unwrap();
        let back: BrowserConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.run_settings(), &RunSettings::default());
    }

    #[test]
    fn the_remote_timeout_builder_rounds_up_to_whole_seconds() {
        use std::time::Duration;
        let config = BrowserConfig::default();
        assert_eq!(config.session.remote_webdriver_timeout_seconds, None);
        let set = config
            .into_builder()
            .remote_webdriver_timeout(Duration::from_millis(2500))
            .build()
            .expect("a valid config");
        assert_eq!(set.session.remote_webdriver_timeout_seconds, Some(3));
        let exact = BrowserConfig::builder()
            .remote_webdriver_timeout(Duration::from_secs(600))
            .build()
            .expect("a valid config");
        assert_eq!(exact.session.remote_webdriver_timeout_seconds, Some(600));
    }

    #[test]
    fn a_driver_mode_is_read_from_its_name() {
        for (text, mode) in [
            ("webdriver", DriverMode::WebDriver),
            ("CDP", DriverMode::Cdp),
            (" uc ", DriverMode::Uc),
        ] {
            assert_eq!(text.parse::<DriverMode>().unwrap(), mode, "{text}");
        }
        assert!("selenium".parse::<DriverMode>().is_err());
        assert_eq!(DriverMode::Cdp.to_string(), "cdp");
    }

    #[test]
    fn reuse_is_off_until_asked_for() {
        let config = BrowserConfig::default();
        assert!(!config.session.reuse);
        assert_eq!(config.session.reuse_class, None);
    }

    #[test]
    fn a_class_session_is_a_reused_session_for_one_class() {
        let config = BrowserConfig::builder()
            .reuse_class_session("Login")
            .build()
            .expect("a valid config");
        assert!(config.session.reuse);
        assert_eq!(
            config.session.reuse_class,
            Some(ReuseClass::Named("Login".to_owned()))
        );
    }

    #[test]
    fn the_module_flag_turns_reuse_on_and_comes_off_alone() {
        let on = BrowserConfig::builder()
            .reuse_module_session(true)
            .build()
            .expect("a valid config");
        assert!(on.session.reuse);
        assert_eq!(on.session.reuse_class, Some(ReuseClass::TestModule));
        let off = on
            .into_builder()
            .reuse_module_session(false)
            .build()
            .expect("a valid config");
        assert!(off.session.reuse, "--rs stays on");
        assert_eq!(off.session.reuse_class, None);
        let named = BrowserConfig::builder()
            .reuse_class_session("Login")
            .reuse_module_session(false)
            .build()
            .expect("a valid config");
        assert_eq!(
            named.session.reuse_class,
            Some(ReuseClass::Named("Login".to_owned())),
            "a named class is not the module flag"
        );
    }

    #[test]
    fn the_reuse_class_round_trips_through_a_config_file() {
        let config = BrowserConfig::builder()
            .reuse_class_session("Login")
            .build()
            .expect("a valid config");
        let text = serde_json::to_string(&config).unwrap();
        assert!(
            text.contains(r#""reuse_class":{"named":"Login"}"#),
            "{text}"
        );
        let back: BrowserConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back, config);
        let module = BrowserConfig::builder()
            .reuse_module_session(true)
            .build()
            .expect("a valid config");
        let text = serde_json::to_string(&module).unwrap();
        assert!(text.contains(r#""reuse_class":"test_module""#), "{text}");
    }

    const ALL: [Browser; 12] = [
        Browser::Remote,
        Browser::Chrome,
        Browser::Chromium,
        Browser::Edge,
        Browser::Firefox,
        Browser::Brave,
        Browser::Comet,
        Browser::Opera,
        Browser::Safari,
        Browser::InternetExplorer,
        Browser::ChromeForTesting,
        Browser::ChromeHeadlessShell,
    ];

    #[test]
    fn every_browser_reads_back_from_its_python_name() {
        for browser in ALL {
            assert_eq!(
                browser.as_str().parse::<Browser>().unwrap(),
                browser,
                "{browser:?}"
            );
            assert_eq!(browser.to_string(), browser.as_str());
        }
        // The names Python SeleniumBase takes.
        for (name, browser) in [
            ("brave", Browser::Brave),
            ("comet", Browser::Comet),
            ("opera", Browser::Opera),
            ("safari", Browser::Safari),
            ("ie", Browser::InternetExplorer),
            ("cft", Browser::ChromeForTesting),
            ("chs", Browser::ChromeHeadlessShell),
            ("remote", Browser::Remote),
            ("Internet Explorer", Browser::InternetExplorer),
            (" CHROME ", Browser::Chrome),
        ] {
            assert_eq!(name.parse::<Browser>().unwrap(), browser, "{name}");
        }
        assert!("netscape".parse::<Browser>().is_err());
        assert!("".parse::<Browser>().is_err());
    }

    #[test]
    fn only_firefox_safari_ie_and_remote_are_not_chromium_based() {
        let others: Vec<_> = ALL.into_iter().filter(|b| !b.is_chromium_based()).collect();
        assert_eq!(
            others,
            [
                Browser::Remote,
                Browser::Firefox,
                Browser::Safari,
                Browser::InternetExplorer
            ]
        );
    }

    #[test]
    fn the_default_browser_is_chrome() {
        assert_eq!(Browser::default(), Browser::Chrome);
        assert_eq!(BrowserConfig::default().browser.kind, Browser::Chrome);
    }

    #[test]
    fn validate_refuses_a_bad_proxy_before_anything_starts() {
        assert!(BrowserConfig::default().validate().is_ok());
        assert!(built(BrowserConfig::builder().proxy("user:pw@h:8080"))
            .validate()
            .is_ok());
        assert!(
            built(BrowserConfig::builder().proxy_pac_url("user:pw@http://h/p.pac"))
                .validate()
                .is_ok()
        );

        let bad_proxy = built(BrowserConfig::builder().proxy("ftp://user:hunter2@h:21"))
            .validate()
            .unwrap_err()
            .to_string();
        assert!(bad_proxy.contains("is NOT in the expected"), "{bad_proxy}");
        assert!(!bad_proxy.contains("hunter2"), "{bad_proxy}");

        let bad_pac = built(BrowserConfig::builder().proxy_pac_url("http://h/p.js"))
            .validate()
            .unwrap_err()
            .to_string();
        assert!(
            bad_pac.contains("The proxy PAC URL must end with \".pac\""),
            "{bad_pac}"
        );
    }

    #[test]
    fn firefox_cannot_answer_a_proxy_login() {
        let firefox = built(BrowserConfig::builder().browser(Browser::Firefox));
        let error = built(firefox.clone().into_builder().proxy("user:pw@h:8080"))
            .validate()
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(
                "Chrome or Edge is required when using a proxy server that has authentication!"
            ),
            "{error}"
        );
        assert!(!error.contains("pw@"), "{error}");
        assert!(built(
            firefox
                .clone()
                .into_builder()
                .proxy_pac_url("user:pw@http://h/p.pac")
        )
        .validate()
        .is_err());
        assert!(built(firefox.into_builder().proxy("h:8080"))
            .validate()
            .is_ok());
        for browser in [Browser::Chrome, Browser::Edge, Browser::Brave] {
            let config = built(
                BrowserConfig::builder()
                    .browser(browser)
                    .proxy("user:pw@h:8080"),
            );
            assert!(config.validate().is_ok(), "{browser}");
        }
    }

    #[test]
    fn remote_needs_capabilities_and_a_server() {
        let remote = built(BrowserConfig::builder().browser(Browser::Remote));
        let error = remote.clone().validate().unwrap_err().to_string();
        assert!(
            error.contains("--browser=remote needs --cap-file or --cap-string"),
            "{error}"
        );
        let caps = CapabilityOverrides::from_text(r#"{"browserName":"chrome"}"#).unwrap();
        let with_caps = built(remote.into_builder().capabilities(caps));
        let error = with_caps.clone().validate().unwrap_err().to_string();
        assert!(
            error.contains("--browser=remote needs a WebDriver server"),
            "{error}"
        );
        let with_server = built(
            with_caps
                .into_builder()
                .webdriver_url("http://hub.example.com:4444/wd/hub"),
        );
        assert!(with_server.validate().is_ok());
    }

    #[test]
    fn a_grid_sets_the_server_and_localhost_keeps_the_local_driver() {
        use crate::browser::options::GridProtocol;
        let config = built(BrowserConfig::builder().grid(
            GridProtocol::Https,
            "USER:KEY@hub.example.com",
            443,
        ));
        assert_eq!(
            config.session.webdriver_url(),
            "https://USER:KEY@hub.example.com/wd/hub"
        );
        // The access key stays out of the debug output.
        assert!(!format!("{config:?}").contains("KEY"));
        let local = built(BrowserConfig::builder().grid(GridProtocol::Http, "localhost", 4444));
        assert!(local.is_default_webdriver_url());
        assert!(BrowserConfig::builder()
            .grid(GridProtocol::Http, " ", 4444)
            .build()
            .is_err());
    }

    #[test]
    fn a_binary_location_is_the_browser_binary_path() {
        let config = BrowserConfig::builder()
            .binary_location("/opt/chrome/chrome")
            .build()
            .expect("a valid config");
        assert_eq!(
            config.browser.executable_path,
            Some(PathBuf::from("/opt/chrome/chrome"))
        );
    }

    #[test]
    fn the_remote_debugger_may_move_only_when_the_default_port_was_asked_for() {
        let default = BrowserConfig::builder()
            .remote_debug(true)
            .build()
            .expect("a valid config");
        assert!(default.browser.debugging_any_port);
        let named = default
            .into_builder()
            .remote_debug_port(9333)
            .build()
            .expect("a valid config");
        assert!(!named.browser.debugging_any_port);
        assert_eq!(named.browser.debugging_port, Some(9333));
        let off = named
            .into_builder()
            .remote_debug(false)
            .build()
            .expect("a valid config");
        assert_eq!(off.browser.debugging_port, None);
        assert!(!off.browser.debugging_any_port);
    }

    #[test]
    fn a_default_config_writes_no_switches_and_a_set_one_sits_in_its_group() {
        let plain = serde_json::to_value(BrowserConfig::default()).unwrap();
        assert!(plain["browser"].get("switches").is_none(), "{plain}");
        assert!(
            plain["session"].get("page_load_strategy").is_none(),
            "{plain}"
        );

        let config = BrowserConfig::builder()
            .browser(Browser::Brave)
            .block_images(true)
            .page_load_strategy(PageLoadStrategy::Eager)
            .build()
            .expect("a valid config");
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["browser"]["kind"], "Brave");
        assert_eq!(
            json["browser"]["switches"],
            serde_json::json!({"block-images": true})
        );
        assert_eq!(json["session"]["page_load_strategy"], "eager");
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn a_config_written_without_browser_and_session_settings_still_reads() {
        let mut old = serde_json::to_value(BrowserConfig::default()).unwrap();
        old.as_object_mut().unwrap().remove("browser");
        old.as_object_mut().unwrap().remove("session");
        let config: BrowserConfig = serde_json::from_value(old).unwrap();
        assert_eq!(config.browser, default_browser_settings());
        assert_eq!(config.session, SessionSettings::default());
    }

    #[test]
    fn device_metrics_turn_mobile_on() {
        let config = BrowserConfig::builder()
            .device_metrics(DeviceMetrics::new(360, 640, 2.0))
            .build()
            .expect("a valid config");
        assert!(config.browser.mobile);
        assert_eq!(
            config.browser.device_metrics,
            Some(DeviceMetrics::new(360, 640, 2.0))
        );
    }

    #[test]
    fn the_debug_output_hides_a_proxy_password_and_a_server_key() {
        let config = BrowserConfig::builder()
            .proxy("socks5://user:secret@proxy.example:1080")
            .proxy_pac_url("http://pac-user:pac-secret@pac.example/p.pac")
            .webdriver_url("https://USER:ACCESSKEY@hub.example.com/wd/hub")
            .build()
            .expect("a valid config");
        let shown = format!("{config:?}");
        for secret in ["secret", "ACCESSKEY", "pac-user", "USER:"] {
            assert!(!shown.contains(secret), "{secret} is in {shown}");
        }
        assert!(shown.contains("socks5://proxy.example:1080"), "{shown}");
        assert!(shown.contains("https://hub.example.com/wd/hub"), "{shown}");
        // The proxy itself is untouched.
        assert_eq!(
            config.browser.proxy.server.as_deref(),
            Some("socks5://user:secret@proxy.example:1080")
        );
    }

    #[test]
    fn the_switch_builders_turn_their_switches_on_and_off() {
        let on = BrowserConfig::builder()
            .dark_mode(true)
            .guest_mode(true)
            .incognito(true)
            .devtools(true)
            .disable_web_security(true)
            .build()
            .expect("a valid config")
            .into_builder()
            .swiftshader(true)
            .enable_3d_apis(true)
            .maximize(true)
            .no_sandbox(true)
            .disable_gpu(true)
            .build()
            .expect("a valid config");
        for switch in [
            LaunchSwitch::DarkMode,
            LaunchSwitch::Guest,
            LaunchSwitch::Incognito,
            LaunchSwitch::Devtools,
            LaunchSwitch::DisableWebSecurity,
            LaunchSwitch::Swiftshader,
            LaunchSwitch::Enable3dApis,
            LaunchSwitch::Maximize,
            LaunchSwitch::NoSandbox,
            LaunchSwitch::DisableGpu,
        ] {
            assert!(on.browser.has(switch), "{switch:?}");
        }
        let off = on
            .into_builder()
            .dark_mode(false)
            .enable_web_security(true)
            .build()
            .expect("a valid config");
        assert!(!off.browser.has(LaunchSwitch::DarkMode));
        assert!(!off.browser.has(LaunchSwitch::DisableWebSecurity));
        assert!(off
            .into_builder()
            .enable_web_security(false)
            .build()
            .expect("a valid config")
            .browser
            .has(LaunchSwitch::DisableWebSecurity));
    }

    #[test]
    fn headless1_asks_for_a_headless_browser_and_headless2_is_headless() {
        let headed = BrowserConfig::builder()
            .headless(false)
            .build()
            .expect("a valid config");
        let one = headed
            .clone()
            .into_builder()
            .headless1(true)
            .build()
            .expect("a valid config");
        assert!(one.browser.headless && one.browser.has(LaunchSwitch::Headless1));
        let two = headed
            .into_builder()
            .headless2(true)
            .build()
            .expect("a valid config");
        assert!(two.browser.headless && !two.browser.has(LaunchSwitch::Headless1));
    }

    #[test]
    fn chromium_arg_text_adds_arguments_and_features_without_repeating_them() {
        let config = BrowserConfig::builder()
            .chromium_arg("mute-audio,disable-features=Translate")
            .chromium_arg("-ignore-gpu-blocklist,disable-features=Translate")
            .disable_features(["Foo", "Translate"])
            .host_resolver_rules("MAP a.test 127.0.0.1")
            .build()
            .expect("a valid config");
        assert_eq!(
            config.browser.chromium.args,
            ["--mute-audio", "--ignore-gpu-blocklist"]
        );
        assert_eq!(
            config.browser.chromium.disable_features,
            ["Translate", "Foo"]
        );
        assert_eq!(
            config.browser.chromium.host_resolver_rules.as_deref(),
            Some("MAP a.test 127.0.0.1")
        );
    }

    #[test]
    fn xvfb_is_the_default_virtual_display_or_none() {
        let on = BrowserConfig::builder()
            .xvfb(true)
            .build()
            .expect("a valid config");
        assert_eq!(on.browser.virtual_display, Some(VirtualDisplay::default()));
        let off = on
            .into_builder()
            .xvfb(false)
            .build()
            .expect("a valid config");
        assert_eq!(off.browser.virtual_display, None);
    }

    #[test]
    fn esc_end_is_off_until_it_is_asked_for() {
        assert!(!BrowserConfig::default().esc_end());
        let on = BrowserConfig::builder()
            .esc_end(true)
            .build()
            .expect("a valid config");
        assert!(on.esc_end());
        assert!(on.browser.has(LaunchSwitch::EscEnd));
        let off = on
            .into_builder()
            .esc_end(false)
            .build()
            .expect("a valid config");
        assert!(!off.esc_end());
    }

    #[test]
    fn rec_sb_cdp_is_off_until_it_is_asked_for_and_is_written_as_a_switch() {
        assert!(!BrowserConfig::default().recording_sb_cdp());
        let on = BrowserConfig::builder()
            .recording_sb_cdp(true)
            .build()
            .expect("a valid config");
        assert!(on.recording_sb_cdp());
        assert!(!on.esc_end(), "the switches are independent");
        let json = serde_json::to_value(&on).unwrap();
        assert_eq!(
            json["browser"]["switches"],
            serde_json::json!({"rec-sb-cdp": true})
        );
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert!(back.recording_sb_cdp());
        let off = on
            .into_builder()
            .recording_sb_cdp(false)
            .build()
            .expect("a valid config");
        assert!(!off.recording_sb_cdp());
    }

    #[test]
    fn recorder_mode_follows_its_switch_and_every_option_that_implies_it() {
        let off = BrowserConfig::default();
        assert!(!off.recorder_mode());
        let recording = off
            .clone()
            .into_builder()
            .recorder_mode(true)
            .build()
            .expect("a valid config");
        assert!(recording.recorder_mode());
        assert!(!recording.rec_print());
        for config in [
            off.clone()
                .into_builder()
                .recording_print(true)
                .build()
                .expect("a valid config"),
            off.clone()
                .into_builder()
                .recording_behave(true)
                .build()
                .expect("a valid config"),
            off.clone()
                .into_builder()
                .recording_sb_mgr(true)
                .build()
                .expect("a valid config"),
            off.clone()
                .into_builder()
                .recording_sb_cdp(true)
                .build()
                .expect("a valid config"),
        ] {
            assert!(config.recorder_mode());
        }
        let on = off
            .into_builder()
            .recording_print(true)
            .recording_behave(true)
            .build()
            .expect("a valid config");
        assert!(on.rec_print() && on.rec_behave() && !on.rec_sb_mgr());
        let json = serde_json::to_value(&on).unwrap();
        assert_eq!(
            json["browser"]["switches"],
            serde_json::json!({"rec-print": true, "rec-behave": true})
        );
    }

    #[test]

    fn esc_end_is_written_beside_the_other_switches() {
        let config = BrowserConfig::builder()
            .esc_end(true)
            .build()
            .expect("a valid config");
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json["browser"]["switches"],
            serde_json::json!({"esc-end": true})
        );
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert!(back.esc_end());
    }

    #[test]
    fn the_remote_debugger_has_a_default_port_and_can_be_closed() {
        let on = BrowserConfig::builder()
            .remote_debug(true)
            .build()
            .expect("a valid config");
        assert_eq!(on.browser.debugging_port, Some(9222));
        let off = on
            .into_builder()
            .remote_debug(false)
            .build()
            .expect("a valid config");
        assert_eq!(off.browser.debugging_port, None);
    }
}
