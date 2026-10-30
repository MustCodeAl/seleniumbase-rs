// thirtyfour deprecates its `NetworkConditions`, which `set_network_conditions`
// still takes.
#![expect(
    deprecated,
    reason = "thirtyfour has deprecated NetworkConditions, which this API still takes"
)]

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::{By, DesiredCapabilities, WebDriver, WebElement};
use tracing::{info, instrument};

use crate::browser::binaries;
use crate::browser::chrome_for_testing::{
    install_chrome_for_testing, install_chrome_headless_shell,
};
use crate::browser::chromium_snapshots::install_chromium;

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::launcher::{launch_driver_for, DriverProcess};
use crate::browser::options::{
    emulated_phone, firefox_preferences, CdpEvent, DriverVersion, LaunchSettings, LaunchSwitch,
    ReleaseChannel,
};
#[cfg(feature = "plugins")]
use crate::browser::plugin_hooks::{self, SessionPlugins};
use crate::browser::proxy_auth::{self, ProxyAuthSidecar};
use crate::browser::reuse::{self, Lease, ReuseKey, SessionAddress};
use crate::browser::shadow_find;

use crate::browser::virtual_display::{self, XvfbProcess};
use crate::config::ad_block_list::{AdBlockList, MAX_URL_PATTERNS};
use crate::error::SeleniumBaseError;
use crate::evasion::cdp::*; /* CdpClient */
use crate::evasion::uc;
use crate::utilities::retry::{poll_until, RetryPolicy};

/// Selenium's `getAttribute` as a page script, a port of its atom: the value of
/// `arguments[1]` on the element `arguments[0]`, the DOM property first (what a
/// field holds now, not the `value=""` of its markup) and the attribute where the
/// property is missing, an object or a function; `selected`, `checked` and the
/// other boolean attributes give `"true"` or `null`. The `/* getAttribute */`
/// marker tells tests which script this is.

const GET_ATTRIBUTE: &str = "/* getAttribute */ \
    const element = arguments[0]; const name = String(arguments[1]).toLowerCase(); \
    const alias = { class: 'className', readonly: 'readOnly' }; \
    const booleans = ['allowfullscreen', 'allowpaymentrequest', 'allowusermedia', 'async', \
        'autofocus', 'autoplay', 'checked', 'compact', 'complete', 'controls', 'declare', \
        'default', 'defaultchecked', 'defaultselected', 'defer', 'disabled', 'ended', \
        'formnovalidate', 'hidden', 'indeterminate', 'iscontenteditable', 'ismap', \
        'itemscope', 'loop', 'multiple', 'muted', 'nohref', 'nomodule', 'noresize', \
        'noshade', 'novalidate', 'nowrap', 'open', 'paused', 'playsinline', 'pubdate', \
        'readonly', 'required', 'reversed', 'scoped', 'seamless', 'seeking', 'selected', \
        'truespeed', 'typemustmatch', 'willvalidate']; \
    const tag = element.tagName ? element.tagName.toUpperCase() : ''; \
    const attribute = element.getAttribute(name); \
    if (name === 'style') { \
        return element.style ? (typeof element.style === 'string' ? element.style \
            : element.style.cssText) : null; \
    } \
    const selectable = tag === 'OPTION' || (tag === 'INPUT' \
        && ['checkbox', 'radio'].includes(String(element.type).toLowerCase())); \
    if ((name === 'selected' || name === 'checked') && selectable) { \
        return (tag === 'OPTION' ? element.selected : element.checked) ? 'true' : null; \
    } \
    if ((tag === 'IMG' && name === 'src') || (tag === 'A' && name === 'href')) { \
        return attribute ? String(element[name]) : attribute; \
    } \
    if (name === 'spellcheck') { \
        const lower = attribute === null ? null : attribute.toLowerCase(); \
        return lower === 'false' || lower === 'true' ? lower : String(element[name]); \
    } \
    const key = alias[name] || String(arguments[1]); \
    if (booleans.includes(name)) { return attribute !== null || !!element[key] ? 'true' : null; } \
    if (name === 'value' && tag === 'LI') { return attribute; } \
    let property; \
    try { property = element[key]; } catch (error) { property = undefined; } \
    if (property === undefined || property === null \
        || typeof property === 'object' || typeof property === 'function') { return attribute; } \
    return String(property);";

/// Owns a WebDriver session, optional CDP client, and optional auto-started driver process.
pub struct BrowserSession {
    driver: Option<WebDriver>,
    cdp: Option<CdpClient>,
    /// A tab of an external Chromium app, reached without WebDriver.
    external_page: Option<crate::sb_cdp::Page>,
    driver_process: Option<DriverProcess>,
    /// The virtual screen of a headed Linux session; dropped, and so stopped,
    /// after the driver.
    virtual_display: Option<XvfbProcess>,
    /// The DevTools commands that make a window refuse the ad hosts of the
    /// configuration, or none if it blocks nothing. They are sent to the first
    /// window and to each window the session switches to.
    ad_block: Vec<(&'static str, Value)>,
    /// The DevTools commands that make a window of an undetected-mode session a
    /// phone, or none: the other modes ask for it with the `mobileEmulation`
    /// capability. Sent like the ad-block commands, because the emulation
    /// belongs to a window.
    mobile: Vec<(&'static str, Value)>,
    /// The hold on a browser kept for reuse (`--reuse-session`). With one,
    /// quitting or dropping the session gives the browser back instead of
    /// closing it, and the reuse registry owns the driver process and the
    /// virtual display.
    reuse: Option<Lease>,
    /// The port of Chrome's remote debugger, when the configuration asked for
    /// one: the port that was asked for, or the free one taken in its place.
    remote_debug: Option<u16>,
    /// The DevTools client that answers the proxy's password prompts, when the
    /// proxy has a login (see `browser::proxy_auth`); closed with the session.
    proxy_login: Option<ProxyAuthSidecar>,
    /// The browser plugins of the session (feature `plugins`), and the scripts
    /// they added; none without plugins.
    #[cfg(feature = "plugins")]
    plugins: Option<SessionPlugins>,
}

impl BrowserSession {
    /// WebDriver interaction: `connect`.
    ///
    /// # Panics
    ///
    /// On a direct CDP attachment (use `cdp_*` or [`pure_cdp_page`](Self::pure_cdp_page)),
    /// after [`quit`](Self::quit), or after a [`reconnect`](Self::reconnect)
    /// that could not start the new browser: the session then has no driver
    /// and a new one must be started.
    pub fn driver(&self) -> &WebDriver {
        self.driver.as_ref().expect(
            "the browser session has no WebDriver: it was quit, it is a direct CDP \
                     attachment, or a reconnect could not start a new browser; \
                     `webdriver()` returns an error instead of panicking",
        )
    }

    /// The session's WebDriver, or an error where [`driver`](Self::driver)
    /// panics.
    ///
    /// Every WebDriver-backed method of the session, and of `BaseCase`, goes
    /// through this, so a session without a driver answers them with an error.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Unsupported`], naming the engine, on a direct CDP
    /// attachment (`cdp_url` or `cdp_discovery`): the driver does not exist
    /// there, so use the `cdp_*` methods or
    /// [`pure_cdp_page`](Self::pure_cdp_page).
    /// [`SeleniumBaseError::SessionNotStarted`] after [`quit`](Self::quit), or
    /// after a [`reconnect`](Self::reconnect) that could not start the new
    /// browser.
    pub fn webdriver(&self) -> Result<&WebDriver, SeleniumBaseError> {
        match self.driver.as_ref() {
            Some(driver) => Ok(driver),
            None if self.external_page.is_some() => Err(SeleniumBaseError::Unsupported(
                "this needs the WebDriver engine, but the session is a direct CDP attachment \
                     (cdp_url): use the cdp_* methods or pure_cdp_page()"
                    .to_owned(),
            )),
            None => Err(SeleniumBaseError::SessionNotStarted),
        }
    }

    /// Starts a browser for `config` and connects to it, or, with
    /// [`BrowserConfig::reuse_session`] on, attaches to a browser kept for
    /// reuse (see [`crate::browser::reuse`]).
    ///
    /// With browser plugins (feature `plugins`,
    /// [`BrowserConfig::with_plugins`]) the set is checked and `before_launch`
    /// runs before the browser is asked for; once it runs, `on_browser` and,
    /// for its window, the packages and `on_page_created`. A refusal or a hook
    /// that fails stops the session with [`SeleniumBaseError::Plugin`] and
    /// closes a browser that was started.
    #[instrument(skip(config))]
    pub async fn connect(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        if config.cdp_url.is_some() || config.cdp_discovery {
            return Self::connect_cdp(&config).await;
        }
        #[cfg(feature = "plugins")]
        if !config.plugins().is_empty() {
            return Self::connect_with_plugins(config).await;
        }
        if config.reuse_session {
            return reuse::connect(config).await;
        }
        Self::connect_new(config).await
    }

    /// Starts a browser for `config` and connects to it, whatever its reuse
    /// setting says.
    pub(crate) async fn connect_new(mut config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        if config.cdp_url.is_some() || config.cdp_discovery {
            return Self::connect_cdp(&config).await;
        }
        info!(browser = ?config.browser, headless = config.headless, "connecting browser session");
        // Everything that can be refused is, before a browser starts.
        config.validate()?;
        validate_mode_support(&config)?;
        // A remote grid cannot be given a proxy login: refused before a browser starts.
        proxy_auth::login_for(&config)?;
        let remote_debug = resolve_remote_debug(&mut config)?;
        // Worked out first, so a list the browser cannot be given is refused
        // before a browser starts.
        let ad_block = ad_block_commands(&config)?;
        let virtual_display = start_virtual_display(&config)?;
        let (driver, driver_process) =
            connect_driver(&config, virtual_display.as_ref().map(XvfbProcess::display)).await?;
        if !config.browser.is_chromium_based() {
            // Chromium takes its window geometry as launch arguments; the
            // others have to be told once they are running.
            apply_window_rect(&driver, &config.launch).await?;
            // Chromium is given `--start-maximized`; the others are asked once
            // they are running, and only when they have a window.
            if config.launch.has(LaunchSwitch::Maximize) && !config.headless {
                driver.maximize_window().await?;
            }
        }
        let cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(driver.handle().clone()))
        } else {
            None
        };

        let mut session = Self {
            driver: Some(driver),
            cdp,
            external_page: None,
            driver_process,
            virtual_display,
            mobile: mobile_commands(&config),
            remote_debug,
            proxy_login: None,
            ad_block,
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        };
        // The proxy must be answerable before the first page loads.
        if let Err(error) = session.begin_proxy_login(&config).await {
            let _ = session.quit().await;
            return Err(error);
        }
        session.initialize_mode(&config).await?;
        info!("browser session connected");
        Ok(session)
    }

    /// A session on a browser leased for reuse, which was set up when it
    /// started and has just been cleaned: its mode is not set up again.
    pub(crate) fn attached(
        config: &BrowserConfig,
        driver: WebDriver,
        lease: Lease,
    ) -> Result<Self, SeleniumBaseError> {
        let ad_block = match ad_block_commands(config) {
            Ok(commands) => commands,
            Err(error) => {
                // The browser stays open for the next test.
                let _ = driver.leak();
                return Err(error);
            }
        };
        let cdp = config
            .is_cdp_enabled()
            .then(|| CdpClient::from_handle(driver.handle().clone()));
        Ok(Self {
            driver: Some(driver),
            cdp,
            external_page: None,
            driver_process: None,
            virtual_display: None,
            ad_block,
            reuse: Some(lease),
            mobile: mobile_commands(config),
            remote_debug: config.launch.remote_debug_port,
            proxy_login: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        })
    }

    /// Keeps the browser of this new session for reuse under `key`, held by
    /// this session. The registry takes the driver process and the virtual
    /// display, which live as long as the browser.
    pub(crate) fn share(&mut self, key: ReuseKey) {
        let Some(driver) = self.driver.as_ref() else {
            return;
        };
        let handle = driver.handle();
        let address = SessionAddress {
            server_url: handle.server_url().to_string(),
            session_id: handle.session_id().to_string(),
            capabilities: serde_json::to_value(handle.capabilities()).unwrap_or(Value::Null),
        };
        self.reuse = Some(reuse::register(
            key,
            address,
            self.driver_process.take(),
            self.virtual_display.take(),
        ));
    }

