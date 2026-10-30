//! A DevTools Protocol connection that the caller carries.
//!
//! [`Browser::launch`](super::Browser::launch) and
//! [`Browser::connect`](super::Browser::connect) talk to Chrome over its
//! DevTools WebSocket. An app that embeds Chromium has a channel inside its own
//! process instead: CEF's `SendDevToolsMessage` with a DevTools message
//! observer, or WebView2's `CallDevToolsProtocolMethod` with its event
//! receivers. Such a channel opens no port and listens on nothing.
//! [`Browser::over_channel`](super::Browser::over_channel) drives a page over
//! one: commands leave through the [`CdpTransport`] the caller implements, and
//! whatever the browser answers comes back through the [`CdpInbox`] it hands
//! out.
//!
//! The channel is page-scoped, as both of those APIs are: it reaches the
//! DevTools agent of one page. Commands addressed to that page go out with no
//! `sessionId`, and events that arrive with none are that page's.
//!
//! # Examples
//!
//! A transport that hands every command to a task standing in for the
//! embedder's channel, which answers through the inbox:
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use seleniumbase_rs::sb_cdp::{Browser, CdpRequest, TransportError};
//! use serde_json::json;
//! use tokio::sync::mpsc;
//!
//! let (outgoing, mut agent) = mpsc::unbounded_channel::<CdpRequest>();
//! let (browser, inbox) = Browser::over_channel(
//!     move |request: CdpRequest| -> Result<(), TransportError> {
//!         outgoing.send(request).map_err(|_| "the channel is gone".into())
//!     },
//! );
//! // The embedder's side: CEF's observer or WebView2's completion handler.
//! tokio::spawn(async move {
//!     while let Some(request) = agent.recv().await {
//!         let result = match request.method.as_str() {
//!             "Runtime.evaluate" => json!({ "result": { "value": 2 } }),
//!             _ => json!({}),
//!         };
//!         inbox.respond(request.id, Ok(result));
//!     }
//! });
//!
//! let answer = browser.execute("Runtime.evaluate", json!({ "expression": "1 + 1" })).await?;
//! assert_eq!(answer["result"]["value"], 2);
//! # Ok(())
//! # }
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde_json::{json, Value};
use tokio::sync::{broadcast, oneshot};

use super::client::{await_reply, dispatch, CdpEvent, Pending, Reply};
use super::sync::locked;
use crate::error::SeleniumBaseError;

/// The target id of the page a channel reaches, as
/// [`Page::id`](super::Page::id) reports it.
pub(crate) const CHANNEL_TARGET: &str = "channel";

/// The session the library files the channel's own page under. It never goes
/// out: a command for it is sent with no `sessionId`.
pub(crate) const CHANNEL_SESSION: &str = "channel";

/// How many events a slow listener can fall behind before it misses some.
const EVENT_BACKLOG: usize = 1024;

/// The error a [`CdpTransport`] returns when it cannot hand a command over.
pub type TransportError = Box<dyn std::error::Error + Send + Sync>;

/// One DevTools Protocol command on its way to the browser.
#[derive(Debug, Clone, PartialEq)]
pub struct CdpRequest {
    /// The number the answer carries back: pass it to
    /// [`CdpInbox::respond`]. Unique for the life of the [`Browser`](super::Browser).
    pub id: u64,
    /// The command, such as `Emulation.setTimezoneOverride`.
    pub method: String,
    /// The command's parameters, a JSON object.
    pub params: Value,
    /// The session of a target attached through the channel, or `None` for
    /// the channel's own page.
    pub session_id: Option<String>,
}

impl CdpRequest {
    /// The message as the protocol writes it on a WebSocket:
    /// `{"id":..,"method":..,"params":..}`, with `"sessionId"` when the
    /// request has one. CEF's `SendDevToolsMessage` takes this text.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut message = json!({ "id": self.id, "method": self.method, "params": self.params });
        if let Some(session) = &self.session_id {
            message["sessionId"] = Value::String(session.clone());
        }
        message.to_string()
    }
}

