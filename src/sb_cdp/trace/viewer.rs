//! The viewer of a trace: one self-contained HTML file.
//!
//! Everything in a trace that came from a page or a caller is untrusted, and
//! the file is opened in a browser, so this module is where it is made safe:
//! the data goes in as JSON that cannot end its `<script>` element, the viewer's
//! own script is fixed and builds the page with `textContent`, the file carries
//! a Content-Security-Policy that lets only that script run and loads nothing,
//! and the one HTML string from a page (a DOM snapshot) is shown in a frame with
//! an empty `sandbox`.

use base64::Engine as _;
use ring::digest::{digest, SHA256};
use serde_json::Value;

use crate::error::SeleniumBaseError;

/// The script that draws the data. Fixed: it is the only script the page's
/// policy allows, by hash.
const VIEWER_JS: &str = include_str!("viewer.js");

/// The viewer's style sheet.
const VIEWER_CSS: &str = include_str!("viewer.css");

/// `value` as JSON that is safe inside `<script type="application/json">`.
///
/// `serde_json` writes `<`, `>` and `&` as they are, so a console line holding
/// `</script><script>` would end the element and start another. Outside a string
/// JSON has none of these characters, so writing each as a `\u` escape changes
/// nothing for `JSON.parse` and nothing for the HTML parser to find. U+2028 and
/// U+2029 are escaped too, for the sake of any reader that treats them as line
/// ends.
///
/// # Errors
///
/// [`SeleniumBaseError::Json`] if the value cannot be serialised.
pub(super) fn script_json(value: &Value) -> Result<String, SeleniumBaseError> {
    let json = serde_json::to_string(value)?;
    let mut safe = String::with_capacity(json.len());
    for ch in json.chars() {
        match ch {
            '<' => safe.push_str("\\u003c"),
            '>' => safe.push_str("\\u003e"),
            '&' => safe.push_str("\\u0026"),
            '\u{2028}' => safe.push_str("\\u2028"),
            '\u{2029}' => safe.push_str("\\u2029"),
            other => safe.push(other),
        }
    }
    Ok(safe)
}

/// The Content-Security-Policy of the file: nothing loads (`default-src
/// 'none'`), the one script is allowed by its hash, inline styles are allowed
/// (the viewer's and a snapshot's), and images are allowed only as `data:` URIs
/// (the screenshots).
fn policy() -> String {
    let hash = base64::engine::general_purpose::STANDARD
        .encode(digest(&SHA256, VIEWER_JS.as_bytes()).as_ref());
    format!(
        "default-src 'none'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; img-src data:"
    )
}

