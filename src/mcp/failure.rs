//! What a model reads when a tool call fails.
//!
//! A failed call becomes a [`FailureReport`]: a stable `kind`, a one-line
//! summary, the call that was tried, the page as it was at that moment (only
//! for failures that concern a page), the hint, and up to three next steps
//! that name tools this server really has. The server sends it as readable
//! text and, for clients that want it, as structured content with the same
//! fields.

use std::fmt::Write as _;
use std::time::Duration;

use serde_json::{json, Map, Value};

use super::tool::ToolError;
use crate::error::SeleniumBaseError;

/// How long the page may take to describe itself after a failure. A failure
/// report must never wait on a page that has hung.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// How much of a long text argument a report repeats.
const ARGUMENT_LIMIT: usize = 200;

/// A failed tool call, explained for whoever has to fix it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FailureReport {
    /// A stable identifier of the kind of failure, such as
    /// `element_not_found` or `invalid_argument`.
    pub kind: String,
    /// One line saying what went wrong.
    pub summary: String,
    /// The tool that was called.
    pub tool: String,
    /// Its arguments, with long text shortened.
    pub arguments: Value,
    /// The page when the call failed: URL, title, ready state, recent
    /// JavaScript errors and how many elements the selector matched. Only for
    /// failures that concern a page, and only if the page answered in time.
    pub page: Option<Value>,
    /// The likely cause and the fix.
    pub hint: Option<String>,
    /// Up to three things to try next, each naming a tool of this server.
    pub next_steps: Vec<String>,
}

impl FailureReport {
    /// Explains `error`, the result of calling `tool` with `arguments`.
    ///
    /// `page` is what the page looked like, if it was read; `has_tool` says
    /// which tools the server offers, so the next steps name only real ones.
    pub(crate) fn new(
        tool: &str,
        arguments: &Map<String, Value>,
        error: &ToolError,
        page: Option<Value>,
        has_tool: &dyn Fn(&str) -> bool,
    ) -> Self {
        let (kind, hint) = match error {
            ToolError::UnknownTool(_) => ("unknown_tool".to_owned(), None),
            ToolError::InvalidArgument { .. } => ("invalid_argument".to_owned(), None),
            ToolError::NoBrowser => ("no_browser".to_owned(), None),
            ToolError::Refused(_) => ("refused".to_owned(), None),
            ToolError::Failed(error) => (error.category().to_owned(), error.hint()),
        };
        let selector = focus_selector(error, arguments);
        let next_steps = next_steps(&kind, tool, selector.as_deref(), has_tool);
        Self {
            kind,
            summary: first_line(&error.to_string()),
            tool: tool.to_owned(),
            arguments: Value::Object(shortened(arguments)),
            page,
            hint,
            next_steps,
        }
    }

    /// The report as structured data, for clients that read it as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "summary": self.summary,
            "tool": self.tool,
            "arguments": self.arguments,
            "page": self.page,
            "hint": self.hint,
            "next_steps": self.next_steps,
        })
    }

    /// The report as text: what failed, what was tried, the page, the hint
    /// and the next steps, one fact per line.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = format!("{} failed: {}\n", self.tool, self.summary);
        let _ = writeln!(text, "kind: {}", self.kind);
        let _ = writeln!(text, "tried: {} {}", self.tool, self.arguments);
        if let Some(page) = &self.page {
            render_page(&mut text, page);
        }
        if let Some(hint) = &self.hint {
            let _ = writeln!(text, "hint: {hint}");
        }
        if !self.next_steps.is_empty() {
            text.push_str("next steps:\n");
            for (number, step) in self.next_steps.iter().enumerate() {
                let _ = writeln!(text, "  {}. {step}", number + 1);
            }
        }
        text.truncate(text.trim_end().len());
        text
    }
}

/// Whether reading the page could explain `error`: element, wait, assertion,
/// navigation and script failures, not a bad argument or a missing browser.
pub(crate) fn concerns_page(error: &ToolError) -> bool {
    matches!(
        error,
        ToolError::Failed(
            SeleniumBaseError::ElementNotFound { .. }
                | SeleniumBaseError::ElementNotInteractable { .. }
                | SeleniumBaseError::StaleElement { .. }
                | SeleniumBaseError::InvalidSelector(_)
                | SeleniumBaseError::WaitTimeout(_)
                | SeleniumBaseError::AssertionFailed(_)
                | SeleniumBaseError::Navigation { .. }
                | SeleniumBaseError::CdpDriver(_)
                | SeleniumBaseError::WebDriver(_)
        )
    )
}

