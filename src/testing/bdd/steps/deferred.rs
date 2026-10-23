//! Deferred assertions: they record a failure and go on, and `Process
//! deferred asserts` fails the scenario with all of them.

use super::normalize_text;

sb_steps! {
    /// Defers the assertion that an element is visible.
    fn deferred_assert_element(sb, selector: String) {
        sb.deferred_assert_element(&selector).await
    }
        "Deferred assert element {selector:string}";

    /// Defers the assertion that an element is in the page.
    fn deferred_assert_element_present(sb, selector: String) {
        sb.deferred_assert_element_present(&selector).await
    }
        "Deferred assert element present {selector:string}";

    /// Defers the assertion that an element's text contains the text.
    fn deferred_assert_text_in_element(sb, text: String, selector: String) {
        sb.deferred_assert_text(&normalize_text(&text), &selector).await
    }
        "Deferred assert text {text:string} in {selector:string}";

    /// Defers the assertion that the page shows the text.
    fn deferred_assert_text(sb, text: String) {
        sb.deferred_assert_text(&normalize_text(&text), "html").await
    }
        "Deferred assert text {text:string}";

    /// Defers the assertion that an element's text is exactly the text.
    fn deferred_assert_exact_text(sb, text: String, selector: String) {
        sb.deferred_assert_exact_text(&normalize_text(&text), &selector).await
    }
        "Deferred assert exact text {text:string} in {selector:string}";

    /// Defers the assertion that an element has some text.
    fn deferred_assert_non_empty_text(sb, selector: String) {
        sb.deferred_assert_non_empty_text(&selector).await
    }
        "Deferred assert non-empty text in {selector:string}";

    /// Fails with every deferred assertion that failed.
    fn process_deferred_asserts(sb) { sb.process_deferred_asserts().await }
        "Process deferred asserts";
}
