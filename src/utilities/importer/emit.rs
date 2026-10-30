//! Writing the Rust: string literals, comments, identifiers and the file.
//!
//! The input is untrusted text and the output is code people compile, so text
//! from the input reaches the output in exactly three ways:
//!
//! - inside a string literal, through [`rust_str`], which escapes quotes,
//!   backslashes, control characters and the invisible and bidirectional
//!   characters, so no text can end the literal or hide code;
//! - inside a `//` comment, through [`comment_text`], which turns line breaks
//!   into spaces and escapes the same characters, so no text can end the
//!   comment;
//! - as an identifier, through [`test_ident`] and [`var_ident`], which keep
//!   only `a-z`, `0-9` and `_`.
//!
//! Numbers are written from parsed values, never copied.

use std::collections::BTreeSet;
use std::fmt::Write as _;

/// One line of a test body.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Line {
    /// Rust code, already safe.
    Code(String),
    /// A comment from the source, not yet neutralised.
    Comment(String),
    /// A statement that was not converted: its 1-based line, its source text
    /// and why.
    Todo {
        line: usize,
        source: String,
        reason: String,
    },
}

/// One generated test.
#[derive(Clone, Debug)]
pub(super) struct TestOut {
    /// A Rust identifier.
    pub(super) name: String,
    /// Why the test is ignored, when the source skipped it.
    pub(super) ignore: Option<String>,
    pub(super) lines: Vec<Line>,
}

/// The support functions a generated file can need.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Helper {
    FieldValue,
    ExpectFieldValue,
    ExpectEnabled,
    SelectOption,
    SetInputFiles,
    Change,
}

/// What the generated code uses besides `Browser`.
#[derive(Clone, Debug, Default)]
pub(super) struct Needs {
    /// Names imported from `seleniumbase_rs::sb_cdp`.
    pub(super) sb_cdp: BTreeSet<&'static str>,
    /// `SelectorBuf` from `seleniumbase_rs::utils::selectors`.
    pub(super) selector_buf: bool,
    pub(super) helpers: BTreeSet<Helper>,
}

/// The tool a file was imported from, for its header and its `TODO` tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tool {
    Playwright,
    Puppeteer,
}

impl Tool {
    pub(super) fn tag(self) -> &'static str {
        match self {
            Self::Playwright => "playwright",
            Self::Puppeteer => "puppeteer",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Playwright => "Playwright",
            Self::Puppeteer => "Puppeteer",
        }
    }
}

/// The longest source text a `TODO` line quotes.
const TODO_TEXT_LIMIT: usize = 240;

/// Whether `c` must be written as a `\u{...}` escape: control characters,
/// the invisible and bidirectional formatting characters (the "Trojan Source"
/// characters, which rustc also rejects), line and paragraph separators, and
/// anything Rust's own debug escaping would escape (unassigned and combining
/// characters).
fn needs_unicode_escape(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2069}'
                | '\u{feff}'
        )
        || (c != '\'' && c != '"' && c != '\\' && c.escape_debug().nth(1).is_some())
}

