
// Additional BaseCase methods to close the parity gap with Python SeleniumBase.
// Included at the end of `base_case.rs` so the code lives in the same module and
// can access private fields and helpers.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use thirtyfour::WebElement;

/// Escape a string for safe use inside a single-quoted JavaScript literal.
fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Generate a TOTP code for the given base32-encoded secret at `timestamp`.
fn generate_totp(secret: &str, timestamp: u64) -> Result<String, SeleniumBaseError> {
    let secret = secret.replace([' ', '-'], "").to_uppercase();
    let key = base32_decode(&secret)?;
    let interval = timestamp / 30;
    let counter = interval.to_be_bytes();

    use ring::hmac;
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key);
    let tag = hmac::sign(&key, &counter);
    let digest = tag.as_ref();

    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let code = ((u32::from(digest[offset]) & 0x7f) << 24
        | (u32::from(digest[offset + 1]) & 0xff) << 16
        | (u32::from(digest[offset + 2]) & 0xff) << 8
        | (u32::from(digest[offset + 3]) & 0xff))
        % 1_000_000;
    Ok(format!("{code:06}"))
}

/// Decode a base32 string (RFC 4648) into bytes.
fn base32_decode(input: &str) -> Result<Vec<u8>, SeleniumBaseError> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut output = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits_left = 0;
    for ch in input.chars() {
        if ch == '=' {
            continue;
        }
        let val = ALPHABET
            .iter()
            .position(|&c| c as char == ch)
            .ok_or_else(|| SeleniumBaseError::AssertionFailed("invalid base32 character".to_owned()))?
            as u32;
        buffer = (buffer << 5) | val;
        bits_left += 5;
        if bits_left >= 8 {
            bits_left -= 8;
            output.push((buffer >> bits_left) as u8);
        }
    }
    Ok(output)
}

/// Return the default downloads directory used by the browser.
fn default_download_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

impl BaseCase {
    // ========================================================================
    // Pure assertion helpers (no browser required)
    // ========================================================================

