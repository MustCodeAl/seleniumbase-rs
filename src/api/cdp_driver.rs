//! Lightweight async CDP driver that connects directly to a browser's
//! debugging WebSocket endpoint.
//!
//! The driver discovers the WebSocket URL via the HTTP
//! `http://{host}:{port}/json/version` endpoint using `reqwest`, then speaks
//! the Chrome DevTools Protocol over a `tokio-tungstenite` WebSocket
//! connection.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::api::cdp_page::CdpNode;
use crate::error::SeleniumBaseError;

/// Type alias for the WebSocket stream returned by `connect_async`.
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A minimal CDP driver that connects to an existing browser debugging port.
pub struct CdpDriver {
    /// Discovered WebSocket debugger URL.
    ws_url: Option<String>,
    /// Active WebSocket connection.
    ws: Option<Arc<Mutex<WsStream>>>,
    /// HTTP client used to discover the debugger URL.
    http_client: reqwest::Client,
    /// Monotonically increasing CDP command id.
    command_id: AtomicU64,
}

impl Default for CdpDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CdpDriver {
    /// Creates a disconnected [`CdpDriver`].
    pub fn new() -> Self {
        Self {
            ws_url: None,
            ws: None,
            http_client: reqwest::Client::new(),
            command_id: AtomicU64::new(1),
        }
    }

