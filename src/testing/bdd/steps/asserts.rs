//! Steps that check the page, and the waits that precede a check.
//!
//! A step without a selector looks at the whole page (`html`), as in Python.
//! A wording with its arguments the other way round has a second function,
//! named `as` the action (see `elements`).

use super::{normalize_text, WAIT_SECONDS};
use crate::error::SeleniumBaseError;

sb_steps! {
    /// Asserts that an element is visible.
    fn assert_element(sb, selector: String) { sb.assert_element(&selector).await }
        "Assert element {selector:string}", "Assert element {selector:string} is visible",
        "Element {selector:string} should be visible";

    /// Asserts that an element's text contains the text.
    fn assert_text_in_element(sb, text: String, selector: String) {
        sb.assert_text(&normalize_text(&text), &selector).await
    }
        "Assert text {text:string} in {selector:string}";

    /// `assert_text_in_element` with the selector first.
    fn assert_text_in_element_selector_first as assert_text_in_element(
        sb, selector: String, text: String
    ) {
        assert_text_in_element(sb, text, selector).await
    }
        "Text in {selector:string} should contain {text:string}";

    /// Asserts that the page shows the text.
    fn assert_text(sb, text: String) { sb.assert_text(&normalize_text(&text), "html").await }
        "Assert text {text:string}", "Assert text {text:string} is visible",
        "Text {text:string} should be visible";

    /// Asserts that an element's text is exactly the text.
    fn assert_exact_text(sb, text: String, selector: String) {
        sb.assert_exact_text(&normalize_text(&text), &selector).await
    }
        "Assert exact text {text:string} in {selector:string}";

    /// `assert_exact_text` with the selector first.
    fn assert_exact_text_selector_first as assert_exact_text(
        sb, selector: String, text: String
    ) {
        assert_exact_text(sb, text, selector).await
    }
        "Text in {selector:string} should be {text:string}";

    /// Asserts that an element has some text.
    fn assert_non_empty_text(sb, selector: String) { sb.assert_non_empty_text(&selector).await }
        "Assert non-empty text in {selector:string}", "Assert text in {selector:string} is not empty",
        "Text in {selector:string} should be non-empty",
        "Text in {selector:string} should not be empty";

    /// Waits for an element to be visible.
    fn wait_for_element(sb, selector: String) {
        sb.wait_for_element(&selector).timeout(std::time::Duration::from_secs(WAIT_SECONDS)).await
    }
        "Wait for {selector:string} to be visible", "Wait for element {selector:string}",
        "User waits for {selector:string} to be visible", "User waits for element {selector:string}";

    /// Waits for an element's text to contain the text.
    fn wait_for_text_in_element(sb, text: String, selector: String) {
        sb.wait_for_text(&normalize_text(&text), &selector, WAIT_SECONDS).await
    }
        "Wait for text {text:string} in {selector:string}",
        "User waits for text {text:string} in {selector:string}";

    /// `wait_for_text_in_element` with the selector first.
    fn wait_for_text_in_element_selector_first as wait_for_text_in_element(
        sb, selector: String, text: String
    ) {
        wait_for_text_in_element(sb, text, selector).await
    }
        "Wait for {selector:string} to have text {text:string}",
        "User waits for {selector:string} to have text {text:string}";

    /// Waits for an element's text to be exactly the text.
    fn wait_for_exact_text_in_element(sb, text: String, selector: String) {
        sb.wait_for_exact_text(&normalize_text(&text), &selector, WAIT_SECONDS).await
    }
        "Wait for exact text {text:string} in {selector:string}",
        "User waits for exact text {text:string} in {selector:string}";

    /// `wait_for_exact_text_in_element` with the selector first.
    fn wait_for_exact_text_in_element_selector_first as wait_for_exact_text_in_element(
        sb, selector: String, text: String
    ) {
        wait_for_exact_text_in_element(sb, text, selector).await
    }
        "Wait for {selector:string} to have exact text {text:string}",
        "User waits for {selector:string} to have exact text {text:string}";

    /// Waits for an element to have some text.
    fn wait_for_non_empty_text_in_element(sb, selector: String) {
        sb.wait_for_non_empty_text(&selector, WAIT_SECONDS).await
    }
        "Wait for non-empty text in {selector:string}",
        "Wait for {selector:string} to have non-empty text",
        "User waits for non-empty text in {selector:string}",
        "User waits for {selector:string} to have non-empty text",
        "Wait for {selector:string} to not have text",
        "Wait for text in {selector:string} to not be empty",
        "User waits for {selector:string} to not have text",
        "User waits for text in {selector:string} to not be empty";

    /// Waits for the page to show the text.
    fn wait_for_text(sb, text: String) {
        sb.wait_for_text(&normalize_text(&text), "html", WAIT_SECONDS).await
    }
        "Wait for text {text:string}", "User waits for text {text:string}";

    /// Asserts that an element is in the page, visible or not.
    fn assert_element_present(sb, selector: String) {
        sb.assert_element_present(&selector).await
    }
        "Assert element present {selector:string}", "Element {selector:string} should be present";

    /// Asserts that an element is not visible.
    fn assert_element_not_visible(sb, selector: String) {
        sb.assert_element_not_visible(&selector).await
    }
        "Assert element not visible {selector:string}",
        "Element {selector:string} should not be visible";

    /// Asserts that the page has a link with this text.
    fn assert_link_text(sb, text: String) { sb.assert_link_text(&normalize_text(&text)).await }
        "Assert link text {text:string}", "Link text {text:string} should be visible";

    /// Asserts the title of the page.
    fn assert_title(sb, title: String) { sb.assert_title(&normalize_text(&title)).await }
        "Assert title {title:string}", "The title should be {title:string}";

    /// Asserts that a file is in the downloads folder.
    fn assert_downloaded_file(sb, file: String) { sb.assert_downloaded_file(&file).await }
        "Assert downloaded file {file:string}", "File {file:string} should be in downloads";

    /// Asserts that an element has an attribute.
    fn assert_attribute(sb, selector: String, attribute: String) {
        match sb.get_attribute(&selector, &attribute).await? {
            Some(_) => Ok(()),
            None => Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected attribute '{attribute}' of '{selector}' to be there"
            ))),
        }
    }
        "In {selector:string} assert attribute {attribute:string}";

    /// Asserts that an element's attribute has the value.
    fn assert_attribute_has_value(sb, selector: String, attribute: String, value: String) {
        sb.assert_attribute(&selector, &attribute, &normalize_text(&value)).await
    }
        "In {selector:string} assert attribute/value {attribute:string}/{value:string}";

    /// Asserts that the text is not visible in an element.
    fn assert_text_not_visible_in_element(sb, text: String, selector: String) {
        sb.assert_text_not_visible(&normalize_text(&text), &selector).await
    }
        "Assert text not visible {text:string} in {selector:string}",
        "Text {text:string} should not be visible in {selector:string}";

    /// Asserts that the text is not visible on the page.
    fn assert_text_not_visible(sb, text: String) {
        sb.assert_text_not_visible(&normalize_text(&text), "html").await
    }
        "Assert text not visible {text:string}", "Text {text:string} should not be visible";

    /// Asserts that no element's text is exactly this, in an element.
    fn assert_exact_text_not_visible_in_element(sb, text: String, selector: String) {
        sb.assert_exact_text_not_visible(&normalize_text(&text), &selector).await
    }
        "Assert exact text not visible {text:string} in {selector:string}",
        "Exact text {text:string} should not be visible in {selector:string}";

    /// Asserts that the exact text is not visible on the page.
    fn assert_exact_text_not_visible(sb, text: String) {
        sb.assert_exact_text_not_visible(&normalize_text(&text), "html").await
    }
        "Assert exact text not visible {text:string}",
        "Exact text {text:string} should not be visible";

    /// Asserts that the title contains the text.
    fn assert_title_contains(sb, substring: String) {
        sb.assert_title_contains(&normalize_text(&substring)).await
    }
        "Assert title contains {substring:string}", "The title should contain {substring:string}";

    /// Asserts the URL of the page.
    fn assert_url(sb, url: String) { sb.assert_url(&normalize_text(&url)).await }
        "Assert URL {url:string}", "The URL should be {url:string}";

    /// Asserts that the URL contains the text.
    fn assert_url_contains(sb, substring: String) {
        sb.assert_url_contains(&normalize_text(&substring)).await
    }
        "Assert URL contains {substring:string}", "The URL should contain {substring:string}";
}
