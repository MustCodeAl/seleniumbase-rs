//! The DevTools Protocol connection behind [`Browser`](super::Browser).
//!
//! One WebSocket carries every tab's traffic. A background task reads it,
//! completes the pending request that matches each response `id`, and
//! broadcasts everything else as an event, so commands issued from different
//! tabs never block one another.
//!
//! The connection is an enum rather than a trait: either a real socket, or
//! (with the `test-util` feature) a scripted [`MockCtrl`](super::MockCtrl).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use super::sync::locked;
use crate::error::SeleniumBaseError;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Pending = Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;
/// Where one command's response arrives.
type Reply = oneshot::Receiver<Result<Value, String>>;

/// How long a single protocol command may take before it is abandoned.
///
/// Generous because commands such as `Page.printToPDF` legitimately take
/// seconds; navigation has its own, separate limit.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Capacity of the event channel. A subscriber that falls this far behind
/// misses events rather than slowing the reader task down.
const EVENT_BACKLOG: usize = 1024;

/// A protocol event pushed by the browser, such as `Page.loadEventFired`.
#[derive(Debug, Clone, PartialEq)]
pub struct CdpEvent {
    /// The event name, for example `Page.loadEventFired`.
    pub method: String,
    /// The event payload.
    pub params: Value,
    /// The tab session the event belongs to, if it came from a tab.
    pub session_id: Option<String>,
}

/// A stream of protocol events.
///
/// Create one *before* triggering the action you want to observe, otherwise
/// the event can arrive before anything is listening.
#[derive(Debug)]
pub struct Events {
    rx: broadcast::Receiver<CdpEvent>,
    session: Option<Arc<str>>,
}

impl Events {
    /// Keeps only the events that belong to one tab's session.
    pub(crate) fn for_session(mut self, session: &str) -> Self {
        self.session = Some(session.into());
        self
    }

