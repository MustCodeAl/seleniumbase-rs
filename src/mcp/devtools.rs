//! Tools that help a model develop, debug and test, not only drive a browser.
//!
//! They look at the page and explain it: an outline of what is there
//! (`inspect_page`), why a selector finds nothing (`diagnose_selector`),
//! selectors that work (`suggest_selectors`), the page's console and failed
//! requests, an accessibility check (`audit_page`), a debug bundle for a bug
//! report, and the session written out as a Rust test. `diagnose_environment`
//! checks the machine and needs no browser.
//!
//! The tools are generic over [`DevSession`], which both engines implement:
//! the page side is one script, `devtools.js`, run through
//! [`DevSession::evaluate`]. It leaves nothing behind in the page, except the
//! console recorder that `get_console_messages` installs when asked to.

use std::future::Future;
use std::time::Duration;

use serde_json::{json, Map, Value};

use super::export::{current_session, export};
use super::failure::shorten;
use super::host::ProbeFuture;
use super::support::{json_output, nonempty, write_file};
use super::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError, OUTPUT_DIR_VAR};
use crate::error::{selector_advice, SeleniumBaseError};
use crate::utils::selectors::Selector;

/// The page side of the tools: one function expression, called as
/// `(<script>)(op, args)`.
const SCRIPT: &str = include_str!("devtools.js");

/// How long `diagnose_environment` waits for a busy session before saying so.
const SESSION_WAIT: Duration = Duration::from_secs(1);

/// What the developer tools need from a browser session.
pub(crate) trait DevSession: Send + Sync + 'static {
    /// Which engine drives the browser: `"cdp"` or `"webdriver"`.
    fn engine(&self) -> &'static str;

    /// Evaluates a JavaScript expression in the current page and returns its
    /// value as JSON.
    fn evaluate(
        &mut self,
        expression: &str,
    ) -> impl Future<Output = Result<Value, SeleniumBaseError>> + Send;

    /// A PNG of the viewport.
    fn screenshot(&mut self) -> impl Future<Output = Result<Vec<u8>, SeleniumBaseError>> + Send;

    /// The names of the session's cookies, never their values.
    fn cookie_names(
        &mut self,
    ) -> impl Future<Output = Result<Vec<String>, SeleniumBaseError>> + Send;

    /// Uncaught script errors the engine collected by itself since the page
    /// loaded, oldest first; empty when the engine collects none.
    fn page_errors(
        &mut self,
    ) -> impl Future<Output = Result<Vec<String>, SeleniumBaseError>> + Send;

    /// Starts recording failed requests, if the engine can. Returns whether
    /// it is recording.
    fn watch_network(&mut self) -> impl Future<Output = Result<bool, SeleniumBaseError>> + Send;

    /// The failed requests recorded since [`watch_network`](Self::watch_network)
    /// and how many older ones were dropped, emptying the log when `clear` is
    /// set; `None` when nothing is being recorded.
    fn network_failures(&mut self, clear: bool) -> Option<(Vec<Value>, usize)>;
}

/// `selector` tagged with the strategy both engines use for it, the form the
/// page script takes.
pub(crate) fn tagged(selector: &str) -> Value {
    let (kind, value) = match Selector::auto(selector) {
        Selector::Css(value) => ("css", value.to_owned()),
        Selector::XPath(value) => ("xpath", value.to_owned()),
        Selector::LinkText(value) => ("link_text", value.to_owned()),
        Selector::PartialLinkText(value) => ("partial_link_text", value.to_owned()),
        Selector::Id(value) => ("css", format!("[id={}]", Value::from(value))),
    };
    json!({ "kind": kind, "value": value })
}

/// The JavaScript expression that runs operation `op` of the page script.
///
/// The script's header comment stays behind: it is for people, not the page.
fn expression(op: &str, args: &Value) -> String {
    let body = SCRIPT
        .find("(function")
        .map_or(SCRIPT, |start| &SCRIPT[start..]);
    format!("{}({}, {args})", body.trim_end(), Value::from(op))
}

/// Runs operation `op` of the page script and returns its answer, always as
/// an object.
///
/// Every operation answers with an object. Anything else (a page that broke a
/// built-in the script uses can cause it) is kept under `unexpected`, so the
/// tools can add fields to the answer without failing.
async fn page_op<S: DevSession>(
    session: &mut S,
    op: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    Ok(match session.evaluate(&expression(op, args)).await? {
        answer @ Value::Object(_) => answer,
        Value::Null => json!({}),
        other => json!({ "unexpected": other }),
    })
}

