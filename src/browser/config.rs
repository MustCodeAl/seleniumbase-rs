use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::browser::options::{
    CapabilityOverrides, CdpEventCapture, DeviceMetrics, DriverVersion, ExtensionArchive,
    FirefoxOptions, LaunchSettings, LaunchSwitch, PageLoadStrategy, ProxyBypassList,
    VirtualDisplay, WindowPosition, WindowSize,
};
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
    /// The `--ad-block` setting. In a WebDriver launch this only turns images
    /// off (`--blink-settings=imagesEnabled=false`); it blocks no ads. Ad hosts
    /// are blocked by the pure CDP engine's `LaunchOptionsBuilder::ad_block`,
    /// and `BaseCase::ad_block` removes ad elements from a page that loaded.
    pub ad_block: bool,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub start_page: Option<String>,
    /// Python's `--reuse-session`: share one browser across the whole run.
    ///
    /// **Not implemented.** The value is stored and passed through (the
    /// `sbase` flag, `Settings` and `SB_REUSE_SESSION` set it), but nothing
    /// reads it: every `BaseCase::new` starts its own browser. `parity/api.toml`
    /// lists the option as planned.
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

    /// Sets [`reuse_session`](Self::reuse_session), which nothing acts on yet.
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

    /// Fetch a driver, or a Chrome build, that has to be downloaded through
    /// [`proxy`](Self::proxy) (`--proxy-driver`).
    pub fn with_proxy_for_driver_download(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::ProxyForDriverDownload, on)
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

    /// Returns true when the configured fingerprint requests native-level
    /// (binary + CDP) spoofing.
    pub fn native_spoofing_enabled(&self) -> bool {
        self.fingerprint
            .as_ref()
            .is_some_and(|fp| fp.flags.native_spoofing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_remote_debugger_has_a_default_port_and_can_be_closed() {
        let on = BrowserConfig::default().with_remote_debug(true);
        assert_eq!(on.launch.remote_debug_port, Some(9222));
        assert_eq!(on.with_remote_debug(false).launch.remote_debug_port, None);
    }
}
