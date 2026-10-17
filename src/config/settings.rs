use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::run_settings::RunSettings;
use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::browser::options::{LaunchSettings, LaunchSwitch, WindowSize};
use crate::browser::reuse::ReuseClass;
use crate::config::plugins::{PluginSelection, PLUGINS_ENV};
use crate::error::SeleniumBaseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub browser: String,
    pub headless: bool,
    /// The default wait, in whole seconds, of a [`BaseCase`](crate::BaseCase)
    /// made from [`to_browser_config`](Self::to_browser_config)
    /// (`SB_TIMEOUT`); see [`RunSettings::with_default_timeout`]. `None` keeps
    /// the `BaseCase` default of 10 seconds.
    pub timeout_seconds: Option<u64>,
    /// Where a person wants screenshots (`SB_SCREENSHOT_DIR`). The library
    /// reads it nowhere: `BaseCase` saves screenshots under its logs folder, and
    /// a program that wants this folder passes it to the call that saves one.
    pub screenshot_dir: String,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    /// The browser window's width in pixels (`SB_WINDOW_WIDTH`). The window
    /// size applies when this and `window_height` are both set; unset, the
    /// browser keeps its own default size.
    pub window_width: Option<u32>,
    /// The browser window's height in pixels (`SB_WINDOW_HEIGHT`); see
    /// `window_width`.
    pub window_height: Option<u32>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub locale: Option<String>,
    pub user_agent: Option<String>,
    pub mode: Option<String>,
    pub ad_block: bool,
    /// Share browsers among the tests of a run (`SB_REUSE_SESSION`, Python's
    /// `--reuse-session`). See [`BrowserConfig::reuse_session`].
    pub reuse_session: bool,
    /// Share a browser among the tests of one Rust module
    /// (`SB_REUSE_CLASS_SESSION`, Python's `--rcs`). It implies
    /// `reuse_session`. See [`BrowserConfig::with_reuse_module_session`].
    pub reuse_class_session: bool,
    pub mobile: bool,
    pub threads: Option<usize>,
    /// Let the person end the current test by pressing ESC in the browser
    /// (`SB_ESC_END`, Python's `--esc-end`). See
    /// [`BrowserConfig::with_esc_end`].
    pub esc_end: bool,
    /// Write recordings as pure CDP code (`SB_REC_SB_CDP`, Python's
    /// `--rec-sb-cdp`). See [`BrowserConfig::with_rec_sb_cdp`].
    pub rec_sb_cdp: bool,
    /// Client-side timeout, in seconds, for each HTTP request sent to a Remote
    /// WebDriver server. `None` keeps the WebDriver client's own default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_webdriver_timeout_seconds: Option<u64>,
    /// The WebDriver server (`SB_WEBDRIVER_URL`). `None` is
    /// `http://localhost:4444`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webdriver_url: Option<String>,
    /// The browser binary to run (`SB_CHROME_BIN`). `None` finds the system
    /// one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome_bin: Option<PathBuf>,
    /// The browser plugins to turn on by name (the `[plugins]` table, and
    /// `SB_PLUGINS`, which replaces `enable`). A build without the feature
    /// `plugins` refuses a non-empty selection when it loads the settings. Turn
    /// it into plugins with `BrowserConfig::with_plugin_selection` or
    /// `LaunchOptionsBuilder::plugin_selection` (feature `plugins`).
    #[serde(default, skip_serializing_if = "PluginSelection::is_empty")]
    pub plugins: PluginSelection,
}

/// The settings a command line asked for explicitly, and nothing else.
///
/// Each field is `None` unless the user passed the matching option. That
/// distinction matters: an option left at its default must not hide the value
/// from a config file or an `SB_*` variable, but an option that was passed
/// beats both. The same overrides can be applied to a [`Settings`] directly
/// ([`apply`](Self::apply)) or handed to a child process, such as a test run,
/// as environment variables ([`env`](Self::env)).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
///
/// let overrides = SettingsOverrides {
///     headless: Some(true),
///     proxy: Some("proxy.example.com:8080".to_owned()),
///     ..SettingsOverrides::default()
/// };
///
/// let mut settings = Settings::default();
/// overrides.apply(&mut settings);
/// assert!(settings.headless);
/// assert_eq!(settings.proxy.as_deref(), Some("proxy.example.com:8080"));
///
/// let env = overrides.env();
/// assert!(env.contains(&("SB_HEADLESS", "true".to_owned())));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsOverrides {
    /// Browser name (`SB_BROWSER`).
    pub browser: Option<String>,
    /// Run without a visible window (`SB_HEADLESS`).
    pub headless: Option<bool>,
    /// Driver mode: `webdriver`, `cdp` or `uc` (`SB_MODE`).
    pub mode: Option<String>,
    /// User-agent string (`SB_USER_AGENT`).
    pub user_agent: Option<String>,
    /// Browser locale (`SB_LOCALE`).
    pub locale: Option<String>,
    /// Block ads (`SB_AD_BLOCK`).
    pub ad_block: Option<bool>,
    /// Proxy address (`SB_PROXY`).
    pub proxy: Option<String>,
    /// Proxy auto-config URL (`SB_PROXY_PAC_URL`).
    pub proxy_pac_url: Option<String>,
    /// Persistent profile directory (`SB_USER_DATA_DIR`).
    pub user_data_dir: Option<String>,
    /// Unpacked extension directory (`SB_EXTENSION_DIR`).
    pub extension_dir: Option<String>,
    /// Share browsers among the tests of a run (`SB_REUSE_SESSION`).
    pub reuse_session: Option<bool>,
    /// Share a browser among the tests of one module
    /// (`SB_REUSE_CLASS_SESSION`).
    pub reuse_class_session: Option<bool>,
    /// Emulate a mobile device (`SB_MOBILE`).
    pub mobile: Option<bool>,
    /// Number of parallel threads (`SB_THREADS`).
    pub threads: Option<usize>,
    /// End the current test when ESC is pressed in the browser
    /// (`SB_ESC_END`).
    pub esc_end: Option<bool>,
    /// Write recordings as pure CDP code (`SB_REC_SB_CDP`).
    pub rec_sb_cdp: Option<bool>,
}