    /// Asserts that two values are equal.
    pub fn assert_equal<T: PartialEq + std::fmt::Debug>(
        &self,
        first: T,
        second: T,
    ) -> Result<(), SeleniumBaseError> {
        if first != second {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {:?} to equal {:?}",
                first, second
            )));
        }
        Ok(())
    }

    /// Alias for `assert_equal`.
    pub fn assert_equals<T: PartialEq + std::fmt::Debug>(
        &self,
        first: T,
        second: T,
    ) -> Result<(), SeleniumBaseError> {
        self.assert_equal(first, second)
    }

    /// Asserts that two values are not equal.
    pub fn assert_not_equal<T: PartialEq + std::fmt::Debug>(
        &self,
        first: T,
        second: T,
    ) -> Result<(), SeleniumBaseError> {
        if first == second {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {:?} to not equal {:?}",
                first, second
            )));
        }
        Ok(())
    }

    /// Alias for `assert_not_equal`.
    pub fn assert_not_equals<T: PartialEq + std::fmt::Debug>(
        &self,
        first: T,
        second: T,
    ) -> Result<(), SeleniumBaseError> {
        self.assert_not_equal(first, second)
    }

    /// Asserts that `value` is true.
    pub fn assert_true(&self, value: bool) -> Result<(), SeleniumBaseError> {
        if !value {
            return Err(SeleniumBaseError::AssertionFailed(
                "Expected true, but got false".to_owned(),
            ));
        }
        Ok(())
    }

    /// Asserts that `value` is false.
    pub fn assert_false(&self, value: bool) -> Result<(), SeleniumBaseError> {
        if value {
            return Err(SeleniumBaseError::AssertionFailed(
                "Expected false, but got true".to_owned(),
            ));
        }
        Ok(())
    }

    /// Asserts that two references point to the same object.
    pub fn assert_is<T: ?Sized>(&self, first: &T, second: &T) -> Result<(), SeleniumBaseError> {
        if !std::ptr::eq(first, second) {
            return Err(SeleniumBaseError::AssertionFailed(
                "Expected references to be identical".to_owned(),
            ));
        }
        Ok(())
    }

    /// Asserts that two references do not point to the same object.
    pub fn assert_is_not<T: ?Sized>(&self, first: &T, second: &T) -> Result<(), SeleniumBaseError> {
        if std::ptr::eq(first, second) {
            return Err(SeleniumBaseError::AssertionFailed(
                "Expected references to be different".to_owned(),
            ));
        }
        Ok(())
    }

    /// Asserts that `value` is `None`.
    pub fn assert_none<T: std::fmt::Debug>(&self, value: Option<T>) -> Result<(), SeleniumBaseError> {
        if value.is_some() {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected None, but got {:?}",
                value
            )));
        }
        Ok(())
    }

    /// Asserts that `value` is not `None`.
    pub fn assert_not_none<T: std::fmt::Debug>(
        &self,
        value: Option<T>,
    ) -> Result<(), SeleniumBaseError> {
        if value.is_none() {
            return Err(SeleniumBaseError::AssertionFailed(
                "Expected Some, but got None".to_owned(),
            ));
        }
        Ok(())
    }

    /// Asserts that `item` is contained in `container`.
    pub fn assert_in<T: PartialEq + std::fmt::Debug>(
        &self,
        item: T,
        container: &[T],
    ) -> Result<(), SeleniumBaseError> {
        if !container.contains(&item) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {:?} to be in {:?}",
                item, container
            )));
        }
        Ok(())
    }

    /// Asserts that `item` is not contained in `container`.
    pub fn assert_not_in<T: PartialEq + std::fmt::Debug>(
        &self,
        item: T,
        container: &[T],
    ) -> Result<(), SeleniumBaseError> {
        if container.contains(&item) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {:?} to not be in {:?}",
                item, container
            )));
        }
        Ok(())
    }

    /// Asserts that `value` is an instance of the type named by `type_name`.
    pub fn assert_is_instance(&self, value: &dyn std::any::Any, type_name: &str) -> Result<(), SeleniumBaseError> {
        let actual = std::any::type_name_of_val(value);
        if !actual.contains(type_name) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected instance of '{}', but got '{}'",
                type_name, actual
            )));
        }
        Ok(())
    }

    /// Asserts that `value` is not an instance of the type named by `type_name`.
    pub fn assert_not_is_instance(
        &self,
        value: &dyn std::any::Any,
        type_name: &str,
    ) -> Result<(), SeleniumBaseError> {
        let actual = std::any::type_name_of_val(value);
        if actual.contains(type_name) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected not instance of '{}', but got '{}'",
                type_name, actual
            )));
        }
        Ok(())
    }

    /// Asserts that two floats are equal within `delta`.
    pub fn assert_almost_equal(
        &self,
        first: f64,
        second: f64,
        delta: f64,
    ) -> Result<(), SeleniumBaseError> {
        if (first - second).abs() > delta {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {} to be within {} of {}",
                first, delta, second
            )));
        }
        Ok(())
    }

    /// Asserts that two floats are not equal within `delta`.
    pub fn assert_not_almost_equal(
        &self,
        first: f64,
        second: f64,
        delta: f64,
    ) -> Result<(), SeleniumBaseError> {
        if (first - second).abs() <= delta {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected {} to not be within {} of {}",
                first, delta, second
            )));
        }
        Ok(())
    }

    /// Asserts that the attribute named `attribute` is present on `css`.
    pub async fn assert_attribute_present(
        &mut self,
        css: &str,
        attribute: &str,
    ) -> Result<(), SeleniumBaseError> {
        if !self.is_attribute_present(css, attribute).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Attribute '{}' is not present on element '{}'",
                attribute, css
            )));
        }
        Ok(())
    }

    /// Asserts that the attribute named `attribute` is not present on `css`.
    pub async fn assert_attribute_not_present(
        &mut self,
        css: &str,
        attribute: &str,
    ) -> Result<(), SeleniumBaseError> {
        if self.is_attribute_present(css, attribute).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Attribute '{}' is unexpectedly present on element '{}'",
                attribute, css
            )));
        }
        Ok(())
    }

    /// Asserts that `text` appears inside a link (`<a>`) element as a substring.
    pub async fn assert_partial_link_text(
        &mut self,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        if self.find_partial_link_text(text).await?.is_none() {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Partial link text '{}' was not found",
                text
            )));
        }
        Ok(())
    }

    /// Asserts that the current URL does not contain `substring`.
    pub async fn assert_url_not_contains(
        &mut self,
        substring: &str,
    ) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if url.contains(substring) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected URL '{}' to not contain '{}'",
                url, substring
            )));
        }
        Ok(())
    }

    /// Asserts that the current URL matches `regex`.
    pub async fn assert_url_matches(
        &mut self,
        regex: &str,
    ) -> Result<(), SeleniumBaseError> {
        let re = regex::Regex::new(regex)
            .map_err(|e| SeleniumBaseError::AssertionFailed(e.to_string()))?;
        let url = self.get_current_url().await?;
        if !re.is_match(&url) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected URL '{}' to match regex '{}'",
                url, regex
            )));
        }
        Ok(())
    }

    // ========================================================================
    // Element query helpers
    // ========================================================================

    /// Returns the first `<a>` element whose full visible text equals `text`, if any.
    pub async fn find_link_text(
        &mut self,
        text: &str,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let by = By::LinkText(text);
        match self.session.find(by).await {
            Ok(el) => Ok(Some(el)),
            Err(_) => Ok(None),
        }
    }

    /// Returns the first `<a>` element whose visible text contains `text`, if any.
    pub async fn find_partial_link_text(
        &mut self,
        text: &str,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let by = By::PartialLinkText(text);
        match self.session.find(by).await {
            Ok(el) => Ok(Some(el)),
            Err(_) => Ok(None),
        }
    }

    /// Returns the first element containing the exact visible `text`, if any.
    pub async fn find_exact_text(
        &mut self,
        text: &str,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let script = format!(
            "return Array.from(document.querySelectorAll('*')).find(el => el.textContent.trim() === '{}');",
            js_escape(text)
        );
        let result = self.execute_script(&script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns the first element whose visible text contains `text`, if any.
    pub async fn find_text(
        &mut self,
        text: &str,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let script = format!(
            "return Array.from(document.querySelectorAll('*')).find(el => el.textContent.includes('{}'));",
            js_escape(text)
        );
        let result = self.execute_script(&script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns the first element whose visible text is non-empty, if any.
    pub async fn find_non_empty_text(
        &mut self,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let script = "return Array.from(document.querySelectorAll('*')).find(el => el.textContent.trim().length > 0);";
        let result = self.execute_script(script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns all currently visible elements matching `css`.
    pub async fn find_visible_elements(
        &mut self,
        css: &str,
    ) -> Result<Vec<WebElement>, SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        let all = self.session.find_all(by).await?;
        let mut visible = Vec::new();
        for el in all {
            if el.is_displayed().await.unwrap_or(false) {
                visible.push(el);
            }
        }
        Ok(visible)
    }

    /// Helper to convert a WebDriver element JSON response into a `WebElement`.
    async fn element_from_script_result(
        &mut self,
        result: Value,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        if result.is_null() {
            return Ok(None);
        }
        // Re-query by a generated unique selector. This is the safest portable
        // way to turn a JS element reference into a WebElement usable by the driver.
        let mark: u64 = rand::rng().random::<u64>();
        let mark_attr = format!("sb-find-mark-{}", mark);
        let script = format!(
            "var el = arguments[0]; if (el && el.setAttribute) el.setAttribute('{}', '1');",
            mark_attr
        );
        self.session
            .driver()
            .execute(&script, vec![result.clone()])
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        let by = By::Css(format!("[{}='1']", mark_attr));
        match self.session.find(by).await {
            Ok(el) => {
                // Remove the marker attribute.
                let _ = self
                    .session
                    .driver()
                    .execute(
                        "arguments[0].removeAttribute(arguments[1]);",
                        vec![el.to_json()?, Value::String(mark_attr)],
                    )
                    .await;
                Ok(Some(el))
            }
            Err(_) => Ok(None),
        }
    }

    // ========================================================================
    // Attribute helpers
    // ========================================================================

    /// Returns true if the element `css` has the attribute `attribute`.
    pub async fn is_attribute_present(
        &mut self,
        css: &str,
        attribute: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let script = format!(
            "var el = document.querySelector('{}'); return !!(el && el.hasAttribute('{}'));",
            js_escape(css),
            js_escape(attribute)
        );
        match self.execute_script(&script).await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }

    /// Waits up to `timeout_secs` for the attribute `attribute` to be present.
    pub async fn wait_for_attribute(
        &mut self,
        css: &str,
        attribute: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_attribute_present(css, attribute).await? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for attribute '{}' on '{}'",
                    attribute, css
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for the attribute `attribute` to be absent.
    pub async fn wait_for_attribute_not_present(
        &mut self,
        css: &str,
        attribute: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if !self.is_attribute_present(css, attribute).await? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for attribute '{}' to be absent on '{}'",
                    attribute, css
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    // ========================================================================
    // Scroll helpers
    // ========================================================================

    /// Scrolls the element `css` into view using `scrollIntoView()`.
    pub async fn scroll_into_view(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("scroll_into_view", Some(css), None);
        let script = format!(
            "document.querySelector('{}').scrollIntoView();",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `scroll_into_view`.
    pub async fn scroll_to_element(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.scroll_into_view(css).await
    }

    /// Scrolls to element `css` with smooth behavior.
    pub async fn slow_scroll_to(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("slow_scroll_to", Some(css), None);
        let script = format!(
            "document.querySelector('{}').scrollIntoView({{behavior: 'smooth'}});",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `slow_scroll_to`.
    pub async fn slow_scroll_to_element(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.slow_scroll_to(css).await
    }

    /// Scrolls the page to the vertical position `y`.
    pub async fn scroll_to_y(&self, y: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollTo(0, {y});"))
            .await?;
        Ok(())
    }

    /// Scrolls the page to the horizontal position `x`.
    pub async fn scroll_to_x(&self, x: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollTo({x}, 0);"))
            .await?;
        Ok(())
    }

    /// Scrolls the page to the position `(x, y)`.
    pub async fn scroll_to_xy(&self, x: i64, y: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollTo({x}, {y});"))
            .await?;
        Ok(())
    }

    /// Scrolls horizontally by `delta_x` pixels.
    pub async fn scroll_by_x(&self, delta_x: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollBy({delta_x}, 0);"))
            .await?;
        Ok(())
    }

    // ========================================================================
    // Select helpers
    // ========================================================================

    /// Selects the `<option>` at zero-based `index`.
    pub async fn select_option_by_index(
        &mut self,
        css: &str,
        index: usize,
    ) -> Result<(), SeleniumBaseError> {
        self.record("select_option_by_index", Some(css), Some(&index.to_string()));
        let script = format!(
            "var s = document.querySelector('{}'); if (s && s.options[{}]) s.selectedIndex = {};",
            js_escape(css),
            index,
            index
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Selects all `<option>` elements in the `<select>` at `css`.
    pub async fn select_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("select_all", Some(css), None);
        let script = format!(
            "var s = document.querySelector('{}'); if (s) {{ for (var i = 0; i < s.options.length; i++) s.options[i].selected = true; s.dispatchEvent(new Event('change')); }}",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `select_all`.
    pub async fn deselect_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.unselect_all(css).await
    }

    /// Unselects all `<option>` elements in the `<select>` at `css`.
    pub async fn unselect_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("unselect_all", Some(css), None);
        let script = format!(
            "var s = document.querySelector('{}'); if (s) {{ for (var i = 0; i < s.options.length; i++) s.options[i].selected = false; s.dispatchEvent(new Event('change')); }}",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Selects the `<option>` at `css` only if it is not already selected.
    pub async fn select_if_unselected(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        if !self.is_selected(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Unselects the `<option>` at `css` only if it is already selected.
    pub async fn unselect_if_selected(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        if self.is_selected(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    // ========================================================================
    // Window / tab helpers
    // ========================================================================

    /// Switches focus to the tab/window with index `index` (0-based).
    pub async fn switch_to_tab(&self, index: usize) -> Result<(), SeleniumBaseError> {
        let handles = self.get_all_window_handles().await?;
        let handle = handles.get(index).ok_or_else(|| {
            SeleniumBaseError::AssertionFailed(format!("No tab at index {}", index))
        })?;
        self.session.switch_to_window(handle).await
    }

    /// Switches focus to the first tab/window.
    pub async fn switch_to_default_tab(&self) -> Result<(), SeleniumBaseError> {
        self.switch_to_tab(0).await
    }

    /// Switches focus to the most recently opened tab/window.
    pub async fn switch_to_newest_tab(&self) -> Result<(), SeleniumBaseError> {
        let handles = self.get_all_window_handles().await?;
        let last = handles.last().ok_or_else(|| {
            SeleniumBaseError::AssertionFailed("No windows available".to_owned())
        })?;
        self.session.switch_to_window(last).await
    }

    /// Returns all open window/tab handles.
    pub async fn get_all_window_handles(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let handles = self
            .session
            .driver()
            .windows()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(handles.into_iter().map(|h| h.to_string()).collect())
    }

    /// Returns the handle of the currently focused window/tab.
    pub async fn get_current_window_handle(&self) -> Result<String, SeleniumBaseError> {
        let handle = self
            .session
            .driver()
            .window()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(handle.to_string())
    }

    /// Brings the current browser window to the front.
    pub async fn bring_to_front(&self) -> Result<(), SeleniumBaseError> {
        self.session
            .driver()
            .execute("window.focus();", vec![])
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    // ========================================================================
    // Frame helpers
    // ========================================================================

    /// Switches context into the frame located by `css`.
    pub async fn frame_switch(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::Css(css).to_by()?;
        self.session.switch_to_frame(by).await
    }

    /// Sets the frame context to the frame containing `css`.
    pub async fn switch_to_frame_of_element(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var el = document.querySelector('{}'); var frame = el ? el.closest('iframe, frame') : null; if (frame) {{ frame.setAttribute('data-sb-frame-target', '1'); return true; }} return false;",
            js_escape(css)
        );
        let found = self.execute_script(&script).await?;
        if found.as_bool() != Some(true) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Element '{}' is not inside a frame",
                css
            )));
        }
        self.session
            .switch_to_frame(By::Css("[data-sb-frame-target='1']"))
            .await?;
        let _ = self
            .session
            .driver()
            .execute("arguments[0].removeAttribute('data-sb-frame-target');", vec![])
            .await;
        Ok(())
    }

    /// Sets the current document body HTML to `html` inside a new frame.
    pub async fn set_content_to_frame(
        &mut self,
        html: &str,
    ) -> Result<(), SeleniumBaseError> {
        let escaped = js_escape(html);
        let script = format!(
            "var f = document.createElement('iframe'); f.srcdoc = '{}'; f.style.width = '100%'; f.style.height = '100%'; document.body.appendChild(f);",
            escaped
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    // ========================================================================
    // Console log capture (JS-based, no CDP required)
    // ========================================================================

    /// Starts capturing browser console logs in a global JS array.
    pub async fn start_recording_console_logs(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            r#"
            if (!window.__sbConsoleLogs) {
                window.__sbConsoleLogs = [];
                const orig = console.log;
                console.log = function(...args) {
                    window.__sbConsoleLogs.push(args.map(a => String(a)).join(' '));
                    orig.apply(console, args);
                };
            }
            return true;
            "#,
        )
        .await?;
        Ok(())
    }

    /// Stops capturing console logs.
    pub async fn stop_recording_console_logs(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            r#"
            window.__sbConsoleLogs = undefined;
            return true;
            "#,
        )
        .await?;
        Ok(())
    }

    /// Returns all console log lines recorded so far.
    pub async fn get_recorded_console_logs(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let result = self
            .execute_script("return window.__sbConsoleLogs || [];")
            .await?;
        match result {
            Value::Array(arr) => {
                let mut logs = Vec::new();
                for v in arr {
                    logs.push(v.as_str().unwrap_or("").to_owned());
                }
                Ok(logs)
            }
            _ => Ok(Vec::new()),
        }
    }

    // ========================================================================
    // MFA / TOTP helpers
    // ========================================================================

    /// Generates the current TOTP code for `secret`.
    pub fn get_totp_code(&self, secret: &str) -> Result<String, SeleniumBaseError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        generate_totp(secret, now)
    }

    /// Alias for `get_totp_code`.
    pub fn get_mfa_code(&self, secret: &str) -> Result<String, SeleniumBaseError> {
        self.get_totp_code(secret)
    }

    /// Alias for `get_totp_code` (Google Authenticator uses standard TOTP).
    pub fn get_google_auth_code(&self, secret: &str) -> Result<String, SeleniumBaseError> {
        self.get_totp_code(secret)
    }

    /// Generates a TOTP code for `secret` and types it into `css`.
    pub async fn enter_totp_code(
        &mut self,
        css: &str,
        secret: &str,
    ) -> Result<(), SeleniumBaseError> {
        let code = self.get_totp_code(secret)?;
        self.type_text(css, &code).await
    }

    /// Alias for `enter_totp_code`.
    pub async fn enter_mfa_code(
        &mut self,
        css: &str,
        secret: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.enter_totp_code(css, secret).await
    }

    // ========================================================================
    // Cookie / saved session helpers
    // ========================================================================

    /// Adds multiple cookies from a map of name/value pairs.
    pub async fn add_cookies(
        &mut self,
        cookies: &HashMap<String, String>,
    ) -> Result<(), SeleniumBaseError> {
        for (name, value) in cookies {
            self.session.add_cookie(name, value).await?;
        }
        Ok(())
    }

    /// Saves all current cookies to a JSON file in the logs directory.
    pub async fn save_saved_cookies(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let cookies = self.session.get_cookies().await?;
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        fs::write(&path, serde_json::to_string_pretty(&cookies)?)?;
        Ok(path)
    }

    /// Loads cookies from a JSON file and adds them to the browser.
    pub async fn load_saved_cookies(
        &mut self,
        filename: &str,
    ) -> Result<(), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        let data = fs::read_to_string(&path)?;
        let cookies: Vec<Value> = serde_json::from_str(&data)?;
        for cookie in cookies {
            if let (Some(name), Some(value)) = (
                cookie["name"].as_str(),
                cookie["value"].as_str(),
            ) {
                self.session.add_cookie(name, value).await?;
            }
        }
        Ok(())
    }

    /// Deletes the saved cookies file from the logs directory.
    pub async fn delete_saved_cookies(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    // ========================================================================
    // Download helpers
    // ========================================================================

    /// Returns the list of files in the browser's default download folder.
    pub fn get_downloaded_files(
        &self,
    ) -> Result<Vec<PathBuf>, SeleniumBaseError> {
        let dir = default_download_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files.push(entry.path());
            }
        }
        Ok(files)
    }

    /// Asserts that `filename` exists in the downloads folder.
    pub fn assert_downloaded_file(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = default_download_dir().join(filename);
        if !path.exists() {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Downloaded file '{}' was not found",
                path.display()
            )));
        }
        Ok(())
    }

    /// Deletes `filename` from the downloads folder.
    pub fn delete_downloaded_file(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = default_download_dir().join(filename);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Downloads `url` by navigating to it in a new tab and waiting for the file.
    pub async fn download_file(
        &mut self,
        url: &str,
        filename: &str,
        timeout_secs: u64,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let original = self.get_current_window_handle().await?;
        self.session.switch_to_new_window().await?;
        self.open(url).await?;
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        let expected = default_download_dir().join(filename);
        loop {
            if expected.exists() {
                self.session.switch_to_window(&original).await?;
                return Ok(expected);
            }
            if std::time::Instant::now() >= deadline {
                let _ = self.session.switch_to_window(&original).await;
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for download '{}'",
                    filename
                )));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    // ========================================================================
    // jQuery-style and JS helpers
    // ========================================================================

    /// Clicks every element matching `css` via JavaScript.
    pub async fn jquery_click_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("jquery_click_all", Some(css), None);
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => el.click());",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Sets the value of every element matching `css` to `text`.
    pub async fn jquery_type(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("jquery_type", Some(css), Some(text));
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => {{ el.value = '{}'; el.dispatchEvent(new Event('input')); el.dispatchEvent(new Event('change')); }});",
            js_escape(css),
            js_escape(text)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Updates the visible text of every element matching `css`.
    pub async fn jquery_update_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("jquery_update_text", Some(css), Some(text));
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => el.textContent = '{}');",
            js_escape(css),
            js_escape(text)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Updates the value of every element matching `css` to `text`.
    pub async fn js_update_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("js_update_text", Some(css), Some(text));
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => {{ if ('value' in el) el.value = '{}'; el.dispatchEvent(new Event('input')); el.dispatchEvent(new Event('change')); }});",
            js_escape(css),
            js_escape(text)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `execute_script`.
    pub async fn evaluate(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.execute_script(script).await
    }

    /// Executes JavaScript and returns a default value on error.
    pub async fn safe_execute_script(
        &self,
        script: &str,
        default: Value,
    ) -> Result<Value, SeleniumBaseError> {
        match self.execute_script(script).await {
            Ok(v) => Ok(v),
            Err(_) => Ok(default),
        }
    }

    /// Converts all links on the page to use the current origin.
    pub async fn internalize_links(&mut self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            r#"
            document.querySelectorAll('a').forEach(a => {
                if (a.href && a.href.startsWith('http') && !a.href.startsWith(window.location.origin)) {
                    a.href = window.location.origin + '/external?url=' + encodeURIComponent(a.href);
                }
            });
            return true;
            "#,
        )
        .await?;
        Ok(())
    }

    // ========================================================================
    // Interaction with offsets
    // ========================================================================

    /// Clicks `css` at offset `(x, y)` relative to the element's top-left corner.
    pub async fn click_with_offset(
        &mut self,
        css: &str,
        x: i64,
        y: i64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("click_with_offset", Some(css), Some(&format!("{}, {}", x, y)));
        let by = Selector::Css(css).to_by()?;
        let element = self.session.find(by).await?;
        self.session
            .driver()
            .action_chain()
            .move_to_element_with_offset(&element, x, y)
            .click()
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Double-clicks `css` at offset `(x, y)`.
    pub async fn double_click_with_offset(
        &mut self,
        css: &str,
        x: i64,
        y: i64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("double_click_with_offset", Some(css), Some(&format!("{}, {}", x, y)));
        let by = Selector::Css(css).to_by()?;
        let element = self.session.find(by).await?;
        self.session
            .driver()
            .action_chain()
            .move_to_element_with_offset(&element, x, y)
            .double_click()
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Drags `source_css` to `target_css` with an optional target offset.
    pub async fn drag_and_drop_with_offset(
        &mut self,
        source_css: &str,
        target_css: &str,
        x: i64,
        y: i64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("drag_and_drop_with_offset", Some(source_css), Some(target_css));
        let source_by = Selector::Css(source_css).to_by()?;
        let target_by = Selector::Css(target_css).to_by()?;
        let _source = self.session.find(source_by).await?;
        let target = self.session.find(target_by).await?;
        self.session
            .driver()
            .action_chain()
            .drag_and_drop_element_by_offset(&target, x, y)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Clicks a sequence of elements in order.
    pub async fn click_chain(&mut self, css_list: &[&str]) -> Result<(), SeleniumBaseError> {
        let mut chain = self.session.driver().action_chain();
        for css in css_list {
            let by = Selector::Css(css).to_by()?;
            let element = self.session.find(by).await?;
            chain = chain.move_to_element_center(&element).click();
        }
        chain.perform().await.map_err(SeleniumBaseError::WebDriver)
    }

    // ========================================================================
    // Wait helpers (additional variants)
    // ========================================================================

    /// Waits up to `timeout_secs` for any element from `css_list` to be present.
    pub async fn wait_for_any_of_elements_present(
        &self,
        css_list: &[&str],
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            for css in css_list {
                if let Ok(by) = Selector::Css(css).to_by() {
                    if let Ok(element) = self.session.driver().find(by).await {
                        return Ok(element);
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(
                    "Timed out waiting for any of the elements".to_owned(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for a link with exact `text`.
    pub async fn wait_for_link_text(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if let Ok(element) = self.session.driver().find(By::LinkText(text)).await {
                return Ok(element);
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for link text '{}'",
                    text
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for a link containing `text`.
    pub async fn wait_for_partial_link_text(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if let Ok(element) = self.session.driver().find(By::PartialLinkText(text)).await {
                return Ok(element);
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for partial link text '{}'",
                    text
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for an element whose text exactly matches `text`.
    pub async fn wait_for_exact_text(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        let script = format!(
            "return Array.from(document.querySelectorAll('*')).some(el => el.textContent.trim() === '{}');",
            js_escape(text)
        );
        loop {
            let result = self.execute_script(&script).await?;
            if result.as_bool() == Some(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for exact text '{}'",
                    text
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for the page title to contain `substring`.
    pub async fn wait_for_title(
        &mut self,
        substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if let Ok(title) = self.session.current_title().await {
                if title.contains(substring) {
                    return Ok(());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "Timed out waiting for title containing '{}'",
                    substring
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    // ========================================================================
    // Demo / design mode
    // ========================================================================

    /// Activates demo mode: highlights clicks and typed text.
    pub async fn activate_demo_mode(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            r#"
            document.body.addEventListener('click', function(e) {
                var d = document.createElement('div');
                d.style.cssText = 'position:fixed;left:' + e.clientX + 'px;top:' + e.clientY + 'px;width:20px;height:20px;border-radius:50%;background:rgba(255,0,0,0.5);pointer-events:none;z-index:999999;';
                document.body.appendChild(d);
                setTimeout(() => d.remove(), 800);
            }, true);
            return true;
            "#,
        )
        .await?;
        Ok(())
    }

    /// Deactivates demo mode by reloading the page (clears event listeners).
    pub async fn deactivate_demo_mode(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    /// Activates design mode, making the page content editable.
    pub async fn activate_design_mode(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("document.designMode = 'on'; return true;")
            .await?;
        Ok(())
    }

    /// Deactivates design mode.
    pub async fn deactivate_design_mode(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("document.designMode = 'off'; return true;")
            .await?;
        Ok(())
    }

    // ========================================================================
    // Data / logging helpers
    // ========================================================================

    /// Appends `data` to `filename` in the logs directory.
    pub fn append_data_to_file(
        &self,
        data: &str,
        filename: &str,
    ) -> Result<(), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", data)?;
        Ok(())
    }

    /// Appends `data` to `logs/sb_<timestamp>.log`.
    pub fn append_data_to_logs(&self, data: &str) -> Result<(), SeleniumBaseError> {
        let filename = format!(
            "sb_{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        self.append_data_to_file(data, &filename)
    }

    /// Saves `data` to `filename` in the logs directory (overwrites).
    pub fn save_data_as(&self, data: &str, filename: &str) -> Result<(), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = dir.join(filename);
        fs::write(&path, data)?;
        Ok(())
    }

    /// Saves `data` to a timestamped log file.
    pub fn save_data_to_logs(&self, data: &str) -> Result<(), SeleniumBaseError> {
        let filename = format!(
            "sb_{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        self.save_data_as(data, &filename)
    }

    /// Returns the path to the latest logs directory.
    pub fn get_log_path(&self) -> Result<PathBuf, SeleniumBaseError> {
        ensure_latest_logs_dir()
    }

    /// Returns the browser download folder path.
    pub fn get_browser_downloads_folder(&self) -> PathBuf {
        default_download_dir()
    }

    /// Returns the configured browser type.
    pub fn get_browser(&self) -> BrowserConfig {
        self.config.clone()
    }

    // ========================================================================
    // More aliases
    // ========================================================================

    /// Alias for `context_click`.
    pub async fn contextmenu_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.context_click(css).await
    }

    /// Alias for `double_click`.
    pub async fn dblclick(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.double_click(css).await
    }

    /// Alias for `refresh`.
    pub async fn refresh_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    /// Alias for `get_current_url`.
    pub async fn get_url(&mut self) -> Result<String, SeleniumBaseError> {
        self.get_current_url().await
    }

    /// Alias for `get_title`.
    pub async fn get_page_title(&mut self) -> Result<String, SeleniumBaseError> {
        self.get_title().await
    }

    /// Alias for `get_text`.
    pub async fn get_text_of_element(
        &mut self,
        css: &str,
    ) -> Result<String, SeleniumBaseError> {
        self.get_text(css).await
    }
}

#[cfg(test)]
mod tests_extra {
    use super::*;

    #[test]
    fn js_escape_basic() {
        assert_eq!(js_escape("hello"), "hello");
        assert_eq!(js_escape("it's"), "it\\'s");
        assert_eq!(js_escape("a\nb"), "a\\nb");
    }

    #[test]
    fn base32_decode_hello() {
        // "Hello!" encoded with standard base32 padding.
        let decoded = base32_decode("JBSWY3DPEE======").unwrap();
        assert_eq!(decoded, b"Hello!");
    }

    #[test]
    fn totp_generates_six_digit_code() {
        let code = generate_totp("JBSWY3DPEHPK3PXP", 0).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn totp_is_deterministic() {
        let code1 = generate_totp("JBSWY3DPEHPK3PXP", 1234567).unwrap();
        let code2 = generate_totp("JBSWY3DPEHPK3PXP", 1234567).unwrap();
        assert_eq!(code1, code2);
    }
}
