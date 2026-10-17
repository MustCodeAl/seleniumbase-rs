#![allow(deprecated)]

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
#[allow(deprecated)]
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::{By, DesiredCapabilities, WebDriver, WebElement};
use tracing::{info, instrument};

use crate::browser::binaries;
use crate::browser::chrome_for_testing::{
    install_chrome_for_testing, install_chrome_headless_shell,
};
use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::launcher::{launch_driver_for, DriverProcess};
use crate::browser::options::{
    firefox_preferences, CdpEvent, DriverVersion, LaunchSettings, LaunchSwitch, ReleaseChannel,
};
#[cfg(feature = "plugins")]
use crate::browser::plugin_hooks::{self, SessionPlugins};
use crate::browser::reuse::{self, Lease, ReuseKey, SessionAddress};
use crate::browser::virtual_display::{self, XvfbProcess};
use crate::config::ad_block_list::AdBlockList;
use crate::error::SeleniumBaseError;
use crate::stealth::cdp::*; /* CdpClient */
use crate::stealth::uc;
use crate::utilities::retry::{poll_until, RetryPolicy};

/// Owns a WebDriver session, optional CDP client, and optional auto-started driver process.
pub struct BrowserSession {
    driver: Option<WebDriver>,
    cdp: Option<CdpClient>,
    driver_process: Option<DriverProcess>,
    /// The virtual screen of a headed Linux session; dropped, and so stopped,
    /// after the driver.
    virtual_display: Option<XvfbProcess>,
    /// The DevTools commands that make a window refuse the ad hosts of the
    /// configuration, or none if it blocks nothing. They are sent to the first
    /// window and to each window the session switches to.
    ad_block: Vec<(&'static str, Value)>,
    /// The hold on a browser kept for reuse (`--reuse-session`). With one,
    /// quitting or dropping the session gives the browser back instead of
    /// closing it, and the reuse registry owns the driver process and the
    /// virtual display.
    reuse: Option<Lease>,
    /// The browser plugins of the session (feature `plugins`), and the scripts
    /// they added; none without plugins.
    #[cfg(feature = "plugins")]
    plugins: Option<SessionPlugins>,
}

impl BrowserSession {
    /// WebDriver interaction: `connect`.
    pub fn driver(&self) -> &WebDriver {
        self.driver.as_ref().unwrap()
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
    pub(crate) async fn connect_new(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        info!(browser = ?config.browser, headless = config.headless, "connecting browser session");
        validate_mode_support(&config)?;
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
        }
        let cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(driver.handle().clone()))
        } else {
            None
        };

        let session = Self {
            driver: Some(driver),
            cdp,
            driver_process,
            virtual_display,
            ad_block,
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        };
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
            driver_process: None,
            virtual_display: None,
            ad_block,
            reuse: Some(lease),
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

    /// Lets go of the driver without ending its session: the browser stays
    /// open for the next test.
    fn keep_browser_open(&mut self) {
        self.cdp = None;
        if let Some(driver) = self.driver.take() {
            let _ = driver.leak();
        }
    }

    /// Creates a disconnected session for test-only use. Any WebDriver call will
    /// panic, so this must only be used when the caller never touches the driver.
    pub fn disconnected() -> Self {
        Self {
            driver: None,
            cdp: None,
            driver_process: None,
            virtual_display: None,
            ad_block: Vec::new(),
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        }
    }

