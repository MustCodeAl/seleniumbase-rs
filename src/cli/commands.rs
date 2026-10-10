//! Reusable, browser-independent helpers backing the `sbase` CLI subcommands.
//!
//! The logic here is deliberately free of any WebDriver dependency so it can be
//! unit tested without a live browser: deferred assertion parsing, download file
//! name resolution, and the synthetic upload page used by `sbase choose-file`.

use std::io::{self, Write};
use std::path::Path;

use crate::error::SeleniumBaseError;

/// CSS pseudo-classes and pseudo-elements, to tell `li:first-child:Hello` (the
/// selector `li:first-child`, the text `Hello`) from `li:Hello`.
const CSS_PSEUDOS: &[&str] = &[
    "active",
    "after",
    "any-link",
    "before",
    "checked",
    "default",
    "disabled",
    "empty",
    "enabled",
    "first",
    "first-child",
    "first-letter",
    "first-line",
    "first-of-type",
    "focus",
    "focus-visible",
    "focus-within",
    "has",
    "hover",
    "in-range",
    "indeterminate",
    "invalid",
    "is",
    "lang",
    "last-child",
    "last-of-type",
    "link",
    "not",
    "nth-child",
    "nth-last-child",
    "nth-last-of-type",
    "nth-of-type",
    "only-child",
    "only-of-type",
    "optional",
    "out-of-range",
    "placeholder-shown",
    "read-only",
    "read-write",
    "required",
    "root",
    "selection",
    "target",
    "valid",
    "visited",
    "where",
];

/// Splits `selector:expected` at the colon that ends the selector.
///
/// Colons inside `[...]`, `(...)`, quotes, after a backslash (`#a\:b`) or
/// starting a known pseudo-class (`:nth-child(2)`, `::before`) belong to the
/// selector. If that leaves no separator (`a:hover`), the first colon is used,
/// as it always was.
fn split_text_spec(rest: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut first_colon = None;
    for (index, c) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' | '\'' if depth > 0 => match quote {
                Some(open) if open == c => quote = None,
                None => quote = Some(c),
                Some(_) => {}
            },
            _ if quote.is_some() => {}
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => {
                first_colon.get_or_insert(index);
                let after = &rest[index + 1..];
                let name_end = after
                    .find(|c: char| !(c.is_ascii_alphabetic() || c == '-'))
                    .unwrap_or(after.len());
                let name = &after[..name_end];
                let is_pseudo = after.starts_with(':')
                    || (CSS_PSEUDOS.contains(&name)
                        && matches!(after[name_end..].chars().next(), None | Some(':' | '(')));
                if !is_pseudo {
                    return Some((&rest[..index], after));
                }
            }
            _ => {}
        }
    }
    first_colon.map(|index| (&rest[..index], &rest[index + 1..]))
}

/// A single assertion requested through `sbase deferred`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeferredSpec {
    /// Assert that an element matching the selector exists.
    Element(String),
    /// Assert that the element's text contains the expected value.
    Text(String, String),
    /// Assert that the page title contains the expected value.
    Title(String),
    /// Assert that the current URL contains the expected value.
    Url(String),
}

