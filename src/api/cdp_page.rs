//! CDP Page automation helpers and the [`CdpPage`] wrapper.
//!
//! These helpers issue Chrome DevTools Protocol commands directly through a
//! [`BrowserSession`][crate::browser::session::BrowserSession], bypassing the
//! WebDriver layer for low-level control over navigation, DOM queries, input
//! events, screenshots, and JavaScript evaluation.
//!
//! Elements are found and measured by running JavaScript with
//! `Runtime.evaluate`. Nothing here switches on the `DOM` protocol domain,
//! neither with `DOM.enable` nor with `DOM.getDocument`, which enables it as a
//! side effect: a page can tell that a client is attached to that domain.

use std::path::Path;
use std::time::{Duration, Instant};

use base64::prelude::*;
use serde_json::{json, Value};

use crate::browser::session::BrowserSession;
use crate::error::SeleniumBaseError;
use crate::js_code::quote;
use crate::sb_cdp::without_final_return;

/// How often [`CdpPage::wait_for_element_present`] re-queries the DOM.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long [`CdpPage::click`] waits for its element, as in
/// [`sb_cdp`](crate::sb_cdp) and Python's `sb.cdp.click`.
const CLICK_TIMEOUT: Duration = Duration::from_secs(7);

/// A node returned by CDP DOM queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdpNode {
    /// CDP `nodeId` used by DOM-domain commands such as `DOM.getBoxModel`.
    ///
    /// It is 0 for a node that [`CdpPage`] found: the page is read without the
    /// DOM domain, which hands out node ids. Use [`object_id`](Self::object_id)
    /// to refer to such a node.
    pub node_id: i64,
    /// Optional CDP `backendNodeId`.
    pub backend_node_id: Option<i64>,
    /// Optional CDP `objectId` from `Runtime` or `DOM` queries. A node that
    /// [`CdpPage`] found has one, for use with `Runtime.callFunctionOn`.
    pub object_id: Option<String>,
    /// Selector that produced this node, when available.
    pub selector: Option<String>,
}

impl CdpNode {
    /// Parses a `DOM.querySelector` response into a [`CdpNode`].
    pub fn from_query_result(value: &Value, selector: &str) -> Option<Self> {
        value
            .get("nodeId")
            .and_then(|v| v.as_i64())
            .map(|node_id| Self {
                node_id,
                backend_node_id: value.get("backendNodeId").and_then(|v| v.as_i64()),
                object_id: value
                    .get("objectId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned()),
                selector: Some(selector.to_owned()),
            })
    }

    /// Parses a single `nodeId` value (for example from `DOM.querySelectorAll`).
    pub fn from_node_id(node_id: i64, selector: &str) -> Self {
        Self {
            node_id,
            backend_node_id: None,
            object_id: None,
            selector: Some(selector.to_owned()),
        }
    }

    /// A node known by its `Runtime` `objectId`, which is how [`CdpPage`] finds
    /// elements. Its [`node_id`](Self::node_id) is 0.
    pub fn from_object_id(object_id: &str, selector: &str) -> Self {
        Self {
            node_id: 0,
            backend_node_id: None,
            object_id: Some(object_id.to_owned()),
            selector: Some(selector.to_owned()),
        }
    }
}

/// Low-level CDP page automation tied to a [`BrowserSession`].
pub struct CdpPage<'a> {
    session: &'a BrowserSession,
}

impl<'a> CdpPage<'a> {
    /// Creates a [`CdpPage`] backed by `session`.
    pub fn new(session: &'a BrowserSession) -> Self {
        Self { session }
    }

    /// Sends a CDP command with JSON parameters and returns the raw response.
    async fn execute(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp_with_params(method, params).await
    }

    /// Navigates to `url` via `Page.navigate`.
    pub async fn open(&self, url: &str) -> Result<(), SeleniumBaseError> {
        self.execute("Page.navigate", json!({"url": url})).await?;
        Ok(())
    }

    /// Finds the first element matching `selector` by running
    /// `document.querySelector` in the page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches and
    /// [`SeleniumBaseError::InvalidSelector`] if the browser rejects the
    /// selector.
    pub async fn find_element(&self, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
        let response = self
            .execute(
                "Runtime.evaluate",
                json!({"expression": query_script(selector), "returnByValue": false}),
            )
            .await?;
        parse_found_node(&response, selector)
    }

    /// Finds all elements matching `selector` by running
    /// `document.querySelectorAll` in the page, in document order.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the browser rejects
    /// the selector.
    pub async fn find_all(&self, selector: &str) -> Result<Vec<CdpNode>, SeleniumBaseError> {
        let response = self
            .execute(
                "Runtime.evaluate",
                json!({"expression": query_all_script(selector), "returnByValue": false}),
            )
            .await?;
        let array = parse_found_node(&response, selector)?;
        let object_id = array.object_id.unwrap_or_default();
        let properties = self
            .execute(
                "Runtime.getProperties",
                json!({"objectId": object_id, "ownProperties": true}),
            )
            .await?;
        parse_found_nodes(&properties, selector)
    }