    /// The WebDriver session id (Python's `driver.session_id`), or `None` for
    /// a session that is not connected.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.driver
            .as_ref()
            .map(|driver| driver.handle().session_id().to_string())
    }

    /// A DevTools Protocol client for this session, whatever its
    /// [`DriverMode`](crate::browser::config::DriverMode): the commands go
    /// through the driver's `/goog/cdp/execute` passthrough, so no mode has to
    /// be switched on. Chromium sessions only; the driver of another browser
    /// answers the commands with an error.
    ///
    /// [`execute_cdp_with_params`](Self::execute_cdp_with_params) needs the
    /// `cdp` or `uc` mode; this is what Recorder Mode uses instead.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] for a session that is not
    /// connected.
    pub fn devtools(&self) -> Result<CdpClient, SeleniumBaseError> {
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session has no browser driver".to_owned())
        })?;
        Ok(CdpClient::from_handle(driver.handle().clone()))
    }

    /// Lets go of the driver without ending its session: the browser stays
    /// open for the next test.
    fn keep_browser_open(&mut self) {
        // The next lease starts a login client of its own.
        self.proxy_login = None;
        self.cdp = None;
        if let Some(driver) = self.driver.take() {
            let _ = driver.leak();
        }
    }

    /// Where Chrome's remote debugger listens (`localhost:9222`), when the
    /// configuration opened one (`--remote-debug`); open `chrome://inspect` in
    /// another Chrome to see the session. With
    /// [`with_remote_debug`](crate::BrowserConfig::with_remote_debug) the port is
    /// the free one taken in place of a busy 9222.
    #[must_use]
    pub fn remote_debug_address(&self) -> Option<String> {
        self.remote_debug.map(|port| format!("localhost:{port}"))
    }

    /// [`attached`](Self::attached), then the proxy login of `config`, if it has
    /// one, read from the `capabilities` the browser was created with.
    ///
    /// A browser kept for reuse was set up by the session that started it, but
    /// the client that answers a proxy's password prompts lives as long as one
    /// session, so each lease starts its own.
    pub(crate) async fn attached_with_login(
        config: &BrowserConfig,
        driver: WebDriver,
        lease: Lease,
        capabilities: &Value,
    ) -> Result<Self, SeleniumBaseError> {
        let mut session = Self::attached(config, driver, lease)?;
        session.begin_proxy_login_with(config, capabilities).await?;
        Ok(session)
    }

    /// Starts answering the proxy's password prompts when `config` has a proxy
    /// with a login, reading the browser's DevTools address from the session's
    /// capabilities.
    async fn begin_proxy_login(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        let capabilities = self.driver.as_ref().map_or(Value::Null, |driver| {
            serde_json::to_value(driver.handle().capabilities()).unwrap_or(Value::Null)
        });
        self.begin_proxy_login_with(config, &capabilities).await
    }

    /// [`begin_proxy_login`](Self::begin_proxy_login) with the capabilities
    /// given.
    async fn begin_proxy_login_with(
        &mut self,
        config: &BrowserConfig,
        capabilities: &Value,
    ) -> Result<(), SeleniumBaseError> {
        let Some(credentials) = proxy_auth::login_for(config)? else {
            return Ok(());
        };
        let address = proxy_auth::debugger_address(capabilities).ok_or_else(|| {
            SeleniumBaseError::cdp_driver(
                "the browser did not report a DevTools address (debuggerAddress), so its proxy \
                 cannot be logged in to",
            )
        })?;
        self.proxy_login = Some(ProxyAuthSidecar::attach(&address, credentials).await?);
        Ok(())
    }

    /// Creates a disconnected session for test-only use. Any WebDriver call will
    /// panic, so this must only be used when the caller never touches the driver.
    pub fn disconnected() -> Self {
        Self {
            driver: None,
            cdp: None,
            external_page: None,
            driver_process: None,
            virtual_display: None,
            ad_block: Vec::new(),
            mobile: Vec::new(),
            remote_debug: None,
            proxy_login: None,
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        }
    }

    /// Connects through the pure engine's existing config, identity and plugin bridge.
    async fn connect_cdp(config: &BrowserConfig) -> Result<Self, SeleniumBaseError> {
        config.validate()?;
        if config.reuse_session || config.recorder_mode() {
            return Err(SeleniumBaseError::invalid_config(
                "direct CDP attachment does not support WebDriver session reuse or Recorder Mode",
            ));
        }
        let options = crate::sb_cdp::LaunchOptions::builder()
            .browser_config(config)
            .build()?;
        let target = options.cdp_target().map(str::to_owned);

        let browser = crate::sb_cdp::Browser::launch(options).await?;
        // Only the window that is picked is prepared: the app's others are
        // left as they are.
        let picked = match &target {
            Some(target) => browser.select_page(target).await,
            None => browser.default_page().await,
        };
        let page = match picked {
            Ok(page) => page,
            Err(error) => {
                browser.close().await?;
                return Err(error);
            }
        };
        let mut session = Self::disconnected();
        session.external_page = Some(page);
        Ok(session)
    }

    /// The pure CDP tab of an external attachment, or `None` for WebDriver.
    /// Provides the full page/browser API, including attach-compatible plugins.
    ///
    /// ```no_run
    /// use seleniumbase_rs::{BrowserConfig, BrowserSession};
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let mut session = BrowserSession::connect(
    ///     BrowserConfig::default().with_cdp_url("http://127.0.0.1:9222")
    /// ).await?;
    /// if let Some(page) = session.pure_cdp_page() {
    ///     page.locator("#save").click().await?;
    /// }
    /// session.quit().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn pure_cdp_page(&self) -> Option<&crate::sb_cdp::Page> {
        self.external_page.as_ref()
    }

    /// Navigates to `url`.
    ///
    /// On Firefox and Safari, the scripts browser plugins added for every
    /// document run again in the new one (feature `plugins`).
    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page.goto(url).await;
        }
        self.webdriver()?.goto(url).await?;
        #[cfg(feature = "plugins")]
        if let Some(plugins) = &self.plugins {
            plugins.after_navigation(self.webdriver()?).await?;
        }
        Ok(())
    }

    /// Navigates back in browser history.
    pub async fn back(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.back().await?;
        Ok(())
    }

    /// Navigates forward in browser history.
    pub async fn forward(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.forward().await?;
        Ok(())
    }

    /// Reloads the current page.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.refresh().await?;
        Ok(())
    }

    /// Returns the current page title.
    pub async fn current_title(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page.title().await;
        }
        let title = self.webdriver()?.title().await?;
        Ok(title)
    }

    /// Returns the current page URL as a string.
    pub async fn current_url(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page.url().await;
        }
        let url = self.webdriver()?.current_url().await?;
        Ok(url.as_str().to_owned())
    }

    /// Clicks the element located by `locator`.
    pub async fn click(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.click().await?;
        Ok(())
    }

    /// Clears and types `text` into the element located by `locator`.
    pub async fn type_text(&mut self, locator: By, text: &str) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.clear().await?;
        element.send_keys(text).await?;
        Ok(())
    }

    /// Clears the value of the element located by `locator`.
    pub async fn clear(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.clear().await?;
        Ok(())
    }

    /// Submits the form containing the element located by `locator`.
    pub async fn submit(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let element_json = element.to_json()?;
        // Form submit via script
        self.webdriver()?
            .execute("arguments[0].closest('form').submit()", vec![element_json])
            .await?;
        Ok(())
    }

    /// Returns the visible text of the element located by `locator`.
    pub async fn text(&self, locator: By) -> Result<String, SeleniumBaseError> {
        let element = self.find(locator).await?;
        Ok(element.text().await?)
    }

    /// Hovers over the element located by `locator`.
    pub async fn hover(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        self.webdriver()?
            .action_chain()
            .move_to_element_center(&element)
            .perform()
            .await?;
        Ok(())
    }

    /// Selects the option with visible text `text`.
    pub async fn select_option_by_text(
        &mut self,
        locator: By,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_visible_text(text).await?;
        Ok(())
    }

    /// Selects the option whose `value` matches.
    pub async fn select_option_by_value(
        &mut self,
        locator: By,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_value(value).await?;
        Ok(())
    }

    /// Switches context into the frame located by `locator`.
    pub async fn switch_to_frame(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.enter_frame().await?;
        Ok(())
    }

    /// Switches context back to the top-level document.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.enter_default_frame().await?;
        Ok(())
    }

    /// Drags the source element onto the target element.
    pub async fn drag_and_drop(
        &mut self,
        source_locator: By,
        target_locator: By,
    ) -> Result<(), SeleniumBaseError> {
        let source = self.find(source_locator).await?;
        let target = self.find(target_locator).await?;
        self.webdriver()?
            .action_chain()
            .drag_and_drop_element(&source, &target)
            .perform()
            .await?;
        Ok(())
    }

    /// Returns the full HTML source of the current page.
    pub async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        Ok(self.webdriver()?.source().await?)
    }

    /// Returns the first element matching `locator`. A CSS locator that crosses
    /// shadow roots (`host::shadow inner`) is looked up through each host's shadow
    /// root, and every lookup of the session goes through here.
    pub async fn find(&self, locator: By) -> Result<WebElement, SeleniumBaseError> {
        shadow_find::find(self.webdriver()?, locator).await
    }

    /// Returns all elements matching `locator`, which may cross shadow roots as
    /// [`find`](Self::find) says. A host that is missing or has no shadow root
    /// gives an empty list.
    pub async fn find_all(&self, locator: By) -> Result<Vec<WebElement>, SeleniumBaseError> {
        shadow_find::find_all(self.webdriver()?, locator).await
    }

    /// WebDriver interaction: `element_present`.
    pub async fn element_present(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let elements = self.find_all(locator).await?;
        Ok(!elements.is_empty())
    }

    /// Waits up to `timeout_secs` seconds for `locator` to match an element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if nothing
    /// matches in time.
    pub async fn wait_for_element(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async { self.find(locator.clone()).await.ok() },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator}"), timeout_secs))
    }

    /// WebDriver interaction: `get_attribute`.
    pub async fn get_attribute(
        &mut self,
        locator: By,
        attribute_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.find(locator).await?;
        self.attribute_or_property(&element, attribute_name).await
    }

    /// The value of `name` on `element`, as Selenium's `get_attribute` gives it:
    /// the DOM property of that name if the element has one (so the text typed
    /// into an input, not the `value=""` its markup has), and otherwise the attribute.
    /// A `<select>` has no `value` attribute, but its property says which option
    /// is chosen; an `<option>` has an `index` property and no such attribute.
    ///
    /// The page script is Selenium's atom, as Python's `get_attribute` runs it, not
    /// the driver's attribute command, which answers with the markup's attribute
    /// alone. `selected`, `checked` and the other boolean attributes are `"true"`
    /// or `None`, as in Selenium.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver fails.
    pub(crate) async fn attribute_or_property(
        &self,
        element: &WebElement,
        name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let value = self
            .webdriver()?
            .execute(GET_ATTRIBUTE, vec![element.to_json()?, json!(name)])
            .await?;
        Ok(value.json().as_str().map(str::to_owned))
    }

    /// Clicks `element` with a script (`element.click()`), which reaches an
    /// element that is not displayed or that another element covers.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver fails.
    pub(crate) async fn js_click_element(
        &self,
        element: &WebElement,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .execute("arguments[0].click();", vec![element.to_json()?])
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `get_property`.
    pub async fn get_property(
        &mut self,
        locator: By,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.find(locator).await?;
        Ok(element.prop(property_name).await?)
    }

    /// Waits up to `timeout` seconds for `locator` to become visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// not visible in time.
    pub async fn wait_for_element_visible(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element) if element.is_displayed().await.unwrap_or(false) => Some(element),
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be visible"), timeout_secs))
    }

    /// Waits up to `timeout` seconds for `locator` to become clickable.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// not clickable in time.
    pub async fn wait_for_element_clickable(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element)
                        if element.is_displayed().await.unwrap_or(false)
                            && element.is_enabled().await.unwrap_or(false) =>
                    {
                        Some(element)
                    }
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be clickable"), timeout_secs))
    }

    /// WebDriver interaction: `execute_async_script`.
    pub async fn execute_async_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.webdriver()?
            .execute_async(script, vec![])
            .await
            .map_err(SeleniumBaseError::WebDriver)
            .map(|ret| ret.json().clone())
    }

    /// WebDriver interaction: `maximize_window`.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .maximize_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `set_window_size`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .set_window_rect(0, 0, width, height)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns the current window size as `(width, height)`.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        let rect = self
            .webdriver()?
            .get_window_rect()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok((rect.width as u32, rect.height as u32))
    }

    /// Switches to the window/tab identified by `handle`.
    ///
    /// A session that blocks ads makes the window refuse the ad hosts too; see
    /// [`apply_ad_block`](Self::apply_ad_block).
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        let h = thirtyfour::common::types::WindowHandle::from(handle.to_string());
        self.webdriver()?
            .switch_to_window(h)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.apply_mobile_emulation().await?;
        self.apply_ad_block().await
    }

    /// Double-clicks the element located by `locator`.
    pub async fn double_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.webdriver()?
            .action_chain()
            .double_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Right-clicks the element located by `locator`.
    pub async fn context_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.webdriver()?
            .action_chain()
            .context_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is enabled.
    pub async fn is_enabled(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self.find(locator).await?;
        element
            .is_enabled()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is selected.
    pub async fn is_selected(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self.find(locator).await?;
        element
            .is_selected()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `is_displayed`.
    pub async fn is_displayed(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        if let Ok(element) = self.find(locator).await {
            element
                .is_displayed()
                .await
                .map_err(SeleniumBaseError::WebDriver)
        } else {
            Ok(false)
        }
    }

    /// WebDriver interaction: `switch_to_alert_accept`.
    pub async fn switch_to_alert_accept(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `switch_to_alert_dismiss`.
    pub async fn switch_to_alert_dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `get_alert_text`.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.webdriver()?
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `type_alert_text`.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `add_cookie`.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        let cookie = thirtyfour::cookie::Cookie::new(name, value);
        self.webdriver()?
            .add_cookie(cookie)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `get_cookie`.
    pub async fn get_cookie(
        &self,
        name: &str,
    ) -> Result<thirtyfour::cookie::Cookie, SeleniumBaseError> {
        self.webdriver()?
            .get_named_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `delete_cookie`.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .delete_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `delete_all_cookies`.
    pub async fn delete_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .delete_all_cookies()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `find_elements`.
    pub async fn find_elements(
        &self,
        locator: thirtyfour::By,
    ) -> Result<Vec<thirtyfour::WebElement>, SeleniumBaseError> {
        self.find_all(locator).await
    }

    /// Opens and switches to a new browser window/tab.
    ///
    /// A session that blocks ads makes the new window refuse the ad hosts too;
    /// see [`apply_ad_block`](Self::apply_ad_block). Browser plugins (feature
    /// `plugins`) give it their packages and `on_page_created`.
    pub async fn switch_to_new_window(&self) -> Result<(), SeleniumBaseError> {
        let handle = self
            .webdriver()?
            .new_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.webdriver()?
            .switch_to_window(handle)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.apply_ad_block().await?;
        self.apply_mobile_emulation().await?;
        #[cfg(feature = "plugins")]
        self.apply_plugins_to_window().await?;
        Ok(())
    }

    /// Waits up to `timeout` seconds for `locator` to be removed.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// still there when the time is up.
    pub async fn wait_for_element_absent(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find_all(locator.clone()).await {
                    Ok(elements) if elements.is_empty() => Some(()),
                    Err(_) => Some(()),
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be absent"), timeout_secs))
    }

    /// Waits up to `timeout_secs` seconds for the element at `locator` to
    /// contain `expected_substring`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the text and the
    /// locator if the text does not appear in time.
    pub async fn wait_for_text(
        &self,
        locator: By,
        expected_substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element) => match shown_text(&element).await {
                        Ok(text) if text.contains(expected_substring) => Some(()),
                        _ => None,
                    },
                    Err(_) => None,
                }
            },
        )
        .await
        .ok_or_else(|| {
            timed_out(
                format!("text '{expected_substring}' in element {locator}"),
                timeout_secs,
            )
        })
    }

    /// Executes arbitrary JavaScript and returns the result.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page
                .evaluate(format!("(function() {{\n{script}\n}}).call(window)"))
                .await;
        }
        let ret = self.webdriver()?.execute(script, Vec::new()).await?;
        Ok(ret.json().clone())
    }

    /// Captures a screenshot and writes it to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.screenshot(path).await?;
        Ok(())
    }

    /// Captures a screenshot and returns the raw PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        Ok(self.webdriver()?.screenshot_as_png().await?)
    }

    /// Enables the Page and Network CDP domains.
    ///
    /// The Runtime domain stays off on purpose: enabling it is a well-known
    /// bot-detection tell. See [`CdpClient::enable_default_domains`].
    ///
    /// [`CdpClient::enable_default_domains`]: crate::evasion::cdp::CdpClient::enable_default_domains
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            for method in crate::evasion::cdp::DEFAULT_DOMAIN_COMMANDS {
                page.execute(method, json!({})).await?;
            }
            return Ok(());
        }
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains()
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Executes a CDP command without parameters.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        if self.external_page.is_some() {
            return self.execute_cdp_with_params(method, json!({})).await;
        }
        let cdp = self.cdp_client()?;
        cdp.execute(method).await
    }

    /// Executes a CDP command with JSON parameters.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page.execute(method, params).await;
        }
        let cdp = self.cdp_client()?;
        cdp.execute_with_params(method, params).await
    }

    /// Clears the browser cache via CDP.
    pub async fn clear_browser_cache(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.clear_cache().await
    }

    /// Clears all browser cookies via CDP.
    ///
    /// A session without a DevTools connection (plain WebDriver mode) clears the
    /// cookies of the current domain through WebDriver instead.
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        if let Ok(cdp) = self.cdp_client() {
            return cdp.clear_cookies().await;
        }
        self.webdriver()?
            .delete_all_cookies()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns all cookies via CDP as JSON: `{"cookies": [...]}`.
    ///
    /// A session without a DevTools connection (plain WebDriver mode) answers
    /// the same shape from WebDriver, with the cookies of the current page.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        if let Ok(cdp) = self.cdp_client() {
            return cdp.get_cookies().await;
        }
        let cookies = self
            .webdriver()?
            .get_all_cookies()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(serde_json::json!({ "cookies": cookies }))
    }

    /// Drains the DevTools Protocol events logged since the last call, oldest
    /// first (Python's `driver.get_log("performance")`).
    ///
    /// The log exists only when the session was started with
    /// [`BrowserConfig::with_cdp_events`] (`--log-cdp-events`,
    /// `--uc-cdp-events`); chromedriver keeps nothing otherwise and the result
    /// is an error from the driver. Entries that are not events are skipped.
    ///
    /// # Errors
    ///
    /// Returns the driver's error when it keeps no `performance` log.
    pub async fn cdp_events(&self) -> Result<Vec<CdpEvent>, SeleniumBaseError> {
        let entries = self.webdriver()?.get_log("performance").await?;
        Ok(entries
            .iter()
            .filter_map(|entry| CdpEvent::from_log_message(&entry.message, entry.timestamp))
            .collect())
    }

    /// Dispatches a CDP mouse click at `(x, y)`.
    pub async fn cdp_mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.mouse_click(x, y).await
    }

    /// Inserts `text` via CDP without element focus.
    pub async fn cdp_type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.keyboard_insert_text(text).await
    }

    /// Configures network throttling via CDP.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.set_network_conditions(conditions.clone()).await
    }

    /// Sets the browser geolocation via CDP.
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        uc::override_geolocation(cdp, latitude, longitude, accuracy).await
    }

    /// Sets the browser timezone via CDP.
    pub async fn set_timezone(&self, timezone_id: &str) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        uc::override_timezone(cdp, timezone_id).await
    }

    /// Applies UC stealth patches and user-agent overrides.
    pub async fn enable_uc_mode(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains()
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        uc::apply_uc_stealth(cdp).await?;
        if let Some(user_agent) = config.user_agent.as_deref() {
            uc::override_user_agent(cdp, user_agent, config.locale.as_deref()).await?;
        } else {
            if let Ok(version_info) = cdp.execute("Browser.getVersion").await {
                if let Some(ua) = version_info["userAgent"].as_str() {
                    if ua.contains("HeadlessChrome") {
                        let stealth_ua = ua.replace("HeadlessChrome", "Chrome");
                        let _ = uc::override_user_agent(cdp, &stealth_ua, config.locale.as_deref())
                            .await;
                    }
                }
            }
        }
        self.apply_fingerprint(config).await?;
        // Also patch the current document for pages already loaded.
        let _ = self
            .webdriver()?
            .execute(
                "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});",
                Vec::new(),
            )
            .await?;
        Ok(())
    }

    /// Applies an optional [`Fingerprint`] to the active session.
    ///
    /// The script is [`session_script`](crate::evasion::evasions::session_script),
    /// the one the pure CDP engine runs. When CDP is available it is registered via
    /// `Page.addScriptToEvaluateOnNewDocument` and CDP overrides (screen size,
    /// geolocation, timezone, UA) are issued. Otherwise the script is executed
    /// directly in the current page as a fallback.
    pub async fn apply_fingerprint(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        let Some(fp) = config.fingerprint.as_ref() else {
            return Ok(());
        };

        let script = fingerprint_script(fp);

        if let Ok(cdp) = self.cdp_client() {
            cdp.add_init_script(&script).await?;
            for (method, params) in fingerprint_overrides(fp) {
                cdp.execute_with_params(&method, params).await.ok();
            }
        } else {
            let _ = self.webdriver()?.execute(&script, Vec::new()).await?;
        }
        Ok(())
    }

    /// Makes the window the session is on refuse the ad and tracker hosts of
    /// the configuration, when [`BrowserConfig::ad_block`] was on.
    ///
    /// The session does this itself: for the window it opens with, again when
    /// it reconnects, and for each window it switches to with
    /// [`switch_to_window`](Self::switch_to_window) or
    /// [`switch_to_new_window`](Self::switch_to_new_window), because the block
    /// belongs to a window. Call it after switching windows some other way.
    /// It does nothing when no ads are blocked.
    ///
    /// The hosts are the built-in [`AdBlockList`], or the list set with
    /// [`BrowserConfig::with_ad_block_list`]. They go to the browser as
    /// `Network.enable` and `Network.setBlockedURLs`, through chromedriver's
    /// DevTools channel, which exists in every Chromium session whatever its
    /// mode. Firefox, Safari and Internet Explorer have none: nothing is sent
    /// and a warning is logged when the session connects.
    ///
    /// # Errors
    ///
    /// Returns the browser's refusal if it rejects a command, and
    /// [`SeleniumBaseError::Unsupported`] if the session is not connected.
    pub async fn apply_ad_block(&self) -> Result<(), SeleniumBaseError> {
        if self.ad_block.is_empty() {
            return Ok(());
        }
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session is not connected".to_owned())
        })?;
        // A WebDriver session of any mode can send DevTools commands; only the
        // CDP and UC modes keep a client for it.
        let own;
        let cdp = match self.cdp.as_ref() {
            Some(cdp) => cdp,
            None => {
                own = CdpClient::from_handle(driver.handle().clone());
                &own
            }
        };
        // The window may hold more than the library's list: what its plugins
        // blocked is put back with it, because the list replaces the window's.
        #[cfg(feature = "plugins")]
        let union = match &self.plugins {
            Some(plugins) => plugins.window_blocklist(driver).await,
            None => None,
        };
        #[cfg(not(feature = "plugins"))]
        let union: Option<Vec<String>> = None;
        for (method, params) in &self.ad_block {
            let params = match &union {
                Some(urls) if *method == "Network.setBlockedURLs" => json!({ "urls": urls }),
                _ => params.clone(),
            };
            cdp.execute_with_params(method, params).await?;
        }
        Ok(())
    }

    /// Makes the window the session is on a phone, for an undetected-mode
    /// session that was asked to be mobile ([`BrowserConfig::mobile`] or
    /// [`BrowserConfig::with_device_metrics`]).
    ///
    /// The other modes ask for the phone with the `mobileEmulation`
    /// capability when the browser starts. The undetected mode drops that
    /// capability, which a page can see, and sends the DevTools commands
    /// instead (see [`DeviceMetrics::emulation_commands`](crate::browser::options::DeviceMetrics::emulation_commands)):
    /// device metrics with `mobile: true`, touch, and the user agent. The
    /// session does this itself for the window it opens with, again when it
    /// reconnects, and for each window it switches to with
    /// [`switch_to_window`](Self::switch_to_window) or
    /// [`switch_to_new_window`](Self::switch_to_new_window), because the
    /// emulation belongs to a window. Call it after switching windows some
    /// other way. It does nothing when the session sends no such commands.
    ///
    /// # Errors
    ///
    /// Returns the browser's refusal if it rejects a command, and
    /// [`SeleniumBaseError::Unsupported`] if the session is not connected.
    pub async fn apply_mobile_emulation(&self) -> Result<(), SeleniumBaseError> {
        if self.mobile.is_empty() {
            return Ok(());
        }
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session is not connected".to_owned())
        })?;
        let own;
        let cdp = match self.cdp.as_ref() {
            Some(cdp) => cdp,
            None => {
                own = CdpClient::from_handle(driver.handle().clone());
                &own
            }
        };
        for (method, params) in &self.mobile {
            cdp.execute_with_params(method, params.clone()).await?;
        }
        Ok(())
    }

    /// Disconnects the current WebDriver session and creates a fresh one.
    ///
    /// This is useful in undetected (UC) mode to obtain a clean session handle.
    /// When the new session started its own chromedriver, the previous
    /// chromedriver process is stopped, so reconnecting does not leak one each time.
    ///
    /// With browser plugins (feature `plugins`), the old browser's plugins
    /// hear it close, and the new browser goes through the hooks as
    /// [`connect`](Self::connect) describes, `before_launch` starting again
    /// from `config`.
    ///
    /// A direct CDP attachment (`cdp_url` or `cdp_discovery`) is attached to
    /// again, at the same endpoint: the old attachment is detached, the app
    /// keeps running, and no browser or driver is ever started.
    pub async fn reconnect(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if self.external_page.is_some() {
            return self.reconnect_cdp(config).await;
        }
        #[cfg(feature = "plugins")]
        if self.plugins.is_some() || !config.plugins().is_empty() {
            return self.reconnect_with_plugins(config).await;
        }
        self.reconnect_plain(config).await
    }

    /// [`reconnect`](Self::reconnect) for a direct CDP attachment: the old
    /// attachment is let go, its sessions detached with the app left running,
    /// and the endpoint of `config` is attached to again. The browser is the
    /// app's and there is no driver, so nothing is ever launched.
    ///
    /// A reconnect that fails leaves the session without a page, as a
    /// WebDriver reconnect that fails leaves it without a driver.
    async fn reconnect_cdp(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if config.cdp_url.is_none() && !config.cdp_discovery {
            return Err(SeleniumBaseError::invalid_config(
                "a direct CDP attachment reconnects to the endpoint of its configuration: \
                     set cdp_url or cdp_discovery",
            ));
        }
        // The old window is let go first, so the plugins hear it close before
        // the new attachment starts and no window is prepared twice.
        if let Some(page) = self.external_page.take() {
            page.browser().close().await?;
        }
        let mut fresh = Self::connect_cdp(config).await?;
        self.external_page = fresh.external_page.take();
        Ok(())
    }

    /// [`reconnect`](Self::reconnect) without running plugin hooks.
    async fn reconnect_plain(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        // Worked out before anything is torn down, so a bad list leaves the
        // session as it was.
        let ad_block = ad_block_commands(config)?;
        proxy_auth::login_for(config)?;
        if let Some(sidecar) = self.proxy_login.take() {
            sidecar.close().await;
        }
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        // A reused browser has just been closed: it leaves the registry, which
        // stops its driver. The new session is this test's own.
        if let Some(lease) = self.reuse.take() {
            lease.discard();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let display = self.virtual_display.as_ref().map(XvfbProcess::display);
        let (driver, process) = connect_driver(config, display).await?;
        self.driver = Some(driver);
        if let Some(process) = process {
            if let Some(mut previous) = self.driver_process.replace(process) {
                previous.kill();
            }
        }
        self.cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(
                self.driver.as_ref().unwrap().handle().clone(),
            ))
        } else {
            None
        };
        self.ad_block = ad_block;
        self.mobile = mobile_commands(config);
        self.begin_proxy_login(config).await?;
        self.initialize_mode(config).await?;
        Ok(())
    }

    /// Closes the browser session and any auto-started driver process.
    ///
    /// A session on a browser kept for reuse gives the browser back instead,
    /// open, for the next test.
    #[instrument(skip(self))]
    pub async fn quit(&mut self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.take() {
            return page.browser().close().await;
        }
        if let Some(lease) = self.reuse.take() {
            info!("giving the browser back for reuse");
            self.keep_browser_open();
            drop(lease);
            return Ok(());
        }
        info!("quitting browser session");
        if let Some(sidecar) = self.proxy_login.take() {
            sidecar.close().await;
        }
        // The plugins hear it while the browser still runs.
        #[cfg(feature = "plugins")]
        if let Some(plugins) = self.plugins.take() {
            plugins.close().await;
        }
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        if let Some(mut process) = self.driver_process.take() {
            process.kill();
        }
        // The browser is gone, so its screen can be too.
        self.virtual_display = None;
        Ok(())
    }
}

