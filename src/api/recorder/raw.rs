//! The actions the page recorder stores, as Python's Recorder Mode keeps them.
//!
//! The page script ([`RECORDER_JS`](super::RECORDER_JS)) appends
//! `[kind, a, b, time]` to `document.recorded_actions` and mirrors the list to
//! sessionStorage `recorded_actions`. A [`RawAction`] is one such array. `a`
//! and `b` are strings, or small arrays (`canva` keeps `[selector, x, y]`
//! and the text assertions `[text, selector]`), so they stay JSON.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::SeleniumBaseError;

/// One action as the page recorder stores it: Python's `[kind, a, b, time]`.
///
/// A text typed into a password field carries a fifth element, `"secret"`
/// (see `saveRecordedActions` in the page script); it is
/// [`secret`](Self::secret) here.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::recorder::RawAction;
///
/// let actions = RawAction::parse_list(
///     r##"[["begin", "https://a.test", "https://a.test/", 100],
///         ["click", "#go", "", "130"]]"##,
/// )?;
/// assert_eq!(actions[1].kind, "click");
/// assert_eq!(actions[1].time_ms, 130);
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub struct RawAction {
    /// What happened: `begin`, `_url_`, `click`, `input`, `as_el`, ...
    pub kind: String,
    /// The first argument: a selector, an origin, or a small array.
    pub arg: Value,
    /// The second argument: an origin, a URL, a typed text, or a small array.
    pub detail: Value,
    /// The browser's `Date.now()` when the action happened, in milliseconds.
    pub time_ms: i64,
    /// Whether the text was typed into a password field.
    pub secret: bool,
}

impl RawAction {
    /// Builds an action.
    pub fn new(kind: &str, arg: Value, detail: Value, time_ms: i64) -> Self {
        Self {
            kind: kind.to_owned(),
            arg,
            detail,
            time_ms,
            secret: false,
        }
    }

    /// Marks the action as typed into a password field.
    #[must_use]
    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Parses sessionStorage `recorded_actions`: a JSON array of
    /// `[kind, a, b, time]` arrays. `time` may be a number or a numeric
    /// string (Python's `save_recorded_actions` writes one).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when `json` is not a JSON
    /// array of arrays that each hold a text kind, two arguments and a time.
    pub fn parse_list(json: &str) -> Result<Vec<Self>, SeleniumBaseError> {
        let value: Value = serde_json::from_str(json).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("recorded actions are not JSON: {error}"))
        })?;
        let Value::Array(items) = value else {
            return Err(SeleniumBaseError::InvalidConfig(
                "recorded actions are not a JSON array".to_owned(),
            ));
        };
        items.into_iter().map(Self::try_from).collect()
    }
}

/// The text of a JSON value, or `""` when it is not text (Python would raise;
/// a recording with an odd action is better processed than lost).
pub(super) fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

fn invalid(what: impl fmt::Display) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!("recorded action: {what}"))
}

/// A millisecond time written as a number or as a numeric string.
fn time_of(value: &Value) -> Result<i64, SeleniumBaseError> {
    let parsed = match value {
        Value::Number(number) => number
            .as_i64()
            // `as` saturates a float; a time is nowhere near the limits.
            .or_else(|| number.as_f64().map(|float| float.floor() as i64)),
        Value::String(text) => text.trim().parse::<i64>().ok().or_else(|| {
            text.trim()
                .parse::<f64>()
                .ok()
                .map(|float| float.floor() as i64)
        }),
        _ => None,
    };
    parsed.ok_or_else(|| invalid(format!("the time {value} is not a number")))
}

impl TryFrom<Value> for RawAction {
    type Error = SeleniumBaseError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Array(mut items) = value else {
            return Err(invalid("not an array"));
        };
        if items.len() < 4 {
            return Err(invalid(format!(
                "{} element(s); the page script writes [kind, a, b, time]",
                items.len()
            )));
        }
        let secret = items
            .get(4)
            .is_some_and(|marker| marker.as_str() == Some("secret"));
        let time_ms = time_of(&items[3])?;
        let kind = match &items[0] {
            Value::String(kind) => kind.clone(),
            other => return Err(invalid(format!("the kind {other} is not text"))),
        };
        items.truncate(3);
        let detail = items.pop().unwrap_or(Value::Null);
        let arg = items.pop().unwrap_or(Value::Null);
        Ok(Self {
            kind,
            arg,
            detail,
            time_ms,
            secret,
        })
    }
}

impl From<RawAction> for Value {
    fn from(action: RawAction) -> Self {
        let mut items = vec![
            Value::String(action.kind),
            action.arg,
            action.detail,
            Value::from(action.time_ms),
        ];
        if action.secret {
            items.push(Value::String("secret".to_owned()));
        }
        Value::Array(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_time_is_a_number_or_a_numeric_string() {
        let actions = RawAction::parse_list(
            r##"[["begin","o","u",12],["click","#a","","34"],["sleep","1","o",56.9]]"##,
        )
        .unwrap();
        let times: Vec<i64> = actions.iter().map(|action| action.time_ms).collect();
        assert_eq!(times, [12, 34, 56]);
    }

    #[test]
    fn arguments_may_be_arrays() {
        let actions = RawAction::parse_list(
            r##"[["canva",["#c",1.5,2],"",9],["as_te",["Hi","h1"],"https://a.test",10]]"##,
        )
        .unwrap();
        assert_eq!(actions[0].arg, json!(["#c", 1.5, 2]));
        assert_eq!(actions[1].arg, json!(["Hi", "h1"]));
        assert_eq!(actions[1].detail, json!("https://a.test"));
    }

    #[test]
    fn a_fifth_element_secret_marks_a_password() {
        let actions = RawAction::parse_list(r##"[["input","#pw","hunter2",5,"secret"]]"##).unwrap();
        assert!(actions[0].secret);
        let back: Value = actions[0].clone().into();
        assert_eq!(back, json!(["input", "#pw", "hunter2", 5, "secret"]));
    }

    #[test]
    fn an_action_survives_a_trip_through_json() {
        let action = RawAction::new("click", json!("#a"), json!("https://a.test/"), 77);
        let text = serde_json::to_string(&action).unwrap();
        assert_eq!(text, r##"["click","#a","https://a.test/",77]"##);
        let back: RawAction = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
    }

    #[test]
    fn bad_input_is_an_invalid_config_error() {
        for bad in [
            "not json",
            r#"{"a":1}"#,
            r##"[["click","#a"]]"##,
            r##"[["click","#a","",null]]"##,
            r##"[["click","#a","","soon"]]"##,
            r##"[[7,"#a","",1]]"##,
            r#"["click"]"#,
        ] {
            let error = RawAction::parse_list(bad).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{bad}: {error:?}"
            );
        }
    }
}