    /// Waits until an element matching `selector` is present in the DOM.
    ///
    /// Polls [`find_element`][Self::find_element] until the element appears or
    /// `timeout` elapses, mirroring SeleniumBase's CDP-mode
    /// `wait_for_element_present()`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::element_not_found`] if the element is
    /// still absent when `timeout` expires.
    pub async fn wait_for_element_present(
        &self,
        selector: &str,
        timeout: Duration,
    ) -> Result<CdpNode, SeleniumBaseError> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.find_element(selector).await {
                Ok(node) => return Ok(node),
                // Waiting longer cannot make a selector valid.
                Err(SeleniumBaseError::InvalidSelector(reason))
                    if reason.contains("is not a valid selector") =>
                {
                    return Err(SeleniumBaseError::InvalidSelector(reason));
                }
                Err(_) => {}
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::element_not_found(selector));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Clicks the center of the element matching `selector`.
    ///
    /// Waits up to 7 seconds for the element, scrolls it into view, reads its
    /// rectangle, takes its center and dispatches `mouseMoved`,
    /// `mousePressed`, and `mouseReleased` input events there. Use
    /// [`click_with_timeout`](Self::click_with_timeout) for another wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches,
    /// [`SeleniumBaseError::ElementNotInteractable`] if the element has no
    /// size, as with `display: none`, and the browser's error if it refuses
    /// an input event.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        self.click_with_timeout(selector, CLICK_TIMEOUT).await
    }

    /// [`click`](Self::click), waiting up to `timeout` for the element.
    ///
    /// # Errors
    ///
    /// As [`click`](Self::click).
    pub async fn click_with_timeout(
        &self,
        selector: &str,
        timeout: Duration,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_present(selector, timeout).await?;
        let rectangle = self.evaluate_raw(&center_script(selector)).await?;
        let (center_x, center_y) = parse_center(&rectangle, selector)?;

        for event_type in ["mouseMoved", "mousePressed", "mouseReleased"] {
            let params = json!({
                "type": event_type,
                "x": center_x,
                "y": center_y,
                "button": "left",
                "clickCount": 1,
            });
            self.execute("Input.dispatchMouseEvent", params).await?;
        }
        Ok(())
    }

    /// Focuses the element, clears its value, and types `text` one key at a
    /// time.
    ///
    /// Each character is delivered as its own `Input.dispatchKeyEvent`, which
    /// produces the trusted key events that pages listening for real keyboard
    /// input expect. Use [`fast_type`][Self::fast_type] when throughput
    /// matters more than realism.
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.focus_and_clear(selector).await?;
        for ch in text.chars() {
            if ch == '\n' || ch == '\r' {
                self.press_enter().await?;
                continue;
            }
            self.execute(
                "Input.dispatchKeyEvent",
                json!({"type": "char", "text": ch.to_string()}),
            )
            .await?;
        }
        Ok(())
    }

    /// Focuses the element, clears its value, and inserts `text` in a single
    /// operation.
    ///
    /// This mirrors SeleniumBase's CDP-mode `fast_type()`: it is the quickest
    /// way to fill a field, but it does not emit per-character key events, so
    /// pages that watch for realistic typing can tell the difference. Prefer
    /// [`type_text`][Self::type_text] when running in a stealth context.
    pub async fn fast_type(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.focus_and_clear(selector).await?;
        let (body, trailing_newline) = match text.strip_suffix('\n') {
            Some(stripped) => (stripped, true),
            None => (text, false),
        };
        if !body.is_empty() {
            self.execute("Input.insertText", json!({"text": body}))
                .await?;
        }
        if trailing_newline {
            self.press_enter().await?;
        }
        Ok(())
    }

    /// Focuses the element matching `selector` and empties its value.
    async fn focus_and_clear(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let script = single_element_script(selector, FOCUS_AND_CLEAR);
        let result = self.evaluate_raw(&script).await?;
        if result.get("value").and_then(Value::as_bool) != Some(true) {
            return Err(SeleniumBaseError::element_not_found(selector));
        }
        Ok(())
    }

    /// Dispatches a single Enter keypress to the focused element.
    async fn press_enter(&self) -> Result<(), SeleniumBaseError> {
        for event_type in ["keyDown", "keyUp"] {
            self.execute(
                "Input.dispatchKeyEvent",
                json!({
                    "type": event_type,
                    "key": "Enter",
                    "code": "Enter",
                    "text": "\r",
                    "windowsVirtualKeyCode": 13,
                    "nativeVirtualKeyCode": 13,
                }),
            )
            .await?;
        }
        Ok(())
    }

    /// Returns the visible text of the first element matching `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let script = single_element_script(
            selector,
            "el ? String(el.innerText !== undefined ? el.innerText : el.textContent) : null",
        );
        let result = self.evaluate_raw(&script).await?;
        extract_string_value(&result).ok_or_else(|| SeleniumBaseError::element_not_found(selector))
    }

    /// Returns the named HTML attribute, or `None` if the element has no such
    /// attribute.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let result = self
            .evaluate_raw(&attribute_script(selector, attribute))
            .await?;
        found_value(&result, selector)
    }

    /// Returns the named DOM property, falling back to the attribute of the
    /// same name when the property is undefined.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn get_property(
        &self,
        selector: &str,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let result = self
            .evaluate_raw(&property_script(selector, property))
            .await?;
        found_value(&result, selector)
    }

    /// Returns `document.title`.
    pub async fn get_title(&self) -> Result<String, SeleniumBaseError> {
        let result = self.evaluate_raw("document.title").await?;
        Ok(extract_string_value(&result).unwrap_or_default())
    }

    /// Returns `window.location.href`.
    pub async fn get_url(&self) -> Result<String, SeleniumBaseError> {
        let result = self.evaluate_raw("window.location.href").await?;
        Ok(extract_string_value(&result).unwrap_or_default())
    }

    /// Evaluates `expression` via `Runtime.evaluate` and returns its value
    /// (`Null` for `undefined`).
    ///
    /// Like Python's `evaluate`, a `return ` that starts the last line is
    /// dropped, so a Selenium-style `return document.title` works.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the script throws.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let result = self
            .evaluate_raw(&without_final_return(expression))
            .await?;
        Ok(result.get("value").cloned().unwrap_or(Value::Null))
    }

    /// Evaluates `expression` via `Runtime.evaluate` with `returnByValue: true`
    /// and returns the raw CDP result object, `{"type": ..., "value": ...}`.
    async fn evaluate_raw(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let response = self
            .execute(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await?;
        if let Some(exception) = response.get("exceptionDetails") {
            let description = exception
                .pointer("/exception/description")
                .or_else(|| exception.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("script error");
            return Err(SeleniumBaseError::cdp_driver(
                description.lines().next().unwrap_or(description),
            ));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Navigates back via `history.back()`.
    pub async fn go_back(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate_raw("history.back()").await?;
        Ok(())
    }

    /// Navigates forward via `history.forward()`.
    pub async fn go_forward(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate_raw("history.forward()").await?;
        Ok(())
    }

    /// Reloads the current page via `location.reload()`.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate_raw("location.reload()").await?;
        Ok(())
    }

    /// Selects the `<option>` with matching visible text inside `<select>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] when the dropdown is
    /// missing and [`SeleniumBaseError::InvalidSelector`] when it has no such
    /// option.
    pub async fn select_option_by_text(
        &self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let result = self
            .evaluate_raw(&select_by_text_script(selector, text))
            .await?;
        selected(&result, selector, || {
            format!("Option with text '{text}' not found in {selector}")
        })
    }

    /// Selects the `<option>` with the given value inside `<select>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] when the dropdown is
    /// missing and [`SeleniumBaseError::InvalidSelector`] when it has no
    /// option with that value.
    pub async fn select_option_by_value(
        &self,
        selector: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let result = self
            .evaluate_raw(&select_by_value_script(selector, value))
            .await?;
        selected(&result, selector, || {
            format!("Option with value '{value}' not found in {selector}")
        })
    }

    /// Selects the `<option>` at `index` (zero-based) inside `<select>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] when the dropdown is
    /// missing and [`SeleniumBaseError::InvalidSelector`] when it has no
    /// option at `index`.
    pub async fn select_option_by_index(
        &self,
        selector: &str,
        index: usize,
    ) -> Result<(), SeleniumBaseError> {
        let result = self
            .evaluate_raw(&select_by_index_script(selector, index))
            .await?;
        selected(&result, selector, || {
            format!("Option at index {index} not found in {selector}")
        })
    }

    /// Captures a PNG screenshot via `Page.captureScreenshot` and writes it to
    /// `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        let response = self
            .execute("Page.captureScreenshot", json!({"format": "png"}))
            .await?;
        let data = response
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SeleniumBaseError::Unsupported("Screenshot data missing".to_owned()))?;
        let bytes = BASE64_STANDARD.decode(data).map_err(|e| {
            SeleniumBaseError::Unsupported(format!("Failed to decode screenshot: {e}"))
        })?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// The body of the script that focuses an element and empties its value, for
/// [`single_element_script`]. It gives `true` when the element exists.
const FOCUS_AND_CLEAR: &str = "el ? (el.focus(), el.value = '', true) : false";

/// The expression that gives the first element matching `selector`, or `null`.
fn query_script(selector: &str) -> String {
    format!("document.querySelector({})", quote(selector))
}

/// The expression that gives an array of every element matching `selector`.
fn query_all_script(selector: &str) -> String {
    format!("Array.from(document.querySelectorAll({}))", quote(selector))
}

/// The expression that scrolls the first element matching `selector` into
/// view and gives `[left, top, width, height]` of it in viewport coordinates,
/// or `null`.
fn center_script(selector: &str) -> String {
    format!(
        "(function() {{ var el = document.querySelector({}); \
         if (!el) return null; \
         if (el.scrollIntoViewIfNeeded) el.scrollIntoViewIfNeeded(true); \
         else el.scrollIntoView(true); \
         var r = el.getBoundingClientRect(); \
         return [r.left, r.top, r.width, r.height]; }})()",
        quote(selector)
    )
}

/// The node in a `Runtime.evaluate` response that was asked for by reference.
fn parse_found_node(response: &Value, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
    if let Some(exception) = response.get("exceptionDetails") {
        let reason = exception
            .pointer("/exception/description")
            .or_else(|| exception.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("the script threw");
        return Err(SeleniumBaseError::InvalidSelector(format!(
            "{selector}: {reason}"
        )));
    }
    response
        .pointer("/result/objectId")
        .and_then(Value::as_str)
        .map(|object_id| CdpNode::from_object_id(object_id, selector))
        .ok_or_else(|| SeleniumBaseError::element_not_found(selector))
}

/// The array elements in a `Runtime.getProperties` response, in index order.
fn parse_found_nodes(
    properties: &Value,
    selector: &str,
) -> Result<Vec<CdpNode>, SeleniumBaseError> {
    let list = properties
        .get("result")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            SeleniumBaseError::cdp_driver("Runtime.getProperties returned no result list")
        })?;
    // The elements are the properties named by an index; `length` and
    // `__proto__` are the others.
    let mut found: Vec<(usize, CdpNode)> = list
        .iter()
        .filter_map(|property| {
            let index = property.get("name")?.as_str()?.parse::<usize>().ok()?;
            let object_id = property.pointer("/value/objectId")?.as_str()?;
            Some((index, CdpNode::from_object_id(object_id, selector)))
        })
        .collect();
    found.sort_by_key(|(index, _)| *index);
    Ok(found.into_iter().map(|(_, node)| node).collect())
}

/// The center of the rectangle that [`center_script`] gave, as the `result`
/// object of a `Runtime.evaluate` response.
fn parse_center(result: &Value, selector: &str) -> Result<(f64, f64), SeleniumBaseError> {
    let value = result.get("value").unwrap_or(&Value::Null);
    if value.is_null() {
        return Err(SeleniumBaseError::element_not_found(selector));
    }
    let numbers: Vec<f64> = value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let &[left, top, width, height] = numbers.as_slice() else {
        return Err(SeleniumBaseError::Unsupported(
            "Invalid element rectangle".to_owned(),
        ));
    };
    if width <= 0.0 && height <= 0.0 {
        return Err(SeleniumBaseError::element_not_interactable(
            selector,
            "it has no size on the page",
        ));
    }
    Ok((left + width / 2.0, top + height / 2.0))
}

/// Builds a script that selects an element and evaluates `inner` on it.
fn single_element_script(selector: &str, inner: &str) -> String {
    let selector = quote(selector);
    format!("(function() {{ var el = document.querySelector({selector}); return {inner}; }})()")
}

/// The expression that gives the named attribute of the first element
/// matching `selector` as the only item of an array, or `null` when nothing
/// matches.
fn attribute_script(selector: &str, attribute: &str) -> String {
    let (selector, attribute) = (quote(selector), quote(attribute));
    format!(
        "(function() {{ var el = document.querySelector({selector}); \
         return el ? [el.getAttribute({attribute})] : null; }})()"
    )
}

/// The expression that gives the named property of the first element matching
/// `selector`, as a string, or its attribute of that name when the property is
/// undefined, as the only item of an array, or `null` when nothing matches.
fn property_script(selector: &str, property: &str) -> String {
    let (selector, property) = (quote(selector), quote(property));
    format!(
        "(function() {{ \
         var el = document.querySelector({selector}); \
         if (!el) return null; \
         var v = el[{property}]; \
         return [v !== undefined ? String(v) : el.getAttribute({property})]; \
         }})()"
    )
}

/// The expression that selects the `<option>` of the first element matching
/// `selector` whose text is `text`. It gives `true` for that, `false` for a
/// dropdown without such an option and `null` for no dropdown.
fn select_by_text_script(selector: &str, text: &str) -> String {
    let (selector, text) = (quote(selector), quote(text));
    format!(
        "(function() {{ \
         var s = document.querySelector({selector}); \
         if (!s) return null; \
         for (var i = 0; i < s.options.length; i++) {{ \
             if (s.options[i].text === {text}) {{ \
                 s.selectedIndex = i; \
                 s.dispatchEvent(new Event('change', {{bubbles: true}})); \
                 return true; \
             }} \
         }} \
         return false; \
         }})()"
    )
}

/// The expression that selects the `<option>` of the first element matching
/// `selector` whose value is `value`. It gives `true` for that, `false` for a
/// dropdown without such an option and `null` for no dropdown.
fn select_by_value_script(selector: &str, value: &str) -> String {
    let (selector, value) = (quote(selector), quote(value));
    format!(
        "(function() {{ \
         var s = document.querySelector({selector}); \
         if (!s) return null; \
         for (var i = 0; i < s.options.length; i++) {{ \
             if (s.options[i].value === {value}) {{ \
                 s.selectedIndex = i; \
                 s.dispatchEvent(new Event('change', {{bubbles: true}})); \
                 return true; \
             }} \
         }} \
         return false; \
         }})()"
    )
}

/// The expression that selects the `<option>` at `index` of the first element
/// matching `selector`. It gives `true` for that, `false` for a dropdown
/// without such an option and `null` for no dropdown.
fn select_by_index_script(selector: &str, index: usize) -> String {
    let selector = quote(selector);
    format!(
        "(function() {{ \
         var s = document.querySelector({selector}); \
         if (!s) return null; \
         if ({index} >= s.options.length) return false; \
         s.selectedIndex = {index}; \
         s.dispatchEvent(new Event('change', {{bubbles: true}})); \
         return true; \
         }})()"
    )
}

/// The outcome of a select script: `true` is done, `false` is a dropdown
/// without the option (`missing_option` says which), anything else is no
/// dropdown.
fn selected(
    result: &Value,
    selector: &str,
    missing_option: impl FnOnce() -> String,
) -> Result<(), SeleniumBaseError> {
    match result.get("value") {
        Some(Value::Bool(true)) => Ok(()),
        Some(Value::Bool(false)) => Err(SeleniumBaseError::InvalidSelector(missing_option())),
        _ => Err(SeleniumBaseError::element_not_found(selector)),
    }
}

/// The text in the one-item array that [`attribute_script`] and
/// [`property_script`] give, or `None` for an item that is not text. `null`
/// instead of an array is an element that was not there.
fn found_value(result: &Value, selector: &str) -> Result<Option<String>, SeleniumBaseError> {
    match result.get("value") {
        Some(Value::Array(items)) => Ok(items.first().and_then(Value::as_str).map(str::to_owned)),
        _ => Err(SeleniumBaseError::element_not_found(selector)),
    }
}

/// Extracts a string `value` from a CDP `Runtime.evaluate` result object.
fn extract_string_value(result: &Value) -> Option<String> {
    result
        .get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cdp_node_from_query_result() {
        let value = json!({"nodeId": 42, "backendNodeId": 100});
        let node = CdpNode::from_query_result(&value, "#id").unwrap();
        assert_eq!(node.node_id, 42);
        assert_eq!(node.backend_node_id, Some(100));
        assert_eq!(node.object_id, None);
        assert_eq!(node.selector, Some("#id".to_owned()));
    }

    #[test]
    fn cdp_node_from_node_id() {
        let node = CdpNode::from_node_id(7, ".cls");
        assert_eq!(node.node_id, 7);
        assert_eq!(node.selector, Some(".cls".to_owned()));
    }

    #[test]
    fn a_selector_becomes_a_javascript_string_literal() {
        assert_eq!(quote("a'b\"c\\d\n"), r#""a'b\"c\\d\n""#);
    }

    #[test]
    fn the_query_scripts_ask_the_page_and_nothing_else() {
        assert_eq!(query_script("#a'b"), r##"document.querySelector("#a'b")"##);
        assert_eq!(
            query_all_script("li"),
            r#"Array.from(document.querySelectorAll("li"))"#
        );
        let center = center_script("button");
        assert!(center.contains("getBoundingClientRect"), "{center}");
        assert!(!center.contains("DOM."), "{center}");
    }

    #[test]
    fn an_element_found_by_reference_carries_its_object_id() {
        let response = json!({"result": {
            "type": "object", "subtype": "node",
            "className": "HTMLDivElement", "objectId": "obj-1",
        }});

        let node = parse_found_node(&response, "#id").unwrap();

        assert_eq!(node, CdpNode::from_object_id("obj-1", "#id"));
        assert_eq!(node.node_id, 0);
        assert_eq!(node.selector.as_deref(), Some("#id"));
    }

    #[test]
    fn nothing_matching_is_element_not_found_and_a_bad_selector_is_not() {
        let nothing = json!({"result": {"type": "object", "subtype": "null", "value": null}});
        assert!(matches!(
            parse_found_node(&nothing, "#gone"),
            Err(SeleniumBaseError::ElementNotFound { .. })
        ));

        let thrown = json!({
            "result": {"type": "object", "subtype": "error"},
            "exceptionDetails": {"text": "Uncaught", "exception": {
                "description": "SyntaxError: '#' is not a valid selector.",
            }},
        });
        match parse_found_node(&thrown, "#") {
            Err(SeleniumBaseError::InvalidSelector(reason)) => {
                assert!(reason.contains("not a valid selector"), "{reason}");
            }
            other => panic!("expected InvalidSelector, got {other:?}"),
        }
    }

    #[test]
    fn the_elements_of_an_array_come_back_in_index_order() {
        let properties = json!({"result": [
            {"name": "1", "value": {"type": "object", "subtype": "node", "objectId": "o2"}},
            {"name": "0", "value": {"type": "object", "subtype": "node", "objectId": "o1"}},
            {"name": "length", "value": {"type": "number", "value": 2}},
            {"name": "__proto__", "value": {"type": "object", "objectId": "proto"}},
        ]});

        let nodes = parse_found_nodes(&properties, "li").unwrap();

        assert_eq!(
            nodes,
            [
                CdpNode::from_object_id("o1", "li"),
                CdpNode::from_object_id("o2", "li")
            ]
        );
        assert!(parse_found_nodes(&json!({"result": []}), "li")
            .unwrap()
            .is_empty());
        assert!(parse_found_nodes(&json!({}), "li").is_err());
    }

    #[test]
    fn the_click_point_is_the_center_of_the_rectangle() {
        let rectangle =
            json!({"type": "object", "subtype": "array", "value": [10.0, 20.0, 100.0, 50.0]});
        let (x, y) = parse_center(&rectangle, "#b").unwrap();
        assert!((x - 60.0).abs() < f64::EPSILON);
        assert!((y - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_rectangle_with_one_empty_side_still_has_a_center() {
        let line = json!({"type": "object", "value": [5.0, 5.0, 0.0, 10.0]});
        assert_eq!(parse_center(&line, "#b").unwrap(), (5.0, 10.0));
    }

    #[test]
    fn a_missing_or_sizeless_or_odd_rectangle_is_an_error_of_its_own_kind() {
        let missing = json!({"type": "object", "subtype": "null", "value": null});
        assert!(matches!(
            parse_center(&missing, "#b"),
            Err(SeleniumBaseError::ElementNotFound { .. })
        ));
        let hidden = json!({"type": "object", "value": [0.0, 0.0, 0.0, 0.0]});
        assert!(matches!(
            parse_center(&hidden, "#b"),
            Err(SeleniumBaseError::ElementNotInteractable { .. })
        ));
        let odd = json!({"type": "object", "value": [1.0, 2.0]});
        assert!(matches!(
            parse_center(&odd, "#b"),
            Err(SeleniumBaseError::Unsupported(_))
        ));
    }

    #[test]
    fn extract_string_value_parsing() {
        let result = json!({"type": "string", "value": "hello"});
        assert_eq!(extract_string_value(&result), Some("hello".to_owned()));
        assert_eq!(extract_string_value(&json!({"type": "object"})), None);
    }

    #[test]
    fn single_element_script_escapes_quotes() {
        let script = single_element_script("[data-test='foo]", "el.textContent");
        assert!(script.contains(r#"document.querySelector("[data-test='foo]")"#));
    }

    /// Text a selector, an attribute name, an option's text or a value could
    /// hold, each of which breaks a script that pastes it between quotes.
    const HOSTILE: &[&str] = &[
        "a[title='x']",
        "x'); alert(1); ('",
        r#"say "hi" and C:\dir\new"#,
        "line one\nline two\r\n\ttabbed",
        "separators \u{2028} and \u{2029} inside",
        "</script><!-- &amp; -->",
        "emoji 🦀 and 日本語",
        "",
    ];

    /// A script cut into its code, with every string literal replaced by `S`,
    /// and what each literal evaluates to, the way a JavaScript engine reads
    /// it. It says why an engine would not read it: a literal that never ends,
    /// or one with a raw line terminator in it.
    fn read(script: &str) -> Result<(String, Vec<String>), String> {
        fn hex(chars: &mut std::str::Chars<'_>, digits: usize) -> Result<char, String> {
            let text: String = chars.by_ref().take(digits).collect();
            u32::from_str_radix(&text, 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| format!("bad escape \\x{text}"))
        }

        let mut skeleton = String::new();
        let mut literals = Vec::new();
        let mut chars = script.chars();
        while let Some(c) = chars.next() {
            if c != '\'' && c != '"' {
                skeleton.push(c);
                continue;
            }
            let mut value = String::new();
            loop {
                match chars.next().ok_or("a string literal never ends")? {
                    end if end == c => break,
                    '\\' => match chars.next().ok_or("a backslash ends the script")? {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'x' => value.push(hex(&mut chars, 2)?),
                        'u' => value.push(hex(&mut chars, 4)?),
                        '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                            return Err("a line continuation inside a string literal".to_owned())
                        }
                        other => value.push(other),
                    },
                    '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                        return Err("a raw line terminator inside a string literal".to_owned())
                    }
                    other => value.push(other),
                }
            }
            skeleton.push('S');
            literals.push(value);
        }
        Ok((skeleton, literals))
    }

    /// `text` with every run of white space as one space.
    fn squash(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Checks that `build` makes a script whose code does not depend on the
    /// text it is given and whose string literals are exactly `literals(text)`,
    /// for every hostile text: nothing a caller passed became code.
    fn assert_inert(build: impl Fn(&str) -> String, literals: impl Fn(&str) -> Vec<String>) {
        let (plain_code, _) = read(&build("plain")).expect("a plain script reads");
        for text in HOSTILE {
            let script = build(text);
            let (code, found) = read(&script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
            assert_eq!(squash(&code), squash(&plain_code), "script: {script:?}");
            assert_eq!(found, literals(text), "script: {script:?}");
        }
    }

    #[test]
    fn the_reader_rejects_what_an_engine_would_reject() {
        assert!(read(r#"f('x', "y")"#).is_ok());
        assert!(read("f('it's')").is_err(), "an apostrophe ends the literal");
        assert!(read("f('a\nb')").is_err(), "a raw newline");
        assert!(read("f(\"a\u{2028}b\")").is_err(), "a raw line separator");
        assert!(read("f(\"open").is_err(), "no end");
    }

    #[test]
    fn the_query_scripts_keep_the_selector_inside_its_string() {
        for build in [query_script, query_all_script, center_script] {
            assert_inert(build, |text| vec![text.to_owned()]);
        }
    }

    #[test]
    fn an_element_script_keeps_the_selector_inside_its_string() {
        assert_inert(
            |text| single_element_script(text, "el ? el.innerText : ''"),
            |text| vec![text.to_owned(), String::new()],
        );
        assert_inert(
            |text| single_element_script(text, FOCUS_AND_CLEAR),
            |text| vec![text.to_owned(), String::new()],
        );
    }

    #[test]
    fn get_attribute_keeps_the_selector_and_the_name_inside_their_strings() {
        assert_inert(
            |text| attribute_script(&format!("a[title='{text}']"), &format!("data-{text}")),
            |text| vec![format!("a[title='{text}']"), format!("data-{text}")],
        );
    }

    #[test]
    fn get_property_keeps_the_selector_and_the_name_inside_their_strings() {
        assert_inert(
            |text| property_script(&format!("a[title='{text}']"), &format!("data-{text}")),
            |text| {
                let name = format!("data-{text}");
                vec![format!("a[title='{text}']"), name.clone(), name]
            },
        );
    }

    #[test]
    fn selecting_an_option_by_text_keeps_the_selector_and_the_text_inside_their_strings() {
        assert_inert(
            |text| select_by_text_script(&format!("select[name='{text}']"), text),
            |text| {
                vec![
                    format!("select[name='{text}']"),
                    text.to_owned(),
                    "change".to_owned(),
                ]
            },
        );
    }

    #[test]
    fn selecting_an_option_by_value_keeps_the_selector_and_the_value_inside_their_strings() {
        assert_inert(
            |text| select_by_value_script(&format!("select[name='{text}']"), text),
            |text| {
                vec![
                    format!("select[name='{text}']"),
                    text.to_owned(),
                    "change".to_owned(),
                ]
            },
        );
    }

    #[test]
    fn selecting_an_option_by_index_keeps_the_selector_inside_its_string() {
        assert_inert(
            |text| select_by_index_script(&format!("select[name='{text}']"), 3),
            |text| vec![format!("select[name='{text}']"), "change".to_owned()],
        );
        let script = select_by_index_script("select", 3);
        assert!(script.contains("3 >= s.options.length"), "{script}");
        assert!(script.contains("s.selectedIndex = 3"), "{script}");
    }

    #[test]
    fn a_value_with_a_newline_or_a_line_separator_is_one_literal() {
        // A textarea's value is the case that broke: `s.value = '<text>'` with a
        // raw newline in it is a syntax error.
        for value in ["first\nsecond", "first\u{2028}second", "it's\r\nhere"] {
            let script = select_by_value_script("textarea", value);
            let (_, literals) = read(&script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
            assert_eq!(literals[1], value, "{script:?}");
        }
    }
}

/// `CdpPage` against a mock WebDriver whose DevTools channel is scripted (no
/// browser): which commands it sends and how it reads the answers.
#[cfg(test)]
mod driven_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply, Seen};
    use crate::browser::config::{BrowserConfig, DriverMode};

    /// A CDP-mode session on a WebDriver that answers each DevTools command
    /// with `answer`; `Err` is a browser that refuses the command.
    async fn driven(
        answer: impl Fn(&str, &Value) -> Result<Value, ()> + Send + Sync + 'static,
    ) -> (MockWebDriver, BrowserSession) {
        let driver = MockWebDriver::start(json!({}), move |request| {
            if request.path != "/goog/cdp/execute" {
                return Reply::Value(Value::Null);
            }
            let command = request.body["cmd"].as_str().unwrap_or_default();
            match answer(command, &request.body["params"]) {
                Ok(value) => Reply::Value(value),
                Err(()) => Reply::Error(500, "unknown error"),
            }
        });
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url())
            .with_mode(DriverMode::Cdp);
        config.auto_start_driver = false;
        let session = BrowserSession::connect(config).await.expect("connects");
        (driver, session)
    }

    /// What `Runtime.evaluate` gives for a script that returned `value`.
    fn returned(value: Value) -> Result<Value, ()> {
        Ok(json!({ "result": { "value": value } }))
    }

    /// The answer for an element given back by reference.
    fn node() -> Result<Value, ()> {
        Ok(json!({ "result": { "type": "object", "subtype": "node", "objectId": "o1" } }))
    }

    /// `Runtime.evaluate` scripts, in the order they were sent.
    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        sent(driver, "Runtime.evaluate")
            .iter()
            .filter_map(|params| params["expression"].as_str().map(str::to_owned))
            .collect()
    }

    /// The parameters of each `command` that was sent.
    fn sent(driver: &MockWebDriver, command: &str) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .filter(|request: &Seen| request.body["cmd"] == command)
            .map(|request| request.body["params"].clone())
            .collect()
    }

    /// A page with one button, 100 x 50 at (10, 20) in the viewport.
    fn button_page(command: &str, params: &Value) -> Result<Value, ()> {
        let expression = params["expression"].as_str().unwrap_or_default();
        match command {
            "Runtime.evaluate" if expression.contains("getBoundingClientRect") => {
                returned(json!([10.0, 20.0, 100.0, 50.0]))
            }
            "Runtime.evaluate" if expression.starts_with("document.querySelector(") => node(),
            _ => Ok(json!({})),
        }
    }

    #[tokio::test]
    async fn a_click_scrolls_the_element_into_view_and_presses_at_its_centre() {
        let (driver, session) = driven(button_page).await;

        CdpPage::new(&session).click("#go").await.unwrap();

        let rectangle = scripts(&driver)
            .into_iter()
            .find(|script| script.contains("getBoundingClientRect"))
            .expect("the rectangle was read");
        assert!(
            rectangle.contains("scrollIntoView"),
            "an element below the fold would be clicked off screen: {rectangle}"
        );
        let events: Vec<_> = sent(&driver, "Input.dispatchMouseEvent")
            .iter()
            .map(|p| (p["type"].as_str().unwrap().to_owned(), p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()))
            .collect();
        assert_eq!(
            events,
            [
                ("mouseMoved".to_owned(), 60.0, 45.0),
                ("mousePressed".to_owned(), 60.0, 45.0),
                ("mouseReleased".to_owned(), 60.0, 45.0),
            ]
        );
    }

    #[tokio::test]
    async fn a_click_the_browser_refuses_is_an_error_not_a_success() {
        let (_driver, session) = driven(|command, params| {
            if command == "Input.dispatchMouseEvent" {
                Err(())
            } else {
                button_page(command, params)
            }
        })
        .await;

        let result = CdpPage::new(&session).click("#go").await;

        assert!(result.is_err(), "a dead connection must not look like a click");
    }

    #[tokio::test]
    async fn a_click_waits_for_an_element_that_has_not_appeared_yet() {
        let looks = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&looks);
        let (_driver, session) = driven(move |command, params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            if command == "Runtime.evaluate"
                && expression.starts_with("document.querySelector(")
                && counter.fetch_add(1, Ordering::SeqCst) < 2
            {
                return returned(Value::Null);
            }
            button_page(command, params)
        })
        .await;

        CdpPage::new(&session)
            .click_with_timeout("#late", Duration::from_secs(5))
            .await
            .unwrap();

        assert!(looks.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn a_click_on_an_element_that_never_appears_is_not_found() {
        let (driver, session) = driven(|command, params| {
            if command == "Runtime.evaluate" {
                returned(Value::Null)
            } else {
                button_page(command, params)
            }
        })
        .await;

        let error = CdpPage::new(&session)
            .click_with_timeout("#nowhere", Duration::ZERO)
            .await
            .unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::ElementNotFound { selector, .. } if selector == "#nowhere"),
            "{error}"
        );
        assert!(sent(&driver, "Input.dispatchMouseEvent").is_empty());
    }

    #[tokio::test]
    async fn text_and_attributes_of_a_missing_element_are_not_found_errors() {
        let (_driver, session) = driven(|_, _| returned(Value::Null)).await;
        let page = CdpPage::new(&session);

        for error in [
            page.get_text("#gone").await.unwrap_err(),
            page.get_attribute("#gone", "href").await.unwrap_err(),
            page.get_property("#gone", "value").await.unwrap_err(),
        ] {
            assert!(
                matches!(&error, SeleniumBaseError::ElementNotFound { selector, .. } if selector == "#gone"),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn an_attribute_the_element_lacks_is_none_and_not_an_error() {
        let (_driver, session) = driven(|_, _| returned(json!([null]))).await;

        let attribute = CdpPage::new(&session)
            .get_attribute("a", "download")
            .await
            .unwrap();

        assert_eq!(attribute, None);
    }

    #[tokio::test]
    async fn text_and_attributes_of_an_element_are_read() {
        let (_driver, session) = driven(|_, params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            if expression.contains("innerText") {
                returned(json!("Hello"))
            } else {
                returned(json!(["/home"]))
            }
        })
        .await;
        let page = CdpPage::new(&session);

        assert_eq!(page.get_text("h1").await.unwrap(), "Hello");
        assert_eq!(
            page.get_attribute("a", "href").await.unwrap().as_deref(),
            Some("/home")
        );
        assert_eq!(
            page.get_property("a", "href").await.unwrap().as_deref(),
            Some("/home")
        );
    }

    #[tokio::test]
    async fn evaluate_gives_the_value_and_drops_a_final_return() {
        let (driver, session) = driven(|_, _| returned(json!(2))).await;

        let value = CdpPage::new(&session)
            .evaluate("const a = 1;\nreturn a + 1")
            .await
            .unwrap();

        assert_eq!(value, 2);
        assert_eq!(
            sent(&driver, "Runtime.evaluate").last().unwrap()["expression"],
            "const a = 1;\na + 1"
        );
    }

    #[tokio::test]
    async fn a_script_that_throws_is_a_driver_error_not_an_unsupported_one() {
        let (_driver, session) = driven(|_, _| {
            Ok(json!({ "exceptionDetails": { "exception": {
                "description": "ReferenceError: nope is not defined\n    at <anonymous>:1:1"
            }}}))
        })
        .await;

        let error = CdpPage::new(&session).evaluate("nope").await.unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::CdpDriver(m) if m == "ReferenceError: nope is not defined"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn the_title_and_the_url_are_strings() {
        let (_driver, session) = driven(|_, params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            returned(json!(if expression == "document.title" {
                "Home"
            } else {
                "https://example.test/"
            }))
        })
        .await;
        let page = CdpPage::new(&session);

        assert_eq!(page.get_title().await.unwrap(), "Home");
        assert_eq!(page.get_url().await.unwrap(), "https://example.test/");
    }

    #[tokio::test]
    async fn selecting_reports_a_missing_dropdown_and_a_missing_option_differently() {
        // No dropdown: the script gives null. A dropdown without the option:
        // false. Found and set: true.
        for (answer, expected) in [(Value::Null, "not found"), (json!(false), "no option")] {
            let (_driver, session) = driven(move |_, _| returned(answer.clone())).await;
            let page = CdpPage::new(&session);

            for error in [
                page.select_option_by_value("select", "mars").await.unwrap_err(),
                page.select_option_by_text("select", "Mars").await.unwrap_err(),
                page.select_option_by_index("select", 9).await.unwrap_err(),
            ] {
                match (expected, &error) {
                    ("not found", SeleniumBaseError::ElementNotFound { .. })
                    | ("no option", SeleniumBaseError::InvalidSelector(_)) => {}
                    _ => panic!("{expected}, but got {error:?}"),
                }
            }
        }

        let (_driver, session) = driven(|_, _| returned(json!(true))).await;
        CdpPage::new(&session)
            .select_option_by_value("select", "earth")
            .await
            .unwrap();
    }

    #[test]
    fn selecting_by_value_checks_that_the_option_exists_before_setting_it() {
        // `select.value = x` leaves the dropdown unset for a value it has no
        // option for, and the script would still report success.
        let script = select_by_value_script("select", "mars");
        assert!(script.contains("s.options[i].value ==="), "{script}");
        assert!(!script.contains("s.value ="), "{script}");
    }
}
