// Tests of what the element assertions, waits and text entry wait for: an
// element that is displayed (`assert_element`, `wait_for_element`,
// `assert_elements`, `add_text`, `update_text`) or only in the DOM
// (`assert_element_present`, `wait_for_query_selector`), and that they wait
// for it as Python's do. They run against the mock WebDriver in
// `webdriver_support` (no browser needed). Included (for tests only) at the end
// of `base_case.rs` like the files beside it.

mod tests_selector_forms {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::SelectorBuf;
    use serde_json::{json, Value};

    /// A page whose every lookup finds the element `a`, displayed and enabled.
    fn any_selector_finds_a() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("POST", "/elements") => Reply::Value(json!([element("a")])),
                ("GET", "/element/a/displayed" | "/element/a/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn a_selector_may_be_a_str_a_string_or_a_selector_buf() {
        let driver = any_selector_finds_a();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();
        let owned = String::from("#a");
        let typed = SelectorBuf::css("#a");

        sb.click("#a").await.unwrap();
        sb.click(owned.clone()).await.unwrap();
        sb.click(&owned).await.unwrap();
        sb.click(SelectorBuf::css("#a")).await.unwrap();
        sb.click(&typed).await.unwrap();

        // Every form reaches the driver as the same lookup, and the same click.
        assert_eq!(driver.requests("/element/a/click").len(), 5);
        let lookups = driver.requests("/element");
        assert!(lookups.len() >= 5, "{} lookups", lookups.len());
        for lookup in lookups {
            assert_eq!(lookup.body["using"], "css selector");
            assert_eq!(lookup.body["value"], "#a");
        }
    }

    #[tokio::test]
    async fn an_explicit_kind_is_not_guessed_again_by_the_method() {
        let driver = any_selector_finds_a();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();

        // The guess reads `//a` as XPath; the constructor says it is CSS, and
        // every lookup the method makes keeps that.
        sb.click(SelectorBuf::css("//a")).await.unwrap();
        for lookup in driver.requests("/element") {
            assert_eq!(lookup.body["using"], "css selector");
            assert_eq!(lookup.body["value"], "//a");
        }
        sb.click("//a").await.unwrap();
        assert_eq!(
            driver.requests("/element").pop().unwrap().body["using"],
            "xpath"
        );
    }

    #[tokio::test]
    async fn a_contains_selector_is_looked_up_as_xpath_by_the_method() {
        let driver = any_selector_finds_a();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();

        sb.click("p:contains('x')").await.unwrap();

        let lookup = driver.requests("/element").pop().unwrap();
        assert_eq!(lookup.body["using"], "xpath");
        assert_eq!(lookup.body["value"], ".//p[contains(., 'x')]");
    }
}


mod tests_element_semantics {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};
    use std::time::{Duration, Instant};

    /// How long the tests that wait let the page take to show its element.
    const LATE: Duration = Duration::from_millis(400);

    /// A time that never comes.
    const NEVER: Duration = Duration::from_secs(3600);

    /// A page with one element, `#a`, that is in the DOM once `found_after`
    /// has passed since the page was made and displayed once `shown_after`
    /// has. Every other selector finds nothing.
    fn page(found_after: Duration, shown_after: Duration) -> MockWebDriver {
        let born = Instant::now();
        MockWebDriver::start(json!({}), move |request| {
            let found = born.elapsed() >= found_after;
            let asked_for_a = request.body["value"].as_str() == Some("#a");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if found && asked_for_a => Reply::Value(element("a")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if found && asked_for_a => {
                    Reply::Value(json!([element("a")]))
                }
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/a/displayed") => {
                    Reply::Value(json!(born.elapsed() >= shown_after))
                }
                ("GET", "/element/a/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn the_visible_assertions_and_waits_wait_for_the_element_to_be_displayed() {
        // Python's `assert_element` and `wait_for_element` wait for a displayed
        // element; these used to look once for one that was in the DOM.
        for check in 0..4 {
            let driver = page(Duration::ZERO, LATE);
            let mut sb = driver.base_case().await;
            sb.set_timeout(5).await.unwrap();
            match check {
                0 => sb.assert_element("#a").await,
                1 => sb.assert_element_visible("#a").await,
                2 => sb.assert_elements(&["#a"]).await,
                _ => sb.wait_for_element("#a").timeout(Duration::from_secs(5)).await,
            }
            .unwrap_or_else(|error| panic!("check {check}: {error:?}"));
        }
    }

    #[tokio::test]
    async fn an_element_that_is_in_the_dom_but_not_displayed_is_not_visible() {
        let driver = page(Duration::ZERO, NEVER);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.assert_element("#a").await.unwrap_err();
        sb.assert_element_visible("#a").await.unwrap_err();
        sb.assert_elements(&["#a"]).await.unwrap_err();
        sb.wait_for_element("#a").timeout(Duration::ZERO).await.unwrap_err();
        // ... but it is present.
        sb.assert_element_present("#a").await.unwrap();
        sb.assert_elements_present(&["#a"]).await.unwrap();
        sb.wait_for_element_present("#a", 0).await.unwrap();
        sb.wait_for_query_selector("#a", 0).await.unwrap();
    }

    #[tokio::test]
    async fn the_present_assertions_wait_for_the_element_to_appear() {
        for check in 0..3 {
            let driver = page(LATE, Duration::ZERO);
            let mut sb = driver.base_case().await;
            sb.set_timeout(5).await.unwrap();
            match check {
                0 => sb.assert_element_present("#a").await,
                1 => sb.assert_elements_present(&["#a"]).await,
                _ => sb.wait_for_query_selector("#a", 5).await,
            }
            .unwrap_or_else(|error| panic!("check {check}: {error:?}"));
        }
        let driver = page(NEVER, Duration::ZERO);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();
        sb.assert_element_present("#a").await.unwrap_err();
        sb.assert_elements_present(&["#a"]).await.unwrap_err();
    }

    #[tokio::test]
    async fn text_entry_waits_for_the_element_to_be_displayed() {
        // `add_text` and `update_text` used to fail on an element that was not
        // there yet.
        for check in 0..4 {
            let driver = page(LATE, LATE);
            let mut sb = driver.base_case().await;
            sb.set_timeout(5).await.unwrap();
            match check {
                0 => sb.add_text("#a", "x").await,
                1 => sb.send_keys("#a", "x").await,
                2 => sb.update_text("#a", "x").await,
                _ => sb.type_text("#a", "x").await,
            }
            .unwrap_or_else(|error| panic!("check {check}: {error:?}"));
            assert_eq!(
                driver.requests("/element/a/value").len(),
                1,
                "check {check} typed once"
            );
        }
    }

    #[tokio::test]
    async fn adding_text_does_not_clear_the_field_but_updating_it_does() {
        let driver = page(Duration::ZERO, Duration::ZERO);
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();

        sb.add_text("#a", "x").await.unwrap();
        assert!(driver.requests("/element/a/clear").is_empty());
        sb.update_text("#a", "y").await.unwrap();
        assert_eq!(driver.requests("/element/a/clear").len(), 1);
    }
}
