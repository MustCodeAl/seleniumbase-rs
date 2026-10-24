//! The listings of `sbrs methods`: the common methods of the test API.
//!
//! Python SeleniumBase's `sbase methods` prints the methods most tests use. This is the same
//! list in Rust spelling, so a person coming from Python sees the call to write on a
//! [`BaseCase`](seleniumbase_rs::BaseCase). The other two listings of that family live
//! beside the options they describe: `sbrs options` in `browser_flags`, `sbrs behave-options`
//! in the library's `cli::scripts::behave_options`.

/// The common methods: how each is called on `sb`, a `BaseCase`, and what it does.
///
/// A test checks every name against `parity/api.toml`, so a method that is renamed or removed
/// cannot stay in this list.
pub(crate) const METHODS: &[(&str, &str)] = &[
    ("sb.open(url)", "Open a URL in the browser."),
    (
        "sb.type_text(selector, text)",
        "Clear a field and type text into it.",
    ),
    ("sb.click(selector)", "Click an element."),
    ("sb.click_link(link_text)", "Click a link by its text."),
    ("sb.go_back()", "Go back one page in the history."),
    (
        "sb.select_option_by_text(selector, text)",
        "Choose an option of a dropdown by its text.",
    ),
    (
        "sb.hover_and_click(hover_selector, click_selector)",
        "Hover over one element, then click another.",
    ),
    (
        "sb.drag_and_drop(drag_selector, drop_selector)",
        "Drag one element onto another.",
    ),
    ("sb.get_text(selector)", "The text of an element."),
    (
        "sb.get_attribute(selector, attribute)",
        "The value of an attribute of an element.",
    ),
    ("sb.get_current_url()", "The URL of the page."),
    ("sb.get_page_source()", "The HTML of the page."),
    ("sb.get_title()", "The title of the page."),
    (
        "sb.assert_element(selector)",
        "Assert that an element is visible.",
    ),
    (
        "sb.assert_text(text, selector)",
        "Assert that an element contains the text.",
    ),
    (
        "sb.assert_exact_text(text, selector)",
        "Assert that an element has exactly this text.",
    ),
    (
        "sb.assert_title(title)",
        "Assert that the page title is the one given.",
    ),
    (
        "sb.assert_true(expression)",
        "Assert that a condition is true.",
    ),
    (
        "sb.assert_false(expression)",
        "Assert that a condition is false.",
    ),
    (
        "sb.assert_equal(first, second)",
        "Assert that two values are equal.",
    ),
    (
        "sb.assert_not_equal(first, second)",
        "Assert that two values are different.",
    ),
    (
        "sb.assert_in(first, second)",
        "Assert that the first is found in the second.",
    ),
    (
        "sb.assert_not_in(first, second)",
        "Assert that the first is not found in the second.",
    ),
    (
        "sb.assert_no_js_errors()",
        "Assert that the page logged no JavaScript errors.",
    ),
    (
        "sb.is_element_present(selector)",
        "Whether an element is in the page.",
    ),
    (
        "sb.is_element_visible(selector)",
        "Whether an element is in the page and visible.",
    ),
    (
        "sb.is_text_visible(text, selector)",
        "Whether an element shows the text.",
    ),
    (
        "sb.is_link_text_visible(link_text)",
        "Whether a link with this text is visible.",
    ),
    (
        "sb.wait_for_element(selector)",
        "Wait until an element is visible, then return it.",
    ),
    (
        "sb.wait_for_text(text, selector)",
        "Wait until an element shows the text.",
    ),
    (
        "sb.wait_for_element_not_visible(selector)",
        "Wait until an element is no longer visible.",
    ),
    (
        "sb.wait_for_text_not_visible(text, selector)",
        "Wait until an element no longer shows the text.",
    ),
    (
        "sb.wait_for_and_accept_alert()",
        "Wait for an alert, then accept it.",
    ),
    (
        "sb.wait_for_and_dismiss_alert()",
        "Wait for an alert, then dismiss it.",
    ),
    (
        "sb.wait_for_ready_state_complete()",
        "Wait until the page has finished loading.",
    ),
    ("sb.sleep(seconds)", "Pause the test for some seconds."),
    ("sb.save_screenshot(name)", "Save a screenshot of the page."),
    ("sb.save_page_source(name)", "Save the HTML of the page."),
    ("sb.switch_to_frame(frame)", "Switch into an iframe."),
    (
        "sb.switch_to_default_content()",
        "Switch back to the page from an iframe.",
    ),
    (
        "sb.switch_to_window(window)",
        "Switch to another window or tab.",
    ),
    ("sb.execute_script(script)", "Run JavaScript in the page."),
    (
        "sb.set_window_size(width, height)",
        "Resize the browser window.",
    ),
];

