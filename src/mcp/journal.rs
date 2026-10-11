//! A bounded log of the tool calls a server has run.
//!
//! The log is what `export_session_as_test` turns into a test and what
//! `capture_debug_bundle` puts in a bug report. It keeps the last
//! [`CAPACITY`] calls, each with its arguments (long text shortened) and a
//! short note of how it ended. A bug report gets each call without the typed
//! text and scripts, and without what a successful call returned.

use std::collections::VecDeque;

use serde_json::{Map, Value};

use super::failure::shorten;
use super::tool::Effect;

/// How many calls the log keeps; older ones are dropped first.
pub(crate) const CAPACITY: usize = 200;

/// How much of a text argument the log keeps. Long enough for any selector,
/// URL or text a test types.
const ARGUMENT_LIMIT: usize = 2000;

/// How much of a call's result or error the log keeps.
const OUTCOME_LIMIT: usize = 300;

/// Argument names whose values a bug report must not repeat: what a model
/// types into a page can be a password, and a script can carry a token.
const SENSITIVE_ARGUMENTS: [&str; 9] = [
    "text",
    "value",
    "password",
    "secret",
    "token",
    "totp_key",
    "expression",
    "script",
    "proxy",
];

/// One tool call and how it ended.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CallRecord {
    /// The tool's name.
    pub(crate) tool: String,
    /// What the tool does to the world.
    pub(crate) effect: Effect,
    /// The arguments, with long text shortened.
    pub(crate) arguments: Map<String, Value>,
    /// Whether the call succeeded.
    pub(crate) ok: bool,
    /// The start of the result, or of the error.
    pub(crate) outcome: String,
    /// How many characters the whole result, or error, had before `outcome` cut it.
    outcome_chars: usize,
    /// When the call ended, as RFC 3339 local time.
    pub(crate) at: String,
}

impl CallRecord {
    /// A record of `tool` called with `arguments`, ending in `outcome`.
    pub(crate) fn new(
        tool: &str,
        effect: Effect,
        arguments: &Map<String, Value>,
        ok: bool,
        outcome: &str,
    ) -> Self {
        let arguments = arguments
            .iter()
            .map(|(name, value)| (name.clone(), shorten(value, ARGUMENT_LIMIT)))
            .collect();
        let mut kept: String = outcome.chars().take(OUTCOME_LIMIT).collect();
        if kept.len() < outcome.len() {
            kept.push_str("...");
        }
        Self {
            tool: tool.to_owned(),
            effect,
            arguments,
            ok,
            outcome: kept,
            outcome_chars: outcome.chars().count(),
            at: chrono::Local::now().to_rfc3339(),
        }
    }

    /// The record as JSON for a bug report: the values of arguments that may
    /// hold secrets are replaced by their length, and so is the result of a call
    /// that succeeded, which can be a cookie, a stored token or a page's text.
    /// The error of a call that failed is kept, since it says what went wrong.
    pub(crate) fn redacted(&self) -> Value {
        let arguments: Map<String, Value> = self
            .arguments
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    Value::String(text) if SENSITIVE_ARGUMENTS.contains(&name.as_str()) => {
                        Value::String(format!("[redacted, {} characters]", text.chars().count()))
                    }
                    other => other.clone(),
                };
                (name.clone(), value)
            })
            .collect();
        serde_json::json!({
            "tool": self.tool,
            "arguments": arguments,
            "ok": self.ok,
            "outcome": if self.ok {
                Value::String(format!("[redacted, {} characters]", self.outcome_chars))
            } else {
                Value::Null
            },
            "error": if self.ok { Value::Null } else { Value::String(self.outcome.clone()) },
            "at": self.at,
        })
    }
}

/// The last [`CAPACITY`] calls, oldest first.
#[derive(Debug, Default)]
pub(crate) struct Journal {
    calls: VecDeque<CallRecord>,
    dropped: usize,
}