    /// Returns the next event, or `None` once the browser connection closes.
    ///
    /// A listener that falls too far behind skips the events it missed.
    pub async fn next(&mut self) -> Option<CdpEvent> {
        loop {
            match self.rx.recv().await {
                Ok(event) => {
                    let wanted = self
                        .session
                        .as_deref()
                        .is_none_or(|session| event.session_id.as_deref() == Some(session));
                    if wanted {
                        return Some(event);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Waits for the next event named `method`, skipping all others.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if it does not arrive in
    /// `timeout`, and [`SeleniumBaseError::BrowserDisconnected`] if the
    /// connection closes first.
    pub async fn wait_for(
        &mut self,
        method: &str,
        timeout: Duration,
    ) -> Result<CdpEvent, SeleniumBaseError> {
        let wait = async {
            loop {
                match self.next().await {
                    Some(event) if event.method == method => return Ok(event),
                    Some(_) => {}
                    None => {
                        return Err(SeleniumBaseError::browser_disconnected(format!(
                            "connection closed while waiting for {method}"
                        )))
                    }
                }
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .unwrap_or_else(|_| {
                Err(SeleniumBaseError::wait_timeout(
                    method.to_owned(),
                    Some(timeout),
                ))
            })
    }
}

/// A connection to a browser's DevTools endpoint.
#[derive(Debug)]
pub(crate) struct Client {
    link: Link,
}

#[derive(Debug)]
enum Link {
    Ws(WsLink),
    #[cfg(any(test, feature = "test-util"))]
    Mock(super::mock::MockCtrl),
}

struct WsLink {
    sink: Mutex<SplitSink<Ws, Message>>,
    pending: Pending,
    next_id: AtomicU64,
    events: broadcast::Sender<CdpEvent>,
    reader: JoinHandle<()>,
}

impl std::fmt::Debug for WsLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WsLink").finish_non_exhaustive()
    }
}

impl Client {
    /// Connects to a DevTools WebSocket such as
    /// `ws://127.0.0.1:9222/devtools/browser/<id>`.
    pub(crate) async fn connect(ws_url: &str) -> Result<Self, SeleniumBaseError> {
        let (stream, _) = connect_async(ws_url).await.map_err(|e| {
            SeleniumBaseError::cdp_driver(format!("WebSocket connect to {ws_url} failed: {e}"))
        })?;
        let (sink, source) = stream.split();
        let pending: Pending = Arc::new(StdMutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let reader = tokio::spawn(read_loop(source, Arc::clone(&pending), events.clone()));
        Ok(Self {
            link: Link::Ws(WsLink {
                sink: Mutex::new(sink),
                pending,
                next_id: AtomicU64::new(1),
                events,
                reader,
            }),
        })
    }

    #[cfg(any(test, feature = "test-util"))]
    pub(crate) fn mocked(ctrl: super::mock::MockCtrl) -> Self {
        Self {
            link: Link::Mock(ctrl),
        }
    }

    /// Subscribes to protocol events from every tab.
    pub(crate) fn events(&self) -> Events {
        let rx = match &self.link {
            Link::Ws(ws) => ws.events.subscribe(),
            #[cfg(any(test, feature = "test-util"))]
            Link::Mock(ctrl) => ctrl.subscribe(),
        };
        Events { rx, session: None }
    }

    /// Sends a command and waits for its response.
    ///
    /// `session_id` addresses a tab; `None` addresses the browser itself.
    pub(crate) async fn send(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, SeleniumBaseError> {
        match &self.link {
            Link::Ws(ws) => ws.send(method, params, session_id).await,
            #[cfg(any(test, feature = "test-util"))]
            Link::Mock(ctrl) => ctrl
                .handle(method, &params, session_id)
                .map_err(|message| SeleniumBaseError::cdp_driver(format!("{method}: {message}"))),
        }
    }

    /// Sends commands one after another without waiting for a response in
    /// between, then waits for all of them, and returns the results in order.
    ///
    /// The browser runs commands for a tab in the order they arrive, so the
    /// effect is the same as sending them one at a time. What changes is the
    /// time: a command that Chrome answers late no longer holds up the ones
    /// behind it. If any command fails, the first error is returned; the others
    /// were already sent and still run.
    pub(crate) async fn send_ordered(
        &self,
        commands: Vec<(&str, Value)>,
        session_id: Option<&str>,
    ) -> Result<Vec<Value>, SeleniumBaseError> {
        match &self.link {
            Link::Ws(ws) => ws.send_ordered(commands, session_id).await,
            #[cfg(any(test, feature = "test-util"))]
            Link::Mock(ctrl) => {
                let mut results = Vec::with_capacity(commands.len());
                let mut first_error = None;
                for (method, params) in commands {
                    match ctrl.handle(method, &params, session_id) {
                        Ok(result) => results.push(result),
                        Err(message) => {
                            first_error.get_or_insert_with(|| {
                                SeleniumBaseError::cdp_driver(format!("{method}: {message}"))
                            });
                        }
                    }
                }
                first_error.map_or(Ok(results), Err)
            }
        }
    }

    /// Whether the connection is still usable.
    pub(crate) fn is_open(&self) -> bool {
        match &self.link {
            Link::Ws(ws) => !ws.reader.is_finished(),
            #[cfg(any(test, feature = "test-util"))]
            Link::Mock(ctrl) => !ctrl.is_disconnected(),
        }
    }

    /// Marks a mocked connection as closed; a real one closes with its process.
    #[cfg(any(test, feature = "test-util"))]
    pub(crate) fn mark_closed(&self) {
        if let Link::Mock(ctrl) = &self.link {
            ctrl.disconnect();
        }
    }
}

impl WsLink {
    /// Registers a command and returns its id, the frame to write, and the
    /// receiver its response will arrive on.
    ///
    /// Registering comes first: the response can beat the insert otherwise.
    fn register(
        &self,
        method: &str,
        params: &Value,
        session_id: Option<&str>,
    ) -> (u64, Message, Reply) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut payload = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            payload["sessionId"] = Value::String(session_id.to_owned());
        }
        let (tx, rx) = oneshot::channel();
        locked(&self.pending).insert(id, tx);
        (id, Message::Text(payload.to_string().into()), rx)
    }

    async fn send(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, SeleniumBaseError> {
        let (id, frame, rx) = self.register(method, &params, session_id);
        let sent = self.sink.lock().await.send(frame).await;
        if let Err(error) = sent {
            self.forget(id);
            return Err(SeleniumBaseError::browser_disconnected(format!(
                "sending {method} failed: {error}"
            )));
        }
        self.finish(id, method, rx).await
    }

    /// Writes every command under one hold of the socket, so nothing can come
    /// between them, then waits for the responses in order.
    async fn send_ordered(
        &self,
        commands: Vec<(&str, Value)>,
        session_id: Option<&str>,
    ) -> Result<Vec<Value>, SeleniumBaseError> {
        let mut waiting: Vec<(u64, &str, Reply)> = Vec::with_capacity(commands.len());
        {
            let mut sink = self.sink.lock().await;
            let mut written = Ok(());
            let mut failed = "";
            for (method, params) in commands {
                let (id, frame, rx) = self.register(method, &params, session_id);
                waiting.push((id, method, rx));
                written = sink.feed(frame).await;
                if written.is_err() {
                    failed = method;
                    break;
                }
            }
            if written.is_ok() {
                written = sink.flush().await;
                failed = "the batch";
            }
            if let Err(error) = written {
                for (id, _, _) in &waiting {
                    self.forget(*id);
                }
                return Err(SeleniumBaseError::browser_disconnected(format!(
                    "sending {failed} failed: {error}"
                )));
            }
        }
        let mut results = Vec::with_capacity(waiting.len());
        let mut remaining = waiting.into_iter();
        while let Some((id, method, rx)) = remaining.next() {
            match self.finish(id, method, rx).await {
                Ok(result) => results.push(result),
                Err(error) => {
                    // The rest already ran or will; their answers are not wanted.
                    for (id, _, _) in remaining {
                        self.forget(id);
                    }
                    return Err(error);
                }
            }
        }
        Ok(results)
    }

    /// Waits for the response to a command that has been written.
    async fn finish(&self, id: u64, method: &str, rx: Reply) -> Result<Value, SeleniumBaseError> {
        match tokio::time::timeout(COMMAND_TIMEOUT, rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(message))) => Err(SeleniumBaseError::cdp_driver(format!(
                "{method}: {message}"
            ))),
            Ok(Err(_)) => Err(SeleniumBaseError::browser_disconnected(format!(
                "connection closed while waiting for {method}"
            ))),
            Err(_) => {
                self.forget(id);
                Err(SeleniumBaseError::wait_timeout(
                    format!("protocol command {method}"),
                    Some(COMMAND_TIMEOUT),
                ))
            }
        }
    }

    fn forget(&self, id: u64) {
        locked(&self.pending).remove(&id);
    }
}

impl Drop for WsLink {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

/// Reads frames until the socket closes, routing each to its destination.
async fn read_loop(
    mut source: SplitStream<Ws>,
    pending: Pending,
    events: broadcast::Sender<CdpEvent>,
) {
    while let Some(frame) = source.next().await {
        match frame {
            Ok(Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    dispatch(&value, &pending, &events);
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }

    // The socket is gone: fail everything still waiting so callers do not hang
    // until their own timeout.
    let waiting: Vec<_> = locked(&pending).drain().collect();
    for (_, tx) in waiting {
        let _ = tx.send(Err("connection closed".to_owned()));
    }
}

/// Routes one decoded message: a response completes its request, anything with
/// a `method` is an event.
fn dispatch(value: &Value, pending: &Pending, events: &broadcast::Sender<CdpEvent>) {
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let waiter = locked(pending).remove(&id);
        if let Some(tx) = waiter {
            let outcome = match value.get("error") {
                Some(error) => Err(error
                    .get("message")
                    .and_then(Value::as_str)
                    .map_or_else(|| error.to_string(), str::to_owned)),
                None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = tx.send(outcome);
        }
        return;
    }
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        // Nobody listening is the normal case, not an error.
        let _ = events.send(CdpEvent {
            method: method.to_owned(),
            params: value.get("params").cloned().unwrap_or(Value::Null),
            session_id: value
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_with(id: u64) -> (Pending, oneshot::Receiver<Result<Value, String>>) {
        let (tx, rx) = oneshot::channel();
        let pending: Pending = Arc::new(StdMutex::new(HashMap::new()));
        pending.lock().unwrap().insert(id, tx);
        (pending, rx)
    }

    #[test]
    fn a_response_completes_only_the_request_it_answers() {
        let (pending, mut rx) = pending_with(7);
        let (events, _keep) = broadcast::channel(4);
        dispatch(&json!({"id": 99, "result": {}}), &pending, &events);
        assert!(
            rx.try_recv().is_err(),
            "an unrelated response must not complete request 7"
        );
        dispatch(&json!({"id": 7, "result": {"ok": true}}), &pending, &events);
        assert_eq!(rx.try_recv().unwrap().unwrap(), json!({"ok": true}));
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    fn a_protocol_error_carries_the_browsers_message() {
        let (pending, mut rx) = pending_with(1);
        let (events, _keep) = broadcast::channel(4);
        dispatch(
            &json!({"id": 1, "error": {"code": -32000, "message": "No node with given id"}}),
            &pending,
            &events,
        );
        assert_eq!(rx.try_recv().unwrap().unwrap_err(), "No node with given id");
    }

    #[test]
    fn an_event_reaches_subscribers_without_consuming_a_request() {
        let (pending, _rx) = pending_with(1);
        let (events, mut sub) = broadcast::channel(4);
        dispatch(
            &json!({"method": "Page.loadEventFired", "params": {"timestamp": 1.5}, "sessionId": "S1"}),
            &pending,
            &events,
        );
        let event = sub.try_recv().unwrap();
        assert_eq!(event.method, "Page.loadEventFired");
        assert_eq!(event.session_id.as_deref(), Some("S1"));
        assert_eq!(pending.lock().unwrap().len(), 1);
    }
}