impl Drop for BrowserSession {
    /// Best-effort cleanup when a session is dropped without an explicit
    /// `quit()` call. This improves disposability (Twelve-Factor IX) by
    /// reducing leaked browser/driver processes in short-lived scripts.
    fn drop(&mut self) {
        if self.reuse.is_some() {
            // The browser stays open; the lease, dropped with the session,
            // gives it back.
            self.keep_browser_open();
            return;
        }
        if self.driver.is_none() {
            return;
        }
        let driver = self.driver.take();
        let process = self.driver_process.take();
        let virtual_display = self.virtual_display.take();
        #[cfg(feature = "plugins")]
        let plugins = self.plugins.take();
        // `drop` cannot await, so the cleanup runs in the background. It is
        // registered so a program that exits next can wait for it with
        // `common::shutdown::drain_cleanups`.
        crate::common::shutdown::spawn_cleanup(async move {
            #[cfg(feature = "plugins")]
            if let Some(plugins) = plugins {
                plugins.close().await;
            }
            if let Some(driver) = driver {
                let _ = driver.quit().await;
            }
            if let Some(mut process) = process {
                process.kill();
            }
            drop(virtual_display);
        });
    }
}

/// Browser plugins (feature `plugins`, ADR 004); see
/// [`BrowserConfig::with_plugins`].
#[cfg(feature = "plugins")]
impl BrowserSession {
    /// Gives the window the session is on the packages and `on_page_created`
    /// of its browser plugins (feature `plugins`).
    ///
    /// The session does this itself for its first window and for each window
    /// it opens with [`switch_to_new_window`](Self::switch_to_new_window).
    /// Call it after reaching a window some other way, such as a pop-up the
    /// page opened. It does nothing for a session without plugins.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for the first package or hook that
    /// fails.
    pub async fn apply_plugins_to_window(&self) -> Result<(), SeleniumBaseError> {
        match &self.plugins {
            Some(plugins) => plugins.prepare_window(self.webdriver()?).await,
            None => Ok(()),
        }
    }

