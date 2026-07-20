#![allow(deprecated)]

use base64::prelude::*;
use rand::RngExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::chart::{Chart, ChartSeries, ChartType};
use crate::api::deferred::DeferredAsserts;
use crate::api::html::BeautifulSoup;
use crate::api::master_qa::{MasterQA, MasterQaSession};
use crate::api::pdf;
use crate::api::presentation::Presentation;
use crate::api::recorder::{ActionRecorder, RecordedAction};
use crate::api::tour::{Tour, TourTheme};
use crate::artifacts::{artifact_path, ensure_latest_logs_dir};
use crate::browser::config::BrowserConfig;
#[cfg(feature = "playwright")]
use crate::browser::playwright::PlaywrightSession;
use crate::browser::session::BrowserSession;
use crate::error::SeleniumBaseError;
use crate::utils::selectors::Selector;
use serde_json::Value;
use std::collections::HashMap;
use thirtyfour::common::keys::Key;
#[allow(deprecated)]
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::By;

pub struct BaseCase {
    session: BrowserSession,
    config: BrowserConfig,
    recorder: Arc<Mutex<ActionRecorder>>,
    tour: Option<Tour>,
    deferred: DeferredAsserts,
    presentation: Option<Presentation>,
    chart: Option<Chart>,
    qa_session: Option<MasterQaSession>,
    #[cfg(feature = "playwright")]
    playwright_session: Option<PlaywrightSession>,
    time_limit_secs: Option<u64>,
    gui_held: Option<(i32, i32)>,
}

impl BaseCase {
    /// Creates a new test case and connects to the browser described by `config`.
    pub async fn new(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        let session = BrowserSession::connect(config.clone()).await?;
        Ok(Self::with_session(config, session))
    }

    /// Creates a `BaseCase` from an existing browser session. Used internally for
    /// reconnecting and for test-only construction.
    pub fn with_session(config: BrowserConfig, session: BrowserSession) -> Self {
        Self {
            session,
            config,
            recorder: Arc::new(Mutex::new(ActionRecorder::default())),
            tour: None,
            deferred: DeferredAsserts::default(),
            presentation: None,
            chart: None,
            qa_session: None,
            #[cfg(feature = "playwright")]
            playwright_session: None,
            time_limit_secs: None,
            gui_held: None,
        }
    }

    /// Creates a `BaseCase` without a live browser session. Useful for unit tests
    /// of helper methods that do not interact with the browser.
    pub fn without_session(config: BrowserConfig) -> Self {
        Self::with_session(config, BrowserSession::disconnected())
    }

    /// Activates the optional Playwright-backed stealth browser mode.
    ///
    /// This creates a fresh Chromium session through the [`playwright`] crate
    /// and stores it in the test case. When this mode is active you can use
    /// [`playwright_session`](BaseCase::playwright_session) or the helpers in
    /// [`crate::api::playwright`] to interact with pages.
    ///
    /// This method is only available when the `playwright` feature is enabled.
    #[cfg(feature = "playwright")]
    pub async fn activate_playwright_mode(&mut self) -> Result<(), SeleniumBaseError> {
        self.playwright_session = Some(PlaywrightSession::launch().await?);
        Ok(())
    }

    /// Returns a mutable reference to the active Playwright session, if any.
    #[cfg(feature = "playwright")]
    pub fn playwright_session(&mut self) -> Option<&mut PlaywrightSession> {
        self.playwright_session.as_mut()
    }

    /// Asserts that `text` is visible inside the element selected by `css`.
    pub async fn assert_text_visible(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_text_visible(text, css).await?;
        if !is_visible {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Text '{}' was not visible in element '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that `text` is not visible inside the element selected by `css`.
    pub async fn assert_text_not_visible(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_text_visible(text, css).await?;
        if is_visible {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Text '{}' was unexpectedly visible in element '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that the attribute named `attribute` on `css` equals `value`.
    pub async fn assert_attribute(
        &mut self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let actual_value = self.get_attribute(css, attribute).await?;
        if actual_value.as_deref() != Some(value) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected attribute '{}' of '{}' to be '{}', but got '{}'",
                attribute,
                css,
                value,
                actual_value.unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// Asserts that the current page title equals `expected`.
    pub async fn assert_title(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let title = self.get_title().await?;
        if title != expected {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected title to be '{}', but got '{}'",
                expected, title
            )));
        }
        Ok(())
    }

    /// Waits until `document.readyState` is `complete`.
    pub async fn wait_for_ready_state_complete(&self) -> Result<(), SeleniumBaseError> {
        let script = "return document.readyState;";
        let start = std::time::Instant::now();
        let timeout_secs = self.time_limit_secs.unwrap_or(10);
        loop {
            if start.elapsed().as_secs() > timeout_secs {
                return Err(SeleniumBaseError::WaitTimeout(
                    "Page readyState did not become 'complete'".into(),
                ));
            }
            if let Ok(Value::String(state)) = self.execute_script(script).await {
                if state == "complete" {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Returns the current browser window position as `(x, y)`.
    pub async fn get_window_position(&self) -> Result<(i64, i64), SeleniumBaseError> {
        let rect = self.session.driver().get_window_rect().await?;
        Ok((rect.x, rect.y))
    }

    /// Moves the browser window to `(x, y)`.
    pub async fn set_window_position(&self, x: u32, y: u32) -> Result<(), SeleniumBaseError> {
        let rect = self.session.driver().get_window_rect().await?;
        self.session
            .driver()
            .set_window_rect(
                x.into(),
                y.into(),
                rect.width.try_into().unwrap_or(0),
                rect.height.try_into().unwrap_or(0),
            )
            .await?;
        Ok(())
    }

    /// Closes the current browser window or tab.
    pub async fn close_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().close_window().await?;
        Ok(())
    }

    /// Switches the context to the parent frame.
    pub async fn switch_to_parent_frame(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().enter_parent_frame().await?;
        Ok(())
    }

    /// Returns `true` if `css` is visible on the page.
    pub async fn is_element_visible(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Returns `true` if `text` is visible inside the element selected by `css`.
    pub async fn is_text_visible(&self, text: &str, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => {
                if !elem.is_displayed().await.unwrap_or(false) {
                    return Ok(false);
                }
                let elem_text = elem.text().await.unwrap_or_default();
                Ok(elem_text.contains(text))
            }
            Err(_) => Ok(false),
        }
    }

    /// Waits up to `timeout` seconds for the element `css` to become invisible.
    pub async fn wait_for_element_not_visible(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("wait_for_element_not_visible", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(self.effective_timeout(timeout));
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Element {} did not become invisible within {} seconds",
                    css, timeout
                )));
            }
            match self.session.driver().find(by.clone()).await {
                Ok(elem) => {
                    if !elem.is_displayed().await.unwrap_or(true) {
                        return Ok(());
                    }
                }
                Err(_) => return Ok(()), // Not present means not visible
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Saves all current cookies to `file_path` as JSON.
    pub async fn save_cookies(&self, file_path: &str) -> Result<(), SeleniumBaseError> {
        let cookies = self.session.driver().get_all_cookies().await?;
        let json = serde_json::to_string_pretty(&cookies).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to serialize cookies: {}", e))
        })?;
        std::fs::write(file_path, json).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to write cookies: {}", e))
        })?;
        Ok(())
    }

