//! The Pure CDP server: [`sb_cdp`](crate::sb_cdp) as MCP tools.
//!
//! One Chrome session is shared by every call. `start_browser` launches it,
//! the other tools drive it, and `close_browser` ends it. Related actions are
//! grouped behind one tool with an `action`, `mode` or `state` argument, which
//! keeps the tool list short enough for a model to choose from. The developer
//! tools follow them; on this engine `get_failed_requests` can also watch the
//! tab's `Network` events.

mod challenges;
mod clicks;
mod conditions;
mod files;
mod forms;
mod navigation;
mod page;
mod script;
mod session;
mod storage;
mod tasks;
mod windows;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{json, Value};

use super::devtools::{self, DevSession};
use super::failure::shorten;
use super::{Closeable, Host, Prop, Settings, ToolDef};
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Browser, Page};

const SERVER_NAME: &str = "seleniumbase-cdp";

const INSTRUCTIONS: &str = "\
Drives Chrome over the DevTools Protocol with no WebDriver, so pages see an \
ordinary browser. Call start_browser once, use the other tools, then call \
close_browser. Selectors may be CSS (preferred), XPath, or link=text. \
Failed actions come back as reports you can act on: they name the page's state \
and the next tools to try. inspect_page outlines the page, diagnose_selector says \
why a selector fails, suggest_selectors finds one that works, and \
export_session_as_test turns the session into a Rust test.";

/// How long the pointer rests on a menu before the item under it is clicked.
const HOVER_PAUSE: Duration = Duration::from_millis(250);

/// How long a check that must not wait is allowed to look.
const NO_WAIT: Duration = Duration::ZERO;

/// The most failed requests a network watch keeps; older ones are dropped.
const MAX_FAILURES: usize = 200;

/// The most unfinished requests a network watch remembers the URL of.
const MAX_PENDING: usize = 2000;

/// The session behind the Pure CDP server: a browser and the tab in use.
#[derive(Debug)]
pub struct Cdp {
    browser: Browser,
    page: Page,
    network: Option<NetworkWatch>,
}

/// The failed requests of one tab, as its `Network` events report them.
#[derive(Debug, Default)]
struct NetworkLog {
    /// URL and resource type of requests that have not finished yet.
    pending: HashMap<String, (String, String)>,
    failures: VecDeque<Value>,
    dropped: usize,
}

impl NetworkLog {
    /// Takes note of one `Network` event.
    fn note(&mut self, method: &str, params: &Value) {
        let id = params["requestId"].as_str().unwrap_or_default();
        match method {
            "Network.requestWillBeSent" => {
                if self.pending.len() >= MAX_PENDING {
                    self.pending.clear();
                }
                let url = params["request"]["url"].as_str().unwrap_or_default();
                let kind = params["type"].as_str().unwrap_or_default();
                self.pending
                    .insert(id.to_owned(), (url.to_owned(), kind.to_owned()));
            }
            "Network.responseReceived" => {
                let status = params["response"]["status"].as_u64().unwrap_or(0);
                if status >= 400 {
                    self.push(&json!({
                        "url": params["response"]["url"],
                        "status": status,
                        "status_text": params["response"]["statusText"],
                        "type": params["type"],
                    }));
                }
            }
            "Network.loadingFailed" => {
                let (url, kind) = self.pending.remove(id).unwrap_or_default();
                self.push(&json!({
                    "url": url,
                    "type": params["type"].as_str().unwrap_or(&kind),
                    "error": params["errorText"],
                    "blocked_reason": params["blockedReason"],
                    "canceled": params["canceled"],
                }));
            }
            "Network.loadingFinished" => {
                self.pending.remove(id);
            }
            _ => {}
        }
    }

    fn push(&mut self, failure: &Value) {
        if self.failures.len() == MAX_FAILURES {
            self.failures.pop_front();
            self.dropped += 1;
        }
        self.failures.push_back(shorten(failure, 300));
    }
}

/// Records the failed requests of one tab until it is dropped.
#[derive(Debug)]
struct NetworkWatch {
    page_id: String,
    log: Arc<Mutex<NetworkLog>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for NetworkWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DevSession for Cdp {
    fn engine(&self) -> &'static str {
        "cdp"
    }

    async fn evaluate(&mut self, expression: &str) -> Result<Value, SeleniumBaseError> {
        self.page.evaluate(expression).await
    }

    async fn screenshot(&mut self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.page.screenshot().await
    }

    async fn cookie_names(&mut self) -> Result<Vec<String>, SeleniumBaseError> {
        Ok(self
            .page
            .cookies()
            .all()
            .await?
            .into_iter()
            .map(|cookie| cookie.name)
            .collect())
    }

