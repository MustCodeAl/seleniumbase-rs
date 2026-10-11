//! Scripted pages for the tests of the scrape runtime.
//!
//! A [`Site`] makes a mocked browser behave like a small web server: each navigation is answered
//! with the next queued response (the last one repeats), the response is announced the way Chrome
//! announces it, and the rendered body is what the page script reads back. A CAPTCHA widget can be
//! shown on demand.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::sb_cdp::sync::locked;
use crate::sb_cdp::{Browser, MockCtrl, Page};

/// The ids a mocked browser gives the main frame of a tab: `T1` for the tab it starts with and `T2`
/// for one it opens. A navigation answer is announced for both, because the mock does not say which
/// tab navigates.
pub(crate) const MAIN_FRAMES: [&str; 2] = ["T1", "T2"];

/// The tab session a mocked browser attaches every tab to.
pub(crate) const MOCK_SESSION: &str = "S1";

/// The viewport the scripted page reports.
const VIEWPORT: [u32; 2] = [1280, 720];

/// Where the scripted CAPTCHA widget sits, as `(x, y, width, height)`.
const WIDGET_BOX: (f64, f64, f64, f64) = (100.0, 200.0, 300.0, 65.0);

/// One answer of a scripted site.
#[derive(Clone, Debug)]
pub(crate) struct Served {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: String,
}

impl Served {
    /// An answer with this status and body and no headers.
    pub(crate) fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_owned(),
        }
    }

    /// The same answer with one more header.
    pub(crate) fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

#[derive(Default)]
struct Script {
    pages: VecDeque<Served>,
    body: String,
    navigations: Vec<String>,
    widget: bool,
    local_storage: Option<Value>,
}

/// A mocked browser answering like a web server.
#[derive(Clone)]
pub(crate) struct Site {
    script: Arc<Mutex<Script>>,
}

impl Site {
    /// Makes `mock` answer navigations with `pages`, in order, the last one for ever.
    pub(crate) fn serve(mock: &MockCtrl, pages: Vec<Served>) -> Self {
        let site = Self {
            script: Arc::new(Mutex::new(Script {
                pages: pages.into(),
                ..Script::default()
            })),
        };
        site.install_navigation(mock);
        site.install_evaluation(mock);
        site
    }

    /// Queues one more answer after the ones already queued.
    pub(crate) fn push(&self, served: Served) {
        locked(&self.script).pages.push_back(served);
    }

    /// Every URL navigated to so far, oldest first.
    pub(crate) fn navigations(&self) -> Vec<String> {
        locked(&self.script).navigations.clone()
    }

    /// Shows or hides a CAPTCHA widget on the page.
    pub(crate) fn show_widget(&self, shown: bool) {
        locked(&self.script).widget = shown;
    }

    /// Sets what the page's `localStorage` holds, as a JSON object.
    pub(crate) fn set_local_storage(&self, entries: Value) {
        locked(&self.script).local_storage = Some(entries);
    }

    fn install_navigation(&self, mock: &MockCtrl) {
        let script = Arc::clone(&self.script);
        let ctrl = mock.clone();
        mock.on("Page.navigate", move |params| {
            let url = params["url"].as_str().unwrap_or_default().to_owned();
            let served = {
                let mut state = locked(&script);
                let next = if state.pages.len() > 1 {
                    state.pages.pop_front()
                } else {
                    state.pages.front().cloned()
                };
                let served = next.unwrap_or_else(|| Served::new(200, ""));
                state.body.clone_from(&served.body);
                state.navigations.push(url.clone());
                served
            };
            for frame in MAIN_FRAMES {
                ctrl.emit(
                    "Network.responseReceived",
                    response_params(frame, "Document", &url, &served),
                    Some(MOCK_SESSION),
                );
            }
            ctrl.emit(
                "Page.loadEventFired",
                json!({ "timestamp": 0.0 }),
                Some(MOCK_SESSION),
            );
            Ok(json!({ "frameId": "F1", "loaderId": "L1" }))
        });
    }

    fn install_evaluation(&self, mock: &MockCtrl) {
        let script = Arc::clone(&self.script);
        mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            let state = locked(&script);
            let value = if expression.contains("outerHTML") {
                json!(state.body)
            } else if expression.contains("innerWidth") {
                json!(VIEWPORT)
            } else if expression.contains("Object.fromEntries") {
                state.local_storage.clone().unwrap_or_else(|| json!({}))
            } else if expression.contains("location.origin") {
                json!(state.navigations.last().map(|url| origin_of(url)))
            } else if expression.contains("const selectors") && state.widget {
                let (x, y, width, height) = WIDGET_BOX;
                json!({ "kind": 0, "x": x, "y": y, "width": width, "height": height })
            } else {
                Value::Null
            };
            Ok(json!({ "result": { "value": value } }))
        });
    }
}

/// The parameters of a `Network.responseReceived` event.
pub(crate) fn response_params(frame: &str, kind: &str, url: &str, served: &Served) -> Value {
    let mut headers = serde_json::Map::new();
    for (name, value) in &served.headers {
        headers.insert(name.clone(), json!(value));
    }
    json!({
        "requestId": "R1",
        "loaderId": "L1",
        "frameId": frame,
        "type": kind,
        "response": { "url": url, "status": served.status, "headers": headers },
    })
}

/// A tab of a fresh mocked browser, and the control that scripts it.
pub(crate) async fn mocked_page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser
        .default_page()
        .await
        .expect("a mocked browser starts with a tab");
    (page, mock)
}

/// A cookie as `Network.getAllCookies` reports it; `expires` is `-1.0` for a session cookie.
pub(crate) fn cdp_cookie(name: &str, value: &str, domain: &str, expires: f64) -> Value {
    json!({
        "name": name,
        "value": value,
        "domain": domain,
        "path": "/",
        "expires": expires,
        "httpOnly": false,
        "secure": true,
        "sameSite": "Lax",
    })
}

/// The origin of `url`, or `null` when it has none.
fn origin_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    Some(parsed.origin().ascii_serialization())
}
