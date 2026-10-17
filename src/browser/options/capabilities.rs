//! Extra WebDriver capabilities, given as JSON or as a Python dictionary.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thirtyfour::Capabilities;

use crate::error::SeleniumBaseError;

/// Capabilities to send to the WebDriver server on top of the ones the other
/// settings produce.
///
/// Python SeleniumBase takes them as `--cap-string='{"browserName":"chrome"}'`
/// or as `--cap-file=FILE`, mostly to describe a session on a Selenium Grid or
/// a cloud service. They are merged into the capabilities of the session: a key
/// the settings did not set is added, objects such as `goog:chromeOptions` are
/// merged key by key, lists such as its `args` gain the new entries, and
/// anything else replaces what was there.
///
/// A key that is neither one of the W3C names nor of the form `vendor:key` is
/// sent in the legacy `desiredCapabilities` only, which grids that still speak
/// the old protocol read.
///
/// [`Debug`] shows the keys but not the values, because grid credentials
/// travel in capabilities.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::CapabilityOverrides;
///
/// let caps = CapabilityOverrides::from_text(r#"{"browserName": "chrome", "name": "smoke"}"#)?;
/// assert_eq!(caps.keys().collect::<Vec<_>>(), ["browserName", "name"]);
///
/// // The Python dictionary a SeleniumBase capabilities file holds.
/// let caps = CapabilityOverrides::from_text("capabilities = {'name': 'smoke', 'record': True}")?;
/// assert_eq!(caps.get("record"), Some(&serde_json::json!(true)));
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CapabilityOverrides(Map<String, Value>);

impl CapabilityOverrides {
    /// Reads capabilities from JSON, or from the Python dictionary literal a
    /// SeleniumBase capabilities file holds (`capabilities = {...}`, with
    /// single or double quotes, `True`/`False`/`None`, `#` comments and
    /// trailing commas).
    ///
    /// # Errors
    ///
    /// Returns an error if the text is neither, or is not an object.
    pub fn from_text(text: &str) -> Result<Self, SeleniumBaseError> {
        let parsed = serde_json::from_str::<Value>(text).or_else(|json_error| {
            let converted = python_dict_to_json(text).ok_or_else(|| json_error.to_string())?;
            serde_json::from_str::<Value>(&converted).map_err(|error| {
                format!("not JSON ({json_error}) and not a Python dictionary ({error})")
            })
        });
        match parsed {
            Ok(Value::Object(map)) => Ok(Self(map)),
            Ok(_) => Err(SeleniumBaseError::parse(
                shorten(text),
                "capabilities as an object of names and values",
            )),
            Err(reason) => Err(SeleniumBaseError::parse(shorten(text), reason)),
        }
    }

    /// Reads capabilities from a file, as [`from_text`](Self::from_text) does.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or does not hold
    /// capabilities.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "cannot read the capabilities file {}: {error}",
                path.display()
            ))
        })?;
        Self::from_text(&text).map_err(|error| {
            SeleniumBaseError::invalid_config(format!("{}: {error}", path.display()))
        })
    }

    /// Adds the capabilities of `other`, which win over the ones already here.
    pub fn merge(&mut self, other: &Self) {
        for (key, value) in &other.0 {
            match self.0.get_mut(key) {
                Some(existing) => merge_value(existing, value),
                None => {
                    self.0.insert(key.clone(), value.clone());
                }
            }
        }
    }

    /// The capability named `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// The names of the capabilities.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sets `field` of the object capability `key` to `value`, or removes the
    /// field when `value` is `None` (and `key` with it when nothing is left).
    /// A `key` that holds something other than an object is replaced.
    #[cfg(feature = "plugins")]
    pub(crate) fn set_field(&mut self, key: &str, field: &str, value: Option<Value>) {
        match value {
            Some(value) => {
                let entry = self
                    .0
                    .entry(key.to_owned())
                    .or_insert_with(|| Value::Object(Map::new()));
                if !entry.is_object() {
                    *entry = Value::Object(Map::new());
                }
                if let Value::Object(object) = entry {
                    object.insert(field.to_owned(), value);
                }
            }
            None => {
                if let Some(Value::Object(object)) = self.0.get_mut(key) {
                    object.remove(field);
                    if object.is_empty() {
                        self.0.remove(key);
                    }
                }
            }
        }
    }

    /// Merges these capabilities into `caps`, as the type's documentation
    /// describes.
    pub fn apply_to(&self, caps: &mut Capabilities) {
        for (key, value) in &self.0 {
            match caps.get_mut(key) {
                Some(existing) => merge_value(existing, value),
                None => {
                    // `Capabilities::set` only fails when serialising, which a
                    // JSON value cannot.
                    let _ = caps.set(key.clone(), value);
                }
            }
        }
    }
}