/// A whole-number argument between 1 and `max`, or `default`.
fn bounded(args: &Args, name: &str, default: usize, max: usize) -> Result<usize, ToolError> {
    match args.opt_usize(name)? {
        None => Ok(default),
        Some(n) if (1..=max).contains(&n) => Ok(n),
        Some(_) => Err(ToolError::invalid(
            name,
            format!("must be between 1 and {max}"),
        )),
    }
}

fn selector_prop() -> Prop {
    Prop::string("CSS selector, XPath, or link=text")
}

/// The developer tools that need a browser, for a server whose session is `S`.
#[expect(
    clippy::too_many_lines,
    reason = "the tool table reads best as one list"
)]
pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "inspect_page",
            "Inspect Page",
            "Outline the current page for writing a test: URL, title, ready state, headings, \
             forms, iframes, shadow-DOM hosts, and every control with its role, accessible name, \
             a selector that finds it, and whether it is visible and enabled. Bounded by limit; \
             says when it was cut short.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "scope",
                    Prop::string("Only look inside the first element this selector matches"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most controls to list, 1 to 500")
                        .min(1.0)
                        .default(50),
                )
                .optional(
                    "include_hidden",
                    Prop::boolean("Also list controls that are not visible").default(false),
                ),
            inspect_page::<S>,
        ),
        ToolDef::new(
            "suggest_selectors",
            "Suggest Selectors",
            "Find the element meant by its text, its role or a rough selector, and return ranked \
             stable selectors for it (test attributes, id, name, aria-label, link text, text \
             XPath, short CSS path), each checked against the page: how many elements it \
             matches and whether it finds this one.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "text",
                    Prop::string("Text the element shows or is labelled with"),
                )
                .optional(
                    "role",
                    Prop::string("ARIA role such as button, link, textbox or checkbox"),
                )
                .optional("selector", Prop::string("A rough selector that finds it"))
                .optional(
                    "limit",
                    Prop::integer("Most selectors per element, 1 to 20")
                        .min(1.0)
                        .default(5),
                ),
            suggest_selectors::<S>,
        ),
        ToolDef::new(
            "diagnose_selector",
            "Diagnose Selector",
            "Explain why a selector finds nothing or cannot be used: invalid syntax (with the \
             browser's message), no match, matches that are hidden, empty, off screen, disabled \
             or covered by another element, matches inside an iframe or a shadow root, and near \
             misses such as a similar id or the same text in different case. Ends with next \
             steps.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            diagnose_selector::<S>,
        ),
        ToolDef::new(
            "get_console_messages",
            "Get Console Messages",
            "Read the page's console messages and uncaught script errors. Messages are recorded \
             from the moment start is true, which replaces the page's console functions (a page \
             script can notice that); the Pure CDP engine also reports uncaught errors since the \
             page loaded without it.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "level",
                    Prop::choice(
                        &["all", "error", "warning", "info", "log"],
                        "The least serious level to include",
                    )
                    .default("all"),
                )
                .optional(
                    "start",
                    Prop::boolean("Start recording console messages in this page").default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the messages after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most messages to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_console_messages::<S>,
        ),
        ToolDef::new(
            "get_failed_requests",
            "Get Failed Requests",
            "List the page's requests that failed: HTTP status 400 and up from the browser's \
             performance timeline, and, once watch is true on the Pure CDP engine, every failed, \
             blocked or refused request of the active tab with its error.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "watch",
                    Prop::boolean("Start recording failed requests (Pure CDP engine)")
                        .default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the recorded failures after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most requests to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_failed_requests::<S>,
        ),
        ToolDef::new(
            "audit_page",
            "Audit Page",
            "Check the page as it is now for accessibility and HTML defects: images without alt \
             text, controls without a label or name, duplicate ids, empty links and buttons, \
             heading order, missing lang or title, and more. Each finding names its rule and a \
             selector for the element.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "severity",
                    Prop::choice(&["all", "error"], "Report warnings too, or errors only")
                        .default("all"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most findings to return, 1 to 500")
                        .min(1.0)
                        .default(50),
                ),
            audit_page::<S>,
        ),
        ToolDef::new(
            "capture_debug_bundle",
            "Capture Debug Bundle",
            "Save what a bug report needs in a new folder under the output directory: a \
             screenshot, the page source (password and hidden input values removed), the URL, \
             title, viewport and user agent, console messages, failed requests, cookie and \
             storage names (never their values) and the last tool calls (typed text and scripts \
             redacted).",
            Effect::Overwrite,
            Schema::new()
                .optional(
                    "name",
                    Prop::string("A label for the folder; letters, digits, - and _ are kept"),
                )
                .optional(
                    "calls",
                    Prop::integer("How many recent tool calls to include, 1 to 200")
                        .min(1.0)
                        .default(20),
                ),
            capture_debug_bundle::<S>,
        ),
        ToolDef::new(
            "export_session_as_test",
            "Export Session As Test",
            "Write the successful steps of this browser session (since the last start_browser) \
             as a Rust #[tokio::test] using BaseCase, into the output directory. Steps with no \
             BaseCase equivalent become // skipped: comments and are listed in the result.",
            Effect::Overwrite,
            Schema::new().optional(
                "test_name",
                Prop::string("Name of the test function and file, such as checkout_works")
                    .default("exported_session"),
            ),
            export_session_as_test::<S>,
        ),
    ]
}

