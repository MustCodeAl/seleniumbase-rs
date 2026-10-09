//! CDP network reactor for intercepting and mutating requests in real time.

use std::collections::HashMap;

use futures_util::{Sink, SinkExt, Stream, StreamExt};
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
        let ws_url = discover_ws_url(host, port).await.inspect_err(|e| {
            e.log_in_context("CdpReactor::start");
        })?;
        let (mut ws_stream, _) = connect_async(&ws_url).await.map_err(|e| {
            let err = SeleniumBaseError::browser_disconnected(format!(
                "CDP WebSocket connect to {ws_url} failed: {e}"
            ));
            err.log_in_context("CdpReactor::start");
            err
        })?;

        // Enable Fetch domain and wait for the command acknowledgement.
        let enable = json!({
            "id": 1,
            "method": "Fetch.enable",
            "params": { "patterns": [{ "urlPattern": "*" }] }
        });
        ws_stream
            .send(Message::Text(enable.to_string().into()))
            .await
            .map_err(|e| SeleniumBaseError::cdp_driver(format!("Fetch.enable send failed: {e}")))?;

        loop {
            let msg = ws_stream
                .next()
                .await
                .ok_or_else(|| {
                    SeleniumBaseError::browser_disconnected(
                        "CDP stream closed while waiting for Fetch.enable response".to_owned(),
                    )
                })?
                .map_err(|e| SeleniumBaseError::cdp_driver(format!("recv failed: {e}")))?;
            if let Message::Text(text) = msg {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id").and_then(|v| v.as_u64()) == Some(1) {
                        break;
                    }
                }
            }
        }

        let (write, read) = ws_stream.split();
        let handle = tokio::spawn(answer_paused_requests(read, write, header_overrides));

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

/// Continues every paused request with `header_overrides` applied, until the
/// connection closes or fails.
async fn answer_paused_requests<R, W>(
    mut read: R,
    mut write: W,
    header_overrides: HashMap<String, String>,
) where
    R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    W: Sink<Message> + Unpin,
{
    // Id 1 was `Fetch.enable`.
    let mut next_id: u64 = 2;
    // `None` means the socket closed; an error means it is no longer usable.
    // Either way there is nothing left to answer, so stop instead of polling
    // a dead stream.
    while let Some(Ok(message)) = read.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if !is_request_paused(&value) {
            continue;
        }
        let Some(params) = value.get("params") else {
            continue;
        };
        let Some(request_id) = params.get("requestId").and_then(Value::as_str) else {
            continue;
        };
        let cmd = build_continue_request(
            next_id,
            request_id,
            &header_overrides,
            params.get("request").and_then(|r| r.get("headers")),
        );
        next_id += 1;
        if write
            .send(Message::Text(cmd.to_string().into()))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn discover_ws_url(host: &str, port: u16) -> Result<String, SeleniumBaseError> {
    let url = format!("http://{host}:{port}/json/version");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| SeleniumBaseError::cdp_driver(format!("http client: {e}")))?;
    let response =
        client.get(&url).send().await.map_err(|e| {
            SeleniumBaseError::network(url.clone(), 0, format!("version request: {e}"))
        })?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(SeleniumBaseError::network(
            url,
            status,
            "non-success status from /json/version".to_owned(),
        ));
    }
    let json: Value = response
        .json()
        .await
        .map_err(|e| SeleniumBaseError::cdp_driver(format!("version json: {e}")))?;
    json.get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| {
            SeleniumBaseError::cdp_driver(format!(
                "webSocketDebuggerUrl missing in response from {url}"
            ))
        })
}

fn is_request_paused(value: &Value) -> bool {
    value.get("method").and_then(|m| m.as_str()) == Some("Fetch.requestPaused")
}

/// Builds the `Fetch.continueRequest` for one paused request.
///
/// The protocol's `headers` list replaces every header of the request, so the
/// request's own headers (`original`, an object of name to value) are kept and
/// each override replaces the header of the same name, whatever its case.
fn build_continue_request(
    id: u64,
    request_id: &str,
    overrides: &HashMap<String, String>,
    original: Option<&Value>,
) -> Value {
    let mut params = json!({ "requestId": request_id });
    if !overrides.is_empty() {
        let mut headers: Vec<(String, String)> = original
            .and_then(Value::as_object)
            .map(|own| {
                own.iter()
                    .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        for (name, value) in overrides {
            headers.retain(|(own, _)| !own.eq_ignore_ascii_case(name));
            headers.push((name.clone(), value.clone()));
        }
        headers.sort();
        params["headers"] = headers
            .into_iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect();
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
        let cmd = build_continue_request(7, "abc", &overrides, None);
        assert_eq!(cmd["id"], 7);
        assert_eq!(cmd["method"], "Fetch.continueRequest");
        assert_eq!(cmd["params"]["requestId"], "abc");
        let headers = cmd["params"]["headers"].as_array().unwrap();
        assert!(headers
            .iter()
            .any(|h| h["name"] == "Accept-Language" && h["value"] == "en-US"));
    }

    #[test]
    fn overrides_keep_the_headers_the_request_already_had() {
        let overrides = HashMap::from([("accept-language".to_owned(), "de".to_owned())]);
        let own = json!({ "Accept-Language": "en", "User-Agent": "UA", "Accept": "*/*" });
        let cmd = build_continue_request(2, "r", &overrides, Some(&own));
        let headers = cmd["params"]["headers"].as_array().unwrap();
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|h| (h["name"].as_str().unwrap(), h["value"].as_str().unwrap()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("Accept", "*/*"),
                ("User-Agent", "UA"),
                ("accept-language", "de")
            ]
        );
    }

    #[test]
    fn no_overrides_leaves_the_request_alone() {
        let cmd = build_continue_request(2, "r", &HashMap::new(), Some(&json!({ "A": "1" })));
        assert!(cmd["params"].get("headers").is_none());
    }

    #[tokio::test]
    async fn the_task_ends_when_the_connection_does() {
        use std::sync::{Arc, Mutex};

        let sent = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = {
            let sent = Arc::clone(&sent);
            futures_util::sink::unfold((), move |(), message: Message| {
                let sent = Arc::clone(&sent);
                async move {
                    if let Message::Text(text) = message {
                        sent.lock().unwrap().push(text.to_string());
                    }
                    Ok::<(), std::convert::Infallible>(())
                }
            })
        };
        let paused = json!({
            "method": "Fetch.requestPaused",
            "params": { "requestId": "q1", "request": { "headers": { "Host": "h" } } }
        });
        let incoming = futures_util::stream::iter(vec![
            Ok(Message::Ping(Vec::new().into())),
            Ok(Message::Text("not json".into())),
            Ok(Message::Text(paused.to_string().into())),
        ]);
        let overrides = HashMap::from([("X-Test".to_owned(), "1".to_owned())]);

        // The stream ends after three frames. Before the fix this never
        // returned: the loop kept polling the finished stream.
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            answer_paused_requests(incoming, Box::pin(sink), overrides),
        )
        .await
        .expect("the task stops when the stream ends");

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "{sent:?}");
        let command: Value = serde_json::from_str(&sent[0]).unwrap();
        assert_eq!(command["id"], 2);
        assert_eq!(command["params"]["requestId"], "q1");
        let names: Vec<&str> = command["params"]["headers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Host", "X-Test"]);
    }
}