impl DeferredSpec {
    /// Parses one `key=value` assertion spec.
    ///
    /// Supported forms:
    ///
    /// - `element=<selector>`
    /// - `text=<selector>:<expected>`
    /// - `title=<expected>`
    /// - `url=<expected>`
    ///
    /// In a `text=` spec the selector ends at the first colon that is not part
    /// of it. A colon inside brackets, parentheses or quotes, one escaped with a
    /// backslash, and one that starts a CSS pseudo-class (`:nth-child(2)`,
    /// `:hover`) stay in the selector; the expected text may contain colons.
    /// With nothing else to go on (`a:hover`) the first colon splits, so
    /// `text=a:hover` means the text `hover` in `a`.
    ///
    /// ```
    /// use seleniumbase_rs::cli::commands::DeferredSpec;
    ///
    /// let spec = DeferredSpec::parse("text=#msg:Welcome").unwrap();
    /// assert_eq!(spec, DeferredSpec::Text("#msg".into(), "Welcome".into()));
    ///
    /// let spec = DeferredSpec::parse("text=li:nth-child(2):Time: 10:30").unwrap();
    /// assert_eq!(
    ///     spec,
    ///     DeferredSpec::Text("li:nth-child(2)".into(), "Time: 10:30".into())
    /// );
    /// assert!(DeferredSpec::parse("nope=1").is_err());
    /// ```
    pub fn parse(raw: &str) -> Result<Self, SeleniumBaseError> {
        let trimmed = raw.trim();
        let (kind, rest) = trimmed.split_once('=').ok_or_else(|| {
            SeleniumBaseError::InvalidConfig(format!(
                "invalid assertion spec '{trimmed}': expected <kind>=<value>"
            ))
        })?;
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "invalid assertion spec '{trimmed}': missing value"
            )));
        }
        match kind.trim().to_ascii_lowercase().as_str() {
            "element" => Ok(Self::Element(rest.to_owned())),
            "text" => {
                let (selector, expected) = split_text_spec(rest).ok_or_else(|| {
                    SeleniumBaseError::InvalidConfig(format!(
                        "invalid assertion spec '{trimmed}': expected text=<selector>:<expected>"
                    ))
                })?;
                let selector = selector.trim();
                let expected = expected.trim();
                if selector.is_empty() || expected.is_empty() {
                    return Err(SeleniumBaseError::InvalidConfig(format!(
                        "invalid assertion spec '{trimmed}': selector and text are required"
                    )));
                }
                Ok(Self::Text(selector.to_owned(), expected.to_owned()))
            }
            "title" => Ok(Self::Title(rest.to_owned())),
            "url" => Ok(Self::Url(rest.to_owned())),
            other => Err(SeleniumBaseError::InvalidConfig(format!(
                "unknown assertion kind '{other}': expected element, text, title, or url"
            ))),
        }
    }

    /// Human-readable description used in the CLI summary output.
    pub fn describe(&self) -> String {
        match self {
            Self::Element(css) => format!("element exists: {css}"),
            Self::Text(css, text) => format!("text {text:?} in {css}"),
            Self::Title(title) => format!("title contains {title:?}"),
            Self::Url(url) => format!("url contains {url:?}"),
        }
    }
}

/// Parses a list of assertion specs, reporting every invalid entry at once.
pub fn parse_deferred_specs(raw: &[String]) -> Result<Vec<DeferredSpec>, SeleniumBaseError> {
    let mut specs = Vec::with_capacity(raw.len());
    let mut errors = Vec::new();
    for entry in raw {
        match DeferredSpec::parse(entry) {
            Ok(spec) => specs.push(spec),
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(specs)
    } else {
        Err(SeleniumBaseError::InvalidConfig(errors.join("; ")))
    }
}

/// Extracts assertion specs from a JSON spec file.
///
/// Accepts a bare array of strings or an object with an `asserts` key. The
/// object must have that key and no other: a file with `"assert"` or
/// `"assertions"` is an error, not a file with no assertions to fail.
///
/// ```
/// use seleniumbase_rs::cli::commands::parse_deferred_spec_json;
///
/// let specs = parse_deferred_spec_json(r#"["element=#a","title=Hi"]"#).unwrap();
/// assert_eq!(specs.len(), 2);
///
/// let specs = parse_deferred_spec_json(r#"{"asserts":["url=/demo"]}"#).unwrap();
/// assert_eq!(specs.len(), 1);
///
/// assert!(parse_deferred_spec_json(r#"{"assert":["url=/demo"]}"#).is_err());
/// ```
pub fn parse_deferred_spec_json(raw: &str) -> Result<Vec<DeferredSpec>, SeleniumBaseError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SpecFile {
        asserts: Vec<String>,
    }

    let invalid = |e: serde_json::Error| {
        SeleniumBaseError::InvalidConfig(format!("failed to parse deferred spec file: {e}"))
    };
    let entries = if raw.trim_start().starts_with('[') {
        serde_json::from_str::<Vec<String>>(raw).map_err(invalid)?
    } else {
        serde_json::from_str::<SpecFile>(raw)
            .map_err(invalid)?
            .asserts
    };
    parse_deferred_specs(&entries)
}

