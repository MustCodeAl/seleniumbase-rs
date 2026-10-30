// Tests of the highlight family and of what demo mode shows around actions and
// assertions. They run against the mock WebDriver in `webdriver_support` (no
// browser needed): they check which scripts go to the page, with which element.
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_highlight {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A page with the visible elements `#a` and `#b` (a selector finds `a`,
    /// the list of them finds both).
    fn page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("POST", "/elements") => Reply::Value(json!([element("a"), element("b")])),
                ("GET", "/element/a/displayed" | "/element/b/displayed" | "/element/a/enabled") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/element/a/text") => Reply::Value(json!("Hi there")),
                ("GET", "/url") => Reply::Value(json!("http://example.test/")),
                ("POST", "/execute/sync") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The same page, where the element is there but not displayed.
    fn hidden_page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("GET", "/element/a/displayed") => Reply::Value(json!(false)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// A page that has no element at all.
    fn empty_page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| match request.path.as_str() {
            "/element" | "/elements" => Reply::Error(404, "no such element"),
            _ => Reply::Value(Value::Null),
        })
    }

    async fn watched(driver: &MockWebDriver, settings: RunSettings) -> BaseCase {
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.set_run_settings(settings);
        sb
    }

    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .map(|request| request.body["script"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    fn animations(driver: &MockWebDriver) -> Vec<String> {
        scripts(driver)
            .into_iter()
            .filter(|script| script.contains("rgba(255, 0, 0, 1)"))
            .collect()
    }

    fn quick() -> RunSettings {
        RunSettings::default()
            .with_demo_sleep(Duration::from_millis(5))
            .with_message_duration(Duration::from_millis(40))
    }

    #[tokio::test]
    async fn a_highlight_plays_one_animation_on_the_element_it_found() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.highlight("#a").await.unwrap();

        let ran = driver.requests("/execute/sync");
        assert_eq!(ran.len(), 1, "one script, not one request per colour");
        let script = ran[0].body["script"].as_str().unwrap();
        assert!(script.contains("n < 4;"), "four loops by default: {script}");
        assert!(script.contains("behavior: 'smooth'"), "it scrolls: {script}");
        assert!(!script.contains("#a"), "the selector is not in the script: {script}");
        assert!(
            ran[0].body["args"][0].to_string().contains('a'),
            "the element is the argument: {}",
            ran[0].body["args"]
        );
    }

    #[tokio::test]
    async fn a_highlight_changes_nothing_on_the_element_but_its_box_shadow() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.highlight("#a").await.unwrap();

        // Python's `highlight` draws no outline and sets no other style: it cycles
        // the box shadow and puts the element's own one back, all in one script
        // that returns once the animation has ended. So a page can see the shadow
        // only while the call runs, and a real browser shows nothing after it.
        let script = &scripts(&driver)[0];
        assert!(!script.contains("outline"), "{script}");
        assert!(!script.contains("cssText"), "{script}");
        let assigned: Vec<&str> = script.split("el.style.").skip(1).collect();
        assert!(assigned.len() >= 3, "the script sets the shadow: {script}");
        assert!(
            assigned.iter().all(|rest| rest.starts_with("boxShadow")),
            "only the box shadow is touched: {script}"
        );
        assert!(
            script.contains("el.style.boxShadow = original;"),
            "the element's own shadow is put back: {script}"
        );
    }

    #[tokio::test]
    async fn the_keywords_change_the_animation() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.highlight("#a").loops(2).scroll(false).await.unwrap();
        let script = &scripts(&driver)[0];
        assert!(script.contains("n < 2;"), "{script}");
        assert!(!script.contains("scrollIntoView"), "{script}");
    }

    #[tokio::test]
    async fn the_highlights_setting_wins_over_the_loops_a_call_names() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default().with_highlights(7)).await;
        sb.highlight("#a").loops(2).await.unwrap();
        sb.highlight_if_visible("#a").await.unwrap();
        let ran = scripts(&driver);
        assert!(ran.iter().all(|script| script.contains("n < 7;")), "{ran:?}");
        assert_eq!(ran.len(), 2);
    }

    #[tokio::test]
    async fn a_highlight_of_a_missing_element_is_an_error_and_if_visible_is_not() {
        let driver = empty_page();
        let sb = watched(&driver, RunSettings::default()).await;
        let error = sb
            .highlight("#nope")
            .timeout(Duration::ZERO)
            .await
            .expect_err("the element is not there");
        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
        sb.highlight_if_visible("#nope").await.unwrap();
        assert!(scripts(&driver).is_empty());

        let hidden = hidden_page();
        let sb = watched(&hidden, RunSettings::default()).await;
        sb.highlight_if_visible("#a").await.unwrap();
        assert!(scripts(&hidden).is_empty(), "a hidden element is not highlighted");
    }

    #[tokio::test]
    async fn highlight_elements_stops_at_the_limit() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.highlight_elements("li").loops(1).await.unwrap();
        assert_eq!(animations(&driver).len(), 2, "all of them without a limit");

        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.highlight_elements("li").loops(1).limit(1).await.unwrap();
        assert_eq!(animations(&driver).len(), 1);
    }

    #[tokio::test]
    async fn a_flash_paints_a_dot_and_refuses_a_colour_that_is_not_hex() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.flash("#a").await.unwrap();
        let script = &scripts(&driver)[0];
        assert!(script.contains("background:#44CC88;"), "{script}");
        assert!(script.contains("show-pointer-ani 1.00s"), "{script}");

        let error = sb
            .flash("#a")
            .color("red; background:url(x)")
            .await
            .expect_err("not hex");
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
        assert_eq!(scripts(&driver).len(), 1, "the bad colour reached no script");
    }

    #[tokio::test]
    async fn a_flash_waits_for_its_pause_only_when_given_one() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        let started = std::time::Instant::now();
        sb.flash("#a").pause(Duration::from_millis(120)).await.unwrap();
        assert!(started.elapsed() >= Duration::from_millis(120));
    }

    #[tokio::test]
    async fn highlight_click_highlights_with_three_loops_unless_demo_mode_does_it() {
        let driver = page();
        let mut sb = watched(&driver, RunSettings::default()).await;
        sb.highlight_click("#a").await.unwrap();
        let ran = animations(&driver);
        assert_eq!(ran.len(), 1);
        assert!(ran[0].contains("n < 3;"), "{}", ran[0]);
        assert_eq!(driver.requests("/element/a/click").len(), 1);

        // In demo mode the click highlights (four loops), so the call does not
        // add a second animation.
        let driver = page();
        let mut sb = watched(&driver, quick().with_demo_mode(true)).await;
        sb.highlight_click("#a").await.unwrap();
        let ran = animations(&driver);
        assert_eq!(ran.len(), 1, "{ran:?}");
        assert!(ran[0].contains("n < 4;"), "{}", ran[0]);
        assert_eq!(driver.requests("/element/a/click").len(), 1);
    }

    #[tokio::test]
    async fn highlight_type_types_after_the_highlight() {
        let driver = page();
        let mut sb = watched(&driver, RunSettings::default()).await;
        sb.highlight_update_text("#a", "hello").loops(1).await.unwrap();
        assert_eq!(animations(&driver).len(), 1);
        assert!(
            driver
                .requests("/element/a/value")
                .iter()
                .any(|request| request.body.to_string().contains("hello")),
            "the text was typed"
        );
    }

    #[tokio::test]
    async fn demo_mode_plays_the_animation_before_the_action() {
        let driver = page();
        let mut sb = watched(&driver, quick().with_demo_mode(true)).await;
        sb.click("#a").await.unwrap();
        let ran = animations(&driver);
        assert_eq!(ran.len(), 1);
        assert!(ran[0].contains("n < 4;") && ran[0].contains("behavior: 'smooth'"), "{}", ran[0]);
        assert_eq!(driver.requests("/element/a/click").len(), 1);
        assert_eq!(sb.demo_pauses(), [PauseKind::Short]);
    }

    #[tokio::test]
    async fn slow_mode_only_scrolls() {
        let driver = page();
        let mut sb = watched(&driver, quick().with_slow_mode(true)).await;
        let started = std::time::Instant::now();
        sb.click("#a").await.unwrap();
        assert!(started.elapsed() >= Duration::from_millis(200), "0.08 s, scroll, 0.12 s");
        let ran = scripts(&driver);
        assert_eq!(ran.len(), 1, "{ran:?}");
        assert!(ran[0].contains("behavior: 'smooth'") && !ran[0].contains("boxShadow"), "{}", ran[0]);
    }

    #[tokio::test]
    async fn neither_mode_adds_a_script_to_an_action() {
        let driver = page();
        let mut sb = watched(&driver, RunSettings::default()).await;
        sb.click("#a").await.unwrap();
        assert!(scripts(&driver).is_empty());
    }

    #[tokio::test]
    async fn activating_demo_mode_only_flips_the_setting() {
        let driver = page();
        let mut sb = watched(&driver, RunSettings::default()).await;
        sb.activate_demo_mode().await.unwrap();
        assert!(sb.run_settings().demo_mode());
        sb.deactivate_demo_mode().await.unwrap();
        assert!(!sb.run_settings().demo_mode());
        assert!(scripts(&driver).is_empty());
        assert!(driver.requests("/refresh").is_empty(), "no page reload");
    }

    #[tokio::test]
    async fn a_passed_assertion_announces_itself_in_demo_mode() {
        let driver = page();
        let sb = watched(&driver, quick().with_demo_mode(true)).await;
        sb.assert_text("Hi", "#m").await.unwrap();

        let ran = scripts(&driver);
        let message = r#""<b>ASSERT TEXT</b>: {Hi} in CSS SELECTOR: #m""#;
        let posted = ran
            .iter()
            .position(|script| script.contains(message) && script.contains("type: \"success\""))
            .unwrap_or_else(|| panic!("no success message in {ran:?}"));
        let flashed = ran
            .iter()
            .position(|script| script.contains("el.__sbShadow = el.style.boxShadow"))
            .expect("the green flash");
        let restored = ran
            .iter()
            .position(|script| script.contains("el.style.boxShadow = el.__sbShadow"))
            .expect("the shadow is put back");
        assert!(flashed < posted && posted < restored, "{ran:?}");
        // The theme is Python's: future, at the bottom right.
        assert!(
            ran.iter().any(|script| script.contains("messenger-on-bottom messenger-on-right")),
            "{ran:?}"
        );
    }

    #[tokio::test]
    async fn the_other_assertions_say_what_python_says() {
        let driver = page();
        let sb = watched(&driver, quick().with_demo_mode(true)).await;
        sb.assert_element("#a").await.unwrap();
        sb.assert_exact_text("Hi there", "#a").await.unwrap();
        sb.assert_url("http://example.test/").await.unwrap();
        sb.assert_url_contains("example").await.unwrap();
        let ran = scripts(&driver).join("\n");
        for message in [
            r#""<b>ASSERT CSS SELECTOR</b>: #a""#,
            r#""<b>ASSERT EXACT TEXT</b>: {Hi there} in CSS SELECTOR: #a""#,
            r#""<b>ASSERT URL</b>: {http:\/\/example.test\/}""#,
            r#""<b>ASSERT URL CONTAINS</b>: {example}""#,
        ] {
            // The text is JSON-escaped in the script; the label is not.
            let label = message.split("</b>").next().unwrap();
            assert!(ran.contains(label), "{label} is not in {ran}");
        }
    }

    #[tokio::test]
    async fn assertions_send_nothing_extra_outside_demo_mode() {
        let driver = page();
        let sb = watched(&driver, RunSettings::default()).await;
        sb.assert_text("Hi", "#a").await.unwrap();
        sb.assert_element("#a").await.unwrap();
        sb.assert_url_contains("example").await.unwrap();
        assert!(scripts(&driver).is_empty());
    }

    #[tokio::test]
    async fn a_message_with_a_highlight_waits_for_the_message_to_go() {
        let driver = page();
        let sb = watched(
            &driver,
            RunSettings::default().with_message_duration(Duration::from_millis(150)),
        )
        .await;
        let started = std::time::Instant::now();
        sb.post_message_and_highlight("<b>Saved</b>", "#a").await.unwrap();
        assert!(started.elapsed() >= Duration::from_millis(150), "{:?}", started.elapsed());

        let ran = scripts(&driver);
        let posted = ran
            .iter()
            .position(|script| script.contains("<b>Saved</b>") && script.contains("type: \"success\""))
            .unwrap_or_else(|| panic!("no success message in {ran:?}"));
        assert!(
            ran[..posted].iter().any(|script| script.contains("el.__sbShadow")),
            "the flash comes first"
        );
        assert!(
            ran[posted..].iter().any(|script| script.contains("el.style.boxShadow = el.__sbShadow")),
            "the shadow is put back after the message"
        );
    }
}
