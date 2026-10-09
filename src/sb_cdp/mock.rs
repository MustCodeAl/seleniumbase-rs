//! A scripted browser for testing code that drives a [`Page`](super::Page).
//!
//! [`Browser::new_mocked`](super::Browser::new_mocked) returns a browser that
//! never launches Chrome. Every protocol command it receives is recorded, and
//! answered by whichever handler the test registered for that command with
//! [`MockCtrl::on`] or [`MockCtrl::reply`].
//!
//! Out of the box a mocked browser has one blank tab, creates and attaches to
//! further tabs on request, and completes every navigation immediately. Any
//! other command succeeds with an empty result until a test says otherwise.
//!
//! # Examples
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use seleniumbase_rs::sb_cdp::Browser;
//! use serde_json::json;
//!
//! let (browser, mock) = Browser::new_mocked();
//! mock.reply("Runtime.evaluate", json!({ "result": { "value": "Example Domain" } }));
//!
//! let page = browser.default_page().await?;
//! assert_eq!(page.title().await?, "Example Domain");
//! assert!(!mock.calls_to("Runtime.evaluate").is_empty());
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::sync::broadcast;

use super::client::CdpEvent;
use super::sync::locked;

/// How a test answers one protocol command.
type Handler = Box<dyn FnMut(&Value) -> Result<Value, String> + Send>;

/// One protocol command a mocked browser received.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The command name, such as `Input.dispatchMouseEvent`.
    pub method: String,
    /// The command parameters.
    pub params: Value,
    /// The tab session it was sent to, or `None` for the browser itself.
    pub session_id: Option<String>,
}

/// Scripts and inspects a mocked browser.
#[derive(Clone)]
pub struct MockCtrl {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    events: broadcast::Sender<CdpEvent>,
}

#[derive(Default)]
struct State {
    /// Searched newest-first, so a test can override any default.
    handlers: Vec<(String, Handler)>,
    calls: Vec<Call>,
}

impl std::fmt::Debug for MockCtrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let calls = self.inner.state.lock().map_or(0, |s| s.calls.len());
        f.debug_struct("MockCtrl")
            .field("calls", &calls)
            .finish_non_exhaustive()
    }
}

impl MockCtrl {
    pub(crate) fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        let ctrl = Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                events,
            }),
        };
        ctrl.install_defaults();
        ctrl
    }

    fn install_defaults(&self) {
        self.reply(
            "Target.getTargets",
            json!({ "targetInfos": [{
                "targetId": "T1", "type": "page", "url": "about:blank", "title": "",
            }]}),
        );
        self.reply("Target.createTarget", json!({ "targetId": "T2" }));
        self.reply("Target.attachToTarget", json!({ "sessionId": "S1" }));
        // Navigations finish at once, as they do when a page is already cached.
        for method in [
            "Page.navigate",
            "Page.reload",
            "Page.navigateToHistoryEntry",
        ] {
            let ctrl = self.clone();
            self.on(method, move |_| {
                ctrl.emit(
                    "Page.loadEventFired",
                    json!({ "timestamp": 0.0 }),
                    Some("S1"),
                );
                Ok(json!({ "frameId": "F1", "loaderId": "L1" }))
            });
        }
        self.reply(
            "Page.getNavigationHistory",
            json!({ "currentIndex": 0, "entries": [{ "id": 1, "url": "about:blank" }] }),
        );
    }

    /// Answers every `method` command by calling `handler` with its parameters.
    ///
    /// A later registration for the same command replaces an earlier one.
    pub fn on(
        &self,
        method: &str,
        handler: impl FnMut(&Value) -> Result<Value, String> + Send + 'static,
    ) {
        locked(&self.inner.state)
            .handlers
            .push((method.to_owned(), Box::new(handler)));
    }

    /// Answers every `method` command with `result`.
    pub fn reply(&self, method: &str, result: Value) {
        self.on(method, move |_| Ok(result.clone()));
    }

    /// Makes every `method` command fail with a protocol error saying `message`.
    pub fn fail(&self, method: &str, message: &str) {
        let message = message.to_owned();
        self.on(method, move |_| Err(message.clone()));
    }

    /// Every command received so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        locked(&self.inner.state).calls.clone()
    }

    /// Every received `method` command, oldest first.
    #[must_use]
    pub fn calls_to(&self, method: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|call| call.method == method)
            .collect()
    }

    /// Pushes a protocol event to everything listening, as the browser would.
    pub fn emit(&self, method: &str, params: Value, session_id: Option<&str>) {
        // Nobody listening is fine for a test.
        let _ = self.inner.events.send(CdpEvent {
            method: method.to_owned(),
            params,
            session_id: session_id.map(str::to_owned),
        });
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.inner.events.subscribe()
    }

    pub(crate) fn handle(
        &self,
        method: &str,
        params: &Value,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        let mut state = locked(&self.inner.state);
        state.calls.push(Call {
            method: method.to_owned(),
            params: params.clone(),
            session_id: session_id.map(str::to_owned),
        });
        state
            .handlers
            .iter_mut()
            .rev()
            .find(|(registered, _)| registered == method)
            .map_or_else(|| Ok(json!({})), |(_, handler)| handler(params))
    }
}
