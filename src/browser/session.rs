#![allow(deprecated)]

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
#[allow(deprecated)]
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::{By, DesiredCapabilities, WebDriver, WebElement};

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::launcher::{launch_chromedriver, DriverProcess};
use crate::error::SeleniumBaseError;
use crate::stealth::cdp::*; /* CdpClient */
use crate::stealth::uc;

/// Owns a WebDriver session, optional CDP client, and optional auto-started driver process.
pub struct BrowserSession {
    driver: Option<WebDriver>,
    cdp: Option<CdpClient>,
    driver_process: Option<DriverProcess>,
}

impl BrowserSession {
    /// WebDriver interaction: `connect`.
    pub fn driver(&self) -> &WebDriver {
        self.driver.as_ref().unwrap()
    }

    pub async fn connect(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        validate_mode_support(&config)?;
        let (driver, driver_process) = connect_driver(&config).await?;
        let cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(driver.handle().clone()))
        } else {
            None
        };

        let session = Self {
            driver: Some(driver),
            cdp,
            driver_process,
        };
        session.initialize_mode(&config).await?;
        Ok(session)
    }

    /// Creates a disconnected session for test-only use. Any WebDriver call will
    /// panic, so this must only be used when the caller never touches the driver.
    pub fn disconnected() -> Self {
        Self {
            driver: None,
            cdp: None,
            driver_process: None,
        }
    }

    /// Navigates to `url`.
    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.driver().goto(url).await?;
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
    pub async fn current_title(&mut self) -> Result<String, SeleniumBaseError> {
        let title = self.driver().title().await?;
        Ok(title)
    }

    /// Returns the current page URL as a string.
    pub async fn current_url(&mut self) -> Result<String, SeleniumBaseError> {
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
    pub async fn text(&mut self, locator: By) -> Result<String, SeleniumBaseError> {
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

    /// WebDriver interaction: `wait_for_element`.
    pub async fn wait_for_element(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver().find(locator.clone()).await {
                return Ok(element);
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(
                    "timed out waiting for element".to_owned(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
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
    pub async fn wait_for_element_visible(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver().find(locator.clone()).await {
                if element.is_displayed().await.unwrap_or(false) {
                    return Ok(element);
                }
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(
                    "timed out waiting for element to be visible".to_owned(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout` seconds for `locator` to become clickable.
    pub async fn wait_for_element_clickable(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver().find(locator.clone()).await {
                if element.is_displayed().await.unwrap_or(false)
                    && element.is_enabled().await.unwrap_or(false)
                {
                    return Ok(element);
                }
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(
                    "timed out waiting for element to be clickable".to_owned(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
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
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        let h = thirtyfour::common::types::WindowHandle::from(handle.to_string());
        self.driver()
            .switch_to_window(h)
            .await
            .map_err(SeleniumBaseError::WebDriver)
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
        Ok(())
    }

    /// Waits up to `timeout` seconds for `locator` to be removed.
    pub async fn wait_for_element_absent(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            match self.driver().find_all(locator.clone()).await {
                Ok(elements) if elements.is_empty() => return Ok(()),
                Err(_) => return Ok(()),
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(
                    "timed out waiting for element to be absent".to_owned(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// WebDriver interaction: `wait_for_text`.
    pub async fn wait_for_text(
        &self,
        locator: By,
        expected_substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver().find(locator.clone()).await {
                let text = element.text().await?;
                if text.contains(expected_substring) {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "timed out waiting for text '{expected_substring}'"
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
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

    /// Enables Page/Network/Runtime CDP domains.
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
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.clear_cookies().await
    }

    /// Returns all cookies via CDP as JSON.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.get_cookies().await
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

    /// Disconnects the current WebDriver session and creates a fresh one.
    ///
    /// This is useful in undetected (UC) mode to obtain a clean session handle
    /// while keeping the same chromedriver process alive.
    pub async fn reconnect(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let (driver, process) = connect_driver(config).await?;
        self.driver = Some(driver);
        if process.is_some() {
            self.driver_process = process;
        }
        self.cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(
                self.driver.as_ref().unwrap().handle().clone(),
            ))
        } else {
            None
        };
        self.initialize_mode(config).await?;
        Ok(())
    }

    /// Closes the browser session and any auto-started driver process.
    pub async fn quit(self) -> Result<(), SeleniumBaseError> {
        if let Some(driver) = self.driver {
            driver.quit().await?;
        }
        if let Some(mut process) = self.driver_process {
            process.kill();
        }
        Ok(())
    }

    async fn initialize_mode(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if config.is_cdp_enabled() {
            self.activate_cdp_mode().await?;
        }
        if config.is_uc_enabled() {
            self.enable_uc_mode(config).await?;
        }
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

fn validate_mode_support(config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    if config.is_cdp_enabled()
        && !matches!(
            config.browser,
            Browser::Chrome | Browser::Chromium | Browser::Edge
        )
    {
        return Err(SeleniumBaseError::InvalidConfig(
            "CDP/UC mode requires a Chromium-based browser (chrome/chromium/edge)".to_owned(),
        ));
    }
    Ok(())
}

async fn try_connect(config: &BrowserConfig, url: &str) -> Result<WebDriver, SeleniumBaseError> {
    match config.browser {
        Browser::Chrome | Browser::Chromium => {
            let mut caps = DesiredCapabilities::chrome();
            apply_chromium_capabilities(&mut caps, config)?;
            Ok(WebDriver::new(url, caps).await?)
        }
        Browser::Edge => {
            let mut caps = DesiredCapabilities::edge();
            apply_chromium_capabilities(&mut caps, config)?;
            Ok(WebDriver::new(url, caps).await?)
        }
        Browser::Firefox => {
            let mut caps = DesiredCapabilities::firefox();
            if config.headless {
                caps.add_arg("-headless")?;
            }
            Ok(WebDriver::new(url, caps).await?)
        }
    }
}

async fn connect_driver(
    config: &BrowserConfig,
) -> Result<(WebDriver, Option<DriverProcess>), SeleniumBaseError> {
    let mut process: Option<DriverProcess> = None;
    let mut url = config.webdriver_url.clone();

    if config.auto_start_driver && config.is_default_webdriver_url() {
        match try_connect(config, &url).await {
            Ok(driver) => return Ok((driver, None)),
            Err(_) => {
                let launched = launch_chromedriver().await?;
                url = launched.url.clone();
                process = Some(launched);
            }
        }
    }

    let driver = try_connect(config, &url).await?;
    Ok((driver, process))
}

fn apply_chromium_capabilities<C: ChromiumLikeCapabilities>(
    caps: &mut C,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    crate::stealth::options::StealthOptions::from(config).apply_to(caps)
}
