use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::browser::options::{
    CapabilityOverrides, CdpEventCapture, DeviceMetrics, DriverVersion, ExtensionArchive,
    FirefoxOptions, LaunchSettings, LaunchSwitch, PageLoadStrategy, ProxyBypassList,
    VirtualDisplay, WindowPosition, WindowSize,
};
use crate::browser::reuse::ReuseClass;
use crate::config::ad_block_list::AdBlockList;
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::Fingerprint;

/// Supported browser types.
///
/// The names Python SeleniumBase uses for them (`--brave`, `--cft`, `--chs`,
/// `--ie`, ...) are read by [`FromStr`] and written by [`Display`].
///
/// [`FromStr`]: std::str::FromStr
/// [`Display`]: std::fmt::Display
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
}

impl Browser {
    /// Whether the browser is Chromium-based, which is what the CDP and UC modes
    /// and the Chrome launch arguments need.
    #[must_use]
    pub const fn is_chromium_based(self) -> bool {
        !matches!(self, Self::Firefox | Self::Safari | Self::InternetExplorer)
    }

    /// The name Python SeleniumBase gives the browser (`--browser=NAME`).
    #[must_use]
    pub const fn name(self) -> &'static str {
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
        }
    }
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
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
            _ => {
                return Err(SeleniumBaseError::parse(
                    text,
                    "a browser: chrome, chromium, edge, firefox, brave, comet, opera, safari, ie, \
                     cft or chs",
                ))
            }
        })
    }
}

/// Driver execution mode.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum DriverMode {
    #[default]
    WebDriver,
    Cdp,
    Uc,
}

/// Configuration for a browser session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BrowserConfig {
    pub webdriver_url: String,
    pub browser: Browser,
    pub headless: bool,
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub locale: Option<String>,
    /// The `--ad-block` setting: refuse the requests of advertising and
    /// tracking hosts.
    ///
    /// A Chromium WebDriver session (Chrome, Chromium, Edge, Brave, Comet,
    /// Opera, Chrome for Testing, `chrome-headless-shell`) sends the hosts of
    /// an [`AdBlockList`] to the browser with `Network.setBlockedURLs` once it
    /// is connected: the built-in list, or the one set with
    /// [`with_ad_block_list`](Self::with_ad_block_list). The block belongs to
    /// a window, so the session repeats it for each window it switches to with
    /// `switch_to_window` or `switch_to_new_window`; a window reached some
    /// other way needs
    /// [`BrowserSession::apply_ad_block`](crate::browser::session::BrowserSession::apply_ad_block).
    ///
    /// What it does not reach, as seen in Chrome 155 (`tests/ad_block_chrome.rs`):
    /// a URL that names a port is not matched, and a frame of another site is
    /// not stopped from loading, nor are its own requests blocked. A frame of
    /// the page's own site is blocked like the page.
    ///
    /// Firefox, Safari and Internet Explorer have no DevTools channel through
    /// WebDriver, so the setting blocks nothing there (a warning says so).
    /// The pure CDP engine blocks hosts with `LaunchOptionsBuilder::ad_block`,
    /// and `BaseCase::ad_block` removes ad elements from a page that loaded.
    pub ad_block: bool,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub start_page: Option<String>,
    /// Python's `--reuse-session` (`--rs`): the tests of a run share browsers.
    ///
    /// With it on, [`BrowserSession::connect`](crate::BrowserSession::connect)
    /// (and so `BaseCase::new`) attaches to a browser that an earlier test
    /// started with the same configuration and has given back, after cleaning
    /// it (other windows closed, cookies and the page's storage deleted,
    /// `about:blank`), and `BaseCase::quit` gives the browser back instead of
    /// closing it. One test holds a browser at a time; a test that starts while
    /// every matching browser is held gets one of its own, kept for reuse too.
    /// Idle browsers close after
    /// [`reuse::idle_timeout`](crate::browser::reuse::idle_timeout) or at
    /// [`shutdown_reused_sessions`](crate::shutdown_reused_sessions). The
    /// `sbrs` flag, `Settings` and `SB_REUSE_SESSION` set it;
    /// [`with_reuse_class_session`](Self::with_reuse_class_session) and
    /// [`with_reuse_module_session`](Self::with_reuse_module_session) narrow
    /// who shares (`--rcs`). See [`crate::browser::reuse`] for how it works and
    /// what differs from Python.
    pub reuse_session: bool,
    pub mobile: bool,
    pub threads: Option<usize>,
    pub auto_start_driver: bool,
    /// Extra Chromium/Edge command-line arguments supplied by callers such as
    /// external profile payload integrations.
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Optional anti-detection fingerprint profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,
    /// Optional explicit path to the browser binary (Chrome/Chromium/Edge).
    /// When `native_spoofing` is enabled and this is unset, the crate will
    /// locate the system Chrome binary, patch a cached copy, and set this
    /// field automatically before launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_binary_path: Option<PathBuf>,
    /// Client-side timeout, in seconds, for each HTTP request sent to a Remote
    /// WebDriver server such as a Selenium Grid.
    ///
    /// `None` leaves the WebDriver client's own default in place. Set it
    /// higher than the page-load timeout and higher than the time a new-session
    /// request may sit in the grid's queue, otherwise healthy sessions will be
    /// cut off. Its purpose is to stop a command blocking forever when a grid
    /// node dies mid-session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_webdriver_timeout_seconds: Option<u64>,
    /// The options that shape how the browser and its driver are started:
    /// preferences, page load strategy, window geometry, mobile emulation,
    /// extensions, Firefox settings, extra capabilities, driver version and
    /// more. Set them with the `with_*` methods below; in a config file they
    /// sit next to the fields above.
    #[serde(default, flatten)]
    pub launch: LaunchSettings,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self::from_runtime_config(&crate::config::RuntimeConfig::default())
    }
}