    /// [`connect`](Self::connect) for a configuration with plugins.
    async fn connect_with_plugins(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        let config = plugin_hooks::before_launch(config).await?;
        let family = plugin_hooks::family_of(config.browser)?;
        let plugins = config.plugins().clone();
        let mut session = match Self::connect_new(config.clone()).await {
            Ok(session) => session,
            Err(error) => {
                // `before_launch` ran for a launch that ends here.
                plugin_hooks::close(&plugins).await;
                return Err(error);
            }
        };
        session.check_server_for(&plugins, family).await?;
        session
            .start_plugins(SessionPlugins::new(plugins, family), &config)
            .await?;
        Ok(session)
    }

    /// Checks `plugins` again against what the server says it is, now that
    /// it has answered: a Tauri window is an embedded webview, whatever
    /// browser the configuration named. A refusal ends the session and tells
    /// the plugins the launch ended, since `before_launch` already ran.
    async fn check_server_for(
        &mut self,
        plugins: &seleniumbase_rs_plugin::PluginSet,
        family: seleniumbase_rs_plugin::BrowserFamily,
    ) -> Result<(), SeleniumBaseError> {
        let capabilities =
            serde_json::to_value(self.webdriver()?.handle().capabilities()).unwrap_or(Value::Null);
        if let Err(error) = plugin_hooks::check_server(plugins, family, &capabilities) {
            if let Err(quit) = self.quit().await {
                tracing::warn!(
                    error = %quit,
                    "the browser of a session its plugins refused did not close"
                );
            }
            plugin_hooks::close(plugins).await;
            return Err(error);
        }
        Ok(())
    }

    /// Calls `on_browser` and prepares the first window, and keeps `plugins`
    /// for the rest of the session.
    ///
    /// # Errors
    ///
    /// The first hook that fails. The browser is closed then: one without
    /// what its plugins add is not handed out.
    async fn start_plugins(
        &mut self,
        plugins: SessionPlugins,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        let mut blocked = plugin_hooks::blocked_urls_of(&self.ad_block);
        for url in fingerprint_blocklist(config) {
            if !blocked.contains(&url) {
                blocked.push(url);
            }
        }
        let plugins = plugins.with_blocked(blocked);
        let started = plugins.start(self.webdriver()?).await;
        self.plugins = Some(plugins);
        if let Err(error) = started {
            if let Err(quit) = self.quit().await {
                tracing::warn!(
                    error = %quit,
                    "the browser of a session its plugins stopped did not close"
                );
            }
            return Err(error);
        }
        Ok(())
    }

    /// [`reconnect`](Self::reconnect) for a session or configuration with
    /// plugins.
    async fn reconnect_with_plugins(
        &mut self,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        // Checked first, so a refusal leaves the session as it was.
        let prepared = plugin_hooks::before_launch(config.clone()).await?;
        if let Some(old) = self.plugins.take() {
            old.close().await;
        }
        if let Err(error) = self.reconnect_plain(&prepared).await {
            plugin_hooks::close(prepared.plugins()).await;
            return Err(error);
        }
        if prepared.plugins().is_empty() {
            return Ok(());
        }
        let family = plugin_hooks::family_of(prepared.browser)?;
        self.check_server_for(prepared.plugins(), family).await?;
        self.start_plugins(
            SessionPlugins::new(prepared.plugins().clone(), family),
            &prepared,
        )
        .await
    }
}

impl BrowserSession {
    async fn initialize_mode(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if config.is_cdp_enabled() {
            self.activate_cdp_mode().await?;
        }
        if config.is_uc_enabled() {
            self.enable_uc_mode(config).await?;
        } else if config.is_cdp_enabled() {
            // CDP-only mode still benefits from fingerprint spoofing.
            self.apply_fingerprint(config).await?;
        }
        self.apply_ad_block().await?;
        self.apply_mobile_emulation().await?;
        Ok(())
    }

    fn cdp_client(&self) -> Result<&CdpClient, SeleniumBaseError> {
        self.cdp.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported(
                "CDP is unavailable for this session. Enable mode cdp or uc.".to_owned(),
            )
        })
    }
}

/// The error of a wait that ran out of time: a [`SeleniumBaseError::WaitTimeout`]
/// that names what was waited for and for how long.
fn timed_out(what: String, timeout_secs: u64) -> SeleniumBaseError {
    SeleniumBaseError::wait_timeout(
        format!("timed out waiting for {what}"),
        Some(Duration::from_secs(timeout_secs)),
    )
}

/// The text Python's text checks compare for `element`: the value of an
/// `<input>` or a `<textarea>`, whose rendered text is empty, and the rendered
/// text of any other element.
pub(crate) async fn shown_text(element: &WebElement) -> Result<String, SeleniumBaseError> {
    let tag = element.tag_name().await.unwrap_or_default();
    if tag.eq_ignore_ascii_case("input") || tag.eq_ignore_ascii_case("textarea") {
        return Ok(element.prop("value").await?.unwrap_or_default());
    }
    Ok(element.text().await?)
}

fn validate_mode_support(config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    if config.is_cdp_enabled() && !config.browser.is_chromium_based() {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "CDP/UC mode requires a Chromium-based browser, not {}",
            config.browser
        )));
    }
    if config.is_uc_enabled()
        && (config.browser == Browser::ChromeHeadlessShell || headless1_uses_shell(config))
    {
        return Err(SeleniumBaseError::InvalidConfig(
            "chrome-headless-shell cannot run the undetected (UC) mode".to_owned(),
        ));
    }
    Ok(())
}

/// Opens a WebDriver session, honoring
/// [`BrowserConfig::remote_webdriver_timeout_seconds`] when it is set.
async fn connect_with_caps(
    config: &BrowserConfig,
    url: &str,
    caps: impl Into<thirtyfour::Capabilities>,
) -> Result<WebDriver, SeleniumBaseError> {
    crate::install_tls_provider();
    let url = server_url_for_joins(url);
    let url = url.as_str();
    match config.remote_webdriver_timeout_seconds {
        Some(seconds) => Ok(WebDriver::builder(url, caps)
            .request_timeout(Duration::from_secs(seconds))
            .connect()
            .await?),
        None => Ok(WebDriver::new(url, caps).await?),
    }
}

/// `url` with a trailing slash on its path, the form a WebDriver client needs.
///
/// thirtyfour joins each command path (`session`, `session/{id}/url`, ...) onto
/// the server URL as a relative URL, so a last path segment without a slash is
/// replaced: `http://hub:4444/wd/hub` would send `POST /wd/session`, and a
/// server behind a path prefix (such as tauri-plugin-sbrs-automation's token)
/// would never be reached.
pub(crate) fn server_url_for_joins(url: &str) -> String {
    if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    }
}

async fn try_connect(config: &BrowserConfig, url: &str) -> Result<WebDriver, SeleniumBaseError> {
    crate::browser::downloads::create_folder_for(config.browser);
    let caps = session_capabilities(config)?;
    connect_with_caps(config, url, caps).await
}

