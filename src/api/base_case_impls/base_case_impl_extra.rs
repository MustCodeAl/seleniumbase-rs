
// Additional BaseCase methods to close the parity gap with Python SeleniumBase.
// Included at the end of `base_case.rs` so the code lives in the same module and
// can access private fields and helpers.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use thirtyfour::WebElement;

use crate::api::misc_calls::{
    ClickWithOffset, DownloadFile, DragByOffset, OffsetOptions, WaitForAttribute,
};

/// Escape a string for safe use inside a single-quoted JavaScript literal,
/// without the quotes. Only [`BaseCase::jq_format`] needs this shape; a page
/// script puts a caller's text in with [`crate::js_code::quote`], which
/// builds the whole literal.
fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// The offset to give WebDriver for a click at `(x, y)` of an element `size`
/// (width, height) big. WebDriver counts from the centre of the element;
/// Python's `click_with_offset` counts from its top-left corner unless
/// `center` is set (`x = -width / 2 + x`), and the driver then truncates the
/// number to a whole one.
fn offset_from_center((x, y): (i64, i64), (width, height): (f64, f64), center: bool) -> (i64, i64) {
    if center {
        return (x, y);
    }
    (
        (x as f64 - width / 2.0) as i64,
        (y as f64 - height / 2.0) as i64,
    )
}

/// A click offset in whole pixels, as Python gives it to the driver: rounded up
/// when it counts from the top-left corner (`math.ceil`), and cut to a whole
/// number by the driver when it counts from the centre.
fn whole_pixels(offset: f64, center: bool) -> i64 {
    if center {
        offset as i64
    } else {
        offset.ceil() as i64
    }
}

/// How long [`fetch_bytes`] waits for a server, as Python's download does
/// (`timeout=5`) but for the whole exchange and with more patience.
const FETCH_BYTES_TIMEOUT: Duration = Duration::from_secs(30);

