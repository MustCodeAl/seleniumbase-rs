//! Shadow DOM selector piercing for the WebDriver engine.
//!
//! A selector such as `my-app ::shadow .form ::shadow input` names an element
//! that sits inside two shadow roots. [`split_shadow_selector`] cuts it at each
//! `::shadow` combinator, and the `build_shadow_*` functions turn the pieces
//! into a script that walks from the document into each host's open shadow
//! root in turn and then does something with the element it ends on.
//!
//! The scripts are *function bodies* for WebDriver's `Execute Script`: they
//! `return` their result, so they are not expressions to hand to
//! `Runtime.evaluate`. Every fragment and value is put in with
//! [`js_code::quote`](crate::js_code::quote), never pasted between quotes.
//!
//! Only open shadow roots can be entered; a closed root makes the walk end on
//! `null` and the script report that the element was not found.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utils::shadow::{build_shadow_click, split_shadow_selector};
//!
//! let fragments = split_shadow_selector("my-app ::shadow button.save");
//! assert_eq!(fragments, ["my-app", "button.save"]);
//!
//! let script = build_shadow_click(&fragments);
//! assert!(script.contains(r#"document.querySelector("my-app")"#));
//! assert!(script.contains(r#"el.shadowRoot.querySelector("button.save")"#));
//! ```

use crate::js_code::quote;

/// The piercing combinator that separates a host from the selector inside its
/// shadow root.
const COMBINATOR: &str = "::shadow";