/// The developer tools that need no browser.
pub(super) fn environment_tools<S: Send + 'static>() -> Vec<ToolDef<S>> {
    vec![ToolDef::new(
        "diagnose_environment",
        "Diagnose Environment",
        "Check this machine before or after a browser problem: whether Chrome and chromedriver \
         are found and their versions match, whether the output directory is usable, which \
         settings are in the environment, and whether a session is running. Needs no browser.",
        Effect::Inspect,
        Schema::new(),
        diagnose_environment::<S>,
    )]
}

// ----------------------------------------------------------------------
// Looking at the page
// ----------------------------------------------------------------------

async fn inspect_page<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let limit = bounded(&args, "limit", 50, 500)?;
    let scope = nonempty(&args, "scope")?.map(tagged);
    let include_hidden = args.bool_or("include_hidden", false)?;
    let mut session = ctx.session().await?;
    let outline = page_op(
        &mut *session,
        "inspect",
        &json!({ "limit": limit, "scope": scope, "include_hidden": include_hidden }),
    )
    .await?;
    if let Some(error) = outline.get("error").and_then(Value::as_str) {
        return Err(ToolError::invalid("scope", error));
    }
    Ok(outline.into())
}

async fn suggest_selectors<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let text = nonempty(&args, "text")?;
    let role = nonempty(&args, "role")?;
    let rough = nonempty(&args, "selector")?;
    if text.is_none() && role.is_none() && rough.is_none() {
        return Err(ToolError::invalid(
            "text",
            "give the element's text, its role or a rough selector",
        ));
    }
    let limit = bounded(&args, "limit", 5, 20)?;
    let mut session = ctx.session().await?;
    let mut found = page_op(
        &mut *session,
        "suggest",
        &json!({ "text": text, "role": role, "selector": rough.map(tagged), "limit": limit }),
    )
    .await?;
    if found["elements"].as_array().is_some_and(Vec::is_empty) {
        found["advice"] = Value::from(
            "No element matched. Check the spelling of the text, or call inspect_page to see the \
             controls the page has.",
        );
    }
    Ok(found.into())
}

async fn diagnose_selector<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let raw = args.str("selector")?;
    let mut session = ctx.session().await?;
    let engine = session.engine();
    let mut report = page_op(
        &mut *session,
        "diagnose",
        &json!({ "selector": tagged(raw), "raw": raw }),
    )
    .await?;
    drop(session);

    let mut advice: Vec<String> = selector_advice(raw).into_iter().collect();
    if raw.contains(":contains(") {
        let xpath = crate::api::base_case::css_to_xpath(raw).ok();
        advice.push(match xpath {
            Some(xpath) => format!(
                ":contains() is not CSS, and querySelector rejects it. The same element in \
                 XPath: {xpath}"
            ),
            None => ":contains() is not CSS, and querySelector rejects it; use XPath such as \
                     //button[contains(., \"text\")]."
                .to_owned(),
        });
    }
    report["verdict"] = Value::from(verdict(&report));
    report["advice"] = Value::from(advice);
    report["next_steps"] = Value::from(diagnosis_steps(raw, &report, engine));
    Ok(report.into())
}

