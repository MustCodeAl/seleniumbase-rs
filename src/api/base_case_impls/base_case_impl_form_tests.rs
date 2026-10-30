// Tests of how `BaseCase` reads and clicks form fields: the text of an
// `<input>` or `<textarea>`, an attribute that is only a DOM property,
// checkboxes that sit at zero opacity, `js_click`, `set_value`. They run
// against the mock WebDriver in `webdriver_support` (no browser needed); the
// real-Chrome checks of the same behaviour are in `tests/form_fields_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_form_signatures {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::SeleniumBaseError;
    use serde_json::{json, Value};

    /// A page with a selected radio button `#on` and an unselected one `#off`,
    /// and a button `#late` that is in the page only from its `after`-th
    /// lookup on.
    fn page(after: usize) -> MockWebDriver {
        let lookups = Arc::new(AtomicUsize::new(0));
        MockWebDriver::start(json!({}), move |request| {
            let selector = request.body["value"].as_str();
            match (request.method.as_str(), request.path.as_str(), selector) {
                ("POST", "/element", Some("#on")) => Reply::Value(element("on")),
                ("POST", "/element", Some("#off")) => Reply::Value(element("off")),
                ("POST", "/element", Some("#late")) => {
                    if lookups.fetch_add(1, Ordering::SeqCst) < after {
                        Reply::Error(404, "no such element")
                    } else {
                        Reply::Value(element("late"))
                    }
                }
                ("POST", "/element", _) => Reply::Error(404, "no such element"),
                ("GET", "/element/on/selected", _) => Reply::Value(json!(true)),
                ("GET", "/element/off/selected", _) => Reply::Value(json!(false)),
                ("GET", "/element/late/displayed", _) => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn a_selection_is_asserted_in_one_expression_as_python_writes_it() {
        // Python: self.assert_true(self.is_selected(selector)). Both take
        // `&self`, so the case is borrowed twice at once; with `&mut self` on
        // `is_selected` this does not compile.
        let driver = page(0);
        let sb = driver.base_case().await;

        sb.assert_true(sb.is_selected("#on").await.unwrap()).unwrap();
        sb.assert_false(sb.is_selected("#off").await.unwrap()).unwrap();
    }

    #[tokio::test]
    async fn wait_for_element_takes_no_timeout_and_waits_for_the_default_one() {
        let driver = page(3);
        let mut sb = driver.base_case().await;
        sb.set_timeout(10).await.unwrap();

        sb.wait_for_element("#late").await.unwrap();
    }

    #[tokio::test]
    async fn the_timeout_of_a_wait_for_element_replaces_the_default() {
        let driver = page(usize::MAX);
        let mut sb = driver.base_case().await;
        sb.set_timeout(30).await.unwrap();

        let error = sb
            .wait_for_element("#late")
            .timeout(Duration::ZERO)
            .await
            .unwrap_err();

        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
    }
}


mod tests_form_set_value {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};

    /// A page whose only element is an `<input>` of type `kind`: the script
    /// that sets its value answers the type.
    fn input_page(kind: &'static str) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                ("POST", "/execute/sync") => {
                    let script = request.body["script"].as_str().unwrap_or_default();
                    if script.contains("getAttribute('type')") {
                        Reply::Value(json!(kind))
                    } else {
                        Reply::Value(Value::Null)
                    }
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The scripts the driver ran that fire a `mousemove` event.
    fn mouse_moves(driver: &MockWebDriver) -> usize {
        driver
            .requests("/execute/sync")
            .iter()
            .filter(|request| request.body["script"].as_str().is_some_and(|s| s.contains("mousemove")))
            .count()
    }

    #[tokio::test]
    async fn set_value_on_a_range_input_fires_a_mousemove_event() {
        // The page's listeners (a progress bar) run on the mouse, not on the
        // value: Python fires `mousemove` after it sets a range input.
        let driver = input_page("range");
        let mut sb = driver.base_case().await;

        sb.set_value("#slider", "100").await.unwrap();

        assert_eq!(mouse_moves(&driver), 1);
        let set = &driver.requests("/execute/sync")[0];
        assert_eq!(set.body["args"][1], "100");
    }

    #[tokio::test]
    async fn set_value_on_a_text_input_fires_no_mouse_event() {
        let driver = input_page("text");
        let mut sb = driver.base_case().await;

        sb.set_value("#name", "hello").await.unwrap();

        assert_eq!(mouse_moves(&driver), 0);
    }

    #[tokio::test]
    async fn a_trailing_newline_presses_enter_and_is_not_part_of_the_value() {
        let driver = input_page("text");
        let mut sb = driver.base_case().await;

        sb.set_value("#search", "query\n").await.unwrap();

        let set = &driver.requests("/execute/sync")[0];
        assert_eq!(set.body["args"][1], "query");
        let keys = driver.requests("/element/e1/value");
        assert_eq!(keys.len(), 1, "Enter was not pressed once");
        assert_eq!(keys[0].body["text"], "\u{e006}");
    }

    #[tokio::test]
    async fn a_range_input_set_with_a_newline_presses_enter_and_not_the_mouse() {
        let driver = input_page("range");
        let mut sb = driver.base_case().await;

        sb.set_value("#slider", "7\n").await.unwrap();

        assert_eq!(driver.requests("/element/e1/value").len(), 1);
        assert_eq!(mouse_moves(&driver), 0);
    }
}


mod tests_form_clicks {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::SeleniumBaseError;
    use serde_json::{json, Value};

    /// A driver that has no element for the first `missing` lookups and the
    /// element `e1` after that.
    fn late_element(missing: usize) -> MockWebDriver {
        let lookups = Arc::new(AtomicUsize::new(0));
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => {
                    if lookups.fetch_add(1, Ordering::SeqCst) < missing {
                        Reply::Error(404, "no such element")
                    } else {
                        Reply::Value(element("e1"))
                    }
                }
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn js_click_reads_every_selector_syntax_and_clicks_the_element_it_found() {
        // `document.querySelector` cannot read `:contains()` or XPath; the
        // element is found by the driver and handed to the script.
        let driver = late_element(0);
        let mut sb = driver.base_case().await;

        let selectors = [
            ("a:contains(\"Coffee Cart\")", "xpath"),
            ("//a[@id='coffee']", "xpath"),
            ("#coffee", "css selector"),
        ];
        for (css, using) in selectors {
            sb.js_click(css).await.unwrap();

            let find = driver.requests("/element").pop().expect("a lookup");
            assert_eq!(find.body["using"], using, "{css}");
            let script = driver.requests("/execute/sync").pop().expect("a script");
            assert_eq!(script.body["script"], "arguments[0].click();", "{css}");
            let clicked = script.body["args"][0].as_object().and_then(|r| r.values().next());
            assert_eq!(clicked, Some(&json!("e1")), "{css}");
        }
    }

    #[tokio::test]
    async fn js_click_waits_for_an_element_that_is_not_there_yet() {
        let driver = late_element(3);
        let mut sb = driver.base_case().await;
        sb.set_timeout(10).await.unwrap();

        sb.js_click("#late").await.unwrap();

        assert_eq!(driver.requests("/execute/sync").len(), 1);
    }

    #[tokio::test]
    async fn js_click_on_an_element_that_never_comes_times_out() {
        let driver = late_element(usize::MAX);
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        let error = sb.js_click("#never").await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
        assert!(driver.requests("/execute/sync").is_empty());
    }
}


mod tests_form_toggles {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};

    /// A page whose only element is the unchecked checkbox `#box`. It is
    /// displayed only once a script has raised its opacity from 0 to 0.001
    /// (as TodoMVC's toggle needs), unless `stays_hidden`: then it has
    /// `display: none` and nothing but a script can click it.
    fn toggle_page(stays_hidden: bool) -> MockWebDriver {
        let raised = Arc::new(AtomicBool::new(false));
        MockWebDriver::start(json!({}), move |request| {
            let known = request.body["value"].as_str() == Some("#box");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if known => Reply::Value(element("box")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if known => Reply::Value(json!([element("box")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/box/property/checked") => Reply::Value(json!("false")),
                ("GET", "/element/box/displayed") => {
                    Reply::Value(json!(raised.load(Ordering::SeqCst) && !stays_hidden))
                }
                ("GET", "/element/box/enabled") => Reply::Value(json!(true)),
                ("POST", "/execute/sync") => {
                    let script = request.body["script"].as_str().unwrap_or_default();
                    if script.contains("0.001") {
                        raised.store(true, Ordering::SeqCst);
                    }
                    Reply::Value(json!(""))
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The scripts the driver ran, oldest first.
    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .into_iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .collect()
    }

    #[tokio::test]
    async fn a_checkbox_at_zero_opacity_is_clicked_after_its_opacity_is_raised() {
        // Python raises the opacity a little, clicks, and puts it back.
        let driver = toggle_page(false);
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        sb.check_if_unchecked("#box").await.unwrap();

        assert_eq!(driver.requests("/element/box/click").len(), 1);
        let scripts = scripts(&driver);
        let raised = scripts.iter().position(|script| script.contains("0.001"));
        let restored = scripts.iter().rposition(|script| script.contains("style.opacity"));
        assert!(raised.is_some(), "the opacity was not raised: {scripts:?}");
        assert!(restored > raised, "the opacity was not put back: {scripts:?}");
    }

    #[tokio::test]
    async fn a_checkbox_that_stays_hidden_is_clicked_with_a_script() {
        let driver = toggle_page(true);
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        sb.check_if_unchecked("#box").await.unwrap();

        assert!(driver.requests("/element/box/click").is_empty());
        let scripts = scripts(&driver);
        assert!(
            scripts.iter().any(|script| script.contains(".click()")),
            "no script clicked the box: {scripts:?}"
        );
    }

    #[tokio::test]
    async fn unchecking_a_checkbox_that_is_not_checked_does_nothing() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("box")),
                ("GET", "/element/box/property/checked") => Reply::Value(json!("false")),
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await;

        sb.uncheck_if_checked("#box").await.unwrap();

        assert!(driver.requests("/element/box/click").is_empty());
        assert!(driver.requests("/execute/sync").is_empty());
    }
}


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