impl BrowserConfig {
    /// Build a `BrowserConfig` from the current process environment.
    ///
    /// This is the preferred constructor for deployable applications because
    /// it honors `SB_WEBDRIVER_URL`, `SB_CHROME_BIN`, and other runtime
    /// variables without requiring code changes per environment.
    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        let runtime = crate::config::RuntimeConfig::from_env()?;
        Ok(Self::from_runtime_config(&runtime))
    }

    fn from_runtime_config(runtime: &crate::config::RuntimeConfig) -> Self {
        Self {
            webdriver_url: runtime.webdriver_url.clone(),
            browser: Browser::Chrome,
            headless: true,
            mode: DriverMode::WebDriver,
            user_agent: None,
            locale: None,
            ad_block: false,
            proxy: None,
            proxy_pac_url: None,
            user_data_dir: None,
            extension_dir: None,
            start_page: None,
            reuse_session: false,
            mobile: false,
            threads: None,
            auto_start_driver: true,
            extra_args: Vec::new(),
            fingerprint: None,
            browser_binary_path: runtime.chrome_bin.clone(),
            remote_webdriver_timeout_seconds: runtime.remote_webdriver_timeout.map(|d| d.as_secs()),
            launch: LaunchSettings::default(),
        }
    }
}

impl BrowserConfig {
    pub fn with_mode(mut self, mode: DriverMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_browser(mut self, browser: Browser) -> Self {
        self.browser = browser;
        self
    }

    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn with_webdriver_url(mut self, url: impl Into<String>) -> Self {
        self.webdriver_url = url.into();
        self
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    pub fn with_proxy_pac_url(mut self, url: impl Into<String>) -> Self {
        self.proxy_pac_url = Some(url.into());
        self
    }

    pub fn with_user_data_dir(mut self, dir: impl Into<String>) -> Self {
        self.user_data_dir = Some(dir.into());
        self
    }

    pub fn with_extension_dir(mut self, dir: impl Into<String>) -> Self {
        self.extension_dir = Some(dir.into());
        self
    }

    pub fn with_start_page(mut self, page: impl Into<String>) -> Self {
        self.start_page = Some(page.into());
        self
    }

    /// Shares browsers among the tests of the run (`--reuse-session`, `--rs`).
    /// See [`reuse_session`](Self::reuse_session).
    pub fn with_reuse_session(mut self, reuse: bool) -> Self {
        self.reuse_session = reuse;
        self
    }

    /// Shares a browser only among the tests that pass the same `class`
    /// (Python's `--reuse-class-session` for the tests of one class). It turns
    /// [`reuse_session`](Self::reuse_session) on.
    pub fn with_reuse_class_session(mut self, class: impl Into<String>) -> Self {
        self.reuse_session = true;
        self.launch.reuse_class = Some(ReuseClass::Named(class.into()));
        self
    }

    /// Shares a browser among the tests of one Rust module (`--rcs`,
    /// `--reuse-class-session`, `SB_REUSE_CLASS_SESSION`): the module plays the
    /// part of Python's test class, and is read from the name of the thread
    /// the test runs on ([`ReuseClass::TestModule`]).
    ///
    /// On, it turns [`reuse_session`](Self::reuse_session) on as well. Off, it
    /// removes a per-module setting and leaves `reuse_session` as it is.
    pub fn with_reuse_module_session(mut self, on: bool) -> Self {
        if on {
            self.reuse_session = true;
            self.launch.reuse_class = Some(ReuseClass::TestModule);
        } else if self.launch.reuse_class == Some(ReuseClass::TestModule) {
            self.launch.reuse_class = None;
        }
        self
    }

    /// Turns [`ad_block`](Self::ad_block) on or off (`--ad-block`).
    pub fn with_ad_block(mut self, on: bool) -> Self {
        self.ad_block = on;
        self
    }

    /// Blocks the hosts of `list` instead of the built-in list, and turns
    /// [`ad_block`](Self::ad_block) on. Set `ad_block` to `false` afterwards to
    /// keep the list but block nothing.
    pub fn with_ad_block_list(mut self, list: AdBlockList) -> Self {
        self.launch.ad_block_list = Some(list);
        self.ad_block = true;
        self
    }

    pub fn with_mobile(mut self, mobile: bool) -> Self {
        self.mobile = mobile;
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }

    /// Presents `fingerprint` in the browser.
    ///
    /// Its launch arguments go to every Chromium-based session. In the
    /// [`Cdp`](DriverMode::Cdp) and [`Uc`](DriverMode::Uc) modes the session
    /// also installs its page script (the
    /// [session script](crate::stealth::evasions::session_script)) and sends
    /// its protocol overrides through DevTools; the plain
    /// [`WebDriver`](DriverMode::WebDriver) mode sends the launch arguments only.
    /// What a fingerprint switches on is decided by the settings it holds and
    /// its [`policy`](Fingerprint::policy); see [`Fingerprint::is_masked`].
    pub fn with_fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    pub fn with_browser_binary_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.browser_binary_path = Some(path.into());
        self
    }

    pub fn is_cdp_enabled(&self) -> bool {
        self.mode == DriverMode::Cdp || self.mode == DriverMode::Uc
    }

    pub fn is_uc_enabled(&self) -> bool {
        self.mode == DriverMode::Uc
    }

    pub fn is_default_webdriver_url(&self) -> bool {
        self.webdriver_url == "http://localhost:4444" || self.webdriver_url.is_empty()
    }

    pub fn with_extra_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.extra_args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn push_extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Replaces all the [`LaunchSettings`] at once.
    pub fn with_launch_settings(mut self, launch: LaunchSettings) -> Self {
        self.launch = launch;
        self
    }

    /// Turns one of the on/off launch settings on or off.
    pub fn with_switch(mut self, switch: LaunchSwitch, on: bool) -> Self {
        self.launch.set(switch, on);
        self
    }

    /// Do not load images (`--block-images`).
    ///
    /// Sets the browser's own content-setting preference, so it also holds for
    /// images that a page requests later.
    pub fn with_block_images(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::BlockImages, on)
    }