/// Carries commands from a [`Browser`](super::Browser) to a browser at the
/// other end of a channel the caller owns.
///
/// An app that embeds Chromium reaches its webview's DevTools agent through
/// a channel in its own process: CEF's `SendDevToolsMessage` with a DevTools
/// message observer, or WebView2's `CallDevToolsProtocolMethod` with its event
/// receivers. Such a channel opens no port and listens on nothing.
/// [`Browser::over_channel`](super::Browser::over_channel) drives that page:
/// commands leave through the transport, and the answers and events the app
/// receives go to the [`CdpInbox`] it hands back.
///
/// The channel is page-scoped, as both of those APIs are. A command for the
/// channel's page has no `session_id`; one that names a session is for a
/// target attached through the channel. The library waits for each answer
/// before it hands over the next command that must run after it, so a
/// channel that runs commands out of order (WebView2 may) is fine.
///
/// A closure `Fn(CdpRequest) -> Result<(), TransportError>` is a transport;
/// see [`Browser::over_channel`](super::Browser::over_channel) for an example.
pub trait CdpTransport: Send + Sync + 'static {
    /// Hands one command over. It must return without waiting for the answer,
    /// which comes back later through the [`CdpInbox`].
    ///
    /// It is called from async code, on whatever thread runs it: hand the
    /// command to the embedder's UI thread if its API needs that.
    ///
    /// # Errors
    ///
    /// Any error says the command could not be handed over. It fails at once
    /// with [`SeleniumBaseError::BrowserDisconnected`] naming the command.
    fn send(&self, request: CdpRequest) -> Result<(), TransportError>;
}

impl<F> CdpTransport for F
where
    F: Fn(CdpRequest) -> Result<(), TransportError> + Send + Sync + 'static,
{
    fn send(&self, request: CdpRequest) -> Result<(), TransportError> {
        self(request)
    }
}

/// What a channel's link and its inbox share.
struct Shared {
    pending: Pending,
    events: broadcast::Sender<CdpEvent>,
    /// Cleared when either side closes the channel.
    open: AtomicBool,
}

impl Shared {
    /// Ends the channel: every command still waiting learns it at once.
    fn close(&self) {
        self.open.store(false, Ordering::SeqCst);
        // Dropping a command's reply sender is what tells its waiter that the
        // connection is gone (`BrowserDisconnected`), not that it was refused.
        locked(&self.pending).clear();
    }
}

/// Where a channel's answers and events come in.
///
/// Cheap to clone. Every method can be called from any thread and returns at
/// once, so it is safe in a callback that runs on the embedder's UI thread.
#[derive(Clone)]
pub struct CdpInbox {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for CdpInbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpInbox")
            .field("open", &self.is_open())
            .finish_non_exhaustive()
    }
}

impl CdpInbox {
    /// Completes the command numbered `id`: `Ok` with the command's `result`
    /// object, `Err` with the browser's error message.
    ///
    /// An answer nobody waits for (a command that already timed out, or an id
    /// this browser never sent) is dropped.
    pub fn respond(&self, id: u64, outcome: Result<Value, String>) {
        let waiter = locked(&self.shared.pending).remove(&id);
        if let Some(reply) = waiter {
            let _ = reply.send(outcome);
        }
    }

    /// Passes an event on to every listener. `session_id` is `None` for an
    /// event of the channel's own page.
    pub fn event(&self, method: &str, params: Value, session_id: Option<&str>) {
        // Nobody listening is the normal case, not an error.
        let _ = self.shared.events.send(CdpEvent {
            method: method.to_owned(),
            params,
            session_id: Some(session_id.unwrap_or(CHANNEL_SESSION).to_owned()),
        });
    }

    /// Takes one raw protocol message as the browser wrote it: an answer,
    /// which carries an `id`, or an event, which carries a `method`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] when the text is not a JSON
    /// object; nothing is delivered then.
    pub fn deliver(&self, message: &str) -> Result<(), SeleniumBaseError> {
        let mut value: Value = serde_json::from_str(message).map_err(|error| {
            SeleniumBaseError::cdp_driver(format!("a DevTools message that is not JSON: {error}"))
        })?;
        let Some(fields) = value.as_object_mut() else {
            return Err(SeleniumBaseError::cdp_driver(
                "a DevTools message that is not a JSON object",
            ));
        };
        if fields.contains_key("method") && !fields.contains_key("sessionId") {
            fields.insert("sessionId".to_owned(), json!(CHANNEL_SESSION));
        }
        dispatch(&value, &self.shared.pending, &self.shared.events);
        Ok(())
    }

