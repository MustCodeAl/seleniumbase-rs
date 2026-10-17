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
        heading(true)
    }

    /// A page whose only element, `h1`, says "Welcome" and is `displayed` or
    /// not. Every other selector finds nothing.
    fn heading(displayed: bool) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            let asked_for_h1 = request.body["value"].as_str() == Some("h1");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if asked_for_h1 => Reply::Value(element("h1")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if asked_for_h1 => Reply::Value(json!([element("h1")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/h1/text") => Reply::Value(json!("Welcome")),
                ("GET", "/element/h1/displayed") => Reply::Value(json!(displayed)),
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
    async fn the_text_waits_take_the_text_first_and_the_selector_second() {
        let driver = welcome_page();
        let sb = driver.base_case().await;

        sb.wait_for_text("Welcome", "h1", 1).await.unwrap();
        sb.wait_for_text_visible("Welcome", "h1", 1).await.unwrap();
        sb.wait_for_exact_text_visible("Welcome", "h1", 1)
            .await
            .unwrap();
        sb.wait_for_exact_text("Welcome", "h1", 1).await.unwrap();
        sb.wait_for_text_not_visible("Goodbye", "h1", 1)
            .await
            .unwrap();
        sb.wait_for_exact_text_not_visible("Goodbye", "h1", 1)
            .await
            .unwrap();

        assert!(
            looked_up(&driver).iter().all(|selector| selector == "h1"),
            "the driver was asked for something that is not a selector: {:?}",
            looked_up(&driver)
        );
    }

    #[tokio::test]
    async fn the_text_waits_look_at_the_element_and_nothing_else() {
        // `wait_for_exact_text` used to look at every element of the page,
        // `<title>` and `<script>` included, so it found text the element
        // does not show. It is `h1` that says "Welcome"; "Home" is elsewhere.
        let driver = welcome_page();
        let sb = driver.base_case().await;

        sb.wait_for_exact_text("Home", "h1", 0).await.unwrap_err();
        sb.wait_for_exact_text_visible("Home", "h1", 0)
            .await
            .unwrap_err();
        sb.wait_for_text("Home", "h1", 0).await.unwrap_err();
        sb.wait_for_text_not_visible("Welcome", "h1", 0)
            .await
            .unwrap_err();
        sb.wait_for_exact_text_not_visible("Welcome", "h1", 0)
            .await
            .unwrap_err();
        assert!(
            driver.requests("/execute/sync").is_empty(),
            "the waits ran a page script instead of asking for the element"
        );
    }

    #[tokio::test]
    async fn text_in_an_element_that_is_not_displayed_is_not_visible() {
        let driver = heading(false);
        let sb = driver.base_case().await;

        sb.wait_for_text_visible("Welcome", "h1", 0)
            .await
            .unwrap_err();
        sb.wait_for_exact_text_visible("Welcome", "h1", 0)
            .await
            .unwrap_err();
        sb.wait_for_text_not_visible("Welcome", "h1", 0)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn the_deferred_text_checks_and_their_delayed_aliases_take_the_text_first() {
        // `delayed_assert_text` used to take the selector first while the
        // method it calls takes the text first, so `("Welcome", "h1")` looked
        // for an element called `Welcome`.
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.deferred_assert_text("Welcome", "h1").await.unwrap();
        sb.delayed_assert_text("Welcome", "h1").await.unwrap();
        sb.deferred_assert_exact_text("Welcome", "h1").await.unwrap();
        sb.delayed_assert_exact_text("Welcome", "h1").await.unwrap();
        sb.process_deferred_asserts()
            .await
            .expect("all four checks hold");

        assert!(
            looked_up(&driver).iter().all(|selector| selector == "h1"),
            "the driver was asked for something that is not a selector: {:?}",
            looked_up(&driver)
        );
    }

    #[tokio::test]
    async fn a_deferred_text_check_that_fails_is_reported_when_the_asserts_are_processed() {
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.delayed_assert_text("Goodbye", "h1").await.unwrap();
        sb.delayed_assert_exact_text("Goodbye", "h1").await.unwrap();
        let error = sb
            .process_deferred_asserts()
            .await
            .expect_err("both checks failed")
            .to_string();
        assert!(error.contains("Goodbye"), "{error}");
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