/// One sentence saying what is wrong with the selector, from a diagnosis.
fn verdict(report: &Value) -> String {
    if let Some(error) = report["syntax_error"].as_str() {
        return format!("The browser cannot parse this selector: {error}");
    }
    let matches = report["matches"].as_u64().unwrap_or(0);
    if matches == 0 {
        if let Some(frame) = report["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|frame| frame["matches"].as_u64().unwrap_or(0) > 0)
        {
            return format!(
                "Nothing matches in the page itself, but {} element(s) match inside the frame {}: \
                 the element is in that frame's document.",
                frame["matches"], frame["frame"]
            );
        }
        if let Some(host) = report["shadow_roots"]
            .as_array()
            .and_then(|hosts| hosts.first())
        {
            return format!(
                "Nothing matches in the page itself, but {} element(s) match inside the shadow \
                 root of {}; selectors do not reach into a shadow root.",
                host["matches"], host["host"]
            );
        }
        if let Some(miss) = report["near_misses"]
            .as_array()
            .and_then(|misses| misses.first())
        {
            return format!(
                "Nothing matches. Close on the page: {} {} (try {}).",
                miss["kind"].as_str().unwrap_or("value"),
                miss["found"],
                miss["try"]
            );
        }
        return "Nothing on this page matches the selector.".to_owned();
    }
    let first = &report["elements"][0];
    let many = if matches > 1 {
        format!(" The selector matches {matches} elements; tools act on the first.")
    } else {
        String::new()
    };
    if report["visible_matches"].as_u64().unwrap_or(0) == 0 {
        let why = if first["display"] == "none" {
            "it has display: none".to_owned()
        } else if first["visibility"] != "visible" {
            format!(
                "it has visibility: {}",
                first["visibility"].as_str().unwrap_or("hidden")
            )
        } else if first["opacity"].as_f64() == Some(0.0) {
            "it has opacity: 0".to_owned()
        } else {
            "it has no size".to_owned()
        };
        return format!("{matches} element(s) match, but none is visible: the first, {why}.{many}");
    }
    if first["enabled"] == false {
        return format!("The element is disabled, so it cannot be used yet.{many}");
    }
    if let Some(cover) = first["covered_by"].as_str() {
        return format!("The element is covered by {cover}, which would get the click.{many}");
    }
    if first["in_viewport"] == false {
        return format!(
            "The element is outside the window; tools scroll it into view first.{many}"
        );
    }
    format!("The selector works: the first match is visible, enabled and not covered.{many}")
}

/// Up to three concrete next steps after a diagnosis.
fn diagnosis_steps(raw: &str, report: &Value, engine: &str) -> Vec<String> {
    let quoted = Value::from(raw);
    let mut steps = Vec::new();
    let matches = report["matches"].as_u64().unwrap_or(0);
    if report["syntax_error"].is_string() || matches == 0 {
        if let Some(frame) = report["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|frame| frame["matches"].as_u64().unwrap_or(0) > 0)
        {
            steps.push(if engine == "webdriver" {
                format!(
                    "switch_to_frame {{\"selector\": {}}}, then use the selector again.",
                    frame["frame"]
                )
            } else {
                "The Pure CDP tools search the top document only; read the frame's content \
                 with run_javascript, or open the frame's URL in a tab."
                    .to_owned()
            });
        }
        if let Some(miss) = report["near_misses"]
            .as_array()
            .and_then(|misses| misses.first())
        {
            steps.push(format!("Try the near miss: {}.", miss["try"]));
        }
        steps.push(format!(
            "suggest_selectors {{\"text\": \"<the text on the element>\"}}: working selectors \
             for {quoted} as it should be."
        ));
        steps.push("inspect_page {}: see the controls the page has now.".to_owned());
    } else {
        let first = &report["elements"][0];
        if report["visible_matches"].as_u64().unwrap_or(0) == 0 {
            steps.push(if engine == "webdriver" {
                format!("wait_for_element {{\"selector\": {quoted}, \"timeout\": 15}}: it may be shown later.")
            } else {
                format!(
                    "wait_for_condition {{\"selector\": {quoted}, \"state\": \"visible\", \
                     \"timeout\": 15}}: it may be shown later."
                )
            });
            steps.push(
                "If it opens from a menu or a tab, act on that first, then check again.".to_owned(),
            );
        }
        if first["covered_by_selector"].is_string() {
            steps.push(format!(
                "Close or remove what covers it first ({}).",
                first["covered_by_selector"]
            ));
        }
        if matches > 1 {
            steps.push(format!(
                "suggest_selectors {{\"selector\": {quoted}}}: a selector that matches only the \
                 element you mean."
            ));
        }
    }
    steps.truncate(3);
    steps
}

async fn audit_page<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    use crate::api::html_inspector::{HtmlInspector, InspectionSeverity};

    let errors_only = args.choice("severity", &["all", "error"], "all")? == "error";
    let limit = bounded(&args, "limit", 50, 500)?;
    let mut session = ctx.session().await?;
    let page = session
        .evaluate("({ url: location.href, html: document.documentElement.outerHTML })")
        .await?;
    drop(session);
    let html = page["html"].as_str().unwrap_or_default().to_owned();
    let result = tokio::task::spawn_blocking(move || HtmlInspector::inspect(&html))
        .await
        .map_err(|error| SeleniumBaseError::unsupported(format!("the audit stopped: {error}")))??;

    let mut by_rule = Map::new();
    let mut issues = Vec::new();
    let (mut errors, mut warnings) = (0_usize, 0_usize);
    for issue in &result.issues {
        let error = issue.severity == InspectionSeverity::Error;
        if error {
            errors += 1;
        } else {
            warnings += 1;
        }
        if errors_only && !error {
            continue;
        }
        let counted = by_rule.entry(issue.rule.clone()).or_insert(Value::from(0));
        *counted = Value::from(counted.as_u64().unwrap_or(0) + 1);
        if issues.len() < limit {
            issues.push(json!({
                "rule": issue.rule,
                "severity": if error { "error" } else { "warning" },
                "message": issue.message,
                "selector": issue.selector,
            }));
        }
    }
    let listed = if errors_only {
        errors
    } else {
        errors + warnings
    };
    Ok(json!({
        "url": page["url"],
        "errors": errors,
        "warnings": warnings,
        "by_rule": by_rule,
        "issues": issues,
        "truncated": issues.len() < listed,
        "note": "Checks the page as it is now, after its scripts ran. These are static checks: \
                 they cannot judge colour contrast, focus order or what a screen reader says.",
    })
    .into())
}

// ----------------------------------------------------------------------
// Console and network
// ----------------------------------------------------------------------

async fn get_console_messages<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let level = args.choice("level", &["all", "error", "warning", "info", "log"], "all")?;
    let levels: Option<&[&str]> = match level {
        "error" => Some(&["error"]),
        "warning" => Some(&["error", "warning"]),
        "info" => Some(&["error", "warning", "info"]),
        "log" => Some(&["error", "warning", "info", "log"]),
        _ => None,
    };
    let start = args.bool_or("start", false)?;
    let clear = args.bool_or("clear", false)?;
    let limit = bounded(&args, "limit", 100, 500)?;
    let mut session = ctx.session().await?;
    let mut messages = page_op(
        &mut *session,
        "console",
        &json!({ "install": start, "clear": clear, "levels": levels, "limit": limit }),
    )
    .await?;
    let page_errors = session.page_errors().await?;
    let engine = session.engine();
    drop(session);

    let errors: Vec<Value> = page_errors
        .iter()
        .rev()
        .take(limit)
        .rev()
        .map(|error| shorten(&Value::from(error.as_str()), 500))
        .collect();
    messages["page_errors"] = Value::from(errors);
    if messages["recording"] == false {
        messages["note"] = Value::from(if engine == "cdp" {
            "Console messages are not being recorded: call again with start true. page_errors \
             lists the uncaught errors since the page loaded either way."
        } else {
            "Console messages and uncaught errors are not being recorded: call again with start \
             true. Recording lasts until the page navigates."
        });
    }
    Ok(messages.into())
}