    /// Discovers and connects to the browser debugging WebSocket at
    /// `host:port`.
    pub async fn start_async(&mut self, host: &str, port: u16) -> Result<(), SeleniumBaseError> {
        let version_url = format!("http://{host}:{port}/json/version");
        let response = self
            .http_client
            .get(&version_url)
            .send()
            .await
            .map_err(|e| SeleniumBaseError::CdpDriver(format!("reqwest failed: {e}")))?;
        let json: Value = response.json().await.map_err(|e| {
            SeleniumBaseError::CdpDriver(format!("failed to read version JSON: {e}"))
        })?;
        let ws_url = json
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SeleniumBaseError::CdpDriver("webSocketDebuggerUrl not found".to_owned())
            })?
            .to_owned();

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| SeleniumBaseError::CdpDriver(format!("WebSocket connect failed: {e}")))?;
        self.ws = Some(Arc::new(Mutex::new(ws_stream)));
        self.ws_url = Some(ws_url);
        Ok(())
    }

    /// Closes the WebSocket connection and clears the stored debugger URL.
    pub async fn stop(&mut self) -> Result<(), SeleniumBaseError> {
        if let Some(ws) = self.ws.take() {
            let mut ws = ws.lock().await;
            ws.close(None)
                .await
                .map_err(|e| SeleniumBaseError::CdpDriver(format!("close failed: {e}")))?;
        }
        self.ws_url = None;
        Ok(())
    }

    /// Builds a CDP command payload. Exposed for tests.
    pub fn build_command(&self, id: u64, method: &str, params: Value) -> Value {
        json!({"id": id, "method": method, "params": params})
    }

    /// Sends a CDP command and waits for the matching response.
    async fn send_command(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        let ws = self.ws.as_ref().ok_or_else(|| {
            SeleniumBaseError::CdpDriver("CdpDriver has not been started".to_owned())
        })?;
        let id = self.command_id.fetch_add(1, Ordering::SeqCst);
        let payload = self.build_command(id, method, params);
        let text = payload.to_string();

        let mut ws = ws.lock().await;
        ws.send(Message::Text(text.into()))
            .await
            .map_err(|e| SeleniumBaseError::CdpDriver(format!("send failed: {e}")))?;

        loop {
            let message = ws
                .next()
                .await
                .ok_or_else(|| {
                    SeleniumBaseError::CdpDriver("WebSocket closed unexpectedly".to_owned())
                })?
                .map_err(|e| SeleniumBaseError::CdpDriver(format!("receive failed: {e}")))?;

            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|e| SeleniumBaseError::CdpDriver(format!("invalid JSON: {e}")))?;
                if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    if let Some(error) = value.get("error") {
                        return Err(SeleniumBaseError::CdpDriver(format!("CDP error: {error}")));
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
            }
        }
    }

    /// Navigates to `url` via `Page.navigate`.
    pub async fn get(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.send_command("Page.navigate", json!({"url": url}))
            .await?;
        Ok(())
    }

    /// Finds the first element matching `selector` via `DOM.querySelector`.
    pub async fn find(&self, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
        self.send_command("DOM.enable", Value::Null).await.ok();
        let response = self
            .send_command(
                "DOM.querySelector",
                json!({"nodeId": 1, "selector": selector}),
            )
            .await?;
        CdpNode::from_query_result(&response, selector).ok_or_else(|| {
            SeleniumBaseError::InvalidSelector(format!("Element not found: {selector}"))
        })
    }

    /// Clicks the center of the element matching `selector`.
    pub async fn click(&mut self, selector: &str) -> Result<(), SeleniumBaseError> {
        let node = self.find(selector).await?;
        let model = self
            .send_command("DOM.getBoxModel", json!({"nodeId": node.node_id}))
            .await?;
        let (center_x, center_y) = box_model_center(&model)?;

        for event_type in ["mouseMoved", "mousePressed", "mouseReleased"] {
            self.send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": event_type,
                    "x": center_x,
                    "y": center_y,
                    "button": "left",
                    "clickCount": 1,
                }),
            )
            .await
            .ok();
        }
        Ok(())
    }

    /// Focuses the element, clears it, and inserts `text`.
    pub async fn type_text(&mut self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let node = self.find(selector).await?;
        self.send_command("DOM.focus", json!({"nodeId": node.node_id}))
            .await
            .ok();

        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let clear_script = format!("document.querySelector('{escaped}').value = '';");
        self.evaluate(&clear_script).await.ok();

        self.send_command("Input.insertText", json!({"text": text}))
            .await?;
        Ok(())
    }

    /// Evaluates `expression` and returns the raw CDP result object.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let response = self
            .send_command(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await?;
        if let Some(exc) = response.get("exceptionDetails") {
            return Err(SeleniumBaseError::CdpDriver(format!("JS exception: {exc}")));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Captures a PNG screenshot and writes it to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        let response = self
            .send_command("Page.captureScreenshot", json!({"format": "png"}))
            .await?;
        let data = response
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SeleniumBaseError::CdpDriver("Screenshot data missing".to_owned()))?;
        let bytes = base64::prelude::BASE64_STANDARD.decode(data).map_err(|e| {
            SeleniumBaseError::CdpDriver(format!("Failed to decode screenshot: {e}"))
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
            SeleniumBaseError::CdpDriver("DOM.getBoxModel content missing".to_owned())
        })?;
    if content.len() < 8 {
        return Err(SeleniumBaseError::CdpDriver(
            "Invalid box model content".to_owned(),
        ));
    }
    let coords: Vec<f64> = content.iter().filter_map(|v| v.as_f64()).collect();
    if coords.len() < 8 {
        return Err(SeleniumBaseError::CdpDriver(
            "Invalid box model coordinates".to_owned(),
        ));
    }
    let center_x = (coords[0] + coords[2] + coords[4] + coords[6]) / 4.0;
    let center_y = (coords[1] + coords[3] + coords[5] + coords[7]) / 4.0;
    Ok((center_x, center_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_command_payload() {
        let driver = CdpDriver::new();
        let payload = driver.build_command(7, "Runtime.evaluate", json!({"expression": "1+1"}));
        assert_eq!(payload["id"], 7);
        assert_eq!(payload["method"], "Runtime.evaluate");
        assert_eq!(payload["params"]["expression"], "1+1");
    }

    #[test]
    fn box_model_center_calculation() {
        let model = json!({
            "model": {
                "content": [10.0, 20.0, 110.0, 20.0, 110.0, 70.0, 10.0, 70.0]
            }
        });
        let (x, y) = box_model_center(&model).unwrap();
        assert!((x - 60.0).abs() < f64::EPSILON);
        assert!((y - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn box_model_center_rejects_short_content() {
        let model = json!({"model": {"content": [1.0, 2.0, 3.0]}});
        assert!(box_model_center(&model).is_err());
    }
}