impl fmt::Debug for CapabilityOverrides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityOverrides")
            .field("keys", &self.0.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// Merges `source` into `target`: objects key by key, lists by appending what
/// is new, anything else by replacing.
fn merge_value(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => merge_value(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (Value::Array(target), Value::Array(source)) => {
            for item in source {
                if !target.contains(item) {
                    target.push(item.clone());
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

/// The start of `text`, for an error message.
fn shorten(text: &str) -> String {
    const LIMIT: usize = 60;
    let mut shown: String = text.chars().take(LIMIT).collect();
    if text.chars().count() > LIMIT {
        shown.push('…');
    }
    shown
}

/// The Python dictionary literal in `text` written as JSON, or `None` when the
/// text holds no braces.
///
/// It takes everything from the first `{` to the last `}`, so
/// `capabilities = {...}` and a bare dictionary both work. Single-quoted
/// strings become double-quoted, `True`, `False` and `None` become `true`,
/// `false` and `null`, `#` comments are dropped and a comma before a closing
/// bracket is removed. It does not evaluate anything.
fn python_dict_to_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end < start {
        return None;
    }
    let mut out = String::with_capacity(end - start + 1);
    let mut chars = text[start..=end].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                out.push('"');
                while let Some(inner) = chars.next() {
                    match inner {
                        '\\' => match chars.next() {
                            // `\'` is just an apostrophe in JSON.
                            Some('\'') => out.push('\''),
                            Some(escaped) => {
                                out.push('\\');
                                out.push(escaped);
                            }
                            None => break,
                        },
                        quote if quote == c => break,
                        // The other quote needs escaping inside a JSON string.
                        '"' => out.push_str("\\\""),
                        other => out.push(other),
                    }
                }
                out.push('"');
            }
            '#' => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '}' | ']' => {
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.truncate(kept - 1);
                }
                out.push(c);
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut word = String::from(c);
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push_str(match word.as_str() {
                    "True" => "true",
                    "False" => "false",
                    "None" => "null",
                    other => other,
                });
            }
            other => out.push(other),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_capabilities_are_read_as_they_are() {
        let caps =
            CapabilityOverrides::from_text(r#"{"browserName":"chrome","name":"test1"}"#).unwrap();
        assert_eq!(caps.get("browserName"), Some(&json!("chrome")));
        assert_eq!(caps.get("name"), Some(&json!("test1")));
    }

    #[test]
    fn a_python_capabilities_file_is_read_without_running_python() {
        let file = "\
# Sample capabilities, for a grid.
capabilities = {
    'browserName': 'chrome',
    \"platformName\": \"WINDOWS\",   # a comment, with 'quotes' in it
    'bstack:options': {
        'userName': 'it\\'s me',
        'debug': True,
        'video': False,
        'local': None,
        'build': 7,
    },
    'tags': ['a', \"b\",],
}
";
        let caps = CapabilityOverrides::from_text(file).unwrap();
        assert_eq!(caps.get("browserName"), Some(&json!("chrome")));
        assert_eq!(caps.get("platformName"), Some(&json!("WINDOWS")));
        assert_eq!(
            caps.get("bstack:options"),
            Some(&json!({
                "userName": "it's me",
                "debug": true,
                "video": false,
                "local": null,
                "build": 7
            }))
        );
        assert_eq!(caps.get("tags"), Some(&json!(["a", "b"])));
    }

    #[test]
    fn a_double_quote_inside_a_single_quoted_string_is_escaped() {
        let caps = CapabilityOverrides::from_text(r#"{'note': 'say "hi"'}"#).unwrap();
        assert_eq!(caps.get("note"), Some(&json!("say \"hi\"")));
    }

    #[test]
    fn text_that_is_not_an_object_is_refused() {
        for text in ["", "chrome", "[1, 2]", "{'a': }", "{'a': foo}", "42"] {
            assert!(CapabilityOverrides::from_text(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn a_file_is_read_and_a_missing_one_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("caps.py");
        std::fs::write(&path, "capabilities = {'name': 'x'}").unwrap();
        assert_eq!(
            CapabilityOverrides::from_file(&path).unwrap().get("name"),
            Some(&json!("x"))
        );
        let missing = dir.path().join("missing.json");
        let error = CapabilityOverrides::from_file(&missing).unwrap_err();
        assert!(error.to_string().contains("missing.json"), "{error}");
        let broken = dir.path().join("broken.json");
        std::fs::write(&broken, "nothing here").unwrap();
        assert!(CapabilityOverrides::from_file(&broken)
            .unwrap_err()
            .to_string()
            .contains("broken.json"));
    }

    #[test]
    fn later_capabilities_win_when_merged() {
        let mut caps = CapabilityOverrides::from_text(r#"{"a": 1, "b": {"x": 1}}"#).unwrap();
        caps.merge(&CapabilityOverrides::from_text(r#"{"a": 2, "b": {"y": 2}, "c": 3}"#).unwrap());
        assert_eq!(
            caps.0,
            *json!({"a": 2, "b": {"x": 1, "y": 2}, "c": 3})
                .as_object()
                .unwrap()
        );
    }

    #[test]
    fn applying_merges_into_what_the_other_settings_set() {
        let mut caps = Capabilities::new();
        caps.set("browserName", "chrome").unwrap();
        caps.set(
            "goog:chromeOptions",
            json!({ "args": ["--headless=new"], "binary": "/usr/bin/chrome" }),
        )
        .unwrap();

        CapabilityOverrides::from_text(
            r#"{
                "browserName": "chromium",
                "se:name": "smoke",
                "goog:chromeOptions": { "args": ["--headless=new", "--mute-audio"], "w3c": true }
            }"#,
        )
        .unwrap()
        .apply_to(&mut caps);

        assert_eq!(caps.get("browserName"), Some(&json!("chromium")));
        assert_eq!(caps.get("se:name"), Some(&json!("smoke")));
        assert_eq!(
            caps.get("goog:chromeOptions"),
            Some(&json!({
                "args": ["--headless=new", "--mute-audio"],
                "binary": "/usr/bin/chrome",
                "w3c": true
            }))
        );
    }

    #[test]
    fn debug_shows_the_keys_and_never_the_values() {
        let caps = CapabilityOverrides::from_text(r#"{"browserstack.key": "hunter2"}"#).unwrap();
        let shown = format!("{caps:?}");
        assert!(shown.contains("browserstack.key"), "{shown}");
        assert!(!shown.contains("hunter2"), "{shown}");
    }
}