async fn get_failed_requests<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let watch = args.bool_or("watch", false)?;
    let clear = args.bool_or("clear", false)?;
    let limit = bounded(&args, "limit", 100, 500)?;
    let mut session = ctx.session().await?;
    let timeline = page_op(&mut *session, "resources", &json!({ "limit": limit })).await?;
    let watching = if watch {
        session.watch_network().await?
    } else {
        session.network_failures(false).is_some()
    };
    let recorded = session.network_failures(clear);
    let engine = session.engine();
    drop(session);

    let mut answer = json!({
        "from_performance_timeline": timeline,
        "watching": watching,
    });
    if let Some((failures, dropped)) = recorded {
        let total = failures.len();
        answer["recorded"] = json!({
            "failures": failures.into_iter().rev().take(limit).rev().collect::<Vec<_>>(),
            "total": total,
            "dropped": dropped,
        });
    }
    answer["note"] = Value::from(match (engine, watching) {
        ("cdp", true) => {
            "recorded lists every failed, blocked or refused request of the tab since watching \
             started. The performance timeline only knows statuses of same-origin or CORS-enabled \
             resources."
        }
        ("cdp", false) => {
            "Only the performance timeline was read; it knows statuses of same-origin or \
             CORS-enabled resources only. Call again with watch true to record every failed \
             request from now on."
        }
        _ => {
            "Only the performance timeline was read (this engine cannot record network events): \
             it knows statuses of same-origin or CORS-enabled resources, and not requests that \
             never got an answer."
        }
    });
    Ok(answer.into())
}