/// `text` as a Rust string literal.
///
/// The one way any text from the input becomes code: quotes, backslashes,
/// line breaks, NUL and every character [`needs_unicode_escape`] names are
/// escaped, so the literal always ends where it should.
pub(super) fn rust_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if needs_unicode_escape(c) => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `text` as the body of one `//` comment line: line breaks and tabs become
/// spaces, runs of white space become one, and the characters
/// [`needs_unicode_escape`] names are shown as `\u{...}`, so the text cannot
/// end the comment.
pub(super) fn comment_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' | '\r' | '\t' => out.push(' '),
            c if needs_unicode_escape(c) => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` on one line and at most [`TODO_TEXT_LIMIT`] characters long.
pub(super) fn one_line(text: &str, limit: usize) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() > limit {
        let mut cut: String = joined.chars().take(limit).collect();
        cut.push_str("...");
        cut
    } else {
        joined
    }
}

/// The text a `TODO` quotes for a statement.
pub(super) fn todo_source(text: &str) -> String {
    one_line(text, TODO_TEXT_LIMIT)
}

/// Rust's keywords, strict and reserved, in the 2021 and 2024 editions.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
    "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "typeof", "unsized", "virtual", "yield", "try", "union",
];

/// Names the generated code itself uses, which a variable must not take.
const RESERVED_LOCALS: &[&str] = &[
    "page",
    "browser",
    "support",
    "std",
    "core",
    "tokio",
    "seleniumbase_rs",
];

/// Lower-case words of `text` joined with `_`: letters and digits kept,
/// everything else a separator, and a lower-to-upper case change a word
/// break (`submitButton` is `submit_button`).
fn snake_words(text: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && previous_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
            previous_lower = false;
        }
    }
    out.trim_matches('_').to_owned()
}

/// A test name as a Rust function name: `Sign in works` is `sign_in_works`.
/// A name that would start with a digit gets `test_` in front, a keyword gets
/// `_test` after it, and an empty one is `fallback`.
pub(super) fn test_ident(title: &str, fallback: &str) -> String {
    let mut name = snake_words(title);
    // Very long titles make unreadable names; keep the first words.
    if name.len() > 80 {
        name.truncate(80);
        name = name.trim_end_matches('_').to_owned();
    }
    if name.is_empty() {
        return fallback.to_owned();
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert_str(0, "test_");
    }
    if KEYWORDS.contains(&name.as_str()) {
        name.push_str("_test");
    }
    name
}

/// A JavaScript or Python variable name as a Rust local: snake case, with
/// `_var` after a keyword or a name the generated code uses.
pub(super) fn var_ident(name: &str) -> String {
    let mut ident = snake_words(name);
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert_str(0, "v_");
    }
    if KEYWORDS.contains(&ident.as_str()) || RESERVED_LOCALS.contains(&ident.as_str()) {
        ident.push_str("_var");
    }
    ident
}

/// A finite number as a Rust float literal (`120.0`), or `None`.
pub(super) fn float(n: f64) -> Option<String> {
    n.is_finite().then(|| format!("{n:?}"))
}

/// A whole number that fits, or `None`.
pub(super) fn whole(n: f64) -> Option<u64> {
    (n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= 9.0e15).then_some(n as u64)
}

/// Renders the file: the header, the imports, the file-level `TODO`s, one
/// `#[tokio::test]` per test and the support module the tests use.
pub(super) fn render(tool: Tool, tests: &[TestOut], file_todos: &[Line], needs: &Needs) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Imported from {} code by `sbrs import-{}`.\n\
         // Each `TODO({})` line marks a statement that was not converted.\n\
         // Run headless with `SB_HEADLESS=true cargo test`.",
        tool.name(),
        tool.tag(),
        tool.tag()
    );
    out.push('\n');
    if !tests.is_empty() {
        let mut names: BTreeSet<&str> = needs.sb_cdp.clone();
        names.insert("Browser");
        let names = names.into_iter().collect::<Vec<_>>().join(", ");
        if names.contains(',') {
            let _ = writeln!(out, "use seleniumbase_rs::sb_cdp::{{{names}}};");
        } else {
            let _ = writeln!(out, "use seleniumbase_rs::sb_cdp::{names};");
        }
        out.push_str("use seleniumbase_rs::tokio;\n");

        if needs.selector_buf {
            out.push_str("use seleniumbase_rs::utils::selectors::SelectorBuf;\n");
        }
        out.push('\n');
    }
    if !file_todos.is_empty() {
        write_lines(&mut out, tool, file_todos, "");
        out.push('\n');
    }
    for test in tests {
        out.push_str("#[tokio::test]\n");
        if let Some(reason) = &test.ignore {
            let _ = writeln!(out, "#[ignore = {}]", rust_str(reason));
        }
        let _ = writeln!(
            out,
            "async fn {}() -> Result<(), Box<dyn std::error::Error>> {{",
            test.name
        );
        let uses_page = test.lines.iter().any(|line| match line {
            Line::Code(code) => mentions(code, "page"),
            _ => false,
        });
        out.push_str("    let browser = Browser::launch(support::launch_options()?).await?;\n");
        if uses_page {
            out.push_str("    let page = browser.default_page().await?;\n");
        } else {
            out.push_str("    let _page = browser.default_page().await?;\n");
        }
        write_lines(&mut out, tool, &test.lines, "    ");
        out.push_str("    browser.close().await?;\n    Ok(())\n}\n\n");
    }
    if !tests.is_empty() {
        out.push_str(&support_module(&needs.helpers));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn write_lines(out: &mut String, tool: Tool, lines: &[Line], indent: &str) {
    for line in lines {
        match line {
            Line::Code(code) => {
                let _ = writeln!(out, "{indent}{code}");
            }
            Line::Comment(text) => {
                let text = comment_text(text);
                if text.is_empty() {
                    let _ = writeln!(out, "{indent}//");
                } else {
                    let _ = writeln!(out, "{indent}// {text}");
                }
            }
            Line::Todo {
                line,
                source,
                reason,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}// TODO({}): {}",
                    tool.tag(),
                    comment_text(source)
                );
                let _ = writeln!(out, "{indent}//     line {line}: {}", comment_text(reason));
            }
        }
    }
}

