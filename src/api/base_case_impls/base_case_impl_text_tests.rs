// Tests of the text assertions and waits: the order of their arguments (the
// text first and the selector second, as in Python SeleniumBase) and what they
// look at. They run against the mock WebDriver in `webdriver_support` (no
// browser needed). Included (for tests only) at the end of `base_case.rs` like
// the files beside it.

mod tests_text_argument_order {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A page with one element, `h1`, which is displayed and says "Welcome".
    /// Every other selector finds nothing.
    fn welcome_page() -> MockWebDriver {
        heading(true)
    }

    /// A page whose only element, `h1`, says "Welcome" and is `displayed` or
    /// not. Every other selector finds nothing.
    fn heading(displayed: bool) -> MockWebDriver {
        page(|_| "Welcome", displayed)
    }

    /// A page whose only element, `h1`, is `displayed` or not and says
    /// `says(n)` the n-th time its text is read (counting from 0), so a test
    /// can have the text arrive late. Every other selector finds nothing.
    fn page(says: fn(usize) -> &'static str, displayed: bool) -> MockWebDriver {
        let reads = Arc::new(AtomicUsize::new(0));
        MockWebDriver::start(json!({}), move |request| {
            let asked_for_h1 = request.body["value"].as_str() == Some("h1");
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if asked_for_h1 => Reply::Value(element("h1")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if asked_for_h1 => Reply::Value(json!([element("h1")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/h1/text") => Reply::Value(json!(says(
                    reads.fetch_add(1, Ordering::SeqCst)
                ))),
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
        let error = sb
            .process_deferred_asserts()
            .await
            .expect_err("the check failed")
            .to_string();
        assert!(error.contains("Goodbye"), "{error}");
    }

    #[tokio::test]
    async fn the_text_assertions_wait_for_text_that_shows_up_late() {
        // Python's `assert_text` waits for the text; this one used to read the
        // text once and fail on a page that was still loading.
        let late = || page(|read| if read < 2 { "Loading" } else { "Welcome" }, true);
        for check in 0..3 {
            let driver = late();
            let mut sb = driver.base_case().await;
            sb.set_timeout(5).await.unwrap();
            match check {
                0 => sb.assert_text("Welcome", "h1").await,
                1 => sb.assert_text_visible("Welcome", "h1").await,
                _ => sb.assert_exact_text("Welcome", "h1").await,
            }
            .expect("the text shows up on the third look");
        }
    }

    #[tokio::test]
    async fn the_not_visible_assertions_wait_for_the_text_to_go() {
        let going = || page(|read| if read < 2 { "Welcome" } else { "Bye" }, true);
        let driver = going();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();
        sb.assert_text_not_visible("Welcome", "h1")
            .await
            .expect("the text is gone on the third look");

        let driver = going();
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();
        sb.assert_exact_text_not_visible("Welcome", "h1")
            .await
            .expect("the text is gone on the third look");

        // Text that stays is reported once the timeout ends.
        let driver = heading(true);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();
        sb.assert_text_not_visible("Welcome", "h1")
            .await
            .expect_err("the text stays");
    }

    #[tokio::test]
    async fn exact_text_ignores_whitespace_at_the_ends() {
        let driver = page(|_| "  Welcome \n", true);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.assert_exact_text("Welcome", "h1").await.unwrap();
        sb.assert_exact_text(" Welcome ", "h1").await.unwrap();
        assert!(sb.is_exact_text_visible("Welcome", "h1").await.unwrap());
        sb.assert_exact_text("Welcom", "h1").await.unwrap_err();
    }

    #[tokio::test]
    async fn text_in_an_element_that_is_not_displayed_does_not_pass_an_assertion() {
        let driver = heading(false);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.assert_text("Welcome", "h1").await.unwrap_err();
        sb.assert_exact_text("Welcome", "h1").await.unwrap_err();
        sb.assert_text_not_visible("Welcome", "h1").await.unwrap();
    }

    #[tokio::test]
    async fn a_deferred_text_check_waits_a_couple_of_seconds_whatever_the_default_timeout() {
        let driver = page(|read| if read < 2 { "Loading" } else { "Welcome" }, true);
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        sb.deferred_assert_text("Welcome", "h1").await.unwrap();
        sb.process_deferred_asserts()
            .await
            .expect("the text showed up within the wait of a deferred check");
    }

    #[tokio::test]
    async fn the_find_text_methods_return_the_element_of_the_selector() {
        // Python's `find_text(text, selector)` returns the selector's element;
        // these used to search all of `<body>` and return an `Option`.
        let driver = welcome_page();
        let mut sb = driver.base_case().await;
        sb.set_timeout(0).await.unwrap();

        for found in [
            sb.find_text("Welcome", "h1").await.unwrap(),
            sb.find_exact_text(" Welcome ", "h1").await.unwrap(),
            sb.find_non_empty_text("h1").await.unwrap(),
        ] {
            assert_eq!(found.element_id().to_string(), "h1");
        }
        sb.find_text("Goodbye", "h1").await.unwrap_err();
        sb.find_exact_text("Welcom", "h1").await.unwrap_err();
        sb.find_non_empty_text("h2").await.unwrap_err();
        assert!(
            looked_up(&driver)
                .iter()
                .all(|selector| selector == "h1" || selector == "h2"),
            "{:?}",
            looked_up(&driver)
        );
        assert!(driver.requests("/execute/sync").is_empty());
    }

    #[tokio::test]
    async fn find_text_waits_for_text_that_shows_up_late() {
        let driver = page(|read| if read < 2 { "Loading" } else { "Welcome" }, true);
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();
        sb.find_text("Welcome", "h1")
            .await
            .expect("the text shows up on the third look");
    }

    #[tokio::test]
    async fn post_message_and_highlight_highlights_the_element_it_is_given() {
        // It used to highlight the toast it had just shown, whatever it was told.
        let driver = welcome_page();
        let sb = driver.base_case().await;

        sb.post_message_and_highlight("Saved", "h1").await.unwrap();

        let scripts: Vec<String> = driver
            .requests("/execute/sync")
            .iter()
            .map(|request| request.body["script"].as_str().unwrap_or_default().to_owned())
            .collect();
        let toast = scripts.first().expect("the toast script");
        assert!(toast.contains("Saved"), "{toast}");
        let highlight = scripts.last().expect("the highlight script");
        assert!(
            highlight.contains("\"h1\"") && !highlight.contains("sb-toast-message"),
            "{highlight}"
        );
    }

    #[tokio::test]
    async fn the_mfa_code_is_typed_followed_by_enter() {
        // Python types the code plus a newline, so the form is submitted.
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("code")),
                ("POST", "/elements") => Reply::Value(json!([element("code")])),
                ("GET", "/element/code/displayed" | "/element/code/enabled") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await;
        let secret = "JBSWY3DPEHPK3PXP";

        for enter in 0..2 {
            if enter == 0 {
                sb.enter_totp_code("#code", secret).await.unwrap();
            } else {
                sb.enter_mfa_code("#code", secret).await.unwrap();
            }
            let sent = driver.requests("/element/code/value");
            let typed = sent.last().expect("keys were sent").body["text"]
                .as_str()
                .expect("the keys")
                .to_owned();
            assert!(
                typed.len() == 7
                    && typed.ends_with('\n')
                    && typed[..6].chars().all(|c| c.is_ascii_digit()),
                "{typed:?}"
            );
        }
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

    #[tokio::test]
    async fn assert_shadow_text_takes_the_text_first_and_the_selector_second() {
        // It used to take (selector, text). The page script answers with the
        // text of the shadow element; the selector must reach the script and
        // the expected text must not.
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!("Welcome")),
                _ => Reply::Value(Value::Null),
            }
        });
        let sb = driver.base_case().await;