/// The capabilities a session for `config` asks the WebDriver server for: what
/// the settings produce, with [`LaunchSettings::capabilities`] merged over it.
fn session_capabilities(
    config: &BrowserConfig,
) -> Result<thirtyfour::Capabilities, SeleniumBaseError> {
    let mut caps: thirtyfour::Capabilities = match config.browser {
        // Brave, Comet, Opera and the two Chrome builds are driven as Chrome is,
        // with their own program set as the binary.
        Browser::Chrome
        | Browser::Chromium
        | Browser::Brave
        | Browser::Comet
        | Browser::Opera
        | Browser::ChromeForTesting
        | Browser::ChromeHeadlessShell => {
            let mut caps = DesiredCapabilities::chrome();
            apply_chromium_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::Edge => {
            let mut caps = DesiredCapabilities::edge();
            apply_chromium_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::Firefox => {
            let mut caps = DesiredCapabilities::firefox();
            apply_firefox_capabilities(&mut caps, config)?;
            caps.into()
        }
        // The server's own browser: only the capabilities the caller gave.
        Browser::Remote => thirtyfour::Capabilities::new(),
        Browser::Safari => {
            let mut caps = DesiredCapabilities::safari();
            apply_generic_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::InternetExplorer => {
            let mut caps = DesiredCapabilities::internet_explorer();
            apply_generic_capabilities(&mut caps, config)?;
            caps.into()
        }
    };
    // Downloads go to ./downloaded_files, as in Python; what the capabilities
    // the caller gave name for themselves (merged next) wins.
    crate::browser::downloads::ask_for_downloads_in(
        &mut caps,
        config.browser,
        &crate::browser::downloads::downloads_dir(),
    );
    // Chrome's password prompts take the keyboard from the page, so they stay
    // off, as in Python; what the capabilities the caller gave name wins.
    crate::browser::password_prompts::ask_for_quiet_passwords_in(&mut caps, config.browser);
    // Recorder Mode polls the page while a person works in it: a dialog the
    // person has not answered must stay open, not be dismissed by the next
    // command (chromedriver's default is to dismiss it and fail the command).
    if config.recorder_mode()
        && config.browser.is_chromium_based()
        && caps.get_mut("unhandledPromptBehavior").is_none()
    {
        // `Capabilities::set` only fails when serialising, which text cannot.
        let _ = caps.set("unhandledPromptBehavior", "ignore");
    }

    config.launch.capabilities.apply_to(&mut caps);
    Ok(caps)
}

/// What every browser's capabilities take from the settings: the page load
/// strategy. Safari and Internet Explorer have nothing else to set.
fn apply_generic_capabilities(
    caps: &mut impl thirtyfour::CapabilitiesHelper,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    if let Some(strategy) = config.launch.page_load_strategy {
        caps.set_page_load_strategy(strategy.into())?;
    }
    Ok(())
}

/// Firefox's `moz:firefoxOptions` and page load strategy from the settings: the
/// headless flag, the browser program, the profile folder (`-profile`, from
/// `user_data_dir`), the arguments, and the preferences, among them the user
/// agent and the language as Python SeleniumBase sets them.
fn apply_firefox_capabilities(
    caps: &mut thirtyfour::FirefoxCapabilities,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    use thirtyfour::CapabilitiesHelper;
    let launch = &config.launch;
    if config.headless {
        caps.add_arg("-headless")?;
    }
    if launch.has(LaunchSwitch::Incognito) {
        caps.add_arg("-private")?;
    }
    if launch.has(LaunchSwitch::Devtools) && !config.headless {
        caps.add_arg("-devtools")?;
    }
    for switch in [
        LaunchSwitch::DarkMode,
        LaunchSwitch::Guest,
        LaunchSwitch::DisableWebSecurity,
        LaunchSwitch::Swiftshader,
        LaunchSwitch::DisableGpu,
    ] {
        if launch.has(switch) {
            tracing::warn!("{switch:?} needs a Chromium browser; Firefox ignores it");
        }
    }
    for arg in launch.firefox.args() {
        caps.add_arg(arg)?;
    }
    if let Some(profile) = &config.user_data_dir {
        caps.add_arg("-profile")?;
        caps.add_arg(profile)?;
    }
    if let Some(binary) = &config.browser_binary_path {
        caps.set_firefox_binary(&binary.to_string_lossy())?;
    }
    if let Some(strategy) = launch.page_load_strategy {
        caps.set_page_load_strategy(strategy.into())?;
    }
    let mut from_settings = firefox_preferences(&launch.switches);
    if let Some(proxy) = &config.proxy {
        let proxy: crate::browser::options::ProxyAddress = proxy.parse()?;
        if proxy.credentials().is_some() {
            return Err(SeleniumBaseError::invalid_config(
                crate::browser::config::FIREFOX_PROXY_LOGIN,
            ));
        }
        from_settings.extend(proxy.firefox_preferences(&launch.proxy_bypass_list));
    }
    if let Some(pac) = &config.proxy_pac_url {
        let pac: crate::browser::options::PacUrl = pac.parse()?;
        if pac.credentials().is_some() {
            return Err(SeleniumBaseError::invalid_config(
                crate::browser::config::FIREFOX_PROXY_LOGIN,
            ));
        }
        from_settings.extend(pac.firefox_preferences());
    }
    if let Some(agent) = &config.user_agent {
        from_settings.insert("general.useragent.override".into(), json!(agent));
    }
    if let Some(locale) = &config.locale {
        from_settings.insert("intl.accept_languages".into(), json!(locale));
    }
    let preferences = launch.firefox.preferences_over(from_settings);
    if !preferences.is_empty() {
        let preferences: thirtyfour::common::capabilities::firefox::FirefoxPreferences =
            serde_json::from_value(Value::Object(preferences))?;
        caps.set_preferences(preferences)?;
    }
    Ok(())
}

/// Moves and resizes the window of a running session, for the browsers that
/// cannot be given the geometry as a launch argument.
async fn apply_window_rect(
    driver: &WebDriver,
    launch: &LaunchSettings,
) -> Result<(), SeleniumBaseError> {
    if launch.window_size.is_none() && launch.window_position.is_none() {
        return Ok(());
    }
    let current = driver.get_window_rect().await?;
    let (x, y) = launch
        .window_position
        .map_or((current.x, current.y), |p| (i64::from(p.x), i64::from(p.y)));
    let (width, height) = launch.window_size.map_or(
        (
            u32::try_from(current.width).unwrap_or(1),
            u32::try_from(current.height).unwrap_or(1),
        ),
        |s| (s.width, s.height),
    );
    driver.set_window_rect(x, y, width, height).await?;
    Ok(())
}

/// The init script that makes a WebDriver session match `fp`: the same one the
/// pure CDP engine runs, since both drive a real Chrome.
fn fingerprint_script(fp: &crate::evasion::fingerprint::Fingerprint) -> String {
    crate::evasion::evasions::session_script(fp)
}

/// The protocol commands, after the init script, that make a WebDriver session
/// match `fp`.
///
/// The `Proxy-Authorization` header a fingerprint with a proxy password can ask
/// for is left out: set through `Network.setExtraHTTPHeaders` it would reach
/// every site the browser visits, and hand the proxy's login to all of them.
fn fingerprint_overrides(fp: &crate::evasion::fingerprint::Fingerprint) -> Vec<(String, Value)> {
    crate::evasion::evasions::safe_cdp_overrides(fp)
}

/// The URL patterns of the `Network.setBlockedURLs` commands in `commands`,
/// each once.
pub(crate) fn blocked_patterns<M: AsRef<str>>(commands: &[(M, Value)]) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for (_, params) in commands
        .iter()
        .filter(|(method, _)| method.as_ref() == "Network.setBlockedURLs")
    {
        let listed = params.get("urls").and_then(Value::as_array);
        for url in listed.into_iter().flatten().filter_map(Value::as_str) {
            if !urls.iter().any(|seen| seen == url) {
                urls.push(url.to_owned());
            }
        }
    }
    urls
}

/// The URL patterns the fingerprint of `config` makes the first window block
/// (`block_trackers`); none when no fingerprint is applied in this mode.
pub(crate) fn fingerprint_blocklist(config: &BrowserConfig) -> Vec<String> {
    match config.fingerprint.as_ref() {
        Some(fp) if config.is_uc_enabled() || config.is_cdp_enabled() => {
            blocked_patterns(&fingerprint_overrides(fp))
        }
        _ => Vec::new(),
    }
}

/// The DevTools commands that make a session refuse the hosts `config` blocks,
/// in the order they are sent; none if `config` blocks nothing or the browser
/// has no DevTools channel.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a list that needs more
/// patterns than the browser is given.
fn ad_block_commands(
    config: &BrowserConfig,
) -> Result<Vec<(&'static str, Value)>, SeleniumBaseError> {
    let Some(list) = AdBlockList::for_config(config) else {
        return Ok(Vec::new());
    };
    if !config.browser.is_chromium_based() {
        tracing::warn!(
            browser = %config.browser,
            "ad_block needs the DevTools Protocol, which WebDriver does not give this browser; \
             nothing is blocked"
        );
        return Ok(Vec::new());
    }
    let mut patterns = list.bounded_url_patterns()?;
    if patterns.is_empty() {
        return Ok(Vec::new());
    }
    // In the CDP and UC modes the fingerprint's own block (its tracker hosts and
    // blocked ports) goes out first through the same command, and a second
    // `Network.setBlockedURLs` replaces the first, so the list carries them.
    // Plain WebDriver applies no fingerprint, so it has nothing to join.
    for tracker in fingerprint_blocklist(config) {
        if !patterns.contains(&tracker) {
            patterns.push(tracker);
        }
    }
    if patterns.len() > MAX_URL_PATTERNS {
        return Err(SeleniumBaseError::invalid_config(format!(
            "the ad-block list and the fingerprint's trackers need {} URL patterns, more than the \
             {MAX_URL_PATTERNS} the browser is given; remove hosts",
            patterns.len()
        )));
    }
    Ok(vec![
        ("Network.enable", json!({})),
        ("Network.setBlockedURLs", json!({ "urls": patterns })),
    ])
}

/// The DevTools commands that make each window of `config`'s session a phone,
/// or none.
///
/// Only the undetected mode needs them: the other Chromium modes are given the
/// `mobileEmulation` capability. A browser that is not Chromium-based cannot
/// emulate a phone, so it starts as a desktop browser, and a warning says so.
fn mobile_commands(config: &BrowserConfig) -> Vec<(&'static str, Value)> {
    let Some((metrics, user_agent)) = emulated_phone(
        config.mobile,
        config.launch.device_metrics,
        config.user_agent.as_deref(),
    ) else {
        return Vec::new();
    };
    if !config.browser.is_chromium_based() {
        tracing::warn!(
            "mobile emulation needs a Chromium browser; {} starts as a desktop browser",
            config.browser
        );
        return Vec::new();
    }
    if !config.is_uc_enabled() {
        return Vec::new();
    }
    metrics.emulation_commands(user_agent)
}

/// Whether `headless1` turns this configuration's Chrome into
/// `chrome-headless-shell`: only Chrome with no program of its own named has a
/// shell to stand for it.
fn headless1_uses_shell(config: &BrowserConfig) -> bool {
    config.launch.has(LaunchSwitch::Headless1)
        && config.browser == Browser::Chrome
        && config.browser_binary_path.is_none()
}

/// Settles the headless variants of `config` before a browser is asked for.
///
/// `chrome-headless-shell` always runs headless, so a headed request is
/// overridden. `headless1` makes the session headless and, for Chrome, runs
/// `chrome-headless-shell` (downloaded when missing, as `--chs` does); another
/// Chromium browser has no such program and runs in its new headless mode, and
/// Firefox simply runs headless. (`headless2` is plain headless.)
fn apply_headless_variants(config: &mut BrowserConfig) {
    if config.browser == Browser::ChromeHeadlessShell && !config.headless {
        tracing::info!(
            "chrome-headless-shell only runs headless; the headed request is overridden"
        );
        config.headless = true;
    }
    if !config.launch.has(LaunchSwitch::Headless1) {
        return;
    }
    config.headless = true;
    if headless1_uses_shell(config) {
        config.browser = Browser::ChromeHeadlessShell;
    } else if config.browser.is_chromium_based() && config.browser != Browser::ChromeHeadlessShell {
        tracing::info!(
            "headless1 needs chrome-headless-shell, which only replaces Chrome with no program of \
             its own named; {} runs in its new headless mode",
            config.browser
        );
    }
}

/// Starts the virtual screen the settings ask for, if this session can use one.
///
/// Python SeleniumBase's `--xvfb` does the same on Linux and nothing elsewhere. A
/// session that connects to a driver somebody else started cannot have its
/// browser moved to another screen, so it gets none either.
fn start_virtual_display(config: &BrowserConfig) -> Result<Option<XvfbProcess>, SeleniumBaseError> {
    let Some(screen) = config.launch.virtual_display else {
        return Ok(None);
    };
    if !virtual_display::wanted(Some(screen), config.headless, cfg!(target_os = "linux")) {
        tracing::info!("a virtual display was asked for, but this session is headless or not on Linux; none is started");
        return Ok(None);
    }
    if !config.auto_start_driver || !config.is_default_webdriver_url() {
        tracing::warn!(
            "a virtual display only works with a driver this crate starts; none is started"
        );
        return Ok(None);
    }
    XvfbProcess::start(screen).map(Some)
}

/// Settles the port of Chrome's remote debugger before the browser starts.
///
/// A browser this crate starts on this machine gets the port that was asked
/// for when it is free. When it is busy, the default port (`with_remote_debug`)
/// gives way to a free one, and a named port (`with_remote_debug_port`) is an
/// error. A browser on a WebDriver server of its own cannot be checked from
/// here. Returns the port the session will have, or `None` for a browser with no
/// remote debugger.
fn resolve_remote_debug(config: &mut BrowserConfig) -> Result<Option<u16>, SeleniumBaseError> {
    let Some(port) = config.launch.remote_debug_port else {
        return Ok(None);
    };
    if !config.browser.is_chromium_based() {
        return Ok(None);
    }
    if !config.auto_start_driver || !config.is_default_webdriver_url() {
        return Ok(Some(port));
    }
    let port = crate::browser::launcher::choose_debug_port(
        port,
        config.launch.remote_debug_any_port,
        crate::browser::launcher::port_is_free,
        crate::browser::launcher::find_free_port,
    )?;
    config.launch.remote_debug_port = Some(port);
    info!("Chrome's remote debugger listens on localhost:{port}; open chrome://inspect");
    Ok(Some(port))
}

async fn connect_driver(
    config: &BrowserConfig,
    display: Option<&str>,
) -> Result<(WebDriver, Option<DriverProcess>), SeleniumBaseError> {
    let mut config = config.clone();
    apply_headless_variants(&mut config);
    resolve_browser_binary(&mut config).await?;
    patch_chrome_binary_if_needed(&mut config).await?;

    let mut process: Option<DriverProcess> = None;
    let mut url = config.webdriver_url.clone();

    if config.auto_start_driver && config.is_default_webdriver_url() {
        match try_connect(&config, &url).await {
            Ok(driver) => return Ok((driver, None)),
            Err(_) => {
                let launched = launch_driver_for(&config, display).await?;
                url = launched.url.clone();
                process = Some(launched);
            }
        }
    }

    let driver = try_connect(&config, &url).await?;
    Ok((driver, process))
}

/// Finds the program of a browser that is not Google Chrome and sets it as
/// the browser binary, unless the caller named one.
///
/// # Errors
///
/// Returns an error that says where it looked when Brave, Comet or Opera cannot
/// be found, or the download error when Chrome for Testing or
/// `chrome-headless-shell`, which are fetched when missing, cannot be.
async fn resolve_browser_binary(config: &mut BrowserConfig) -> Result<(), SeleniumBaseError> {
    let needs_program = matches!(
        config.browser,
        Browser::Brave
            | Browser::Comet
            | Browser::Opera
            | Browser::Chromium
            | Browser::ChromeForTesting
            | Browser::ChromeHeadlessShell
    );
    if !needs_program || config.browser_binary_path.is_some() {
        return Ok(());
    }
    let proxy = config
        .launch
        .has(LaunchSwitch::ProxyForDriverDownload)
        .then_some(config.proxy.as_deref())
        .flatten();
    let found = match binaries::locate(config.browser) {
        Some(found) => found,
        // Chrome for Testing, chrome-headless-shell and Chromium are downloads, so a missing
        // one is fetched: the stable build of the first two, the newest snapshot of Chromium.
        None => match config.browser {
            Browser::ChromeForTesting => {
                let stable = DriverVersion::Channel(ReleaseChannel::Stable);
                install_chrome_for_testing(&stable, proxy).await?
            }
            Browser::ChromeHeadlessShell => {
                let stable = DriverVersion::Channel(ReleaseChannel::Stable);
                install_chrome_headless_shell(&stable, proxy).await?
            }
            Browser::Chromium => install_chromium(None, proxy).await?,
            _ => {
                return Err(SeleniumBaseError::invalid_config(missing_browser_message(
                    config.browser,
                )))
            }
        },
    };
    // A chromedriver drives only the Chrome of its own major version, and the Chromium found
    // is not the Chrome that is installed: the driver follows the Chromium.
    if config.browser == Browser::Chromium && config.launch.driver_version.is_none() {
        config.launch.driver_version = Some(DriverVersion::MatchBrowser);
    }
    config.browser_binary_path = Some(found);
    Ok(())
}

/// What to tell the user when a browser's program cannot be found.
fn missing_browser_message(browser: Browser) -> String {
    match browser {
        Browser::ChromeForTesting | Browser::ChromeHeadlessShell => format!(
            "{browser} is not installed: nothing was found under {}/{}. Download it \
             there, or name its program with BrowserConfig::with_browser_binary_path.",
            binaries::DOWNLOADS_DIR,
            if browser == Browser::ChromeForTesting {
                binaries::CHROME_FOR_TESTING_DIR
            } else {
                binaries::CHROME_HEADLESS_SHELL_DIR
            },
        ),
        _ => format!(
            "{browser} was not found in its usual install locations or on the PATH. \
             Install it, or name its program with BrowserConfig::with_browser_binary_path."
        ),
    }
}

async fn patch_chrome_binary_if_needed(
    config: &mut BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    if !config.native_spoofing_enabled() {
        return Ok(());
    }
    if !matches!(
        config.browser,
        Browser::Chrome | Browser::Chromium | Browser::Edge
    ) {
        return Ok(());
    }
    if config.browser_binary_path.is_some() {
        return Ok(());
    }

    // A bad unrelated variable must not hide SB_CHROME_BIN.
    let runtime = crate::config::RuntimeConfig::from_env_lenient().0;
    let source = runtime
        .chrome_bin
        .or_else(crate::evasion::patcher::find_system_chrome)
        .ok_or_else(|| {
            SeleniumBaseError::invalid_config(
                "native_spoofing requires a Chrome/Chromium binary but none was found. \
                 Set SB_CHROME_BIN or BrowserConfig::browser_binary_path."
                    .to_owned(),
            )
        })?;

    let source_str = source.display().to_string();
    tracing::info!(source = %source_str, "native_spoofing enabled: patching chrome binary");

    let patched = tokio::task::spawn_blocking(move || {
        let mut patcher = crate::evasion::patcher::ChromeBinaryPatcher::new(source);
        if let Some(cache) = runtime.patch_cache_dir {
            patcher = patcher.with_cache_dir(cache);
        }
        patcher.patch(crate::evasion::patcher::EnginePatch::chrome_binary())
    })
    .await
    .map_err(|e| {
        SeleniumBaseError::patcher(source_str.clone(), format!("patch task panicked: {e}"))
    })??;

    tracing::info!(path = %patched.display(), "using patched chrome binary");
    config.browser_binary_path = Some(patched);
    Ok(())
}

fn apply_chromium_capabilities<C: ChromiumLikeCapabilities>(
    caps: &mut C,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    crate::evasion::options::StealthOptions::from(config).apply_to(caps)
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    use crate::evasion::evasions::{raw_overrides, CREDENTIAL_HEADER_METHOD};
    use crate::evasion::fingerprint::{Fingerprint, OsType, ProxyConfig};

    fn with_proxy_login() -> Fingerprint {
        Fingerprint::randomized(OsType::Linux, 3)
            .to_builder()
            .proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "proxy.example.com".to_owned(),
                port: 8080,
                username: Some("alice".to_owned()),
                password: Some("fake-proxy-password".to_owned()),
                save_traffic: false,
            })
            .build()
            .expect("the builder builds")
    }

    #[test]
    fn the_proxy_login_is_never_sent_to_every_site() {
        let fingerprint = with_proxy_login();
        // The source of the overrides does ask for the header.
        assert!(raw_overrides(&fingerprint).contains_key(CREDENTIAL_HEADER_METHOD));

        let sent = fingerprint_overrides(&fingerprint);

        assert!(sent
            .iter()
            .all(|(method, _)| method != CREDENTIAL_HEADER_METHOD));
        let everything = format!("{sent:?}");
        assert!(!everything.contains("Proxy-Authorization"), "{everything}");
        assert!(!everything.contains("fake-proxy-password"), "{everything}");
        assert!(
            !everything.contains("YWxpY2U6ZmFrZS1wcm94eS1wYXNzd29yZA"),
            "{everything}"
        );
    }

    #[test]
    fn a_webdriver_session_gets_the_same_script_as_the_pure_cdp_engine() {
        // Both drive a real Chrome, which answers for plugins, mime types, the
        // PDF viewer flag and the network report itself.
        let fingerprint = Fingerprint::randomized(OsType::Linux, 3);
        let script = fingerprint_script(&fingerprint);
        assert_eq!(
            script,
            crate::evasion::providers::session_registry(&fingerprint).bootstrap(
                &crate::evasion::providers::EvasionContext::new(&fingerprint)
            )
        );
        assert!(
            !script.contains("mimeTypes"),
            "a spoof Chrome does not need"
        );
    }

    #[test]
    fn the_other_overrides_are_still_sent_in_a_stable_order() {
        let fingerprint = with_proxy_login();
        let sent = fingerprint_overrides(&fingerprint);
        let methods: Vec<_> = sent.iter().map(|(method, _)| method.as_str()).collect();
        assert!(
            methods.contains(&"Network.setUserAgentOverride"),
            "{methods:?}"
        );
        assert!(
            methods.contains(&"Emulation.setTimezoneOverride"),
            "{methods:?}"
        );
        let mut sorted = methods.clone();
        sorted.sort_unstable();
        assert_eq!(methods, sorted);
    }
}