/// Result of running one deferred assertion.
#[derive(Clone, Debug)]
pub struct DeferredResult {
    /// Description of the assertion that ran.
    pub description: String,
    /// Failure message, or `None` when the assertion passed.
    pub error: Option<String>,
}

impl DeferredResult {
    /// Returns `true` when the assertion completed without an error.
    ///
    /// ```
    /// use seleniumbase_rs::cli::commands::DeferredResult;
    ///
    /// let ok = DeferredResult { description: "title".into(), error: None };
    /// assert!(ok.passed());
    /// ```
    pub fn passed(&self) -> bool {
        self.error.is_none()
    }
}

/// Formats a deferred-assertion summary block.
pub fn format_deferred_summary(results: &[DeferredResult]) -> String {
    let failed = results.iter().filter(|r| r.error.is_some()).count();
    let passed = results.len() - failed;
    let mut out = String::from("Deferred assertion summary\n");
    out.push_str("--------------------------\n");
    for result in results {
        match &result.error {
            None => out.push_str(&format!("PASS  {}\n", result.description)),
            Some(error) => out.push_str(&format!("FAIL  {} -> {}\n", result.description, error)),
        }
    }
    out.push_str(&format!(
        "--------------------------\ntotal={} passed={} failed={}\n",
        results.len(),
        passed,
        failed
    ));
    out
}

/// Derives a download file name from a URL.
///
/// Falls back to `download.bin` when the URL has no usable final path segment,
/// including a segment that is only dots (`.` and `..` name folders, not
/// files). Characters that no common file system accepts in a name, and
/// control characters, are dropped; the result never contains a path separator.
///
/// ```
/// use seleniumbase_rs::cli::commands::download_file_name;
///
/// assert_eq!(download_file_name("https://x.io/a/report.pdf?v=2"), "report.pdf");
/// assert_eq!(download_file_name("https://x.io/"), "download.bin");
/// assert_eq!(download_file_name("https://x.io/a/.."), "download.bin");
/// ```
pub fn download_file_name(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let path = match without_query.split_once("://") {
        // Skip the authority component so bare hosts do not become file names.
        Some((_, rest)) => rest.split_once('/').map(|(_, tail)| tail).unwrap_or(""),
        None if without_query.starts_with("data:") => "",
        None => without_query,
    };
    // A Windows path (`C:\files\a.zip`) is a name too, split at its backslashes.
    let candidate = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    let sanitized: String = candidate
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '"' | '<' | '>' | '|'))
        .collect();
    if sanitized.chars().all(|c| c == '.') {
        "download.bin".to_owned()
    } else {
        sanitized
    }
}

/// Returns `true` when a URL or path most likely points at a PDF document.
pub fn looks_like_pdf(url: &str) -> bool {
    download_file_name(url)
        .to_ascii_lowercase()
        .ends_with(".pdf")
}

/// The attributes that make `<input type="file">` match a simple selector.
#[derive(Default)]
struct InputAttributes {
    id: Option<String>,
    classes: Vec<String>,
    name: Option<String>,
}

/// Reads a compound selector made of an optional `input`, then any of `#id`,
/// `.class`, `[name=value]` and `[type=file]`, such as `input#upload.big`.
/// Returns `None` for anything else (a descendant, a pseudo-class, a second
/// id), which no single generated input can be made to match.
fn parse_input_selector(selector: &str) -> Option<InputAttributes> {
    fn ident_end(text: &str) -> usize {
        text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(text.len())
    }

    let mut attrs = InputAttributes::default();
    let mut rest = selector.trim();
    if rest
        .get(..5)
        .is_some_and(|tag| tag.eq_ignore_ascii_case("input"))
    {
        rest = &rest[5..];
    }
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('#') {
            let end = ident_end(after);
            if end == 0 || attrs.id.is_some() {
                return None;
            }
            attrs.id = Some(after[..end].to_owned());
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix('.') {
            let end = ident_end(after);
            if end == 0 {
                return None;
            }
            attrs.classes.push(after[..end].to_owned());
            rest = &after[end..];
        } else {
            // Anything but `#`, `.` or `[` is not a compound selector.
            let after = rest.strip_prefix('[')?;
            let close = after.find(']')?;
            let (attribute, value) = after[..close].split_once('=')?;
            let value = value.trim().trim_matches(['"', '\'']);
            match attribute.trim() {
                "name" if attrs.name.is_none() => attrs.name = Some(value.to_owned()),
                "type" if value.eq_ignore_ascii_case("file") => {}
                _ => return None,
            }
            rest = &after[close + 1..];
        }
    }
    Some(attrs)
}

