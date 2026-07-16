use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::{By, DesiredCapabilities, WebDriver, WebElement};

use crate::cdp::CdpClient;
use crate::config::{Browser, BrowserConfig};
use crate::error::SeleniumBaseError;
use crate::uc;

pub struct BrowserSession {
    driver: WebDriver,
    cdp: Option<CdpClient>,
}

impl BrowserSession {
    /// WebDriver interaction: `connect`.
    pub fn driver(&self) -> &WebDriver {
        &self.driver
    }

    pub async fn connect(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        validate_mode_support(&config)?;
        let driver = connect_driver(&config).await?;
        let cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(driver.handle.clone()))
        } else {
            None
        };

        let session = Self { driver, cdp };
        session.initialize_mode(&config).await?;
        Ok(session)
    }

    /// WebDriver interaction: `goto`.
    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.driver.goto(url).await?;
        Ok(())
    }

    /// WebDriver interaction: `back`.
    pub async fn back(&self) -> Result<(), SeleniumBaseError> {
        self.driver.back().await?;
        Ok(())
    }

    /// WebDriver interaction: `forward`.
    pub async fn forward(&self) -> Result<(), SeleniumBaseError> {
        self.driver.forward().await?;
        Ok(())
    }

    /// WebDriver interaction: `refresh`.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.driver.refresh().await?;
        Ok(())
    }

    /// WebDriver interaction: `current_title`.
    pub async fn current_title(&mut self) -> Result<String, SeleniumBaseError> {
        let title = self.driver.title().await?;
        Ok(title)
    }

    /// WebDriver interaction: `current_url`.
    pub async fn current_url(&mut self) -> Result<String, SeleniumBaseError> {
        let url = self.driver.current_url().await?;
        Ok(url.as_str().to_owned())
    }

    /// WebDriver interaction: `click`.
    pub async fn click(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        element.click().await?;
        Ok(())
    }

    /// WebDriver interaction: `type_text`.
    pub async fn type_text(&mut self, locator: By, text: &str) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        element.clear().await?;
        element.send_keys(text).await?;
        Ok(())
    }

    /// WebDriver interaction: `clear`.
    pub async fn clear(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        element.clear().await?;
        Ok(())
    }

    /// WebDriver interaction: `submit`.
    pub async fn submit(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        let element_json = element.to_json()?;
        // Form submit via script
        self.driver
            .execute("arguments[0].closest('form').submit()", vec![element_json])
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `text`.
    pub async fn text(&mut self, locator: By) -> Result<String, SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        Ok(element.text().await?)
    }

    /// WebDriver interaction: `hover`.
    pub async fn hover(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        self.driver
            .action_chain()
            .move_to_element_center(&element)
            .perform()
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `select_option_by_text`.
    pub async fn select_option_by_text(
        &mut self,
        locator: By,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_visible_text(text).await?;
        Ok(())
    }

    /// WebDriver interaction: `select_option_by_value`.
    pub async fn select_option_by_value(
        &mut self,
        locator: By,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_value(value).await?;
        Ok(())
    }

    /// WebDriver interaction: `switch_to_frame`.
    pub async fn switch_to_frame(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        element.enter_frame().await?;
        Ok(())
    }

    /// WebDriver interaction: `switch_to_default_content`.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.driver.enter_default_frame().await?;
        Ok(())
    }

    /// WebDriver interaction: `drag_and_drop`.
    pub async fn drag_and_drop(
        &mut self,
        source_locator: By,
        target_locator: By,
    ) -> Result<(), SeleniumBaseError> {
        let source = self.driver.find(source_locator).await?;
        let target = self.driver.find(target_locator).await?;
        self.driver
            .action_chain()
            .drag_and_drop_element(&source, &target)
            .perform()
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `page_source`.
    pub async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        Ok(self.driver.source().await?)
    }

    /// WebDriver interaction: `find`.
    pub async fn find(&mut self, locator: By) -> Result<WebElement, SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        Ok(element)
    }

    /// WebDriver interaction: `find_all`.
    pub async fn find_all(&self, locator: By) -> Result<Vec<WebElement>, SeleniumBaseError> {
        Ok(self.driver.find_all(locator).await?)
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
            if let Ok(element) = self.driver.find(locator.clone()).await {
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
        let element = self.driver.find(locator).await?;
        Ok(element.attr(attribute_name).await?)
    }

    /// WebDriver interaction: `get_property`.
    pub async fn get_property(
        &mut self,
        locator: By,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.driver.find(locator).await?;
        Ok(element.prop(property_name).await?)
    }

    /// WebDriver interaction: `wait_for_element_visible`.
    pub async fn wait_for_element_visible(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver.find(locator.clone()).await {
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

    /// WebDriver interaction: `wait_for_element_clickable`.
    pub async fn wait_for_element_clickable(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(element) = self.driver.find(locator.clone()).await {
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
        self.driver
            .execute_async(script, vec![])
            .await
            .map_err(SeleniumBaseError::WebDriver)
            .map(|ret| ret.json().clone())
    }

    /// WebDriver interaction: `maximize_window`.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .maximize_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `set_window_size`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.driver
            .set_window_rect(0, 0, width, height)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `get_window_size`.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        let rect = self
            .driver
            .get_window_rect()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok((rect.width as u32, rect.height as u32))
    }

    /// WebDriver interaction: `switch_to_window`.
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        let h = thirtyfour::common::types::WindowHandle::from(handle.to_string());
        self.driver
            .switch_to_window(h)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `double_click`.
    pub async fn double_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.driver
            .action_chain()
            .double_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `context_click`.
    pub async fn context_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.driver
            .action_chain()
            .context_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `is_enabled`.
    pub async fn is_enabled(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self
            .driver
            .find(locator)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        element
            .is_enabled()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `is_selected`.
    pub async fn is_selected(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self
            .driver
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
        if let Ok(element) = self.driver.find(locator).await {
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
        self.driver
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `switch_to_alert_dismiss`.
    pub async fn switch_to_alert_dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `get_alert_text`.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.driver
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `type_alert_text`.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.driver
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `add_cookie`.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        let cookie = thirtyfour::cookie::Cookie::new(name, value);
        self.driver
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
        self.driver
            .get_named_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `delete_cookie`.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.driver
            .delete_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `delete_all_cookies`.
    pub async fn delete_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.driver
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
        self.driver
            .find_all(locator)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `switch_to_new_window`.
    pub async fn switch_to_new_window(&self) -> Result<(), SeleniumBaseError> {
        let handle = self
            .driver
            .new_window()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.driver
            .switch_to_window(handle)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `wait_for_element_absent`.
    pub async fn wait_for_element_absent(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            match self.driver.find_all(locator.clone()).await {
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
            if let Ok(element) = self.driver.find(locator.clone()).await {
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

    /// WebDriver interaction: `execute_script`.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        let ret = self.driver.execute(script, Vec::new()).await?;
        Ok(ret.json().clone())
    }

    /// WebDriver interaction: `screenshot`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        self.driver.screenshot(path).await?;
        Ok(())
    }

    /// WebDriver interaction: `screenshot_as_png`.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        Ok(self.driver.screenshot_as_png().await?)
    }

    /// WebDriver interaction: `activate_cdp_mode`.
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains().await?;
        Ok(())
    }

    /// WebDriver interaction: `execute_cdp`.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.execute(method).await
    }

    /// WebDriver interaction: `execute_cdp_with_params`.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.execute_with_params(method, params).await
    }

    /// WebDriver interaction: `clear_browser_cache`.
    pub async fn clear_browser_cache(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.clear_cache().await
    }

    /// WebDriver interaction: `clear_browser_cookies`.
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.clear_cookies().await
    }

    /// WebDriver interaction: `get_cookies`.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.get_cookies().await
    }

    /// WebDriver interaction: `cdp_mouse_click`.
    pub async fn cdp_mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.mouse_click(x, y).await
    }

    /// WebDriver interaction: `cdp_type_text`.
    pub async fn cdp_type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.keyboard_insert_text(text).await
    }

    /// WebDriver interaction: `set_network_conditions`.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.set_network_conditions(conditions).await
    }

    /// WebDriver interaction: `enable_uc_mode`.
    pub async fn enable_uc_mode(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        let cdp = self.cdp_client()?;
        cdp.enable_default_domains().await?;
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
            .driver
            .execute(
                "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});",
                Vec::new(),
            )
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `quit`.
    pub async fn quit(self) -> Result<(), SeleniumBaseError> {
        self.driver.quit().await?;
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

async fn connect_driver(config: &BrowserConfig) -> Result<WebDriver, SeleniumBaseError> {
    match config.browser {
        Browser::Chrome | Browser::Chromium => {
            let mut caps = DesiredCapabilities::chrome();
            apply_chromium_capabilities(&mut caps, config)?;
            Ok(WebDriver::new(&config.webdriver_url, caps).await?)
        }
        Browser::Edge => {
            let mut caps = DesiredCapabilities::edge();
            apply_chromium_capabilities(&mut caps, config)?;
            Ok(WebDriver::new(&config.webdriver_url, caps).await?)
        }
        Browser::Firefox => {
            let mut caps = DesiredCapabilities::firefox();
            if config.headless {
                caps.add_arg("-headless")?;
            }
            Ok(WebDriver::new(&config.webdriver_url, caps).await?)
        }
    }
}

fn apply_chromium_capabilities<C: ChromiumLikeCapabilities>(
    caps: &mut C,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    caps.add_arg("--disable-gpu")?;
    caps.add_arg("--window-size=1280,720")?;
    if config.headless {
        caps.add_arg("--headless=new")?;
    }
    if config.ad_block {
        caps.add_arg("--blink-settings=imagesEnabled=false")?;
    }
    if let Some(locale) = config.locale.as_deref() {
        caps.add_arg(&format!("--lang={locale}"))?;
    }
    if let Some(user_agent) = config.user_agent.as_deref() {
        caps.add_arg(&format!("--user-agent={user_agent}"))?;
    }
    if let Some(proxy) = config.proxy.as_deref() {
        caps.add_arg(&format!("--proxy-server={proxy}"))?;
    }
    if config.is_uc_enabled() {
        caps.add_arg("--disable-blink-features=AutomationControlled")?;
        caps.add_arg("--disable-infobars")?;
        caps.add_arg("--disable-popup-blocking")?;
        caps.add_arg("--no-first-run")?;
        caps.add_arg("--disable-notifications")?;
        caps.add_arg("--disable-background-networking")?;
        caps.add_arg("--disable-client-side-phishing-detection")?;
        caps.add_arg("--disable-default-apps")?;
        caps.add_arg("--disable-prompt-on-repost")?;
        caps.add_arg("--disable-sync")?;
        caps.add_arg("--disable-translate")?;
        caps.add_arg("--metrics-recording-only")?;
        caps.add_arg("--no-default-browser-check")?;
        caps.add_arg("--password-store=basic")?;
        caps.add_arg("--use-mock-keychain")?;
        caps.add_arg("--disable-search-engine-choice-screen")?;
        caps.add_arg("--safebrowsing-disable-download-protection")?;
        caps.add_exclude_switch("enable-automation")?;
        caps.add_experimental_option("useAutomationExtension", false)?;
    }
    Ok(())
}
