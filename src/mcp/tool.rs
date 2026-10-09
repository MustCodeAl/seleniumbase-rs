//! Tool definitions: arguments, results and errors.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{Tool, ToolAnnotations};
use serde_json::{Map, Value};

use super::host::Ctx;
use super::schema::Schema;
use crate::error::SeleniumBaseError;

/// The longest wait or pause a single call may ask for, in seconds.
///
/// A model chooses these numbers, and one call holds the browser for its
/// whole duration, so they are bounded.
pub const MAX_SECONDS: f64 = 3600.0;

/// The result of a tool call.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Plain text, such as a confirmation.
    Text(String),
    /// Structured data, rendered as indented JSON.
    Json(Value),
}

impl Output {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Text(text) | Self::Json(Value::String(text)) => text.clone(),
            Self::Json(value) => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
        }
    }

    /// The text of a [`Text`](Self::Text) output, for assertions in tests.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Json(_) => None,
        }
    }

    /// The value of a [`Json`](Self::Json) output, for assertions in tests.
    #[must_use]
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Text(_) => None,
        }
    }
}

impl From<String> for Output {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for Output {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<Value> for Output {
    fn from(value: Value) -> Self {
        Self::Json(value)
    }
}

/// Why a tool call did not succeed.
///
/// These become error results the model can read and react to, not protocol
/// failures: a mistyped selector should be fixable by the next call.
#[derive(Debug)]
#[non_exhaustive]
pub enum ToolError {
    /// No tool has that name.
    UnknownTool(String),
    /// An argument is missing or has the wrong type.
    InvalidArgument {
        /// The argument name.
        name: String,
        /// What was wrong with it.
        reason: String,
    },
    /// A browser tool was called before `start_browser`.
    NoBrowser,
    /// The server's configuration forbids the request.
    Refused(String),
    /// The browser operation itself failed.
    Failed(SeleniumBaseError),
}

impl ToolError {
    /// An argument error naming the offending argument.
    #[must_use]
    pub fn invalid(name: &str, reason: impl Into<String>) -> Self {
        Self::InvalidArgument {
            name: name.to_owned(),
            reason: reason.into(),
        }
    }

    /// The message shown to the model, with a remediation hint when one exists.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Failed(error) => match error.hint() {
                Some(hint) => format!("{error}\n\nHint: {hint}"),
                None => error.to_string(),
            },
            other => other.to_string(),
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            Self::InvalidArgument { name, reason } => {
                write!(f, "invalid argument '{name}': {reason}")
            }
            Self::NoBrowser => {
                f.write_str("no browser session is running; call start_browser first")
            }
            Self::Refused(why) => write!(f, "refused: {why}"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SeleniumBaseError> for ToolError {
    fn from(error: SeleniumBaseError) -> Self {
        Self::Failed(error)
    }
}

/// The arguments of one call, with typed accessors that name the argument in
/// every error.
#[derive(Debug, Clone, Default)]
pub struct Args(Map<String, Value>);

impl Args {
    /// Wraps a JSON object of arguments.
    #[must_use]
    pub fn new(map: Map<String, Value>) -> Self {
        Self(map)
    }

    /// Moves the value of argument `from` to `to`, if `from` was given.
    pub(crate) fn rename(&mut self, from: &str, to: &str) {
        if let Some(value) = self.0.remove(from) {
            self.0.insert(to.to_owned(), value);
        }
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name).filter(|value| !value.is_null())
    }

    /// A required string.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is missing or not a string.
    pub fn str(&self, name: &str) -> Result<&str, ToolError> {
        self.opt_str(name)?
            .ok_or_else(|| ToolError::invalid(name, "this argument is required"))
    }

    /// An optional string.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is present but not a string.
    pub fn opt_str(&self, name: &str) -> Result<Option<&str>, ToolError> {
        match self.get(name) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(_) => Err(ToolError::invalid(name, "expected a string")),
        }
    }

    /// A string, or `default` when omitted.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is present but not a string.
    pub fn str_or<'a>(&'a self, name: &str, default: &'a str) -> Result<&'a str, ToolError> {
        Ok(self.opt_str(name)?.unwrap_or(default))
    }