#[cfg(test)]
mod capability_tests {
    use serde_json::json;

    use super::*;
    use crate::browser::options::{
        CapabilityOverrides, FirefoxOptions, PageLoadStrategy, WindowSize,
    };

    /// `prefs` without the preferences that name the downloads folder.
    fn without_downloads(prefs: &Value) -> Value {
        let mut rest = prefs.clone();
        if let Some(map) = rest.as_object_mut() {
            map.retain(|name, _| !name.starts_with("browser.download."));
        }
        rest
    }

    #[test]
    fn chrome_is_asked_to_save_downloads_in_downloaded_files() {
        let caps = caps_json(&BrowserConfig::default());
        let prefs = &caps["goog:chromeOptions"]["prefs"];
        let folder = crate::browser::downloads::downloads_dir();
        assert_eq!(
            prefs["download.default_directory"],
            folder.to_string_lossy().as_ref()
        );
        assert_eq!(prefs["download.prompt_for_download"], false);
    }

    #[test]
    fn a_preference_the_caller_names_wins_over_the_downloads_folder() {
        let mut config = BrowserConfig::default();
        config.launch.capabilities = CapabilityOverrides::from_text(
            r#"{"goog:chromeOptions": {"prefs": {"download.default_directory": "/elsewhere"}}}"#,
        )
        .unwrap();
        let prefs = &caps_json(&config)["goog:chromeOptions"]["prefs"];
        assert_eq!(prefs["download.default_directory"], "/elsewhere");
        assert_eq!(prefs["download.prompt_for_download"], false);
    }

    fn caps_json(config: &BrowserConfig) -> Value {
        serde_json::to_value(session_capabilities(config).unwrap()).unwrap()
    }

    #[test]
    fn firefox_gets_its_arguments_preferences_and_page_load_strategy() {
        let mut firefox = FirefoxOptions::default();
        firefox.add_args("private-window").unwrap();
        firefox
            .add_preferences("javascript.enabled:true,intl.accept_languages:fr")
            .unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(true)
            .with_block_images(true)
            .with_disable_js(true)
            .with_do_not_track(true)
            .with_page_load_strategy(PageLoadStrategy::Eager)
            .with_firefox_options(firefox);
        let caps = caps_json(&config);

        assert_eq!(caps["browserName"], "firefox");
        assert_eq!(caps["pageLoadStrategy"], "eager");
        let options = &caps["moz:firefoxOptions"];
        assert_eq!(options["args"], json!(["-headless", "--private-window"]));
        // An explicit preference wins over the one a switch sets.
        assert_eq!(
            without_downloads(&options["prefs"]),
            json!({
                "permissions.default.image": 2,
                "javascript.enabled": true,
                "privacy.donottrackheader.enabled": true,
                "intl.accept_languages": "fr",
            })
        );
    }

    #[test]
    fn a_plain_firefox_session_asks_for_no_preferences_but_the_downloads_folder() {
        let caps = caps_json(&BrowserConfig::default().with_browser(Browser::Firefox));
        let options = &caps["moz:firefoxOptions"];
        assert_eq!(without_downloads(&options["prefs"]), json!({}), "{options}");
        assert!(options.get("binary").is_none(), "{options}");
        assert!(caps.get("pageLoadStrategy").is_none());
    }

    #[test]
    fn firefox_gets_its_program_profile_user_agent_and_language() {
        let mut firefox = FirefoxOptions::default();
        firefox.add_preferences("intl.accept_languages:de").unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(true)
            .with_browser_binary_path("/opt/firefox/firefox")
            .with_user_data_dir("/tmp/ff-profile")
            .with_user_agent("Agent/1.0")
            .with_locale("fr")
            .with_firefox_options(firefox);
        let options = &caps_json(&config)["moz:firefoxOptions"];
        assert_eq!(options["binary"], "/opt/firefox/firefox");
        assert_eq!(
            options["args"],
            json!(["-headless", "-profile", "/tmp/ff-profile"])
        );
        assert_eq!(options["prefs"]["general.useragent.override"], "Agent/1.0");
        // A preference named outright wins over the one the locale sets.
        assert_eq!(options["prefs"]["intl.accept_languages"], "de");
    }

    #[test]
    fn edge_is_asked_for_as_microsoft_edge_with_its_own_options() {
        let config = BrowserConfig::default()
            .with_browser(Browser::Edge)
            .with_headless(true)
            .with_browser_binary_path("/opt/edge/msedge");
        let caps = caps_json(&config);
        assert_eq!(caps["browserName"], "MicrosoftEdge");
        assert!(caps.get("goog:chromeOptions").is_none(), "{caps}");
        let options = &caps["ms:edgeOptions"];
        assert_eq!(options["binary"], "/opt/edge/msedge");
        let args = options["args"].as_array().unwrap();
        assert!(
            args.iter().any(|arg| arg
                .as_str()
                .is_some_and(|arg| arg.starts_with("--headless"))),
            "{args:?}"
        );
    }

