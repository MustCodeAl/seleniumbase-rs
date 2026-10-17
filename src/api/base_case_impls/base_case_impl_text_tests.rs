// Tests of the text assertions and waits: the order of their arguments (the
// text first and the selector second, as in Python SeleniumBase) and what they
// look at. They run against the mock WebDriver in `webdriver_support` (no
// browser needed). Included (for tests only) at the end of `base_case.rs` like
// the files beside it.

mod tests_text_argument_order {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};

    /// A page with one element, `h1`, which is displayed and says "Welcome".
    /// Every other selector finds nothing.
    fn welcome_page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            let asked_for_h1 = request.body["value"].as_str() == Some("h1");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if asked_for_h1 => Reply::Value(element("h1")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if asked_for_h1 => Reply::Value(json!([element("h1")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/h1/text") => Reply::Value(json!("Welcome")),
                ("GET", "/element/h1/displayed") => Reply::Value(json!(true)),
                ("GET", "/element/h1/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The selectors the driver was asked to find, oldest first.
    fn looked_up(driver: &MockWebDriver) -> Vec<String> {
        ["/element", "/elements"]
            .into_iter()
            .flat_map(|path| driver.requests(path))
            .map(|request| request.body["value"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[tokio::test]
    async fn the_text_assertions_take_the_text_first_and_the_selector_second() {
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.assert_text("Welcome", "h1")
            .await
            .expect("h1 contains Welcome");
        sb.assert_exact_text("Welcome", "h1")
            .await
            .expect("h1 is exactly Welcome");
        sb.assert_exact_text_not_visible("Goodbye", "h1")
            .await
            .expect("h1 does not say Goodbye");
        sb.assert_exact_text_not_visible("Welcome", "h1")
            .await
            .expect_err("h1 does say Welcome");

        assert!(
            looked_up(&driver).iter().all(|selector| selector == "h1"),
            "the driver was asked for something that is not a selector: {:?}",
            looked_up(&driver)
        );
    }

    #[tokio::test]
    async fn swapped_arguments_are_named_in_the_failure() {
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        // The old order: selector first, text second.
        let message = sb
            .assert_text("h1", "Welcome")
            .await
            .expect_err("no element is called Welcome")
            .to_string();
        assert!(message.contains("swapped"), "{message}");
        let message = sb
            .assert_exact_text("h1", "Welcome")
            .await
            .expect_err("no element is called Welcome")
            .to_string();
        assert!(message.contains("swapped"), "{message}");
    }

    #[tokio::test]
    async fn a_failed_text_assertion_names_the_text_and_the_selector() {
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        let error = sb
            .assert_text("Goodbye", "h1")
            .await
            .expect_err("h1 does not say Goodbye");
        let message = error.to_string();
        assert!(
            message.contains("Goodbye") && message.contains("h1"),
            "{message}"
        );
    }
}
