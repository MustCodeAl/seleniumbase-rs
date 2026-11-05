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
mod windows;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{json, Value};

use super::devtools::{self, DevSession};
use super::failure::shorten;
use super::support::{cookie_file, is_missing, json_output, nonempty, write_file};
use super::{
    Args, Closeable, Ctx, Effect, Host, Output, Prop, Schema, Settings, Started, ToolDef, ToolError,
};
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Browser, Key, LaunchOptions, Page, Scroll, SelectBy, State};

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
    tools.push(plugin_task_tool().untrusted());
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

// ----------------------------------------------------------------------
// Session
// ----------------------------------------------------------------------



// ----------------------------------------------------------------------
// Plugin tasks
// ----------------------------------------------------------------------

/// The most results of a plugin task that an answer carries; the count says how many there were.
#[cfg(feature = "plugins")]
const MAX_TASK_RECORDS: usize = 200;

/// The longest text of one value in the results of a plugin task.
#[cfg(feature = "plugins")]
const MAX_RECORD_TEXT: usize = 2000;

/// `run_plugin_task`: the task of a plugin the operator enabled, on a browser of its own.
fn plugin_task_tool() -> ToolDef<Cdp> {
    ToolDef::new(
        "run_plugin_task",
        "Run Plugin Task",
        "Run the task of a plugin on a browser of its own (not the running session), and return its \
         summary and its first results. Only plugins the operator turned on for this server \
         (`sbrs-mcp --plugin NAME`) can run, and only ones that offer a task; a task that needs a \
         person at the keyboard cannot be run here (use `sbrs plugin run`). The browser is closed \
         when the task ends.",
        Effect::Mixed,
        Schema::new()
            .required("name", Prop::string("The plugin whose task to run"))
            .optional(
                "input",
                Prop::string("The task's input as JSON text, such as {\"start\":\"https://example.com/\"}; {} when omitted"),
            )
            .optional(
                "plugins",
                Prop::string(
                    "Comma-separated names of the other enabled plugins to run beside it; all \
                     of the operator's plugins when omitted",
                ),
            )
            .optional(
                "headless",
                Prop::boolean("Force headless (true) or headed (false)"),
            ),
        run_plugin_task,
    )
}

/// What a plugin task writes to its sink in an MCP call: the first results are kept for the
/// answer, and each one is counted and reported as progress.
#[cfg(feature = "plugins")]
struct TaskRecords {
    kept: Mutex<Vec<Value>>,
    count: std::sync::atomic::AtomicU32,
    reported: tokio::sync::mpsc::UnboundedSender<u32>,
}

#[cfg(feature = "plugins")]
impl TaskRecords {
    fn new(reported: tokio::sync::mpsc::UnboundedSender<u32>) -> Self {
        Self {
            kept: Mutex::new(Vec::new()),
            count: std::sync::atomic::AtomicU32::new(0),
            reported,
        }
    }

    /// The results kept and how many there were; the progress channel closes with `self`.
    fn finish(self) -> (Vec<Value>, u32) {
        let kept = self
            .kept
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        (kept, self.count.into_inner())
    }
}

#[cfg(feature = "plugins")]
impl seleniumbase_rs_plugin::Sink for TaskRecords {
    fn record(&self, line: &Value) -> seleniumbase_rs_plugin::Result<()> {
        let count = self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        {
            let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
            if kept.len() < MAX_TASK_RECORDS {
                kept.push(shorten(line, MAX_RECORD_TEXT));
            }
        }
        // A client that is not listening, or a forwarder that is gone, is nothing to stop a task for.
        let _ = self.reported.send(count);
        Ok(())
    }
}

/// How a task asks a person for help over MCP: it cannot.
#[cfg(feature = "plugins")]
struct NoPerson;

#[cfg(feature = "plugins")]
#[seleniumbase_rs_plugin::async_trait]
impl seleniumbase_rs_plugin::Prompt for NoPerson {
    async fn wait(
        &self,
        _message: &str,
        _page: &dyn seleniumbase_rs_plugin::PageHandle,
    ) -> seleniumbase_rs_plugin::Result<()> {
        Err(seleniumbase_rs_plugin::PluginError::other(
            "this task needs a person at the keyboard; run it with `sbrs plugin run`",
        ))
    }
}