impl SettingsOverrides {
    /// Writes every option that is set into `settings`.
    pub fn apply(&self, settings: &mut Settings) {
        if let Some(browser) = &self.browser {
            settings.browser.clone_from(browser);
        }
        if let Some(headless) = self.headless {
            settings.headless = headless;
        }
        if let Some(mode) = &self.mode {
            settings.mode = Some(mode.clone());
        }
        if let Some(user_agent) = &self.user_agent {
            settings.user_agent = Some(user_agent.clone());
        }
        if let Some(locale) = &self.locale {
            settings.locale = Some(locale.clone());
        }
        if let Some(ad_block) = self.ad_block {
            settings.ad_block = ad_block;
        }
        if let Some(proxy) = &self.proxy {
            settings.proxy = Some(proxy.clone());
        }
        if let Some(pac) = &self.proxy_pac_url {
            settings.proxy_pac_url = Some(pac.clone());
        }
        if let Some(dir) = &self.user_data_dir {
            settings.user_data_dir = Some(dir.clone());
        }
        if let Some(dir) = &self.extension_dir {
            settings.extension_dir = Some(dir.clone());
        }
        if let Some(reuse) = self.reuse_session {
            settings.reuse_session = reuse;
        }
        if let Some(reuse) = self.reuse_class_session {
            settings.reuse_class_session = reuse;
        }
        if let Some(mobile) = self.mobile {
            settings.mobile = mobile;
        }
        if let Some(threads) = self.threads {
            settings.threads = Some(threads);
        }
        if let Some(esc_end) = self.esc_end {
            settings.esc_end = esc_end;
        }
        if let Some(rec_sb_cdp) = self.rec_sb_cdp {
            settings.rec_sb_cdp = rec_sb_cdp;
        }
    }

