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


/// One smoke test per file of `base_case_impls/` that reads a selector: a
/// method of the file is called with each form a selector takes, and the
/// driver must be sent the same requests every time.
mod tests_selector_forms_by_file {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::SelectorBuf;
    use crate::api::recorder::RecordedCommand;
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

    /// The method, path and body of each request sent after the first
    /// `skipped` ones (the session's own).
    fn sent_after(driver: &MockWebDriver, skipped: usize) -> Vec<(String, String, String)> {
        driver
            .all_requests()
            .into_iter()
            .skip(skipped)
            .map(|request| (request.method, request.path, request.body.to_string()))
            .collect()
    }

    /// Defines a test that runs `$call` once for each form of `$selector`
    /// (`&str`, `String`, `SelectorBuf` and `&SelectorBuf`, all `#a`), each on a
    /// page of its own, and checks that the four runs sent the same requests
    /// and that there was at least one.
    macro_rules! the_forms_reach_the_same_calls {
        ($name:ident, |$sb:ident, $selector:ident| $call:expr) => {
            #[tokio::test]
            async fn $name() {
                let mut runs = Vec::new();
                for form in 0..4 {
                    let driver = any_selector_finds_a();
                    let $sb = &mut driver.base_case().await;
                    $sb.set_timeout(1).await.unwrap();
                    let skipped = driver.all_requests().len();
                    match form {
                        0 => {
                            let $selector = "#a";
                            let _ = $call.await;
                        }
                        1 => {
                            let $selector = String::from("#a");
                            let _ = $call.await;
                        }
                        2 => {
                            let $selector = SelectorBuf::css("#a");
                            let _ = $call.await;
                        }
                        _ => {
                            let $selector = &SelectorBuf::css("#a");
                            let _ = $call.await;
                        }
                    }
                    runs.push(sent_after(&driver, skipped));
                }
                assert!(!runs[0].is_empty(), "the call sent nothing");
                for run in &runs[1..] {
                    assert_eq!(run, &runs[0]);
                }
            }
        };
    }

    // base_case_impl_common.rs
    the_forms_reach_the_same_calls!(common_assert_element_present, |sb, selector| sb
        .assert_element_present(selector));
    // base_case_impl_dom.rs
    the_forms_reach_the_same_calls!(dom_remove_elements, |sb, selector| sb
        .remove_elements(selector));
    // base_case_impl_extra.rs
    the_forms_reach_the_same_calls!(extra_jquery_click_all, |sb, selector| sb
        .jquery_click_all(selector));
    // base_case_impl_misc.rs
    the_forms_reach_the_same_calls!(misc_deferred_assert_element_present, |sb, selector| sb
        .deferred_assert_element_present(selector));
    // base_case_impl_mouse.rs
    the_forms_reach_the_same_calls!(mouse_hover_element, |sb, selector| sb
        .hover_element(selector));
    // base_case_impl_shadow.rs
    the_forms_reach_the_same_calls!(shadow_shadow_click, |sb, selector| sb
        .shadow_click(selector));
    // base_case_impl_parity.rs
    the_forms_reach_the_same_calls!(parity_js_click_if_visible, |sb, selector| sb
        .js_click_if_visible(selector));
    // base_case_impl_highlight.rs
    the_forms_reach_the_same_calls!(highlight_highlight_if_visible, |sb, selector| sb
        .highlight_if_visible(selector));
    // base_case_impl_cdp_page.rs
    the_forms_reach_the_same_calls!(cdp_page_cdp_get_text, |sb, selector| sb
        .cdp_get_text(selector));

    #[tokio::test]
    async fn a_selector_is_recorded_as_the_user_wrote_it() {
        let driver = any_selector_finds_a();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();

        sb.click("p:contains('x')").await.unwrap();
        sb.click("text=Home").await.unwrap();
        sb.click(SelectorBuf::xpath("//a[@id='x']")).await.unwrap();

        let recorded: Vec<RecordedCommand> = sb
            .recorder
            .lock()
            .unwrap()
            .actions
            .iter()
            .map(|action| action.command.clone())
            .collect();
        let click = |selector: &str| RecordedCommand::Click {
            selector: selector.to_owned(),
        };
        // `:contains()` stays CSS text until the lookup; the prefix `text=` is
        // written as `link=`, the one spelling of an exact link text.
        assert_eq!(
            recorded,
            [
                click("p:contains('x')"),
                click("link=Home"),
                click("//a[@id='x']")
            ]
        );
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