// ----------------------------------------------------------------------
// Failure reports
// ----------------------------------------------------------------------

/// Reads the page after a failed call: URL, title, ready state, recent script
/// errors, and how many elements `selector` matches. `None` when no browser
/// is running.
pub(super) fn probe<S: DevSession>(ctx: Ctx<S>, selector: Option<String>) -> ProbeFuture {
    Box::pin(async move {
        let mut session = ctx.try_session().await?;
        let args = json!({ "selector": selector.as_deref().map(tagged), "raw": selector });
        let mut state = match page_op(&mut *session, "state", &args).await {
            Ok(state) => state,
            Err(error) => {
                return Some(
                    json!({ "unavailable": format!("the page could not be read: {error}") }),
                )
            }
        };
        if let Ok(errors) = session.page_errors().await {
            let mut recent: Vec<Value> = state["javascript_errors"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            recent.extend(
                errors
                    .into_iter()
                    .map(|error| shorten(&Value::from(error), 300)),
            );
            let skip = recent.len().saturating_sub(5);
            state["javascript_errors"] = Value::from(recent.split_off(skip));
        }
        Some(state)
    })
}

// ----------------------------------------------------------------------
// Debug bundles and exported tests
// ----------------------------------------------------------------------

async fn capture_debug_bundle<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let label = crate::artifacts::safe_stem(nonempty(&args, "name")?.unwrap_or("bundle"), "bundle");
    let wanted_calls = bounded(&args, "calls", 20, 200)?;
    let folder = format!(
        "debug_bundles/{label}_{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
    );
    let settings = ctx.settings().clone();
    let path_of = |file: &str| settings.output_path(Some(&folder), file);

    let mut problems: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut session = ctx.session().await?;
    let engine = session.engine();

    let mut facts = page_op(&mut *session, "environment", &json!({})).await?;
    match session.screenshot().await {
        Ok(png) => {
            let path = path_of("screenshot.png")?;
            write_file(&path, &png).await?;
            files.push(path.display().to_string());
        }
        Err(error) => problems.push(format!("screenshot: {error}")),
    }
    match page_op(&mut *session, "source", &json!({})).await {
        Ok(source) => {
            let path = path_of("page.html")?;
            write_file(
                &path,
                source["html"].as_str().unwrap_or_default().as_bytes(),
            )
            .await?;
            files.push(path.display().to_string());
            facts["page_source_redacted_inputs"] = source["redacted_inputs"].clone();
        }
        Err(error) => problems.push(format!("page source: {error}")),
    }
    let console = page_op(&mut *session, "console", &json!({ "limit": 500 })).await;
    let timeline = page_op(&mut *session, "resources", &json!({ "limit": 500 })).await;
    let page_errors = session.page_errors().await;
    let recorded = session.network_failures(false);
    let cookies = session.cookie_names().await;
    drop(session);

    let (calls, dropped) = ctx.calls();
    let recent: Vec<Value> = calls
        .iter()
        .rev()
        .take(wanted_calls)
        .rev()
        .map(super::journal::CallRecord::redacted)
        .collect();
    let mut outcome = |part: &str, result: Result<Value, ToolError>| {
        result.unwrap_or_else(|error| {
            problems.push(format!("{part}: {error}"));
            Value::Null
        })
    };
    let bundle = json!({
        "captured_at": chrono::Local::now().to_rfc3339(),
        "engine": engine,
        "page": facts,
        "console": outcome("console", console),
        "page_errors": outcome("page errors", page_errors.map(Value::from).map_err(ToolError::from)),
        "failed_requests": {
            "from_performance_timeline": outcome("performance timeline", timeline),
            "recorded": recorded.map(|(failures, dropped)| json!({ "failures": failures, "dropped": dropped })),
        },
        "cookie_names": outcome("cookies", cookies.map(Value::from).map_err(ToolError::from)),
        "tool_calls": { "recent": recent, "older_dropped": dropped },
        "redaction": "Cookie and storage values are left out, password and hidden input values \
                      are replaced in page.html, and typed text and scripts in tool calls are \
                      replaced by their length.",
        "problems": problems,
    });
    let path = path_of("bundle.json")?;
    let text = serde_json::to_string_pretty(&bundle).map_err(SeleniumBaseError::from)?;
    write_file(&path, text.as_bytes()).await?;
    files.push(path.display().to_string());
    json_output(json!({
        "folder": settings.output_path(None, &folder)?.display().to_string(),
        "files": files,
        "problems": bundle["problems"],
    }))
}