/// The plugins a `run_plugin_task` call runs: `name` and the `others` (comma-separated) it names,
/// or all of the operator's plugins when it names none, with the operator's options.
///
/// # Errors
///
/// [`ToolError::Refused`] when `name` is not enabled for the server, and
/// [`ToolError::InvalidArgument`] when `others` names a plugin that is not.
#[cfg(feature = "plugins")]
fn task_selection(
    operator: &crate::config::plugins::PluginSelection,
    name: &str,
    others: Option<&str>,
) -> Result<crate::config::plugins::PluginSelection, ToolError> {
    let enabled = |plugin: &str| operator.enabled().iter().any(|enabled| enabled == plugin);
    let list = || {
        if operator.enabled().is_empty() {
            "none".to_owned()
        } else {
            operator.enabled().join(", ")
        }
    };
    if !enabled(name) {
        return Err(ToolError::Refused(format!(
            "the plugin `{name}` is not enabled for this server: the operator turns plugins on \
             with `sbrs-mcp --plugin NAME` and no tool argument adds one (enabled: {})",
            list()
        )));
    }
    let Some(others) = others else {
        return Ok(operator.clone());
    };
    let mut subset = crate::config::plugins::PluginSelection::new();
    for plugin in std::iter::once(name).chain(others.split(',').map(str::trim)) {
        if plugin.is_empty() || subset.enabled().iter().any(|chosen| chosen == plugin) {
            continue;
        }
        if !enabled(plugin) {
            return Err(ToolError::invalid(
                "plugins",
                format!(
                    "the plugin `{plugin}` is not enabled for this server (enabled: {})",
                    list()
                ),
            ));
        }
        subset = subset.enable(plugin);
        if let Some(options) = operator.options_of(plugin) {
            subset = subset.with_options(plugin, options.clone());
        }
    }
    Ok(subset)
}



#[cfg(feature = "plugins")]
async fn plugin_task(ctx: &Ctx<Cdp>, args: &Args) -> Result<Output, ToolError> {
    use crate::common::shutdown::{Outcome, Shutdown};

    let name = args.str("name")?;
    let selection = task_selection(
        ctx.settings().plugin_selection(),
        name,
        args.opt_str("plugins")?,
    )?;
    let input: Value = match args.opt_str("input")? {
        Some(text) => serde_json::from_str(text)
            .map_err(|error| ToolError::invalid("input", format!("not JSON: {error}")))?,
        None => json!({}),
    };
    let headless = args
        .opt_bool("headless")?
        .unwrap_or(cfg!(target_os = "linux"));

    // Every mistake in the name and the plugins is found before a browser starts.
    let options = crate::cli::plugin_cmd::task_launch_options(
        crate::config::settings::Settings {
            headless,
            plugins: selection,
            ..crate::config::settings::Settings::default()
        },
        name,
    )?;

    let (reported, mut counts) = tokio::sync::mpsc::unbounded_channel();
    let records = TaskRecords::new(reported);
    let progress = ctx.progress();
    let forwarder = tokio::spawn(async move {
        while let Some(count) = counts.recv().await {
            progress
                .report(count, None, format!("{count} results so far"))
                .await;
        }
    });
    // The server stops on its own signals; a task here is not raced against one of its own.
    let (mut shutdown, _never) = Shutdown::manual();
    let ran = crate::plugin_host::run_task_until(
        &mut shutdown,
        options,
        name,
        &input,
        &records,
        &NoPerson,
    )
    .await;
    let (kept, total) = records.finish();
    // The channel closed with the sink; the forwarder ends once it has said what is left.
    let _ = forwarder.await;

    match ran? {
        Outcome::Completed(summary) => Ok(json!({
            "plugin": name,
            "summary": summary,
            "records": kept,
            "record_count": total,
            "records_truncated": usize::try_from(total).is_ok_and(|total| total > kept.len()),
        })
        .into()),
        Outcome::Interrupted(signal) => Err(ToolError::Refused(format!(
            "the task of `{name}` was stopped by {signal}"
        ))),
    }
}