    /// Ends the channel, as when the webview behind it is destroyed: commands
    /// still waiting fail with [`SeleniumBaseError::BrowserDisconnected`] at
    /// once, later ones fail too, and
    /// [`Browser::is_connected`](super::Browser::is_connected) turns `false`.
    pub fn close(&self) {
        self.shared.close();
    }

    /// Whether the channel is still open: neither this inbox nor the browser
    /// closed it.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.shared.open.load(Ordering::SeqCst)
    }
}

/// The library's side of a channel: what [`Client`](super::client::Client)
/// sends through.
pub(super) struct ChannelLink {
    transport: Box<dyn CdpTransport>,
    shared: Arc<Shared>,
    next_id: AtomicU64,
}

impl std::fmt::Debug for ChannelLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelLink").finish_non_exhaustive()
    }
}

impl ChannelLink {
    /// A link over `transport`, and the inbox its answers go into.
    pub(super) fn new(transport: impl CdpTransport) -> (Self, CdpInbox) {
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let shared = Arc::new(Shared {
            pending: Arc::new(StdMutex::new(std::collections::HashMap::new())),
            events,
            open: AtomicBool::new(true),
        });
        let inbox = CdpInbox {
            shared: Arc::clone(&shared),
        };
        let link = Self {
            transport: Box::new(transport),
            shared,
            next_id: AtomicU64::new(1),
        };
        (link, inbox)
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.shared.events.subscribe()
    }

    pub(super) fn is_open(&self) -> bool {
        self.shared.open.load(Ordering::SeqCst)
    }

    pub(super) fn close(&self) {
        self.shared.close();
    }