    /// Navigates to `url`.
    ///
    /// On Firefox and Safari, the scripts browser plugins added for every
    /// document run again in the new one (feature `plugins`).
    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.driver().goto(url).await?;
        #[cfg(feature = "plugins")]
        if let Some(plugins) = &self.plugins {
            plugins.after_navigation(self.driver()).await?;
        }
        Ok(())
    }

    /// Navigates back in browser history.
    pub async fn back(&self) -> Result<(), SeleniumBaseError> {
        self.driver().back().await?;
        Ok(())
    }

    /// Navigates forward in browser history.
    pub async fn forward(&self) -> Result<(), SeleniumBaseError> {
        self.driver().forward().await?;
        Ok(())
    }

    /// Reloads the current page.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.driver().refresh().await?;
        Ok(())
    }

    /// Returns the current page title.
    pub async fn current_title(&self) -> Result<String, SeleniumBaseError> {
        let title = self.driver().title().await?;
        Ok(title)
    }

    /// Returns the current page URL as a string.
    pub async fn current_url(&self) -> Result<String, SeleniumBaseError> {
        let url = self.driver().current_url().await?;
        Ok(url.as_str().to_owned())
    }

    /// Clicks the element located by `locator`.
    pub async fn click(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        element.click().await?;
        Ok(())
    }

    /// Clears and types `text` into the element located by `locator`.
    pub async fn type_text(&mut self, locator: By, text: &str) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        element.clear().await?;
        element.send_keys(text).await?;
        Ok(())
    }

    /// Clears the value of the element located by `locator`.
    pub async fn clear(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        element.clear().await?;
        Ok(())
    }

    /// Submits the form containing the element located by `locator`.
    pub async fn submit(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        let element_json = element.to_json()?;
        // Form submit via script
        self.driver()
            .execute("arguments[0].closest('form').submit()", vec![element_json])
            .await?;
        Ok(())
    }

    /// Returns the visible text of the element located by `locator`.
    pub async fn text(&self, locator: By) -> Result<String, SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        Ok(element.text().await?)
    }

    /// Hovers over the element located by `locator`.
    pub async fn hover(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        self.driver()
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
        let element = self.driver().find(locator).await?;
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
        let element = self.driver().find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_value(value).await?;
        Ok(())
    }

    /// Switches context into the frame located by `locator`.
    pub async fn switch_to_frame(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        element.enter_frame().await?;
        Ok(())
    }

    /// Switches context back to the top-level document.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.driver().enter_default_frame().await?;
        Ok(())
    }

    /// Drags the source element onto the target element.
    pub async fn drag_and_drop(
        &mut self,
        source_locator: By,
        target_locator: By,
    ) -> Result<(), SeleniumBaseError> {
        let source = self.driver().find(source_locator).await?;
        let target = self.driver().find(target_locator).await?;
        self.driver()
            .action_chain()
            .drag_and_drop_element(&source, &target)
            .perform()
            .await?;
        Ok(())
    }

    /// Returns the full HTML source of the current page.
    pub async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        Ok(self.driver().source().await?)
    }

    /// Returns the first element matching `locator`.
    pub async fn find(&mut self, locator: By) -> Result<WebElement, SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
        Ok(element)
    }

    /// Returns all elements matching `locator`.
    pub async fn find_all(&self, locator: By) -> Result<Vec<WebElement>, SeleniumBaseError> {
        Ok(self.driver().find_all(locator).await?)
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
        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async { self.driver().find(locator.clone()).await.ok() },
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
        let element = self.driver().find(locator).await?;
        Ok(element.attr(attribute_name).await?)
    }

    /// WebDriver interaction: `get_property`.
    pub async fn get_property(
        &mut self,
        locator: By,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.driver().find(locator).await?;
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
        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.driver().find(locator.clone()).await {
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
        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.driver().find(locator.clone()).await {
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
        self.driver()
            .execute_async(script, vec![])
            .await
            .map_err(SeleniumBaseError::WebDriver)
            .map(|ret| ret.json().clone())
    }

    /// WebDriver interaction: `maximize_window`.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.driver()
            .maximize_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `set_window_size`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.driver()
            .set_window_rect(0, 0, width, height)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns the current window size as `(width, height)`.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        let rect = self
            .driver()
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
        self.driver()
            .switch_to_window(h)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.apply_ad_block().await
    }

    /// Double-clicks the element located by `locator`.
    pub async fn double_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.driver()
            .action_chain()
            .double_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Right-clicks the element located by `locator`.
    pub async fn context_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.driver()
            .action_chain()
            .context_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is enabled.
    pub async fn is_enabled(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self
            .driver()
            .find(locator)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        element
            .is_enabled()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is selected.
    pub async fn is_selected(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self
            .driver()
            .find(locator)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        element
            .is_selected()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `is_displayed`.
    pub async fn is_displayed(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        if let Ok(element) = self.driver().find(locator).await {
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
        self.driver()
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `switch_to_alert_dismiss`.
    pub async fn switch_to_alert_dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.driver()
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `get_alert_text`.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.driver()
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `type_alert_text`.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.driver()
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `add_cookie`.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        let cookie = thirtyfour::cookie::Cookie::new(name, value);
        self.driver()
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
        self.driver()
            .get_named_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `delete_cookie`.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.driver()
            .delete_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `delete_all_cookies`.
    pub async fn delete_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.driver()
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
        self.driver()
            .find_all(locator)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Opens and switches to a new browser window/tab.
    ///
    /// A session that blocks ads makes the new window refuse the ad hosts too;
    /// see [`apply_ad_block`](Self::apply_ad_block). Browser plugins (feature
    /// `plugins`) give it their packages and `on_page_created`.
    pub async fn switch_to_new_window(&self) -> Result<(), SeleniumBaseError> {
        let handle = self
            .driver()
            .new_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.driver()
            .switch_to_window(handle)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.apply_ad_block().await?;
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
        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.driver().find_all(locator.clone()).await {
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
        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.driver().find(locator.clone()).await {
                    Ok(element) => match element.text().await {
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
        let ret = self.driver().execute(script, Vec::new()).await?;
        Ok(ret.json().clone())
    }

    /// Captures a screenshot and writes it to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        self.driver().screenshot(path).await?;
        Ok(())
    }

    /// Captures a screenshot and returns the raw PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        Ok(self.driver().screenshot_as_png().await?)
    }

    /// Enables the Page and Network CDP domains.
    ///
    /// The Runtime domain stays off on purpose: enabling it is a well-known
    /// bot-detection tell. See [`CdpClient::enable_default_domains`].
    ///
    /// [`CdpClient::enable_default_domains`]: crate::stealth::cdp::CdpClient::enable_default_domains
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains()
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Executes a CDP command without parameters.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.execute(method).await
    }

    /// Executes a CDP command with JSON parameters.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
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
        self.driver()
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
            .driver()
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
        let entries = self.driver().get_log("performance").await?;
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
            .driver()
            .execute(
                "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});",
                Vec::new(),
            )
            .await?;
        Ok(())
    }

    /// Applies an optional [`Fingerprint`] to the active session.
    ///
    /// The script is [`session_script`](crate::stealth::evasions::session_script),
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
            let _ = self.driver().execute(&script, Vec::new()).await?;
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
        for (method, params) in &self.ad_block {
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
    pub async fn reconnect(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        #[cfg(feature = "plugins")]
        if self.plugins.is_some() || !config.plugins().is_empty() {
            return self.reconnect_with_plugins(config).await;
        }
        self.reconnect_plain(config).await
    }

    /// [`reconnect`](Self::reconnect) without running plugin hooks.
    async fn reconnect_plain(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        // Worked out before anything is torn down, so a bad list leaves the
        // session as it was.
        let ad_block = ad_block_commands(config)?;
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
        self.initialize_mode(config).await?;
        Ok(())
    }

    /// Closes the browser session and any auto-started driver process.
    ///
    /// A session on a browser kept for reuse gives the browser back instead,
    /// open, for the next test.
    #[instrument(skip(self))]
    pub async fn quit(&mut self) -> Result<(), SeleniumBaseError> {
        if let Some(lease) = self.reuse.take() {
            info!("giving the browser back for reuse");
            self.keep_browser_open();
            drop(lease);
            return Ok(());
        }
        info!("quitting browser session");
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
            Some(plugins) => plugins.prepare_window(self.driver()).await,
            None => Ok(()),
        }
    }

    /// [`connect`](Self::connect) for a configuration with plugins.
    async fn connect_with_plugins(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        let config = plugin_hooks::before_launch(config).await?;
        let family = plugin_hooks::family_of(config.browser)?;
        let plugins = config.plugins().clone();
        let mut session = match Self::connect_new(config).await {
            Ok(session) => session,
            Err(error) => {
                // `before_launch` ran for a launch that ends here.
                plugin_hooks::close(&plugins).await;
                return Err(error);
            }
        };
        session
            .start_plugins(SessionPlugins::new(plugins, family))
            .await?;
        Ok(session)
    }

    /// Calls `on_browser` and prepares the first window, and keeps `plugins`
    /// for the rest of the session.
    ///
    /// # Errors
    ///
    /// The first hook that fails. The browser is closed then: one without
    /// what its plugins add is not handed out.
    async fn start_plugins(&mut self, plugins: SessionPlugins) -> Result<(), SeleniumBaseError> {
        let started = plugins.start(self.driver()).await;
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
        self.start_plugins(SessionPlugins::new(prepared.plugins().clone(), family))
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

fn validate_mode_support(config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    if config.is_cdp_enabled() && !config.browser.is_chromium_based() {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "CDP/UC mode requires a Chromium-based browser, not {}",
            config.browser
        )));
    }
    if config.is_uc_enabled() && config.browser == Browser::ChromeHeadlessShell {
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
fn fingerprint_script(fp: &crate::stealth::fingerprint::Fingerprint) -> String {
    crate::stealth::evasions::session_script(fp)
}

/// The protocol commands, after the init script, that make a WebDriver session
/// match `fp`.
///
/// The `Proxy-Authorization` header a fingerprint with a proxy password can ask
/// for is left out: set through `Network.setExtraHTTPHeaders` it would reach
/// every site the browser visits, and hand the proxy's login to all of them.
fn fingerprint_overrides(fp: &crate::stealth::fingerprint::Fingerprint) -> Vec<(String, Value)> {
    crate::stealth::evasions::safe_cdp_overrides(fp)
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
    // `Network.setBlockedURLs` replaces the first, so the two lists are joined.
    if config.is_cdp_enabled() {
        let fingerprint_urls = config
            .fingerprint
            .as_ref()
            .into_iter()
            .flat_map(crate::stealth::evasions::safe_cdp_overrides)
            .filter(|(method, _)| method == "Network.setBlockedURLs")
            .flat_map(|(_, params)| {
                params["urls"]
                    .as_array()
                    .map(|urls| {
                        urls.iter()
                            .filter_map(|u| u.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_else(Vec::<String>::new)
            });
        for url in fingerprint_urls {
            if !patterns.contains(&url) {
                patterns.push(url);
            }
        }
    }
    Ok(vec![
        ("Network.enable", json!({})),
        ("Network.setBlockedURLs", json!({ "urls": patterns })),
    ])
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

async fn connect_driver(
    config: &BrowserConfig,
    display: Option<&str>,
) -> Result<(WebDriver, Option<DriverProcess>), SeleniumBaseError> {
    let mut config = config.clone();
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
            | Browser::ChromeForTesting
            | Browser::ChromeHeadlessShell
    );
    if !needs_program || config.browser_binary_path.is_some() {
        return Ok(());
    }
    let found = match binaries::locate(config.browser) {
        Some(found) => found,
        // Chrome for Testing is a download, so a missing one is fetched.
        None if matches!(
            config.browser,
            Browser::ChromeForTesting | Browser::ChromeHeadlessShell
        ) =>
        {
            let stable = DriverVersion::Channel(ReleaseChannel::Stable);
            let proxy = config
                .launch
                .has(LaunchSwitch::ProxyForDriverDownload)
                .then_some(config.proxy.as_deref())
                .flatten();
            if config.browser == Browser::ChromeForTesting {
                install_chrome_for_testing(&stable, proxy).await?
            } else {
                install_chrome_headless_shell(&stable, proxy).await?
            }
        }
        None => {
            return Err(SeleniumBaseError::invalid_config(missing_browser_message(
                config.browser,
            )))
        }
    };
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
        .or_else(crate::stealth::patcher::find_system_chrome)
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
        let mut patcher = crate::stealth::patcher::ChromeBinaryPatcher::new(source);
        if let Some(cache) = runtime.patch_cache_dir {
            patcher = patcher.with_cache_dir(cache);
        }
        patcher.patch(crate::stealth::patcher::EnginePatch::chrome_binary())
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
    crate::stealth::options::StealthOptions::from(config).apply_to(caps)
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    use crate::stealth::evasions::{cdp_overrides, CREDENTIAL_HEADER_METHOD};
    use crate::stealth::fingerprint::{Fingerprint, OsType, ProxyConfig};

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
        assert!(cdp_overrides(&fingerprint).contains_key(CREDENTIAL_HEADER_METHOD));

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
            crate::stealth::providers::session_registry(&fingerprint).bootstrap(
                &crate::stealth::providers::EvasionContext::new(&fingerprint)
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
            options["prefs"],
            json!({
                "permissions.default.image": 2,
                "javascript.enabled": true,
                "privacy.donottrackheader.enabled": true,
                "intl.accept_languages": "fr",
            })
        );
    }

    #[test]
    fn a_plain_firefox_session_asks_for_no_preferences() {
        let caps = caps_json(&BrowserConfig::default().with_browser(Browser::Firefox));
        let options = &caps["moz:firefoxOptions"];
        assert!(options.get("prefs").is_none(), "{options}");
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
    fn the_block_keeps_what_the_fingerprint_blocks_where_the_fingerprint_is_applied() {
        let fingerprint = crate::stealth::fingerprint::Fingerprint::builder()
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