/// The selector a failure is about: the one the error names, or else the
/// call's own selector argument.
pub(crate) fn focus_selector(error: &ToolError, arguments: &Map<String, Value>) -> Option<String> {
    if let ToolError::Failed(
        SeleniumBaseError::ElementNotFound { selector, .. }
        | SeleniumBaseError::ElementNotInteractable { selector, .. }
        | SeleniumBaseError::StaleElement { selector },
    ) = error
    {
        return Some(selector.clone());
    }
    ["selector", "dropdown_selector", "css"]
        .iter()
        .find_map(|name| arguments.get(*name).and_then(Value::as_str))
        .filter(|selector| !selector.is_empty())
        .map(str::to_owned)
}

/// Up to three next steps for a failure of `kind`, naming only tools for
/// which `has_tool` is true.
fn next_steps(
    kind: &str,
    tool: &str,
    selector: Option<&str>,
    has_tool: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    use Step::{
        Bundle, Console, Diagnose, Environment, Inspect, Requests, Retry, Scroll, Start, Suggest,
        Wait,
    };
    let plan: &[Step] = match kind {
        "element_not_found" | "wait_timeout" if selector.is_some() => &[Diagnose, Wait, Suggest],
        "element_not_interactable" => &[Diagnose, Scroll, Inspect],
        "stale_element" => &[Retry, Inspect],
        "invalid_selector" => &[Diagnose, Suggest, Inspect],
        "element_not_found" | "wait_timeout" | "assertion_failed" => &[Inspect, Console, Bundle],
        "navigation" | "network" | "download" => &[Requests, Environment, Bundle],
        "cdp_driver" | "webdriver" => &[Console, Inspect, Bundle],
        "browser_launch" | "browser_disconnected" | "session_not_started" | "no_browser" => {
            &[Environment, Start]
        }
        _ => &[],
    };
    plan.iter()
        .filter_map(|step| step.describe(tool, selector, has_tool))
        .take(3)
        .collect()
}

/// One thing a model can try after a failure.
#[derive(Debug, Clone, Copy)]
enum Step {
    Diagnose,
    Suggest,
    Inspect,
    Wait,
    Scroll,
    Retry,
    Console,
    Requests,
    Environment,
    Bundle,
    Start,
}

impl Step {
    /// The step as a call with its arguments and a reason, if the server has
    /// the tool it needs. `Wait` and `Scroll` use whichever of the servers'
    /// tools for it exists.
    fn describe(
        self,
        tool: &str,
        selector: Option<&str>,
        has_tool: &dyn Fn(&str) -> bool,
    ) -> Option<String> {
        let call = |name: &str, extra: &str| match selector {
            Some(selector) => format!("{name} {{\"selector\": {}{extra}}}", Value::from(selector)),
            None => format!("{name} {{}}"),
        };
        let pick = |first: &'static str, second: &'static str| {
            if has_tool(first) {
                first
            } else {
                second
            }
        };
        let (name, text) = match self {
            Self::Diagnose => (
                "diagnose_selector",
                format!(
                    "{}: says whether the selector is invalid, hidden, covered, disabled, inside \
                     an iframe or a shadow root, and lists near misses.",
                    call("diagnose_selector", "")
                ),
            ),
            Self::Suggest => (
                "suggest_selectors",
                "suggest_selectors {\"text\": \"<the text on the element>\"}: ranked selectors, \
                 each checked against the page."
                    .to_owned(),
            ),
            Self::Inspect => (
                "inspect_page",
                "inspect_page {}: an outline of the page with a working selector for every \
                 control."
                    .to_owned(),
            ),
            Self::Wait => {
                let name = pick("wait_for_condition", "wait_for_element");
                let extra = if name == "wait_for_condition" {
                    ", \"state\": \"visible\", \"timeout\": 15"
                } else {
                    ", \"timeout\": 15"
                };
                (
                    name,
                    format!(
                        "{}: if the element appears late, wait for it before acting.",
                        call(name, extra)
                    ),
                )
            }
            Self::Scroll => {
                let name = pick("focus_element", "scroll_into_view");
                let extra = if name == "focus_element" {
                    ", \"action\": \"scroll_to_element\""
                } else {
                    ""
                };
                (
                    name,
                    format!("{}: bring it into view first.", call(name, extra)),
                )
            }
            Self::Retry => (
                tool,
                format!("{tool} again with the same arguments: the page re-rendered the element."),
            ),
            Self::Console => (
                "get_console_messages",
                "get_console_messages {}: the page's own script errors may explain it.".to_owned(),
            ),
            Self::Requests => (
                "get_failed_requests",
                "get_failed_requests {}: shows requests that failed or were refused.".to_owned(),
            ),
            Self::Environment => (
                "diagnose_environment",
                "diagnose_environment {}: checks the browser, the driver and their versions."
                    .to_owned(),
            ),
            Self::Bundle => (
                "capture_debug_bundle",
                "capture_debug_bundle {}: saves a screenshot, the page source, console messages \
                 and failed requests for a bug report."
                    .to_owned(),
            ),
            Self::Start => (
                "start_browser",
                "start_browser {}: start a browser session.".to_owned(),
            ),
        };
        has_tool(name).then_some(text)
    }
}