/// Whether `code` uses the name `word` (not as part of a longer name).
pub(super) fn mentions(code: &str, word: &str) -> bool {
    let bytes = code.as_bytes();
    let mut from = 0;
    while let Some(found) = code[from..].find(word) {
        let at = from + found;
        let end = at + word.len();
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(end).copied();
        let boundary = |b: Option<u8>| !b.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
        if boundary(before) && boundary(after) && before != Some(b'"') {
            return true;
        }
        from = end;
    }
    false
}

/// The support module: the launch options every test uses, and the helpers
/// in `helpers`.
fn support_module(helpers: &BTreeSet<Helper>) -> String {
    let mut helpers = helpers.clone();
    if helpers.contains(&Helper::ExpectFieldValue) {
        helpers.insert(Helper::FieldValue);
    }
    let polls =
        helpers.contains(&Helper::ExpectFieldValue) || helpers.contains(&Helper::ExpectEnabled);
    let mut imports = vec!["LaunchOptions"];
    if !helpers.is_empty() {
        imports.push("Locator");
    }
    if helpers.contains(&Helper::SelectOption) || helpers.contains(&Helper::Change) {
        imports.push("SelectBy");
    }
    let mut out = String::from(
        "/// What the imported tests share: how the browser is launched, and the\n\
         /// checks that the pure CDP engine has no single call for.\n\
         mod support {\n",
    );
    if polls {
        out.push_str("    use std::time::{Duration, Instant};\n\n");
    }
    out.push_str("    use seleniumbase_rs::config::settings::Settings;\n");
    let _ = writeln!(
        out,
        "    use seleniumbase_rs::sb_cdp::{};\n",
        if imports.len() == 1 {
            imports[0].to_owned()
        } else {
            format!("{{{}}}", imports.join(", "))
        }
    );
    if polls {
        out.push_str(
            "    /// How often a check looks again while it waits.\n\
             \x20   const POLL: Duration = Duration::from_millis(100);\n\n",
        );
    }
    out.push_str(LAUNCH_OPTIONS);
    for helper in &helpers {
        out.push('\n');
        out.push_str(match helper {
            Helper::FieldValue => FIELD_VALUE,
            Helper::ExpectFieldValue => EXPECT_FIELD_VALUE,
            Helper::ExpectEnabled => EXPECT_ENABLED,
            Helper::SelectOption => SELECT_OPTION,
            Helper::SetInputFiles => SET_INPUT_FILES,
            Helper::Change => CHANGE,
        });
    }
    out.push_str("}\n");
    out
}

