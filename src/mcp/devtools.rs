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

mod diagnostics;
mod environment;
mod exports;
mod selectors;

pub(super) use environment::redact_setting;
pub(super) use exports::is_test_name;

use std::time::Duration;

use serde_json::{json, Value};

use super::failure::shorten;
use super::host::ProbeFuture;
use super::{Args, Ctx, Effect, Prop, Schema, ToolDef, ToolError};
use crate::error::SeleniumBaseError;
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
pub(super) async fn page_op<S: DevSession>(
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
pub(super) fn bounded(
    args: &Args,
    name: &str,
    default: usize,
    max: usize,
) -> Result<usize, ToolError> {
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
pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    [selectors::tools(), diagnostics::tools(), exports::tools()]
        .into_iter()
        .flatten()
        .collect()
}

/// The developer tools of [`tools`] called `names`, for a server that offers
/// only some of them. Each keeps its single definition above.
pub(super) fn tools_named<S: DevSession>(names: &[&str]) -> Vec<ToolDef<S>> {
    let mut tools = tools::<S>();
    tools.retain(|tool| names.contains(&tool.name()));
    tools
}

/// The developer tools that need no browser.
pub(super) fn environment_tools<S: Send + 'static>() -> Vec<ToolDef<S>> {
    vec![ToolDef::new(
        "diagnose_environment",
        "Diagnose Environment",
        "Check this machine before or after a browser problem: the platform, whether Chrome and \
         chromedriver are found and their major versions match (with the command that fixes a \
         mismatch), the driver cache folder and what is in it, whether the output directory is \
         writable and how much disk is free, the cargo features this build has and lacks, the \
         SB_* settings in the environment (secret values redacted), and whether a session is \
         running. Needs no browser.",
        Effect::Inspect,
        Schema::new(),
        environment::diagnose_environment::<S>,
    )]
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
}
