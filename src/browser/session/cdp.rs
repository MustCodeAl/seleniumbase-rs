//! The DevTools Protocol side of a session: the CDP client, the commands sent
//! through it, the undetected mode, and the pure CDP page of an external
//! attachment.

use serde_json::{json, Value};

use crate::browser::config::BrowserConfig;
use crate::browser::devtools::CdpClient;
use crate::browser::options::{CdpEvent, NetworkThrottle};
use crate::browser::proxy_auth;
use crate::cookie::Cookie;
use crate::error::SeleniumBaseError;
use crate::evasion::uc;
use crate::sb_cdp::DevToolsEndpoint;

use super::BrowserSession;

/// How long [`BrowserSession::devtools_endpoint`] waits for a driver's browser
/// to answer its `/json/version` probe.
const DEVTOOLS_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// What the drivers of the browsers that have no DevTools channel report as their
/// `browserName`, compared as lower-case prefixes (Safari Technology Preview is a Safari).
const BROWSERS_WITHOUT_DEVTOOLS: [&str; 3] = ["firefox", "safari", "internet explorer"];

impl BrowserSession {
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
    /// connected. A command the browser refuses through the client is
    /// [`SeleniumBaseError::CdpDriver`].
    pub fn devtools(&self) -> Result<CdpClient, SeleniumBaseError> {
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session has no browser driver".to_owned())
        })?;
        Ok(CdpClient::new(driver))
    }

    /// The DevTools endpoint of the browser this session drives: the addresses
    /// another DevTools client attaches to (Python's `get_endpoint_url()`),
    /// such as a Playwright client (`PlaywrightSidecar::connect`).
    ///
    /// A direct CDP attachment answers the endpoint of its browser. A
    /// WebDriver Chromium session reads the `debuggerAddress` its driver
    /// reported and asks that address for the browser's WebSocket address
    /// (`/json/version`).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::SessionNotStarted`] after [`quit`](Self::quit).
    /// [`SeleniumBaseError::Unsupported`] when the driver reported no
    /// `debuggerAddress` (Firefox, Safari, a grid that hides it), as
    /// [`devtools`](Self::devtools) does for a session that is not connected.
    /// [`SeleniumBaseError::CdpDriver`] when that address does not answer or
    /// advertises no WebSocket.
    pub async fn devtools_endpoint(&self) -> Result<DevToolsEndpoint, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return Ok(page.browser().devtools_endpoint());
        }
        let capabilities =
            serde_json::to_value(self.webdriver()?.handle().capabilities()).unwrap_or(Value::Null);
        let address = proxy_auth::debugger_address(&capabilities).ok_or_else(|| {
            SeleniumBaseError::Unsupported(
                "the driver reported no DevTools address (debuggerAddress): only a Chromium \
                 session has an endpoint to share"
                    .to_owned(),
            )
        })?;
        let address = if address.contains("://") {
            address
        } else {
            format!("http://{address}")
        };
        let (endpoint, _version) =
            crate::sb_cdp::endpoint::resolve(&address, DEVTOOLS_PROBE_TIMEOUT).await?;
        Ok(endpoint)
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

    /// Enables the Page and Network CDP domains.
    ///
    /// The Runtime domain stays off on purpose: enabling it is a well-known
    /// bot-detection tell. See [`CdpClient::enable_default_domains`].
    ///
    /// [`CdpClient::enable_default_domains`]: crate::browser::devtools::CdpClient::enable_default_domains
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            for method in crate::evasion::cdp::DEFAULT_DOMAIN_COMMANDS {
                page.execute(method, json!({})).await?;
            }
            return Ok(());
        }
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains().await?;
        Ok(())
    }

    /// Executes a CDP command without parameters.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        if self.external_page.is_some() {
            return self.execute_cdp_with_params(method, json!({})).await;
        }
        let cdp = self.cdp_client()?;
        cdp.execute(method, json!({})).await
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
        cdp.execute(method, params).await
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
            .map_err(SeleniumBaseError::from)
    }

    /// Returns the cookies of the current page, the same [`Cookie`] in both modes.
    ///
    /// A session with a DevTools connection reads them with `Network.getCookies` for the
    /// current URL, as chromedriver does, so a frame of another origin adds none; a session
    /// without one (plain WebDriver mode) reads them through WebDriver. Either way these are
    /// the cookies the browser would send to the current URL, not every cookie it holds. A
    /// session cookie has no `expires`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if DevTools refuses the command,
    /// [`SeleniumBaseError::WebDriver`] if the driver refuses to list the cookies, and
    /// [`SeleniumBaseError::CdpDriver`] if DevTools answers without a list of cookies.
    pub async fn get_cookies(&self) -> Result<Vec<Cookie>, SeleniumBaseError> {
        if let Ok(cdp) = self.cdp_client() {
            let url = self.webdriver()?.current_url().await?;
            let response = cdp.get_cookies(url.as_str()).await?;
            let cookies = response["cookies"].as_array().ok_or_else(|| {
                SeleniumBaseError::CdpDriver(
                    "Network.getCookies answered no list of cookies".to_owned(),
                )
            })?;
            return Ok(cookies.iter().map(Cookie::from_protocol).collect());
        }
        let cookies = self.webdriver()?.get_all_cookies().await?;
        Ok(cookies.iter().map(Cookie::from_webdriver).collect())
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

    /// Slows the network of the window the session is on, cuts it off, or puts it back, as
    /// `throttle` says. The default [`NetworkThrottle`] changes nothing, so it ends a throttle.
    ///
    /// It works in every [`DriverMode`](crate::browser::config::DriverMode), the plain WebDriver
    /// mode included, on a Chromium browser: `Network.enable` and then
    /// `Network.emulateNetworkConditions` go through the driver's `/goog/cdp/execute`
    /// passthrough ([`devtools`](Self::devtools)), so no mode has to be switched on. A session
    /// attached over CDP sends them to its own tab. The throttle belongs to the window it was
    /// set on, as the rest of the DevTools emulation does: the session does not repeat it for a
    /// window it switches to.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Unsupported`], naming the browser, for Firefox, Safari and Internet
    /// Explorer, whose drivers have no DevTools channel.
    /// [`SeleniumBaseError::CdpDriver`] if the browser refuses a command.
    /// [`SeleniumBaseError::SessionNotStarted`] after [`quit`](Self::quit).
    pub async fn set_network_throttle(
        &self,
        throttle: &NetworkThrottle,
    ) -> Result<(), SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page.emulation().network_throttle(throttle).await;
        }
        self.require_devtools_channel()?;
        let cdp = self.devtools()?;
        for (method, params) in throttle.commands() {
            cdp.execute(method, params).await?;
        }
        Ok(())
    }

    /// Refuses a session whose driver has no DevTools channel (Firefox, Safari, Internet
    /// Explorer), naming the browser the driver reported.
    fn require_devtools_channel(&self) -> Result<(), SeleniumBaseError> {
        let capabilities =
            serde_json::to_value(self.webdriver()?.handle().capabilities()).unwrap_or(Value::Null);
        let name = capabilities["browserName"].as_str().unwrap_or_default();
        let lowered = name.to_ascii_lowercase();
        if BROWSERS_WITHOUT_DEVTOOLS
            .iter()
            .any(|prefix| lowered.starts_with(prefix))
        {
            return Err(SeleniumBaseError::Unsupported(format!(
                "{name} has no DevTools channel through WebDriver, so the network of its \
                     window cannot be throttled; use a Chromium browser"
            )));
        }
        Ok(())
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
        cdp.enable_default_domains().await?;
        uc::apply_uc_stealth(cdp).await?;
        if let Some(user_agent) = config.browser.user_agent.as_deref() {
            uc::override_user_agent(cdp, user_agent, config.browser.lang.as_deref()).await?;
        } else {
            if let Ok(version_info) = cdp.execute("Browser.getVersion", json!({})).await
                && let Some(ua) = version_info["userAgent"].as_str()
                && ua.contains("HeadlessChrome")
            {
                let stealth_ua = ua.replace("HeadlessChrome", "Chrome");
                let _ =
                    uc::override_user_agent(cdp, &stealth_ua, config.browser.lang.as_deref()).await;
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

    /// Sets up the modes `config` asks for on a session that has just
    /// connected: the CDP domains, the undetected-mode patches or the
    /// fingerprint, and the ad block and phone emulation of its first window.
    pub(super) async fn initialize_mode(
        &self,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
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

    /// The session's DevTools client, kept for the CDP and UC modes.
    pub(super) fn cdp_client(&self) -> Result<&CdpClient, SeleniumBaseError> {
        self.cdp.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported(
                "CDP is unavailable for this session. Enable mode cdp or uc.".to_owned(),
            )
        })
    }
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;
    use crate::sb_cdp::Browser;

    #[tokio::test]
    async fn a_direct_attachment_answers_the_endpoint_of_its_browser() {
        let (browser, _mock) = Browser::new_mocked();
        let page = browser.default_page().await.expect("a page on the mock");
        let mut session = BrowserSession::disconnected();
        session.external_page = Some(page);

        let endpoint = session
            .devtools_endpoint()
            .await
            .expect("an attachment has its browser's endpoint");

        // What `Browser::http_url` says of the mock (`tests/offline/sb_cdp_parity_mock.rs`).
        assert_eq!(endpoint.http_url(), "http://127.0.0.1:0");
        assert_eq!(endpoint, browser.devtools_endpoint());
    }

    #[tokio::test]
    async fn a_session_that_is_not_connected_has_no_endpoint() {
        let session = BrowserSession::disconnected();

        let error = session.devtools_endpoint().await.expect_err("no session");

        assert!(
            matches!(error, SeleniumBaseError::SessionNotStarted),
            "{error:?}"
        );
    }
}

/// `set_network_throttle` against the mock WebDriver in `webdriver_support` (no
/// browser): which DevTools commands a session sends, in which modes, and what
/// it answers when it cannot send them.
#[cfg(test)]
mod network_throttle_tests {
    use std::time::Duration;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::DriverMode;
    use crate::browser::options::NetworkThrottle;
    use crate::sb_cdp::Browser;

    fn slow() -> NetworkThrottle {
        NetworkThrottle {
            offline: false,
            latency: Duration::from_millis(400),
            download: Some(200_000),
            upload: Some(94_000),
        }
    }

    fn slow_params() -> Value {
        json!({
            "offline": false,
            "latency": 400,
            "downloadThroughput": 200_000,
            "uploadThroughput": 94_000,
        })
    }

    /// A driver of the browser called `browser_name` that answers every command.
    fn driver_of(browser_name: &str) -> MockWebDriver {
        MockWebDriver::start(json!({ "browserName": browser_name }), |_| {
            Reply::Value(json!({}))
        })
    }

    /// A session on `driver` that starts no driver of its own.
    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.session.auto_start_driver = false;
        config
    }

    /// The bodies of the commands the driver's DevTools channel was sent.
    fn devtools_commands(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| request.body)
            .collect()
    }

    #[tokio::test]
    async fn plain_webdriver_mode_throttles_through_the_drivers_devtools_channel() {
        let driver = driver_of("chrome");
        let config = config_for(&driver).with_mode(DriverMode::WebDriver);
        let mut session = BrowserSession::connect(config).await.expect("connects");

        session
            .set_network_throttle(&slow())
            .await
            .expect("the commands are sent without CDP mode");

        assert_eq!(
            devtools_commands(&driver),
            [
                json!({ "cmd": "Network.enable", "params": {} }),
                json!({ "cmd": "Network.emulateNetworkConditions", "params": slow_params() }),
            ]
        );
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn the_cdp_mode_sends_the_same_command() {
        let driver = driver_of("chrome");
        let config = config_for(&driver).with_mode(DriverMode::Cdp);
        let mut session = BrowserSession::connect(config).await.expect("connects");

        session
            .set_network_throttle(&slow())
            .await
            .expect("the commands are sent");

        let emulated: Vec<_> = devtools_commands(&driver)
            .into_iter()
            .filter(|command| command["cmd"] == "Network.emulateNetworkConditions")
            .collect();
        assert_eq!(
            emulated,
            [json!({ "cmd": "Network.emulateNetworkConditions", "params": slow_params() })]
        );
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn every_chromium_driver_is_sent_the_commands() {
        for browser_name in ["chrome", "msedge", "chromium"] {
            let driver = driver_of(browser_name);
            let mut session = BrowserSession::connect(config_for(&driver))
                .await
                .expect("connects");

            session
                .set_network_throttle(&NetworkThrottle::default())
                .await
                .unwrap_or_else(|error| panic!("{browser_name}: {error:?}"));

            assert_eq!(devtools_commands(&driver).len(), 2, "{browser_name}");
            session.quit().await.expect("quits");
        }
    }

    #[tokio::test]
    async fn a_command_the_browser_refuses_is_a_cdp_driver_error() {
        let driver = MockWebDriver::start(json!({ "browserName": "chrome" }), |request| {
            if request.path == "/goog/cdp/execute"
                && request.body["cmd"] == "Network.emulateNetworkConditions"
            {
                Reply::Error(500, "unknown error")
            } else {
                Reply::Value(json!({}))
            }
        });
        let mut session = BrowserSession::connect(config_for(&driver))
            .await
            .expect("connects");

        let error = session
            .set_network_throttle(&slow())
            .await
            .expect_err("the browser refuses");

        assert!(
            matches!(error, SeleniumBaseError::CdpDriver(_)),
            "{error:?}"
        );
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_browser_without_a_devtools_channel_is_refused_by_name_and_sent_nothing() {
        for browser_name in ["firefox", "Safari", "internet explorer"] {
            let driver = driver_of(browser_name);
            let mut session = BrowserSession::connect(config_for(&driver))
                .await
                .expect("connects");

            let error = session
                .set_network_throttle(&slow())
                .await
                .expect_err("no DevTools channel");

            assert!(
                matches!(&error, SeleniumBaseError::Unsupported(reason) if reason.contains(browser_name)),
                "{browser_name}: {error:?}"
            );
            assert!(devtools_commands(&driver).is_empty(), "{browser_name}");
            session.quit().await.expect("quits");
        }
    }

    #[tokio::test]
    async fn a_direct_attachment_throttles_its_own_tab() {
        let (browser, mock) = Browser::new_mocked();
        let page = browser.default_page().await.expect("a page on the mock");
        let mut session = BrowserSession::disconnected();
        session.external_page = Some(page);

        session
            .set_network_throttle(&slow())
            .await
            .expect("the tab takes it");

        let sent: Vec<_> = mock
            .calls()
            .into_iter()
            .filter(|call| call.method.starts_with("Network."))
            .map(|call| (call.method, call.params))
            .collect();
        assert_eq!(
            sent,
            [
                ("Network.enable".to_owned(), json!({})),
                ("Network.emulateNetworkConditions".to_owned(), slow_params()),
            ]
        );
    }

    #[tokio::test]
    async fn a_session_that_is_not_connected_is_not_started() {
        let session = BrowserSession::disconnected();

        let error = session
            .set_network_throttle(&slow())
            .await
            .expect_err("no session");

        assert!(
            matches!(error, SeleniumBaseError::SessionNotStarted),
            "{error:?}"
        );
    }
}