const LAUNCH_OPTIONS: &str = r#"    /// The launch options: headless when `SB_HEADLESS` says so, as for the rest
    /// of `seleniumbase-rs`; without it, the engine's default.
    pub fn launch_options() -> Result<LaunchOptions, seleniumbase_rs::SeleniumBaseError> {
        let mut builder = LaunchOptions::builder();
        if std::env::var_os("SB_HEADLESS").is_some() {
            builder = builder.headless(Settings::from_env()?.headless);
        }
        builder.build()
    }
"#;

const FIELD_VALUE: &str = r#"    /// The current value of a form field (`inputValue`). `Locator` has no
    /// reader for it, so the field is focused and the focused element read.
    pub async fn field_value(field: &Locator) -> Result<String, Box<dyn std::error::Error>> {
        field.focus().await?;
        let value = field
            .page()
            .evaluate("String(document.activeElement ? document.activeElement.value ?? '' : '')")
            .await?;
        Ok(value.as_str().unwrap_or_default().to_owned())
    }
"#;

const EXPECT_FIELD_VALUE: &str = r#"    /// Waits until the field's value is `expected` (or, with `negate`, is
    /// not), as `toHaveValue` does.
    pub async fn expect_field_value(
        field: &Locator,
        expected: &str,
        negate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + field.timeout();
        loop {
            let actual = field_value(field).await?;
            if (actual == expected) != negate {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let not = if negate { "not " } else { "" };
                return Err(format!(
                    "expected {field} {not}to have value {expected:?}, but it was {actual:?}"
                )
                .into());
            }
            tokio::time::sleep(POLL).await;
        }
    }
"#;

const EXPECT_ENABLED: &str = r#"    /// Waits until the control is enabled (`true`) or disabled (`false`), as
    /// `toBeEnabled` and `toBeDisabled` do.
    pub async fn expect_enabled(
        field: &Locator,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + field.timeout();
        loop {
            if field.is_enabled().await? == enabled {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let state = if enabled { "enabled" } else { "disabled" };
                return Err(format!("expected {field} to be {state}").into());
            }
            tokio::time::sleep(POLL).await;
        }
    }
"#;

const SELECT_OPTION: &str = r#"    /// Chooses the option whose value is `wanted`, or else the one whose text
    /// is, as `selectOption("...")` does.
    pub async fn select_option(
        field: &Locator,
        wanted: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if field.select_option(SelectBy::Value(wanted)).await.is_err() {
            field.select_option(SelectBy::Text(wanted)).await?;
        }
        Ok(())
    }
"#;

const CHANGE: &str = r#"    /// Sets a field to `value` the way the Chrome Recorder's `change` step
    /// does: a `<select>` chooses the option with that value, anything else is
    /// filled.
    pub async fn change(field: &Locator, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        if field.info().await?.tag == "select" {
            field.select_option(SelectBy::Value(value)).await?;
        } else {
            field.fill(value).await?;
        }
        Ok(())
    }
"#;