/// Whether `name` can be both a Rust function name and a file name.
fn is_test_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 64
}

async fn export_session_as_test<S: DevSession>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let name = args.str_or("test_name", "exported_session")?;
    if !is_test_name(name) {
        return Err(ToolError::invalid(
            "test_name",
            "use letters, digits and _ only, starting with a letter, at most 64 characters",
        ));
    }
    let (calls, dropped) = ctx.calls();
    let exported = export(current_session(&calls));
    if exported.steps == 0 && exported.skipped.is_empty() {
        return Err(ToolError::Refused(
            "this session has no successful steps to export yet; open a page and act on it first"
                .to_owned(),
        ));
    }
    let code = exported.recorder.to_rust_test(name);
    let path = ctx
        .settings()
        .output_path(Some("exported_tests"), &format!("{name}.rs"))?;
    write_file(&path, code.as_bytes()).await?;
    let skipped: Vec<Value> = exported
        .skipped
        .iter()
        .map(|(tool, reason)| json!({ "tool": tool, "reason": reason }))
        .collect();
    let mut note = "The test drives the browser through WebDriver (BaseCase), whichever server \
                    recorded the session. Put it under tests/ and run it with cargo test."
        .to_owned();
    if dropped > 0 {
        note.push_str(" The call log keeps the last 200 calls; earlier steps are not in it.");
    }
    json_output(json!({
        "path": path.display().to_string(),
        "steps": exported.steps,
        "skipped": skipped,
        "note": note,
        "code": code,
    }))
}

// ----------------------------------------------------------------------
// Environment
// ----------------------------------------------------------------------