/// The page lines of a rendered report.
fn render_page(text: &mut String, page: &Value) {
    if let Some(reason) = page.get("unavailable").and_then(Value::as_str) {
        let _ = writeln!(text, "page: not read ({reason})");
        return;
    }
    let field = |name: &str| page.get(name).and_then(Value::as_str).unwrap_or("?");
    let _ = writeln!(
        text,
        "page: {} (title {:?}, ready state {})",
        field("url"),
        field("title"),
        field("ready_state")
    );
    if let Some(found) = page.get("selector").filter(|found| found.is_object()) {
        let selector = found.get("value").and_then(Value::as_str).unwrap_or("?");
        match found.get("error").and_then(Value::as_str) {
            Some(error) => {
                let _ = writeln!(text, "selector {selector:?}: cannot be used ({error})");
            }
            None => {
                let _ = writeln!(
                    text,
                    "selector {selector:?}: {} matching, {} visible",
                    found.get("matches").unwrap_or(&Value::Null),
                    found.get("visible").unwrap_or(&Value::Null)
                );
            }
        }
    }
    let errors: Vec<&str> = page
        .get("javascript_errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if errors.is_empty() {
        text.push_str("recent JavaScript errors: none recorded\n");
    } else {
        let _ = writeln!(text, "recent JavaScript errors: {}", errors.join(" | "));
    }
}

/// `arguments` with every long string cut short, so a report stays small.
fn shortened(arguments: &Map<String, Value>) -> Map<String, Value> {
    arguments
        .iter()
        .map(|(name, value)| (name.clone(), shorten(value, ARGUMENT_LIMIT)))
        .collect()
}