/// Builds a blank data-URL page containing a file input that matches `selector`.
///
/// The selector may be an optional `input` followed by any of `#id`,
/// `.class`, `[name=...]` and `[type=file]`: `#upload`, `input.picker.big`,
/// `input#doc[name=doc]`. Anything else (a descendant, a pseudo-class) falls
/// back to a plain `input[type=file]` with the id `sbase-file-input`.
///
/// ```
/// use seleniumbase_rs::cli::commands::blank_file_input_page;
///
/// assert!(blank_file_input_page("#upload").contains("id=\"upload\""));
/// assert!(blank_file_input_page(".picker").contains("class=\"picker\""));
/// let page = blank_file_input_page("input#upload.a.b");
/// assert!(page.contains("id=\"upload\"") && page.contains("class=\"a b\""));
/// ```
pub fn blank_file_input_page(selector: &str) -> String {
    let attributes = parse_input_selector(selector).unwrap_or_default();
    let input_id = html_attr_escape(attributes.id.as_deref().unwrap_or("sbase-file-input"));
    let mut attrs = String::new();
    if !attributes.classes.is_empty() {
        attrs.push_str(&format!(
            " class=\"{}\"",
            html_attr_escape(&attributes.classes.join(" "))
        ));
    }
    if let Some(name) = &attributes.name {
        attrs.push_str(&format!(" name=\"{}\"", html_attr_escape(name)));
    }

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>sbase choose-file</title></head><body><main>\
<h1>File upload check</h1>\
<label for=\"{input_id}\">Select a file</label>\
<input type=\"file\" id=\"{input_id}\"{attrs}>\
</main></body></html>"
    )
}

/// Wraps HTML in a `data:` URL so it can be opened without a local web server.
pub fn html_data_url(html: &str) -> String {
    let mut encoded = String::with_capacity(html.len() * 2);
    for byte in html.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("data:text/html;charset=utf-8,{encoded}")
}

/// Creates `path` holding `contents`, and refuses to replace a file that
/// already exists.
///
/// The `sbase mk*` commands write starter files into the current folder; a
/// second run must not erase the test or page the user has since edited.
/// Missing parent folders are created. An existing file (or a link at that
/// path) is an [`io::ErrorKind::AlreadyExists`] error that names the path.
///
/// ```
/// use seleniumbase_rs::cli::commands::write_new_file;
///
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("out").join("page.html");
/// write_new_file(&path, "first").unwrap();
/// assert!(write_new_file(&path, "second").is_err());
/// assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
/// ```
pub fn write_new_file(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists; remove it or choose another name",
                    path.display()
                ),
            ),
            _ => error,
        })?;
    file.write_all(contents.as_bytes())
}

