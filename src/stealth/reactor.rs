//! CDP network reactor for intercepting and mutating requests in real time.

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::error::SeleniumBaseError;

/// Intercepts CDP `Fetch.requestPaused` events and mutates request headers.
pub struct CdpReactor {
    handle: Option<JoinHandle<()>>,
}

impl CdpReactor {
    /// Connects to the browser debugger at `host:port`, enables Fetch
    /// interception, and spawns a background task that continues every paused
    /// request with `header_overrides` applied.
    pub async fn start(
        host: &str,
        port: u16,
        header_overrides: HashMap<String, String>,
    ) -> Result<Self, SeleniumBaseError> {
        let ws_url = discover_ws_url(host, port).await?;
        let (mut ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| SeleniumBaseError::CdpDriver(format!("connect failed: {e}")))?;

        // Enable Fetch domain and wait for the command acknowledgement.
        let enable = json!({
            "id": 1,
            "method": "Fetch.enable",
            "params": { "patterns": [{ "urlPattern": "*" }] }
        });
        ws_stream
            .send(Message::Text(enable.to_string().into()))
            .await
            .map_err(|e| SeleniumBaseError::CdpDriver(format!("send failed: {e}")))?;

        loop {
            let msg = ws_stream
                .next()
                .await
                .ok_or_else(|| SeleniumBaseError::CdpDriver("stream closed".to_owned()))?
                .map_err(|e| SeleniumBaseError::CdpDriver(format!("recv failed: {e}")))?;
            if let Message::Text(text) = msg {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id").and_then(|v| v.as_u64()) == Some(1) {
                        break;
                    }
                }
            }
        }

        let (mut write, mut read) = ws_stream.split();
        let mut next_id: u64 = 2;
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = read.next() => {
                        let Some(Ok(Message::Text(text))) = msg else { continue };
                        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                        if !is_request_paused(&value) {
                            continue;
                        }
                        let Some(request_id) = value
                            .get("params")
                            .and_then(|p| p.get("requestId"))
                            .and_then(|v| v.as_str())
                        else {
                            continue;
                        };
                        let cmd = build_continue_request(next_id, request_id, &header_overrides);
                        next_id += 1;
                        let _ = write.send(Message::Text(cmd.to_string().into())).await;
                    }
                }
            }
        });

        Ok(Self {
            handle: Some(handle),
        })
    }

    /// Aborts the background reactor task.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Drop for CdpReactor {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn discover_ws_url(host: &str, port: u16) -> Result<String, SeleniumBaseError> {
    let url = format!("http://{host}:{port}/json/version");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| SeleniumBaseError::CdpDriver(format!("http client: {e}")))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| SeleniumBaseError::CdpDriver(format!("version request: {e}")))?;
    let json: Value = response
        .json()
        .await
        .map_err(|e| SeleniumBaseError::CdpDriver(format!("version json: {e}")))?;
    json.get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| SeleniumBaseError::CdpDriver("webSocketDebuggerUrl missing".to_owned()))
}

fn is_request_paused(value: &Value) -> bool {
    value.get("method").and_then(|m| m.as_str()) == Some("Fetch.requestPaused")
}

fn build_continue_request(id: u64, request_id: &str, overrides: &HashMap<String, String>) -> Value {
    let mut params = json!({ "requestId": request_id });
    if !overrides.is_empty() {
        let headers: Vec<Value> = overrides
            .iter()
            .map(|(k, v)| json!({ "name": k, "value": v }))
            .collect();
        params["headers"] = json!(headers);
    }
    json!({ "id": id, "method": "Fetch.continueRequest", "params": params })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_request_paused() {
        let value = json!({ "method": "Fetch.requestPaused", "params": { "requestId": "123" } });
        assert!(is_request_paused(&value));
    }

    #[test]
    fn ignores_other_events() {
        let value = json!({ "method": "Page.loadEventFired" });
        assert!(!is_request_paused(&value));
    }

    #[test]
    fn continue_request_includes_headers() {
        let mut overrides = HashMap::new();
        overrides.insert("Accept-Language".to_owned(), "en-US".to_owned());
        let cmd = build_continue_request(7, "abc", &overrides);
        assert_eq!(cmd["id"], 7);
        assert_eq!(cmd["method"], "Fetch.continueRequest");
        assert_eq!(cmd["params"]["requestId"], "abc");
        let headers = cmd["params"]["headers"].as_array().unwrap();
        assert!(headers
            .iter()
            .any(|h| h["name"] == "Accept-Language" && h["value"] == "en-US"));
    }
}