/// `value` with strings longer than `limit` characters cut short.
pub(crate) fn shorten(value: &Value, limit: usize) -> Value {
    match value {
        Value::String(text) if text.chars().count() > limit => {
            let kept: String = text.chars().take(limit).collect();
            let more = text.chars().count() - limit;
            Value::String(format!("{kept}... ({more} more characters)"))
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| shorten(item, limit)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(name, item)| (name.clone(), shorten(item, limit)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The first line of `text`, which for a multi-line error is its summary.
fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers pass `json!` temporaries"
    )]
    fn arguments(value: Value) -> Map<String, Value> {
        let Value::Object(map) = value else {
            panic!("tool arguments are an object")
        };
        map
    }

    fn every_tool(_: &str) -> bool {
        true
    }

    #[test]
    fn a_missing_element_names_its_kind_the_call_and_selector_tools() {
        let error = ToolError::Failed(SeleniumBaseError::element_not_found("#buy"));
        let report = FailureReport::new(
            "click_element",
            &arguments(json!({ "selector": "#buy" })),
            &error,
            None,
            &every_tool,
        );

        assert_eq!(report.kind, "element_not_found");
        assert_eq!(
            report.summary,
            "element not found: #buy (strategy: css selector)"
        );
        assert_eq!(report.next_steps.len(), 3);
        assert!(report.next_steps[0].starts_with("diagnose_selector {\"selector\": \"#buy\"}"));
        assert!(report.hint.as_deref().unwrap().contains("#buy"));
        let text = report.render();
        assert!(text.starts_with("click_element failed: element not found: #buy"));
        assert!(text.contains("\ntried: click_element {\"selector\":\"#buy\"}\n"));
        assert!(text.contains("\n  1. diagnose_selector"), "{text}");
    }

    #[test]
    fn next_steps_name_only_tools_the_server_has() {
        let error = ToolError::Failed(SeleniumBaseError::element_not_found("#buy"));
        let only = |tool: &str| matches!(tool, "wait_for_element" | "inspect_page");
        let report = FailureReport::new(
            "click_element",
            &arguments(json!({ "selector": "#buy" })),
            &error,
            None,
            &only,
        );

        assert_eq!(report.next_steps.len(), 1, "{:?}", report.next_steps);
        assert!(report.next_steps[0].starts_with("wait_for_element {\"selector\": \"#buy\", "));
    }

    #[test]
    fn the_page_is_rendered_with_matches_and_script_errors() {
        let error = ToolError::Failed(SeleniumBaseError::element_not_found("#buy"));
        let page = json!({
            "url": "https://shop.test/cart",
            "title": "Cart",
            "ready_state": "complete",
            "javascript_errors": ["TypeError: x is undefined"],
            "selector": { "value": "#buy", "matches": 2, "visible": 0 },
        });
        let text = FailureReport::new(
            "click_element",
            &arguments(json!({ "selector": "#buy" })),
            &error,
            Some(page),
            &every_tool,
        )
        .render();

        assert!(text
            .contains("\npage: https://shop.test/cart (title \"Cart\", ready state complete)\n"));
        assert!(
            text.contains("\nselector \"#buy\": 2 matching, 0 visible\n"),
            "{text}"
        );
        assert!(text.contains("\nrecent JavaScript errors: TypeError: x is undefined\n"));
    }

    #[test]
    fn a_page_that_did_not_answer_is_said_so() {
        let error = ToolError::Failed(SeleniumBaseError::wait_timeout("visible #x", None));
        let text = FailureReport::new(
            "wait_for_element",
            &Map::new(),
            &error,
            Some(json!({ "unavailable": "it did not answer within 2 seconds" })),
            &every_tool,
        )
        .render();

        assert!(text.contains("\npage: not read (it did not answer within 2 seconds)\n"));
    }

    #[test]
    fn only_page_failures_are_worth_reading_the_page_for() {
        assert!(concerns_page(&ToolError::Failed(
            SeleniumBaseError::element_not_found("#x")
        )));
        assert!(concerns_page(&ToolError::Failed(
            SeleniumBaseError::assertion_failed("title")
        )));
        assert!(!concerns_page(&ToolError::NoBrowser));
        assert!(!concerns_page(&ToolError::invalid("url", "required")));
        assert!(!concerns_page(&ToolError::Failed(
            SeleniumBaseError::browser_launch("chrome", "missing")
        )));
        assert!(!concerns_page(&ToolError::Failed(SeleniumBaseError::Io(
            std::io::Error::other("disk")
        ))));
    }

    #[test]
    fn the_selector_comes_from_the_error_before_the_arguments() {
        let error = ToolError::Failed(SeleniumBaseError::element_not_found("#inner"));
        assert_eq!(
            focus_selector(&error, &arguments(json!({ "selector": "#outer" }))).as_deref(),
            Some("#inner")
        );
        let assertion = ToolError::Failed(SeleniumBaseError::assertion_failed("text"));
        assert_eq!(
            focus_selector(&assertion, &arguments(json!({ "dropdown_selector": "#c" }))).as_deref(),
            Some("#c")
        );
        assert_eq!(focus_selector(&assertion, &Map::new()), None);
    }

    #[test]
    fn long_arguments_are_cut_short_and_the_structure_kept() {
        let long = "x".repeat(500);
        let short = shorten(
            &json!({ "text": long, "n": 3, "list": ["y".repeat(300)] }),
            10,
        );
        assert_eq!(short["text"], "xxxxxxxxxx... (490 more characters)");
        assert_eq!(short["n"], 3);
        assert_eq!(short["list"][0], "yyyyyyyyyy... (290 more characters)");
    }

    #[test]
    fn a_missing_browser_says_to_start_one() {
        let report = FailureReport::new(
            "open_url",
            &Map::new(),
            &ToolError::NoBrowser,
            None,
            &every_tool,
        );
        assert_eq!(report.kind, "no_browser");
        assert_eq!(
            report.next_steps.last().map(String::as_str),
            Some("start_browser {}: start a browser session.")
        );
        assert!(report.to_json()["page"].is_null());
    }

    #[test]
    fn a_multi_line_error_is_summarised_by_its_first_line() {
        let error = ToolError::Failed(SeleniumBaseError::assertion_failed(
            "JavaScript errors found =>\nfirst\nsecond",
        ));
        let report = FailureReport::new(
            "assert_no_js_errors",
            &Map::new(),
            &error,
            None,
            &every_tool,
        );
        assert_eq!(
            report.summary,
            "assertion failed: JavaScript errors found =>"
        );
    }
}