    /// A string, number or boolean, as text. For arguments such as a select
    /// option that clients send as either `"2"` or `2`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is missing or not a scalar.
    pub fn scalar(&self, name: &str) -> Result<String, ToolError> {
        match self.get(name) {
            Some(Value::String(text)) => Ok(text.clone()),
            Some(Value::Number(number)) => Ok(number.to_string()),
            Some(Value::Bool(flag)) => Ok(flag.to_string()),
            Some(_) => Err(ToolError::invalid(name, "expected a string or a number")),
            None => Err(ToolError::invalid(name, "this argument is required")),
        }
    }

    /// A number of seconds, or `default` when omitted.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is not a number, or is negative.
    pub fn seconds_or(&self, name: &str, default: f64) -> Result<Duration, ToolError> {
        let seconds = match self.get(name) {
            None => default,
            Some(value) => value
                .as_f64()
                .ok_or_else(|| ToolError::invalid(name, "expected a number of seconds"))?,
        };
        if !seconds.is_finite() || !(0.0..=MAX_SECONDS).contains(&seconds) {
            return Err(ToolError::invalid(
                name,
                format!("must be between 0 and {MAX_SECONDS} seconds"),
            ));
        }
        Ok(Duration::from_secs_f64(seconds))
    }

    /// An optional whole number.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is present but not a whole number.
    pub fn opt_i64(&self, name: &str) -> Result<Option<i64>, ToolError> {
        match self.get(name) {
            None => Ok(None),
            Some(value) => value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
                .map(Some)
                .ok_or_else(|| ToolError::invalid(name, "expected a whole number")),
        }
    }

    /// An optional non-negative whole number.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is present but not one.
    pub fn opt_usize(&self, name: &str) -> Result<Option<usize>, ToolError> {
        match self.opt_i64(name)? {
            None => Ok(None),
            Some(n) => usize::try_from(n)
                .ok()
                .map(Some)
                .ok_or_else(|| ToolError::invalid(name, "expected a non-negative whole number")),
        }
    }

    /// A whole number, or `default` when omitted.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is not a whole number.
    pub fn i64_or(&self, name: &str, default: i64) -> Result<i64, ToolError> {
        Ok(self.opt_i64(name)?.unwrap_or(default))
    }

    /// An optional boolean, so that "unset" stays distinct from `false`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is present but not a boolean.
    pub fn opt_bool(&self, name: &str) -> Result<Option<bool>, ToolError> {
        match self.get(name) {
            None => Ok(None),
            Some(Value::Bool(flag)) => Ok(Some(*flag)),
            Some(_) => Err(ToolError::invalid(name, "expected true or false")),
        }
    }

    /// A boolean, or `default` when omitted.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] if it is not a boolean.
    pub fn bool_or(&self, name: &str, default: bool) -> Result<bool, ToolError> {
        Ok(self.opt_bool(name)?.unwrap_or(default))
    }

    /// A string that must be one of `allowed`, or `default` when omitted.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidArgument`] listing the allowed values.
    pub fn choice<'a>(
        &self,
        name: &str,
        allowed: &[&'a str],
        default: &'a str,
    ) -> Result<&'a str, ToolError> {
        match self.opt_str(name)? {
            None => Ok(default),
            Some(given) => allowed
                .iter()
                .copied()
                .find(|option| *option == given)
                .ok_or_else(|| {
                    ToolError::invalid(
                        name,
                        format!("expected one of {}, got {given:?}", allowed.join(", ")),
                    )
                }),
        }
    }
}

/// What a tool does to the world, as MCP annotation hints.
///
/// Clients use these to decide what to confirm with the user. They describe
/// the tool and are not enforced. A tool whose actions differ in kind, such as
/// one that can both read and navigate, uses [`Mixed`](Self::Mixed): no single
/// answer would be honest, so only its title is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Effect {
    /// Reads from the open web without changing anything.
    Observe,
    /// Looks at this machine or the server itself without changing anything.
    Inspect,
    /// Changes the page or the session. Repeating it may repeat the change.
    Act,
    /// Changes the session in a way that repeating it does not compound.
    Set,
    /// Writes local files, possibly replacing existing ones.
    Overwrite,
    /// Actions of different kinds behind one tool.
    Mixed,
}