    #[test]
    fn capability_overrides_are_merged_into_every_browsers_capabilities() {
        let overrides = CapabilityOverrides::from_text(
            r#"{"se:name": "smoke", "goog:chromeOptions": {"args": ["--mute-audio"]}}"#,
        )
        .unwrap();
        for browser in [
            Browser::Chrome,
            Browser::Chromium,
            Browser::Edge,
            Browser::Firefox,
        ] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_capabilities(overrides.clone());
            assert_eq!(caps_json(&config)["se:name"], "smoke", "{browser:?}");
        }

        // Chrome's own arguments keep what the settings added.
        let config = BrowserConfig::default()
            .with_window_size(WindowSize::new(800, 600))
            .with_capabilities(overrides);
        let args = caps_json(&config)["goog:chromeOptions"]["args"].clone();
        let args = args.as_array().unwrap();
        assert!(args.contains(&json!("--mute-audio")), "{args:?}");
        assert!(args.contains(&json!("--window-size=800,600")), "{args:?}");
    }

    #[test]
    fn an_override_can_replace_what_the_settings_chose() {
        let config = BrowserConfig::default()
            .with_page_load_strategy(PageLoadStrategy::Eager)
            .with_capabilities(
                CapabilityOverrides::from_text(r#"{"pageLoadStrategy": "none"}"#).unwrap(),
            );
        assert_eq!(caps_json(&config)["pageLoadStrategy"], "none");
    }

    #[test]
    fn firefox_gets_its_proxy_as_preferences() {
        let firefox = BrowserConfig::default().with_browser(Browser::Firefox);
        let socks = firefox
            .clone()
            .with_proxy("socks5://proxy.test:1080")
            .with_proxy_bypass_list("*.example.com;localhost".parse().unwrap());
        let caps = caps_json(&socks);
        let prefs = &caps["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.type"], 1);
        assert_eq!(prefs["network.proxy.socks"], "proxy.test");
        assert_eq!(prefs["network.proxy.socks_port"], 1080);
        assert_eq!(prefs["network.proxy.socks_version"], 5);
        assert_eq!(
            prefs["network.proxy.no_proxies_on"],
            ".example.com, localhost"
        );

        let http = caps_json(&firefox.clone().with_proxy("h.test:8080"));
        let prefs = &http["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.http"], "h.test");
        assert_eq!(prefs["network.proxy.http_port"], 8080);
        assert_eq!(prefs["network.proxy.ssl"], "h.test");
        assert_eq!(prefs["network.proxy.ssl_port"], 8080);

        let pac = caps_json(&firefox.with_proxy_pac_url("http://pac.test/p.pac"));
        let prefs = &pac["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.type"], 2);
        assert_eq!(
            prefs["network.proxy.autoconfig_url"],
            "http://pac.test/p.pac"
        );
    }

    #[test]
    fn firefox_with_a_proxy_login_is_refused_with_pythons_message() {
        for config in [
            BrowserConfig::default()
                .with_browser(Browser::Firefox)
                .with_proxy("user:hunter2@h.test:8080"),
            BrowserConfig::default()
                .with_browser(Browser::Firefox)
                .with_proxy_pac_url("user:hunter2@http://pac.test/p.pac"),
        ] {
            let error = session_capabilities(&config).unwrap_err().to_string();
            assert!(
                error.contains("Chrome or Edge is required when using a proxy server"),
                "{error}"
            );
            assert!(!error.contains("hunter2"), "{error}");
        }
    }

    #[test]
    fn remote_asks_for_nothing_but_the_capabilities_it_was_given() {
        let caps = CapabilityOverrides::from_text(
            r#"{"browserName":"chrome","bstack:options":{"os":"Windows"}}"#,
        )
        .unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Remote)
            .with_headless(true)
            .with_block_images(true)
            .with_window_size(WindowSize::new(800, 600))
            .with_capabilities(caps);
        assert_eq!(
            caps_json(&config),
            json!({"browserName": "chrome", "bstack:options": {"os": "Windows"}})
        );
    }

    #[test]
    fn the_remote_debugger_port_is_settled_before_the_browser_starts() {
        let mut off = BrowserConfig::default();
        assert_eq!(resolve_remote_debug(&mut off).unwrap(), None);

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = listener.local_addr().unwrap().port();

        // A port that was named and is in use is an error.
        let named = BrowserConfig::default().with_remote_debug_port(busy);
        let error = resolve_remote_debug(&mut named.clone())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("port {busy} is already in use")),
            "{error}"
        );

        // The default port gives way to a free one.
        let mut moving = BrowserConfig::default();
        moving.launch.remote_debug_port = Some(busy);
        moving.launch.remote_debug_any_port = true;
        let chosen = resolve_remote_debug(&mut moving).unwrap().unwrap();
        assert_ne!(chosen, busy);
        assert_eq!(moving.launch.remote_debug_port, Some(chosen));

        // A browser on a server of its own is not checked from here, and
        // Firefox has no such debugger.
        let mut elsewhere = named
            .clone()
            .with_webdriver_url("http://hub.test:4444/wd/hub");
        assert_eq!(resolve_remote_debug(&mut elsewhere).unwrap(), Some(busy));
        let mut firefox = named.with_browser(Browser::Firefox);
        assert_eq!(resolve_remote_debug(&mut firefox).unwrap(), None);
        drop(listener);
    }

    #[test]
    fn the_remote_debugger_address_is_localhost_and_its_port() {
        let mut session = BrowserSession::disconnected();
        assert_eq!(session.remote_debug_address(), None);
        session.remote_debug = Some(9222);
        assert_eq!(
            session.remote_debug_address().as_deref(),
            Some("localhost:9222")
        );
    }
}

/// Ad blocking in a WebDriver session, against the mock WebDriver in
/// `webdriver_support` (no browser): which DevTools commands a session sends.
#[cfg(test)]
mod ad_block_tests {
    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::DriverMode;
    use crate::config::ad_block_list::MAX_URL_PATTERNS;

    const CHROMIUM: [Browser; 8] = [
        Browser::Chrome,
        Browser::Chromium,
        Browser::Edge,
        Browser::Brave,
        Browser::Comet,
        Browser::Opera,
        Browser::ChromeForTesting,
        Browser::ChromeHeadlessShell,
    ];

    fn own_list() -> AdBlockList {
        AdBlockList::from_hosts(["tracker.example.net", "ads.example.com"]).unwrap()
    }

    fn own_patterns() -> Value {
        json!([
            "*://ads.example.com/*",
            "*://*.ads.example.com/*",
            "*://tracker.example.net/*",
            "*://*.tracker.example.net/*",
        ])
    }