// ----------------------------------------------------------------------
// Navigation
// ----------------------------------------------------------------------

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





// ----------------------------------------------------------------------
// Reading
// ----------------------------------------------------------------------









// ----------------------------------------------------------------------
// Interacting
// ----------------------------------------------------------------------











// ----------------------------------------------------------------------
// Waiting and asserting
// ----------------------------------------------------------------------





// ----------------------------------------------------------------------
// Browser state
// ----------------------------------------------------------------------













// ----------------------------------------------------------------------
// Output and scripting
// ----------------------------------------------------------------------



#[cfg(all(test, feature = "plugins"))]
mod plugin_task_tests {
    use seleniumbase_rs_plugin::Sink as _;
    use serde_json::json;

    use super::*;
    use crate::config::plugins::PluginSelection;

    fn operator() -> PluginSelection {
        PluginSelection::new()
            .enable("shield")
            .enable("crx")
            .with_options("shield", json!({ "level": "strict" }))
    }

    #[test]
    fn a_plugin_the_operator_did_not_enable_is_refused_and_the_enabled_ones_are_named() {
        let error = task_selection(&operator(), "crawler", None).unwrap_err();

        let ToolError::Refused(reason) = &error else {
            panic!("expected a refusal, got {error:?}");
        };
        assert!(reason.contains("`crawler` is not enabled"), "{reason}");
        assert!(reason.contains("enabled: shield, crx"), "{reason}");

        let none = task_selection(&PluginSelection::new(), "crawler", None).unwrap_err();
        assert!(none.to_string().contains("enabled: none"), "{none}");
    }

    #[test]
    fn without_a_list_the_task_runs_with_all_the_operators_plugins_and_options() {
        let chosen = task_selection(&operator(), "crx", None).unwrap();

        assert_eq!(chosen, operator());
    }

    #[test]
    fn a_list_picks_a_subset_with_the_operators_options_and_the_task_plugin_first() {
        let chosen = task_selection(&operator(), "crx", Some(" shield, ,crx")).unwrap();

        assert_eq!(chosen.enabled(), ["crx", "shield"]);
        assert_eq!(
            chosen.options_of("shield"),
            Some(&json!({ "level": "strict" }))
        );
        let alone = task_selection(&operator(), "shield", Some("")).unwrap();
        assert_eq!(alone.enabled(), ["shield"]);
    }

    #[test]
    fn a_list_cannot_add_a_plugin_the_operator_did_not_enable() {
        let error = task_selection(&operator(), "crx", Some("shield,tor")).unwrap_err();

        assert!(
            matches!(&error, ToolError::InvalidArgument { name, .. } if name == "plugins"),
            "{error:?}"
        );
        assert!(
            error.to_string().contains("`tor` is not enabled"),
            "{error}"
        );
    }

    #[test]
    fn the_results_are_counted_kept_up_to_a_limit_and_reported_in_order() {
        let (reported, mut counts) = tokio::sync::mpsc::unbounded_channel();
        let records = TaskRecords::new(reported);
        let total = u32::try_from(MAX_TASK_RECORDS + 5).unwrap();

        for index in 0..total {
            records.record(&json!({ "index": index })).unwrap();
        }
        let (kept, count) = records.finish();

        assert_eq!(count, total);
        assert_eq!(kept.len(), MAX_TASK_RECORDS);
        assert_eq!(kept[0], json!({ "index": 0 }));
        let mut seen = Vec::new();
        while let Ok(count) = counts.try_recv() {
            seen.push(count);
        }
        assert_eq!(seen, (1..=total).collect::<Vec<_>>());
    }

    #[test]
    fn a_long_value_in_a_result_is_shortened_for_the_answer() {
        let (reported, _counts) = tokio::sync::mpsc::unbounded_channel();
        let records = TaskRecords::new(reported);

        records
            .record(&json!({ "text": "x".repeat(MAX_RECORD_TEXT * 3) }))
            .unwrap();
        let (kept, _) = records.finish();

        let text = kept[0]["text"].as_str().unwrap();
        assert!(text.len() < MAX_RECORD_TEXT * 2, "{} bytes", text.len());
    }
}
