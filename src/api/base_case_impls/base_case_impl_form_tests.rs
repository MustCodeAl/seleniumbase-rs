// Tests of how `BaseCase` reads and clicks form fields: the text of an
// `<input>` or `<textarea>`, an attribute that is only a DOM property,
// checkboxes that sit at zero opacity, `js_click`, `set_value`. They run
// against the mock WebDriver in `webdriver_support` (no browser needed); the
// real-Chrome checks of the same behaviour are in `tests/form_fields_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_form_attributes {
    use super::SeleniumBaseError;

    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};

    /// A page with a `<select id=pick>`: it has an `id` attribute and no
    /// `value` attribute, but the DOM property `value` says "v2". The page
    /// script that reads a property answers "v2".
    fn select_page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            let known = request.body["value"].as_str() == Some("#pick");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if known => Reply::Value(element("pick")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if known => Reply::Value(json!([element("pick")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/pick/attribute/id") => Reply::Value(json!("pick")),
                ("POST", "/execute/sync") => Reply::Value(json!("v2")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The number in an option's element id (`o2` is 2).
    fn option_number(id: &str) -> usize {
        id.trim_start_matches('o').parse().unwrap_or_default()
    }

    /// A `<select id=pick>` of three options, "One", "Two" and "Three", with the
    /// values v1, v2 and v3. Their `index` is a property only: the page script
    /// that reads it answers for the option it is given.
    fn options_page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            let known = request.body["value"].as_str() == Some("#pick");
            let element_id = request.path.split('/').nth(2).unwrap_or_default();
            let option = option_number(element_id);
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if known => Reply::Value(element("pick")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if known => Reply::Value(json!([element("pick")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/pick/name") => Reply::Value(json!("select")),
                ("POST", "/element/pick/elements") => {
                    Reply::Value(json!([element("o1"), element("o2"), element("o3")]))
                }
                ("GET", path) if path.ends_with("/text") => {
                    Reply::Value(json!(["", "One", "Two", "Three"][option]))
                }
                ("GET", path) if path.ends_with("/attribute/value") => {
                    Reply::Value(json!(format!("v{option}")))
                }
                ("POST", "/execute/sync") => {
                    let option = request.body["args"][0]
                        .as_object()
                        .and_then(|reference| reference.values().next())
                        .and_then(Value::as_str)
                        .map_or(0, option_number);
                    Reply::Value(json!((option - 1).to_string()))
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn get_select_options_gives_the_text_the_value_or_the_index_of_each_option() {
        let driver = options_page();
        let mut sb = driver.base_case().await;

        let texts = sb.get_select_options("#pick").await.unwrap();
        assert_eq!(texts, ["One", "Two", "Three"]);
        let values = sb
            .get_select_options("#pick")
            .attribute("value")
            .await
            .unwrap();
        assert_eq!(values, ["v1", "v2", "v3"]);
        let indexes = sb
            .get_select_options("#pick")
            .attribute("index")
            .await
            .unwrap();
        assert_eq!(indexes, ["0", "1", "2"]);
    }

    #[tokio::test]
    async fn get_select_options_refuses_an_attribute_python_does_not_take() {
        let driver = options_page();
        let mut sb = driver.base_case().await;

        let error = sb
            .get_select_options("#pick")
            .attribute("title")
            .await
            .unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
    }


    /// The property-reading scripts the driver was asked to run.
    fn property_reads(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/execute/sync")
            .into_iter()
            .filter(|request| {
                request.body["script"]
                    .as_str()
                    .is_some_and(|script| script.contains("arguments[0][arguments[1]]"))
            })
            .map(|request| request.body["args"][1].clone())
            .collect()
    }

    #[tokio::test]
    async fn get_attribute_falls_back_to_the_dom_property() {
        // Selenium's `get_attribute` gives the DOM property when the element
        // has no such attribute: the chosen value of a `<select>`.
        let driver = select_page();
        let mut sb = driver.base_case().await;

        let value = sb.get_attribute("#pick", "value").await.unwrap();
        assert_eq!(value.as_deref(), Some("v2"));
        sb.assert_attribute("#pick", "value", "v2").await.unwrap();
        assert_eq!(property_reads(&driver), [json!("value"), json!("value")]);
    }

    #[tokio::test]
    async fn an_attribute_that_is_there_is_not_looked_up_as_a_property() {
        let driver = select_page();
        let mut sb = driver.base_case().await;

        let id = sb.get_attribute("#pick", "id").await.unwrap();
        assert_eq!(id.as_deref(), Some("pick"));
        assert!(property_reads(&driver).is_empty());
    }
}


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