impl Journal {
    /// Adds a call, dropping the oldest when the log is full.
    pub(crate) fn push(&mut self, record: CallRecord) {
        if self.calls.len() == CAPACITY {
            self.calls.pop_front();
            self.dropped += 1;
        }
        self.calls.push_back(record);
    }

    /// The calls kept, oldest first.
    pub(crate) fn calls(&self) -> Vec<CallRecord> {
        self.calls.iter().cloned().collect()
    }

    /// How many calls were dropped to stay within [`CAPACITY`].
    pub(crate) fn dropped(&self) -> usize {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers pass `json!` temporaries"
    )]
    fn record(tool: &str, arguments: Value) -> CallRecord {
        let Value::Object(arguments) = arguments else {
            panic!("tool arguments are an object")
        };
        CallRecord::new(tool, Effect::Act, &arguments, true, "done")
    }

    #[test]
    fn the_log_keeps_the_newest_calls_and_counts_what_it_dropped() {
        let mut journal = Journal::default();
        for n in 0..CAPACITY + 5 {
            journal.push(record(
                "open_url",
                json!({ "url": format!("https://a.test/{n}") }),
            ));
        }

        let calls = journal.calls();
        assert_eq!(calls.len(), CAPACITY);
        assert_eq!(journal.dropped(), 5);
        assert_eq!(calls[0].arguments["url"], "https://a.test/5");
    }

    #[test]
    fn a_bug_report_never_repeats_typed_text_or_scripts() {
        let typed = record(
            "type_text",
            json!({ "selector": "#password", "text": "hunter2" }),
        );
        let script = record("run_javascript", json!({ "expression": "token()" }));

        let typed = typed.redacted();
        assert_eq!(typed["arguments"]["selector"], "#password");
        assert_eq!(typed["arguments"]["text"], "[redacted, 7 characters]");
        assert!(!typed.to_string().contains("hunter2"));
        assert!(!script.redacted().to_string().contains("token()"));
    }

    #[test]
    fn a_bug_report_never_repeats_what_a_tool_returned() {
        // A cookie, a storage value or a page's text can be a session token.
        let cookies = r#"[{"name":"sid","value":"s3cret-session"}]"#;
        let call = CallRecord::new("get_cookies", Effect::Observe, &Map::new(), true, cookies);
        let report = call.redacted();
        assert!(!report.to_string().contains("s3cret"), "{report}");
        assert_eq!(report["ok"], true);
        assert_eq!(report["tool"], "get_cookies");
        assert_eq!(
            report["outcome"],
            format!("[redacted, {} characters]", cookies.chars().count())
        );
        assert_eq!(report["error"], Value::Null);

        // The size is that of the whole result, not of the part the log kept.
        let long = format!("{}s3cret-late", "z".repeat(5000));
        let call = CallRecord::new("get_content", Effect::Observe, &Map::new(), true, &long);
        assert_eq!(call.redacted()["outcome"], "[redacted, 5011 characters]");
        assert_eq!(long.chars().count(), 5011);
    }

    #[test]
    fn a_failed_call_keeps_its_error_in_a_bug_report() {
        let call = CallRecord::new(
            "click_element",
            Effect::Act,
            &Map::new(),
            false,
            "element not found: #buy",
        );
        let report = call.redacted();
        assert_eq!(report["error"], "element not found: #buy");
        assert_eq!(report["outcome"], Value::Null);
    }

    #[test]
    fn outcomes_and_arguments_are_bounded() {
        let long = "y".repeat(5000);
        let call = CallRecord::new(
            "get_content",
            Effect::Observe,
            json!({ "selector": long }).as_object().unwrap(),
            false,
            &"z".repeat(1000),
        );

        assert!(call.outcome.len() <= OUTCOME_LIMIT + 3);
        assert!(call.arguments["selector"].as_str().unwrap().len() < 2100);
        assert_eq!(call.redacted()["outcome"], Value::Null);
        assert!(call.redacted()["error"].as_str().unwrap().ends_with("..."));
    }
}