const SET_INPUT_FILES: &str = r#"    /// Puts the file at `path` into a file input, as `setInputFiles` does: the
    /// input is focused and the focused element handed to
    /// `DOM.setFileInputFiles`.
    pub async fn set_input_files(
        field: &Locator,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::fs::canonicalize(path)?;
        field.focus().await?;
        let page = field.page();
        let query = page
            .evaluate("({ expression: 'document.activeElement' })")
            .await?;
        let found = page.execute("Runtime.evaluate", query).await?;
        let mut params = page.evaluate("({ files: [''] })").await?;
        params["files"][0] = path.to_string_lossy().as_ref().into();
        params["objectId"] = found["result"]["objectId"].clone();
        page.execute("DOM.setFileInputFiles", params).await?;
        Ok(())
    }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_literal_cannot_be_ended_by_its_text() {
        assert_eq!(rust_str(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(rust_str("line\nbreak\r\t\0"), r#""line\nbreak\r\t\0""#);
        assert_eq!(rust_str("\"#; evil(); r#\""), r##""\"#; evil(); r#\"""##);
        assert_eq!(rust_str("\u{202e}x\u{2028}"), r#""\u{202e}x\u{2028}""#);
        assert_eq!(rust_str("\u{7}"), r#""\u{7}""#);
        assert_eq!(rust_str("Café ü 日本"), "\"Café ü 日本\"");
        assert_eq!(rust_str("it's"), "\"it's\"");
    }

    #[test]
    fn string_literals_parse_back_to_the_same_text() {
        for text in [
            "plain",
            "\"quoted\" and \\backslash\\",
            "\u{0}\u{1}\u{1f}\u{7f}\u{85}",
            "\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{2066}\u{2067}\u{2068}\u{2069}",
            "e\u{301}",
            "*/ /* // \"#",
            "\u{feff}\u{200b}",
        ] {
            let literal = rust_str(text);
            let parsed: syn::LitStr =
                syn::parse_str(&literal).unwrap_or_else(|e| panic!("{literal}: {e}"));
            assert_eq!(parsed.value(), text, "{literal}");
        }
    }

    #[test]
    fn comment_text_stays_on_one_line() {
        assert_eq!(comment_text("a\nb\r\nc\td"), "a b c d");
        assert_eq!(comment_text("x\u{2028}evil();"), "x\\u{2028}evil();");
        assert_eq!(comment_text("*/ code"), "*/ code");
        assert_eq!(comment_text("\u{202e}"), "\\u{202e}");
    }

    #[test]
    fn identifiers_keep_only_safe_characters() {
        assert_eq!(test_ident("Sign in works", "t"), "sign_in_works");
        assert_eq!(
            test_ident("shop > checkout (guest)", "t"),
            "shop_checkout_guest"
        );
        assert_eq!(test_ident("2 items", "t"), "test_2_items");
        assert_eq!(test_ident("type", "t"), "type_test");
        assert_eq!(test_ident("日本", "fallback"), "fallback");
        assert_eq!(test_ident("a\"); evil(); //", "t"), "a_evil");
        assert_eq!(var_ident("submitButton"), "submit_button");
        assert_eq!(var_ident("BASE_URL"), "base_url");
        assert_eq!(var_ident("type"), "type_var");
        assert_eq!(var_ident("page"), "page_var");
        assert_eq!(var_ident("$el"), "el");
        assert_eq!(var_ident("9lives"), "v_9lives");
    }

    #[test]
    fn numbers_are_written_from_values() {
        assert_eq!(float(120.0).as_deref(), Some("120.0"));
        assert_eq!(float(0.5).as_deref(), Some("0.5"));
        assert_eq!(float(f64::NAN), None);
        assert_eq!(whole(3.0), Some(3));
        assert_eq!(whole(-1.0), None);
        assert_eq!(whole(1.5), None);
    }

    #[test]
    fn a_todo_names_the_tool_line_and_reason_on_two_comment_lines() {
        let rendered = render(
            Tool::Playwright,
            &[TestOut {
                name: "t".into(),
                ignore: Some("skipped \"here\"".into()),
                lines: vec![
                    Line::Code("page.reload().await?;".into()),
                    Line::Todo {
                        line: 7,
                        source: "await page.route('**', r => r.abort())\nevil()".into(),
                        reason: "request interception".into(),
                    },
                ],
            }],
            &[],
            &Needs::default(),
        );
        assert!(
            rendered.contains("#[ignore = \"skipped \\\"here\\\"\"]"),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "    // TODO(playwright): await page.route('**', r => r.abort()) evil()\n    //     line 7: request interception\n"
            ),
            "{rendered}"
        );
        syn::parse_file(&rendered).unwrap_or_else(|e| panic!("{e}\n{rendered}"));
    }

    #[test]
    fn mentions_finds_whole_names_only() {
        assert!(mentions("page.goto(\"x\")", "page"));
        assert!(!mentions("page_two.goto()", "page"));
        assert!(!mentions("let homepage = 1;", "page"));
        assert!(mentions("&title)", "title"));
    }
}
