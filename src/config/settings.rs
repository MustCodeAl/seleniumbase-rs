use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::run_settings::RunSettings;
use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::browser::options::{LaunchSettings, LaunchSwitch, WindowSize};
use crate::browser::reuse::ReuseClass;
use crate::config::plugins::{PluginSelection, PLUGINS_ENV};
use crate::config::test_settings::TestSettings;
use crate::core::test_run::ReportSettings;

use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::Fingerprint;

/// The variable that names the fingerprint file to apply to every browser.
pub const FINGERPRINT_ENV: &str = "SB_FINGERPRINT";

/// The variable that names the page every browser opens first (Python's `--start-page`;
/// `--start-page`, `--url` and `--start_page` on `sbrs`).
pub const START_PAGE_ENV: &str = "SB_START_PAGE";

/// The variable that names the settings file to read instead of the global file of the
/// current folder (Python's `--settings-file`). A relative path is relative to the folder
/// the program runs in; [`Settings::load`] reads it.
pub const SETTINGS_FILE_ENV: &str = "SB_SETTINGS_FILE";

/// The `[fingerprint]` table of the settings file: which fingerprint file every browser
/// of the run presents (`SB_FINGERPRINT`, and `--fingerprint` on `sbrs`).
///
/// The file is the v2 `Fingerprint` JSON that `Fingerprint::save` and
/// `sbrs fingerprint capture` write. A relative path is relative to the folder the
/// program runs in. The command line wins over `SB_FINGERPRINT`, which wins over this
/// table.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Settings;
///
/// let settings: Settings = toml::from_str(
///     r#"
///     headless = true
///
///     [fingerprint]
///     file = "profiles/real-chrome.json"
///     "#,
/// )?;
///
/// assert_eq!(
///     settings.fingerprint.file.as_deref(),
///     Some(std::path::Path::new("profiles/real-chrome.json"))
/// );
/// # Ok::<(), toml::de::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FingerprintSettings {
    /// The fingerprint file to apply. `None` applies none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

