//! Firefox command-line arguments and preferences.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::error::SeleniumBaseError;

/// The value of a Firefox preference.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::FirefoxPreference;
///
/// assert_eq!(FirefoxPreference::parse("false"), FirefoxPreference::Bool(false));
/// assert_eq!(FirefoxPreference::parse("2"), FirefoxPreference::Int(2));
/// assert_eq!(FirefoxPreference::parse("en-US"), FirefoxPreference::Text("en-US".into()));
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FirefoxPreference {
    /// A true or false preference.
    Bool(bool),
    /// A whole-number preference.
    Int(i64),
    /// A text preference.
    Text(String),
}

impl FirefoxPreference {
    /// Reads the value text of `--firefox-pref=NAME:VALUE`: `true` and `false`
    /// (in any case) are booleans, whole numbers are numbers, and anything
    /// else is text. Put quotes around a value to keep it text: `"true"`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        let quoted = text.len() >= 2
            && ((text.starts_with('"') && text.ends_with('"'))
                || (text.starts_with('\'') && text.ends_with('\'')));
        if quoted {
            return Self::Text(text[1..text.len() - 1].to_owned());
        }
        if text.eq_ignore_ascii_case("true") {
            Self::Bool(true)
        } else if text.eq_ignore_ascii_case("false") {
            Self::Bool(false)
        } else if let Ok(number) = text.parse::<i64>() {
            Self::Int(number)
        } else {
            Self::Text(text.to_owned())
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Bool(value) => json!(value),
            Self::Int(value) => json!(value),
            Self::Text(value) => json!(value),
        }
    }
}

/// Command-line arguments and preferences for Firefox.
///
/// Python SeleniumBase takes them as `--firefox-arg="ARG=N,ARG2"` and
/// `--firefox-pref="NAME:VALUE,NAME2:VALUE2"`, both comma-separated.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::{FirefoxOptions, FirefoxPreference};
///
/// let mut firefox = FirefoxOptions::default();
/// firefox.add_args("width=800,-height=600")?;
/// firefox.add_preferences("dom.webnotifications.enabled:false,intl.accept_languages:fr")?;
/// assert_eq!(firefox.args(), ["--width=800", "-height=600"]);
/// assert_eq!(
///     firefox.preferences()["dom.webnotifications.enabled"],
///     FirefoxPreference::Bool(false)
/// );
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct FirefoxOptions {
    args: Vec<String>,
    preferences: BTreeMap<String, FirefoxPreference>,
}

impl FirefoxOptions {
    /// The arguments, each as Firefox gets it.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The preferences set explicitly, by name.
    #[must_use]
    pub fn preferences(&self) -> &BTreeMap<String, FirefoxPreference> {
        &self.preferences
    }

    /// Whether nothing is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.args.is_empty() && self.preferences.is_empty()
    }

    /// Adds the arguments in a comma-separated list. An entry that starts with
    /// a dash is passed as it is; any other gets `--` in front.
    ///
    /// # Errors
    ///
    /// Returns an error for an entry that is only dashes or contains a space,
    /// which would reach Firefox as more than one argument.
    pub fn add_args(&mut self, list: &str) -> Result<(), SeleniumBaseError> {
        for arg in list.split(',').map(str::trim).filter(|arg| !arg.is_empty()) {
            self.add_arg(arg)?;
        }
        Ok(())
    }

    /// Adds one argument; see [`add_args`](Self::add_args).
    ///
    /// # Errors
    ///
    /// Returns an error for an argument that is only dashes or contains
    /// whitespace.
    pub fn add_arg(&mut self, arg: &str) -> Result<(), SeleniumBaseError> {
        let arg = arg.trim();
        if arg.trim_start_matches('-').is_empty() || arg.contains(char::is_whitespace) {
            return Err(SeleniumBaseError::parse(
                arg,
                "a Firefox argument such as width=800, without spaces",
            ));
        }
        let arg = if arg.starts_with('-') {
            arg.to_owned()
        } else {
            format!("--{arg}")
        };
        if !self.args.contains(&arg) {
            self.args.push(arg);
        }
        Ok(())
    }

    /// Adds the preferences in a comma-separated `NAME:VALUE` list. A later
    /// value for the same name replaces the earlier one.
    ///
    /// # Errors
    ///
    /// Returns an error for an entry with no colon or an empty name.
    pub fn add_preferences(&mut self, list: &str) -> Result<(), SeleniumBaseError> {
        for entry in list
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (name, value) = entry
                .split_once(':')
                .filter(|(name, _)| !name.trim().is_empty())
                .ok_or_else(|| SeleniumBaseError::parse(entry, "a preference as NAME:VALUE"))?;
            self.set_preference(name.trim(), FirefoxPreference::parse(value));
        }
        Ok(())
    }

    /// Sets a preference.
    pub fn set_preference(&mut self, name: impl Into<String>, value: FirefoxPreference) {
        self.preferences.insert(name.into(), value);
    }

    /// The `prefs` map for `moz:firefoxOptions`: the explicit preferences laid
    /// over `base`.
    #[must_use]
    pub fn preferences_over(&self, mut base: Map<String, Value>) -> Map<String, Value> {
        for (name, value) in &self.preferences {
            base.insert(name.clone(), value.to_json());
        }
        base
    }
}

