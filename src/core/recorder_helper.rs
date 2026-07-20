use regex::Regex;

/// Sanitize a selector string so it can be embedded in generated Rust code.
pub fn sanitize_selector(sel: &str) -> String {
    sel.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Convert a recorded action into a SeleniumBase Rust API call snippet.
pub fn action_to_rust_snippet(action: &str, selector: &str, value: &str) -> String {
    let sel = sanitize_selector(selector);
    let val = sanitize_selector(value);
    match action {
        "open" => format!(r#"sb.open("{}");"#, sel),
        "click" => format!(r#"sb.click("{}");"#, sel),
        "type" => format!(r#"sb.type("{}", "{}");"#, sel, val),
        "clear" => format!(r#"sb.clear("{}");"#, sel),
        "assert_text" => format!(r#"sb.assert_text("{}", "{}");"#, val, sel),
        "wait_for_element" => format!(r#"sb.wait_for_element("{}");"#, sel),
        _ => format!(r#"// unsupported action: {} on {}"#, action, sel),
    }
}

/// Extract the tag name from a simple CSS selector.
pub fn selector_tag_name(selector: &str) -> Option<String> {
    let re = Regex::new(r"^[a-zA-Z]+").ok()?;
    re.find(selector).map(|m| m.as_str().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_to_rust_snippet() {
        assert_eq!(
            action_to_rust_snippet("click", "#btn", ""),
            r##"sb.click("#btn");"##
        );
        assert_eq!(
            action_to_rust_snippet("type", "#q", "hello"),
            r##"sb.type("#q", "hello");"##
        );
    }

    #[test]
    fn test_selector_tag_name() {
        assert_eq!(selector_tag_name("input#q"), Some("input".into()));
    }
}