    /// The `SB_*` environment variables that carry these options to a child
    /// process, one for each option that is set.
    ///
    /// A process that loads its settings with [`Settings::load`] sees them as
    /// the highest-priority overrides it can receive from outside. A proxy
    /// address may contain a password, so hand the result only to processes
    /// you trust with it.
    #[must_use]
    pub fn env(&self) -> Vec<(&'static str, String)> {
        let text = |name, value: &Option<String>| value.clone().map(|value| (name, value));
        let flag = |name, value: Option<bool>| value.map(|value| (name, value.to_string()));
        [
            text("SB_BROWSER", &self.browser),
            flag("SB_HEADLESS", self.headless),
            text("SB_MODE", &self.mode),
            text("SB_USER_AGENT", &self.user_agent),
            text("SB_LOCALE", &self.locale),
            flag("SB_AD_BLOCK", self.ad_block),
            text("SB_PROXY", &self.proxy),
            text("SB_PROXY_PAC_URL", &self.proxy_pac_url),
            text("SB_USER_DATA_DIR", &self.user_data_dir),
            text("SB_EXTENSION_DIR", &self.extension_dir),
            flag("SB_REUSE_SESSION", self.reuse_session),
            flag("SB_REUSE_CLASS_SESSION", self.reuse_class_session),
            flag("SB_MOBILE", self.mobile),
            self.threads
                .map(|threads| ("SB_THREADS", threads.to_string())),
            flag("SB_ESC_END", self.esc_end),
            flag("SB_REC_SB_CDP", self.rec_sb_cdp),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            browser: "chrome".to_owned(),
            headless: false,
            timeout_seconds: None,
            screenshot_dir: "screenshots".to_owned(),
            proxy: None,
            proxy_pac_url: None,
            window_width: None,
            window_height: None,
            user_data_dir: None,
            extension_dir: None,
            locale: None,
            user_agent: None,
            mode: None,
            ad_block: false,
            reuse_session: false,
            reuse_class_session: false,
            mobile: false,
            threads: None,
            esc_end: false,
            rec_sb_cdp: false,
            remote_webdriver_timeout_seconds: None,
            webdriver_url: None,
            chrome_bin: None,
            plugins: PluginSelection::default(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let settings = Self::parse_file(path)?;
        settings.plugins.ensure_supported()?;
        Ok(settings)
    }

    /// Reads a settings file without checking that this build can run the
    /// plugins it names, so that [`load`](Self::load) can let `SB_PLUGINS`
    /// replace them first and check the result.
    fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}"))
        })?;
        let settings: Self = if path.extension().is_some_and(|ext| ext == "toml") {
            toml::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
            })?
        } else {
            serde_json::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
            })?
        };
        settings.validate()?;
        Ok(settings)
    }

    /// Checks the values that name something: the browser, the driver mode and
    /// the window size.
    ///
    /// [`from_file`](Self::from_file), [`load`](Self::load) and
    /// [`from_env`](Self::from_env) call it, so a misspelt `browser` or `mode`
    /// is an error there instead of a silent Chrome in WebDriver mode. Call it
    /// yourself on a `Settings` built by hand, before
    /// [`to_browser_config`](Self::to_browser_config).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for an unknown browser or
    /// mode, and for a window width without a height (or the other way round)
    /// or one of zero pixels.
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        self.browser
            .parse::<Browser>()
            .map_err(|e| SeleniumBaseError::InvalidConfig(format!("browser: {e}")))?;
        if let Some(mode) = &self.mode {
            mode.parse::<DriverMode>()
                .map_err(|e| SeleniumBaseError::InvalidConfig(format!("mode: {e}")))?;
        }
        match (self.window_width, self.window_height) {
            (None, None) => {}
            (Some(width), Some(height)) if width > 0 && height > 0 => {}
            other => {
                return Err(SeleniumBaseError::InvalidConfig(format!(
                    "window_width and window_height must both be set, and above zero: {other:?}"
                )));
            }
        }
        Ok(())
    }

    #[deprecated(
        since = "0.5.0",
        note = "use Settings::from_file, which reads TOML for a .toml name and JSON otherwise"
    )]
    pub fn from_toml<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read TOML settings: {e}"))
        })?;
        let settings: Self = toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })?;
        settings.plugins.ensure_supported()?;
        Ok(settings)
    }

    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        let settings = Self::default();
        Self::apply_env_overrides(settings)
    }

    /// Loads the global config file if one exists.
    ///
    /// Searches, in order: `sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`.
    pub fn load_global() -> Result<Self, SeleniumBaseError> {
        let settings = Self::parse_global()?;
        settings.plugins.ensure_supported()?;
        Ok(settings)
    }

    /// [`load_global`](Self::load_global) without the plugin support check.
    fn parse_global() -> Result<Self, SeleniumBaseError> {
        let candidates = ["sbase_config.toml", ".sbase_config.toml", ".sbase_config"];
        for candidate in &candidates {
            if Path::new(candidate).is_file() {
                return Self::parse_file(candidate);
            }
        }
        Ok(Self::default())
    }

    /// Loads the global config file if one exists ([`load_global`](Self::load_global)),
    /// then applies the `SB_*` environment variables on top.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the file cannot be read
    /// or parsed, or a variable or the result is not valid.
    pub fn load() -> Result<Self, SeleniumBaseError> {
        Self::load_with(None::<&Path>, |name| std::env::var(name).ok())
    }

    /// Loads the settings file `path` (TOML for a `.toml` name, JSON
    /// otherwise), then applies the `SB_*` environment variables on top.
    /// [`from_file`](Self::from_file) alone does not read the environment.
    ///
    /// # Errors
    ///
    /// As [`load`](Self::load).
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        Self::load_with(Some(path), |name| std::env::var(name).ok())
    }

    /// [`load`](Self::load) and [`load_from`](Self::load_from) with the
    /// overrides taken from `var`.
    fn load_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SeleniumBaseError> {
        // The plugin support check waits for the overrides, which may replace
        // the file's plugins (`SB_PLUGINS=` turns them off) before it runs.
        let settings = match path {
            Some(p) => Self::parse_file(p)?,
            None => Self::parse_global()?,
        };
        Self::apply_overrides(settings, var)
    }

    /// Converts these settings into a `BrowserConfig` used by `BaseCase`.
    ///
    /// Settings that came from a file or the environment are checked already.
    /// For one built by hand, call [`validate`](Self::validate) first: an
    /// unknown browser or mode becomes Chrome or WebDriver here.
    pub fn to_browser_config(&self) -> BrowserConfig {
        let browser = self.browser.parse::<Browser>().unwrap_or_default();
        let mode = self
            .mode
            .as_deref()
            .and_then(|mode| mode.parse::<DriverMode>().ok())
            .unwrap_or_default();
        let mut launch = LaunchSettings::default();
        if let (Some(width), Some(height)) = (self.window_width, self.window_height) {
            launch.window_size = Some(WindowSize::new(width, height));
        }
        if let Some(seconds) = self.timeout_seconds {
            launch.run = RunSettings::default().with_default_timeout(Duration::from_secs(seconds));
        }
        launch.set(LaunchSwitch::EscEnd, self.esc_end);
        launch.set(LaunchSwitch::RecSbCdp, self.rec_sb_cdp);
        launch.reuse_class = self.reuse_class_session.then_some(ReuseClass::TestModule);
        BrowserConfig {
            webdriver_url: self
                .webdriver_url
                .clone()
                .unwrap_or_else(|| crate::config::RuntimeConfig::default().webdriver_url),
            browser,
            headless: self.headless,
            mode,
            user_agent: self.user_agent.clone(),
            locale: self.locale.clone(),
            ad_block: self.ad_block,
            proxy: self.proxy.clone(),
            proxy_pac_url: self.proxy_pac_url.clone(),
            user_data_dir: self.user_data_dir.clone(),
            extension_dir: self.extension_dir.clone(),
            reuse_session: self.reuse_session || self.reuse_class_session,
            mobile: self.mobile,
            threads: self.threads,
            auto_start_driver: true,
            start_page: None,
            extra_args: Vec::new(),
            fingerprint: None,
            browser_binary_path: self.chrome_bin.clone(),
            remote_webdriver_timeout_seconds: self.remote_webdriver_timeout_seconds,
            launch,
        }
    }

    fn apply_env_overrides(settings: Self) -> Result<Self, SeleniumBaseError> {
        Self::apply_overrides(settings, |name| std::env::var(name).ok())
    }

    /// Applies the `SB_*` overrides that `var` reports on top of `settings`.
    ///
    /// `var` looks a variable up by name, so the overrides can come from the
    /// process environment or from any other source. A blank value (empty or
    /// whitespace only, as a CI matrix exports for an unset entry) counts as
    /// unset, as `SB_LOG_LEVEL` does in `RuntimeConfig`; `SB_PLUGINS` is the
    /// exception, where an empty value is an answer: no plugin at all.
    fn apply_overrides(
        mut settings: Self,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SeleniumBaseError> {
        let plugins_variable = var(PLUGINS_ENV);
        let var = |name: &str| var(name).filter(|value| !value.trim().is_empty());
        if let Some(v) = var("SB_BROWSER") {
            settings.browser = v;
        }
        if let Some(v) = var("SB_HEADLESS") {
            settings.headless = parse_bool(&v)?;
        }
        if let Some(v) = var("SB_TIMEOUT") {
            settings.timeout_seconds = Some(
                v.parse()
                    .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_TIMEOUT: {e}")))?,
            );
        }
        if let Some(v) = var("SB_WEBDRIVER_URL") {
            settings.webdriver_url = Some(v);
        }
        if let Some(v) = var("SB_CHROME_BIN") {
            settings.chrome_bin = Some(PathBuf::from(v));
        }
        if let Some(v) = var("SB_REMOTE_WEBDRIVER_TIMEOUT") {
            settings.remote_webdriver_timeout_seconds = Some(v.parse().map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("SB_REMOTE_WEBDRIVER_TIMEOUT: {e}"))
            })?);
        }
        if let Some(v) = var("SB_SCREENSHOT_DIR") {
            settings.screenshot_dir = v;
        }
        if let Some(v) = var("SB_PROXY") {
            settings.proxy = Some(v);
        }
        if let Some(v) = var("SB_WINDOW_WIDTH") {
            settings.window_width =
                Some(v.parse().map_err(|e| {
                    SeleniumBaseError::InvalidConfig(format!("SB_WINDOW_WIDTH: {e}"))
                })?);
        }
        if let Some(v) = var("SB_WINDOW_HEIGHT") {
            settings.window_height =
                Some(v.parse().map_err(|e| {
                    SeleniumBaseError::InvalidConfig(format!("SB_WINDOW_HEIGHT: {e}"))
                })?);
        }
        if let Some(v) = var("SB_USER_DATA_DIR") {
            settings.user_data_dir = Some(v);
        }
        if let Some(v) = var("SB_EXTENSION_DIR") {
            settings.extension_dir = Some(v);
        }
        if let Some(v) = var("SB_LOCALE") {
            settings.locale = Some(v);
        }
        if let Some(v) = var("SB_USER_AGENT") {
            settings.user_agent = Some(v);
        }
        if let Some(v) = var("SB_MODE") {
            settings.mode = Some(v);
        }
        if let Some(v) = var("SB_AD_BLOCK") {
            settings.ad_block = parse_bool(&v)?;
        }
        if let Some(v) = var("SB_REUSE_SESSION") {
            settings.reuse_session = parse_bool(&v)?;
        }
        if let Some(v) = var("SB_REUSE_CLASS_SESSION") {
            settings.reuse_class_session = parse_bool(&v)?;
        }
        if let Some(v) = var("SB_MOBILE") {
            settings.mobile = parse_bool(&v)?;
        }
        if let Some(v) = var("SB_THREADS") {
            settings.threads = Some(
                v.parse()
                    .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_THREADS: {e}")))?,
            );
        }
        if let Some(v) = var("SB_PROXY_PAC_URL") {
            settings.proxy_pac_url = Some(v);
        }
        if let Some(v) = var("SB_ESC_END") {
            settings.esc_end = parse_bool(&v)?;
        }
        if let Some(v) = plugins_variable {
            // The variable names the plugins; the options of the file stay.
            settings.plugins.set_enabled_from_list(&v);
        }
        if let Some(v) = var("SB_REC_SB_CDP") {
            settings.rec_sb_cdp = parse_bool(&v)?;
        }
        settings.validate()?;
        // Plugins that cannot run are an error here, not a silent omission.
        settings.plugins.ensure_supported()?;
        Ok(settings)
    }
}

