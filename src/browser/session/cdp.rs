//! The DevTools Protocol side of a session: the CDP client, the commands sent
//! through it, the undetected mode, and the pure CDP page of an external
//! attachment.

// thirtyfour deprecates its `NetworkConditions`, which `set_network_conditions`
// still takes.
#![expect(
    deprecated,
    reason = "thirtyfour has deprecated NetworkConditions, which this API still takes"
)]

use serde_json::{json, Value};
use thirtyfour::extensions::cdp::NetworkConditions;

use crate::browser::config::BrowserConfig;
use crate::browser::options::CdpEvent;
use crate::error::SeleniumBaseError;
use crate::evasion::cdp::CdpClient;
use crate::evasion::uc;

use super::BrowserSession;

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
    /// connected.
    pub fn devtools(&self) -> Result<CdpClient, SeleniumBaseError> {
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session has no browser driver".to_owned())
        })?;
        Ok(CdpClient::from_handle(driver.handle().clone()))
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
