use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::api::run_settings::RunSettings;
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
    /// A WebDriver server that already runs the browser, such as a Selenium
    /// Grid or a cloud provider (`--browser=remote`). The session asks for
    /// nothing but the capabilities of
    /// [`with_capabilities`](BrowserConfig::with_capabilities) (`--cap-file`,
    /// `--cap-string`), so the other browser options (mobile mode, switches,
    /// window geometry) do not apply to it. It needs a server that is not the
    /// local default: see [`with_webdriver_url`](BrowserConfig::with_webdriver_url)
    /// and [`with_grid`](BrowserConfig::with_grid).
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
            Self::Remote => "remote",
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
pub enum DriverMode {
    #[default]
    WebDriver,
    Cdp,
    Uc,
}

impl DriverMode {
    /// The name `Settings::mode`, `SB_MODE` and `sbrs` use for the mode.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WebDriver => "webdriver",
            Self::Cdp => "cdp",
            Self::Uc => "uc",
        }
    }
}

impl std::fmt::Display for DriverMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
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
    /// it (other windows closed, the page's storage deleted, cookies deleted if
    /// [`crumbs`](crate::browser::reuse::crumbs) are on, `about:blank`), and `BaseCase::quit` gives the browser back instead of
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
        use crate::stealth::options::without_credentials;
        f.debug_struct("BrowserConfig")
            .field("webdriver_url", &without_credentials(&self.webdriver_url))
            .field("browser", &self.browser)
            .field("headless", &self.headless)
            .field("mode", &self.mode)
            .field("user_agent", &self.user_agent)
            .field("locale", &self.locale)
            .field("ad_block", &self.ad_block)
            .field("proxy", &self.proxy.as_deref().map(without_credentials))
            .field(
                "proxy_pac_url",
                &self.proxy_pac_url.as_deref().map(without_credentials),
            )
            .field("user_data_dir", &self.user_data_dir)
            .field("extension_dir", &self.extension_dir)
            .field("start_page", &self.start_page)
            .field("reuse_session", &self.reuse_session)
            .field("mobile", &self.mobile)
            .field("threads", &self.threads)
            .field("auto_start_driver", &self.auto_start_driver)
            .field("extra_args", &self.extra_args)
            .field("fingerprint", &self.fingerprint)
            .field("browser_binary_path", &self.browser_binary_path)
            .field(
                "remote_webdriver_timeout_seconds",
                &self.remote_webdriver_timeout_seconds,
            )
            .field("launch", &self.launch)
            .finish()
    }
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
    /// How the browser is driven: WebDriver, DevTools without a driver
    /// binary, or the undetected mode (`--cdp`, `--uc`).
    pub fn with_mode(mut self, mode: DriverMode) -> Self {
        self.mode = mode;
        self
    }

    /// Which browser to start (`--browser`).
    pub fn with_browser(mut self, browser: Browser) -> Self {
        self.browser = browser;
        self
    }

    /// Runs the browser without a window (`--headless`); `false` opens one
    /// (`--headed`). A default configuration is headless.
    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    /// The WebDriver server to connect to, such as a Selenium Grid
    /// (`--server`, `--port`). The default is `http://localhost:4444`, or
    /// `SB_WEBDRIVER_URL` for [`from_env`](Self::from_env).
    pub fn with_webdriver_url(mut self, url: impl Into<String>) -> Self {
        self.webdriver_url = url.into();
        self
    }

    /// The user-agent string the browser sends (`--agent`, `--user-agent`).
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// The browser's language, such as `fr-FR` (`--locale`).
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Routes traffic through a proxy written
    /// `[scheme://][user:password@]host:port` (`--proxy`).
    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// A proxy auto-config script the browser fetches to choose its proxy
    /// (`--proxy-pac-url`).
    pub fn with_proxy_pac_url(mut self, url: impl Into<String>) -> Self {
        self.proxy_pac_url = Some(url.into());
        self
    }

    /// A browser profile folder that keeps cookies and settings between runs
    /// (`--user-data-dir`).
    pub fn with_user_data_dir(mut self, dir: impl Into<String>) -> Self {
        self.user_data_dir = Some(dir.into());
        self
    }

    /// An unpacked browser extension to load (`--extension-dir`).
    pub fn with_extension_dir(mut self, dir: impl Into<String>) -> Self {
        self.extension_dir = Some(dir.into());
        self
    }

    /// A page the browser opens as soon as it starts (`--start-page`).
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

    /// Names the program of the browser (Python's `binary_location`,
    /// `--binary-location`). The same as
    /// [`with_browser_binary_path`](Self::with_browser_binary_path).
    pub fn with_binary_location(self, path: impl Into<PathBuf>) -> Self {
        self.with_browser_binary_path(path)
    }

    /// Sends the session to a Selenium Grid: the WebDriver URL Python
    /// SeleniumBase builds from `--protocol`, `--server` and `--port`
    /// (see [`grid_url`](crate::browser::options::grid_url)).
    ///
    /// A `servername` of `localhost` leaves the configuration as it is, so the
    /// local driver runs, as in Python.
    ///
    /// # Errors
    ///
    /// Returns an error when `servername` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::browser::options::GridProtocol;
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let config = BrowserConfig::default().with_grid(
    ///     GridProtocol::Http,
    ///     "USER:KEY@hub.browserstack.com",
    ///     80,
    /// )?;
    /// assert_eq!(
    ///     config.webdriver_url,
    ///     "http://USER:KEY@hub.browserstack.com/wd/hub"
    /// );
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn with_grid(
        mut self,
        protocol: crate::browser::options::GridProtocol,
        servername: &str,
        port: u16,
    ) -> Result<Self, SeleniumBaseError> {
        if servername.trim() != "localhost" {
            self.webdriver_url = crate::browser::options::grid_url(protocol, servername, port)?;
        }
        Ok(self)
    }

    /// Checks the settings that can be refused before anything starts: the
    /// proxy and the PAC URL, a login on a Firefox proxy, and `--browser=remote`
    /// without capabilities or a server. A session checks them itself when it
    /// connects; call this to fail early.
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
    /// assert!(BrowserConfig::default().with_proxy("user:pw@proxy.test:8080").validate().is_ok());
    /// let error = BrowserConfig::default().with_proxy("user:pw@proxy.test").validate();
    /// assert!(error.is_err());
    /// ```
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        use crate::browser::options::{PacUrl, ProxyAddress};
        let proxy = self
            .proxy
            .as_deref()
            .map(str::parse::<ProxyAddress>)
            .transpose()?;
        let pac = self
            .proxy_pac_url
            .as_deref()
            .map(str::parse::<PacUrl>)
            .transpose()?;
        let has_login = proxy.is_some_and(|proxy| proxy.credentials().is_some())
            || pac.is_some_and(|pac| pac.credentials().is_some());
        if has_login && self.browser == Browser::Firefox {
            return Err(SeleniumBaseError::invalid_config(FIREFOX_PROXY_LOGIN));
        }
        if self.browser == Browser::Remote {
            if self.launch.capabilities.is_empty() {
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
    /// (and so `save_recording_to_logs` and `sbrs run-scenario`)
    /// returns
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

    /// The client-side timeout of each request sent to a Remote WebDriver
    /// server such as a Selenium Grid (`SB_REMOTE_WEBDRIVER_TIMEOUT`). A part of
    /// a second rounds up to a whole second. See
    /// [`remote_webdriver_timeout_seconds`](Self::remote_webdriver_timeout_seconds).
    #[must_use]
    pub fn with_remote_webdriver_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.remote_webdriver_timeout_seconds = Some(whole_seconds(timeout));
        self
    }

    /// The run-wide settings the test starts with (Python's `SB(demo=, slow=,
    /// demo_sleep=, message_duration=)`).
    ///
    /// They cover demo and slow mode, how long messages last, the default wait
    /// and the like. A [`BaseCase`](crate::BaseCase) made from this
    /// configuration ([`BaseCase::new`](crate::BaseCase::new), and so
    /// [`run_browser_test`](crate::run_browser_test) and the plugin runner)
    /// starts with them, and with the default wait as its timeout;
    /// [`BaseCase::set_run_settings`](crate::BaseCase::set_run_settings)
    /// changes them later. They are not part of a configuration file.
    ///
    /// This replaces the whole [`RunSettings`], default wait included. A
    /// configuration from [`Settings::to_browser_config`](crate::config::Settings::to_browser_config)
    /// already carries the `SB_TIMEOUT` default wait there, so build on
    /// [`run_settings`](Self::run_settings) (clone it, change what you need)
    /// instead of starting from `RunSettings::default()`, or that wait is lost.
    #[must_use]
    pub fn with_run_settings(mut self, settings: RunSettings) -> Self {
        self.launch.run = settings;
        self
    }

    /// The run-wide settings; see [`with_run_settings`](Self::with_run_settings).
    #[must_use]
    pub fn run_settings(&self) -> &RunSettings {
        &self.launch.run
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

    /// Force Chrome's dark theme on web pages (`--dark`, `dark_mode`).
    /// Chromium only.
    pub fn with_dark_mode(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DarkMode, on)
    }

    /// Open the browser as a guest (`--guest`, `guest_mode`). Left out when the
    /// session loads an extension. Chromium only.
    pub fn with_guest_mode(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Guest, on)
    }

    /// Open the browser in an incognito or private window (`--incognito`,
    /// `incognito_mode`). Left out for Chromium when the session loads an
    /// extension.
    pub fn with_incognito(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Incognito, on)
    }

    /// Open the developer tools with each tab (`--devtools`). A headed session
    /// only.
    pub fn with_devtools(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Devtools, on)
    }

    /// Switch web security off (`--disable-ws`, `--dws`): cross-origin
    /// requests and certificate errors are allowed. Chromium only.
    pub fn with_disable_ws(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DisableWebSecurity, on)
    }

    /// Keep web security on (`--enable-ws`), which is the default; the
    /// opposite of [`with_disable_ws`](Self::with_disable_ws).
    pub fn with_enable_ws(self, on: bool) -> Self {
        self.with_disable_ws(!on)
    }

    /// Render with SwiftShader, a software WebGL renderer (`--swiftshader`).
    /// Chromium only.
    pub fn with_swiftshader(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Swiftshader, on)
    }

    /// Keep the GPU on in a headless session, for pages that need WebGL
    /// (`--enable-3d-apis`). Chromium only.
    pub fn with_enable_3d_apis(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Enable3dApis, on)
    }

    /// Open the window maximized (`--maximize`). A headed session only.
    pub fn with_maximize(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::Maximize, on)
    }

    /// Start Chromium without its sandbox (`--no-sandbox`). Sessions on Linux
    /// already do.
    pub fn with_no_sandbox(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::NoSandbox, on)
    }

    /// Switch the GPU off, headed sessions too (`--disable-gpu`). Chromium
    /// only.
    pub fn with_disable_gpu(self, on: bool) -> Self {
        self.with_switch(LaunchSwitch::DisableGpu, on)
    }

    /// Run headless with `chrome-headless-shell`, Chrome's old headless
    /// implementation (`--headless1`). On, it also makes the session
    /// [headless](Self::headless).
    ///
    /// Chrome runs as [`Browser::ChromeHeadlessShell`], which is downloaded
    /// when it is missing; another Chromium browser, which has no such
    /// program, runs in its new headless mode; Firefox runs headless. The
    /// undetected mode cannot use it.
    pub fn with_headless1(mut self, on: bool) -> Self {
        if on {
            self.headless = true;
        }
        self.with_switch(LaunchSwitch::Headless1, on)
    }

    /// Runs headless in Chrome's new mode (`--headless2`), which is what
    /// [`with_headless`](Self::with_headless) already does.
    pub fn with_headless2(self, on: bool) -> Self {
        self.with_headless(on)
    }

    /// Opens a virtual display for a headed browser on Linux (`--xvfb`), of
    /// the default size (`--xvfb-metrics` sets another through
    /// [`VirtualDisplay`]). Off removes it.
    pub fn with_xvfb(mut self, on: bool) -> Self {
        self.launch.virtual_display = on.then(VirtualDisplay::default);
        self
    }

    /// Adds Chromium switches written as Python's `--chromium-arg` takes them:
    /// comma-separated, each made a `--` switch, and `disable-features=` items
    /// joined with the other features to disable. May be called more than
    /// once.
    ///
    /// See [`parse_chromium_args`](crate::browser::options::parse_chromium_args).
    pub fn with_chromium_arg(mut self, text: &str) -> Self {
        let (args, features) = crate::browser::options::parse_chromium_args(text);
        self.extra_args.extend(args);
        self.with_disable_features(features)
    }

    /// Chromium features to switch off (`--disable-features`), joined with the
    /// session's own into the one switch Chrome reads. Each feature is kept
    /// once; may be called more than once.
    pub fn with_disable_features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for feature in features {
            let feature = feature.into();
            if !self.launch.disable_features.contains(&feature) {
                self.launch.disable_features.push(feature);
            }
        }
        self
    }

    /// Rules that map host names to addresses in Chromium
    /// (`--host-resolver-rules`), such as `MAP example.com 127.0.0.1`.
    pub fn with_host_resolver_rules(mut self, rules: impl Into<String>) -> Self {
        self.launch.host_resolver_rules = Some(rules.into());
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
        self.launch.remote_debug_any_port = false;
        self
    }

    /// Open Chrome's remote debugger on port 9222, or close it again
    /// (`--remote-debug`, `--remote-debugger`). Debugger at
    /// `http://localhost:9222`. When 9222 is in use by something else, a free
    /// port is taken instead and logged; see
    /// [`BrowserSession::remote_debug_address`](crate::browser::session::BrowserSession::remote_debug_address).
    pub fn with_remote_debug(mut self, on: bool) -> Self {
        self.launch.remote_debug_port =
            on.then_some(crate::stealth::options::DEFAULT_REMOTE_DEBUG_PORT);
        self.launch.remote_debug_any_port = on;
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
    /// let settings = Settings::load()?;
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
    fn run_settings_are_set_by_a_builder_and_stay_out_of_config_files() {
        let demo = RunSettings::default().with_demo_mode(true);
        let config = BrowserConfig::default().with_run_settings(demo.clone());
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
        assert_eq!(config.remote_webdriver_timeout_seconds, None);
        let set = config.with_remote_webdriver_timeout(Duration::from_millis(2500));
        assert_eq!(set.remote_webdriver_timeout_seconds, Some(3));
        let exact =
            BrowserConfig::default().with_remote_webdriver_timeout(Duration::from_secs(600));
        assert_eq!(exact.remote_webdriver_timeout_seconds, Some(600));
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
        assert_eq!(BrowserConfig::default().browser, Browser::Chrome);
    }

    #[test]
    fn validate_refuses_a_bad_proxy_before_anything_starts() {
        assert!(BrowserConfig::default().validate().is_ok());
        assert!(BrowserConfig::default()
            .with_proxy("user:pw@h:8080")
            .validate()
            .is_ok());
        assert!(BrowserConfig::default()
            .with_proxy_pac_url("user:pw@http://h/p.pac")
            .validate()
            .is_ok());

        let bad_proxy = BrowserConfig::default()
            .with_proxy("ftp://user:hunter2@h:21")
            .validate()
            .unwrap_err()
            .to_string();
        assert!(bad_proxy.contains("is NOT in the expected"), "{bad_proxy}");
        assert!(!bad_proxy.contains("hunter2"), "{bad_proxy}");

        let bad_pac = BrowserConfig::default()
            .with_proxy_pac_url("http://h/p.js")
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
        let firefox = BrowserConfig::default().with_browser(Browser::Firefox);
        let error = firefox
            .clone()
            .with_proxy("user:pw@h:8080")
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
        assert!(firefox
            .clone()
            .with_proxy_pac_url("user:pw@http://h/p.pac")
            .validate()
            .is_err());
        assert!(firefox.with_proxy("h:8080").validate().is_ok());
        for browser in [Browser::Chrome, Browser::Edge, Browser::Brave] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_proxy("user:pw@h:8080");
            assert!(config.validate().is_ok(), "{browser}");
        }
    }

    #[test]
    fn remote_needs_capabilities_and_a_server() {
        let remote = BrowserConfig::default().with_browser(Browser::Remote);
        let error = remote.clone().validate().unwrap_err().to_string();
        assert!(
            error.contains("--browser=remote needs --cap-file or --cap-string"),
            "{error}"
        );
        let caps = CapabilityOverrides::from_text(r#"{"browserName":"chrome"}"#).unwrap();
        let with_caps = remote.with_capabilities(caps);
        let error = with_caps.clone().validate().unwrap_err().to_string();
        assert!(
            error.contains("--browser=remote needs a WebDriver server"),
            "{error}"
        );
        assert!(with_caps
            .with_webdriver_url("http://hub.example.com:4444/wd/hub")
            .validate()
            .is_ok());
    }

    #[test]
    fn a_grid_sets_the_server_and_localhost_keeps_the_local_driver() {
        use crate::browser::options::GridProtocol;
        let config = BrowserConfig::default()
            .with_grid(GridProtocol::Https, "USER:KEY@hub.example.com", 443)
            .unwrap();
        assert_eq!(
            config.webdriver_url,
            "https://USER:KEY@hub.example.com/wd/hub"
        );
        // The access key stays out of the debug output.
        assert!(!format!("{config:?}").contains("KEY"));
        let local = BrowserConfig::default()
            .with_grid(GridProtocol::Http, "localhost", 4444)
            .unwrap();
        assert!(local.is_default_webdriver_url());
        assert!(BrowserConfig::default()
            .with_grid(GridProtocol::Http, " ", 4444)
            .is_err());
    }

    #[test]
    fn a_binary_location_is_the_browser_binary_path() {
        let config = BrowserConfig::default().with_binary_location("/opt/chrome/chrome");
        assert_eq!(
            config.browser_binary_path,
            Some(PathBuf::from("/opt/chrome/chrome"))
        );
    }

    #[test]
    fn the_remote_debugger_may_move_only_when_the_default_port_was_asked_for() {
        let default = BrowserConfig::default().with_remote_debug(true);
        assert!(default.launch.remote_debug_any_port);
        let named = default.with_remote_debug_port(9333);
        assert!(!named.launch.remote_debug_any_port);
        assert_eq!(named.launch.remote_debug_port, Some(9333));
        let off = named.with_remote_debug(false);
        assert_eq!(off.launch.remote_debug_port, None);
        assert!(!off.launch.remote_debug_any_port);
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
    fn the_debug_output_hides_a_proxy_password_and_a_server_key() {
        let config = BrowserConfig::default()
            .with_proxy("socks5://user:secret@proxy.example:1080")
            .with_proxy_pac_url("http://pac-user:pac-secret@pac.example/p.pac")
            .with_webdriver_url("https://USER:ACCESSKEY@hub.example.com/wd/hub");
        let shown = format!("{config:?}");
        for secret in ["secret", "ACCESSKEY", "pac-user", "USER:"] {
            assert!(!shown.contains(secret), "{secret} is in {shown}");
        }
        assert!(shown.contains("socks5://proxy.example:1080"), "{shown}");
        assert!(shown.contains("https://hub.example.com/wd/hub"), "{shown}");
        // The proxy itself is untouched.
        assert_eq!(
            config.proxy.as_deref(),
            Some("socks5://user:secret@proxy.example:1080")
        );
    }

    #[test]
    fn the_switch_builders_turn_their_switches_on_and_off() {
        let on = BrowserConfig::default()
            .with_dark_mode(true)
            .with_guest_mode(true)
            .with_incognito(true)
            .with_devtools(true)
            .with_disable_ws(true)
            .with_swiftshader(true)
            .with_enable_3d_apis(true)
            .with_maximize(true)
            .with_no_sandbox(true)
            .with_disable_gpu(true);
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
            assert!(on.launch.has(switch), "{switch:?}");
        }
        let off = on.with_dark_mode(false).with_enable_ws(true);
        assert!(!off.launch.has(LaunchSwitch::DarkMode));
        assert!(!off.launch.has(LaunchSwitch::DisableWebSecurity));
        assert!(off
            .with_enable_ws(false)
            .launch
            .has(LaunchSwitch::DisableWebSecurity));
    }

    #[test]
    fn headless1_asks_for_a_headless_browser_and_headless2_is_headless() {
        let headed = BrowserConfig::default().with_headless(false);
        let one = headed.clone().with_headless1(true);
        assert!(one.headless && one.launch.has(LaunchSwitch::Headless1));
        let two = headed.with_headless2(true);
        assert!(two.headless && !two.launch.has(LaunchSwitch::Headless1));
    }

    #[test]
    fn chromium_arg_text_adds_arguments_and_features_without_repeating_them() {
        let config = BrowserConfig::default()
            .with_chromium_arg("mute-audio,disable-features=Translate")
            .with_chromium_arg("-ignore-gpu-blocklist,disable-features=Translate")
            .with_disable_features(["Foo", "Translate"])
            .with_host_resolver_rules("MAP a.test 127.0.0.1");
        assert_eq!(
            config.extra_args,
            ["--mute-audio", "--ignore-gpu-blocklist"]
        );
        assert_eq!(config.launch.disable_features, ["Translate", "Foo"]);
        assert_eq!(
            config.launch.host_resolver_rules.as_deref(),
            Some("MAP a.test 127.0.0.1")
        );
    }

    #[test]
    fn xvfb_is_the_default_virtual_display_or_none() {
        let on = BrowserConfig::default().with_xvfb(true);
        assert_eq!(on.launch.virtual_display, Some(VirtualDisplay::default()));
        assert_eq!(on.with_xvfb(false).launch.virtual_display, None);
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