fn html_attr_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_spec_kind() {
        assert_eq!(
            DeferredSpec::parse("element=#login").unwrap(),
            DeferredSpec::Element("#login".to_owned())
        );
        assert_eq!(
            DeferredSpec::parse("text=#msg:Hello there").unwrap(),
            DeferredSpec::Text("#msg".to_owned(), "Hello there".to_owned())
        );
        assert_eq!(
            DeferredSpec::parse("TITLE=Demo Page").unwrap(),
            DeferredSpec::Title("Demo Page".to_owned())
        );
        assert_eq!(
            DeferredSpec::parse(" url=/demo_page ").unwrap(),
            DeferredSpec::Url("/demo_page".to_owned())
        );
    }

    #[test]
    fn text_spec_keeps_colons_in_expected_value() {
        assert_eq!(
            DeferredSpec::parse("text=#t:a:b").unwrap(),
            DeferredSpec::Text("#t".to_owned(), "a:b".to_owned())
        );
    }

    #[test]
    fn rejects_malformed_specs() {
        for bad in [
            "element",
            "element=",
            "bogus=1",
            "text=#only-selector",
            "text=:x",
        ] {
            assert!(
                DeferredSpec::parse(bad).is_err(),
                "expected error for {bad}"
            );
        }
    }

    #[test]
    fn parse_many_reports_all_errors() {
        let raw = vec![
            "element=#ok".to_owned(),
            "bad".to_owned(),
            "worse".to_owned(),
        ];
        let err = parse_deferred_specs(&raw).unwrap_err().to_string();
        assert!(err.contains("bad"));
        assert!(err.contains("worse"));
    }

    #[test]
    fn parses_json_spec_files() {
        let list = parse_deferred_spec_json(r#"["element=#a", "text=#b:c"]"#).unwrap();
        assert_eq!(list.len(), 2);

        let wrapped = parse_deferred_spec_json(r#"{"asserts": ["title=Demo"]}"#).unwrap();
        assert_eq!(wrapped, vec![DeferredSpec::Title("Demo".to_owned())]);

        assert!(parse_deferred_spec_json("nope").is_err());
        assert!(parse_deferred_spec_json(r#"["oops"]"#).is_err());
    }

    #[test]
    fn describes_specs() {
        assert_eq!(
            DeferredSpec::parse("element=#a").unwrap().describe(),
            "element exists: #a"
        );
        assert!(DeferredSpec::parse("url=/x")
            .unwrap()
            .describe()
            .contains("url contains"));
    }

    #[test]
    fn formats_summary_with_counts() {
        let results = vec![
            DeferredResult {
                description: "element exists: #a".to_owned(),
                error: None,
            },
            DeferredResult {
                description: "title contains \"X\"".to_owned(),
                error: Some("not found".to_owned()),
            },
        ];
        let summary = format_deferred_summary(&results);
        assert!(summary.contains("PASS  element exists: #a"));
        assert!(summary.contains("FAIL  title contains \"X\" -> not found"));
        assert!(summary.contains("total=2 passed=1 failed=1"));
    }

    #[test]
    fn derives_download_names() {
        assert_eq!(download_file_name("https://x.io/a/b/file.zip"), "file.zip");
        assert_eq!(download_file_name("https://x.io/f.pdf#page=2"), "f.pdf");
        assert_eq!(download_file_name("https://x.io"), "download.bin");
        assert_eq!(download_file_name("https://x.io/"), "download.bin");
        assert_eq!(download_file_name("report.pdf"), "report.pdf");
        assert_eq!(
            download_file_name("data:text/html,<p>x</p>"),
            "download.bin"
        );
        assert_eq!(download_file_name(""), "download.bin");
    }

    #[test]
    fn detects_pdf_urls() {
        assert!(looks_like_pdf("https://x.io/report.PDF?x=1"));
        assert!(!looks_like_pdf("https://x.io/index.html"));
    }

    #[test]
    fn builds_matching_upload_pages() {
        assert!(blank_file_input_page("#upload").contains("id=\"upload\""));
        assert!(!blank_file_input_page("#upload").contains("sbase-file-input"));
        assert!(blank_file_input_page(".picker").contains("class=\"picker\""));
        assert!(blank_file_input_page("input[name=doc]").contains("name=\"doc\""));
        assert!(blank_file_input_page("input[type=file]").contains("type=\"file\""));
    }

    #[test]
    fn escapes_attribute_values() {
        assert!(blank_file_input_page("input[name='a\"b']").contains("name=\"a&quot;b\""));
    }

    #[test]
    fn the_input_matches_a_compound_selector() {
        let page = blank_file_input_page("input#upload.big.wide[name=doc]");
        assert!(
            page.contains("id=\"upload\" class=\"big wide\" name=\"doc\""),
            "{page}"
        );
        assert!(blank_file_input_page("INPUT.x").contains("class=\"x\""));
    }

    #[test]
    fn a_selector_it_cannot_satisfy_gets_the_plain_input() {
        for selector in [
            "",
            "div input",
            "#a #b",
            "#a b",
            "input:checked",
            "ñ",
            "#",
            ".",
        ] {
            let page = blank_file_input_page(selector);
            assert!(
                page.contains("id=\"sbase-file-input\""),
                "{selector:?}: {page}"
            );
            assert!(!page.contains("class="), "{selector:?}");
        }
    }

    #[test]
    fn a_colon_inside_the_selector_does_not_end_it() {
        for (spec, selector, expected) in [
            ("text=li:nth-child(2):Hello", "li:nth-child(2)", "Hello"),
            (
                "text=a[href=\"http://x.io\"]:Go",
                "a[href=\"http://x.io\"]",
                "Go",
            ),
            (
                "text=a[href='http://x.io']:Go",
                "a[href='http://x.io']",
                "Go",
            ),
            ("text=#a\\:b:Hi", "#a\\:b", "Hi"),
            ("text=input:checked:Yes", "input:checked", "Yes"),
            ("text=p::first-line:abc", "p::first-line", "abc"),
            (
                "text=li:not(.x):last-child:Bye",
                "li:not(.x):last-child",
                "Bye",
            ),
            ("text=#msg:Time: 10:30", "#msg", "Time: 10:30"),
            ("text=a:hover", "a", "hover"),
        ] {
            assert_eq!(
                DeferredSpec::parse(spec).unwrap(),
                DeferredSpec::Text(selector.to_owned(), expected.to_owned()),
                "{spec}"
            );
        }
    }

    #[test]
    fn a_text_spec_with_only_a_selector_colon_is_rejected() {
        for bad in ["text=a[href=\"http://x.io\"]", "text=li:nth-child(2):"] {
            assert!(DeferredSpec::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_spec_file_with_a_misspelled_key_is_an_error() {
        for bad in [
            r#"{"assert": ["title=X"]}"#,
            r#"{"asserts": ["title=X"], "extra": 1}"#,
            "{}",
            r#"[1, 2]"#,
            r#"{"asserts": "title=X"}"#,
        ] {
            assert!(parse_deferred_spec_json(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_deferred_spec_json("  [ \"title=X\" ]").unwrap(),
            vec![DeferredSpec::Title("X".to_owned())]
        );
    }

    #[test]
    fn a_download_name_is_never_a_folder() {
        for url in [
            "https://x.io/a/..",
            "https://x.io/a/.",
            "https://x.io/..",
            "https://x.io/a/../",
            "../..",
            "..",
            "...",
        ] {
            assert_eq!(download_file_name(url), "download.bin", "{url}");
        }
    }

    #[test]
    fn a_download_name_has_no_separator_or_control_character() {
        assert_eq!(download_file_name(r"C:\files\a.zip"), "a.zip");
        assert_eq!(download_file_name(r"https://x.io/a\b.txt"), "b.txt");
        assert_eq!(download_file_name("https://x.io/a%0Ab.txt"), "a%0Ab.txt");
        assert_eq!(download_file_name("file:///tmp/a\nb.pdf"), "ab.pdf");
        assert_eq!(download_file_name("https://x.io/.hidden"), ".hidden");
    }

    #[test]
    fn write_new_file_keeps_what_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("f.txt");

        write_new_file(&path, "mine").unwrap();
        let error = write_new_file(&path, "theirs").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains("f.txt"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
    }

    #[cfg(unix)]
    #[test]
    fn write_new_file_does_not_write_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "precious").unwrap();
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(write_new_file(&link, "overwritten").is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "precious");
    }

    #[test]
    fn encodes_data_urls() {
        let url = html_data_url("<p>a b</p>");
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        assert!(url.contains("%3Cp%3E"));
        assert!(!url.contains(' '));
    }
}