    /// Registers a command, hands it to the transport and returns the id and
    /// the receiver its answer arrives on.
    fn hand_over(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<(u64, Reply), SeleniumBaseError> {
        if !self.is_open() {
            return Err(SeleniumBaseError::browser_disconnected(format!(
                "the DevTools channel is closed, so {method} was not sent"
            )));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        // Registered first: the answer can come back before `send` returns.
        let (tx, rx) = oneshot::channel();
        locked(&self.shared.pending).insert(id, tx);
        let request = CdpRequest {
            id,
            method: method.to_owned(),
            params,
            session_id: session_id
                .filter(|session| *session != CHANNEL_SESSION)
                .map(str::to_owned),
        };
        if let Err(error) = self.transport.send(request) {
            locked(&self.shared.pending).remove(&id);
            return Err(SeleniumBaseError::browser_disconnected(format!(
                "sending {method} failed: {error}"
            )));
        }
        Ok((id, rx))
    }

    pub(super) async fn send(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, SeleniumBaseError> {
        let (id, rx) = self.hand_over(method, params, session_id)?;
        await_reply(&self.shared.pending, id, method, rx).await
    }

    /// Hands every command over in order, then waits for the answers in order.
    pub(super) async fn send_ordered(
        &self,
        commands: Vec<(&str, Value)>,
        session_id: Option<&str>,
    ) -> Result<Vec<Value>, SeleniumBaseError> {
        // One at a time: an embedder's channel need not run commands in the
        // order it took them (WebView2 says it may not), so each waits for
        // the answer to the one before.
        let mut results = Vec::with_capacity(commands.len());
        for (method, params) in commands {
            results.push(self.send(method, params, session_id).await?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_written_as_the_protocol_writes_it() {
        let mut request = CdpRequest {
            id: 7,
            method: "Page.enable".to_owned(),
            params: json!({}),
            session_id: None,
        };
        let plain: Value = serde_json::from_str(&request.to_json()).unwrap();
        assert_eq!(
            plain,
            json!({ "id": 7, "method": "Page.enable", "params": {} })
        );

        request.session_id = Some("S9".to_owned());
        let addressed: Value = serde_json::from_str(&request.to_json()).unwrap();
        assert_eq!(addressed["sessionId"], "S9");
    }

    #[test]
    fn the_channels_own_page_session_never_goes_out() {
        let sent = Arc::new(StdMutex::new(Vec::new()));
        let log = Arc::clone(&sent);
        let (link, _inbox) =
            ChannelLink::new(move |request: CdpRequest| -> Result<(), TransportError> {
                locked(&log).push(request);
                Ok(())
            });

        link.hand_over("Page.enable", json!({}), Some(CHANNEL_SESSION))
            .unwrap();
        link.hand_over("DOM.enable", json!({}), Some("FRAME-SESSION"))
            .unwrap();
        link.hand_over("Browser.getVersion", json!({}), None)
            .unwrap();

        let sessions: Vec<Option<String>> =
            locked(&sent).iter().map(|r| r.session_id.clone()).collect();
        assert_eq!(sessions, [None, Some("FRAME-SESSION".to_owned()), None]);
    }

    #[test]
    fn an_event_without_a_session_is_the_pages() {
        let (link, inbox) =
            ChannelLink::new(|_: CdpRequest| -> Result<(), TransportError> { Ok(()) });
        let mut events = link.subscribe();

        inbox.event("Page.loadEventFired", json!({}), None);
        inbox
            .deliver(r#"{"method":"Network.dataReceived","params":{"requestId":"1"}}"#)
            .unwrap();
        inbox
            .deliver(r#"{"method":"DOM.updated","params":{},"sessionId":"OTHER"}"#)
            .unwrap();

        let sessions: Vec<Option<String>> = (0..3)
            .map(|_| events.try_recv().unwrap().session_id)
            .collect();
        assert_eq!(
            sessions,
            [
                Some(CHANNEL_SESSION.to_owned()),
                Some(CHANNEL_SESSION.to_owned()),
                Some("OTHER".to_owned())
            ]
        );
    }

    #[test]
    fn a_closed_channel_hands_nothing_over() {
        let handed = Arc::new(StdMutex::new(0_usize));
        let count = Arc::clone(&handed);
        let (link, inbox) = ChannelLink::new(move |_: CdpRequest| -> Result<(), TransportError> {
            *locked(&count) += 1;
            Ok(())
        });

        inbox.close();
        let refused = link.hand_over("Page.enable", json!({}), None);

        assert!(
            matches!(refused, Err(SeleniumBaseError::BrowserDisconnected { .. })),
            "{refused:?}"
        );
        assert_eq!(*locked(&handed), 0);
        assert!(!link.is_open());
    }

    #[tokio::test]
    async fn ordered_commands_are_handed_over_one_answer_at_a_time() {
        use std::sync::atomic::AtomicUsize;

        let in_flight = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CdpRequest>();
        let (counting, peak) = (Arc::clone(&in_flight), Arc::clone(&most));
        let (link, inbox) =
            ChannelLink::new(move |request: CdpRequest| -> Result<(), TransportError> {
                let now = counting.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tx.send(request).map_err(|_closed| "gone".into())
            });
        let answered = Arc::clone(&in_flight);
        let agent = tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Some(request) = rx.recv().await {
                seen.push(request.method.clone());
                answered.fetch_sub(1, Ordering::SeqCst);
                inbox.respond(request.id, Ok(json!({ "n": seen.len() })));
                if seen.len() == 3 {
                    break;
                }
            }
            seen
        });

        let results = link
            .send_ordered(
                vec![
                    ("A.one", json!({})),
                    ("A.two", json!({})),
                    ("A.three", json!({})),
                ],
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            results,
            [json!({ "n": 1 }), json!({ "n": 2 }), json!({ "n": 3 })]
        );
        assert_eq!(agent.await.unwrap(), ["A.one", "A.two", "A.three"]);
        assert_eq!(most.load(Ordering::SeqCst), 1, "never two at once");
    }

    #[test]

    fn text_that_is_not_a_message_is_refused() {
        let (_link, inbox) =
            ChannelLink::new(|_: CdpRequest| -> Result<(), TransportError> { Ok(()) });

        assert!(inbox.deliver("not json").is_err());
        assert!(inbox.deliver("[1, 2]").is_err());
    }
}