impl Effect {
    fn annotations(self, title: &str) -> ToolAnnotations {
        let base = ToolAnnotations::with_title(title);
        match self {
            Self::Observe => base
                .read_only(true)
                .destructive(false)
                .idempotent(true)
                .open_world(true),
            Self::Inspect => base
                .read_only(true)
                .destructive(false)
                .idempotent(true)
                .open_world(false),
            Self::Act => base
                .read_only(false)
                .destructive(false)
                .idempotent(false)
                .open_world(true),
            Self::Set => base
                .read_only(false)
                .destructive(false)
                .idempotent(true)
                .open_world(true),
            Self::Overwrite => base
                .read_only(false)
                .destructive(true)
                .idempotent(false)
                .open_world(false),
            Self::Mixed => base,
        }
    }
}

/// The future a tool handler returns.
pub type ToolFuture = Pin<Box<dyn Future<Output = Result<Output, ToolError>> + Send>>;

type Handler<S> = Box<dyn Fn(Ctx<S>, Args) -> ToolFuture + Send + Sync>;

/// A tool: its description to clients, and what it does when called.
pub struct ToolDef<S> {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    effect: Effect,
    schema: Arc<Map<String, Value>>,
    handler: Handler<S>,
}

impl<S> fmt::Debug for ToolDef<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolDef")
            .field("name", &self.name)
            .field("effect", &self.effect)
            .finish_non_exhaustive()
    }
}

impl<S> ToolDef<S> {
    /// Defines a tool whose handler is an `async fn(Ctx<S>, Args)`.
    #[must_use]
    pub fn new<F, Fut>(
        name: &'static str,
        title: &'static str,
        description: &'static str,
        effect: Effect,
        schema: Schema,
        handler: F,
    ) -> Self
    where
        F: Fn(Ctx<S>, Args) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Output, ToolError>> + Send + 'static,
    {
        Self {
            name,
            title,
            description,
            effect,
            schema: schema.build(),
            handler: Box::new(move |ctx, args| Box::pin(handler(ctx, args))),
        }
    }

