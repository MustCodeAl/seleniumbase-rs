use rand::Rng;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::artifacts::{artifact_path, ensure_latest_logs_dir};
use crate::config::BrowserConfig;
use crate::core::selectors::Selector;
use crate::core::session::BrowserSession;
use crate::error::SeleniumBaseError;
use crate::recorder::{ActionRecorder, RecordedAction};
use serde_json::Value;
use thirtyfour::extensions::cdp::NetworkConditions;

pub struct BaseCase {
    session: BrowserSession,
    recorder: Arc<Mutex<ActionRecorder>>,
}

impl BaseCase {
    /// Executes the `new` action.
    pub async fn new(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        let session = BrowserSession::connect(config).await?;
        Ok(Self {
            session,
            recorder: Arc::new(Mutex::new(ActionRecorder::default())),
        })
    }

    /// Executes the `open` action.

    /// Executes the `is_element_visible` action.

    /// Executes the `assert_text_visible` action.
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

    /// Executes the `assert_text_not_visible` action.
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

    /// Executes the `assert_attribute` action.
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

    /// Executes the `assert_title` action.
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

    /// Executes the `wait_for_ready_state_complete` action.
    pub async fn wait_for_ready_state_complete(&self) -> Result<(), SeleniumBaseError> {
        let script = "return document.readyState;";
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_secs() > 10 {
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

    /// Executes the `get_window_position` action.
    pub async fn get_window_position(&self) -> Result<(i64, i64), SeleniumBaseError> {
        let rect = self.session.driver().get_window_rect().await?;
        Ok((rect.x, rect.y))
    }

    /// Executes the `set_window_position` action.
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

    /// Executes the `close_window` action.
    pub async fn close_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().close_window().await?;
        Ok(())
    }

    /// Executes the `switch_to_parent_frame` action.
    pub async fn switch_to_parent_frame(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().enter_parent_frame().await?;
        Ok(())
    }

    pub async fn is_element_visible(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Executes the `is_text_visible` action.
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

    /// Executes the `wait_for_element_not_visible` action.
    pub async fn wait_for_element_not_visible(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("wait_for_element_not_visible", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
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

    /// Executes the `save_cookies` action.
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

    /// Executes the `load_cookies` action.
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

    /// Executes the `highlight_click` action.
    pub async fn highlight_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.highlight(css).await?;
        self.click(css).await
    }

    /// Executes the `hover_and_click` action.

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

    /// Executes the `wait_for_element_present` action.
    pub async fn wait_for_element_present(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
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

    /// Executes the `go_back` action.
    pub async fn go_back(&self) -> Result<(), SeleniumBaseError> {
        self.session.back().await
    }

    /// Executes the `go_forward` action.
    pub async fn go_forward(&self) -> Result<(), SeleniumBaseError> {
        self.session.forward().await
    }

    /// Executes the `click` action.
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

    /// Executes the `clear` action.
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

    /// Executes the `submit` action.
    pub async fn submit(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("submit", Some(css), None);
        self.session.submit(by).await
    }

    /// Executes the `get_text` action.
    pub async fn get_text(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.text(by).await
    }

    /// Executes the `hover` action.
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

    /// Executes the `select_option_by_text` action.
    pub async fn select_option_by_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("select_option_by_text", Some(css), Some(text));
        self.session.select_option_by_text(by, text).await
    }

    /// Executes the `select_option_by_value` action.
    pub async fn select_option_by_value(
        &mut self,
        css: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("select_option_by_value", Some(css), Some(value));
        self.session.select_option_by_value(by, value).await
    }

    /// Executes the `switch_to_frame` action.
    pub async fn switch_to_frame(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("switch_to_frame", Some(css), None);
        self.session.switch_to_frame(by).await
    }

    /// Executes the `switch_to_default_content` action.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_default_content", None, None);
        self.session.switch_to_default_content().await
    }

    /// Executes the `drag_and_drop` action.
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

    /// Executes the `is_element_present` action.
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

    /// Executes the `get_title` action.
    pub async fn get_title(&mut self) -> Result<String, SeleniumBaseError> {
        self.session.current_title().await
    }

    /// Executes the `get_current_url` action.
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

    /// Executes the `execute_script` action.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_script(script).await
    }

    /// Executes the `activate_cdp_mode` action.
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        self.session.activate_cdp_mode().await
    }