/// The body of `url`, fetched with the user agent `agent`; redirects are
/// followed.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Network`] if the request fails or the answer
/// is not a success status.
async fn fetch_bytes(url: &str, agent: &str) -> Result<Vec<u8>, SeleniumBaseError> {
    let failed = |e: reqwest::Error| SeleniumBaseError::network(url, 0, e.to_string());
    crate::install_tls_provider();
    let client = reqwest::Client::builder()
        .timeout(FETCH_BYTES_TIMEOUT)
        .user_agent(agent)
        .build()
        .map_err(failed)?;
    let response = client.get(url).send().await.map_err(failed)?;
    let status = response.status();
    if !status.is_success() {
        return Err(SeleniumBaseError::network(url, status.as_u16(), status.to_string()));
    }
    Ok(response.bytes().await.map_err(failed)?.to_vec())
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

/// The folder downloads are saved in: `./downloaded_files`, as in Python.
fn default_download_dir() -> PathBuf {
    crate::browser::downloads::downloads_dir()
}

/// Waits up to `timeout_secs` for the file at `path` to exist.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::WaitTimeout`] if it is still missing then.
async fn wait_for_file(path: &Path, timeout_secs: u64) -> Result<(), SeleniumBaseError> {
    poll_until_true(
        timeout_secs,
        format!("File '{}' was not found", path.display()),
        || async { Ok(path.exists()) },
    )
    .await
}

/// Turns what [`page_scripts::select_by_index_script`] returned into a result.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::AssertionFailed`] naming `css` and `index`
/// for anything but `"ok"`.
fn select_index_outcome(status: &Value, css: &str, index: usize) -> Result<(), SeleniumBaseError> {
    let problem = match status.as_str() {
        Some("ok") => return Ok(()),
        Some("no-option") => format!("'{css}' has no option at index {index}"),
        Some("no-select") => format!("'{css}' is not a <select> element"),
        Some("disabled") => format!("option {index} of '{css}' is disabled"),
        _ => format!("option {index} of '{css}' could not be selected"),
    };
    Err(SeleniumBaseError::AssertionFailed(problem))
}

/// `text` without one trailing newline, and whether it had one: Python's
/// `press_keys` presses Enter for a trailing `\n` instead of typing it.
fn split_trailing_enter(text: &str) -> (&str, bool) {
    match text.strip_suffix('\n') {
        Some(body) => (body, true),
        None => (text, false),
    }
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

    /// Asserts that the fallible closure `f` returns an error.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seleniumbase_rs::{BaseCase, SeleniumBaseError};
    ///
    /// fn example(sb: &BaseCase) -> Result<(), SeleniumBaseError> {
    ///     sb.assert_raises(|| Err::<i32, _>("expected failure"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn assert_raises<T, E, F>(&self, f: F) -> Result<(), SeleniumBaseError>
    where
        F: FnOnce() -> Result<T, E>,
        E: std::fmt::Debug,
    {
        match f() {
            Ok(_) => Err(SeleniumBaseError::AssertionFailed(
                "Expected an error, but the operation succeeded".to_owned(),
            )),
            Err(e) => {
                debug!(error = ?e, "assert_raises caught expected error");
                Ok(())
            }
        }
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
    ///
    /// `type_name` is looked for in the full name of the value's type, so
    /// `"String"` matches `alloc::string::String`. Pass the value itself
    /// (`&count`): a `&dyn Any` has already lost its type.
    pub fn assert_is_instance<T: ?Sized>(&self, value: &T, type_name: &str) -> Result<(), SeleniumBaseError> {
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
    ///
    /// The name is matched as in [`assert_is_instance`](Self::assert_is_instance).
    pub fn assert_not_is_instance<T: ?Sized>(
        &self,
        value: &T,
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
        let message = demo::braced_message("ASSERT PARTIAL LINK TEXT", text);
        self.demo_assert_success(&message, Some(&format!("partial_link={text}")))
            .await;
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

    /// Waits up to the default timeout ([`set_timeout`](Self::set_timeout))
    /// for the element `css` to be displayed with exactly `text` as its text
    /// (ignoring whitespace at the ends), and returns it. The text comes first
    /// and the selector second, as in Python's `find_exact_text(text,
    /// selector)`; pass `"body"` to look at the whole page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not show
    /// that text when the timeout ends.
    pub async fn find_exact_text(
        &self,
        text: &str,
        css: &str,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.find_with_visible_text(TextMatch::Exact, text, css)
            .await
    }

    /// Waits up to the default timeout ([`set_timeout`](Self::set_timeout))
    /// for the element `css` to be displayed with text that contains `text`,
    /// and returns it. Text that is hidden (`display: none`) does not count.
    /// The text comes first and the selector second, as in Python's
    /// `find_text(text, selector)`; pass `"body"` to look at the whole page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not show
    /// that text when the timeout ends.
    pub async fn find_text(&self, text: &str, css: &str) -> Result<WebElement, SeleniumBaseError> {
        self.find_with_visible_text(TextMatch::Contains, text, css)
            .await
    }

    /// Waits up to the default timeout ([`set_timeout`](Self::set_timeout))
    /// for the element `css` to be displayed with text that is not blank, and
    /// returns it. Corresponds to Python's `find_non_empty_text(selector)`;
    /// pass `"body"` to look at the whole page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element shows no text
    /// when the timeout ends.
    pub async fn find_non_empty_text(&self, css: &str) -> Result<WebElement, SeleniumBaseError> {
        self.find_with_visible_text(TextMatch::NonEmpty, "", css)
            .await
    }

    /// Polls, for up to the default timeout, until the element `css` is
    /// displayed with text that `kind` finds in `text`, and returns it.
    async fn find_with_visible_text(
        &self,
        kind: TextMatch,
        text: &str,
        css: &str,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.check_time_limit()?;
        let by = Selector::auto(css).to_by()?;
        let what = match kind {
            TextMatch::Contains => format!("Text '{text}' was not visible in '{css}'"),
            TextMatch::Exact => format!("Exact text '{text}' was not visible in '{css}'"),
            TextMatch::NonEmpty => format!("Non-empty text was not visible in '{css}'"),
        };
        poll_until_found(self.default_wait_secs(), what, || {
            let by = by.clone();
            async move {
                let Ok(element) = self.session.webdriver()?.find(by).await else {
                    return Ok(Polled::Pending { seen: None });
                };
                if !element.is_displayed().await.unwrap_or(false) {
                    return Ok(Polled::Pending { seen: None });
                }
                let shown = element.text().await.unwrap_or_default();
                Ok(if kind.holds(&shown, text) {
                    Polled::Found(element)
                } else {
                    Polled::Pending {
                        seen: Some(format!("last text: '{shown}'")),
                    }
                })
            }
        })
        .await
    }

    /// Returns all currently visible elements matching `css`.
    pub async fn find_visible_elements(
        &mut self,
        css: &str,
    ) -> Result<Vec<WebElement>, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
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
        self.session.webdriver()?
            .execute(&script, vec![result.clone()])
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        let by = By::Css(format!("[{}='1']", mark_attr));
        match self.session.find(by).await {
            Ok(el) => {
                // Remove the marker attribute.
                let _ = self.session.webdriver()?
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
        &self,
        css: &str,
        attribute: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let script = format!(
            "var el = document.querySelector({}); return !!(el && el.hasAttribute({}));",
            quote(css),
            quote(attribute)
        );
        match self.execute_script(&script).await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }

    /// Waits for the element `selector` picks to have the attribute `attribute`,
    /// as Python's `wait_for_attribute(selector, attribute, value=None, by,
    /// timeout=None)` does. The keywords are [`WaitForAttribute::value`] (the
    /// value it must have; without one the attribute only has to exist) and
    /// [`WaitForAttribute::timeout`] (default: the case's timeout).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the attribute and the
    /// selector if it is still absent after the timeout.
    pub fn wait_for_attribute<'a>(
        &'a mut self,
        selector: &'a str,
        attribute: &'a str,
    ) -> WaitForAttribute<'a> {
        WaitForAttribute::new(self, selector, attribute, false)
    }
    
    /// The work of [`wait_for_attribute`](Self::wait_for_attribute) and
    /// [`wait_for_attribute_not_present`](Self::wait_for_attribute_not_present).
    pub(crate) async fn run_wait_for_attribute(
        &self,
        css: &str,
        attribute: &str,
        value: Option<&str>,
        timeout: Option<Duration>,
        absent: bool,
    ) -> Result<(), SeleniumBaseError> {
        let seconds = self.wait_secs(timeout);
        let with = value.map_or_else(String::new, |value| format!(" with the value '{value}'"));
        let failure = if absent {
            format!("Attribute '{attribute}'{with} was still present on '{css}'")
        } else {
            format!("Attribute '{attribute}'{with} was not present on '{css}'")
        };
        poll_until_true(seconds, failure, || async {
            Ok(self.attribute_matches(css, attribute, value).await? != absent)
        })
        .await
    }
    
    /// Whether the element `css` picks has the attribute `attribute` and, if
    /// `value` is given, that value of it.
    async fn attribute_matches(
        &self,
        css: &str,
        attribute: &str,
        value: Option<&str>,
    ) -> Result<bool, SeleniumBaseError> {
        let wanted = value.map_or_else(|| "null".to_owned(), quote);
        let script = format!(
            "var el = document.querySelector({}); \
             if (!el || !el.hasAttribute({})) {{ return false; }} \
             var wanted = {wanted}; \
             return wanted === null || el.getAttribute({}) === wanted;",
            quote(css),
            quote(attribute),
            quote(attribute)
        );
        match self.execute_script(&script).await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }

    /// Waits until the element `selector` picks no longer has the attribute
    /// `attribute` (with the value [`WaitForAttribute::value`] names, if one is
    /// given), as Python's `wait_for_attribute_not_present(selector, attribute,
    /// value=None, by, timeout=None)` does. The keywords are those of
    /// [`wait_for_attribute`](Self::wait_for_attribute).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the attribute and the
    /// selector if it is still present after the timeout.
    pub fn wait_for_attribute_not_present<'a>(
        &'a mut self,
        selector: &'a str,
        attribute: &'a str,
    ) -> WaitForAttribute<'a> {
        WaitForAttribute::new(self, selector, attribute, true)
    }

    // ========================================================================
    // Scroll helpers
    // ========================================================================

    /// Scrolls the element `css` into view using `scrollIntoView()`.
    pub async fn scroll_into_view(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("scroll_into_view", Some(css), None);
        let script = format!(
            "document.querySelector({}).scrollIntoView();",
            quote(css)
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
            "document.querySelector({}).scrollIntoView({{behavior: 'smooth'}});",
            quote(css)
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

    /// Selects the `<option>` at zero-based `index` of the `<select>` `css`,
    /// and fires `input` and `change` so the page reacts as it would to a
    /// user's choice. Corresponds to Python's `select_option_by_index`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if `css` is not a
    /// `<select>`, has no option at `index`, or that option is disabled; the
    /// selection is left as it was.
    pub async fn select_option_by_index(
        &mut self,
        css: &str,
        index: usize,
    ) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Present).await?;
        self.record("select_option_by_index", Some(css), Some(&index.to_string()));
        let status = self
            .execute_script(&page_scripts::select_by_index_script(css, index))
            .await?;
        select_index_outcome(&status, css, index)
    }

    /// Returns all elements matching `css`, as Python's `select_all` does:
    /// there it is another name for `find_elements`. To select every option
    /// of a multi-select, use
    /// [`select_all_options`](Self::select_all_options).
    pub async fn select_all(
        &mut self,
        css: &str,
    ) -> Result<Vec<WebElement>, SeleniumBaseError> {
        self.find_elements(css).await
    }

    /// Selects all `<option>` elements in the `<select>` at `css`.
    pub async fn select_all_options(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("select_all_options", Some(css), None);
        let script = format!(
            "var s = document.querySelector({}); if (s) {{ for (var i = 0; i < s.options.length; i++) s.options[i].selected = true; s.dispatchEvent(new Event('change')); }}",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `unselect_all`.
    pub async fn deselect_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.unselect_all(css).await
    }

    /// Unselects all `<option>` elements in the `<select>` at `css`.
    pub async fn unselect_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("unselect_all", Some(css), None);
        let script = format!(
            "var s = document.querySelector({}); if (s) {{ for (var i = 0; i < s.options.length; i++) s.options[i].selected = false; s.dispatchEvent(new Event('change')); }}",
            quote(css)
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
        let handles = self.session.webdriver()?
            .windows()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(handles.into_iter().map(|h| h.to_string()).collect())
    }

    /// Returns the handle of the currently focused window/tab.
    pub async fn get_current_window_handle(&self) -> Result<String, SeleniumBaseError> {
        let handle = self.session.webdriver()?
            .window()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(handle.to_string())
    }

    /// Raises the z-index of the element `css` to 999999 once it is visible.
    /// Use it when a click fails with "Other element would receive the
    /// click". Corresponds to Python's `bring_to_front(selector)`. To bring
    /// the browser window forward, see
    /// [`bring_active_window_to_front`](Self::bring_active_window_to_front).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not
    /// become visible.
    pub async fn bring_to_front(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Visible).await?;
        let found = self
            .execute_script(&page_scripts::bring_to_front_script(css))
            .await?;
        if found.as_bool() != Some(true) {
            return Err(SeleniumBaseError::element_not_found(css));
        }
        Ok(())
    }

    /// Focuses the current browser window.
    pub(crate) async fn focus_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.webdriver()?
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
        let by = Selector::auto(css).to_by()?;
        self.session.switch_to_frame(by).await
    }

    /// Switches into the `<iframe>` or `<frame>` that holds the element `css`
    /// and returns the frame's `name` (or its `id`, or an empty string if it
    /// has neither). Returns `None`, and switches nothing, when the element is
    /// in the current document. Corresponds to Python's
    /// `switch_to_frame_of_element`.
    ///
    /// Like Python's, it looks one level down: an element inside frames
    /// nested in each other is not found. Frames from other origins are
    /// searched too, because the browser driver can enter them.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if no frame of the
    /// current document holds the element; the frame context is then as it
    /// was.
    pub async fn switch_to_frame_of_element(
        &mut self,
        css: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        if self.is_element_present(css).await? {
            return Ok(None);
        }
        let frames = self.session.find_all(By::Css("iframe, frame")).await?;
        for frame in frames {
            let name = frame.attr("name").await?.filter(|name| !name.is_empty());
            let identifier = match name {
                Some(name) => name,
                None => frame.attr("id").await?.unwrap_or_default(),
            };
            if frame.enter_frame().await.is_err() {
                continue;
            }
            if self.is_element_present(css).await.unwrap_or(false) {
                return Ok(Some(identifier));
            }
            self.switch_to_parent_frame().await?;
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "Could not switch to an iframe containing element '{css}'"
        )))
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

    /// Alias for `get_totp_code`: Python's `get_google_auth_password(totp_key)`
    /// is the same call as its `get_mfa_code`.
    pub fn get_google_auth_password(&self, totp_key: &str) -> Result<String, SeleniumBaseError> {
        self.get_mfa_code(totp_key)
    }


    /// Generates a TOTP code for `secret`, types it into `css` and presses
    /// Enter, as Python's `enter_mfa_code(selector, totp_key)` does (it types
    /// the code followed by a newline).
    pub async fn enter_totp_code(
        &mut self,
        css: &str,
        secret: &str,
    ) -> Result<(), SeleniumBaseError> {
        let code = self.get_totp_code(secret)?;
        self.type_text(css, &format!("{code}\n")).await
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

    /// Asserts that `filename` exists in the downloads folder, waiting up to
    /// the default timeout ([`set_timeout`](Self::set_timeout)) for a download
    /// that is still being written. Corresponds to Python's
    /// `assert_downloaded_file`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if the file is not there
    /// when the timeout ends.
    pub async fn assert_downloaded_file(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = crate::artifacts::confined_path(&default_download_dir(), filename)?;
        let timeout = self.default_wait_secs();
        wait_for_file(&path, timeout).await.map_err(|_| {
            SeleniumBaseError::AssertionFailed(format!(
                "Downloaded file '{}' was not found after {timeout}s",
                path.display()
            ))
        })?;
        let message = demo::bracketed_message("ASSERT DOWNLOADED FILE", filename);
        self.demo_assert_success(&message, None).await;
        Ok(())
    }

    /// Deletes `filename` from the downloads folder.
    pub fn delete_downloaded_file(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = crate::artifacts::confined_path(&default_download_dir(), filename)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Downloads `url` by navigating to it in a new tab and waiting for the
    /// file, then returns to the tab it started in.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the file if it has not
    /// arrived after the timeout; the first tab is current again either way.
    pub async fn download_file_in_browser(
        &mut self,
        url: &str,
        filename: &str,
        timeout_secs: u64,
    ) -> Result<PathBuf, SeleniumBaseError> { let original = self.get_current_window_handle().await?;
    self.session.switch_to_new_window().await?;
    self.open(url).await?;
    let expected = crate::artifacts::confined_path(&default_download_dir(), filename)?;
    let waited = wait_for_file(&expected, self.effective_timeout(timeout_secs)).await;
    let returned = self.session.switch_to_window(&original).await;
    waited?;
    returned?;
    Ok(expected) }
    
    /// Fetches `file_url` with the browser's user agent and saves the body in the
    /// downloads folder (`./downloaded_files`), under the last part of the URL's
    /// path, as Python's `download_file(file_url, destination_folder=None)` does.
    /// Returns the path of the file. The keyword is
    /// [`DownloadFile::destination_folder`].
    ///
    /// This is a plain HTTP request that sends no cookies of the page. To have
    /// the browser itself download a file, use
    /// [`download_file_in_browser`](Self::download_file_in_browser).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::api::base_case::BaseCase;
    /// # async fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let path = sb.download_file("https://example.com/report.csv").await?;
    /// let copy = sb
    ///     .download_file("https://example.com/report.csv")
    ///     .destination_folder("out")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Network`] if the request fails or the answer
    /// is not a success status, and an error if the file name is not a plain one
    /// or the file cannot be written.
    pub fn download_file<'a>(&'a self, file_url: &'a str) -> DownloadFile<'a> {
        DownloadFile::new(self, file_url)
    }
    
    /// The work of [`download_file`](Self::download_file) and
    /// [`save_file_as`](Self::save_file_as): the file is saved as `new_name`, or
    /// under the last part of the URL's path.
    pub(crate) async fn run_download(
        &self,
        file_url: &str,
        new_name: Option<&str>,
        folder: Option<&Path>,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let name = match new_name {
            Some(name) => name.to_owned(),
            None => crate::cli::commands::download_file_name(file_url),
        };
        let folder = folder.map_or_else(default_download_dir, Path::to_path_buf);
        let path = crate::artifacts::confined_path(&folder, &name)?;
        let agent = self.get_user_agent().await?;
        let body = fetch_bytes(file_url, &agent).await?;
        fs::create_dir_all(&folder)?;
        fs::write(&path, body)?;
        Ok(path)
    }

    // ========================================================================
    // jQuery-style and JS helpers
    // ========================================================================

    /// Clicks every element matching `css` via JavaScript.
    pub async fn jquery_click_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("jquery_click_all", Some(css), None);
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.click());",
            quote(css)
        );
        self.execute_script(&script).await?;
        self.demo_pause(PauseKind::Full).await;
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
            "document.querySelectorAll({}).forEach(el => {{ el.value = {}; el.dispatchEvent(new Event('input')); el.dispatchEvent(new Event('change')); }});",
            quote(css),
            quote(text)
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
            "document.querySelectorAll({}).forEach(el => el.textContent = {});",
            quote(css),
            quote(text)
        );
        self.execute_script(&script).await?;
        self.demo_pause(PauseKind::Full).await;
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
            "document.querySelectorAll({}).forEach(el => {{ if ('value' in el) el.value = {}; el.dispatchEvent(new Event('input')); el.dispatchEvent(new Event('change')); }});",
            quote(css),
            quote(text)
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

    /// Clicks `selector` at the offset `(x, y)` in pixels, as Python's
    /// `click_with_offset(selector, x, y, by, mark=None, timeout=None,
    /// center=None, scroll=True)` does: `(0, 0)` is the
    /// top-left corner of the element, or with [`ClickWithOffset::center`] its
    /// centre. The other keywords are [`ClickWithOffset::mark`],
    /// [`ClickWithOffset::timeout`] and [`ClickWithOffset::scroll`]. The offset
    /// may be fractional; it is rounded up to whole pixels. Waits for
    /// the element to be displayed.
    ///
    /// WebDriver measures an offset from the centre of the element. This
    /// method used to pass `(x, y)` to it as it was while its documentation said
    /// "from the top-left corner", so every click was off by half the element's
    /// size; it now converts, as Python does.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not show
    /// up and the WebDriver error if the offset is outside the page.
    pub fn click_with_offset<'a>(
        &'a mut self,
        selector: &'a str,
        x: impl Into<f64>,
        y: impl Into<f64>,
    ) -> ClickWithOffset<'a> {
        ClickWithOffset::new(self, selector, (x.into(), y.into()), false)
    }

    /// Double-clicks `selector` at the offset `(x, y)`; see
    /// [`click_with_offset`](Self::click_with_offset) for what `(0, 0)` is.
    pub fn double_click_with_offset<'a>(
        &'a mut self,
        selector: &'a str,
        x: impl Into<f64>,
        y: impl Into<f64>,
    ) -> ClickWithOffset<'a> {
        ClickWithOffset::new(self, selector, (x.into(), y.into()), true)
    }
    
    /// The work of [`click_with_offset`](Self::click_with_offset) and
    /// [`double_click_with_offset`](Self::double_click_with_offset).
    pub(crate) async fn run_click_with_offset(
        &mut self,
        css: &str,
        offset: (f64, f64),
        double: bool,
        options: OffsetOptions,
    ) -> Result<(), SeleniumBaseError> {
        let action = if double {
            "double_click_with_offset"
        } else {
            "click_with_offset"
        };
        self.click_at_offset(action, css, offset, double, options).await
    }

    /// The click behind [`click_with_offset`](Self::click_with_offset) and
    /// [`double_click_with_offset`](Self::double_click_with_offset). It records
    /// `action` with the offset as `x, y`, or `x, y, center` for an offset from
    /// the centre, which is what the recorder's generators read.
    async fn click_at_offset(
        &mut self,
        action: &str,
        css: &str,
        (x, y): (f64, f64),
        double: bool,
        options: OffsetOptions,
    ) -> Result<(), SeleniumBaseError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the offset must be numbers, got ({x}, {y})"
            )));
        }
        let by = Selector::auto(css).to_by()?;
        let seconds = self.wait_secs(options.timeout);
        self.await_ready_within(css, Ready::Visible, seconds).await?;
        let center = options.center.unwrap_or(false);
        let (x, y) = (whole_pixels(x, center), whole_pixels(y, center));
        let offset = if center {
            format!("{x}, {y}, center")
        } else {
            format!("{x}, {y}")
        };
        self.record(action, Some(css), Some(&offset));
        self.demo_highlight(css).await?;
        let element = self.session.find(by).await?;
        if options.scroll {
            // Python scrolls first: the offset is measured from the centre of the
            // part of the element in view, so an element below the fold or taller
            // than the window must be where the click lands before it is measured.
            element.scroll_into_view().await?;
        }
        let size = element.rect().await?;
        if options
            .mark
            .unwrap_or_else(|| self.run_settings.demo_mode())
        {
            // Python's dot: two squares drawn on the 2D context of a canvas.
            let (dot_x, dot_y) = if center {
                (
                    size.width / 2.0 + x as f64 - 3.0,
                    size.height / 2.0 + y as f64 - 3.0,
                )
            } else {
                (x as f64 - 3.0, y as f64 - 3.0)
            };
            let script = format!(
                "var canvas = document.querySelector({}); \
                 if (!canvas || !canvas.getContext) {{ return false; }} \
                 var ctx = canvas.getContext('2d'); \
                 ctx.fillStyle = '#F8F808'; ctx.fillRect({dot_x}, {dot_y}, 7, 7); \
                 ctx.fillStyle = '#F80808'; ctx.fillRect({dot_x} + 1, {dot_y} + 1, 5, 5); \
                 return true;",
                quote(css)
            );
            self.execute_script(&script).await?;
        }
        let (from_x, from_y) = offset_from_center((x, y), (size.width, size.height), center);
        let chain = self.session.webdriver()?
            .action_chain()
            .move_to_element_with_offset(&element, from_x, from_y);
        let chain = if double {
            chain.double_click()
        } else {
            chain.click()
        };
        chain.perform().await.map_err(SeleniumBaseError::WebDriver)?;
        self.demo_pause(PauseKind::Full).await;
        Ok(())
    }

    /// Drags the element `selector` picks by `x` pixels right and `y` pixels
    /// down, as Python's `drag_and_drop_with_offset(selector, x, y, by,
    /// timeout=None)` does. The keyword is [`DragByOffset::timeout`]. Waits for
    /// the element to be visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not show
    /// up and the WebDriver error if the drag fails.
    pub fn drag_and_drop_with_offset<'a>(
        &'a mut self,
        selector: &'a str,
        x: i64,
        y: i64,
    ) -> DragByOffset<'a> {
        DragByOffset::new(self, selector, x, y)
    }
    
    /// The work of [`drag_and_drop_with_offset`](Self::drag_and_drop_with_offset).
    pub(crate) async fn run_drag_by_offset(
        &mut self,
        css: &str,
        x: i64,
        y: i64,
        timeout: Option<Duration>,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let seconds = self.wait_secs(timeout);
        self.await_ready_within(css, Ready::Visible, seconds).await?;
        self.record("drag_and_drop_with_offset", Some(css), None);
        self.demo_highlight(css).await?;
        let element = self.session.find(by).await?;
        self.session.webdriver()?
            .action_chain()
            .drag_and_drop_element_by_offset(&element, x, y)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        self.demo_pause(PauseKind::Full).await;
        Ok(())
    }

    /// Clicks a sequence of elements in order.
    pub async fn click_chain(&mut self, css_list: &[&str]) -> Result<(), SeleniumBaseError> {
        let mut chain = self.session.webdriver()?.action_chain();
        for css in css_list {
            let by = Selector::auto(css).to_by()?;
            let element = self.session.find(by).await?;
            chain = chain.move_to_element_center(&element).click();
        }
        chain.perform().await.map_err(SeleniumBaseError::WebDriver)
    }

    // ========================================================================
    // Wait helpers (additional variants)
    // ========================================================================

    /// Waits up to `timeout_secs` for any element from `css_list` to be present,
    /// and returns the first one found (in the order of `css_list`).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] at once, without
    /// waiting, if a selector is empty. Returns
    /// [`SeleniumBaseError::WaitTimeout`] naming the selectors if none matches
    /// after the timeout.
    pub async fn wait_for_any_of_elements_present(
        &self,
        css_list: &[&str],
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        let locators = css_list
            .iter()
            .map(|css| Selector::auto(css).to_by())
            .collect::<Result<Vec<_>, _>>()?;
        poll_until_found(
            self.effective_timeout(timeout_secs),
            format!("None of the selectors [{}] was present", css_list.join(", ")),
            || async {
                for by in &locators {
                    let found = self.session.webdriver()?.find_all(by.clone()).await?;
                    if let Some(element) = found.into_iter().next() {
                        return Ok(Polled::Found(element));
                    }
                }
                Ok(Polled::Pending { seen: None })
            },
        )
        .await
    }

    /// Waits up to `timeout_secs` for a link with exact `text`, and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the text if no such
    /// link appears.
    pub async fn wait_for_link_text(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        poll_until_found(
            self.effective_timeout(timeout_secs),
            format!("Link text '{text}' was not present"),
            || async {
                let links = self.session.webdriver()?.find_all(By::LinkText(text)).await?;
                Ok(first_found(links))
            },
        )
        .await
    }

    /// Waits up to `timeout_secs` for a link containing `text`, and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the text if no such
    /// link appears.
    pub async fn wait_for_partial_link_text(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        poll_until_found(
            self.effective_timeout(timeout_secs),
            format!("Partial link text '{text}' was not present"),
            || async {
                let links = self.session.webdriver()?
                    .find_all(By::PartialLinkText(text))
                    .await?;
                Ok(first_found(links))
            },
        )
        .await
    }

    /// Waits up to `timeout_secs` for the element `css` to be displayed with
    /// exactly `text` as its text. The same as
    /// [`wait_for_exact_text_visible`](Self::wait_for_exact_text_visible), as
    /// in Python, where `wait_for_exact_text(text, selector)` is its alias.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the text if the
    /// element does not show it in time.
    pub async fn wait_for_exact_text(
        &self,
        text: &str,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_exact_text_visible(text, css, timeout_secs)
            .await
    }

    /// Waits up to `timeout_secs` for the page title to contain `substring`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming `substring` and the
    /// last title seen if the title never contains it.
    pub async fn wait_for_title(
        &mut self,
        substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_found(
            self.effective_timeout(timeout_secs),
            format!("Title did not contain '{substring}'"),
            || async {
                let title = self.session.webdriver()?.title().await?;
                Ok(if title.contains(substring) {
                    Polled::Found(())
                } else {
                    Polled::Pending {
                        seen: Some(format!("last title: '{title}'")),
                    }
                })
            },
        )
        .await
    }

    // ========================================================================
    // Demo / design mode
    // ========================================================================

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
        Self::append_data_in(&ensure_latest_logs_dir()?, data, filename)
    }

    /// Appends `data` and a newline to the file `filename` in `dir`.
    fn append_data_in(dir: &Path, data: &str, filename: &str) -> Result<(), SeleniumBaseError> {
        use std::io::Write;
        let path = crate::artifacts::confined_path(dir, filename)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{data}")?;
        Ok(())
    }

    /// The name of the log file `save_data_to_logs` and `append_data_to_logs`
    /// write: `sb_` and the Unix time in seconds.
    fn timestamped_log_name() -> String {
        format!(
            "sb_{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )
    }

    /// Appends `data` to `sb_<seconds>.log` in the log folder of the running test.
    /// The folder is the one [`save_screenshot_to_logs`](Self::save_screenshot_to_logs)
    /// writes to.
    pub fn append_data_to_logs(&self, data: &str) -> Result<(), SeleniumBaseError> {
        Self::append_data_in(&self.test_logs_dir()?, data, &Self::timestamped_log_name())
    }

    /// Saves `data` to `filename` in the logs directory (overwrites).
    pub fn save_data_as(&self, data: &str, filename: &str) -> Result<(), SeleniumBaseError> {
        Self::save_data_in(&ensure_latest_logs_dir()?, data, filename)
    }

    /// Writes `data` to the file `filename` in `dir`, replacing it.
    fn save_data_in(dir: &Path, data: &str, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = crate::artifacts::confined_path(dir, filename)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Saves `data` to `sb_<seconds>.log` in the log folder of the running test.
    /// The folder is the one [`save_screenshot_to_logs`](Self::save_screenshot_to_logs)
    /// writes to.
    pub fn save_data_to_logs(&self, data: &str) -> Result<(), SeleniumBaseError> {
        Self::save_data_in(&self.test_logs_dir()?, data, &Self::timestamped_log_name())
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

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sbrs-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn waiting_for_a_file_sees_one_that_appears_later() {
        let dir = scratch_dir("late");
        let path = dir.join("report.csv");
        let writer = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            fs::write(writer, "a,b").unwrap();
        });
        wait_for_file(&path, 5).await.expect("the file appears within the timeout");
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_for_a_file_that_never_appears_times_out() {
        let missing = scratch_dir("never").join("nope.csv");
        let error = wait_for_file(&missing, 2).await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
    }

    #[test]
    fn selecting_an_option_that_is_not_there_is_an_error_not_a_silent_success() {
        assert!(select_index_outcome(&serde_json::json!("ok"), "#size", 1).is_ok());
        for (status, expected) in [
            ("no-option", "no option at index 99"),
            ("no-select", "is not a <select>"),
            ("disabled", "is disabled"),
        ] {
            let error = select_index_outcome(&serde_json::json!(status), "#size", 99)
                .expect_err(status)
                .to_string();
            assert!(error.contains("#size") && error.contains(expected), "{status}: {error}");
        }
        // A script that returned nothing at all did not select anything either.
        assert!(select_index_outcome(&Value::Null, "#size", 1).is_err());
    }

    #[test]
    fn only_one_trailing_newline_is_taken_for_the_enter_key() {
        assert_eq!(split_trailing_enter("rust\n"), ("rust", true));
        assert_eq!(split_trailing_enter("rust"), ("rust", false));
        assert_eq!(split_trailing_enter("a\nb"), ("a\nb", false));
        assert_eq!(split_trailing_enter("a\n\n"), ("a\n", true));
        assert_eq!(split_trailing_enter("\n"), ("", true));
        assert_eq!(split_trailing_enter(""), ("", false));
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

    #[test]
    fn totp_matches_the_rfc_6238_sha1_test_vectors() {
        // RFC 6238, appendix B: the secret is the ASCII string
        // "12345678901234567890"; its codes have eight digits there, and the
        // six-digit code is the last six of those.
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        for (time, code) in [
            (59, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
            (20_000_000_000, "353130"),
        ] {
            assert_eq!(generate_totp(secret, time).unwrap(), code, "at {time}");
        }
    }

    #[test]
    fn totp_ignores_spaces_dashes_and_case_in_the_secret() {
        let plain = generate_totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
        assert_eq!(
            generate_totp("gezd gnbv-gy3t qojq gezd gnbv-gy3t qojq", 59).unwrap(),
            plain
        );
    }

    #[test]
    fn totp_refuses_a_secret_that_is_not_base32() {
        for secret in ["not base32!", "GEZD1", "ÄÖÜ"] {
            assert!(
                matches!(generate_totp(secret, 59), Err(SeleniumBaseError::AssertionFailed(_))),
                "{secret:?}"
            );
        }
    }

    #[test]
    fn a_value_is_an_instance_of_its_own_type() {
        let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        sb.assert_is_instance(&5_i32, "i32").unwrap();
        sb.assert_is_instance(&String::from("x"), "String").unwrap();
        sb.assert_is_instance(&vec![1_u8], "Vec").unwrap();
        assert!(sb.assert_is_instance(&5_i32, "String").is_err());

        sb.assert_not_is_instance(&5_i32, "String").unwrap();
        assert!(sb.assert_not_is_instance(&5_i32, "i32").is_err());
    }
}
