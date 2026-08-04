//! Shadow DOM selector piercing helpers.

/// Splits a selector on the `::shadow` piercing combinator.
///
/// Example: `"my-app ::shadow .form ::shadow input"`
/// returns `["my-app", ".form", "input"]`.
pub fn split_shadow_selector(selector: &str) -> Vec<String> {
    selector
        .split("::shadow")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Builds a JavaScript expression that walks through shadow roots using the
/// provided CSS fragments and returns the final element (or `null`).
pub fn build_shadow_query(fragments: &[String]) -> String {
    let mut script = "return (function(){\n".to_owned();
    script.push_str("  let el = document.querySelector(");
    script.push_str(&serde_json::json!(fragments[0]).to_string());
    script.push_str(");\n");
    for fragment in &fragments[1..] {
        script.push_str("  if (!el) return null;\n");
        script.push_str("  el = el.shadowRoot ? el.shadowRoot.querySelector(");
        script.push_str(&serde_json::json!(fragment).to_string());
        script.push_str(") : null;\n");
    }
    script.push_str("  return el;\n})();");
    script
}

/// Builds a JS expression that clicks the pierced element.
pub fn build_shadow_click(fragments: &[String]) -> String {
    let mut script = build_shadow_query(fragments);
    script.truncate(script.rfind("return el;").unwrap_or(script.len()));
    script.push_str("if (el) { el.click(); return true; } return false;");
    script
}

/// Builds a JS expression that types text into the pierced element.
pub fn build_shadow_type(fragments: &[String], text: &str) -> String {
    let mut script = "return (function(){\n".to_owned();
    script.push_str("  let el = document.querySelector(");
    script.push_str(&serde_json::json!(fragments[0]).to_string());
    script.push_str(");\n");
    for fragment in &fragments[1..] {
        script.push_str("  if (!el || !el.shadowRoot) return false;\n");
        script.push_str("  el = el.shadowRoot.querySelector(");
        script.push_str(&serde_json::json!(fragment).to_string());
        script.push_str(");\n");
    }
    script.push_str("  if (!el) return false;\n");
    script.push_str(&format!("  el.value = {};\n", serde_json::json!(text)));
    script.push_str("  el.dispatchEvent(new Event('input', {bubbles: true}));\n");
    script.push_str("  el.dispatchEvent(new Event('change', {bubbles: true}));\n");
    script.push_str("  return true;\n})();");
    script
}

/// Builds a JS expression that extracts text from the pierced element.
pub fn build_shadow_text(fragments: &[String]) -> String {
    let mut script = build_shadow_query(fragments);
    script.truncate(script.rfind("return el;").unwrap_or(script.len()));
    script.push_str("return el ? (el.textContent || el.value || '') : '';");
    script
}

/// Builds a JS expression that extracts an attribute from the pierced element.
pub fn build_shadow_attribute(fragments: &[String], attribute: &str) -> String {
    let mut script = build_shadow_query(fragments);
    script.truncate(script.rfind("return el;").unwrap_or(script.len()));
    script.push_str(&format!(
        "return el ? (el.getAttribute({}) || '') : '';",
        serde_json::json!(attribute)
    ));
    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic_shadow_selector() {
        assert_eq!(
            split_shadow_selector("my-app ::shadow .form ::shadow input"),
            vec!["my-app", ".form", "input"]
        );
    }

    #[test]
    fn build_query_chains_shadow_roots() {
        let fragments = vec!["my-app".to_owned(), ".form".to_owned(), "input".to_owned()];
        let script = build_shadow_query(&fragments);
        assert!(script.contains("document.querySelector(\"my-app\")"));
        assert!(script.contains("el.shadowRoot.querySelector(\".form\")"));
        assert!(script.contains("el.shadowRoot.querySelector(\"input\")"));
    }

    #[test]
    fn build_click_returns_boolean() {
        let fragments = vec!["host".to_owned(), "button".to_owned()];
        let script = build_shadow_click(&fragments);
        assert!(script.contains("el.click()"));
        assert!(script.contains("return true"));
    }
}