    /// Executes the `execute_cdp` action.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp(method).await
    }

    /// Executes the `execute_cdp_with_params` action.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp_with_params(method, params).await
    }

    /// Executes the `clear_browser_cache` action.
    pub async fn clear_browser_cache(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cache().await
    }

    /// Executes the `clear_browser_cookies` action.
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cookies().await
    }

    /// Executes the `get_cookies` action.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        self.session.get_cookies().await
    }

    /// Executes the `cdp_mouse_click` action.
    pub async fn cdp_mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        self.session.cdp_mouse_click(x, y).await
    }

    /// Executes the `cdp_type_text` action.
    pub async fn cdp_type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.session.cdp_type_text(text).await
    }

    /// Executes the `cdp_click_element` action.
    pub async fn cdp_click_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("cdp_click_element", Some(css), None);
        let by = Selector::Css(css).to_by()?;
        let element = self.session.find(by).await?;
        let rect = element.rect().await?;
        let center_x = rect.x + (rect.width / 2.0);
        let center_y = rect.y + (rect.height / 2.0);
        self.session.cdp_mouse_click(center_x, center_y).await
    }

    /// Executes the `set_network_conditions` action.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        self.session.set_network_conditions(conditions).await
    }

    /// Executes the `wait_for_element` action.
    pub async fn wait_for_element(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.wait_for_element(by, timeout_secs).await?;
        Ok(())
    }

    /// Executes the `get_attribute` action.
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

    /// Executes the `wait_for_element_visible` action.
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

    /// Executes the `wait_for_element_absent` action.
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

    /// Executes the `post_message` action.
    pub async fn post_message(
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

    /// Executes the `save_screenshot` action.
    pub async fn save_screenshot<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        self.session.screenshot(path.as_ref()).await
    }

    /// Executes the `save_page_source` action.
    pub async fn save_page_source<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
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
        self.save_screenshot(&path).await?;
        Ok(path)
    }

    /// Executes the `save_page_source_to_logs` action.
    pub async fn save_page_source_to_logs(&self) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = artifact_path(&dir, "page_source", "html");
        self.save_page_source(&path).await?;
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

    /// Executes the `sleep` action.
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

    /// Executes the `maximize_window` action.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.record("maximize_window", None, None);
        self.session.maximize_window().await
    }

    /// Executes the `set_window_size` action.
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

    /// Executes the `double_click` action.
    pub async fn double_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("double_click", Some(css), None);
        self.session.double_click(by).await
    }

    /// Executes the `context_click` action.
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
        let is_visible = match self.is_displayed(css).await {
            Ok(visible) => visible,
            Err(_) => false,
        };
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

    /// Executes the `wait_for_element_clickable` action.
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

    /// Executes the `get_shadow_root` action.
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

    /// Executes the `find_element` action.
    pub async fn find_element(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.record("find_element", Some(css), None);
        let by = thirtyfour::By::Css(css);
        self.session.wait_for_element(by, 10).await
    }

    /// Executes the `find_elements` action.
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

    /// Executes the `quit` action.
    pub async fn quit(self) -> Result<(), SeleniumBaseError> {
        self.session.quit().await
    }

    fn record(&self, name: &str, target: Option<&str>, value: Option<&str>) {
        if let Ok(mut recorder) = self.recorder.lock() {
            recorder.record(name, target, value);
        }
    }

    /// Executes the `add_text` action.
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
        
        let mut rng = rand::thread_rng();
        for c in text.chars() {
            elem.send_keys(&c.to_string()).await?;
            let delay = rng.gen_range(30..=120);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        Ok(())
    }

    /// Executes the `human_click` action, adding a random pre-click delay.
    pub async fn human_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.record("human_click", Some(css), None);
        let elem = self.session.driver().find(by).await?;
        
        let mut rng = rand::thread_rng();
        let pre_delay = rng.gen_range(100..=300);
        tokio::time::sleep(Duration::from_millis(pre_delay)).await;
        
        elem.click().await?;
        Ok(())
    }

    /// Executes the `smooth_scroll_to` action.
    pub async fn smooth_scroll_to(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector('{}').scrollIntoView({{behavior: 'smooth', block: 'center'}});", 
            css.replace("'", "\'")
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
}
