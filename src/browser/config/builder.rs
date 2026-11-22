//! The builder of a [`BrowserConfig`].

use std::path::PathBuf;

use super::{split_extension_dirs, whole_seconds, Browser, BrowserConfig, DriverMode};
use crate::api::run_settings::RunSettings;
use crate::browser::options::{
    grid_url, BrowserSettings, CapabilityOverrides, CdpEventCapture, DeviceMetrics, DriverVersion,
    ExtensionArchive, FirefoxOptions, GridProtocol, LaunchSwitch, PageLoadStrategy,
    ProxyBypassList, SessionSettings, VirtualDisplay, WindowPosition, WindowSize,
};
use crate::browser::reuse::ReuseClass;
use crate::config::ad_block_list::AdBlockList;
use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::Fingerprint;

/// A setting whose check can fail, kept by the builder until [`BrowserConfigBuilder::build`]
/// runs it, in the order the setters were called.
#[derive(Clone)]
enum Deferred {
    /// [`BrowserConfigBuilder::grid`]: the Selenium Grid address, which is built (and refused
    /// when the server name is empty) when the configuration is.
    Grid {
        protocol: GridProtocol,
        servername: String,
        port: u16,
    },
    /// [`BrowserConfigBuilder::plugins_named`] and [`BrowserConfigBuilder::plugin_selection`].
    #[cfg(feature = "plugins")]
    Plugins(crate::config::plugins::PluginSelection),
}

// Written by hand: a Grid server name may carry `USER:KEY@`, and a plugin's options may carry
// a token; `{builder:?}` must not print either.
impl std::fmt::Debug for Deferred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grid { protocol, port, .. } => f
                .debug_struct("Grid")
                .field("protocol", protocol)
                .field("servername", &"<configured>")
                .field("port", port)
                .finish(),
            #[cfg(feature = "plugins")]
            Self::Plugins(selection) => f
                .debug_struct("Plugins")
                .field("enabled", &selection.enabled())
                .finish_non_exhaustive(),
        }
    }
}

impl Deferred {
    fn apply(&self, config: &mut BrowserConfig) -> Result<(), SeleniumBaseError> {
        match self {
            Self::Grid {
                protocol,
                servername,
                port,
            } => {
                config.session.webdriver_url = Some(grid_url(*protocol, servername, *port)?);
            }
            #[cfg(feature = "plugins")]
            Self::Plugins(selection) => {
                let set = crate::plugin_host::add_selection(
                    config.plugins.set().clone(),
                    &crate::plugin_host::builtin_registry(),
                    selection,
                )?;
                config.plugins = set.into();
            }
        }
        Ok(())
    }
}

/// Builds a [`BrowserConfig`]: [`BrowserConfig::builder`] starts from the default
/// configuration, [`BrowserConfig::into_builder`] from one you already hold (what
/// [`Settings::to_browser_config`](crate::Settings::to_browser_config) or
/// [`BrowserConfig::from_env`] returned). A setter takes the builder and gives it back;
/// [`build`](Self::build) returns the configuration.
///
/// A setter that cannot be checked on its own (the Selenium Grid address, the plugin names)
/// is checked by `build`, which is where the error is returned.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::{BrowserConfig, DriverMode};
///
/// let config = BrowserConfig::builder()
///     .headless(false)
///     .mode(DriverMode::Uc)
///     .start_page("https://example.com/")
///     .build()?;
/// assert!(!config.browser.headless);
/// assert_eq!(config.session.mode, Some(DriverMode::Uc));
///
/// let again = config.into_builder().headless(true).build()?;
/// assert!(again.browser.headless);
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Debug, Clone)]
#[must_use = "a builder does nothing until `build` is called"]
pub struct BrowserConfigBuilder {
    config: BrowserConfig,
    deferred: Vec<Deferred>,
}

impl BrowserConfigBuilder {
    pub(super) fn new(config: BrowserConfig) -> Self {
        Self {
            config,
            deferred: Vec::new(),
        }
    }

    /// The configuration, after the checks the setters left for it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when [`grid`](Self::grid) was given an empty server
    /// name; [`SeleniumBaseError::Plugin`] for a name `plugins_named` or
    /// `plugin_selection` (feature `plugins`) was given that this build does not have,
    /// options a plugin refuses, or a plugin that is in the set already. The first setting that
    /// fails, in the order the setters were called, is the error.
    ///
    /// A configuration is not [validated](BrowserConfig::validate) here: a session checks
    /// the proxy and the remote browser itself when it connects.
    pub fn build(mut self) -> Result<BrowserConfig, SeleniumBaseError> {
        for step in &self.deferred {
            step.apply(&mut self.config)?;
        }
        Ok(self.config)
    }

    /// How the browser is driven: WebDriver, DevTools without a driver
    /// binary, or the undetected mode (`--cdp`, `--uc`).
    pub fn mode(mut self, mode: DriverMode) -> Self {
        self.config.session.mode = Some(mode);
        self
    }

    /// Which browser to start (`--browser`).
    pub fn browser(mut self, browser: Browser) -> Self {
        self.config.browser.kind = browser;
        self
    }

    /// Runs the browser without a window (`--headless`); `false` opens one
    /// (`--headed`). A default configuration is headless.
    pub fn headless(mut self, headless: bool) -> Self {
        self.config.browser.headless = headless;
        self
    }

