use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::browser::options::{
    CapabilityOverrides, CdpEventCapture, DeviceMetrics, ExtensionArchive, FirefoxOptions,
    LaunchSettings, LaunchSwitch, PageLoadStrategy, ProxyBypassList, WindowPosition, WindowSize,
};
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::Fingerprint;

/// Supported browser types.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Browser {
    #[default]
    Chrome,
    Chromium,
    Edge,
    Firefox,
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
    pub ad_block: bool,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub start_page: Option<String>,
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

    pub fn with_reuse_session(mut self, reuse: bool) -> Self {
        self.reuse_session = reuse;
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

    /// Returns true when the configured fingerprint requests native-level
    /// (binary + CDP) spoofing.
    pub fn native_spoofing_enabled(&self) -> bool {
        self.fingerprint
            .as_ref()
            .is_some_and(|fp| fp.flags.native_spoofing)
    }
}