    /// A session on `driver` that starts no driver of its own.
    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        config
    }

    /// A WebDriver that answers every command, DevTools commands included.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |_| Reply::Value(json!({})))
    }

    /// The bodies of the DevTools commands the driver was sent, oldest first.
    fn devtools_commands(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| request.body)
            .collect()
    }

    #[test]
    fn nothing_is_sent_while_ad_blocking_is_off() {
        assert!(ad_block_commands(&BrowserConfig::default())
            .unwrap()
            .is_empty());

        // A list that is set but not asked for blocks nothing.
        let mut config = BrowserConfig::default().with_ad_block_list(own_list());
        config.ad_block = false;
        assert!(ad_block_commands(&config).unwrap().is_empty());
    }

    #[test]
    fn the_builtin_list_goes_to_the_browser_after_network_enable() {
        let commands = ad_block_commands(&BrowserConfig::default().with_ad_block(true)).unwrap();

        let patterns = AdBlockList::builtin().bounded_url_patterns().unwrap();
        assert_eq!(
            commands,
            [
                ("Network.enable", json!({})),
                ("Network.setBlockedURLs", json!({ "urls": patterns })),
            ]
        );
        let urls = commands[1].1["urls"].as_array().unwrap();
        assert!(urls.contains(&json!("*://doubleclick.net/*")));
        assert!(urls.contains(&json!("*://*.doubleclick.net/*")));
    }

    #[test]
    fn a_list_of_the_callers_own_is_sent_in_place_of_the_builtin_one() {
        let config = BrowserConfig::default().with_ad_block_list(own_list());
        let commands = ad_block_commands(&config).unwrap();

        assert_eq!(
            commands,
            [
                ("Network.enable", json!({})),
                ("Network.setBlockedURLs", json!({ "urls": own_patterns() })),
            ]
        );
    }

    #[test]
    fn the_trackers_of_a_fingerprint_go_with_the_ad_block_list() {
        use crate::evasion::fingerprint::Fingerprint;

        let fingerprint = Fingerprint::builder()
            .block_trackers(true)
            .build()
            .expect("the builder builds");
        let tracker = format!("*{}*", fingerprint.tracker_hosts()[0]);

        let mut config = BrowserConfig::default()
            .with_ad_block_list(own_list())
            .with_fingerprint(fingerprint);
        // A plain WebDriver session does not apply the fingerprint, so it has no trackers.
        assert!(fingerprint_blocklist(&config).is_empty());
        let plain = ad_block_commands(&config).unwrap();
        assert_eq!(plain[1].1, json!({ "urls": own_patterns() }));

        config.mode = DriverMode::Cdp;
        let commands = ad_block_commands(&config).unwrap();
        let urls = blocked_patterns(&commands);
        let own: Vec<String> = serde_json::from_value(own_patterns()).unwrap();
        assert_eq!(urls[..own.len()], own[..]);
        assert!(urls.contains(&tracker), "{urls:?}");
        assert!(fingerprint_blocklist(&config).contains(&tracker));
    }

    #[test]
    fn the_block_keeps_what_the_fingerprint_blocks_where_the_fingerprint_is_applied() {
        let fingerprint = crate::evasion::fingerprint::Fingerprint::builder()
            .blocked_ports(vec![9222])
            .build()
            .expect("the builder builds");
        let urls = |mode: DriverMode| {
            let config = BrowserConfig::default()
                .with_mode(mode)
                .with_ad_block_list(own_list())
                .with_fingerprint(fingerprint.clone());
            ad_block_commands(&config).unwrap()[1].1["urls"]
                .as_array()
                .unwrap()
                .clone()
        };

        // A later `Network.setBlockedURLs` replaces the fingerprint's, so the
        // list that is sent holds both.
        for mode in [DriverMode::Cdp, DriverMode::Uc] {
            let urls = urls(mode);
            assert!(urls.contains(&json!("*://localhost:9222/*")), "{mode:?}");
            for own in own_patterns().as_array().unwrap() {
                assert!(urls.contains(own), "{mode:?}: {own}");
            }
        }
        // Plain WebDriver applies no fingerprint, so there is nothing to join.
        assert_eq!(json!(urls(DriverMode::WebDriver)), own_patterns());
    }

    #[test]
    fn an_empty_list_of_the_callers_own_sends_nothing() {
        let config = BrowserConfig::default().with_ad_block_list(AdBlockList::new());
        assert!(ad_block_commands(&config).unwrap().is_empty());
    }

    #[test]
    fn every_chromium_browser_is_sent_the_block_and_the_others_nothing() {
        for browser in CHROMIUM {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_ad_block(true);
            assert_eq!(ad_block_commands(&config).unwrap().len(), 2, "{browser}");
        }
        for browser in [Browser::Firefox, Browser::Safari, Browser::InternetExplorer] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_ad_block(true);
            assert!(ad_block_commands(&config).unwrap().is_empty(), "{browser}");
        }
    }

    #[test]
    fn a_list_too_long_for_the_browser_is_refused() {
        let hosts: Vec<String> = (0..=MAX_URL_PATTERNS / 2)
            .map(|i| format!("host{i}.com"))
            .collect();
        let config =
            BrowserConfig::default().with_ad_block_list(AdBlockList::from_hosts(hosts).unwrap());
        assert!(matches!(
            ad_block_commands(&config),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }

    #[tokio::test]
    async fn a_chromium_session_sends_the_two_commands_once_it_is_connected() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());

        let mut session = BrowserSession::connect(config).await.expect("connects");

        assert_eq!(
            devtools_commands(&driver),
            [
                json!({ "cmd": "Network.enable", "params": {} }),
                json!({ "cmd": "Network.setBlockedURLs", "params": { "urls": own_patterns() } }),
            ]
        );
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_session_that_does_not_block_ads_sends_no_devtools_command() {
        let driver = driver();

        let mut session = BrowserSession::connect(config_for(&driver))
            .await
            .expect("connects");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_browser_without_a_devtools_channel_is_sent_nothing() {
        let driver = driver();
        let config = config_for(&driver)
            .with_browser(Browser::Firefox)
            .with_ad_block(true);

        let mut session = BrowserSession::connect(config).await.expect("connects");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn the_builtin_list_is_sent_when_no_list_of_ones_own_is_set() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block(true);

        let mut session = BrowserSession::connect(config).await.expect("connects");

        let sent = devtools_commands(&driver);
        let patterns = AdBlockList::builtin().bounded_url_patterns().unwrap();
        assert_eq!(sent.len(), 2, "{sent:?}");
        assert_eq!(sent[1]["params"], json!({ "urls": patterns }));
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_reconnected_session_blocks_again() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config.clone())
            .await
            .expect("connects");

        session.reconnect(&config).await.expect("reconnects");

        assert_eq!(devtools_commands(&driver).len(), 4);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_window_reached_some_other_way_can_be_blocked_with_apply_ad_block() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config).await.expect("connects");

        session.apply_ad_block().await.expect("blocks again");

        assert_eq!(devtools_commands(&driver).len(), 4);
        session.quit().await.expect("quits");
    }

    /// A WebDriver that opens windows and switches to them.
    fn window_driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "w2", "type": "tab" })),
                ("POST", "/window") => Reply::Value(Value::Null),
                _ => Reply::Value(json!({})),
            }
        })
    }

    #[tokio::test]
    async fn each_window_the_session_switches_to_is_blocked_too() {
        let driver = window_driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config).await.expect("connects");
        assert_eq!(devtools_commands(&driver).len(), 2, "the first window");

        session.switch_to_new_window().await.expect("a new window");
        assert_eq!(devtools_commands(&driver).len(), 4, "the new window");

        session.switch_to_window("w2").await.expect("a switch");
        let sent = devtools_commands(&driver);
        assert_eq!(sent.len(), 6, "the window switched to");
        assert_eq!(sent[4], sent[0]);
        assert_eq!(sent[5], sent[1]);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn switching_windows_sends_nothing_when_no_ads_are_blocked() {
        let driver = window_driver();
        let mut session = BrowserSession::connect(config_for(&driver))
            .await
            .expect("connects");

        session.switch_to_new_window().await.expect("a new window");
        session.switch_to_window("w2").await.expect("a switch");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_browser_that_refuses_the_block_fails_the_connection() {
        let driver = MockWebDriver::start(json!({}), |request| {
            if request.path == "/goog/cdp/execute" {
                Reply::Error(500, "unknown error")
            } else {
                Reply::Value(json!({}))
            }
        });
        let config = config_for(&driver).with_ad_block(true);

        let connected = BrowserSession::connect(config).await;

        assert!(
            matches!(connected, Err(SeleniumBaseError::Unsupported(_))),
            "a block that is not in place must not look like one that is"
        );
    }

    #[tokio::test]
    async fn a_list_too_long_is_refused_before_a_browser_is_asked_for() {
        let driver = driver();
        let hosts: Vec<String> = (0..=MAX_URL_PATTERNS / 2)
            .map(|i| format!("host{i}.com"))
            .collect();
        let config =
            config_for(&driver).with_ad_block_list(AdBlockList::from_hosts(hosts).unwrap());

        let connected = BrowserSession::connect(config).await;

        assert!(matches!(
            connected,
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert!(
            driver.requests("/session").is_empty(),
            "no session was opened for a configuration that cannot work"
        );
    }
}

/// How the waits of a session end: with a `WaitTimeout` that names what was
/// waited for, against the mock WebDriver in `webdriver_support`.
#[cfg(test)]
mod wait_tests {
    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{element, MockWebDriver, Reply};

    /// A WebDriver on which no element can be found, but `#present` can be
    /// listed.
    fn driver_without_elements() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn session(driver: &MockWebDriver) -> BrowserSession {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        BrowserSession::connect(config).await.expect("connects")
    }

    /// The message of the `WaitTimeout` in `result`; panics with what came
    /// instead.
    fn message_of_the_timeout<T: std::fmt::Debug>(result: Result<T, SeleniumBaseError>) -> String {
        match result {
            Err(SeleniumBaseError::WaitTimeout(message)) => message,
            other => panic!("expected a WaitTimeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn waiting_for_an_element_that_never_comes_is_a_wait_timeout_naming_it() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let message = message_of_the_timeout(session.wait_for_element(By::Css("#a"), 0).await);

        assert!(
            message.contains("timed out waiting for element CSS(#a)"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn waiting_for_visibility_and_clickability_end_in_wait_timeouts_too() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let visible =
            message_of_the_timeout(session.wait_for_element_visible(By::Css("#v"), 0).await);
        let clickable =
            message_of_the_timeout(session.wait_for_element_clickable(By::Css("#c"), 0).await);

        assert!(
            visible.contains("element CSS(#v) to be visible"),
            "{visible}"
        );
        assert!(
            clickable.contains("element CSS(#c) to be clickable"),
            "{clickable}"
        );
    }

    #[tokio::test]
    async fn waiting_for_an_element_to_go_away_ends_in_a_wait_timeout_naming_it() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        // The mock lists an element for every locator, so it never goes.
        let message =
            message_of_the_timeout(session.wait_for_element_absent(By::Css("#gone"), 0).await);

        assert!(
            message.contains("element CSS(#gone) to be absent"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn waiting_for_text_ends_in_a_wait_timeout_naming_the_text_and_the_element() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let message =
            message_of_the_timeout(session.wait_for_text(By::Css("#t"), "Welcome", 0).await);

        assert!(message.contains("text 'Welcome'"), "{message}");
        assert!(message.contains("CSS(#t)"), "{message}");
    }

    #[tokio::test]
    async fn the_message_of_a_wait_timeout_says_how_long_the_wait_was() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let error = session
            .wait_for_element(By::Css("#a"), 0)
            .await
            .expect_err("the element never comes");

        let text = error.to_string();
        assert!(text.starts_with("wait timeout: "), "{text}");
        assert!(text.ends_with("(after 0ms)"), "{text}");
    }
}

#[cfg(test)]
mod server_url_tests {
    use super::server_url_for_joins;

    fn session_url(server: &str) -> String {
        let base = url::Url::parse(&server_url_for_joins(server)).expect("a valid URL");
        base.join("session").expect("a relative path").to_string()
    }

    #[test]
    fn a_path_prefix_survives_the_command_paths() {
        assert_eq!(
            session_url("http://hub:4444/wd/hub"),
            "http://hub:4444/wd/hub/session"
        );
        assert_eq!(
            session_url("http://127.0.0.1:4545/e2etoken"),
            "http://127.0.0.1:4545/e2etoken/session"
        );
    }

    #[test]
    fn urls_that_already_work_are_unchanged() {
        assert_eq!(
            session_url("http://localhost:4444"),
            "http://localhost:4444/session"
        );
        assert_eq!(
            session_url("http://hub:4444/wd/hub/"),
            "http://hub:4444/wd/hub/session"
        );
        assert_eq!(
            server_url_for_joins("http://hub:4444/wd/hub/"),
            "http://hub:4444/wd/hub/"
        );
    }
}

#[cfg(test)]
mod mobile_tests {
    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::DriverMode;
    use crate::browser::options::{DeviceMetrics, MOBILE_USER_AGENT};

    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        config
    }

    /// A WebDriver that answers every command, opens windows and switches to
    /// them.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "w2", "type": "tab" })),
                ("POST", "/window") => Reply::Value(Value::Null),
                _ => Reply::Value(json!({})),
            }
        })
    }

    fn devtools_commands(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| request.body)
            .collect()
    }

    fn names(commands: &[Value]) -> Vec<&str> {
        commands
            .iter()
            .map(|command| command["cmd"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn only_the_undetected_mode_sends_the_phone_over_devtools() {
        let mobile = BrowserConfig::default().with_mobile(true);
        assert!(mobile_commands(&mobile).is_empty());
        assert!(mobile_commands(&BrowserConfig::default().with_mode(DriverMode::Uc)).is_empty());

        let uc = mobile.clone().with_mode(DriverMode::Uc);
        let commands = mobile_commands(&uc);
        assert_eq!(
            commands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            [
                "Emulation.setDeviceMetricsOverride",
                "Emulation.setTouchEmulationEnabled",
                "Emulation.setUserAgentOverride"
            ]
        );
        assert_eq!(
            commands[0].1,
            json!({ "width": 390, "height": 715, "deviceScaleFactor": 3.0, "mobile": true })
        );
        assert_eq!(commands[2].1["userAgent"], MOBILE_USER_AGENT);

        let own = uc
            .with_device_metrics(DeviceMetrics::new(360, 640, 2.0))
            .with_user_agent("MyAgent/1.0");
        let commands = mobile_commands(&own);
        assert_eq!(commands[0].1["width"], 360);
        assert_eq!(commands[2].1["userAgent"], "MyAgent/1.0");
    }

    #[test]
    fn a_browser_that_is_not_chromium_is_sent_no_phone() {
        let firefox = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_mobile(true);
        assert!(mobile_commands(&firefox).is_empty());
    }

    #[tokio::test]
    async fn an_undetected_mobile_session_is_made_a_phone_and_each_new_window_too() {
        let driver = driver();
        let config = config_for(&driver)
            .with_mode(DriverMode::Uc)
            .with_mobile(true);
        let mut session = match BrowserSession::connect(config).await {
            Ok(session) => session,
            Err(error) => panic!("connects: {error}"),
        };
        let first = devtools_commands(&driver);
        let phone: Vec<&str> = names(&first)
            .into_iter()
            .filter(|name| name.starts_with("Emulation."))
            .collect();
        assert_eq!(
            phone,
            [
                "Emulation.setDeviceMetricsOverride",
                "Emulation.setTouchEmulationEnabled",
                "Emulation.setUserAgentOverride"
            ],
            "{first:?}"
        );

        let before = first.len();
        session.switch_to_new_window().await.expect("a new window");
        let after = devtools_commands(&driver);
        assert_eq!(after.len(), before + 3, "{after:?}");
        assert_eq!(names(&after)[before], "Emulation.setDeviceMetricsOverride");

        session.switch_to_window("w2").await.expect("a switch");
        assert_eq!(devtools_commands(&driver).len(), before + 6);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_session_that_is_not_mobile_sends_no_emulation_command() {
        let driver = driver();
        let config = config_for(&driver).with_mode(DriverMode::Uc);
        let mut session = BrowserSession::connect(config).await.expect("connects");
        let sent = devtools_commands(&driver);
        assert!(
            names(&sent)
                .iter()
                .all(|name| !name.starts_with("Emulation.setDeviceMetrics")),
            "{sent:?}"
        );
        session.quit().await.expect("quits");
    }
}

#[cfg(test)]
mod headless_variant_tests {
    use super::*;
    use crate::browser::config::DriverMode;

    fn settled(config: BrowserConfig) -> BrowserConfig {
        let mut config = config;
        apply_headless_variants(&mut config);
        config
    }

    #[test]
    fn headless1_runs_chrome_as_chrome_headless_shell() {
        let mut config = BrowserConfig::default()
            .with_headless(false)
            .with_headless1(true);
        config.browser_binary_path = None;
        let settled = settled(config);
        assert_eq!(settled.browser, Browser::ChromeHeadlessShell);
        assert!(settled.headless);
    }

    #[test]
    fn headless1_leaves_a_chrome_with_its_own_program_and_other_browsers_alone() {
        let own = BrowserConfig::default()
            .with_headless1(true)
            .with_browser_binary_path("/opt/chrome/chrome");
        assert_eq!(settled(own).browser, Browser::Chrome);

        for browser in [Browser::Edge, Browser::Brave, Browser::Firefox] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_headless(false)
                .with_headless1(true);
            let config = settled(config);
            assert_eq!(config.browser, browser);
            assert!(config.headless, "{browser}");
        }
    }

    #[test]
    fn headless2_and_a_plain_session_change_nothing() {
        let config = BrowserConfig::default()
            .with_headless(false)
            .with_headless2(true);
        let after = settled(config.clone());
        assert_eq!(after, config);
        assert!(after.headless);
    }

    #[test]
    fn the_headless_shell_is_made_headless() {
        let shell = BrowserConfig::default()
            .with_browser(Browser::ChromeHeadlessShell)
            .with_headless(false);
        assert!(settled(shell).headless);
    }

    #[test]
    fn headless1_cannot_run_the_undetected_mode_just_as_the_shell_cannot() {
        let mut config = BrowserConfig::default()
            .with_mode(DriverMode::Uc)
            .with_headless1(true);
        config.browser_binary_path = None;
        let error = validate_mode_support(&config).expect_err("refused");
        assert!(
            error
                .to_string()
                .contains("chrome-headless-shell cannot run"),
            "{error}"
        );
        let with_program = config.with_browser_binary_path("/opt/chrome/chrome");
        assert!(validate_mode_support(&with_program).is_ok());
    }

    #[test]
    fn firefox_is_asked_for_a_private_window_and_developer_tools() {
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(false)
            .with_incognito(true)
            .with_devtools(true);
        let caps = serde_json::to_value(session_capabilities(&config).unwrap()).unwrap();
        assert_eq!(
            caps["moz:firefoxOptions"]["args"],
            json!(["-private", "-devtools"])
        );

        let headless = config.with_headless(true);
        let caps = serde_json::to_value(session_capabilities(&headless).unwrap()).unwrap();
        assert_eq!(
            caps["moz:firefoxOptions"]["args"],
            json!(["-headless", "-private"])
        );
    }
}