    /// Loads cookies from `file_path` (JSON) and adds them to the browser.
    pub async fn load_cookies(&self, file_path: &str) -> Result<(), SeleniumBaseError> {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to read cookies: {}", e))
        })?;
        let cookies: Vec<thirtyfour::cookie::Cookie> =
            serde_json::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("Failed to deserialize cookies: {}", e))
            })?;
        for cookie in cookies {
            self.session.driver().add_cookie(cookie).await?;
        }
        Ok(())
    }

    /// Highlights the element `css`, then clicks it.
    pub async fn highlight_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.highlight(css).await?;
        self.click(css).await
    }

    /// Executes the `is_checked` action.
    pub async fn is_checked(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let is_checked = self.get_property(css, "checked").await?;
        Ok(is_checked.as_deref() == Some("true"))
    }

    /// Executes the `check_if_unchecked` action.
    pub async fn check_if_unchecked(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_checked(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Executes the `uncheck_if_checked` action.
    pub async fn uncheck_if_checked(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_checked(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Executes the `open_new_window` action.
    pub async fn open_new_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.open('');").await?;
        self.switch_to_newest_window().await?;
        Ok(())
    }

    /// Executes the `open_new_tab` action.
    pub async fn open_new_tab(&mut self) -> Result<(), SeleniumBaseError> {
        self.open_new_window().await // Essentially the same in modern browsers
    }

    /// Executes the `switch_to_default_window` action.
    pub async fn switch_to_default_window(&mut self) -> Result<(), SeleniumBaseError> {
        let handles = self.session.driver().windows().await?;
        if let Some(first) = handles.first() {
            self.session
                .driver()
                .switch_to_window(first.clone())
                .await?;
        }
        Ok(())
    }

    /// Executes the `switch_to_newest_window` action.
    pub async fn switch_to_newest_window(&mut self) -> Result<(), SeleniumBaseError> {
        let handles = self.session.driver().windows().await?;
        if let Some(last) = handles.last() {
            self.session.driver().switch_to_window(last.clone()).await?;
        }
        Ok(())
    }

    /// Executes the `get_active_element_css` action.
    pub async fn get_active_element_css(&self) -> Result<String, SeleniumBaseError> {
        let script = r#"
            let el = document.activeElement;
            if (!el) return '';
            let path = [];
            while (el.nodeType === Node.ELEMENT_NODE) {
                let selector = el.nodeName.toLowerCase();
                if (el.id) {
                    selector += '#' + el.id;
                    path.unshift(selector);
                    break;
                } else {
                    let sib = el, nth = 1;
                    while (sib = sib.previousElementSibling) {
                        if (sib.nodeName.toLowerCase() == selector)
                           nth++;
                    }
                    if (nth != 1) selector += ":nth-of-type("+nth+")";
                }
                path.unshift(selector);
                el = el.parentNode;
            }
            return path.join(' > ');
        "#;
        match self.execute_script(script).await? {
            Value::String(s) => Ok(s),
            _ => Ok(String::new()),
        }
    }

    /// Waits up to `timeout` seconds for `css` to appear in the DOM.
    pub async fn wait_for_element_present(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(self.effective_timeout(timeout));
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Element {} not present after {} seconds",
                    css, timeout
                )));
            }
            if self.session.driver().find(by.clone()).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn open(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.record("open", Some(url), None);
        self.session.goto(url).await
    }

    /// Executes the `refresh` action.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.session.refresh().await
    }

    /// Navigates back in browser history.
    pub async fn go_back(&self) -> Result<(), SeleniumBaseError> {
        self.session.back().await
    }

    /// Navigates forward in browser history.
    pub async fn go_forward(&self) -> Result<(), SeleniumBaseError> {
        self.session.forward().await
    }

    /// Clicks the element selected by `css`.
    pub async fn click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("click", Some(css), None);
        self.session.click(by).await
    }

    /// Executes the `type_text` action.
    pub async fn type_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("type_text", Some(css), Some(text));
        self.session.type_text(by, text).await
    }

    /// Clears the value of the element selected by `css`.
    pub async fn clear(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("clear", Some(css), None);
        self.session.clear(by).await
    }

    /// Executes the `click_link_text` action.
    pub async fn click_link_text(&mut self, link_text: &str) -> Result<(), SeleniumBaseError> {
        let by = thirtyfour::prelude::By::LinkText(link_text.to_owned());
        self.record("click_link_text", Some(link_text), None);
        self.session.click(by).await
    }

    /// Submits the form containing the element selected by `css`.
    pub async fn submit(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("submit", Some(css), None);
        self.session.submit(by).await
    }

    /// Returns the visible text of the element `css`.
    pub async fn get_text(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.text(by).await
    }

    /// Hovers over the element selected by `css`.
    pub async fn hover(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("hover", Some(css), None);
        self.session.hover(by).await
    }

    /// Executes the `hover_and_click` action.
    pub async fn hover_and_click(
        &mut self,
        hover_css: &str,
        click_css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.hover(hover_css).await?;
        self.sleep(0.1).await;
        self.click(click_css).await
    }

    /// Selects the `<option>` with visible text `text`.
    pub async fn select_option_by_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("select_option_by_text", Some(css), Some(text));
        self.session.select_option_by_text(by, text).await
    }

    /// Selects the `<option>` whose `value` matches.
    pub async fn select_option_by_value(
        &mut self,
        css: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("select_option_by_value", Some(css), Some(value));
        self.session.select_option_by_value(by, value).await
    }

    /// Switches context into the frame selected by `css`.
    pub async fn switch_to_frame(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("switch_to_frame", Some(css), None);
        self.session.switch_to_frame(by).await
    }

    /// Switches context back to the top-level document.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_default_content", None, None);
        self.session.switch_to_default_content().await
    }

    /// Drags the source element onto the target element.
    pub async fn drag_and_drop(
        &mut self,
        source_css: &str,
        target_css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let source_by = Selector::Css(source_css).to_by()?;
        let target_by = Selector::Css(target_css).to_by()?;
        self.record("drag_and_drop", Some(source_css), Some(target_css));
        self.session.drag_and_drop(source_by, target_by).await
    }

    /// Returns `true` if `css` exists in the DOM.
    pub async fn is_element_present(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.element_present(by).await
    }

    /// Executes the `assert_element` action.
    pub async fn assert_element(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_present(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{css}' to be present"
        )))
    }

    /// Executes the `assert_element_absent` action.
    pub async fn assert_element_absent(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_element_present(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{css}' to be absent"
        )))
    }

    /// Returns the current page title.
    pub async fn get_title(&mut self) -> Result<String, SeleniumBaseError> {
        self.session.current_title().await
    }

    /// Returns the current page URL.
    pub async fn get_current_url(&mut self) -> Result<String, SeleniumBaseError> {
        self.session.current_url().await
    }

    /// Executes the `assert_url_contains` action.
    pub async fn assert_url_contains(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if url.contains(expected) {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected URL to contain '{expected}', got '{url}'"
        )))
    }

    /// Executes the `get_page_source` action.
    pub async fn get_page_source(&self) -> Result<String, SeleniumBaseError> {
        self.session.page_source().await
    }

    /// Executes arbitrary JavaScript and returns the result.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_script(script).await
    }

    /// Activates CDP domains on the current session.
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        self.session.activate_cdp_mode().await
    }

    /// Reconnects the underlying WebDriver session (useful in UC mode for a clean session).
    pub async fn reconnect(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.reconnect(&self.config).await
    }

    /// Executes a CDP command without parameters.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp(method).await
    }

    /// Executes a CDP command with the given JSON parameters.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp_with_params(method, params).await
    }

    /// Clears the browser cache via CDP.
    pub async fn clear_browser_cache(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cache().await
    }

    /// Clears all browser cookies via CDP.
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cookies().await
    }

    /// Returns all cookies via CDP as JSON.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        self.session.get_cookies().await
    }

    /// Dispatches a CDP mouse click at screen coordinates `(x, y)`.
    pub async fn cdp_mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        self.session.cdp_mouse_click(x, y).await
    }

    /// Inserts `text` via CDP without using element focus.
    pub async fn cdp_type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.session.cdp_type_text(text).await
    }

    /// Finds `css` and dispatches a CDP mouse click at its center.
    pub async fn cdp_click_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("cdp_click_element", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        let element = self.session.find(by).await?;
        let rect = element.rect().await?;
        let center_x = rect.x + (rect.width / 2.0);
        let center_y = rect.y + (rect.height / 2.0);
        self.session.cdp_mouse_click(center_x, center_y).await
    }

    /// Configures network throttling/latency via CDP.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        self.session.set_network_conditions(conditions).await
    }

    /// Sets the browser timezone via CDP (e.g. "America/New_York").
    pub async fn set_timezone(&self, timezone_id: &str) -> Result<(), SeleniumBaseError> {
        self.session.set_timezone(timezone_id).await
    }

    /// Sets the browser geolocation via CDP.
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), SeleniumBaseError> {
        self.session
            .set_geolocation(latitude, longitude, accuracy)
            .await
    }

    /// Alias for `wait_for_element_present`.
    pub async fn wait_for_element(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.wait_for_element(by, timeout_secs).await?;
        Ok(())
    }

    /// Returns the value of `attribute` on the element `css`, if present.
    pub async fn get_attribute(
        &mut self,
        css: &str,
        attribute_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.get_attribute(by, attribute_name).await
    }

    /// Executes the `get_property` action.
    pub async fn get_property(
        &mut self,
        css: &str,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.get_property(by, property_name).await
    }

    /// Waits up to `timeout` seconds for `css` to become visible.
    pub async fn wait_for_element_visible(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("wait_for_element_visible", Some(css), None);
        self.session
            .wait_for_element_visible(by, timeout_secs)
            .await?;
        Ok(())
    }

    /// Waits up to `timeout` seconds for `css` to be removed from the DOM.
    pub async fn wait_for_element_absent(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("wait_for_element_absent", Some(css), None);
        self.session.wait_for_element_absent(by, timeout_secs).await
    }

    /// Executes the `wait_for_text` action.
    pub async fn wait_for_text(
        &self,
        css: &str,
        expected_substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session
            .wait_for_text(by, expected_substring, timeout_secs)
            .await
    }

    /// Executes the `assert_title_contains` action.
    pub async fn assert_title_contains(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let title = self.get_title().await?;
        if title.contains(expected) {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected title to contain '{expected}', got '{title}'"
        )))
    }

    /// Executes the `assert_text` action.
    pub async fn assert_text(
        &mut self,
        css: &str,
        expected: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("assert_text", Some(css), Some(expected));
        let by = Selector::Css(css).to_by()?;
        let text = self.session.text(by).await?;
        if text.contains(expected) {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected text '{expected}' in element '{css}', got '{text}'"
        )))
    }

    /// Executes the `assert_exact_text` action.
    pub async fn assert_exact_text(
        &mut self,
        css: &str,
        expected: &str,
    ) -> Result<(), SeleniumBaseError> {
        let text = self.get_text(css).await?;
        if text == expected {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected exact text '{expected}' in element '{css}', got '{text}'"
        )))
    }

    /// Executes the `highlight` action.
    pub async fn highlight(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var e=document.querySelector({q}); if(e){{e.style.outline='3px solid magenta'; e.style.outlineOffset='2px';}}",
            q = serde_json::to_string(css).map_err(|e| {
                SeleniumBaseError::InvalidSelector(format!("failed to escape selector: {e}"))
            })?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Displays a transient message overlay on the page for `duration_secs`.
    pub async fn post_message_for(
        &self,
        message: &str,
        duration_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let escaped = serde_json::to_string(message).map_err(|e| {
            SeleniumBaseError::InvalidSelector(format!("failed to escape message: {e}"))
        })?;
        let script = format!(
            "(() => {{
                const id='sb-rs-msg';
                let box=document.getElementById(id);
                if(!box) {{
                  box=document.createElement('div');
                  box.id=id;
                  box.style.position='fixed';
                  box.style.top='16px';
                  box.style.right='16px';
                  box.style.zIndex='2147483647';
                  box.style.background='rgba(20,20,20,0.92)';
                  box.style.color='#fff';
                  box.style.padding='10px 14px';
                  box.style.borderRadius='8px';
                  box.style.fontFamily='Arial,sans-serif';
                  box.style.fontSize='14px';
                  document.body.appendChild(box);
                }}
                box.textContent={msg};
                setTimeout(() => {{ if (box && box.parentNode) box.parentNode.removeChild(box); }}, {ms});
            }})();",
            msg = escaped,
            ms = duration_secs.saturating_mul(1000),
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Captures a screenshot and writes it to `path`.
    async fn save_screenshot_to_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), SeleniumBaseError> {
        self.session.screenshot(path.as_ref()).await
    }

    /// Returns the current screenshot as PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.session.screenshot_as_png().await
    }

    /// Saves the current page source to `path`.
    async fn save_page_source_to_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), SeleniumBaseError> {
        let html = self.get_page_source().await?;
        std::fs::write(path.as_ref(), html).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write page source '{}': {e}",
                path.as_ref().display()
            ))
        })?;
        Ok(())
    }

    /// Executes the `save_screenshot_to_logs` action.
    pub async fn save_screenshot_to_logs(&self) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = artifact_path(&dir, "screenshot", "png");
        self.save_screenshot_to_path(&path).await?;
        Ok(path)
    }

    /// Executes the `save_page_source_to_logs` action.
    pub async fn save_page_source_to_logs(&self) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = artifact_path(&dir, "page_source", "html");
        self.save_page_source_to_path(&path).await?;
        Ok(path)
    }

    /// Executes the `recorded_actions` action.
    pub fn recorded_actions(&self) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let recorder = self
            .recorder
            .lock()
            .map_err(|_| SeleniumBaseError::Unsupported("recorder mutex poisoned".to_owned()))?;
        Ok(recorder.actions.clone())
    }

    /// Executes the `export_recording_as_rust` action.
    pub fn export_recording_as_rust(&self) -> Result<String, SeleniumBaseError> {
        let recorder = self
            .recorder
            .lock()
            .map_err(|_| SeleniumBaseError::Unsupported("recorder mutex poisoned".to_owned()))?;
        Ok(recorder.to_rust_script())
    }

    /// Executes the `save_recording_to_logs` action.
    pub fn save_recording_to_logs(&self) -> Result<(PathBuf, PathBuf), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let json_path = artifact_path(&dir, "recording", "json");
        let rust_path = artifact_path(&dir, "recording", "rs");

        let actions = self.recorded_actions()?;
        let json_data = serde_json::to_string_pretty(&actions).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to serialize recording json: {e}"))
        })?;
        std::fs::write(&json_path, json_data).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to write recording json: {e}"))
        })?;

        let rust_script = self.export_recording_as_rust()?;
        std::fs::write(&rust_path, rust_script).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to write recording rust file: {e}"))
        })?;

        Ok((json_path, rust_path))
    }

    /// Pauses execution for `seconds`.
    pub async fn sleep(&self, seconds: f64) {
        let millis = if seconds <= 0.0 {
            0_u64
        } else {
            (seconds * 1000.0) as u64
        };
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }

    /// Executes the `execute_async_script` action.
    pub async fn execute_async_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_async_script(script).await
    }

    /// Maximizes the browser window.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.record("maximize_window", None, None);
        self.session.maximize_window().await
    }

    /// Resizes the browser window to `width` x `height`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.record(
            "set_window_size",
            Some(&format!("{},{}", width, height)),
            None,
        );
        self.session.set_window_size(width, height).await
    }

    /// Executes the `get_window_size` action.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        self.session.get_window_size().await
    }

    /// Executes the `switch_to_window` action.
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_window", Some(handle), None);
        self.session.switch_to_window(handle).await
    }

    /// Double-clicks the element selected by `css`.
    pub async fn double_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("double_click", Some(css), None);
        self.session.double_click(by).await
    }

    /// Right-clicks the element selected by `css`.
    pub async fn context_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("context_click", Some(css), None);
        self.session.context_click(by).await
    }

    /// Executes the `is_enabled` action.
    pub async fn is_enabled(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.is_enabled(by).await
    }

    /// Executes the `is_selected` action.
    pub async fn is_selected(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.is_selected(by).await
    }

    /// Executes the `is_displayed` action.
    pub async fn is_displayed(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.is_displayed(by).await
    }

    /// Executes the `accept_alert` action.
    pub async fn accept_alert(&self) -> Result<(), SeleniumBaseError> {
        self.record("accept_alert", None, None);
        self.session.switch_to_alert_accept().await
    }

    /// Executes the `dismiss_alert` action.
    pub async fn dismiss_alert(&self) -> Result<(), SeleniumBaseError> {
        self.record("dismiss_alert", None, None);
        self.session.switch_to_alert_dismiss().await
    }

    /// Executes the `get_alert_text` action.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.session.get_alert_text().await
    }

    /// Executes the `type_alert_text` action.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("type_alert_text", Some(text), None);
        self.session.type_alert_text(text).await
    }

    /// Executes the `set_local_storage_item` action.
    pub async fn set_local_storage_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!("window.localStorage.setItem('{}', '{}');", key, value);
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `get_local_storage_item` action.
    pub async fn get_local_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        let script = format!("return window.localStorage.getItem('{}');", key);
        self.execute_script(&script).await
    }

    /// Executes the `remove_local_storage_item` action.
    pub async fn remove_local_storage_item(&self, key: &str) -> Result<(), SeleniumBaseError> {
        let script = format!("window.localStorage.removeItem('{}');", key);
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `clear_local_storage` action.
    pub async fn clear_local_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.localStorage.clear();").await?;
        Ok(())
    }

    /// Executes the `scroll_to_bottom` action.
    pub async fn scroll_to_bottom(&self) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to_bottom", None, None);
        self.execute_script("window.scrollTo(0, document.body.scrollHeight);")
            .await?;
        Ok(())
    }

    /// Executes the `scroll_to_top` action.
    pub async fn scroll_to_top(&self) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to_top", None, None);
        self.execute_script("window.scrollTo(0, 0);").await?;
        Ok(())
    }

    /// Executes the `scroll_to` action.
    pub async fn scroll_to(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to", Some(css), None);
        let script = format!("document.querySelector('{}').scrollIntoView();", css);
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `assert_element_visible` action.
    pub async fn assert_element_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_displayed(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{}' to be visible",
            css
        )))
    }

    /// Executes the `assert_element_not_visible` action.
    pub async fn assert_element_not_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_displayed(css).await.unwrap_or_default();
        if !is_visible {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{}' to not be visible",
            css
        )))
    }

    /// Executes the `click_if_visible` action.
    pub async fn click_if_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_displayed(css).await.unwrap_or(false) {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Waits up to `timeout` seconds for `css` to become clickable.
    pub async fn wait_for_element_clickable(
        &mut self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("wait_for_element_clickable", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        self.session
            .wait_for_element_clickable(by, timeout_secs)
            .await?;
        Ok(())
    }

    /// Returns the shadow root of the element `css`.
    pub async fn get_shadow_root(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.record("get_shadow_root", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        let element = self.session.wait_for_element(by, 10).await?;
        element
            .get_shadow_root()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Executes the `add_cookie` action.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        self.record("add_cookie", Some(name), Some(value));
        self.session.add_cookie(name, value).await
    }

    /// Executes the `get_cookie` action.
    pub async fn get_cookie(
        &self,
        name: &str,
    ) -> Result<thirtyfour::cookie::Cookie, SeleniumBaseError> {
        self.session.get_cookie(name).await
    }

    /// Executes the `delete_cookie` action.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.record("delete_cookie", Some(name), None);
        self.session.delete_cookie(name).await
    }

    /// Executes the `js_click` action.
    pub async fn js_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_click", Some(css), None);
        let script = format!("document.querySelector('{}').click();", css);
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `js_type` action.
    pub async fn js_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_type", Some(css), Some(text));
        let script = format!("document.querySelector('{}').value = '{}';", css, text);
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `set_attribute` action.
    pub async fn set_attribute(
        &mut self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record(
            "set_attribute",
            Some(css),
            Some(&format!("{}={}", attribute, value)),
        );
        let script = format!(
            "document.querySelector('{}').setAttribute('{}', '{}');",
            css, attribute, value
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `remove_attribute` action.
    pub async fn remove_attribute(
        &mut self,
        css: &str,
        attribute: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("remove_attribute", Some(css), Some(attribute));
        let script = format!(
            "document.querySelector('{}').removeAttribute('{}');",
            css, attribute
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `choose_file` action.
    pub async fn choose_file(
        &mut self,
        css: &str,
        file_path: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("choose_file", Some(css), Some(file_path));
        let by = thirtyfour::By::Css(css);
        let element = self.session.wait_for_element(by, 10).await?;

        let path = std::path::Path::new(file_path);
        let abs_path = std::fs::canonicalize(path)
            .map_err(|e| SeleniumBaseError::AssertionFailed(format!("File not found: {}", e)))?;

        // For choose_file, we use send_keys with the absolute file path on an <input type="file">
        element
            .send_keys(abs_path.to_string_lossy().as_ref())
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Executes the `delete_all_cookies` action.
    pub async fn delete_all_cookies(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("delete_all_cookies", None, None);
        self.session.delete_all_cookies().await
    }

    /// Executes the `switch_to_new_window` action.
    pub async fn switch_to_new_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_new_window", None, None);
        self.session.switch_to_new_window().await
    }

    /// Returns the first element matching `css`.
    pub async fn find_element(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.record("find_element", Some(css), None);
        let by = thirtyfour::By::Css(css);
        self.session.wait_for_element(by, 10).await
    }

    /// Returns all elements matching `css`.
    pub async fn find_elements(
        &mut self,
        css: &str,
    ) -> Result<Vec<thirtyfour::WebElement>, SeleniumBaseError> {
        self.record("find_elements", Some(css), None);
        let by = thirtyfour::By::Css(css);
        self.session.find_elements(by).await
    }

    /// Executes the `slow_click` action.
    pub async fn slow_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("slow_click", Some(css), None);
        let by = thirtyfour::By::Css(css);
        let element = self.session.wait_for_element_clickable(by, 10).await?;
        self.hover(css).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        element
            .click()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Closes the browser and ends the session.
    pub async fn quit(self) -> Result<(), SeleniumBaseError> {
        self.session.quit().await
    }

    fn record(&self, name: &str, target: Option<&str>, value: Option<&str>) {
        if let Ok(mut recorder) = self.recorder.lock() {
            recorder.record(name, target, value);
        }
    }

    /// Appends `text` to the element selected by `css`.
    pub async fn add_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("add_text", Some(css), Some(text));
        let elem = self.session.driver().find(by).await?;
        elem.send_keys(text).await?;
        Ok(())
    }

    /// Executes the `send_keys` action.
    pub async fn send_keys(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.add_text(css, text).await
    }

    /// Executes the `get_value` action.
    pub async fn get_value(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        Ok(self.get_attribute(css, "value").await?.unwrap_or_default())
    }

    /// Executes the `click_visible_elements` action.
    pub async fn click_visible_elements(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let elems = self.session.driver().find_all(by).await?;
        for elem in elems {
            if elem.is_displayed().await.unwrap_or(false) {
                elem.click().await?;
            }
        }
        Ok(())
    }

    /// Executes the `wait_for_and_accept_alert` action.
    pub async fn wait_for_and_accept_alert(&self, timeout: u64) -> Result<(), SeleniumBaseError> {
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Alert not present after {} seconds",
                    timeout
                )));
            }
            if self.session.driver().get_alert_text().await.is_ok() {
                self.session.driver().accept_alert().await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Executes the `wait_for_and_dismiss_alert` action.
    pub async fn wait_for_and_dismiss_alert(&self, timeout: u64) -> Result<(), SeleniumBaseError> {
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Alert not present after {} seconds",
                    timeout
                )));
            }
            if self.session.driver().get_alert_text().await.is_ok() {
                self.session.driver().dismiss_alert().await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Executes the `is_link_text_visible` action.
    pub async fn is_link_text_visible(&self, link_text: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::LinkText(link_text).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Executes the `is_partial_link_text_visible` action.
    pub async fn is_partial_link_text_visible(
        &self,
        partial_link_text: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let by = Selector::PartialLinkText(partial_link_text).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Executes the `assert_link_text` action.
    pub async fn assert_link_text(&self, link_text: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_link_text_visible(link_text).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Link text '{}' is not visible",
                link_text
            )));
        }
        Ok(())
    }

    /// Executes the `click_partial_link_text` action.
    pub async fn click_partial_link_text(
        &mut self,
        partial_link_text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::PartialLinkText(partial_link_text).to_by()?;
        self.record("click_partial_link_text", Some(partial_link_text), None);
        self.session.driver().find(by).await?.click().await?;
        Ok(())
    }

    /// Executes the `human_type` action, typing character by character with random delays.
    pub async fn human_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("human_type", Some(css), Some(text));
        let elem = self.session.driver().find(by).await?;
        elem.click().await?; // Focus the field

        let mut rng = rand::rng();
        for c in text.chars() {
            elem.send_keys(&c.to_string()).await?;
            let delay = rng.random_range(30..=120);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        Ok(())
    }

    /// Executes the `human_click` action, adding a random pre-click delay.
    pub async fn human_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("human_click", Some(css), None);
        let elem = self.session.driver().find(by).await?;

        let mut rng = rand::rng();
        let pre_delay = rng.random_range(100..=300);
        tokio::time::sleep(Duration::from_millis(pre_delay)).await;

        elem.click().await?;
        Ok(())
    }

    /// Executes the `smooth_scroll_to` action.
    pub async fn smooth_scroll_to(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).scrollIntoView({{behavior: 'smooth', block: 'center'}});",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        Ok(())
    }

    /// Executes the `uc_click` action (alias for human_click, often used in UC mode stealth).
    pub async fn uc_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.human_click(css).await
    }

    /// Executes the `uc_type` action (alias for human_type, often used in UC mode stealth).
    pub async fn uc_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.human_type(css, text).await
    }

    // --- JS Code Execution Helpers ---

    pub async fn execute_active_css_js(&mut self) -> Result<(), SeleniumBaseError> {
        let js = crate::stealth::js::active_css_js::get_active_css_js();
        self.execute_script(js).await?;
        Ok(())
    }

    pub async fn activate_recorder(&mut self) -> Result<(), SeleniumBaseError> {
        let js = crate::stealth::js::recorder_js::get_recorder_js();
        self.execute_script(js).await?;
        Ok(())
    }

    // --- Tour Helpers ---

    pub async fn create_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name));
        Ok(())
    }

    pub async fn add_tour_step(
        &mut self,
        message: &str,
        target: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        match self.tour.as_mut() {
            Some(tour) => {
                tour.add_step(message, target);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No tour created. Call create_tour first.".to_owned(),
            )),
        }
    }

    pub async fn play_tour(&mut self) -> Result<(), SeleniumBaseError> {
        let tour = self
            .tour
            .take()
            .ok_or_else(|| SeleniumBaseError::InvalidConfig("No tour created.".to_owned()))?;
        let result = tour.play(self).await;
        self.tour = Some(tour);
        result
    }

    pub async fn export_tour(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.tour.as_ref() {
            Some(tour) => tour.export_html(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No tour created.".to_owned(),
            )),
        }
    }

    // --- Visual Testing ---

    pub async fn check_window(&mut self, name: &str, _level: f64) -> Result<(), SeleniumBaseError> {
        let baseline_dir = PathBuf::from("visual_baseline");
        std::fs::create_dir_all(&baseline_dir).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to create visual_baseline dir: {e}"))
        })?;
        let baseline_path = baseline_dir.join(format!("{name}.png"));
        let current = self.screenshot_as_png().await?;

        if baseline_path.exists() {
            let baseline = std::fs::read(&baseline_path).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to read baseline: {e}"))
            })?;
            if baseline != current {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Visual regression detected for '{name}'"
                )));
            }
        } else {
            std::fs::write(&baseline_path, current).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to write baseline: {e}"))
            })?;
        }
        Ok(())
    }

    // --- MasterQA Integration ---

    pub async fn verify(&mut self, statement: &str) -> Result<(), SeleniumBaseError> {
        if !MasterQA::verify(statement) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Manual QA verification failed for: {statement}"
            )));
        }
        Ok(())
    }

    // --- Deferred Asserts ---

    pub async fn deferred_assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.deferred.add_element(css);
        Ok(())
    }

    pub async fn deferred_assert_text(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred.add_text(text, css);
        Ok(())
    }

    pub async fn process_deferred_asserts(&mut self) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        let result = deferred.process(self).await;
        self.deferred = deferred;
        result
    }

    // --- Dashboards and Charts ---

    pub async fn create_presentation(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.presentation = Some(Presentation::new(title));
        Ok(())
    }

    pub async fn add_presentation_slide(&mut self, content: &str) -> Result<(), SeleniumBaseError> {
        match self.presentation.as_mut() {
            Some(presentation) => {
                presentation.add_slide(content);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No presentation created. Call create_presentation first.".to_owned(),
            )),
        }
    }

    pub async fn save_presentation(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.presentation.as_ref() {
            Some(presentation) => presentation.save(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No presentation created.".to_owned(),
            )),
        }
    }

    pub async fn create_pie_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Pie));
        Ok(())
    }

    pub async fn create_bar_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Bar));
        Ok(())
    }

    pub async fn create_line_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Line));
        Ok(())
    }

    pub async fn create_area_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Area));
        Ok(())
    }

    pub async fn create_column_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Column));
        Ok(())
    }

    pub async fn add_data_point(
        &mut self,
        label: &str,
        value: i32,
    ) -> Result<(), SeleniumBaseError> {
        match self.chart.as_mut() {
            Some(chart) => {
                chart.add_data_point(label, value);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created. Call create_pie_chart first.".to_owned(),
            )),
        }
    }

    pub async fn save_pie_chart(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        self.save_chart(filename).await
    }

    pub async fn save_chart(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.chart.as_ref() {
            Some(chart) => chart.save(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created.".to_owned(),
            )),
        }
    }

    // --- Additional Python parity methods ---

    /// Alias for `type_text` that clears the field first.
    pub async fn update_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("update_text", Some(css), Some(text));
        let by = Selector::Css(css).to_by()?;
        self.session.type_text(by, text).await
    }

    /// Focus the element matching `css`.
    pub async fn focus(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).focus();",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Minimize the browser window.
    pub async fn minimize_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().minimize_window().await?;
        Ok(())
    }

    /// Set the window rectangle directly.
    pub async fn set_window_rect(
        &self,
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    ) -> Result<(), SeleniumBaseError> {
        self.session
            .driver()
            .set_window_rect(x, y, width, height)
            .await?;
        Ok(())
    }

    /// Reset window size to the default 1280x720.
    pub async fn reset_window_size(&self) -> Result<(), SeleniumBaseError> {
        self.set_window_size(1280, 720).await
    }

    /// Scroll vertically by `delta_y` pixels.
    pub async fn scroll_by_y(&self, delta_y: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollBy(0, {delta_y});"))
            .await?;
        Ok(())
    }

    /// Scroll up by 400 pixels.
    pub async fn scroll_up(&self) -> Result<(), SeleniumBaseError> {
        self.scroll_by_y(-400).await
    }

    /// Scroll down by 400 pixels.
    pub async fn scroll_down(&self) -> Result<(), SeleniumBaseError> {
        self.scroll_by_y(400).await
    }

    /// Click the element matching the given XPath.
    pub async fn click_xpath(&mut self, xpath: &str) -> Result<(), SeleniumBaseError> {
        self.record("click_xpath", Some(xpath), None);
        let by = By::XPath(xpath.to_owned());
        self.session.click(by).await
    }

    /// JavaScript-click the element only if it is present in the DOM.
    pub async fn js_click_if_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_present(css).await? {
            self.js_click(css).await?;
        }
        Ok(())
    }

    /// JavaScript-click all elements matching `css`.
    pub async fn js_click_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_click_all", Some(css), None);
        let script = format!(
            "document.querySelectorAll({}).forEach(e => e.click());",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Click the element using jQuery if available, otherwise fall back to JS click.
    pub async fn jquery_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("jquery_click", Some(css), None);
        let script = format!(
            "(function(){{
                if (window.jQuery && jQuery({}).length) {{ jQuery({})[0].click(); }}
                else {{ document.querySelector({}).click(); }}
            }})();",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?,
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?,
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Hide the element matching `css` by setting `display:none`.
    pub async fn hide_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).style.display='none';",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Show the element matching `css` by setting `display:block`.
    pub async fn show_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).style.display='block';",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Remove the element matching `css` from the DOM.
    pub async fn remove_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var e=document.querySelector({}); if(e) e.parentNode.removeChild(e);",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Block common ad elements on the current page.
    pub async fn block_ads(&mut self) -> Result<(), SeleniumBaseError> {
        let selectors = [
            "[id*='google_ads']",
            "[id*='ad-']",
            "[class*='ad-']",
            "[class*='ads ']",
            "iframe[src*='ads']",
            "iframe[src*='doubleclick']",
        ];
        let joined = selectors.join(",");
        let script = format!(
            "document.querySelectorAll({}).forEach(e => e.remove());",
            serde_json::to_string(&joined)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Assert that the current URL exactly equals `expected`.
    pub async fn assert_url(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if url == expected {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected URL '{expected}', got '{url}'"
        )))
    }

    /// Return the unique absolute links on the current page.
    pub async fn get_unique_links(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let script = r#"
            return Array.from(document.querySelectorAll('a[href]'))
                .map(a => a.href)
                .filter((v, i, a) => a.indexOf(v) === i);
        "#;
        match self.execute_script(script).await? {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    /// Fail if any link on the page returns HTTP 404.
    pub async fn assert_no_404_errors(&mut self) -> Result<(), SeleniumBaseError> {
        let links = self.get_unique_links().await?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| SeleniumBaseError::Unsupported(e.to_string()))?;
        let mut broken = Vec::new();
        for link in links {
            if link.starts_with("http://") || link.starts_with("https://") {
                match client.head(&link).send().await {
                    Ok(resp) if resp.status().as_u16() == 404 => broken.push(link),
                    _ => {}
                }
            }
        }
        if broken.is_empty() {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "found {} broken link(s) with 404: {:?}",
                broken.len(),
                broken
            )))
        }
    }

    /// Alias for `assert_no_404_errors`.
    pub async fn assert_no_broken_links(&mut self) -> Result<(), SeleniumBaseError> {
        self.assert_no_404_errors().await
    }

    /// Return all visible text options for a `<select>` element.
    pub async fn get_select_options(
        &mut self,
        css: &str,
    ) -> Result<Vec<String>, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let element = self.session.wait_for_element(by, 10).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        let mut options = Vec::new();
        for opt in select.options().await? {
            options.push(opt.text().await.unwrap_or_default());
        }
        Ok(options)
    }

    /// Set the `value` property of the element.
    pub async fn set_value(&mut self, css: &str, value: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).value = {};",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?,
            serde_json::to_string(value)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Set the text content of the element.
    pub async fn set_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).textContent = {};",
            serde_json::to_string(css)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?,
            serde_json::to_string(text)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Get the `src` URL of an image element.
    pub async fn get_image_url(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        self.get_attribute(css, "src").await?.ok_or_else(|| {
            SeleniumBaseError::AssertionFailed(format!("element '{css}' has no src attribute"))
        })
    }

    /// Extract the domain (scheme + host) from the current URL.
    pub async fn get_domain_url(&mut self) -> Result<String, SeleniumBaseError> {
        let url = self.get_current_url().await?;
        reqwest::Url::parse(&url)
            .map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))
            .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("")))
    }

    /// Convert a simple XPath expression to an approximate CSS selector.
    pub fn convert_xpath_to_css(&self, xpath: &str) -> Result<String, SeleniumBaseError> {
        crate::utils::selectors::xpath_to_css(xpath)
    }

    /// Set a soft upper bound for wait-based helpers.
    pub fn set_time_limit(&mut self, seconds: u64) -> Result<(), SeleniumBaseError> {
        self.time_limit_secs = Some(seconds);
        Ok(())
    }

    /// Return whether a JavaScript alert is currently present.
    pub async fn is_alert_present(&self) -> Result<bool, SeleniumBaseError> {
        Ok(self.session.driver().get_alert_text().await.is_ok())
    }

    /// Get the browser user agent string.
    pub async fn get_user_agent(&self) -> Result<String, SeleniumBaseError> {
        match self.execute_script("return navigator.userAgent;").await? {
            Value::String(ua) => Ok(ua),
            _ => Ok(String::new()),
        }
    }

    /// Get the browser locale code.
    pub async fn get_locale_code(&self) -> Result<String, SeleniumBaseError> {
        match self
            .execute_script("return navigator.language || 'en-US';")
            .await?
        {
            Value::String(l) => Ok(l),
            _ => Ok("en-US".to_owned()),
        }
    }

    /// Return whether the element is clickable (visible and enabled).
    pub async fn is_element_clickable(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)
                && elem.is_enabled().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the element is enabled.
    pub async fn is_element_enabled(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.is_enabled(by).await
    }

    fn effective_timeout(&self, requested: u64) -> u64 {
        self.time_limit_secs
            .map(|limit| limit.min(requested))
            .unwrap_or(requested)
    }

    /// Return whether `text` exactly matches the visible text of the element.
    pub async fn is_exact_text_visible(
        &self,
        text: &str,
        css: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)
                && elem.text().await.unwrap_or_default() == text),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the element has any non-empty visible text.
    pub async fn is_non_empty_text_visible(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => {
                let text = elem.text().await.unwrap_or_default();
                Ok(elem.is_displayed().await.unwrap_or(false) && !text.trim().is_empty())
            }
            Err(_) => Ok(false),
        }
    }

    /// Alias for `assert_element_absent`.
    pub async fn assert_element_not_present(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.assert_element_absent(css).await
    }

    /// Assert that all provided CSS selectors are present.
    pub async fn assert_elements_present(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            self.assert_element(css).await?;
        }
        Ok(())
    }

    /// Wait until any of the provided selectors is visible, then return it.
    pub async fn wait_for_any_of_elements_visible(
        &self,
        selectors: &[&str],
        timeout_secs: u64,
    ) -> Result<String, SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            for css in selectors {
                if self.is_element_visible(css).await.unwrap_or(false) {
                    return Ok((*css).to_owned());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(
                    "none of the selectors became visible".to_owned(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Assert that at least one of the selectors is visible.
    pub async fn assert_any_of_elements_visible(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            if self.is_element_visible(css).await.unwrap_or(false) {
                return Ok(());
            }
        }
        Err(SeleniumBaseError::AssertionFailed(
            "none of the selectors is visible".to_owned(),
        ))
    }

    /// Return the inner text of the element.
    pub async fn get_text_content(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let elem = self.session.driver().find(by).await?;
        Ok(elem.text().await.unwrap_or_default())
    }

    /// Press a named key on the active element.
    pub async fn press_keys(&self, key_name: &str) -> Result<(), SeleniumBaseError> {
        let key = match key_name.to_ascii_lowercase().as_str() {
            "enter" => Key::Enter,
            "return" => Key::Return,
            "tab" => Key::Tab,
            "escape" => Key::Escape,
            "esc" => Key::Escape,
            "space" => Key::Space,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            _ => {
                return Err(SeleniumBaseError::InvalidSelector(format!(
                    "unknown key '{key_name}'"
                )))
            }
        };
        self.session
            .driver()
            .action_chain()
            .send_keys(key.value().to_string())
            .perform()
            .await?;
        Ok(())
    }

    // --- Session storage helpers ---

    pub async fn set_session_storage_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "window.sessionStorage.setItem({}, {});",
            serde_json::to_string(key)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?,
            serde_json::to_string(value)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    pub async fn get_session_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        let script = format!(
            "return window.sessionStorage.getItem({});",
            serde_json::to_string(key)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await
    }

    pub async fn remove_session_storage_item(&self, key: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "window.sessionStorage.removeItem({});",
            serde_json::to_string(key)
                .map_err(|e| SeleniumBaseError::InvalidSelector(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    pub async fn clear_session_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.sessionStorage.clear();")
            .await?;
        Ok(())
    }

    pub async fn get_session_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        match self
            .execute_script("return Object.keys(window.sessionStorage);")
            .await?
        {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn get_session_storage_items(
        &self,
    ) -> Result<HashMap<String, String>, SeleniumBaseError> {
        let script = r#"
            const items = {};
            for (let i = 0; i < window.sessionStorage.length; i++) {
                const k = window.sessionStorage.key(i);
                items[k] = window.sessionStorage.getItem(k);
            }
            return items;
        "#;
        match self.execute_script(script).await? {
            Value::Object(map) => Ok(map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect()),
            _ => Ok(HashMap::new()),
        }
    }

    pub async fn get_local_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        match self
            .execute_script("return Object.keys(window.localStorage);")
            .await?
        {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn get_local_storage_items(
        &self,
    ) -> Result<HashMap<String, String>, SeleniumBaseError> {
        let script = r#"
            const items = {};
            for (let i = 0; i < window.localStorage.length; i++) {
                const k = window.localStorage.key(i);
                items[k] = window.localStorage.getItem(k);
            }
            return items;
        "#;
        match self.execute_script(script).await? {
            Value::Object(map) => Ok(map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect()),
            _ => Ok(HashMap::new()),
        }
    }

    // --- Navigation aliases ---

    pub async fn get(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn go_to(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn open_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn visit(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn visit_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn reload(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    pub async fn reload_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    /// Open `url` only if the current URL is not already `url`.
    pub async fn open_if_not_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        if self.get_current_url().await? != url {
            self.open(url).await?;
        }
        Ok(())
    }

    pub async fn goto_if_not_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open_if_not_url(url).await
    }

    // --- Input aliases ---

    pub async fn input(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn fill(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn write(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn select(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.select_option_by_text(css, text).await
    }

    pub async fn right_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.context_click(css).await
    }

    /// Alias to find an element that can be used for further chaining.
    pub async fn get_element(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.find_element(css).await
    }

    /// Alias for `find_element`.
    pub async fn locator(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.find_element(css).await
    }

    /// Alias for `wait_for_element_visible`.
    pub async fn wait_for_selector(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_visible(css, timeout_secs).await
    }

    /// Alias for `wait_for_element` (present).
    pub async fn wait_for_query_selector(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element(css, timeout_secs).await
    }
}

// Additional BaseCase implementations split out to keep the file manageable.
include!("base_case_impl_pdf_html.rs");
include!("base_case_impl_extra.rs");
include!("base_case_impl_cdp_page.rs");
include!("base_case_impl_dialog_inspector.rs");
include!("base_case_impl_shadow.rs");
include!("base_case_impl_gui.rs");
include!("base_case_impl_masterqa.rs");
include!("base_case_impl_remaining.rs");
include!("base_case_impl_remaining_links.rs");
include!("base_case_impl_remaining_downloads.rs");
include!("base_case_impl_remaining_storage.rs");
include!("base_case_impl_remaining_dom.rs");
include!("base_case_impl_remaining_window.rs");
include!("base_case_impl_remaining_mouse.rs");
include!("base_case_impl_remaining_alerts.rs");
include!("base_case_impl_remaining_browser.rs");
include!("base_case_impl_remaining_nav.rs");
include!("base_case_impl_remaining_media.rs");
include!("base_case_impl_remaining_tours.rs");
include!("base_case_impl_remaining_charts.rs");
include!("base_case_impl_remaining_presentations.rs");
include!("base_case_impl_remaining_jslibs.rs");
include!("base_case_impl_remaining_misc.rs");