        sb.assert_shadow_text("Welcome", "app-root ::shadow h1")
            .await
            .expect("the shadow element says Welcome");
        let script = driver
            .requests("/execute/sync")
            .last()
            .and_then(|request| request.body["script"].as_str().map(str::to_owned))
            .expect("a script was sent");
        assert!(script.contains("app-root") && script.contains("h1"), "{script}");
        assert!(!script.contains("Welcome"), "{script}");

        // The old order is a different expectation: the text is not the selector.
        let message = sb
            .assert_shadow_text("app-root ::shadow h1", "Welcome")
            .await
            .expect_err("the shadow element does not say that")
            .to_string();
        assert!(message.contains("Welcome") && message.contains("app-root"), "{message}");
    }

    #[tokio::test]
    async fn wait_takes_whole_or_fractional_seconds() {
        // `wait` is `sleep`, as in Python, so `wait(0.5)` and `wait(2)` both
        // compile; it used to take a `u64`.
        let driver = welcome_page();
        let sb = driver.base_case().await;

        let started = std::time::Instant::now();
        sb.wait(0.2).await;
        assert!(started.elapsed() >= std::time::Duration::from_millis(190));
        sb.wait(0).await;
        sb.wait(0_u8).await;
        sb.wait(0.0_f32).await;
    }
}