/// Splits a selector on the `::shadow` piercing combinator.
///
/// Each piece is trimmed and empty pieces are dropped, so a leading or
/// trailing combinator is ignored and the result is empty only if the selector
/// is blank. The split is textual: a `::shadow` inside a quoted attribute
/// value is also treated as a combinator.
///
/// Example: `"my-app ::shadow .form ::shadow input"`
/// returns `["my-app", ".form", "input"]`.
#[must_use]
pub fn split_shadow_selector(selector: &str) -> Vec<String> {
    selector
        .split(COMBINATOR)
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A function-body script that walks the shadow roots named by `fragments`.
///
/// If any step finds nothing the script returns `missing`; otherwise it runs
/// `found`, which sees the element as `el` and must `return`. With no
/// fragments there is nothing to find, so it returns `missing`.
fn walk(fragments: &[String], missing: &str, found: &str) -> String {
    let Some((first, rest)) = fragments.split_first() else {
        return format!("return {missing};");
    };
    let mut script = String::from("return (function(){\n");
    script.push_str(&format!(
        "  var el = document.querySelector({});\n",
        quote(first)
    ));
    for fragment in rest {
        script.push_str(&format!(
            "  el = el && el.shadowRoot ? el.shadowRoot.querySelector({}) : null;\n",
            quote(fragment)
        ));
    }
    script.push_str(&format!("  if (!el) return {missing};\n  {found}\n}})();"));
    script
}

/// Builds a script that walks through shadow roots using the provided CSS
/// fragments and returns the final element, or `null` if any step finds
/// nothing.
#[must_use]
pub fn build_shadow_query(fragments: &[String]) -> String {
    walk(fragments, "null", "return el;")
}

/// Builds a script that clicks the pierced element and returns `true`, or
/// returns `false` if it was not found.
#[must_use]
pub fn build_shadow_click(fragments: &[String]) -> String {
    walk(fragments, "false", "el.click(); return true;")
}

/// Builds a script that types `text` into the pierced element and returns
/// `true`, or returns `false` if it was not found.
///
/// The value is set and then an `input` and a `change` event are dispatched,
/// the way a user's typing would.
#[must_use]
pub fn build_shadow_type(fragments: &[String], text: &str) -> String {
    walk(
        fragments,
        "false",
        &format!(
            "el.value = {}; \
             el.dispatchEvent(new Event('input', {{bubbles: true}})); \
             el.dispatchEvent(new Event('change', {{bubbles: true}})); \
             return true;",
            quote(text)
        ),
    )
}

/// Builds a script that returns the text of the pierced element (its text
/// content, or else its value), or `''` if it was not found.
#[must_use]
pub fn build_shadow_text(fragments: &[String]) -> String {
    walk(fragments, "''", "return el.textContent || el.value || '';")
}

/// Builds a script that returns an attribute of the pierced element, or `''`
/// if the element or the attribute is missing.
#[must_use]
pub fn build_shadow_attribute(fragments: &[String], attribute: &str) -> String {
    walk(
        fragments,
        "''",
        &format!("return el.getAttribute({}) || '';", quote(attribute)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragments(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    /// Whether every bracket outside a string literal is closed, in order. A
    /// cheap guard against a malformed template, since no JavaScript engine
    /// runs in these tests.
    fn brackets_balance(script: &str) -> bool {
        let mut open = Vec::new();
        let mut chars = script.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' | '\'' => {
                    // Skip to the matching quote, honouring backslash escapes.
                    while let Some(inner) = chars.next() {
                        if inner == '\\' {
                            chars.next();
                        } else if inner == c {
                            break;
                        }
                    }
                }
                '(' | '{' | '[' => open.push(c),
                ')' | '}' | ']' => {
                    let opener = match c {
                        ')' => '(',
                        '}' => '{',
                        _ => '[',
                    };
                    if open.pop() != Some(opener) {
                        return false;
                    }
                }
                _ => {}
            }
        }
        open.is_empty()
    }

    #[test]
    fn split_basic_shadow_selector() {
        assert_eq!(
            split_shadow_selector("my-app ::shadow .form ::shadow input"),
            ["my-app", ".form", "input"]
        );
    }

    #[test]
    fn splitting_ignores_blank_pieces_and_untouched_selectors() {
        assert_eq!(split_shadow_selector("div.card > a"), ["div.card > a"]);
        assert_eq!(split_shadow_selector("::shadow input"), ["input"]);
        assert_eq!(split_shadow_selector("my-app ::shadow"), ["my-app"]);
        assert_eq!(split_shadow_selector("a::shadow::shadowb"), ["a", "b"]);
        assert!(split_shadow_selector("").is_empty());
        assert!(split_shadow_selector("  ::shadow  ").is_empty());
    }

    #[test]
    fn splitting_keeps_unicode_and_attribute_selectors_intact() {
        assert_eq!(
            split_shadow_selector("カード[data-name=\"é\"] ::shadow button"),
            ["カード[data-name=\"é\"]", "button"]
        );
    }

    #[test]
    fn build_query_chains_shadow_roots() {
        let script = build_shadow_query(&fragments(&["my-app", ".form", "input"]));

        assert!(script.contains("document.querySelector(\"my-app\")"));
        assert!(script.contains("el.shadowRoot.querySelector(\".form\")"));
        assert!(script.contains("el.shadowRoot.querySelector(\"input\")"));
        assert!(script.contains("if (!el) return null;"));
        assert!(script.ends_with("return el;\n})();"), "{script}");
        // The host is looked up first, then each shadow root in order.
        let host = script.find("my-app").unwrap();
        let form = script.find(".form").unwrap();
        let input = script.find("\"input\"").unwrap();
        assert!(host < form && form < input);
    }

    #[test]
    fn a_selector_with_no_shadow_root_is_one_document_lookup() {
        let script = build_shadow_query(&fragments(&["#only"]));

        assert!(script.contains("document.querySelector(\"#only\")"));
        assert!(!script.contains("shadowRoot"));
    }

    #[test]
    fn build_click_returns_boolean() {
        let script = build_shadow_click(&fragments(&["host", "button"]));

        assert!(script.contains("el.click()"));
        assert!(script.contains("return true"));
        assert!(script.contains("if (!el) return false;"));
    }

    #[test]
    fn build_type_sets_the_value_and_fires_the_events() {
        let script = build_shadow_type(&fragments(&["host", "input"]), "hello");

        assert!(script.contains("el.value = \"hello\";"));
        assert!(script.contains("new Event('input', {bubbles: true})"));
        assert!(script.contains("new Event('change', {bubbles: true})"));
        assert!(script.contains("if (!el) return false;"));
    }

    #[test]
    fn build_text_and_attribute_return_empty_text_when_missing() {
        let text = build_shadow_text(&fragments(&["host", "p"]));
        let attribute = build_shadow_attribute(&fragments(&["host", "a"]), "href");

        assert!(text.contains("if (!el) return '';"));
        assert!(text.contains("el.textContent || el.value || ''"));
        assert!(attribute.contains("el.getAttribute(\"href\") || ''"));
    }

    #[test]
    fn a_hostile_fragment_or_value_stays_inside_its_string_literal() {
        let hostile = "x\"); alert(1); (\"";
        let host_and_hostile = fragments(&["host", hostile]);

        for script in [
            build_shadow_query(&host_and_hostile),
            build_shadow_click(&host_and_hostile),
            build_shadow_type(&fragments(&["host"]), hostile),
            build_shadow_attribute(&fragments(&["host"]), hostile),
        ] {
            assert!(!script.contains("alert(1); (\")"), "{script}");
            assert!(script.contains(r#"x\"); alert(1); (\""#), "{script}");
            assert!(brackets_balance(&script), "{script}");
        }
    }

    #[test]
    fn newlines_and_unicode_in_values_do_not_break_the_script() {
        let script = build_shadow_type(&fragments(&["host", "input"]), "line one\nline two ✓ 日本");

        assert!(script.contains(r"line one\nline two"));
        assert!(brackets_balance(&script), "{script}");
    }

    #[test]
    fn no_fragments_means_nothing_to_find_and_no_panic() {
        assert_eq!(build_shadow_query(&[]), "return null;");
        assert_eq!(build_shadow_click(&[]), "return false;");
        assert_eq!(build_shadow_type(&[], "x"), "return false;");
        assert_eq!(build_shadow_text(&[]), "return '';");
        assert_eq!(build_shadow_attribute(&[], "id"), "return '';");
    }

    #[test]
    fn every_builder_makes_a_balanced_function_body() {
        let path = fragments(&["a", "b", "c"]);

        for script in [
            build_shadow_query(&path),
            build_shadow_click(&path),
            build_shadow_type(&path, "t"),
            build_shadow_text(&path),
            build_shadow_attribute(&path, "id"),
        ] {
            assert!(script.starts_with("return (function(){"), "{script}");
            assert!(script.ends_with("})();"), "{script}");
            assert!(brackets_balance(&script), "{script}");
        }
    }

    #[test]
    fn the_bracket_checker_notices_a_broken_script() {
        assert!(brackets_balance("f(\"a)\", [1]) { }"));
        assert!(!brackets_balance("f(\"a\""));
        assert!(!brackets_balance("(]"));
        assert!(!brackets_balance("}"));
    }
}