/// The viewer for `data` (see `Trace` for its shape) as the text of an HTML file.
///
/// # Errors
///
/// [`SeleniumBaseError::Json`] if the data cannot be serialised.
pub(super) fn render(data: &Value) -> Result<String, SeleniumBaseError> {
    let policy = policy();
    let data = script_json(data)?;
    Ok(format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Trace</title>\n\
         <style>{VIEWER_CSS}</style>\n\
         </head>\n\
         <body>\n\
         <div id=\"app\"></div>\n\
         <script type=\"application/json\" id=\"trace-data\">{data}</script>\n\
         <script>{VIEWER_JS}</script>\n\
         </body>\n\
         </html>\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Data with the strings a hostile page or caller would use.
    fn hostile() -> Value {
        json!({
            "version": 1,
            "started_at": "</script><script>alert('start')</script>",
            "duration_ms": 10,
            "actions": [{
                "id": 1,
                "name": "click",
                "selector": "#a\"></script><img src=x onerror=alert(1)><script>alert('selector')</script>",
                "detail": "<!-- not a comment",
                "start_ms": 0,
                "end_ms": 5,
                "ok": false,
                "error": "</SCRIPT ><script>alert('error')</script>",
                "url": "https://evil.test/?q=</script><script>alert(1)</script>",
                "title": "</title><script>alert('title')</script>",
                "screenshot": null,
                "snapshot": "<!doctype html><script>alert('snapshot')</script><body onload=alert(1)>",
            }],
            "console": [
                { "at_ms": 1, "level": "error", "text": "</script><script>alert('console')</script>", "action": 1 },
                { "at_ms": 2, "level": "log", "text": "line\u{2028}separator \u{2029} & <b>bold</b>" },
            ],
            "network": [],
            "limits": {},
        })
    }

    /// How often `needle` occurs in `text`, ignoring ASCII case.
    fn count(text: &str, needle: &str) -> usize {
        text.to_ascii_lowercase().matches(needle).count()
    }

    #[test]
    fn json_for_a_script_element_has_no_markup_characters() {
        let json = script_json(&hostile()).unwrap();

        for forbidden in ['<', '>', '&', '\u{2028}', '\u{2029}'] {
            assert!(!json.contains(forbidden), "{forbidden:?} in {json}");
        }
        assert!(json.contains("\\u003c/script\\u003e"));
    }

    #[test]
    fn the_escaped_json_reads_back_as_the_same_value() {
        let data = hostile();

        let json = script_json(&data).unwrap();

        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), data);
    }

    #[test]
    fn a_hostile_console_line_and_selector_cannot_open_a_script() {
        let html = render(&hostile()).unwrap();

        // The data element and the viewer script, and no other.
        assert_eq!(count(&html, "<script"), 2, "{html}");
        assert_eq!(count(&html, "</script"), 2, "{html}");
        assert_eq!(count(&html, "<style"), 1);
        assert_eq!(count(&html, "<img"), 0);
        assert_eq!(count(&html, "<iframe"), 0);
        assert_eq!(count(&html, "<!--"), 0);
        assert!(!html.contains("<b>"), "markup in page data was not escaped");
    }

    #[test]
    fn the_data_element_comes_before_the_viewer_script_and_holds_the_data() {
        let html = render(&hostile()).unwrap();

        let data_at = html.find("id=\"trace-data\"").unwrap();
        let script_at = html.rfind("<script>").unwrap();
        assert!(data_at < script_at);
        let start = html[data_at..].find('>').unwrap() + data_at + 1;
        let end = html[start..].find("</script>").unwrap() + start;
        assert_eq!(
            serde_json::from_str::<Value>(&html[start..end]).unwrap(),
            hostile()
        );
    }

    #[test]
    fn the_policy_names_the_hash_of_the_script_in_the_file() {
        let html = render(&json!({})).unwrap();

        let open = html.rfind("<script>").unwrap() + "<script>".len();
        let close = html.rfind("</script>").unwrap();
        let hash = base64::engine::general_purpose::STANDARD
            .encode(digest(&SHA256, html[open..close].as_bytes()).as_ref());
        assert!(
            html.contains(&format!("script-src 'sha256-{hash}';")),
            "no policy for the hash {hash}"
        );
    }

    #[test]
    fn the_policy_loads_nothing_and_runs_no_other_script() {
        let policy = policy();

        assert!(policy.starts_with("default-src 'none';"), "{policy}");
        assert!(policy.contains("script-src 'sha256-"));
        for allowed in ["unsafe-eval", "connect-src", "frame-src", "https:", "http:", "*"] {
            assert!(!policy.contains(allowed), "{allowed} in {policy}");
        }
        assert!(
            !policy.contains("script-src 'unsafe-inline'"),
            "inline scripts must be allowed by hash only"
        );
    }

    #[test]
    fn the_file_refers_to_nothing_outside_itself() {
        let html = render(&json!({ "actions": [] })).unwrap();

        for external in [
            "http://", "https://", "//cdn", "<link", "@import", "url(", "src=\"", "href=",
        ] {
            assert!(!html.contains(external), "{external:?} in the viewer");
        }
    }

    #[test]
    fn the_viewer_script_cannot_end_its_element_or_start_a_comment() {
        let lowered = VIEWER_JS.to_ascii_lowercase();

        assert!(!lowered.contains("</script"));
        assert!(!lowered.contains("<!--"));
        assert!(!VIEWER_CSS.to_ascii_lowercase().contains("</style"));
    }

    /// The viewer's code, without its comment lines.
    fn viewer_code() -> String {
        VIEWER_JS
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_viewer_script_builds_the_page_without_markup_from_data() {
        let code = viewer_code();

        for forbidden in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "new Function",
            "setTimeout(\"",
            "setTimeout('",
            "createContextualFragment",
            "DOMParser",
            "location",
            "window.open",
            "fetch(",
            "XMLHttpRequest",
            "import(",
        ] {
            assert!(!code.contains(forbidden), "{forbidden} in the viewer script");
        }
    }

    #[test]
    fn a_snapshot_goes_only_into_a_frame_with_an_empty_sandbox() {
        let code = viewer_code();

        // One frame is made, one document is given to it, and the sandbox is set
        // (empty) before that. No sandbox token is ever added.
        assert_eq!(code.matches("srcdoc").count(), 1);
        assert_eq!(code.matches("el('iframe'").count(), 1);
        let sandbox = code.find("setAttribute('sandbox', '')").unwrap();
        assert!(sandbox < code.find("srcdoc").unwrap());
        assert!(!code.contains("allow-"));
        assert!(!code.contains("sandbox = "));
        assert!(!code.contains("sandbox.add"));
        // The frame is given the policy again, so it loads nothing.
        assert!(code.contains("default-src 'none'"));
    }
}