fn parse_bool(value: &str) -> Result<bool, SeleniumBaseError> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "cannot parse bool: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_settings_are_sensible() {
        let s = Settings::default();
        assert_eq!(s.browser, "chrome");
        assert!(!s.headless);
        assert_eq!(s.timeout_seconds, None);
        assert_eq!(s.screenshot_dir, "screenshots");
        assert_eq!((s.window_width, s.window_height), (None, None));
        assert!(s.proxy.is_none());
    }

    fn with_vars(
        pairs: &'static [(&'static str, &'static str)],
    ) -> Result<Settings, SeleniumBaseError> {
        Settings::apply_overrides(Settings::default(), |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn the_webdriver_url_and_the_browser_binary_reach_the_browser_config() {
        let defaults = Settings::default().to_browser_config();
        assert_eq!(defaults.webdriver_url, "http://localhost:4444");
        assert_eq!(defaults.browser_binary_path, None);

        let config = with_vars(&[
            ("SB_WEBDRIVER_URL", "http://grid.test:4444"),
            ("SB_CHROME_BIN", "/opt/chrome/chrome"),
        ])
        .unwrap()
        .to_browser_config();
        assert_eq!(config.webdriver_url, "http://grid.test:4444");
        assert_eq!(
            config.browser_binary_path,
            Some(PathBuf::from("/opt/chrome/chrome"))
        );
    }

    #[test]
    fn the_window_size_reaches_the_launch_settings_when_both_sides_are_set() {
        assert_eq!(
            Settings::default().to_browser_config().launch.window_size,
            None
        );
        let config = with_vars(&[("SB_WINDOW_WIDTH", "1280"), ("SB_WINDOW_HEIGHT", "720")])
            .unwrap()
            .to_browser_config();
        assert_eq!(config.launch.window_size, Some(WindowSize::new(1280, 720)));
    }

    #[test]
    fn a_lone_or_empty_window_dimension_is_an_error() {
        assert!(with_vars(&[("SB_WINDOW_WIDTH", "1280")]).is_err());
        assert!(with_vars(&[("SB_WINDOW_WIDTH", "0"), ("SB_WINDOW_HEIGHT", "720")]).is_err());
    }

    #[test]
    fn the_timeout_reaches_the_run_settings() {
        assert_eq!(
            Settings::default()
                .to_browser_config()
                .run_settings()
                .default_timeout(),
            None
        );
        let config = with_vars(&[("SB_TIMEOUT", "5")])
            .unwrap()
            .to_browser_config();
        assert_eq!(
            config.run_settings().default_timeout(),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn a_blank_variable_counts_as_unset() {
        let settings = with_vars(&[
            ("SB_MODE", ""),
            ("SB_BROWSER", "  "),
            ("SB_CHROME_BIN", ""),
            ("SB_WEBDRIVER_URL", ""),
            ("SB_TIMEOUT", ""),
        ])
        .unwrap();
        assert_eq!(settings.mode, None);
        assert_eq!(settings.browser, "chrome");
        assert_eq!(settings.chrome_bin, None);
        assert_eq!(settings.webdriver_url, None);
        assert_eq!(settings.timeout_seconds, None);
    }

    #[test]
    fn an_unknown_browser_or_mode_is_an_error_not_chrome() {
        let browser = with_vars(&[("SB_BROWSER", "chrom")]).unwrap_err();
        assert!(browser.to_string().contains("chrom"), "{browser}");
        let mode = with_vars(&[("SB_MODE", "stealth")]).unwrap_err();
        assert!(mode.to_string().contains("stealth"), "{mode}");
        assert!(with_vars(&[("SB_BROWSER", "Firefox"), ("SB_MODE", "UC")]).is_ok());

        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"chrom\"\n").unwrap();
        assert!(Settings::from_file(file.path()).is_err());
    }

    #[test]
    fn remote_webdriver_timeout_defaults_to_unset() {
        // SeleniumBase's REMOTE_WEBDRIVER_TIMEOUT defaults to None, leaving the
        // client's own timeout in place.
        assert!(Settings::default()
            .remote_webdriver_timeout_seconds
            .is_none());
        assert!(Settings::default()
            .to_browser_config()
            .remote_webdriver_timeout_seconds
            .is_none());
    }

    #[test]
    fn remote_webdriver_timeout_reaches_the_browser_config() {
        let settings = Settings {
            remote_webdriver_timeout_seconds: Some(420),
            ..Settings::default()
        };
        assert_eq!(
            settings
                .to_browser_config()
                .remote_webdriver_timeout_seconds,
            Some(420)
        );
    }

    #[test]
    fn remote_webdriver_timeout_round_trips_through_json() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(br#"{"remote_webdriver_timeout_seconds":600}"#)
            .unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.remote_webdriver_timeout_seconds, Some(600));
    }

    #[test]
    fn settings_from_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let json = r#"{"browser":"firefox","headless":true,"timeout_seconds":60,"screenshot_dir":"shots","proxy":"http://proxy:8080","window_width":1280,"window_height":720,"user_data_dir":null}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser, "firefox");
        assert!(s.headless);
        assert_eq!(s.timeout_seconds, Some(60));
        assert_eq!(s.screenshot_dir, "shots");
        assert_eq!(s.proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(s.window_width, Some(1280));
        assert_eq!(s.window_height, Some(720));
    }

    #[test]
    fn settings_from_toml() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        let toml = r#"
browser = "edge"
headless = true
proxy = "http://proxy:8080"
mode = "uc"
mobile = true
threads = 4
"#;
        tmp.write_all(toml.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser, "edge");
        assert!(s.headless);
        assert_eq!(s.proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(s.mode, Some("uc".to_string()));
        assert!(s.mobile);
        assert_eq!(s.threads, Some(4));
    }

    #[test]
    fn to_browser_config_maps_fields() {
        let s = Settings {
            browser: "firefox".to_string(),
            headless: true,
            mode: Some("cdp".to_string()),
            mobile: true,
            threads: Some(2),
            proxy_pac_url: Some("http://proxy/proxy.pac".to_string()),
            ..Default::default()
        };
        let config = s.to_browser_config();
        assert!(matches!(
            config.browser,
            crate::browser::config::Browser::Firefox
        ));
        assert!(config.headless);
        assert!(matches!(config.mode, DriverMode::Cdp));
        assert!(config.mobile);
        assert_eq!(config.threads, Some(2));
        assert_eq!(
            config.proxy_pac_url,
            Some("http://proxy/proxy.pac".to_string())
        );
    }

    fn every_option() -> SettingsOverrides {
        SettingsOverrides {
            browser: Some("firefox".to_owned()),
            headless: Some(true),
            mode: Some("uc".to_owned()),
            user_agent: Some("TestAgent/1.0".to_owned()),
            locale: Some("fr-FR".to_owned()),
            ad_block: Some(true),
            proxy: Some("proxy.test:8080".to_owned()),
            proxy_pac_url: Some("http://proxy.test/proxy.pac".to_owned()),
            user_data_dir: Some("/tmp/profile".to_owned()),
            extension_dir: Some("/tmp/ext".to_owned()),
            reuse_session: Some(true),
            reuse_class_session: Some(true),
            mobile: Some(true),
            threads: Some(4),
            esc_end: Some(true),
            rec_sb_cdp: Some(true),
        }
    }

    #[test]
    fn reuse_follows_its_variables_and_reaches_the_browser_config() {
        let off = Settings::default().to_browser_config();
        assert!(!off.reuse_session);
        assert_eq!(off.launch.reuse_class, None);

        let run = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REUSE_SESSION").then(|| "true".to_owned())
        })
        .unwrap()
        .to_browser_config();
        assert!(run.reuse_session);
        assert_eq!(run.launch.reuse_class, None, "--rs shares across the run");

        let module = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REUSE_CLASS_SESSION").then(|| "1".to_owned())
        })
        .unwrap()
        .to_browser_config();
        assert!(module.reuse_session, "--rcs implies --rs, as in Python");
        assert_eq!(module.launch.reuse_class, Some(ReuseClass::TestModule));

        let from_toml: Settings = toml::from_str("reuse_class_session = true").unwrap();
        assert!(from_toml.reuse_class_session);
        let only = SettingsOverrides {
            reuse_class_session: Some(true),
            ..SettingsOverrides::default()
        };
        assert_eq!(only.env(), [("SB_REUSE_CLASS_SESSION", "true".to_owned())]);
    }

    #[test]
    fn overrides_that_are_set_replace_the_settings() {
        let mut settings = Settings::default();
        every_option().apply(&mut settings);

        assert_eq!(settings.browser, "firefox");
        assert!(
            settings.headless && settings.ad_block && settings.reuse_session && settings.mobile
        );
        assert_eq!(settings.mode.as_deref(), Some("uc"));
        assert_eq!(settings.user_agent.as_deref(), Some("TestAgent/1.0"));
        assert_eq!(settings.locale.as_deref(), Some("fr-FR"));
        assert_eq!(settings.proxy.as_deref(), Some("proxy.test:8080"));
        assert_eq!(
            settings.proxy_pac_url.as_deref(),
            Some("http://proxy.test/proxy.pac")
        );
        assert_eq!(settings.user_data_dir.as_deref(), Some("/tmp/profile"));
        assert_eq!(settings.extension_dir.as_deref(), Some("/tmp/ext"));
        assert_eq!(settings.threads, Some(4));
        assert!(settings.esc_end && settings.rec_sb_cdp);
    }

    #[test]
    fn the_plugins_table_of_a_toml_file_reaches_the_settings() {
        let settings: Settings = toml::from_str(
            r#"
            headless = true

            [plugins]
            enable = ["shield", "crx"]

            [plugins.shield]
            level = "strict"
            "#,
        )
        .unwrap();

        assert!(settings.headless);
        assert_eq!(settings.plugins.enabled(), ["shield", "crx"]);
        assert_eq!(
            settings.plugins.options_of("shield").unwrap()["level"],
            "strict"
        );
    }

    #[test]
    fn settings_without_plugins_write_no_plugins_key() {
        let text = toml::to_string(&Settings::default()).unwrap();
        assert!(!text.contains("plugins"), "{text}");
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!json.contains("plugins"), "{json}");

        let with: Settings = toml::from_str("[plugins]\nenable = [\"shield\"]").unwrap();
        let round_trip: Settings = toml::from_str(&toml::to_string(&with).unwrap()).unwrap();
        assert_eq!(round_trip, with);

        // The options table comes after `enable` and the other keys on the way out.
        let with: Settings = toml::from_str(
            "headless = true\n[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();
        let round_trip: Settings = toml::from_str(&toml::to_string(&with).unwrap()).unwrap();
        assert_eq!(round_trip, with);
        let json_trip: Settings =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(json_trip, with);
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn sb_plugins_replaces_the_names_and_keeps_the_options_of_the_file() {
        let from_file: Settings =
            toml::from_str("[plugins]\nenable = [\"crx\"]\n[plugins.shield]\nlevel = \"strict\"\n")
                .unwrap();

        let settings = Settings::apply_overrides(from_file, |name| {
            (name == "SB_PLUGINS").then(|| "shield, block-resources".to_owned())
        })
        .unwrap();

        assert_eq!(settings.plugins.enabled(), ["shield", "block-resources"]);
        assert!(settings.plugins.options_of("shield").is_some());
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_sb_plugins_list_drops_the_options_of_the_names_it_no_longer_has() {
        let from_file: Settings = toml::from_str(
            "[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();

        let off = Settings::apply_overrides(from_file.clone(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(off.plugins.is_empty(), "{:?}", off.plugins);

        let other = Settings::apply_overrides(from_file, |name| {
            (name == "SB_PLUGINS").then(|| "crx".to_owned())
        })
        .unwrap();
        assert_eq!(other.plugins.enabled(), ["crx"]);
        assert!(other.plugins.options_of("shield").is_none());
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_empty_sb_plugins_turns_the_plugins_of_the_file_off() {
        let from_file: Settings = toml::from_str("[plugins]\nenable = [\"shield\"]\n").unwrap();

        let settings = Settings::apply_overrides(from_file.clone(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(settings.plugins.enabled().is_empty());

        let untouched = Settings::apply_overrides(from_file, |_| None).unwrap();
        assert_eq!(untouched.plugins.enabled(), ["shield"]);
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn without_plugin_support_an_empty_sb_plugins_still_overrides_the_file() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n")
            .unwrap();

        let settings = Settings::load_with(Some(file.path()), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(settings.plugins.is_empty());

        // Without the override the file's selection stops the load.
        let error = Settings::load_with(Some(file.path()), |_| None).unwrap_err();
        assert!(
            error.to_string().contains("--features plugins-builtin"),
            "{error}"
        );
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    #[allow(deprecated)] // `from_toml` is deprecated but still has to refuse the selection
    fn without_plugin_support_a_selection_stops_the_load() {
        let hint = "--features plugins-builtin";
        let error = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_PLUGINS").then(|| "shield".to_owned())
        })
        .unwrap_err();
        assert!(error.to_string().contains(hint), "{error}");

        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[plugins]\nenable = [\"shield\"]\n")
            .unwrap();
        for result in [
            Settings::from_file(file.path()),
            Settings::from_toml(file.path()),
        ] {
            let error = result.unwrap_err();
            assert!(error.to_string().contains(hint), "{error}");
        }

        // Asking for nothing is fine.
        Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
    }

    #[test]
    fn rec_sb_cdp_is_off_by_default_and_follows_its_variable_and_key() {
        assert!(!Settings::default().rec_sb_cdp);
        assert!(!Settings::default().to_browser_config().rec_sb_cdp());
        let settings = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REC_SB_CDP").then(|| "yes".to_owned())
        })
        .unwrap();
        assert!(settings.rec_sb_cdp);
        assert!(settings.to_browser_config().rec_sb_cdp());
        assert!(!settings.to_browser_config().esc_end());
        let from_toml: Settings = toml::from_str("rec_sb_cdp = true").unwrap();
        assert!(from_toml.rec_sb_cdp);
        let only = SettingsOverrides {
            rec_sb_cdp: Some(true),
            ..SettingsOverrides::default()
        };
        assert_eq!(only.env(), [("SB_REC_SB_CDP", "true".to_owned())]);
    }

    #[test]
    fn esc_end_is_off_by_default_and_reaches_the_browser_config() {
        assert!(!Settings::default().esc_end);
        assert!(!Settings::default().to_browser_config().esc_end());
        let settings = Settings {
            esc_end: true,
            ..Settings::default()
        };
        assert!(settings.to_browser_config().esc_end());
    }

    #[test]
    fn the_esc_end_variable_and_the_config_file_key_turn_it_on() {
        for (value, expected) in [("true", true), ("1", true), ("off", false)] {
            let settings = Settings::apply_overrides(Settings::default(), |name| {
                (name == "SB_ESC_END").then(|| value.to_owned())
            })
            .unwrap();
            assert_eq!(settings.esc_end, expected, "{value}");
        }
        assert!(Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_ESC_END").then(|| "maybe".to_owned())
        })
        .is_err());

        let from_toml: Settings = toml::from_str("esc_end = true").unwrap();
        assert!(from_toml.esc_end);
        let old: Settings = serde_json::from_str(r#"{"browser":"edge"}"#).unwrap();
        assert!(!old.esc_end, "a file without the key leaves it off");
    }

    #[test]
    fn the_esc_end_override_is_exported_as_its_variable() {
        let only = SettingsOverrides {
            esc_end: Some(true),
            ..SettingsOverrides::default()
        };
        assert_eq!(only.env(), [("SB_ESC_END", "true".to_owned())]);
    }

    #[test]
    fn overrides_that_are_not_set_leave_the_settings_alone() {
        let before = Settings {
            browser: "edge".to_owned(),
            headless: true,
            proxy: Some("from-config:1".to_owned()),
            threads: Some(2),
            ..Settings::default()
        };
        let mut after = before.clone();

        SettingsOverrides::default().apply(&mut after);

        assert_eq!(after, before);
    }

    #[test]
    fn an_explicit_false_beats_a_true_in_the_settings() {
        let mut settings = Settings {
            headless: true,
            ..Settings::default()
        };
        SettingsOverrides {
            headless: Some(false),
            ..SettingsOverrides::default()
        }
        .apply(&mut settings);
        assert!(!settings.headless);
    }

    #[test]
    fn the_environment_form_means_the_same_as_applying_directly() {
        let overrides = every_option();
        let vars: std::collections::HashMap<_, _> = overrides.env().into_iter().collect();
        assert_eq!(vars.len(), 16, "one variable per option");

        let via_env =
            Settings::apply_overrides(Settings::default(), |name| vars.get(name).cloned()).unwrap();
        let mut direct = Settings::default();
        overrides.apply(&mut direct);

        assert_eq!(via_env, direct);
    }

    #[test]
    fn options_that_are_not_set_produce_no_variable() {
        assert!(SettingsOverrides::default().env().is_empty());
        let only_proxy = SettingsOverrides {
            proxy: Some("p:1".to_owned()),
            ..SettingsOverrides::default()
        };
        assert_eq!(only_proxy.env(), [("SB_PROXY", "p:1".to_owned())]);
    }

    #[test]
    fn an_override_lookup_that_finds_nothing_changes_nothing() {
        let original = Settings {
            browser: "edge".to_owned(),
            mobile: true,
            ..Settings::default()
        };
        let same = Settings::apply_overrides(original.clone(), |_| None).unwrap();
        assert_eq!(same, original);
    }

    #[test]
    fn a_bad_override_value_is_an_error() {
        let result = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_THREADS").then(|| "many".to_owned())
        });
        assert!(result.is_err());
    }

    #[test]
    fn parse_bool_values() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("YES").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(!parse_bool("false").unwrap());
        assert!(!parse_bool("OFF").unwrap());
        assert!(parse_bool("not-a-bool").is_err());
    }
}