async fn diagnose_environment<S: Send + 'static>(
    ctx: Ctx<S>,
    _args: Args,
) -> Result<Output, ToolError> {
    use crate::browser::driver_version::binary_major;
    use crate::resources::assets::{lookup_chromedriver, DriverLookup};

    let mut problems: Vec<String> = Vec::new();
    let session = match tokio::time::timeout(SESSION_WAIT, ctx.try_session()).await {
        Ok(Some(_)) => "running",
        Ok(None) => "none",
        Err(_) => "busy: another call is using it",
    };

    let chrome = crate::stealth::patcher::find_system_chrome();
    let chrome_major = match &chrome {
        Some(path) => binary_major(path).await,
        None => None,
    };
    if chrome.is_none() {
        problems.push(
            "No Chrome or Chromium was found. Install Google Chrome, or give start_browser a \
             browser_executable_path (Pure CDP server)."
                .to_owned(),
        );
    }
    let driver = lookup_chromedriver();
    let driver_major = match driver.found() {
        Some(path) => binary_major(path).await,
        None => None,
    };
    match &driver {
        DriverLookup::MissingConfigured(path) => problems.push(format!(
            "CHROMEDRIVER_PATH names {}, which is not a file. Fix or unset it, or run `sbase \
             install`.",
            path.display()
        )),
        DriverLookup::NotFound => problems.push(
            "No chromedriver was found. The WebDriver servers (sb, driver) need one: run `sbase \
             install`. The Pure CDP server does not."
                .to_owned(),
        ),
        DriverLookup::Found(_) => {}
    }
    let versions_match = match (driver_major, chrome_major) {
        (Some(driver), Some(browser)) => {
            if driver != browser {
                problems.push(format!(
                    "chromedriver {driver} does not match Chrome {browser}; a driver only drives \
                     its own browser version. Run `sbase install` to fetch chromedriver {browser}."
                ));
            }
            Value::Bool(driver == browser)
        }
        _ => Value::Null,
    };

    let output_dir = ctx.settings().output_dir().to_path_buf();
    let metadata = std::fs::metadata(&output_dir).ok();
    let writable = metadata
        .as_ref()
        .map(|meta| meta.is_dir() && !meta.permissions().readonly());
    if writable == Some(false) {
        problems.push(format!(
            "The output directory {} is not a writable directory; set {OUTPUT_DIR_VAR} to one \
             that is.",
            output_dir.display()
        ));
    }
    let variable =
        |name: &str| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned());

    json_output(json!({
        "seleniumbase_rs": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "browser_session": session,
        "chrome": chrome.as_ref().map_or(json!({ "found": false }), |path| json!({
            "path": path.display().to_string(),
            "major_version": chrome_major,
        })),
        "chromedriver": match &driver {
            DriverLookup::Found(path) => json!({
                "path": path.display().to_string(),
                "major_version": driver_major,
            }),
            DriverLookup::MissingConfigured(path) => json!({
                "found": false,
                "configured": path.display().to_string(),
            }),
            DriverLookup::NotFound => json!({ "found": false }),
        },
        "versions_match": versions_match,
        "output_dir": {
            "path": output_dir.display().to_string(),
            "exists": metadata.is_some(),
            "writable": writable,
        },
        "environment": {
            "SB_MCP_OUTPUT_DIR": variable(OUTPUT_DIR_VAR),
            "CHROMEDRIVER_PATH": variable("CHROMEDRIVER_PATH"),
            "SB_PROXY": if std::env::var_os("SB_PROXY").is_some() { "set (value not shown)" } else { "unset" },
        },
        "problems": problems,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_tagged_the_way_the_engines_read_them() {
        assert_eq!(tagged("#buy"), json!({ "kind": "css", "value": "#buy" }));
        assert_eq!(
            tagged("//a[1]"),
            json!({ "kind": "xpath", "value": "//a[1]" })
        );
        assert_eq!(
            tagged("link=Home"),
            json!({ "kind": "link_text", "value": "Home" })
        );
        assert_eq!(
            tagged("partial_link=Ho"),
            json!({ "kind": "partial_link_text", "value": "Ho" })
        );
    }

    #[test]
    fn the_page_script_is_one_call_with_its_arguments_as_json() {
        let expression = expression("state", &json!({ "raw": "a\"b" }));
        assert!(
            expression.starts_with("(function (op, args) {"),
            "{}",
            &expression[..40]
        );
        assert!(expression.ends_with("})(\"state\", {\"raw\":\"a\\\"b\"})"));
    }

    #[test]
    fn test_names_must_be_rust_identifiers_and_file_names() {
        assert!(is_test_name("checkout_works"));
        assert!(is_test_name("_t2"));
        for bad in ["", "2fast", "a-b", "../x", "a b", "a.rs", &"x".repeat(65)] {
            assert!(!is_test_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_verdict_names_the_first_reason_a_selector_cannot_be_used() {
        let verdict_of = |report: Value| verdict(&report);
        assert!(verdict_of(
            json!({ "syntax_error": "'a[' is not a valid selector", "matches": 0 })
        )
        .starts_with("The browser cannot parse this selector"));
        assert!(verdict_of(json!({
            "matches": 0,
            "frames": [{ "frame": "iframe#pay", "matches": 1 }],
        }))
        .contains("inside the frame \"iframe#pay\""));
        assert!(verdict_of(json!({
            "matches": 0, "frames": [], "shadow_roots": [],
            "near_misses": [{ "kind": "id", "found": "submit-btn", "try": "#submit-btn" }],
        }))
        .contains("try \"#submit-btn\""));
        assert!(verdict_of(json!({
            "matches": 2, "visible_matches": 0,
            "elements": [{ "display": "none", "visibility": "visible", "opacity": 1 }],
        }))
        .contains("display: none"));
        assert!(verdict_of(json!({
            "matches": 1, "visible_matches": 1,
            "elements": [{ "enabled": true, "covered_by": "div.cookie-banner", "in_viewport": true }],
        }))
        .contains("covered by div.cookie-banner"));
        assert!(verdict_of(json!({
            "matches": 1, "visible_matches": 1,
            "elements": [{ "enabled": true, "in_viewport": true }],
        }))
        .starts_with("The selector works"));
    }
}
