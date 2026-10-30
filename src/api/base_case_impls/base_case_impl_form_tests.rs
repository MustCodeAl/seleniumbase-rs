// Tests of how `BaseCase` reads and clicks form fields: the text of an
// `<input>` or `<textarea>`, an attribute that is only a DOM property,
// checkboxes that sit at zero opacity, `js_click`, `set_value`. They run
// against the mock WebDriver in `webdriver_support` (no browser needed); the
// real-Chrome checks of the same behaviour are in `tests/form_fields_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_form_fields {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};

    /// The element id behind each selector of the page.
    fn id_of(selector: Option<&str>) -> Option<&'static str> {
        match selector {
            Some("#name") => Some("name"),
            Some("#notes") => Some("notes"),
            Some("#plain") => Some("plain"),
            _ => None,
        }
    }

    /// A page with three elements. `#name` is an `<input>` and `#notes` a
    /// `<textarea>`: both are displayed, hold text in their value and have no
    /// rendered text. `#plain` is a `<p>` that says "Plain text".
    fn fields() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            let id = id_of(request.body["value"].as_str());
            match (request.method.as_str(), request.path.as_str(), id) {
                ("POST", "/element", Some(id)) => Reply::Value(element(id)),
                ("POST", "/element", None) => Reply::Error(404, "no such element"),
                ("POST", "/elements", Some(id)) => Reply::Value(json!([element(id)])),
                ("POST", "/elements", None) => Reply::Value(json!([])),
                ("GET", "/element/name/name", _) => Reply::Value(json!("input")),
                ("GET", "/element/notes/name", _) => Reply::Value(json!("textarea")),
                ("GET", "/element/plain/name", _) => Reply::Value(json!("p")),
                ("GET", "/element/name/property/value", _) => {
                    Reply::Value(json!("This is Automated"))
                }
                ("GET", "/element/notes/property/value", _) => {
                    Reply::Value(json!("Testing Time!\n"))
                }
                ("GET", "/element/name/text" | "/element/notes/text", _) => {
                    Reply::Value(json!(""))
                }
                ("GET", "/element/plain/text", _) => Reply::Value(json!("Plain text")),
                ("GET", path, _) if path.ends_with("/displayed") => Reply::Value(json!(true)),
                ("GET", path, _) if path.ends_with("/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn the_text_assertions_read_the_value_of_an_input_or_a_textarea() {
        // Python's `assert_text` and `assert_exact_text` compare the value of
        // an `<input>` or `<textarea>`, whose rendered text is empty.
        let driver = fields();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.assert_text("This is Automated", "#name").await.unwrap();
        sb.assert_text("Automated", "#name").await.unwrap();
        sb.assert_exact_text("This is Automated", "#name")
            .await
            .unwrap();
        sb.assert_text("Testing Time!\n", "#notes").await.unwrap();
        sb.assert_exact_text("Testing Time!", "#notes")
            .await
            .unwrap();
        sb.assert_text("Plain", "#plain").await.unwrap();

        sb.assert_text("Plain", "#name").await.unwrap_err();
        sb.assert_exact_text("Automated", "#name")
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn the_text_checks_read_the_value_of_an_input_or_a_textarea() {
        let driver = fields();
        let sb = driver.base_case().await;

        assert!(sb.is_text_visible("Automated", "#name").await.unwrap());
        assert!(!sb.is_text_visible("Plain", "#name").await.unwrap());
        assert!(sb
            .is_exact_text_visible("This is Automated", "#name")
            .await
            .unwrap());
        sb.wait_for_text_visible("Time", "#notes", 0).await.unwrap();
        sb.wait_for_exact_text_visible("Testing Time!", "#notes", 0)
            .await
            .unwrap();
        sb.wait_for_text("Automated", "#name", 0).await.unwrap();
    }
}