impl fmt::Debug for FirefoxOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FirefoxOptions")
            .field("args", &self.args)
            .field("preferences", &self.preferences.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_values_are_typed_the_way_python_types_them() {
        assert_eq!(
            FirefoxPreference::parse("true"),
            FirefoxPreference::Bool(true)
        );
        assert_eq!(
            FirefoxPreference::parse(" FALSE "),
            FirefoxPreference::Bool(false)
        );
        assert_eq!(FirefoxPreference::parse("0"), FirefoxPreference::Int(0));
        assert_eq!(FirefoxPreference::parse("-7"), FirefoxPreference::Int(-7));
        assert_eq!(
            FirefoxPreference::parse("1.5"),
            FirefoxPreference::Text("1.5".into())
        );
        assert_eq!(
            FirefoxPreference::parse("\"true\""),
            FirefoxPreference::Text("true".into())
        );
        assert_eq!(
            FirefoxPreference::parse("''"),
            FirefoxPreference::Text(String::new())
        );
        assert_eq!(
            FirefoxPreference::parse("\""),
            FirefoxPreference::Text("\"".into())
        );
    }

    #[test]
    fn arguments_get_a_dash_pair_unless_they_have_one() {
        let mut firefox = FirefoxOptions::default();
        firefox
            .add_args("width=800, -height=600 ,--private,,")
            .unwrap();
        assert_eq!(firefox.args(), ["--width=800", "-height=600", "--private"]);
        // The same argument twice counts once.
        firefox.add_args("private").unwrap();
        assert_eq!(firefox.args().len(), 3);
    }

    #[test]
    fn an_argument_that_firefox_would_split_is_refused() {
        let mut firefox = FirefoxOptions::default();
        assert!(firefox.add_arg("--width 800").is_err());
        assert!(firefox.add_arg("--").is_err());
        assert!(firefox.add_arg("").is_err());
        assert!(firefox.is_empty());
    }

    #[test]
    fn preferences_split_at_the_first_colon_so_values_may_hold_colons() {
        let mut firefox = FirefoxOptions::default();
        firefox
            .add_preferences("a.b:1, c.d:true,e.f:http://x:80/,g.h:old,g.h:new")
            .unwrap();
        let preferences = firefox.preferences();
        assert_eq!(preferences["a.b"], FirefoxPreference::Int(1));
        assert_eq!(preferences["c.d"], FirefoxPreference::Bool(true));
        assert_eq!(
            preferences["e.f"],
            FirefoxPreference::Text("http://x:80/".into())
        );
        assert_eq!(preferences["g.h"], FirefoxPreference::Text("new".into()));
        assert_eq!(preferences.len(), 4);
    }

    #[test]
    fn a_preference_without_a_name_or_colon_is_refused() {
        let mut firefox = FirefoxOptions::default();
        assert!(firefox.add_preferences("nocolon").is_err());
        assert!(firefox.add_preferences(":value").is_err());
        assert!(firefox.is_empty());
    }

    #[test]
    fn explicit_preferences_win_over_the_ones_a_switch_sets() {
        let mut firefox = FirefoxOptions::default();
        firefox.set_preference("javascript.enabled", FirefoxPreference::Bool(true));
        firefox.set_preference("a", FirefoxPreference::Text("x".into()));
        let base = json!({ "javascript.enabled": false, "permissions.default.image": 2 });
        let merged = firefox.preferences_over(base.as_object().unwrap().clone());
        assert_eq!(
            Value::Object(merged),
            json!({ "javascript.enabled": true, "permissions.default.image": 2, "a": "x" })
        );
    }

    #[test]
    fn it_round_trips_through_json_with_plain_values() {
        let mut firefox = FirefoxOptions::default();
        firefox.add_args("headless").unwrap();
        firefox.add_preferences("a:1,b:true,c:text").unwrap();
        let text = serde_json::to_string(&firefox).unwrap();
        assert_eq!(
            text,
            r#"{"args":["--headless"],"preferences":{"a":1,"b":true,"c":"text"}}"#
        );
        assert_eq!(
            serde_json::from_str::<FirefoxOptions>(&text).unwrap(),
            firefox
        );
    }
}