impl FingerprintSettings {
    /// Whether no fingerprint file is named.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file.is_none()
    }
}

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
    /// The fingerprint file every browser presents (the `[fingerprint]` table, and
    /// `SB_FINGERPRINT`). Turn it into the fingerprint with
    /// [`load_fingerprint`](Self::load_fingerprint); [`to_browser_config`](Self::to_browser_config)
    /// does it for a `BaseCase`.
    #[serde(default, skip_serializing_if = "FingerprintSettings::is_empty")]
    pub fingerprint: FingerprintSettings,
    /// The page every browser opens first (`SB_START_PAGE`, the `start_page` key, and
    /// `--start-page` on `sbrs`; Python's `--start-page`). It becomes
    /// [`BrowserConfig::start_page`], which `BaseCase::new` opens. `None` opens no page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_page: Option<String>,
    /// The settings file the run reads (`SB_SETTINGS_FILE`, Python's `--settings-file`).
    /// It names the file, so it is never read from one.
    #[serde(skip)]
    pub settings_file: Option<PathBuf>,
    /// Run headless with `chrome-headless-shell` (`SB_HEADLESS1`, `--headless1`).
    pub headless1: bool,
    /// Do not load images (`SB_BLOCK_IMAGES`, `--block-images`).
    pub block_images: bool,
    /// Turn JavaScript off (`SB_DISABLE_JS`, `--disable-js`).
    pub disable_js: bool,
    /// Refuse every cookie (`SB_DISABLE_COOKIES`, `--disable-cookies`).
    pub disable_cookies: bool,
    /// Send the `DNT: 1` header (`SB_DO_NOT_TRACK`, `--do-not-track`).
    pub do_not_track: bool,
    /// Download PDF files instead of showing them (`SB_EXTERNAL_PDF`, `--external-pdf`).
    pub external_pdf: bool,
    /// Leave Chrome Sync on (`SB_ENABLE_SYNC`, `--enable-sync`).
    pub enable_sync: bool,
    /// Strip the Content Security Policy (`SB_DISABLE_CSP`, `--disable-csp`).
    pub disable_csp: bool,
    /// Keep Chrome's automation extension (`SB_USE_AUTO_EXT`, `--use-auto-ext`).
    pub use_auto_ext: bool,
    /// Download the driver through the proxy (`SB_PROXY_DRIVER`, `--proxy-driver`).
    pub proxy_driver: bool,
    /// Force Chrome's dark theme (`SB_DARK_MODE`, `--dark-mode`).
    pub dark_mode: bool,
    /// Open the developer tools with each tab (`SB_DEVTOOLS`, `--devtools`).
    pub devtools: bool,
    /// Switch the GPU off (`SB_DISABLE_GPU`, `--disable-gpu`).
    pub disable_gpu: bool,
    /// Switch web security off (`SB_DISABLE_WS`, `--disable-ws`).
    pub disable_ws: bool,
    /// Open the browser as a guest (`SB_GUEST_MODE`, `--guest-mode`).
    pub guest_mode: bool,
    /// Open the browser in an incognito window (`SB_INCOGNITO`, `--incognito`).
    pub incognito: bool,
    /// Start Chromium without its sandbox (`SB_NO_SANDBOX`, `--no-sandbox`).
    pub no_sandbox: bool,
    /// Render with SwiftShader (`SB_SWIFTSHADER`, `--swiftshader`).
    pub swiftshader: bool,
    /// Keep the GPU in a headless session, for WebGL (`SB_ENABLE_3D_APIS`).
    pub enable_3d_apis: bool,
    /// Maximize the window (`SB_MAXIMIZE`, `--maximize`).
    pub maximize: bool,
    /// Extra Chromium switches, separated by commas (`SB_CHROMIUM_ARG`).
    pub chromium_arg: Option<crate::config::browser_options::ArgList>,
    /// Chromium features to switch off, separated by commas (`SB_DISABLE_FEATURES`).
    pub disable_features: Option<String>,
    /// Chromium host-resolver rules (`SB_HOST_RESOLVER_RULES`).
    pub host_resolver_rules: Option<String>,
    /// The Selenium Grid server (`SB_SERVER`, `--server`); see [`to_browser_config`](Self::to_browser_config).
    pub server: Option<String>,
    /// The Selenium Grid port (`SB_PORT`, `--port`).
    pub port: Option<u16>,
    /// The Selenium Grid protocol (`SB_PROTOCOL`, `--protocol`).
    pub protocol: Option<String>,
    /// A language code such as `fr` or `ar_eg` (`SB_LOCALE_CODE`); it wins over `locale`.
    pub locale_code: Option<String>,
    /// The window size as `WIDTH,HEIGHT` (`SB_WINDOW_SIZE`); it wins over `window_width` and
    /// `window_height`.
    pub window_size: Option<String>,
    /// The window position as `X,Y` (`SB_WINDOW_POSITION`).
    pub window_position: Option<String>,
    /// The phone screen to emulate as `WIDTH,HEIGHT,PIXEL_RATIO` (`SB_DEVICE_METRICS`).
    pub device_metrics: Option<String>,
    /// How long a navigation waits: `normal`, `eager` or `none` (`SB_PAGE_LOAD_STRATEGY`).
    pub page_load_strategy: Option<String>,
    /// The driver version (`SB_DRIVER_VERSION`).
    pub driver_version: Option<String>,
    /// Hosts that skip the proxy, separated by commas or semicolons (`SB_PROXY_BYPASS_LIST`).
    pub proxy_bypass_list: Option<String>,
    /// A JSON file of capabilities to add (`SB_CAP_FILE`).
    pub cap_file: Option<PathBuf>,
    /// A JSON object of capabilities to add (`SB_CAP_STRING`); it wins over `cap_file`.
    pub cap_string: Option<String>,
    /// Extension `.zip` or `.crx` files, separated by commas (`SB_EXTENSION_ZIP`).
    pub extension_zip: Option<String>,
    /// Firefox arguments, separated by commas (`SB_FIREFOX_ARG`).
    pub firefox_arg: Option<String>,
    /// Firefox preferences as `NAME:VALUE`, separated by commas (`SB_FIREFOX_PREF`).
    pub firefox_pref: Option<String>,
    /// Open a virtual display for a headed browser on Linux (`SB_XVFB`).
    pub xvfb: bool,
    /// The virtual display size as `WIDTH,HEIGHT` (`SB_XVFB_METRICS`).
    pub xvfb_metrics: Option<String>,
    /// Open Chrome's remote debugger on port 9222 (`SB_REMOTE_DEBUG`).
    pub remote_debug: bool,
    /// Log the DevTools Protocol events (`SB_LOG_CDP_EVENTS`).
    pub log_cdp_events: bool,
    /// Log the DevTools Protocol events in undetected mode only (`SB_UC_CDP_EVENTS`).
    pub uc_cdp_events: bool,
    /// Demo mode: highlight and pause after each action (`SB_DEMO`).
    pub demo: bool,
    /// Slow mode: pause briefly after each action (`SB_SLOW`).
    pub slow: bool,
    /// Do not wait for the page's JavaScript (`SB_SKIP_JS_WAITS`, `--sjw`).
    pub skip_js_waits: bool,
    /// Set the visual baselines again (`SB_VISUAL_BASELINE`).
    pub visual_baseline: bool,
    /// Delete the cookies of a reused browser (`SB_CRUMBS`); [`to_browser_config`](Self::to_browser_config)
    /// turns the process-wide switch on, as `browser::reuse::set_crumbs(true)` does.
    pub crumbs: bool,
    /// Seconds to pause after an action in demo mode (`SB_DEMO_SLEEP`).
    pub demo_sleep: Option<f64>,
    /// Seconds a message stays on the page (`SB_MESSAGE_DURATION`).
    pub message_duration: Option<f64>,
    /// How many times to flash a highlighted element (`SB_HIGHLIGHTS`); wired by the run
    /// settings of the `sb` fixture (RS-1).
    pub highlights: Option<u32>,
    /// Seconds between the steps of a tour (`SB_INTERVAL`); wired by RS-1.
    pub interval: Option<f64>,
    /// Seconds to wait before a MasterQA verification (`SB_VERIFY_DELAY`); wired by RS-1.
    pub verify_delay: Option<f64>,
    /// Check each page for JavaScript errors (`SB_CHECK_JS`); wired by RS-1.
    pub check_js: bool,
    /// Wait for AngularJS after opening a page (`SB_WAIT_FOR_ANGULARJS`); wired by RS-1.
    pub wait_for_angularjs: bool,
    /// Remove the beforeunload prompt after a page opens (`SB_DISABLE_BEFOREUNLOAD`); wired by RS-1.
    pub disable_beforeunload: bool,
    /// Multiplies the default waits (`SB_TIMEOUT_MULTIPLIER`); wired by RS-1 (`RunSettings::with_timeout_multiplier`).
    pub timeout_multiplier: Option<f64>,
    /// The `[test]` table: test data, debug modes, reruns, evidence and reports.
    pub test: TestSettings,
    /// The `[report]` table: what a run writes beyond the `[test]` table's options
    /// (`SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`, `SB_REPORT_DB`). See
    /// [`ReportSettings::from_settings`].
    pub report: ReportSettings,
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
    /// The fingerprint file to apply (`SB_FINGERPRINT`).
    pub fingerprint: Option<String>,
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
    /// The other options of the option table, each as its `SB_*` variable and the text of its
    /// value (`--dark` is `("SB_DARK_MODE", "true")`). [`try_apply`](Self::try_apply) reads
    /// them as the environment is read, and [`env`](Self::env) hands them on unchanged; a
    /// later pair for one variable wins.
    pub options: Vec<(&'static str, String)>,

    /// The page every browser opens first (`SB_START_PAGE`).
    pub start_page: Option<String>,
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
        if let Some(file) = &self.fingerprint {
            settings.fingerprint.file = Some(PathBuf::from(file));
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
        if let Some(page) = &self.start_page {
            settings.start_page = Some(page.clone());
        }
    }

    /// [`apply`](Self::apply), then the `options` as the environment is read, then the checks
    /// of [`Settings::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a value that does not parse (naming
    /// its variable) and for a result that is not valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
    ///
    /// let overrides = SettingsOverrides {
    ///     options: vec![("SB_DARK_MODE", "true".to_owned())],
    ///     ..SettingsOverrides::default()
    /// };
    /// let mut settings = Settings::default();
    /// overrides.try_apply(&mut settings)?;
    /// assert!(settings.dark_mode);
    ///
    /// let wrong = SettingsOverrides {
    ///     options: vec![("SB_DEVICE_METRICS", "1,2".to_owned())],
    ///     ..SettingsOverrides::default()
    /// };
    /// assert!(wrong.try_apply(&mut Settings::default()).is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn try_apply(&self, settings: &mut Settings) -> Result<(), SeleniumBaseError> {
        self.apply(settings);
        settings.apply_options(&self.options)
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
        {
            let mut pairs: Vec<(&'static str, String)> = [
                text("SB_BROWSER", &self.browser),
                flag("SB_HEADLESS", self.headless),
                text("SB_MODE", &self.mode),
                text("SB_USER_AGENT", &self.user_agent),
                text("SB_LOCALE", &self.locale),
                flag("SB_AD_BLOCK", self.ad_block),
                text("SB_PROXY", &self.proxy),
                text("SB_PROXY_PAC_URL", &self.proxy_pac_url),
                text(FINGERPRINT_ENV, &self.fingerprint),
                text("SB_USER_DATA_DIR", &self.user_data_dir),
                text("SB_EXTENSION_DIR", &self.extension_dir),
                flag("SB_REUSE_SESSION", self.reuse_session),
                flag("SB_REUSE_CLASS_SESSION", self.reuse_class_session),
                flag("SB_MOBILE", self.mobile),
                self.threads
                    .map(|threads| ("SB_THREADS", threads.to_string())),
                flag("SB_ESC_END", self.esc_end),
                flag("SB_REC_SB_CDP", self.rec_sb_cdp),
                text(START_PAGE_ENV, &self.start_page),
            ]
            .into_iter()
            .flatten()
            .collect();
            pairs.extend(self.options.iter().cloned());
            pairs
        }
    }
}