    /// Turn JavaScript off (`--disable-js`). Most pages break without it.
    pub fn with_disable_js(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DisableJavaScript, on)
    }

    /// Refuse every cookie (`--disable-cookies`).
    pub fn with_disable_cookies(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DisableCookies, on)
    }

    /// Send the `DNT: 1` header (`--do-not-track`).
    pub fn with_do_not_track(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DoNotTrack, on)
    }

    /// Download PDF files instead of showing them in the browser
    /// (`--external-pdf`).
    pub fn with_external_pdf(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::ExternalPdf, on)
    }

    /// Leave Chrome Sync on (`--enable-sync`); sessions switch it off
    /// otherwise.
    pub fn with_enable_sync(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::EnableSync, on)
    }

    /// Strip the Content Security Policy from every response
    /// (`--disable-csp`, `--dcsp`, `--no-csp`). Chromium only.
    pub fn with_disable_csp(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DisableCsp, on)
    }

    /// Keep Chrome's automation extension (`--use-auto-ext`, `--auto-ext`).
    /// Upstream marks it deprecated. Chromium only.
    pub fn with_automation_extension(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::AutomationExtension, on)
    }

    /// Load the SeleniumBase Recorder extension (`recorder_ext`). Chromium
    /// only.
    pub fn with_recorder_extension(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::RecorderExtension, on)
    }

    /// Fetch a driver, or a Chrome build, that has to be downloaded through
    /// [`proxy`](Self::proxy) (`--proxy-driver`).
    pub fn with_proxy_for_driver_download(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::ProxyForDriverDownload, on)
    }

    /// Let the person end the current test by pressing ESC in the browser
    /// (`--esc-end`, `--esc_end`, `--ee`).
    ///
    /// `BaseCase` puts a key listener in the page and, before it clicks,
    /// highlights, waits for a page to be ready or waits for an element to be
    /// visible, looks whether ESC was pressed; if so the call returns
    /// [`SeleniumBaseError::Skipped`], which `run_browser_test` treats as a
    /// skipped test and not a failure.
    pub fn with_esc_end(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::EscEnd, on)
    }

    /// Whether pressing ESC in the browser ends the test
    /// ([`with_esc_end`](Self::with_esc_end)).
    #[must_use]
    pub fn esc_end(&self) -> bool {
        self.launch.has(LaunchSwitch::EscEnd)
    }

    /// Write the recording as a program for the pure CDP engine instead of one
    /// for `BaseCase` (`--rec-sb-cdp`, `rec_sb_cdp`).
    ///
    /// With it on, [`BaseCase::export_recording_as_rust`](crate::BaseCase::export_recording_as_rust)
    /// (and so `save_recording_to_logs`, `sbrs run-scenario` and
    /// `sbrs export-recording`) returns
    /// [`ActionRecorder::to_cdp_script`](crate::api::recorder::ActionRecorder::to_cdp_script).
    /// The session itself still runs on WebDriver; only the generated code
    /// changes.
    pub fn with_rec_sb_cdp(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::RecSbCdp, on)
    }

    /// Whether recordings are written as pure CDP code
    /// ([`with_rec_sb_cdp`](Self::with_rec_sb_cdp)).
    #[must_use]
    pub fn rec_sb_cdp(&self) -> bool {
        self.launch.has(LaunchSwitch::RecSbCdp)
    }

    /// How long a navigation waits (`--page-load-strategy`, `--pls`).
    pub fn with_page_load_strategy(mut self, strategy: PageLoadStrategy) -> Self {
        self.launch.page_load_strategy = Some(strategy);
        self
    }

    /// The size of the browser window (`--window-size`, `size`).
    pub fn with_window_size(mut self, size: WindowSize) -> Self {
        self.launch.window_size = Some(size);
        self
    }

    /// The position of the browser window (`--window-position`, `position`).
    pub fn with_window_position(mut self, position: WindowPosition) -> Self {
        self.launch.window_position = Some(position);
        self
    }

    /// Emulate a phone with this screen (`--metrics`, `d_width`, `d_height`,
    /// `d_p_r`). It turns mobile emulation on, with the user agent from
    /// [`user_agent`](Self::user_agent) or, without one, an Android one.
    pub fn with_device_metrics(mut self, metrics: DeviceMetrics) -> Self {
        self.launch.device_metrics = Some(metrics);
        self.mobile = true;
        self
    }

    /// Hosts that skip the proxy (`--proxy-bypass-list`).
    pub fn with_proxy_bypass_list(mut self, hosts: ProxyBypassList) -> Self {
        self.launch.proxy_bypass_list = hosts;
        self
    }

    /// Load a Chromium extension from a `.zip` or `.crx` file
    /// (`--extension-zip`, `--crx`). May be called more than once.
    pub fn with_extension_archive(mut self, archive: ExtensionArchive) -> Self {
        if !self.launch.extension_archives.contains(&archive) {
            self.launch.extension_archives.push(archive);
        }
        self
    }

    /// Firefox arguments and preferences (`--firefox-arg`, `--firefox-pref`).
    pub fn with_firefox_options(mut self, firefox: FirefoxOptions) -> Self {
        self.launch.firefox = firefox;
        self
    }

    /// Capabilities to add to the session's (`--cap-string`, `--cap-file`).
    pub fn with_capabilities(mut self, capabilities: CapabilityOverrides) -> Self {
        self.launch.capabilities = capabilities;
        self
    }

    /// Which chromedriver to run (`--driver-version`).
    pub fn with_driver_version(mut self, version: DriverVersion) -> Self {
        self.launch.driver_version = Some(version);
        self
    }

    /// Open a headed browser on a virtual screen on Linux (`--xvfb`,
    /// `--xvfb-metrics`).
    pub fn with_virtual_display(mut self, display: VirtualDisplay) -> Self {
        self.launch.virtual_display = Some(display);
        self
    }

    /// Open Chrome's remote debugger on `port`. Anyone on the machine who can
    /// reach the port can drive the browser, so use it for debugging only.
    pub fn with_remote_debug_port(mut self, port: u16) -> Self {
        self.launch.remote_debug_port = Some(port);
        self
    }

    /// Open Chrome's remote debugger on port 9222, or close it again
    /// (`--remote-debug`, `--remote-debugger`). Debugger at
    /// `http://localhost:9222`.
    pub fn with_remote_debug(mut self, on: bool) -> Self {
        self.launch.remote_debug_port =
            on.then_some(crate::stealth::options::DEFAULT_REMOTE_DEBUG_PORT);
        self
    }

    /// Log the DevTools Protocol events of Chromium sessions
    /// (`--log-cdp-events`, `--uc-cdp-events`).
    pub fn with_cdp_events(mut self, capture: CdpEventCapture) -> Self {
        self.launch.cdp_events = capture;
        self
    }

    /// The browser plugins (feature `plugins`, ADR 004) whose hooks run for
    /// every session started with this configuration.
    ///
    /// [`BrowserSession::connect`](crate::BrowserSession::connect), and so
    /// `BaseCase::new`, checks the set against the browser, lets
    /// `before_launch` change the start-up settings (arguments, preferences,
    /// extensions, proxy, headless) before the browser starts, then calls
    /// `on_browser` and gives the first window the packages and
    /// `on_page_created`; quitting calls `on_close`. A refusal or a failing
    /// hook stops the session with [`SeleniumBaseError::Plugin`]. What each
    /// browser can do is in the Browser Plugins guide: Chromium browsers get
    /// the DevTools Protocol, Firefox and Safari do not.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::plugin_api::PluginSet;
    /// use seleniumbase_rs::{BaseCase, BrowserConfig};
    ///
    /// # async fn demo(plugins: PluginSet) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sb = BaseCase::new(BrowserConfig::default().with_plugins(plugins)).await?;
    /// sb.open("https://example.com").await?;
    /// sb.quit().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "plugins")]
    pub fn with_plugins(mut self, plugins: seleniumbase_rs_plugin::PluginSet) -> Self {
        self.launch.plugins = plugins.into();
        self
    }

    /// Adds the built-in plugins called `names` (feature `plugins`) to those
    /// of [`with_plugins`](Self::with_plugins).
    ///
    /// The names are the cargo features `plugin-shield`,
    /// `plugin-block-resources` and `plugin-crx` (`plugins-builtin` for all);
    /// each plugin gets its default options. Use
    /// [`with_plugin_selection`](Self::with_plugin_selection) to give options.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for a name this build does not have (the
    /// message lists the ones it has), or a plugin that is in the set already.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let error = BrowserConfig::default()
    ///     .with_plugins_named(["no-such-plugin"])
    ///     .unwrap_err();
    /// assert!(error.to_string().contains("no-such-plugin"));
    /// ```
    #[cfg(feature = "plugins")]
    pub fn with_plugins_named<I, S>(self, names: I) -> Result<Self, SeleniumBaseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let selection = names.into_iter().fold(
            crate::config::plugins::PluginSelection::new(),
            |selection, name| selection.enable(name),
        );
        self.with_plugin_selection(&selection)
    }

    /// Adds the plugins a [`PluginSelection`](crate::config::plugins::PluginSelection)
    /// names, with their options (feature `plugins`), to those of
    /// [`with_plugins`](Self::with_plugins): the `[plugins]` table of the
    /// settings file and `SB_PLUGINS`, as `Settings::plugins` holds them.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for an unknown name, options a plugin
    /// refuses, or a plugin that is in the set already.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::config::settings::Settings;
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// # fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let settings = Settings::load(None::<&str>)?;
    /// let config = settings
    ///     .to_browser_config()
    ///     .with_plugin_selection(&settings.plugins)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "plugins")]
    pub fn with_plugin_selection(
        mut self,
        selection: &crate::config::plugins::PluginSelection,
    ) -> Result<Self, SeleniumBaseError> {
        let set = crate::plugin_host::add_selection(
            self.launch.plugins.set().clone(),
            &crate::plugin_host::builtin_registry(),
            selection,
        )?;
        self.launch.plugins = set.into();
        Ok(self)
    }

    /// The browser plugins set with [`with_plugins`](Self::with_plugins).
    #[cfg(feature = "plugins")]
    #[must_use]
    pub fn plugins(&self) -> &seleniumbase_rs_plugin::PluginSet {
        self.launch.plugins.set()
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

    #[cfg(feature = "plugins")]
    #[test]
    fn an_unknown_plugin_name_is_refused_when_it_is_added() {
        let error = BrowserConfig::default()
            .with_plugins_named(["no-such-plugin"])
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

        let config = BrowserConfig::default()
            .with_plugins(PluginSet::new().with(Mine).unwrap())
            .with_plugins_named(["shield"])
            .unwrap();
        assert_eq!(config.plugins().names(), ["mine", "shield"]);

        let again = config
            .with_plugin_selection(&PluginSelection::new().enable("shield"))
            .unwrap_err();
        assert!(again.to_string().contains("already in the set"), "{again}");
    }

    #[test]
    fn reuse_is_off_until_asked_for() {
        let config = BrowserConfig::default();
        assert!(!config.reuse_session);
        assert_eq!(config.launch.reuse_class, None);
    }

    #[test]
    fn a_class_session_is_a_reused_session_for_one_class() {
        let config = BrowserConfig::default().with_reuse_class_session("Login");
        assert!(config.reuse_session);
        assert_eq!(
            config.launch.reuse_class,
            Some(ReuseClass::Named("Login".to_owned()))
        );
    }

    #[test]
    fn the_module_flag_turns_reuse_on_and_comes_off_alone() {
        let on = BrowserConfig::default().with_reuse_module_session(true);
        assert!(on.reuse_session);
        assert_eq!(on.launch.reuse_class, Some(ReuseClass::TestModule));
        let off = on.with_reuse_module_session(false);
        assert!(off.reuse_session, "--rs stays on");
        assert_eq!(off.launch.reuse_class, None);
        let named = BrowserConfig::default()
            .with_reuse_class_session("Login")
            .with_reuse_module_session(false);
        assert_eq!(
            named.launch.reuse_class,
            Some(ReuseClass::Named("Login".to_owned())),
            "a named class is not the module flag"
        );
    }

    #[test]
    fn the_reuse_class_round_trips_through_a_config_file() {
        let config = BrowserConfig::default().with_reuse_class_session("Login");
        let text = serde_json::to_string(&config).unwrap();
        assert!(
            text.contains(r#""reuse_class":{"named":"Login"}"#),
            "{text}"
        );
        let back: BrowserConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back, config);
        let module = BrowserConfig::default().with_reuse_module_session(true);
        let text = serde_json::to_string(&module).unwrap();
        assert!(text.contains(r#""reuse_class":"test_module""#), "{text}");
    }

    const ALL: [Browser; 11] = [
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
                browser.name().parse::<Browser>().unwrap(),
                browser,
                "{browser:?}"
            );
            assert_eq!(browser.to_string(), browser.name());
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
            ("Internet Explorer", Browser::InternetExplorer),
            (" CHROME ", Browser::Chrome),
        ] {
            assert_eq!(name.parse::<Browser>().unwrap(), browser, "{name}");
        }
        assert!("netscape".parse::<Browser>().is_err());
        assert!("".parse::<Browser>().is_err());
    }

    #[test]
    fn only_firefox_safari_and_ie_are_not_chromium_based() {
        let others: Vec<_> = ALL.into_iter().filter(|b| !b.is_chromium_based()).collect();
        assert_eq!(
            others,
            [Browser::Firefox, Browser::Safari, Browser::InternetExplorer]
        );
    }

    #[test]
    fn the_default_browser_is_chrome() {
        assert_eq!(Browser::default(), Browser::Chrome);
        assert_eq!(BrowserConfig::default().browser, Browser::Chrome);
    }

    #[test]
    fn a_default_config_writes_no_launch_settings_and_a_set_one_sits_beside_the_rest() {
        let plain = serde_json::to_value(BrowserConfig::default()).unwrap();
        assert!(plain.get("switches").is_none(), "{plain}");
        assert!(plain.get("page_load_strategy").is_none(), "{plain}");

        let config = BrowserConfig::default()
            .with_browser(Browser::Brave)
            .with_block_images(true)
            .with_page_load_strategy(PageLoadStrategy::Eager);
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["browser"], "Brave");
        assert_eq!(json["switches"], serde_json::json!(["block-images"]));
        assert_eq!(json["page_load_strategy"], "eager");
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn a_config_written_before_launch_settings_existed_still_reads() {
        let mut old = serde_json::to_value(BrowserConfig::default()).unwrap();
        old.as_object_mut().unwrap().remove("launch");
        let config: BrowserConfig = serde_json::from_value(old).unwrap();
        assert_eq!(config.launch, LaunchSettings::default());
    }

    #[test]
    fn device_metrics_turn_mobile_on() {
        let config =
            BrowserConfig::default().with_device_metrics(DeviceMetrics::new(360, 640, 2.0));
        assert!(config.mobile);
        assert_eq!(
            config.launch.device_metrics,
            Some(DeviceMetrics::new(360, 640, 2.0))
        );
    }

    #[test]
    fn esc_end_is_off_until_it_is_asked_for() {
        assert!(!BrowserConfig::default().esc_end());
        let on = BrowserConfig::default().with_esc_end(true);
        assert!(on.esc_end());
        assert!(on.launch.has(LaunchSwitch::EscEnd));
        assert!(!on.with_esc_end(false).esc_end());
    }

    #[test]
    fn rec_sb_cdp_is_off_until_it_is_asked_for_and_is_written_as_a_switch() {
        assert!(!BrowserConfig::default().rec_sb_cdp());
        let on = BrowserConfig::default().with_rec_sb_cdp(true);
        assert!(on.rec_sb_cdp());
        assert!(!on.esc_end(), "the switches are independent");
        let json = serde_json::to_value(&on).unwrap();
        assert_eq!(json["switches"], serde_json::json!(["rec-sb-cdp"]));
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert!(back.rec_sb_cdp());
        assert!(!on.with_rec_sb_cdp(false).rec_sb_cdp());
    }

    #[test]
    fn esc_end_is_written_beside_the_other_switches() {
        let config = BrowserConfig::default().with_esc_end(true);
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["switches"], serde_json::json!(["esc-end"]));
        let back: BrowserConfig = serde_json::from_value(json).unwrap();
        assert!(back.esc_end());
    }

    #[test]
    fn the_remote_debugger_has_a_default_port_and_can_be_closed() {
        let on = BrowserConfig::default().with_remote_debug(true);
        assert_eq!(on.launch.remote_debug_port, Some(9222));
        assert_eq!(on.with_remote_debug(false).launch.remote_debug_port, None);
    }
}
