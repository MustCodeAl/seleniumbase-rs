//! Building blocks for the JavaScript this crate sends to a page.
//!
//! Most of the JavaScript the crate runs is written where it is used: the Pure
//! CDP engine's page helper is `src/sb_cdp/helper.js`, the stealth scripts are
//! under `src/stealth/js`, and the WebDriver `BaseCase` builds short scripts
//! on the fly (for example the shadow DOM walkers in
//! [`utils::shadow`](crate::utils::shadow)). What they share is this: a value
//! that came from a caller, such as a selector, a piece of text or an attribute
//! name, has to be put into a script as a *string literal*.
//!
//! Pasting it between quotes (`format!("document.querySelector('{css}')")`)
//! breaks on the first apostrophe and lets a selector such as `x'); alert(1);
//! ('` run code in the page. [`quote`] builds the literal instead.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::js_code::quote;
//!
//! let css = r#"a[title="it's"]"#;
//! let script = format!("return document.querySelector({}) !== null;", quote(css));
//! assert_eq!(
//!     script,
//!     r#"return document.querySelector("a[title=\"it's\"]") !== null;"#
//! );
//! ```

/// `value` as a JavaScript string literal, quotes included, safe to place
/// anywhere an expression is allowed.
///
/// The result is a JSON string, which is also a valid JavaScript string, with
/// a few characters escaped further so it stays harmless wherever it ends up:
/// `<`, `>` and `&` (so it cannot close a `<script>` element or open a comment
/// if the script is ever embedded in HTML), and the line separators U+2028 and
/// U+2029 (a line terminator inside a literal in older engines).
///
/// Reading the literal back in JavaScript gives exactly `value`.
#[must_use]
pub fn quote(value: &str) -> String {
    // Serializing a `&str` cannot fail.
    let json = serde_json::to_string(value).unwrap_or_else(|_| String::from("\"\""));
    let mut literal = String::with_capacity(json.len());
    for c in json.chars() {
        match c {
            '<' => literal.push_str("\\u003c"),
            '>' => literal.push_str("\\u003e"),
            '&' => literal.push_str("\\u0026"),
            '\u{2028}' => literal.push_str("\\u2028"),
            '\u{2029}' => literal.push_str("\\u2029"),
            other => literal.push(other),
        }
    }
    literal
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a JavaScript engine reads back from the literal. Every escape
    /// `quote` produces is also valid JSON, so a JSON parser stands in for one.
    fn read_back(literal: &str) -> String {
        serde_json::from_str(literal).unwrap()
    }

    #[test]
    fn plain_text_is_wrapped_in_double_quotes() {
        assert_eq!(quote("div.card .item"), r#""div.card .item""#);
        assert_eq!(quote(""), r#""""#);
        assert_eq!(quote("#login"), r##""#login""##);
        // `>` is escaped like `<`, so a child combinator reads back unchanged.
        assert_eq!(quote("ul > li"), "\"ul \\u003e li\"");
        assert_eq!(read_back(&quote("ul > li")), "ul > li");
    }

    #[test]
    fn quotes_backslashes_and_newlines_are_escaped() {
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quote(r"C:\dir"), r#""C:\\dir""#);
        assert_eq!(quote("a\nb\r\tc"), r#""a\nb\r\tc""#);
    }

    #[test]
    fn an_apostrophe_cannot_end_the_literal() {
        let hostile = "x'); alert(1); ('";
        let literal = quote(hostile);

        assert!(literal.starts_with('"') && literal.ends_with('"'));
        // The apostrophes are inside a double-quoted literal, so they are data.
        assert_eq!(read_back(&literal), hostile);
        assert!(!literal[1..literal.len() - 1].contains('"'));
    }

    #[test]
    fn a_closing_script_tag_cannot_appear_in_the_literal() {
        let literal = quote("</script><!-- &amp;");

        assert!(!literal.contains("</script"), "{literal}");
        assert!(!literal.contains("<!--"), "{literal}");
        assert!(!literal.contains('&'), "{literal}");
        assert_eq!(read_back(&literal), "</script><!-- &amp;");
    }

    #[test]
    fn line_separators_are_escaped() {
        let literal = quote("a\u{2028}b\u{2029}c");

        assert!(!literal.contains('\u{2028}') && !literal.contains('\u{2029}'));
        assert_eq!(literal, r#""a\u2028b\u2029c""#);
        assert_eq!(read_back(&literal), "a\u{2028}b\u{2029}c");
    }

    #[test]
    fn unicode_and_control_characters_survive_the_round_trip() {
        for text in [
            "日本語 тест ünï",
            "emoji 🦀 and 👩‍💻",
            "nul \u{0} bell \u{7} esc \u{1b}",
            "lone surrogate-ish \u{FFFD}",
            "\u{7f}\u{80}\u{9f}",
        ] {
            assert_eq!(read_back(&quote(text)), text);
        }
    }
}