/// What `sbrs methods` prints: a short introduction and the table `CALL  WHAT`.
pub(crate) fn methods_listing() -> String {
    let width = METHODS
        .iter()
        .map(|(call, _)| call.len())
        .max()
        .unwrap_or(0);
    let mut text = String::from(
        "The common methods of the test API, as they are called on `sb`, a BaseCase.\n\
         Each one is async: write `.await` after the call, as in `sb.open(url).await?`.\n\
         Selectors are CSS. Every method: `cargo doc --open`, and the map from Python's\n\
         names in docs/parity.md.\n\n",
    );
    for (call, what) in METHODS {
        text.push_str(&format!("  {call:<width$}  {what}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest of the Python API, read at compile time from the repository.
    const PARITY: &str = include_str!("../../../parity/api.toml");

    /// The `BaseCase` items the manifest says exist, whether implemented or composed.
    fn basecase_items() -> Vec<String> {
        let mut items = Vec::new();
        let mut in_basecase = false;
        let mut available = false;
        let mut rust: Option<String> = None;
        let mut settle = |in_basecase: bool, available: bool, rust: &mut Option<String>| {
            if let Some(item) = rust.take() {
                if in_basecase && available {
                    items.push(item);
                }
            }
        };
        for line in PARITY.lines() {
            if line.starts_with('[') {
                settle(in_basecase, available, &mut rust);
                in_basecase = line.starts_with("[basecase.");
                available = false;
            } else if let Some(status) = line.strip_prefix("status = ") {
                available = matches!(status.trim(), "\"implemented\"" | "\"composed\"");
            } else if let Some(item) = line.strip_prefix("rust = ") {
                rust = Some(item.trim().trim_matches('"').to_owned());
            }
        }
        settle(in_basecase, available, &mut rust);
        items
    }

    /// The method name of a call such as `sb.open(url)`.
    fn method_of(call: &str) -> &str {
        call.strip_prefix("sb.")
            .and_then(|rest| rest.split('(').next())
            .unwrap_or_else(|| panic!("`{call}` is not written as sb.method(...)"))
    }

    #[test]
    fn every_listed_method_is_a_basecase_method_of_the_manifest() {
        let items = basecase_items();
        assert!(
            items.len() > 100,
            "the manifest was not read: {}",
            items.len()
        );
        for (call, _) in METHODS {
            let item = format!("BaseCase::{}", method_of(call));
            assert!(
                items.contains(&item),
                "`{call}` is not an implemented or composed {item} in parity/api.toml"
            );
        }
    }

    #[test]
    fn the_calls_are_well_formed_and_listed_once() {
        let mut seen = std::collections::BTreeSet::new();
        for (call, what) in METHODS {
            assert!(call.ends_with(')'), "`{call}` has no closing parenthesis");
            assert!(what.ends_with('.'), "`{what}` is not a sentence");
            assert!(seen.insert(method_of(call)), "`{call}` is listed twice");
        }
    }

    #[test]
    fn the_listing_has_a_row_for_every_method_and_aligned_columns() {
        let listing = methods_listing();
        let rows: Vec<&str> = listing.lines().filter(|l| l.starts_with("  sb.")).collect();
        assert_eq!(rows.len(), METHODS.len());
        let columns: Vec<usize> = rows
            .iter()
            .map(|row| row[2..].find("  ").map(|at| at + 2).unwrap())
            .collect();
        let widest = METHODS.iter().map(|(call, _)| call.len()).max().unwrap();
        // The text begins in the same column on every row: after the widest call.
        for (row, (call, what)) in rows.iter().zip(METHODS) {
            assert!(row.starts_with(&format!("  {call}")), "{row}");
            assert!(row.ends_with(what), "{row}");
        }
        assert!(columns.iter().all(|&at| at > 2));
        assert!(rows.iter().all(|row| row.len() >= 2 + widest + 2));
    }
}