    /// The WebDriver server to connect to, such as a Selenium Grid
    /// (`--server`, `--port`). The default is `http://localhost:4444`, or
    /// `SB_WEBDRIVER_URL` for [`BrowserConfig::from_env`].
    ///
    /// It replaces the address an earlier [`grid`](Self::grid) gave.
    pub fn webdriver_url(mut self, url: impl Into<String>) -> Self {
        self.deferred
            .retain(|step| !matches!(step, Deferred::Grid { .. }));
        self.config.session.webdriver_url = Some(url.into());
        self
    }

    /// Attaches directly to an existing Chromium debugger, without WebDriver.
    /// Use `BaseCase::cdp_*` methods, or `LaunchOptionsBuilder::browser_config`.
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    /// let config = BrowserConfig::builder()
    ///     .cdp_url("http://127.0.0.1:9222")
    ///     .build()?;
    /// assert!(config.cdp_url.is_some());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn cdp_url(mut self, url: impl Into<String>) -> Self {
        self.config.cdp_url = Some(url.into());
        self
    }

    /// Opts into the Tauri plugin's authenticated `GET <webdriver_url>/cdp`.
    /// An explicit `cdp_url` takes precedence. No WebDriver session is created.
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    /// let config = BrowserConfig::builder()
    ///     .webdriver_url("http://127.0.0.1:4445/example-token/")
    ///     .cdp_discovery(true)
    ///     .build()?;
    /// assert!(config.cdp_discovery);
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn cdp_discovery(mut self, enabled: bool) -> Self {
        self.config.cdp_discovery = enabled;
        self
    }

    /// Picks the window `BaseCase` drives on a direct CDP attachment
    /// ([`cdp_url`](Self::cdp_url), [`cdp_discovery`](Self::cdp_discovery)):
    /// a target id, or part of the window's URL. The app's other windows are
    /// left as they are. Without it the first window is driven.
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    /// let config = BrowserConfig::builder()
    ///     .cdp_url("http://127.0.0.1:9222")
    ///     .cdp_target("/settings")
    ///     .build()?;
    /// assert_eq!(config.cdp_target.as_deref(), Some("/settings"));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn cdp_target(mut self, target: impl Into<String>) -> Self {
        self.config.cdp_target = Some(target.into());
        self
    }

    /// The user-agent string the browser sends (`--agent`, `--user-agent`).
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.browser.user_agent = Some(user_agent.into());
        self
    }

    /// The browser's language, such as `fr-FR` (`--locale`).
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.config.browser.lang = Some(locale.into());
        self
    }

    /// Sets the emulation timezone ID, such as `"America/New_York"` (`--timezone`, `SB_TIMEZONE`).
    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        self.config.browser.timezone = Some(timezone.into());
        self
    }

    /// Sets the emulation geolocation coordinates (latitude, longitude, and optional accuracy in meters).
    pub fn geolocation(mut self, latitude: f64, longitude: f64, accuracy: Option<f64>) -> Self {
        self.config.browser.geolocation = Some((latitude, longitude, accuracy));
        self
    }

    /// Sets the emulation platform, such as `"Win32"` or `"MacIntel"`.
    pub fn platform(mut self, platform: impl Into<String>) -> Self {
        self.config.browser.platform = Some(platform.into());
        self
    }

    /// Routes traffic through a proxy written
    /// `[scheme://][user:password@]host:port` (`--proxy`).
    ///
    /// Chrome is given `--proxy-server=host:port` and never the login. A
    /// Chrome or Edge session on a WebDriver server this machine runs logs in
    /// to a proxy that asks for a password through a DevTools client beside the
    /// driver, in every tab (also one the page opens), and gives the login to
    /// the proxy only, never to a website that asks for its own. A remote
    /// WebDriver server cannot share its browser's DevTools endpoint, so a
    /// proxy with a login is refused there when the session connects; Firefox
    /// refuses a login too.
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.config.browser.proxy.server = Some(proxy.into());
        self
    }

    /// A proxy auto-config script the browser fetches to choose its proxy
    /// (`--proxy-pac-url`).
    pub fn proxy_pac_url(mut self, url: impl Into<String>) -> Self {
        self.config.browser.proxy.pac_url = Some(url.into());
        self
    }

    /// A browser profile folder that keeps cookies and settings between runs
    /// (`--user-data-dir`).
    pub fn user_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.browser.user_data_dir = Some(dir.into());
        self
    }

    /// An unpacked browser extension to load (`--extension-dir`).
    pub fn extension_dir(mut self, dir: impl Into<String>) -> Self {
        self.config.browser.extensions = split_extension_dirs(&dir.into());
        self
    }

    /// A page the browser opens as soon as it starts (`--start-page`).
    pub fn start_page(mut self, page: impl Into<String>) -> Self {
        self.config.browser.url = Some(page.into());
        self
    }

    /// Shares browsers among the tests of the run (`--reuse-session`, `--rs`).
    /// See [`session.reuse`](crate::browser::options::SessionSettings::reuse).
    pub fn reuse_session(mut self, reuse: bool) -> Self {
        self.config.session.reuse = reuse;
        self
    }

    /// Shares a browser only among the tests that pass the same `class`
    /// (Python's `--reuse-class-session` for the tests of one class). It turns
    /// [`session.reuse`](crate::browser::options::SessionSettings::reuse) on.
    pub fn reuse_class_session(mut self, class: impl Into<String>) -> Self {
        self.config.session.reuse = true;
        self.config.session.reuse_class = Some(ReuseClass::Named(class.into()));
        self
    }

    /// Shares a browser among the tests of one Rust module (`--rcs`,
    /// `--reuse-class-session`, `SB_REUSE_CLASS_SESSION`): the module plays the
    /// part of Python's test class, and is read from the name of the thread
    /// the test runs on ([`ReuseClass::TestModule`]).
    ///
    /// On, it turns [`session.reuse`](crate::browser::options::SessionSettings::reuse) on as well. Off, it
    /// removes a per-module setting and leaves `reuse_session` as it is.
    pub fn reuse_module_session(mut self, on: bool) -> Self {
        if on {
            self.config.session.reuse = true;
            self.config.session.reuse_class = Some(ReuseClass::TestModule);
        } else if self.config.session.reuse_class == Some(ReuseClass::TestModule) {
            self.config.session.reuse_class = None;
        }
        self
    }

    /// Turns [`browser.ad_block`](crate::browser::options::BrowserSettings::ad_block) on or off (`--ad-block`).
    pub fn ad_block(mut self, on: bool) -> Self {
        self.config.browser.ad_block = on;
        self
    }

    /// Blocks the hosts of `list` instead of the built-in list, and turns
    /// [`browser.ad_block`](crate::browser::options::BrowserSettings::ad_block) on. Set `ad_block` to `false` afterwards to
    /// keep the list but block nothing.
    pub fn ad_block_list(mut self, list: AdBlockList) -> Self {
        self.config.browser.ad_block_list = Some(list);
        self.config.browser.ad_block = true;
        self
    }

    /// Emulates a phone (`--mobile`); see [`device_metrics`](Self::device_metrics) for its screen.
    pub fn mobile(mut self, mobile: bool) -> Self {
        self.config.browser.mobile = mobile;
        self
    }

    /// Presents `fingerprint` in the browser.
    ///
    /// Its launch arguments go to every Chromium-based session. In the
    /// [`Cdp`](DriverMode::Cdp) and [`Uc`](DriverMode::Uc) modes the session
    /// also installs its page script (the
    /// [session script](crate::evasion::evasions::session_script)) and sends
    /// its protocol overrides through DevTools; the plain
    /// [`WebDriver`](DriverMode::WebDriver) mode sends the launch arguments only.
    /// What a fingerprint switches on is decided by the settings it holds and
    /// its [`policy`](Fingerprint::policy); see [`Fingerprint::is_masked`].
    pub fn fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.config.fingerprint = Some(fingerprint);
        self
    }

    /// The program of the browser (`--binary-location`); the same as
    /// [`binary_location`](Self::binary_location).
    pub fn browser_binary_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.browser.executable_path = Some(path.into());
        self
    }

    /// Names the program of the browser (Python's `binary_location`,
    /// `--binary-location`). The same as
    /// [`browser_binary_path`](Self::browser_binary_path).
    pub fn binary_location(self, path: impl Into<PathBuf>) -> Self {
        self.browser_binary_path(path)
    }

    /// Sends the session to a Selenium Grid: the WebDriver URL Python
    /// SeleniumBase builds from `--protocol`, `--server` and `--port`
    /// (see [`grid_url`](crate::browser::options::grid_url)).
    ///
    /// A `servername` of `localhost` leaves the configuration as it is, so the
    /// local driver runs, as in Python. The address is built by
    /// [`build`](Self::build), which returns the error for an empty `servername`; a
    /// later [`webdriver_url`](Self::webdriver_url) replaces it.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::browser::options::GridProtocol;
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let config = BrowserConfig::builder()
    ///     .grid(GridProtocol::Http, "USER:KEY@hub.browserstack.com", 80)
    ///     .build()?;
    /// assert_eq!(
    ///     config.session.webdriver_url(),
    ///     "http://USER:KEY@hub.browserstack.com/wd/hub"
    /// );
    ///
    /// let refused = BrowserConfig::builder()
    ///     .grid(GridProtocol::Http, " ", 80)
    ///     .build();
    /// assert!(refused.is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn grid(mut self, protocol: GridProtocol, servername: &str, port: u16) -> Self {
        if servername.trim() != "localhost" {
            self.deferred.push(Deferred::Grid {
                protocol,
                servername: servername.to_owned(),
                port,
            });
        }
        self
    }

    /// Replaces all the Chromium arguments (`--chromium-arg`).
    pub fn extra_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.config.browser.chromium.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Adds one Chromium argument after those already set.
    pub fn extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.config.browser.chromium.args.push(arg.into());
        self
    }

    /// Replaces all the [`BrowserSettings`] at once.
    pub fn browser_settings(mut self, browser: BrowserSettings) -> Self {
        self.config.browser = browser;
        self
    }

    /// Replaces all the [`SessionSettings`] at once, the address a
    /// [`grid`](Self::grid) gave included.
    pub fn session_settings(mut self, session: SessionSettings) -> Self {
        self.deferred
            .retain(|step| !matches!(step, Deferred::Grid { .. }));
        self.config.session = session;
        self
    }

    /// Turns one of the on/off launch settings on or off.
    pub fn switch(mut self, switch: LaunchSwitch, on: bool) -> Self {
        self.config.browser.set(switch, on);
        self
    }

    /// Do not load images (`--block-images`).
    ///
    /// Sets the browser's own content-setting preference, so it also holds for
    /// images that a page requests later.
    pub fn block_images(self, on: bool) -> Self {
        self.switch(LaunchSwitch::BlockImages, on)
    }

    /// Turn JavaScript off (`--disable-js`). Most pages break without it.
    pub fn disable_js(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableJavaScript, on)
    }

    /// Refuse every cookie (`--disable-cookies`).
    pub fn disable_cookies(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableCookies, on)
    }

    /// Send the `DNT: 1` header (`--do-not-track`).
    pub fn do_not_track(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DoNotTrack, on)
    }

    /// Download PDF files instead of showing them in the browser
    /// (`--external-pdf`).
    pub fn external_pdf(self, on: bool) -> Self {
        self.switch(LaunchSwitch::ExternalPdf, on)
    }

    /// Leave Chrome Sync on (`--enable-sync`); sessions switch it off
    /// otherwise.
    pub fn enable_sync(self, on: bool) -> Self {
        self.switch(LaunchSwitch::EnableSync, on)
    }

    /// Strip the Content Security Policy from every response
    /// (`--disable-csp`, `--dcsp`, `--no-csp`). Chromium only.
    pub fn disable_csp(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableCsp, on)
    }

    /// Keep Chrome's automation extension (`--use-auto-ext`, `--auto-ext`).
    /// Upstream marks it deprecated. Chromium only.
    pub fn automation_extension(self, on: bool) -> Self {
        self.switch(LaunchSwitch::AutomationExtension, on)
    }

    /// Load the SeleniumBase Recorder extension (`recorder_ext`). Chromium
    /// only.
    pub fn recorder_extension(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecorderExtension, on)
    }

    /// Fetch a driver, or a Chrome build, that has to be downloaded through
    /// [`browser.proxy.server`](crate::browser::options::ProxySettings::server) (`--proxy-driver`).
    pub fn proxy_for_driver_download(self, on: bool) -> Self {
        self.switch(LaunchSwitch::ProxyForDriverDownload, on)
    }

    /// Let the person end the current test by pressing ESC in the browser
    /// (`--esc-end`, `--esc_end`, `--ee`).
    ///
    /// `BaseCase` puts a key listener in the page and, before it clicks,
    /// highlights, waits for a page to be ready or waits for an element to be
    /// visible, looks whether ESC was pressed; if so the call returns
    /// [`SeleniumBaseError::Skipped`], which `run_browser_test` treats as a
    /// skipped test and not a failure.
    pub fn esc_end(self, on: bool) -> Self {
        self.switch(LaunchSwitch::EscEnd, on)
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
    pub fn recording_sb_cdp(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecSbCdp, on)
    }

    /// Turn Recorder Mode on or off (`--rec`, `--record`, `--recorder`,
    /// `--codegen`, `recorder_mode`).
    ///
    /// With it on, `BaseCase` puts the recorder script into every page, keeps
    /// what the person does, and writes it as a test under `recordings/` when
    /// the case ends (`BaseCase::quit`). Chromium only: another browser makes
    /// `BaseCase::new` fail.
    pub fn recorder_mode(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecorderMode, on)
    }

    /// Print the recording when it is written (`--rec-print`). Turns Recorder
    /// Mode on.
    pub fn recording_print(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecPrint, on)
    }

    /// Also write the recording as a Gherkin feature (`--rec-behave`). Turns
    /// Recorder Mode on.
    pub fn recording_behave(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecBehave, on)
    }

    /// Write the recording as a standalone script instead of a test
    /// (`--rec-sb-mgr`). Turns Recorder Mode on.
    pub fn recording_sb_mgr(self, on: bool) -> Self {
        self.switch(LaunchSwitch::RecSbMgr, on)
    }

    /// The client-side timeout of each request sent to a Remote WebDriver
    /// server such as a Selenium Grid (`SB_REMOTE_WEBDRIVER_TIMEOUT`). A part of
    /// a second rounds up to a whole second. See
    /// [`session.remote_webdriver_timeout_seconds`](crate::browser::options::SessionSettings::remote_webdriver_timeout_seconds).
    pub fn remote_webdriver_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.session.remote_webdriver_timeout_seconds = Some(whole_seconds(timeout));
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
    /// configuration from [`Settings::to_browser_config`](crate::Settings::to_browser_config)
    /// already carries the `SB_TIMEOUT` default wait there, so build on
    /// [`BrowserConfig::run_settings`] (clone it, change what you need)
    /// instead of starting from `RunSettings::default()`, or that wait is lost.
    pub fn run_settings(mut self, settings: RunSettings) -> Self {
        self.config.run = settings;
        self
    }

    /// How long a navigation waits (`--page-load-strategy`, `--pls`).
    pub fn page_load_strategy(mut self, strategy: PageLoadStrategy) -> Self {
        self.config.session.page_load_strategy = Some(strategy);
        self
    }

    /// The size of the browser window (`--window-size`, `size`).
    pub fn window_size(mut self, size: WindowSize) -> Self {
        self.config.browser.window_size = Some(size);
        self
    }

    /// The position of the browser window (`--window-position`, `position`).
    pub fn window_position(mut self, position: WindowPosition) -> Self {
        self.config.browser.window_position = Some(position);
        self
    }

    /// Emulate a phone with this screen (`--metrics`, `d_width`, `d_height`,
    /// `d_p_r`). It turns mobile emulation on, with the user agent from
    /// [`browser.user_agent`](crate::browser::options::BrowserSettings::user_agent) or, without one, an Android one.
    pub fn device_metrics(mut self, metrics: DeviceMetrics) -> Self {
        self.config.browser.device_metrics = Some(metrics);
        self.config.browser.mobile = true;
        self
    }

    /// Force Chrome's dark theme on web pages (`--dark`, `dark_mode`).
    /// Chromium only.
    pub fn dark_mode(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DarkMode, on)
    }

    /// Open the browser as a guest (`--guest`, `guest_mode`). Left out when the
    /// session loads an extension. Chromium only.
    pub fn guest_mode(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Guest, on)
    }

    /// Open the browser in an incognito or private window (`--incognito`,
    /// `incognito_mode`). Left out for Chromium when the session loads an
    /// extension.
    pub fn incognito(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Incognito, on)
    }

    /// Open the developer tools with each tab (`--devtools`). A headed session
    /// only.
    pub fn devtools(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Devtools, on)
    }

    /// Switch web security off (`--disable-ws`, `--dws`): cross-origin
    /// requests and certificate errors are allowed. Chromium only.
    pub fn disable_web_security(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableWebSecurity, on)
    }

    /// Keep web security on (`--enable-ws`), which is the default; the
    /// opposite of [`disable_web_security`](Self::disable_web_security).
    pub fn enable_web_security(self, on: bool) -> Self {
        self.disable_web_security(!on)
    }

    /// Render with SwiftShader, a software WebGL renderer (`--swiftshader`).
    /// Chromium only.
    pub fn swiftshader(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Swiftshader, on)
    }

    /// Keep the GPU on in a headless session, for pages that need WebGL
    /// (`--enable-3d-apis`). Chromium only.
    pub fn enable_3d_apis(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Enable3dApis, on)
    }

    /// Open the window maximized (`--maximize`). A headed session only.
    pub fn maximize(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Maximize, on)
    }

    /// Start Chromium without its sandbox (`--no-sandbox`). Sessions on Linux
    /// already do.
    pub fn no_sandbox(self, on: bool) -> Self {
        self.switch(LaunchSwitch::NoSandbox, on)
    }

    /// Switch the GPU off, headed sessions too (`--disable-gpu`). Chromium
    /// only.
    pub fn disable_gpu(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableGpu, on)
    }

    /// Run headless with `chrome-headless-shell`, Chrome's old headless
    /// implementation (`--headless1`). On, it also makes the session
    /// [headless](crate::browser::options::BrowserSettings::headless).
    ///
    /// Chrome runs as [`Browser::ChromeHeadlessShell`], which is downloaded
    /// when it is missing; another Chromium browser, which has no such
    /// program, runs in its new headless mode; Firefox runs headless. The
    /// undetected mode cannot use it.
    pub fn headless1(mut self, on: bool) -> Self {
        if on {
            self.config.browser.headless = true;
        }
        self.switch(LaunchSwitch::Headless1, on)
    }

    /// Runs headless in Chrome's new mode (`--headless2`), which is what
    /// [`headless`](Self::headless) already does.
    pub fn headless2(self, on: bool) -> Self {
        self.headless(on)
    }

    /// Opens a virtual display for a headed browser on Linux (`--xvfb`), of
    /// the default size (`--xvfb-metrics` sets another through
    /// [`VirtualDisplay`]). Off removes it.
    pub fn xvfb(mut self, on: bool) -> Self {
        self.config.browser.virtual_display = on.then(VirtualDisplay::default);
        self
    }

    /// Adds Chromium switches written as Python's `--chromium-arg` takes them:
    /// comma-separated, each made a `--` switch, and `disable-features=` items
    /// joined with the other features to disable. May be called more than
    /// once.
    ///
    /// See [`parse_chromium_args`](crate::browser::options::parse_chromium_args).
    pub fn chromium_arg(mut self, text: &str) -> Self {
        let (args, features) = crate::browser::options::parse_chromium_args(text);
        self.config.browser.chromium.args.extend(args);
        self.disable_features(features)
    }

    /// Chromium features to switch off (`--disable-features`), joined with the
    /// session's own into the one switch Chrome reads. Each feature is kept
    /// once; may be called more than once.
    pub fn disable_features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for feature in features {
            let feature = feature.into();
            if !self
                .config
                .browser
                .chromium
                .disable_features
                .contains(&feature)
            {
                self.config.browser.chromium.disable_features.push(feature);
            }
        }
        self
    }

    /// Rules that map host names to addresses in Chromium
    /// (`--host-resolver-rules`), such as `MAP example.com 127.0.0.1`.
    pub fn host_resolver_rules(mut self, rules: impl Into<String>) -> Self {
        self.config.browser.chromium.host_resolver_rules = Some(rules.into());
        self
    }

    /// Hosts that skip the proxy (`--proxy-bypass-list`).
    pub fn proxy_bypass_list(mut self, hosts: ProxyBypassList) -> Self {
        self.config.browser.proxy.bypass_list = hosts;
        self
    }

    /// Load a Chromium extension from a `.zip` or `.crx` file
    /// (`--extension-zip`, `--crx`). May be called more than once.
    pub fn extension_archive(mut self, archive: ExtensionArchive) -> Self {
        if !self.config.session.extension_archives.contains(&archive) {
            self.config.session.extension_archives.push(archive);
        }
        self
    }

    /// Firefox arguments and preferences (`--firefox-arg`, `--firefox-pref`).
    pub fn firefox_options(mut self, firefox: FirefoxOptions) -> Self {
        self.config.session.firefox = firefox;
        self
    }

    /// Capabilities to add to the session's (`--cap-string`, `--cap-file`).
    pub fn capabilities(mut self, capabilities: CapabilityOverrides) -> Self {
        self.config.session.capabilities = capabilities;
        self
    }

    /// Which chromedriver to run (`--driver-version`).
    pub fn driver_version(mut self, version: DriverVersion) -> Self {
        self.config.session.driver_version = Some(version);
        self
    }

    /// Open a headed browser on a virtual screen on Linux (`--xvfb`,
    /// `--xvfb-metrics`).
    pub fn virtual_display(mut self, display: VirtualDisplay) -> Self {
        self.config.browser.virtual_display = Some(display);
        self
    }

    /// Open Chrome's remote debugger on `port`. Anyone on the machine who can
    /// reach the port can drive the browser, so use it for debugging only.
    pub fn remote_debug_port(mut self, port: u16) -> Self {
        self.config.browser.debugging_port = Some(port);
        self.config.browser.debugging_any_port = false;
        self
    }

    /// Open Chrome's remote debugger on port 9222, or close it again
    /// (`--remote-debug`, `--remote-debugger`). Debugger at
    /// `http://localhost:9222`. When 9222 is in use by something else, a free
    /// port is taken instead and logged; see
    /// [`BrowserSession::remote_debug_address`](crate::browser::session::BrowserSession::remote_debug_address).
    pub fn remote_debug(mut self, on: bool) -> Self {
        self.config.browser.debugging_port =
            on.then_some(crate::evasion::options::DEFAULT_REMOTE_DEBUG_PORT);
        self.config.browser.debugging_any_port = on;
        self
    }

    /// Log the DevTools Protocol events of Chromium sessions
    /// (`--log-cdp-events`, `--uc-cdp-events`).
    pub fn cdp_events(mut self, capture: CdpEventCapture) -> Self {
        self.config.session.cdp_events = capture;
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
    /// It replaces the set, and the names [`plugins_named`](Self::plugins_named) and
    /// [`plugin_selection`](Self::plugin_selection) added before it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::plugin_api::PluginSet;
    /// use seleniumbase_rs::{BaseCase, BrowserConfig};
    ///
    /// # async fn demo(plugins: PluginSet) -> Result<(), Box<dyn std::error::Error>> {
    /// let config = BrowserConfig::builder().plugins(plugins).build()?;
    /// let mut sb = BaseCase::new(config).await?;
    /// sb.open("https://example.com").await?;
    /// sb.quit().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "plugins")]
    pub fn plugins(mut self, plugins: seleniumbase_rs_plugin::PluginSet) -> Self {
        self.deferred
            .retain(|step| !matches!(step, Deferred::Plugins(_)));
        self.config.plugins = plugins.into();
        self
    }

    /// Adds the built-in plugins called `names` (feature `plugins`) to those
    /// of [`plugins`](Self::plugins).
    ///
    /// The names are the cargo features `plugin-shield`,
    /// `plugin-block-resources` and `plugin-crx` (`plugins-builtin` for all);
    /// each plugin gets its default options. Use
    /// [`plugin_selection`](Self::plugin_selection) to give options.
    ///
    /// [`build`](Self::build) returns [`SeleniumBaseError::Plugin`] for a name this build does
    /// not have (the message lists the ones it has), or a plugin that is in the set already.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let error = BrowserConfig::builder()
    ///     .plugins_named(["no-such-plugin"])
    ///     .build()
    ///     .unwrap_err();
    /// assert!(error.to_string().contains("no-such-plugin"));
    /// ```
    #[cfg(feature = "plugins")]
    pub fn plugins_named<I, S>(self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let selection = names.into_iter().fold(
            crate::config::plugins::PluginSelection::new(),
            |selection, name| selection.enable(name),
        );
        self.plugin_selection(&selection)
    }

    /// Adds the plugins a [`PluginSelection`](crate::config::plugins::PluginSelection)
    /// names, with their options (feature `plugins`), to those of
    /// [`plugins`](Self::plugins): the `[plugins]` table of the
    /// settings file and `SB_PLUGINS`, as `Settings::plugins` holds them.
    ///
    /// [`build`](Self::build) returns [`SeleniumBaseError::Plugin`] for an unknown name,
    /// options a plugin refuses, or a plugin that is in the set already.
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
    ///     .into_builder()
    ///     .plugin_selection(&settings.plugins)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "plugins")]
    pub fn plugin_selection(mut self, selection: &crate::config::plugins::PluginSelection) -> Self {
        self.deferred.push(Deferred::Plugins(selection.clone()));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::options::DEFAULT_WEBDRIVER_URL;

    fn config_of(builder: BrowserConfigBuilder) -> BrowserConfig {
        builder.build().expect("a valid config")
    }

    #[test]
    fn a_builder_with_no_setter_builds_the_default_configuration() {
        assert_eq!(
            config_of(BrowserConfig::builder()),
            BrowserConfig::default()
        );
    }

    #[test]
    fn setters_write_into_the_group_that_owns_the_setting() {
        let config = config_of(
            BrowserConfig::builder()
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
                .webdriver_url("http://hub.test:4444/wd/hub"),
        );
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
    fn the_switch_setters_turn_their_launch_setting_on_and_off() {
        let on = config_of(
            BrowserConfig::builder()
                .block_images(true)
                .dark_mode(true)
                .incognito(true)
                .esc_end(true)
                .recording_sb_cdp(true),
        );
        for switch in [
            LaunchSwitch::BlockImages,
            LaunchSwitch::DarkMode,
            LaunchSwitch::Incognito,
            LaunchSwitch::EscEnd,
            LaunchSwitch::RecSbCdp,
        ] {
            assert!(on.browser.has(switch), "{switch:?}");
        }
        assert!(on.esc_end());
        assert!(on.recording_sb_cdp());
        let off = config_of(
            on.into_builder()
                .dark_mode(false)
                .switch(LaunchSwitch::EscEnd, false),
        );
        assert!(!off.browser.has(LaunchSwitch::DarkMode));
        assert!(!off.esc_end());
        assert!(
            off.browser.has(LaunchSwitch::BlockImages),
            "the others stay"
        );
    }

    #[test]
    fn a_setter_that_implies_another_setting_sets_it_too() {
        let metrics =
            config_of(BrowserConfig::builder().device_metrics(DeviceMetrics::new(390, 844, 3.0)));
        assert!(metrics.browser.mobile);
        let class = config_of(BrowserConfig::builder().reuse_class_session("Login"));
        assert!(class.session.reuse);
        assert_eq!(
            class.session.reuse_class,
            Some(ReuseClass::Named("Login".to_owned()))
        );
        let listed = config_of(BrowserConfig::builder().ad_block_list(AdBlockList::default()));
        assert!(listed.browser.ad_block);
        let shell = config_of(BrowserConfig::builder().headless(false).headless1(true));
        assert!(shell.browser.headless, "--headless1 is headless");
        assert!(shell.browser.has(LaunchSwitch::Headless1));
    }

    #[test]
    fn the_module_flag_turns_reuse_on_and_comes_off_alone() {
        let on = config_of(BrowserConfig::builder().reuse_module_session(true));
        assert!(on.session.reuse);
        assert_eq!(on.session.reuse_class, Some(ReuseClass::TestModule));
        let off = config_of(on.into_builder().reuse_module_session(false));
        assert!(off.session.reuse, "--rs stays on");
        assert_eq!(off.session.reuse_class, None);
        let named = config_of(
            BrowserConfig::builder()
                .reuse_class_session("Login")
                .reuse_module_session(false),
        );
        assert_eq!(
            named.session.reuse_class,
            Some(ReuseClass::Named("Login".to_owned())),
            "a named class is not the module flag"
        );
    }

    #[test]
    fn chromium_arguments_and_features_are_added_and_kept_once() {
        let config = config_of(
            BrowserConfig::builder()
                .chromium_arg("mute-audio,disable-features=A,B")
                .disable_features(["B", "C"])
                .extra_arg("--lang=fr"),
        );
        assert!(config
            .browser
            .chromium
            .args
            .contains(&"--mute-audio".to_owned()));
        assert_eq!(
            config.browser.chromium.args.last().map(String::as_str),
            Some("--lang=fr")
        );
        assert_eq!(config.browser.chromium.disable_features, ["A", "B", "C"]);
        let replaced = config_of(config.into_builder().extra_args(["--only"]));
        assert_eq!(replaced.browser.chromium.args, ["--only"]);
    }

    #[test]
    fn the_remote_timeout_rounds_up_to_whole_seconds() {
        use std::time::Duration;
        let set = config_of(
            BrowserConfig::builder().remote_webdriver_timeout(Duration::from_millis(2500)),
        );
        assert_eq!(set.session.remote_webdriver_timeout_seconds, Some(3));
        let exact =
            config_of(BrowserConfig::builder().remote_webdriver_timeout(Duration::from_secs(600)));
        assert_eq!(exact.session.remote_webdriver_timeout_seconds, Some(600));
    }

    #[test]
    fn run_settings_are_set_by_the_builder_and_stay_out_of_config_files() {
        let demo = RunSettings::builder().demo_mode(true).build();
        let config = config_of(BrowserConfig::builder().run_settings(demo.clone()));
        assert_eq!(config.run_settings(), &demo);
        let text = serde_json::to_string(&config).expect("a config is written as JSON");
        let back: BrowserConfig = serde_json::from_str(&text).expect("it reads back");
        assert_eq!(back.run_settings(), &RunSettings::default());
    }

    #[test]
    fn into_builder_gives_back_a_loaded_configuration_unchanged() {
        let from_env = BrowserConfig::from_vars(|name| match name {
            "SB_WEBDRIVER_URL" => Some("http://hub.test:4444/wd/hub".to_owned()),
            "SB_CHROME_BIN" => Some("/opt/chrome".to_owned()),
            "SB_REMOTE_WEBDRIVER_TIMEOUT" => Some("45".to_owned()),
            _ => None,
        })
        .expect("a valid environment");
        assert_eq!(config_of(from_env.clone().into_builder()), from_env);

        let demo = RunSettings::builder().demo_mode(true).build();
        let rich = config_of(
            BrowserConfig::builder()
                .headless(false)
                .proxy("proxy.test:8080")
                .run_settings(demo)
                .cdp_target("/settings"),
        );
        assert_eq!(config_of(rich.clone().into_builder()), rich);

        let from_json: BrowserConfig = serde_json::from_str(
            r#"{"browser": {"kind": "Firefox", "headless": false}, "session": {"mode": "Cdp"}}"#,
        )
        .expect("a config file");
        assert_eq!(config_of(from_json.clone().into_builder()), from_json);
    }

    #[test]
    fn a_setter_on_a_loaded_configuration_changes_only_its_setting() {
        let loaded = BrowserConfig::from_vars(|name| {
            (name == "SB_CHROME_BIN").then(|| "/opt/chrome".to_owned())
        })
        .expect("a valid environment");
        let changed = config_of(loaded.clone().into_builder().mode(DriverMode::Uc));
        assert_eq!(changed.session.mode, Some(DriverMode::Uc));
        assert_eq!(changed.browser, loaded.browser);
        assert_eq!(changed.session.webdriver_url, loaded.session.webdriver_url);
    }

    #[test]
    fn grid_builds_the_selenium_grid_address() {
        let config = config_of(BrowserConfig::builder().grid(
            GridProtocol::Https,
            "USER:KEY@hub.browserstack.com",
            443,
        ));
        assert_eq!(
            config.session.webdriver_url(),
            "https://USER:KEY@hub.browserstack.com/wd/hub"
        );
        let local =
            config_of(BrowserConfig::builder().grid(GridProtocol::Http, " localhost ", 4444));
        assert_eq!(local.session.webdriver_url(), DEFAULT_WEBDRIVER_URL);
        assert!(
            local.is_default_webdriver_url(),
            "localhost leaves it as it is"
        );
    }

    #[test]
    fn build_refuses_a_grid_without_a_server_name_as_with_grid_did() {
        for name in ["", "  "] {
            let error = BrowserConfig::builder()
                .grid(GridProtocol::Http, name, 4444)
                .build()
                .expect_err("an empty Selenium Grid server name");
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig { .. }),
                "{error:?}"
            );
            assert!(
                error.to_string().contains("Selenium Grid server name"),
                "{error}"
            );
        }
    }

    #[test]
    fn the_setter_called_last_decides_between_a_grid_and_a_webdriver_url() {
        let url_last = config_of(
            BrowserConfig::builder()
                .grid(GridProtocol::Http, "hub.test", 4444)
                .webdriver_url("http://other.test:4444/wd/hub"),
        );
        assert_eq!(
            url_last.session.webdriver_url(),
            "http://other.test:4444/wd/hub"
        );
        let grid_last = config_of(
            BrowserConfig::builder()
                .webdriver_url("http://other.test:4444/wd/hub")
                .grid(GridProtocol::Http, "hub.test", 4444),
        );
        assert_eq!(
            grid_last.session.webdriver_url(),
            "http://hub.test:4444/wd/hub"
        );
        // A refused grid that a later URL replaces is not an error, as it was not one when the
        // URL was written after `with_grid` returned its error handled.
        let replaced = BrowserConfig::builder()
            .grid(GridProtocol::Http, "", 4444)
            .session_settings(SessionSettings::default())
            .build();
        assert!(replaced.is_ok());
    }

    #[test]
    fn the_debug_text_hides_a_grid_login_and_the_config_secrets() {
        let builder = BrowserConfig::builder()
            .grid(GridProtocol::Http, "USER:GRIDKEY@hub.test", 80)
            .proxy("user:PROXYPW@proxy.test:8080");
        let text = format!("{builder:?}");
        assert!(!text.contains("GRIDKEY"), "{text}");
        assert!(!text.contains("PROXYPW"), "{text}");
    }

    #[test]
    fn a_later_webdriver_url_drops_a_refused_grid() {
        let config = BrowserConfig::builder()
            .grid(GridProtocol::Http, "", 4444)
            .webdriver_url("http://other.test:4444/wd/hub")
            .build()
            .expect("the URL replaces the refused grid");
        assert_eq!(
            config.session.webdriver_url(),
            "http://other.test:4444/wd/hub"
        );
    }

    #[test]
    fn the_last_of_two_grids_decides_the_address() {
        let config = config_of(
            BrowserConfig::builder()
                .grid(GridProtocol::Http, "first.test", 4444)
                .grid(GridProtocol::Http, "second.test", 4444),
        );
        assert_eq!(
            config.session.webdriver_url(),
            "http://second.test:4444/wd/hub"
        );
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn the_first_failing_setter_in_call_order_is_the_error() {
        let grid_first = BrowserConfig::builder()
            .grid(GridProtocol::Http, "", 4444)
            .plugins_named(["no-such-plugin"])
            .build()
            .expect_err("both settings fail");
        assert!(
            matches!(grid_first, SeleniumBaseError::InvalidConfig { .. }),
            "{grid_first:?}"
        );
        let plugins_first = BrowserConfig::builder()
            .plugins_named(["no-such-plugin"])
            .grid(GridProtocol::Http, "", 4444)
            .build()
            .expect_err("both settings fail");
        assert!(
            matches!(plugins_first, SeleniumBaseError::Plugin { .. }),
            "{plugins_first:?}"
        );
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn the_debug_text_hides_the_options_of_a_plugin() {
        use crate::config::plugins::PluginSelection;
        let selection = PluginSelection::new()
            .enable("tokened")
            .options("tokened", serde_json::json!({ "token": "PLUGINSECRET" }));
        let builder = BrowserConfig::builder().plugin_selection(&selection);
        let text = format!("{builder:?}");
        assert!(!text.contains("PLUGINSECRET"), "{text}");
        assert!(text.contains("tokened"), "{text}");
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_unknown_plugin_name_is_refused_when_the_configuration_is_built() {
        let builder = BrowserConfig::builder().plugins_named(["no-such-plugin"]);
        let error = builder.build().expect_err("an unknown plugin");
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

        let config = config_of(
            BrowserConfig::builder()
                .plugins(PluginSet::new().with(Mine).expect("a plugin set"))
                .plugins_named(["shield"]),
        );
        assert_eq!(config.plugins().names(), ["mine", "shield"]);

        let again = config
            .into_builder()
            .plugin_selection(&PluginSelection::new().enable("shield"))
            .build()
            .expect_err("shield is in the set already");
        assert!(again.to_string().contains("already in the set"), "{again}");

        let replaced = config_of(
            BrowserConfig::builder()
                .plugins_named(["shield"])
                .plugins(PluginSet::new()),
        );
        assert!(
            replaced.plugins().names().is_empty(),
            "plugins() replaces what came before"
        );
    }
}
