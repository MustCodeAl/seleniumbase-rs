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

/// How often [`CdpPage::wait_for_element_present`] re-queries the DOM.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

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
            if let Ok(node) = self.find_element(selector).await {
                return Ok(node);
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::element_not_found(selector));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Clicks the center of the element matching `selector`.
    ///
    /// Reads the element's rectangle in the page, takes its center and
    /// dispatches `mouseMoved`, `mousePressed`, and `mouseReleased` input
    /// events there.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches and
    /// [`SeleniumBaseError::ElementNotInteractable`] if the element has no
    /// size, as with `display: none`.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let script = center_script(selector);
        let rectangle = self.evaluate(&script).await?;
        let (center_x, center_y) = parse_center(&rectangle, selector)?;

        for event_type in ["mouseMoved", "mousePressed", "mouseReleased"] {
            let params = json!({
                "type": event_type,
                "x": center_x,
                "y": center_y,
                "button": "left",
                "clickCount": 1,
            });
            self.execute("Input.dispatchMouseEvent", params).await.ok();
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
        let result = self.evaluate(&script).await?;
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
    pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let script = single_element_script(selector, "el ? el.innerText : ''");
        let result = self.evaluate(&script).await?;
        Ok(extract_string_value(&result).unwrap_or_default())
    }

    /// Returns the named HTML attribute, or `None` if absent.
    pub async fn get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_attr = attribute.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(function() {{ var el = document.querySelector('{escaped_selector}'); return el ? el.getAttribute('{escaped_attr}') : null; }})()"
        );
        let result = self.evaluate(&script).await?;
        Ok(extract_string_value(&result))
    }

    /// Returns the named DOM property, falling back to the attribute of the
    /// same name when the property is undefined.
    pub async fn get_property(
        &self,
        selector: &str,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_prop = property.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(function() {{ \
             var el = document.querySelector('{escaped_selector}'); \
             if (!el) return null; \
             var v = el['{escaped_prop}']; \
             return v !== undefined ? String(v) : el.getAttribute('{escaped_prop}'); \
             }})()"
        );
        let result = self.evaluate(&script).await?;
        Ok(extract_string_value(&result))
    }

    /// Returns `document.title`.
    pub async fn get_title(&self) -> Result<String, SeleniumBaseError> {
        let result = self.evaluate("document.title").await?;
        Ok(extract_string_value(&result).unwrap_or_default())
    }

    /// Returns `window.location.href`.
    pub async fn get_url(&self) -> Result<String, SeleniumBaseError> {
        let result = self.evaluate("window.location.href").await?;
        Ok(extract_string_value(&result).unwrap_or_default())
    }

    /// Evaluates `expression` via `Runtime.evaluate` with `returnByValue: true`
    /// and returns the raw CDP result object.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let response = self
            .execute(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await?;
        if let Some(exc) = response.get("exceptionDetails") {
            return Err(SeleniumBaseError::Unsupported(format!(
                "JS exception: {exc}"
            )));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Navigates back via `history.back()`.
    pub async fn go_back(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate("history.back()").await?;
        Ok(())
    }

    /// Navigates forward via `history.forward()`.
    pub async fn go_forward(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate("history.forward()").await?;
        Ok(())
    }

    /// Reloads the current page via `location.reload()`.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate("location.reload()").await?;
        Ok(())
    }

    /// Selects the `<option>` with matching visible text inside `<select>`.
    pub async fn select_option_by_text(
        &self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_text = text.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(function() {{ \
             var s = document.querySelector('{escaped_selector}'); \
             if (!s) return false; \
             for (var i = 0; i < s.options.length; i++) {{ \
                 if (s.options[i].text === '{escaped_text}') {{ \
                     s.selectedIndex = i; \
                     s.dispatchEvent(new Event('change', {{bubbles: true}})); \
                     return true; \
                 }} \
             }} \
             return false; \
             }})()"
        );
        let result = self.evaluate(&script).await?;
        if result.get("value").and_then(|v| v.as_bool()) != Some(true) {
            return Err(SeleniumBaseError::InvalidSelector(format!(
                "Option with text '{text}' not found in {selector}"
            )));
        }
        Ok(())
    }

    /// Selects the `<option>` with the given value inside `<select>`.
    pub async fn select_option_by_value(
        &self,
        selector: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_value = value.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(function() {{ \
             var s = document.querySelector('{escaped_selector}'); \
             if (!s) return false; \
             s.value = '{escaped_value}'; \
             s.dispatchEvent(new Event('change', {{bubbles: true}})); \
             return true; \
             }})()"
        );
        let result = self.evaluate(&script).await?;
        if result.get("value").and_then(|v| v.as_bool()) != Some(true) {
            return Err(SeleniumBaseError::InvalidSelector(format!(
                "Failed to set value '{value}' on {selector}"
            )));
        }
        Ok(())
    }

    /// Selects the `<option>` at `index` (zero-based) inside `<select>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] when the dropdown is
    /// missing or has no option at `index`.
    pub async fn select_option_by_index(
        &self,
        selector: &str,
        index: usize,
    ) -> Result<(), SeleniumBaseError> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(function() {{ \
             var s = document.querySelector('{escaped_selector}'); \
             if (!s) return false; \
             if ({index} >= s.options.length) return false; \
             s.selectedIndex = {index}; \
             s.dispatchEvent(new Event('change', {{bubbles: true}})); \
             return true; \
             }})()"
        );
        let result = self.evaluate(&script).await?;
        if result.get("value").and_then(|v| v.as_bool()) != Some(true) {
            return Err(SeleniumBaseError::InvalidSelector(format!(
                "Option at index {index} not found in {selector}"
            )));
        }
        Ok(())
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

/// `text` as a JavaScript string literal (a JSON string is one).
fn js_string(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

/// The expression that gives the first element matching `selector`, or `null`.
fn query_script(selector: &str) -> String {
    format!("document.querySelector({})", js_string(selector))
}

/// The expression that gives an array of every element matching `selector`.
fn query_all_script(selector: &str) -> String {
    format!(
        "Array.from(document.querySelectorAll({}))",
        js_string(selector)
    )
}

/// The expression that gives `[left, top, width, height]` of the first element
/// matching `selector` in viewport coordinates, or `null`.
fn center_script(selector: &str) -> String {
    format!(
        "(function() {{ var el = document.querySelector({}); \
         if (!el) return null; \
         var r = el.getBoundingClientRect(); \
         return [r.left, r.top, r.width, r.height]; }})()",
        js_string(selector)
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
    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
    format!("(function() {{ var el = document.querySelector('{escaped}'); return {inner}; }})()")
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
        assert_eq!(js_string("a'b\"c\\d\n"), r#""a'b\"c\\d\n""#);
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
        assert!(script.contains("[data-test=\\'foo]"));
    }
}
