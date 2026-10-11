//! Translating the action names of a JSON scenario.
//!
//! A [`Scenario`](crate::api::scenario::Scenario) names each step's action with
//! an `action` field, in English: `{"action": "click", "css": "button"}`. Here
//! the same step can be written in any language of the dictionary
//! (`{"action": "点击", "css": "button"}`), and a file can be rewritten from one
//! language to another. Only the action names change; every other key, value and
//! the layout of the file stay as they were.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utils::translate::{scenario::translate_scenario, Language};
//!
//! let english = r#"{"name": "demo", "steps": [{"action": "open", "url": "https://example.com"}]}"#;
//! let spanish = translate_scenario(english, Language::Spanish)?;
//! assert!(spanish.text.contains(r#""action": "abrir""#));
//! assert_eq!(spanish.translated, 1);
//!
//! // And back again.
//! let back = translate_scenario(&spanish.text, Language::English)?;
//! assert_eq!(back.text, english);
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use serde_json::Value;

use super::language::Language;
use super::master_dict;
use super::translator::translate_to;
use crate::error::SeleniumBaseError;

/// A scenario with its action names translated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Translation {
    /// The scenario text, with the actions in the new language.
    pub text: String,
    /// How many steps had an action the dictionary knows.
    pub translated: usize,
    /// The actions it does not know, left as they were.
    pub unknown: Vec<String>,
}

/// Rewrites every step's `action` of the scenario `json` into `language`,
/// whichever language each is written in now.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Json`] if `json` is not JSON, or
/// [`SeleniumBaseError::InvalidConfig`] if it has no `steps` list.
pub fn translate_scenario(
    json: &str,
    language: Language,
) -> Result<Translation, SeleniumBaseError> {
    let parsed: Value = serde_json::from_str(json)?;
    if !parsed.get("steps").is_some_and(Value::is_array) {
        return Err(SeleniumBaseError::invalid_config(
            "this is not a scenario: it has no `steps` list",
        ));
    }
    let mut translated = 0;
    let mut unknown = Vec::new();
    let text = rewrite_actions(json, |action| {
        if let Some(method) = master_dict::english_of(action) {
            translated += 1;
            Some(translate_to(method, language))
        } else {
            unknown.push(action.to_owned());
            None
        }
    });
    Ok(Translation {
        text,
        translated,
        unknown,
    })
}

/// `json` with every step's `action` written in English, so the scenario
/// parser, which only knows the English names, can read a scenario in any
/// language. An action no language knows is left as it is.
#[must_use]
pub fn scenario_to_english(json: &str) -> String {
    rewrite_actions(json, |action| {
        master_dict::english_of(action)
            .filter(|method| *method != action)
            .map(str::to_owned)
    })
}

/// Where the closing quote of the string that opens at `start` is, as an index
/// one past it. A string that never closes ends at the end of the text.
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The index of the first byte at or after `from` that is not white space.
fn skip_space(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// What an open `{` or `[` is, as far as [`rewrite_actions`] is concerned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Container {
    /// The object that is the whole scenario.
    Root,
    /// The root's `steps` list.
    Steps,
    /// An object that is an element of the `steps` list: a step.
    Step,
    /// Any other object or list, wherever it is.
    Other,
}

