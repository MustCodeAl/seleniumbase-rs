//! CDP Page automation helpers and the [`CdpPage`] wrapper.
//!
//! These helpers issue Chrome DevTools Protocol commands directly through a
//! [`BrowserSession`][crate::browser::session::BrowserSession], bypassing the
//! WebDriver layer for low-level control over navigation, DOM queries, input
//! events, screenshots, and JavaScript evaluation.

use std::path::Path;

use base64::prelude::*;
use serde_json::{json, Value};

use crate::browser::session::BrowserSession;
use crate::error::SeleniumBaseError;

/// A node returned by CDP DOM queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdpNode {
    /// CDP `nodeId` used by DOM-domain commands such as `DOM.getBoxModel`.
    pub node_id: i64,
    /// Optional CDP `backendNodeId`.
    pub backend_node_id: Option<i64>,
    /// Optional CDP `objectId` from `Runtime` or `DOM` queries.
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

    /// Finds the first element matching `selector` via `DOM.querySelector`.
    pub async fn find_element(&self, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
        self.session.execute_cdp("DOM.enable").await.ok();
        let response = self
            .execute(
                "DOM.querySelector",
                json!({"nodeId": 1, "selector": selector}),
            )
            .await?;
        CdpNode::from_query_result(&response, selector).ok_or_else(|| {
            SeleniumBaseError::InvalidSelector(format!("Element not found: {selector}"))
        })
    }

    /// Finds all elements matching `selector` via `DOM.querySelectorAll`.
    pub async fn find_all(&self, selector: &str) -> Result<Vec<CdpNode>, SeleniumBaseError> {
        self.session.execute_cdp("DOM.enable").await.ok();
        let response = self
            .execute(
                "DOM.querySelectorAll",
                json!({"nodeId": 1, "selector": selector}),
            )
            .await?;
        let node_ids = response
            .get("nodeIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                SeleniumBaseError::Unsupported("DOM.querySelectorAll missing nodeIds".to_owned())
            })?;
        Ok(node_ids
            .iter()
            .filter_map(|v| v.as_i64().map(|id| CdpNode::from_node_id(id, selector)))
            .collect())
    }

    /// Clicks the center of the element matching `selector`.
    ///
    /// Uses `DOM.getBoxModel` to compute the element center and dispatches
    /// `mouseMoved`, `mousePressed`, and `mouseReleased` input events.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let node = self.find_element(selector).await?;
        let model = self
            .execute("DOM.getBoxModel", json!({"nodeId": node.node_id}))
            .await?;
        let (center_x, center_y) = box_model_center(&model)?;

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

    /// Focuses the element, clears its value, and inserts `text`.
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let node = self.find_element(selector).await?;
        self.execute("DOM.focus", json!({"nodeId": node.node_id}))
            .await
            .ok();

        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let clear_script = format!("document.querySelector('{escaped}').value = '';");
        self.evaluate(&clear_script).await.ok();

        self.execute("Input.insertText", json!({"text": text}))
            .await?;
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

/// Computes the center of an element from a `DOM.getBoxModel` response.
fn box_model_center(model: &Value) -> Result<(f64, f64), SeleniumBaseError> {
    let content = model
        .get("model")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .ok_or_else(|| {
            SeleniumBaseError::Unsupported("DOM.getBoxModel content missing".to_owned())
        })?;
    if content.len() < 8 {
        return Err(SeleniumBaseError::Unsupported(
            "Invalid box model content".to_owned(),
        ));
    }
    let coords: Vec<f64> = content.iter().filter_map(|v| v.as_f64()).collect();
    if coords.len() < 8 {
        return Err(SeleniumBaseError::Unsupported(
            "Invalid box model coordinates".to_owned(),
        ));
    }
    let center_x = (coords[0] + coords[2] + coords[4] + coords[6]) / 4.0;
    let center_y = (coords[1] + coords[3] + coords[5] + coords[7]) / 4.0;
    Ok((center_x, center_y))
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
    fn box_model_center_calculation() {
        let model = json!({
            "model": {
                "content": [0.0, 0.0, 100.0, 0.0, 100.0, 50.0, 0.0, 50.0]
            }
        });
        let (x, y) = box_model_center(&model).unwrap();
        assert!((x - 50.0).abs() < f64::EPSILON);
        assert!((y - 25.0).abs() < f64::EPSILON);
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