    async fn page_errors(&mut self) -> Result<Vec<String>, SeleniumBaseError> {
        self.page.js_errors().await
    }

    async fn watch_network(&mut self) -> Result<bool, SeleniumBaseError> {
        if self
            .network
            .as_ref()
            .is_some_and(|watch| watch.page_id == self.page.id())
        {
            return Ok(true);
        }
        // Subscribed first, so no event between the two calls is missed.
        let mut events = self.page.events();
        self.page.execute("Network.enable", json!({})).await?;
        let log = Arc::new(Mutex::new(NetworkLog::default()));
        let sink = Arc::clone(&log);
        let task = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                if event.method.starts_with("Network.") {
                    sink.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .note(&event.method, &event.params);
                }
            }
        });
        // A watch of another tab is dropped here, which stops it.
        self.network = Some(NetworkWatch {
            page_id: self.page.id().to_owned(),
            log,
            task,
        });
        Ok(true)
    }

    fn network_failures(&mut self, clear: bool) -> Option<(Vec<Value>, usize)> {
        let watch = self.network.as_ref()?;
        let mut log = watch.log.lock().unwrap_or_else(PoisonError::into_inner);
        let found = (log.failures.iter().cloned().collect(), log.dropped);
        if clear {
            log.failures.clear();
            log.dropped = 0;
        }
        Some(found)
    }
}

impl Cdp {
    /// Serves an existing browser, starting on its first tab.
    ///
    /// Use this to expose a browser you launched or connected to yourself,
    /// instead of letting `start_browser` launch one.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser has no tab to start on.
    pub async fn attach(browser: Browser) -> Result<Self, SeleniumBaseError> {
        let page = browser.default_page().await?;
        Ok(Self {
            browser,
            page,
            network: None,
        })
    }
}

impl super::fingerprint::CaptureSession for Cdp {
    async fn navigate_to(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        navigate_for_client(&self.page, url).await
    }

    async fn capture_now(
        &mut self,
    ) -> Result<crate::evasion::capture::CapturedFingerprint, SeleniumBaseError> {
        self.page.capture_fingerprint().await
    }
}

impl Closeable for Cdp {
    async fn close(self) {
        // The server is shutting down; there is nobody left to report to.
        let _ = self.browser.close().await;
    }
}

/// Builds the Pure CDP server, with the developer tools.
#[must_use]
pub fn host(settings: Settings) -> Host<Cdp> {
    let mut tools = super::tool::mark_untrusted(
        tools(),
        &["get_content", "get_attributes", "run_javascript"],
    );
    tools.extend(devtools::tools());
    tools.extend(devtools::environment_tools());
    tools.extend(super::pagetools::tools());
    tools.extend(super::fingerprint::tools());
    // A task's results and errors are what the pages it visited held.
    tools.push(tasks::tool().untrusted());
    // `wait_for_email` exists only for an operator who turned the imap plugin on with options.
    #[cfg(feature = "plugin-imap")]
    if super::email::offered(settings.plugin_selection()) {
        tools.push(super::email::tool::<Cdp>());
    }
    Host::new(SERVER_NAME, INSTRUCTIONS, tools, settings)
        .with_probe(devtools::probe::<Cdp>)
        .with_refs(super::pagetools::verify::<Cdp>)
        .with_resources(super::resources::standard())
        .with_prompts(super::prompts::browser())
}

fn selector_prop() -> Prop {
    Prop::string("CSS selector, XPath, or link=text")
}

fn timeout_prop(default: f64) -> Prop {
    Prop::number("Seconds to wait for the element")
        .min(0.0)
        .default(default)
}

fn tools() -> Vec<ToolDef<Cdp>> {
    [
        session::tools(),
        navigation::tools(),
        page::tools(),
        clicks::tools(),
        forms::tools(),
        conditions::tools(),
        storage::tools(),
        windows::tools(),
        challenges::tools(),
        files::tools(),
        script::tools(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Navigates for the `navigate` tool. With plugins (feature `plugins`) their
/// `before_navigate` hooks see an MCP client as who asked.
#[cfg(feature = "plugins")]
async fn navigate_for_client(
    page: &crate::sb_cdp::Page,
    url: &str,
) -> Result<(), SeleniumBaseError> {
    page.goto_from(url, seleniumbase_rs_plugin::NavOrigin::Mcp)
        .await
}

#[cfg(not(feature = "plugins"))]
async fn navigate_for_client(
    page: &crate::sb_cdp::Page,
    url: &str,
) -> Result<(), SeleniumBaseError> {
    page.goto(url).await
}