/// Calls `replace` with the text of every `"action": "..."` that sits directly in
/// a step (an object in the `steps` list of the root object), and swaps in what
/// it returns. Everything else, spacing and key order included, is copied as it
/// is; an `action` in any other object, even one three levels down like a
/// step's, is not a step's.
fn rewrite_actions(json: &str, mut replace: impl FnMut(&str) -> Option<String>) -> String {
    let bytes = json.as_bytes();
    let mut out = String::with_capacity(json.len());
    let mut copied_to = 0;
    // The containers that are open, the outermost first.
    let mut open: Vec<Container> = Vec::new();
    // Whether the last key of the root object is `steps`.
    let mut steps_key = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let end = string_end(bytes, i);
                if open.last() == Some(&Container::Root)
                    && bytes.get(skip_space(bytes, end)) == Some(&b':')
                {
                    // Read as JSON, so `"steps"` is `steps`, as it is to the parser.
                    steps_key = serde_json::from_str::<String>(&json[i..end])
                        .is_ok_and(|key| key == "steps");
                }
                if open.last() == Some(&Container::Step) && &json[i..end] == "\"action\"" {
                    let colon = skip_space(bytes, end);
                    if bytes.get(colon) == Some(&b':') {
                        let value_start = skip_space(bytes, colon + 1);
                        if bytes.get(value_start) == Some(&b'"') {
                            let value_end = string_end(bytes, value_start);
                            if let Ok(action) =
                                serde_json::from_str::<String>(&json[value_start..value_end])
                            {
                                if let Some(new) = replace(&action) {
                                    out.push_str(&json[copied_to..value_start]);
                                    out.push_str(
                                        &serde_json::to_string(&new)
                                            .unwrap_or_else(|_| "\"\"".to_owned()),
                                    );
                                    copied_to = value_end;
                                }
                            }
                            i = value_end;
                            continue;
                        }
                    }
                }
                i = end;
            }
            opener @ (b'{' | b'[') => {
                open.push(match (open.last(), opener) {
                    (None, b'{') => Container::Root,
                    (Some(Container::Root), b'[') if steps_key => Container::Steps,
                    (Some(Container::Steps), b'{') => Container::Step,
                    _ => Container::Other,
                });
                i += 1;
            }
            b'}' | b']' => {
                open.pop();
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.push_str(&json[copied_to..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGLISH: &str = r##"{
  "name": "login",
  "steps": [
    { "action": "open", "url": "https://seleniumbase.io/simple/login" },
    {"css": "#username", "text": "demo_user", "action": "type_text"},
    { "action" : "click" , "css": "button" },
    { "action": "assert_text", "css": "h1", "contains": "Welcome" }
  ]
}
"##;

    #[test]
    fn every_action_is_rewritten_and_nothing_else_is_touched() {
        let spanish = translate_scenario(ENGLISH, Language::Spanish).unwrap();
        assert_eq!(spanish.translated, 4);
        assert!(spanish.unknown.is_empty());
        let expected = ENGLISH
            .replace("\"open\"", "\"abrir\"")
            .replace("\"type_text\"", "\"escribir\"")
            .replace("\"click\"", "\"clic\"")
            .replace("\"assert_text\"", "\"verificar\"");
        assert_eq!(spanish.text, expected);
    }

    #[test]
    fn a_translation_goes_back_to_the_original_through_any_language() {
        for language in Language::ALL {
            let there = translate_scenario(ENGLISH, language).unwrap();
            let back = translate_scenario(&there.text, Language::English).unwrap();
            assert_eq!(back.text, ENGLISH, "through {language}");
            assert_eq!(back.translated, 4);
        }
    }

    #[test]
    fn a_translation_may_start_from_another_language_than_english() {
        let chinese = translate_scenario(ENGLISH, Language::Chinese).unwrap();
        assert!(chinese.text.contains("\"点击\""), "{}", chinese.text);
        let french = translate_scenario(&chinese.text, Language::French).unwrap();
        assert!(french.text.contains("\"cliquer\""), "{}", french.text);
        assert!(!french.text.contains("点击"));
    }

    #[test]
    fn only_a_steps_action_is_rewritten() {
        let json = r#"{"name": "click", "action": "click", "steps": [
            {"action": "click", "css": "action", "text": "open",
             "extra": {"action": "open"}},
            {"css": "x", "note": "\"action\": \"open\""}
        ]}"#;
        let result = translate_scenario(json, Language::Dutch).unwrap();
        assert_eq!(result.translated, 1);
        assert_eq!(
            result.text,
            json.replacen(
                "\"action\": \"click\", \"css\"",
                "\"action\": \"klikken\", \"css\"",
                1
            )
        );
    }

    #[test]
    fn an_action_outside_the_steps_list_is_not_rewritten() {
        // Each `action` here sits three levels down, as a step's does, but
        // only the one in the root's `steps` list is a step.
        let json = concat!(
            r#"{"name":"n","meta":{"x":{"action":"click"}},"log":[{"action":"open"}],"#,
            r#""steps":[{"action":"click"}],"later":[{"action":"open"}],"#,
            r#""nested":{"steps":[{"action":"open"}]}}"#
        );
        let result = translate_scenario(json, Language::Spanish).unwrap();
        assert_eq!(result.translated, 1);
        assert!(result.unknown.is_empty());
        assert_eq!(
            result.text,
            json.replacen(
                r#""steps":[{"action":"click"}]"#,
                r#""steps":[{"action":"clic"}]"#,
                1
            )
        );
        assert_eq!(scenario_to_english(&result.text), json);

        // The case the reviewers reported: a scenario with no steps at all.
        let empty = r#"{"name":"n","meta":{"x":{"action":"click"}},"steps":[]}"#;
        let result = translate_scenario(empty, Language::Spanish).unwrap();
        assert_eq!((result.translated, result.text.as_str()), (0, empty));
        let spanish = r#"{"meta":{"x":{"action":"clic"}},"steps":[]}"#;
        assert_eq!(scenario_to_english(spanish), spanish);
    }

    #[test]
    fn a_steps_key_written_with_escapes_or_after_other_keys_still_counts() {
        let json = r#"{"a":[{"action":"open"}],"steps":[{"action":"open"}],"b":1}"#;
        let result = translate_scenario(json, Language::Spanish).unwrap();
        assert_eq!(result.translated, 1);
        assert_eq!(
            result.text,
            json.replacen(
                r#""steps":[{"action":"open"}]"#,
                r#""steps":[{"action":"abrir"}]"#,
                1
            )
        );
    }

    #[test]
    fn an_action_no_language_knows_is_left_alone_and_reported() {
        let json =
            r#"{"name": "x", "steps": [{"action": "teleport"}, {"action": "open", "url": "u"}]}"#;
        let result = translate_scenario(json, Language::Italian).unwrap();
        assert_eq!(result.unknown, ["teleport"]);
        assert_eq!(result.translated, 1);
        assert!(result.text.contains("\"teleport\""));
        assert!(result.text.contains("\"aprire\""));
    }

    #[test]
    fn the_older_short_names_become_the_method_they_stand_for() {
        let json = r#"{"name": "x", "steps": [{"action": "type", "css": "a", "text": "b"}]}"#;
        let result = translate_scenario(json, Language::English).unwrap();
        assert!(result.text.contains("\"type_text\""), "{}", result.text);
    }

    #[test]
    fn what_is_not_a_scenario_is_refused() {
        assert!(translate_scenario("not json", Language::English).is_err());
        let error = translate_scenario(r#"{"name": "x"}"#, Language::English)
            .unwrap_err()
            .to_string();
        assert!(error.contains("steps"), "{error}");
        assert!(translate_scenario("[]", Language::English).is_err());
    }

    #[test]
    fn a_scenario_in_any_language_reads_as_english() {
        for language in Language::ALL {
            let written = translate_scenario(ENGLISH, language).unwrap().text;
            assert_eq!(scenario_to_english(&written), ENGLISH, "{language}");
        }
        // Unknown actions and non-scenarios pass through unchanged.
        let odd = r#"{"steps": [{"action": "teleport"}]}"#;
        assert_eq!(scenario_to_english(odd), odd);
        assert_eq!(scenario_to_english("{broken"), "{broken");
        assert_eq!(scenario_to_english(""), "");
    }

    #[test]
    fn an_action_with_escapes_is_read_as_the_text_it_means() {
        // `click` is `click`.
        let json = r#"{"steps": [{"action": "click", "css": "a"}]}"#;
        let result = translate_scenario(json, Language::Spanish).unwrap();
        assert!(result.text.contains("\"clic\""), "{}", result.text);
        assert!(!result.text.contains("u0063"), "{}", result.text);
    }
}
