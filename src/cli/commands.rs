//! Reusable, browser-independent helpers backing the `sbase` CLI subcommands.
//!
//! The logic here is deliberately free of any WebDriver dependency so it can be
//! unit tested without a live browser: deferred assertion parsing, download file
//! name resolution, and the synthetic upload page used by `sbase choose-file`.

use crate::error::SeleniumBaseError;

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
    /// ```
    /// use seleniumbase_rs::cli::commands::DeferredSpec;
    ///
    /// let spec = DeferredSpec::parse("text=#msg:Welcome").unwrap();
    /// assert_eq!(spec, DeferredSpec::Text("#msg".into(), "Welcome".into()));
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
                let (selector, expected) = rest.split_once(':').ok_or_else(|| {
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
/// Accepts a bare array of strings or an object with an `asserts` key:
///
/// ```
/// use seleniumbase_rs::cli::commands::parse_deferred_spec_json;
///
/// let specs = parse_deferred_spec_json(r#"["element=#a","title=Hi"]"#).unwrap();
/// assert_eq!(specs.len(), 2);
///
/// let specs = parse_deferred_spec_json(r#"{"asserts":["url=/demo"]}"#).unwrap();
/// assert_eq!(specs.len(), 1);
/// ```
pub fn parse_deferred_spec_json(raw: &str) -> Result<Vec<DeferredSpec>, SeleniumBaseError> {
    #[derive(serde::Deserialize)]
    struct SpecFile {
        #[serde(default)]
        asserts: Vec<String>,
    }

    let entries = if let Ok(list) = serde_json::from_str::<Vec<String>>(raw) {
        list
    } else {
        let file: SpecFile = serde_json::from_str(raw).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse deferred spec file: {e}"))
        })?;
        file.asserts
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
/// Falls back to `download.bin` when the URL has no usable final path segment.
///
/// ```
/// use seleniumbase_rs::cli::commands::download_file_name;
///
/// assert_eq!(download_file_name("https://x.io/a/report.pdf?v=2"), "report.pdf");
/// assert_eq!(download_file_name("https://x.io/"), "download.bin");
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
    let candidate = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let sanitized: String = candidate
        .chars()
        .filter(|c| !matches!(c, '\\' | ':' | '*' | '"' | '<' | '>' | '|'))
        .collect();
    if sanitized.is_empty() {
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

/// Builds a blank data-URL page containing a file input that matches `selector`.
///
/// Simple `#id`, `.class`, `[name=...]` and bare tag selectors are supported;
/// anything else falls back to a generic `input[type=file]`.
///
/// ```
/// use seleniumbase_rs::cli::commands::blank_file_input_page;
///
/// assert!(blank_file_input_page("#upload").contains("id=\"upload\""));
/// assert!(blank_file_input_page(".picker").contains("class=\"picker\""));
/// ```
pub fn blank_file_input_page(selector: &str) -> String {
    let selector = selector.trim();
    let mut input_id = "sbase-file-input".to_owned();
    let mut attrs = String::new();
    if let Some(id) = selector.strip_prefix('#') {
        input_id = html_attr_escape(id);
    } else if let Some(class) = selector.strip_prefix('.') {
        attrs.push_str(&format!(" class=\"{}\"", html_attr_escape(class)));
    } else if let Some(name) = selector
        .strip_prefix("input[name=")
        .or_else(|| selector.strip_prefix("[name="))
    {
        let name = name.trim_end_matches(']').trim_matches(['"', '\'']);
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
        assert!(blank_file_input_page("#a\"b").contains("&quot;"));
    }

    #[test]
    fn encodes_data_urls() {
        let url = html_data_url("<p>a b</p>");
        assert!(url.starts_with("data:text/html;charset=utf-8,"));
        assert!(url.contains("%3Cp%3E"));
        assert!(!url.contains(' '));
    }
}