    /// The tool's name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn describe(&self) -> Tool {
        Tool::new(self.name, self.description, Arc::clone(&self.schema))
            .with_annotations(self.effect.annotations(self.title))
    }

    pub(crate) fn call(&self, ctx: Ctx<S>, args: Args) -> ToolFuture {
        (self.handler)(ctx, args)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn args(v: Value) -> Args {
        let Value::Object(map) = v else {
            panic!("tool arguments are an object")
        };
        Args::new(map)
    }

    #[test]
    fn a_required_string_names_the_argument_when_it_is_missing_or_mistyped() {
        let a = args(json!({ "n": 3 }));
        assert!(matches!(
            a.str("missing"),
            Err(ToolError::InvalidArgument { name, .. }) if name == "missing"
        ));
        assert!(matches!(
            a.str("n"),
            Err(ToolError::InvalidArgument { name, .. }) if name == "n"
        ));
        assert_eq!(args(json!({ "s": "x" })).str("s").unwrap(), "x");
    }

    #[test]
    fn null_counts_as_absent_so_clients_may_send_it_for_unset_options() {
        let a = args(json!({ "s": null, "b": null }));
        assert_eq!(a.opt_str("s").unwrap(), None);
        assert_eq!(a.opt_bool("b").unwrap(), None);
        assert_eq!(a.str_or("s", "fallback").unwrap(), "fallback");
    }

    #[test]
    fn seconds_accept_fractions_and_reject_negative_huge_and_non_numbers() {
        assert_eq!(
            args(json!({ "t": 0.5 })).seconds_or("t", 9.0).unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(
            args(json!({})).seconds_or("t", 2.0).unwrap(),
            Duration::from_secs(2)
        );
        for bad in [
            json!({ "t": -1 }),
            json!({ "t": 1.0e9 }),
            json!({ "t": "soon" }),
        ] {
            assert!(
                matches!(
                    args(bad.clone()).seconds_or("t", 1.0),
                    Err(ToolError::InvalidArgument { .. })
                ),
                "{bad} should be rejected"
            );
        }
        assert!(
            args(json!({ "t": MAX_SECONDS }))
                .seconds_or("t", 1.0)
                .is_ok(),
            "the limit itself is allowed"
        );
    }

    #[test]
    fn whole_numbers_come_from_numbers_or_numeric_strings() {
        assert_eq!(args(json!({ "n": 4 })).opt_i64("n").unwrap(), Some(4));
        assert_eq!(args(json!({ "n": " 7 " })).opt_i64("n").unwrap(), Some(7));
        assert!(args(json!({ "n": "seven" })).opt_i64("n").is_err());
        assert!(args(json!({ "n": 1.5 })).opt_i64("n").is_err());
        assert!(
            args(json!({ "n": -1 })).opt_usize("n").is_err(),
            "a count cannot be negative"
        );
    }

    #[test]
    fn a_scalar_accepts_strings_and_numbers_but_not_structures() {
        assert_eq!(args(json!({ "v": "2" })).scalar("v").unwrap(), "2");
        assert_eq!(args(json!({ "v": 2 })).scalar("v").unwrap(), "2");
        assert!(args(json!({ "v": [1] })).scalar("v").is_err());
        assert!(args(json!({})).scalar("v").is_err());
    }

    #[test]
    fn a_choice_falls_back_to_its_default_and_lists_the_options_on_error() {
        let a = args(json!({ "mode": "b" }));
        assert_eq!(a.choice("mode", &["a", "b"], "a").unwrap(), "b");
        assert_eq!(a.choice("other", &["a", "b"], "a").unwrap(), "a");
        let error = args(json!({ "mode": "z" }))
            .choice("mode", &["a", "b"], "a")
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("mode") && message.contains("a, b") && message.contains('z'),
            "{message}"
        );
    }

    #[test]
    fn renaming_moves_a_value_and_leaves_other_arguments_alone() {
        let mut a = args(json!({ "dropdown_selector": "#c", "option": "x" }));
        a.rename("dropdown_selector", "selector");
        assert_eq!(a.str("selector").unwrap(), "#c");
        assert!(a.opt_str("dropdown_selector").unwrap().is_none());
        assert_eq!(a.str("option").unwrap(), "x");
        a.rename("absent", "anything");
        assert!(a.opt_str("anything").unwrap().is_none());
    }

    #[test]
    fn json_text_renders_bare_and_structures_render_indented() {
        assert_eq!(Output::Json(json!("plain")).render(), "plain");
        assert_eq!(Output::Text("t".into()).render(), "t");
        assert_eq!(Output::Json(json!(true)).render(), "true");
        assert!(Output::Json(json!({ "a": 1 }))
            .render()
            .contains("\n  \"a\": 1"));
    }

    #[test]
    fn a_failed_browser_error_carries_its_hint_to_the_model() {
        let failure = SeleniumBaseError::element_not_found("#nope");
        let hint = failure.hint();
        let message = ToolError::Failed(failure).message();
        assert!(message.contains("#nope"), "{message}");
        if let Some(hint) = hint {
            assert!(message.contains(&hint), "{message}");
        }
    }

    #[test]
    fn effects_map_to_honest_annotations() {
        let observe = Effect::Observe.annotations("T");
        assert_eq!(observe.read_only_hint, Some(true));
        assert_eq!(observe.title.as_deref(), Some("T"));
        let overwrite = Effect::Overwrite.annotations("T");
        assert_eq!(
            (overwrite.read_only_hint, overwrite.destructive_hint),
            (Some(false), Some(true))
        );
        let mixed = Effect::Mixed.annotations("T");
        assert_eq!(
            (
                mixed.read_only_hint,
                mixed.destructive_hint,
                mixed.idempotent_hint
            ),
            (None, None, None)
        );
    }
}