/// The part of a settings file that [`Settings::load_plugin_table`] reads. Every other key
/// is left alone, so a value that is wrong for a browser does not stop a program that
/// wants the plugins only.
#[derive(Deserialize)]
struct PluginsOnly {
    #[serde(default)]
    plugins: PluginSelection,
}

/// The part of a settings file that [`Settings::resolve_fingerprint_file`] reads.
#[derive(Deserialize)]
struct FingerprintOnly {
    #[serde(default)]
    fingerprint: FingerprintSettings,
}

/// The `[fingerprint]` table of the settings file `path` (TOML for a `.toml` name, JSON
/// otherwise), without checking the rest of the file.
fn parse_fingerprint_table(path: &Path) -> Result<FingerprintSettings, SeleniumBaseError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}")))?;
    let file: FingerprintOnly = if path.extension().is_some_and(|ext| ext == "toml") {
        toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
        })?
    };
    Ok(file.fingerprint)
}

/// The `[plugins]` table of the settings file `path` (TOML for a `.toml` name, JSON
/// otherwise), without checking the rest of the file.
fn parse_plugins_table(path: &Path) -> Result<PluginSelection, SeleniumBaseError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}")))?;
    let file: PluginsOnly = if path.extension().is_some_and(|ext| ext == "toml") {
        toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
        })?
    };
    Ok(file.plugins)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            browser: "chrome".to_owned(),
            headless: false,
            timeout_seconds: None,
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
            fingerprint: FingerprintSettings::default(),
            start_page: None,
            settings_file: None,
            headless1: false,
            block_images: false,
            disable_js: false,
            disable_cookies: false,
            do_not_track: false,
            external_pdf: false,
            enable_sync: false,
            disable_csp: false,
            use_auto_ext: false,
            proxy_driver: false,
            dark_mode: false,
            devtools: false,
            disable_gpu: false,
            disable_ws: false,
            guest_mode: false,
            incognito: false,
            no_sandbox: false,
            swiftshader: false,
            enable_3d_apis: false,
            maximize: false,
            chromium_arg: None,
            disable_features: None,
            host_resolver_rules: None,
            server: None,
            port: None,
            protocol: None,
            locale_code: None,
            window_size: None,
            window_position: None,
            device_metrics: None,
            page_load_strategy: None,
            driver_version: None,
            proxy_bypass_list: None,
            cap_file: None,
            cap_string: None,
            extension_zip: None,
            firefox_arg: None,
            firefox_pref: None,
            xvfb: false,
            xvfb_metrics: None,
            remote_debug: false,
            log_cdp_events: false,
            uc_cdp_events: false,
            demo: false,
            slow: false,
            skip_js_waits: false,
            visual_baseline: false,
            crumbs: false,
            demo_sleep: None,
            message_duration: None,
            highlights: None,
            interval: None,
            verify_delay: None,
            check_js: false,
            wait_for_angularjs: false,
            disable_beforeunload: false,
            timeout_multiplier: None,
            test: TestSettings::default(),
            report: ReportSettings::default(),
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
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// Reads a settings file without checking that this build can run the
    /// plugins it names, so that [`load`](Self::load) can let `SB_PLUGINS`
    /// replace them first and check the result.
    fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let settings = Self::read_file(path)?;
        settings.validate()?;
        Ok(settings)
    }

    /// Reads and deserializes a settings file, without validating its values.
    fn read_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}"))
        })?;
        let settings = crate::config::browser_options::parse_settings(path, &content)?;
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
        self.browser.parse::<Browser>().map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "browser: {e} (set by --browser, SB_BROWSER or the settings file)"
            ))
        })?;
        self.check_launch_values()?;

        if let Some(mode) = &self.mode {
            mode.parse::<DriverMode>().map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!(
                    "mode: {e} (set by --mode, SB_MODE or the settings file)"
                ))
            })?;
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

    /// The fingerprint the `[fingerprint]` table or `SB_FINGERPRINT` names, read from its
    /// file; `None` when the settings name none.
    ///
    /// # Errors
    ///
    /// As [`Fingerprint::load`]: the file cannot be read, is too large, or is not a
    /// fingerprint.
    pub fn load_fingerprint(&self) -> Result<Option<Fingerprint>, SeleniumBaseError> {
        self.fingerprint
            .file
            .as_deref()
            .map(Fingerprint::load)
            .transpose()
    }

    /// Checks that the fingerprint file the settings name, if any, loads.
    ///
    /// [`from_file`](Self::from_file), [`load`](Self::load) and
    /// [`from_env`](Self::from_env) call it, so a wrong path is an error where the
    /// settings are read and not a browser without the fingerprint. Call it yourself on
    /// a `Settings` built by hand, before [`to_browser_config`](Self::to_browser_config).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] naming the file and the reason.
    pub fn check_fingerprint(&self) -> Result<(), SeleniumBaseError> {
        self.load_fingerprint().map(drop).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("the fingerprint of the settings: {error}"))
        })
    }

    /// The fingerprint file a command presents, without reading the rest of the settings:
    /// `flag` (the command line), else `SB_FINGERPRINT`, else the `[fingerprint]` table of
    /// the settings file `config` (the global file when `None`, as [`load`](Self::load)
    /// finds it).
    ///
    /// A command that starts a browser of its own (`sbrs devtools`) uses this, so that a
    /// value of the file that is wrong for a browser (`headless = "maybe"`) is not its
    /// problem. The file named is not read.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the settings file cannot be read or its
    /// `[fingerprint]` table is not valid.
    pub fn resolve_fingerprint_file<P: AsRef<Path>>(
        config: Option<P>,
        flag: Option<&Path>,
    ) -> Result<Option<PathBuf>, SeleniumBaseError> {
        Self::fingerprint_file_with(config, flag, |name| std::env::var(name).ok())
    }

    /// [`resolve_fingerprint_file`](Self::resolve_fingerprint_file) with the variable
    /// taken from `var`.
    fn fingerprint_file_with<P: AsRef<Path>>(
        config: Option<P>,
        flag: Option<&Path>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<PathBuf>, SeleniumBaseError> {
        if let Some(flag) = flag {
            return Ok(Some(flag.to_owned()));
        }
        if let Some(file) = var(FINGERPRINT_ENV).filter(|value| !value.trim().is_empty()) {
            return Ok(Some(PathBuf::from(file)));
        }
        let table = match config {
            Some(path) => Some(path.as_ref().to_owned()),
            None => Self::global_file(),
        };
        match table {
            Some(path) => Ok(parse_fingerprint_table(&path)?.file),
            None => Ok(None),
        }
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
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// [`load_global`](Self::load_global) without the plugin support check.
    fn parse_global() -> Result<Self, SeleniumBaseError> {
        match Self::global_file() {
            Some(candidate) => Self::parse_file(candidate),
            None => Ok(Self::default()),
        }
    }

    /// The global settings file of the current folder, if there is one.
    fn global_file() -> Option<PathBuf> {
        Self::global_file_with(&|name| std::env::var(name).ok())
    }

    /// The settings file to read: the one `SB_SETTINGS_FILE` names (when `var` reports it
    /// and it is not blank), else the global file of the current folder if there is one.
    pub(super) fn global_file_with(var: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
        if let Some(path) = var(SETTINGS_FILE_ENV).filter(|path| !path.trim().is_empty()) {
            return Some(PathBuf::from(path));
        }
        ["sbase_config.toml", ".sbase_config.toml", ".sbase_config"]
            .into_iter()
            .find(|candidate| Path::new(candidate).is_file())
            .map(PathBuf::from)
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

    /// Loads only the plugin selection: the `[plugins]` table of the settings
    /// file `path` (the global file when `None`, as [`load`](Self::load)
    /// finds it) with `SB_PLUGINS` on top, which is [`load`](Self::load)'s
    /// `plugins` without the rest of the settings.
    ///
    /// A command that starts a browser without needing the other settings
    /// (`sbrs devtools`, `sbrs plugin list`, `sbrs-mcp`) uses this, so that a
    /// value of the file that is wrong for a browser (`headless = "maybe"`) or
    /// an invalid `SB_HEADLESS` is not its problem: only the `[plugins]` table
    /// is read from the file. Whether this build can run the plugins is not
    /// checked here, because the command line adds to the selection first:
    /// call [`PluginSelection::ensure_supported`] on the result, or build it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the file cannot be read or is
    /// not valid TOML (JSON for a name that does not end in `.toml`), or its
    /// `[plugins]` table is not a plugin selection.
    pub fn load_plugin_selection<P: AsRef<Path>>(
        path: Option<P>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        Self::plugin_selection_with(path, |name| std::env::var(name).ok())
    }

    /// [`load_plugin_selection`](Self::load_plugin_selection) with the
    /// variable taken from `var`.
    fn plugin_selection_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        let mut selection = Self::load_plugin_table(path)?;
        // As in `apply_overrides`: the variable names the plugins, and an
        // empty one is an answer, not an unset variable.
        if let Some(list) = var(PLUGINS_ENV) {
            selection.set_enabled_from_list(&list);
        }
        Ok(selection)
    }

    /// The `[plugins]` table of the settings file `path` (the global file
    /// when `None`, as [`load`](Self::load) finds it) and nothing else of it:
    /// no other value of the file is checked, and `SB_PLUGINS` is not read.
    /// A folder without a settings file selects no plugin.
    ///
    /// A program that applies `SB_PLUGINS` and command-line flags itself uses
    /// this, and so can tell which plugins the file turned on.
    ///
    /// # Errors
    ///
    /// As [`load_plugin_selection`](Self::load_plugin_selection).
    pub fn load_plugin_table<P: AsRef<Path>>(
        path: Option<P>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        match path {
            Some(path) => parse_plugins_table(path.as_ref()),
            None => match Self::global_file() {
                Some(candidate) => parse_plugins_table(&candidate),
                None => Ok(PluginSelection::new()),
            },
        }
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
            None => match Self::global_file_with(&var) {
                Some(file) => Self::parse_file(file)?,
                None => Self::default(),
            },
        };
        Self::apply_overrides(settings, var)
    }

    /// Reads the settings as [`load_from`](Self::load_from) does (the file `path`, or the
    /// global file when `None`, then the `SB_*` variables), and reports everything it
    /// would refuse, not only the first thing: the file that cannot be read, each variable
    /// that does not parse, then the browser, the mode and the window size, and the plugins.
    ///
    /// A part that cannot be read is left at its default, so the rest is still checked and
    /// the returned settings are the ones a program would run with if it ignored the
    /// problems. Nothing is started.
    pub fn check<P: AsRef<Path>>(path: Option<P>) -> SettingsCheck {
        Self::check_with(path, |name| std::env::var(name).ok())
    }

    /// [`check`](Self::check) with the variables taken from `var`.
    pub fn check_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> SettingsCheck {
        let file = match &path {
            Some(path) => Some(path.as_ref().to_path_buf()),
            None => Self::global_file_with(&var),
        };
        let mut problems = Vec::new();
        let from_file = match &file {
            Some(file) => Self::read_file(file).unwrap_or_else(|error| {
                problems.push(SettingsProblem {
                    source: file.display().to_string(),
                    error,
                });
                Self::default()
            }),
            None => Self::default(),
        };
        let (settings, from_variables) = Self::apply_overrides_lenient(from_file, var);
        problems.extend(from_variables.into_iter().map(|problem| SettingsProblem {
            source: problem.source.clone(),
            error: named(problem),
        }));
        if let Err(error) = settings.validate() {
            problems.push(SettingsProblem {
                source: "validation".to_owned(),
                error,
            });
        }
        if let Err(error) = settings.plugins.ensure_supported() {
            problems.push(SettingsProblem {
                source: "plugins".to_owned(),
                error,
            });
        }
        if let Err(error) = settings.check_fingerprint() {
            problems.push(SettingsProblem {
                source: "fingerprint".to_owned(),
                error,
            });
        }
        SettingsCheck {
            file,
            settings,
            problems,
        }
    }

    /// Converts these settings into a `BrowserConfig` used by `BaseCase`.
    ///
    /// Settings that came from a file or the environment are checked already.
    /// For one built by hand, call [`validate`](Self::validate) first: an
    /// unknown browser or mode becomes Chrome or WebDriver here.
    pub fn to_browser_config(&self) -> BrowserConfig {
        let base = self.base_browser_config();
        self.with_launch_options(base.clone())
            .unwrap_or_else(|error| {
                // A `Settings` that was read is checked already; this is one made by hand.
                tracing::warn!(%error, "the launch options of the settings are ignored");
                base
            })
    }

    /// The configuration of the settings that were there from the first release;
    /// the launch options of the table are added on top of it.
    pub(super) fn base_browser_config(&self) -> BrowserConfig {
        self.apply_process_wide();

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
            cdp_url: None,
            cdp_discovery: false,
            cdp_target: None,

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
            start_page: self.start_page.clone(),
            extra_args: Vec::new(),
            fingerprint: self.load_fingerprint().unwrap_or_else(|error| {
                // A `Settings` that was read is checked already; this is one made by hand.
                tracing::warn!(%error, "the fingerprint file of the settings is ignored");
                None
            }),
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
        settings: Self,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SeleniumBaseError> {
        let (settings, problems) = Self::apply_overrides_lenient(settings, var);
        if let Some(problem) = problems.into_iter().next() {
            return Err(named(problem));
        }
        settings.validate()?;
        // Plugins that cannot run are an error here, not a silent omission.
        settings.plugins.ensure_supported()?;
        // A fingerprint file that cannot be read is too, not a browser without it.
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// Writes options given as `SB_*` variables and the text of their values, the way
    /// [`load`](Self::load) writes the environment: the pairs of the same variable are read
    /// last one first, a blank value counts as unset, and a pair whose variable no option of
    /// the table reads (the shorthands, `SB_PLUGINS`) is ignored. The result is checked with
    /// [`validate`](Self::validate).
    ///
    /// This is how the flags of `sbrs` that have no field of their own in
    /// [`SettingsOverrides`] reach the settings.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a value that does not parse (naming
    /// its variable) and for a result that is not valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Settings;
    ///
    /// let mut settings = Settings::default();
    /// settings.apply_options(&[
    ///     ("SB_DEVICE_METRICS", "360,640,2".to_owned()),
    ///     ("SB_BLOCK_IMAGES", "true".to_owned()),
    /// ])?;
    /// assert!(settings.block_images);
    /// assert_eq!(settings.device_metrics.as_deref(), Some("360,640,2"));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn apply_options(&mut self, options: &[(&str, String)]) -> Result<(), SeleniumBaseError> {
        let var = |name: &str| {
            options
                .iter()
                .rev()
                .find(|(variable, _)| *variable == name)
                .map(|(_, value)| value.clone())
                .filter(|value| !value.trim().is_empty())
        };
        let mut problems = Vec::new();
        crate::config::test_options::apply_env(self, &var, &mut problems);
        if let Some(problem) = problems.into_iter().next() {
            return Err(named(problem));
        }
        self.validate()
    }

    /// [`apply_overrides`](Self::apply_overrides) up to the checks: a variable that does
    /// not parse is left out and comes back in the list, in the order the variables are
    /// read, instead of ending the function. Nothing is validated.
    fn apply_overrides_lenient(
        mut settings: Self,
        var: impl Fn(&str) -> Option<String>,
    ) -> (Self, Vec<SettingsProblem>) {
        let mut problems = Vec::new();
        let plugins_variable = var(PLUGINS_ENV);
        let var = |name: &str| var(name).filter(|value| !value.trim().is_empty());
        // The options of the table (`config::test_options`) are read in table order.
        crate::config::test_options::apply_env(&mut settings, &var, &mut problems);
        if let Some(v) = plugins_variable {
            // The variable names the plugins; the options of the file stay.
            settings.plugins.set_enabled_from_list(&v);
        }
        (settings, problems)
    }
}

/// What [`Settings::check`] found.
#[derive(Debug)]
pub struct SettingsCheck {
    /// The settings file that was read: the one named, or the one found in the current
    /// folder. `None` when there is none.
    pub file: Option<PathBuf>,
    /// The settings: the file's, with the variables on top. A part that could not be read
    /// is at its default.
    pub settings: Settings,
    /// Everything [`Settings::load_from`] would refuse, in the order it would meet it; empty
    /// when it would succeed.
    pub problems: Vec<SettingsProblem>,
}

/// Something wrong with the settings, and where it came from.
#[derive(Debug)]
pub struct SettingsProblem {
    /// The settings file, an `SB_*` variable, or `validation` or `plugins` for a check of the
    /// result.
    pub source: String,
    /// What is wrong.
    pub error: SeleniumBaseError,
}

/// The error of `problem`, saying which variable (and which flag) it came from when its text
/// does not: `SB_HEADLESS (--headless): cannot parse bool: maybe`.
fn named(problem: SettingsProblem) -> SeleniumBaseError {
    let SettingsProblem { source, error } = problem;
    let SeleniumBaseError::InvalidConfig(text) = &error else {
        return error;
    };
    if !source.starts_with("SB_") || text.contains(&source) {
        return error;
    }
    let flag = crate::config::test_options::TEST_OPTIONS
        .iter()
        .find(|row| row.env == source && row.kind.is_flag() && row.kind.reads_env())
        .map_or_else(String::new, |row| format!(" (--{})", row.long));
    SeleniumBaseError::InvalidConfig(format!("{source}{flag}: {text}"))
}

pub(crate) fn parse_bool(value: &str) -> Result<bool, SeleniumBaseError> {
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
    fn a_refused_variable_is_named_with_its_flag() {
        let refused = |name: &'static str, value: &str| {
            Settings::default()
                .apply_options(&[(name, value.to_owned())])
                .unwrap_err()
                .to_string()
        };

        let error = refused("SB_HEADLESS", "maybe");
        assert!(
            error.contains("SB_HEADLESS (--headless): cannot parse bool: maybe"),
            "{error}"
        );
        // A variable with no flag of its own is named alone.
        let error = refused("SB_TIMEOUT", "soon");
        assert!(error.contains("SB_TIMEOUT: "), "{error}");
        // A name or a mode is checked after the settings are read, and says where it was set.
        let error = refused("SB_BROWSER", "chrom");
        assert!(error.contains("SB_BROWSER"), "{error}");
        let error = refused("SB_MODE", "stealth");
        assert!(error.contains("SB_MODE"), "{error}");
    }

    #[test]
    fn default_settings_are_sensible() {
        let s = Settings::default();
        assert_eq!(s.browser, "chrome");
        assert!(!s.headless);
        assert_eq!(s.timeout_seconds, None);
        assert!(!s.dark_mode);
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
        let json = r#"{"browser":"firefox","headless":true,"timeout_seconds":60,"proxy":"http://proxy:8080","window_width":1280,"window_height":720,"user_data_dir":null}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser, "firefox");
        assert!(s.headless);
        assert_eq!(s.timeout_seconds, Some(60));
        assert!(!s.test.pdb);
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

    /// A folder holding a fingerprint file, and the path of the file.
    fn fingerprint_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");
        Fingerprint::windows_desktop().save(&path).unwrap();
        (dir, path)
    }

    fn every_option(fingerprint: &Path) -> SettingsOverrides {
        SettingsOverrides {
            fingerprint: Some(fingerprint.display().to_string()),
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
            start_page: Some("https://example.test/".to_owned()),
            options: Vec::new(),
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
        let (_dir, file) = fingerprint_file();
        let mut settings = Settings::default();
        every_option(&file).apply(&mut settings);

        assert_eq!(settings.fingerprint.file.as_deref(), Some(file.as_path()));
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
        assert_eq!(
            settings.start_page.as_deref(),
            Some("https://example.test/")
        );
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

    #[test]
    fn the_plugin_table_is_read_whatever_the_rest_of_the_file_holds() {
        // Not a valid `Settings` file: `headless` is not a boolean, `browser` is no browser.
        let body = "headless = \"maybe\"\nbrowser = \"nope\"\n[plugins]\nenable = [\"shield\"]\n\
                    [plugins.shield]\nlevel = \"strict\"\n";
        let mut toml_file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        toml_file.write_all(body.as_bytes()).unwrap();
        assert!(
            Settings::from_file(toml_file.path()).is_err(),
            "the file as a whole is wrong"
        );

        let table = Settings::load_plugin_table(Some(toml_file.path())).unwrap();
        assert_eq!(table.enabled(), ["shield"]);
        assert_eq!(table.options_of("shield").unwrap()["level"], "strict");
        let selection = Settings::plugin_selection_with(Some(toml_file.path()), |_| None).unwrap();
        assert_eq!(selection, table);

        let mut json_file = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        json_file
            .write_all(br#"{"headless":"maybe","plugins":{"enable":["crx"]}}"#)
            .unwrap();
        let table = Settings::load_plugin_table(Some(json_file.path())).unwrap();
        assert_eq!(table.enabled(), ["crx"]);

        // A file that is not TOML at all still cannot be read.
        let mut broken = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        broken.write_all(b"[plugins\n").unwrap();
        let error = Settings::load_plugin_table(Some(broken.path())).unwrap_err();
        assert!(error.to_string().contains("failed to parse"), "{error}");
    }

    #[test]
    fn a_settings_file_without_a_plugins_table_selects_nothing() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"headless = true\n").unwrap();
        assert!(Settings::load_plugin_table(Some(file.path()))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_plugin_selection_alone_is_the_file_with_sb_plugins_on_top() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(
            b"headless = true\n[plugins]\nenable = [\"shield\", \"crx\"]\n\
              [plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();

        let from_file = Settings::plugin_selection_with(Some(file.path()), |_| None).unwrap();
        assert_eq!(from_file.enabled(), ["shield", "crx"]);
        assert_eq!(from_file.options_of("shield").unwrap()["level"], "strict");

        // The variable replaces the names and drops the options of the others;
        // another variable with a bad value is not read at all.
        let replaced = Settings::plugin_selection_with(Some(file.path()), |name| match name {
            "SB_PLUGINS" => Some("crx".to_owned()),
            "SB_HEADLESS" => Some("maybe".to_owned()),
            _ => None,
        })
        .unwrap();
        assert_eq!(replaced.enabled(), ["crx"]);
        assert!(replaced.options_of("shield").is_none());

        // An empty variable is an answer: no plugin at all.
        let off = Settings::plugin_selection_with(Some(file.path()), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(off.is_empty(), "{off:?}");

        // A file that is not there is an error, as it is for `load_from`.
        let error =
            Settings::plugin_selection_with(Some("no-such-settings.toml"), |_| None).unwrap_err();
        assert!(error.to_string().contains("failed to read"), "{error}");
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
        let error = Settings::from_file(file.path()).unwrap_err();
        assert!(error.to_string().contains(hint), "{error}");

        // Asking for nothing is fine.
        Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
    }

    fn variables(
        pairs: &'static [(&'static str, &'static str)],
    ) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn check_reports_every_bad_variable_and_keeps_the_good_ones() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"locale = \"de-DE\"\n").unwrap();

        let check = Settings::check_with(
            Some(file.path()),
            variables(&[
                ("SB_HEADLESS", "maybe"),
                ("SB_THREADS", "many"),
                ("SB_WINDOW_WIDTH", "wide"),
                ("SB_BROWSER", "chrom"),
                ("SB_MOBILE", "yes"),
            ]),
        );

        let sources: Vec<&str> = check.problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(
            sources,
            ["SB_HEADLESS", "SB_WINDOW_WIDTH", "SB_THREADS", "validation"]
        );
        assert!(check.problems[0].error.to_string().contains("maybe"));
        assert!(check.problems[2]
            .error
            .to_string()
            .starts_with("invalid configuration: SB_THREADS:"));
        assert!(check.problems[3].error.to_string().contains("chrom"));
        // The good variables and the file are kept.
        assert!(check.settings.mobile);
        assert_eq!(check.settings.locale.as_deref(), Some("de-DE"));
        assert_eq!(check.file.as_deref(), Some(file.path()));
    }

    #[test]
    fn check_finds_nothing_in_good_settings_and_load_agrees() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"edge\"\nthreads = 2\n")
            .unwrap();
        let pairs: &'static [(&str, &str)] = &[("SB_HEADLESS", "true"), ("SB_TIMEOUT", "7")];

        let check = Settings::check_with(Some(file.path()), variables(pairs));
        let loaded = Settings::load_with(Some(file.path()), variables(pairs)).unwrap();

        assert!(check.problems.is_empty(), "{:?}", check.problems);
        assert_eq!(check.settings, loaded);
    }

    #[test]
    fn check_names_a_file_that_cannot_be_read_and_still_checks_the_variables() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"headless = \"maybe\"\n").unwrap();

        let broken = Settings::check_with(Some(file.path()), variables(&[("SB_THREADS", "x")]));
        let missing = Settings::check_with(Some("no-such-settings.toml"), variables(&[]));

        let sources: Vec<&str> = broken.problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(
            sources,
            [file.path().display().to_string().as_str(), "SB_THREADS"]
        );
        assert!(broken.problems[0]
            .error
            .to_string()
            .contains("failed to parse TOML settings"));
        assert_eq!(missing.problems.len(), 1);
        assert!(missing.problems[0]
            .error
            .to_string()
            .contains("failed to read"));
    }

    #[test]
    fn the_first_problem_check_reports_is_the_error_load_returns() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"firefox\"\n").unwrap();
        for pairs in [
            &[("SB_HEADLESS", "maybe"), ("SB_THREADS", "x")][..],
            &[("SB_BROWSER", "chrom")][..],
            &[("SB_WINDOW_WIDTH", "800")][..],
        ] {
            let pairs: &'static [(&str, &str)] = Box::leak(pairs.to_vec().into_boxed_slice());
            let check = Settings::check_with(Some(file.path()), variables(pairs));
            let error = Settings::load_with(Some(file.path()), variables(pairs)).unwrap_err();
            assert_eq!(
                check.problems[0].error.to_string(),
                error.to_string(),
                "{pairs:?}"
            );
        }
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
        let (_dir, file) = fingerprint_file();
        let overrides = every_option(&file);
        let vars: std::collections::HashMap<_, _> = overrides.env().into_iter().collect();
        assert_eq!(vars.len(), 18, "one variable per option");

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
    fn the_settings_file_names_its_fingerprint_in_a_table_and_a_default_writes_none() {
        let from_toml: Settings =
            toml::from_str("headless = true\n[fingerprint]\nfile = \"profiles/real.json\"\n")
                .unwrap();
        let from_json: Settings =
            serde_json::from_str(r#"{"fingerprint": {"file": "profiles/real.json"}}"#).unwrap();

        assert_eq!(
            from_toml.fingerprint.file.as_deref(),
            Some(Path::new("profiles/real.json"))
        );
        assert_eq!(from_json.fingerprint, from_toml.fingerprint);
        let round_trip: Settings = toml::from_str(&toml::to_string(&from_toml).unwrap()).unwrap();
        assert_eq!(round_trip, from_toml);
        assert!(!toml::to_string(&Settings::default())
            .unwrap()
            .contains("fingerprint"));
        assert!(!serde_json::to_string(&Settings::default())
            .unwrap()
            .contains("fingerprint"));
        assert!(Settings::default().fingerprint.is_empty());
    }

    #[test]
    fn a_misspelt_key_of_the_fingerprint_table_is_an_error() {
        let result = toml::from_str::<Settings>("[fingerprint]\nfle = \"x.json\"\n");

        assert!(result.is_err());
    }

    #[test]
    fn the_fingerprint_variable_beats_the_table_of_the_file() {
        let (dir, from_variable) = fingerprint_file();
        let from_table = dir.path().join("table.json");
        Fingerprint::macos_desktop().save(&from_table).unwrap();
        let file: Settings = toml::from_str(&format!(
            "[fingerprint]\nfile = '{}'\n",
            from_table.display()
        ))
        .unwrap();
        let variable = from_variable.display().to_string();

        let untouched = Settings::apply_overrides(file.clone(), |_| None).unwrap();
        let replaced = Settings::apply_overrides(file, |name| {
            (name == FINGERPRINT_ENV).then(|| variable.clone())
        })
        .unwrap();

        assert_eq!(
            untouched.fingerprint.file.as_deref(),
            Some(from_table.as_path())
        );
        assert_eq!(
            replaced.fingerprint.file.as_deref(),
            Some(from_variable.as_path())
        );
    }

    #[test]
    fn the_command_line_beats_the_variable_and_the_table() {
        let (dir, from_flag) = fingerprint_file();
        let from_variable = dir.path().join("variable.json");
        Fingerprint::linux_desktop().save(&from_variable).unwrap();
        let variable = from_variable.display().to_string();
        let mut settings = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| variable.clone())
        })
        .unwrap();
        assert_eq!(
            settings.fingerprint.file.as_deref(),
            Some(from_variable.as_path())
        );

        SettingsOverrides {
            fingerprint: Some(from_flag.display().to_string()),
            ..SettingsOverrides::default()
        }
        .apply(&mut settings);

        assert_eq!(
            settings.fingerprint.file.as_deref(),
            Some(from_flag.as_path())
        );
    }

    #[test]
    fn the_fingerprint_reaches_the_browser_config() {
        let (_dir, file) = fingerprint_file();
        let settings = Settings {
            fingerprint: FingerprintSettings { file: Some(file) },
            ..Settings::default()
        };

        let config = settings.to_browser_config();

        assert_eq!(config.fingerprint, Some(Fingerprint::windows_desktop()));
        assert_eq!(settings.load_fingerprint().unwrap(), config.fingerprint);
        assert!(Settings::default()
            .to_browser_config()
            .fingerprint
            .is_none());
        assert!(Settings::default().load_fingerprint().unwrap().is_none());
    }

    #[test]
    fn a_fingerprint_file_that_cannot_be_used_stops_the_settings_from_loading() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json").display().to_string();
        let garbage = dir.path().join("garbage.json");
        fs::write(&garbage, "{ \"screen_width\": 1 }").unwrap();
        let garbage = garbage.display().to_string();

        let not_there = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| missing.clone())
        })
        .unwrap_err();
        let not_a_fingerprint = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| garbage.clone())
        })
        .unwrap_err();

        assert!(
            matches!(not_there, SeleniumBaseError::InvalidConfig(_)),
            "{not_there:?}"
        );
        assert!(
            not_there.to_string().contains("missing.json"),
            "{not_there}"
        );
        assert!(
            not_a_fingerprint.to_string().contains("screen_width"),
            "{not_a_fingerprint}"
        );
        assert_eq!(not_there.exit_code(), 2);
    }

    #[test]
    fn a_settings_file_with_a_wrong_fingerprint_path_does_not_load() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[fingerprint]\nfile = \"/no/such/fingerprint.json\"\n")
            .unwrap();

        let error = Settings::from_file(file.path()).unwrap_err();

        assert!(error.to_string().contains("fingerprint.json"), "{error}");
    }

    #[test]
    fn a_blank_fingerprint_variable_counts_as_unset() {
        let settings = with_vars(&[("SB_FINGERPRINT", "   ")]).unwrap();

        assert!(settings.fingerprint.is_empty());
    }

    #[test]
    fn a_settings_made_by_hand_with_a_missing_file_gets_no_fingerprint_and_says_why_when_asked() {
        let settings = Settings {
            fingerprint: FingerprintSettings {
                file: Some(PathBuf::from("/no/such/fingerprint.json")),
            },
            ..Settings::default()
        };

        assert!(settings.to_browser_config().fingerprint.is_none());
        let error = settings.check_fingerprint().unwrap_err();
        assert!(error.to_string().contains("fingerprint.json"), "{error}");
    }

    #[test]
    fn the_override_is_exported_as_its_variable() {
        let only = SettingsOverrides {
            fingerprint: Some("profiles/real.json".to_owned()),
            ..SettingsOverrides::default()
        };

        assert_eq!(
            only.env(),
            [("SB_FINGERPRINT", "profiles/real.json".to_owned())]
        );
    }

    #[test]
    fn the_fingerprint_file_a_command_presents_comes_from_the_flag_then_the_variable_then_the_table(
    ) {
        // Not a valid `Settings` file: `headless` is not a boolean. Only the table is read.
        let body = "headless = \"maybe\"\n[fingerprint]\nfile = \"from-table.json\"\n";
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(body.as_bytes()).unwrap();
        let variable =
            |name: &str| (name == FINGERPRINT_ENV).then(|| "from-variable.json".to_owned());
        let flag = Path::new("from-flag.json");

        let resolved = |flag, var: &dyn Fn(&str) -> Option<String>| {
            Settings::fingerprint_file_with(Some(file.path()), flag, var).unwrap()
        };

        assert_eq!(resolved(Some(flag), &variable).as_deref(), Some(flag));
        assert_eq!(
            resolved(None, &variable).as_deref(),
            Some(Path::new("from-variable.json"))
        );
        assert_eq!(
            resolved(None, &|_| None).as_deref(),
            Some(Path::new("from-table.json"))
        );
        assert_eq!(
            resolved(None, &|_| Some("  ".to_owned())).as_deref(),
            Some(Path::new("from-table.json"))
        );
        // Nothing is named anywhere.
        let mut empty = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        empty.write_all(b"headless = true\n").unwrap();
        assert!(
            Settings::fingerprint_file_with(Some(empty.path()), None, |_| None)
                .unwrap()
                .is_none()
        );
        // A settings file that is not there is an error, as it is for the plugins.
        let error = Settings::fingerprint_file_with(Some("no-such-settings.toml"), None, |_| None)
            .unwrap_err();
        assert!(error.to_string().contains("failed to read"), "{error}");
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

    #[test]
    fn the_start_page_comes_from_the_variable_the_file_or_the_command_line() {
        // Nothing names a page: a browser opens none.
        assert_eq!(Settings::default().start_page, None);
        assert_eq!(Settings::default().to_browser_config().start_page, None);

        // The variable, a blank one counting as unset.
        let from_variable = with_vars(&[("SB_START_PAGE", "https://start.test/")]).unwrap();
        assert_eq!(
            from_variable.start_page.as_deref(),
            Some("https://start.test/")
        );
        assert_eq!(
            from_variable.to_browser_config().start_page.as_deref(),
            Some("https://start.test/")
        );
        assert_eq!(
            with_vars(&[("SB_START_PAGE", "  ")]).unwrap().start_page,
            None
        );

        // The `start_page` key of the settings file, and the variable over it.
        let from_file: Settings = toml::from_str("start_page = \"https://file.test/\"").unwrap();
        assert_eq!(from_file.start_page.as_deref(), Some("https://file.test/"));
        let both = Settings::apply_overrides(from_file, |name| {
            (name == START_PAGE_ENV).then(|| "https://start.test/".to_owned())
        })
        .unwrap();
        assert_eq!(both.start_page.as_deref(), Some("https://start.test/"));

        // The command line over both, and on to the tests as the variable.
        let mut settings = both;
        let flag = SettingsOverrides {
            start_page: Some("https://flag.test/".to_owned()),
            ..SettingsOverrides::default()
        };
        flag.apply(&mut settings);
        assert_eq!(settings.start_page.as_deref(), Some("https://flag.test/"));
        assert_eq!(
            flag.env(),
            [(START_PAGE_ENV, "https://flag.test/".to_owned())]
        );
        // A settings file that does not name one writes none.
        assert!(!toml::to_string(&Settings::default())
            .unwrap()
            .contains("start_page"));
    }
}
