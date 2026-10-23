// Tests of the pauses demo mode and slow mode make after an action, and of how
// long a posted message stays. They run against the mock WebDriver in
// `webdriver_support` (no browser needed). Included (for tests only) at the end
// of `base_case.rs` like the files beside it.

mod tests_demo_pauses {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A pause short enough for a test: a full pause is 34 ms, a short one
    /// 10 ms.
    const PAUSE: Duration = Duration::from_millis(34);

    const BEFORE: &str = "http://example.test/before";
    const AFTER: &str = "http://example.test/after";

    /// A page with one element, `#a`. A click on it, and keys sent to it, take
    /// the browser to another URL when `navigates` is set.
    fn page(navigates: bool) -> MockWebDriver {
        let moved = Arc::new(AtomicBool::new(false));
        MockWebDriver::start(json!({}), move |request| {
            let script = request.body["script"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("POST", "/elements") => Reply::Value(json!([element("a")])),
                ("GET", "/element/a/displayed" | "/element/a/enabled") => {
                    Reply::Value(json!(true))
                }
                ("POST", "/element/a/click" | "/element/a/value") => {
                    moved.store(navigates, Ordering::SeqCst);
                    Reply::Value(Value::Null)
                }
                ("GET", "/url") => Reply::Value(json!(if moved.load(Ordering::SeqCst) {
                    AFTER
                } else {
                    BEFORE
                })),
                ("GET" | "POST", "/window/rect") => {
                    Reply::Value(json!({ "x": 0, "y": 0, "width": 800, "height": 600 }))
                }
                ("POST", "/execute/sync") if script.contains("readyState") => {
                    Reply::Value(json!("complete"))
                }
                // The answer a script that looks for an element gives.
                ("POST", "/execute/sync") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn url_reads(driver: &MockWebDriver) -> usize {
        driver
            .requests("/url")
            .iter()
            .filter(|request| request.method == "GET")
            .count()
    }

    async fn demo_case(driver: &MockWebDriver) -> BaseCase {
        let mut sb = driver.base_case().await;
        sb.set_run_settings(
            RunSettings::default()
                .with_demo_mode(true)
                .with_demo_sleep(PAUSE),
        );
        sb
    }

    async fn slow_case(driver: &MockWebDriver) -> BaseCase {
        let mut sb = driver.base_case().await;
        sb.set_run_settings(
            RunSettings::default()
                .with_slow_mode(true)
                .with_demo_sleep(PAUSE),
        );
        sb
    }

    #[tokio::test]
    async fn opening_a_page_pauses_in_full_without_reading_the_url() {
        let driver = page(false);
        let mut sb = demo_case(&driver).await;
        sb.open("https://example.test/").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Full]);
        assert_eq!(url_reads(&driver), 0);
    }

    #[tokio::test]
    async fn a_click_that_stays_on_the_page_pauses_short_in_demo_mode() {
        let driver = page(false);
        let mut sb = demo_case(&driver).await;
        sb.click("#a").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Short]);
        // One read before the click and one after.
        assert_eq!(url_reads(&driver), 2);
    }

    #[tokio::test]
    async fn a_click_that_changes_the_url_pauses_in_full() {
        let driver = page(true);
        let mut sb = demo_case(&driver).await;
        sb.click("#a").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Full]);
    }

    #[tokio::test]
    async fn slow_mode_pauses_in_full_and_never_reads_the_url() {
        for navigates in [false, true] {
            let driver = page(navigates);
            let mut sb = slow_case(&driver).await;
            sb.click("#a").await.unwrap();
            assert_eq!(sb.demo_pauses(), [PauseKind::Full], "navigates: {navigates}");
            assert_eq!(url_reads(&driver), 0, "slow mode does not need the URL");
        }
    }

    #[tokio::test]
    async fn neither_mode_pauses_or_reads_the_url() {
        let driver = page(true);
        let mut sb = driver.base_case().await;
        sb.open("https://example.test/").await.unwrap();
        sb.click("#a").await.unwrap();
        sb.type_text("#a", "x").await.unwrap();
        sb.go_back().await.unwrap();
        assert!(sb.demo_pauses().is_empty());
        assert_eq!(url_reads(&driver), 0);
    }

    #[tokio::test]
    async fn typing_follows_the_same_rule_as_a_click() {
        for (navigates, expected) in [(false, PauseKind::Short), (true, PauseKind::Full)] {
            for action in 0..3 {
                let driver = page(navigates);
                let mut sb = demo_case(&driver).await;
                match action {
                    0 => sb.type_text("#a", "x").await,
                    1 => sb.update_text("#a", "x").await,
                    _ => sb.add_text("#a", "x").await,
                }
                .unwrap();
                assert_eq!(
                    sb.demo_pauses(),
                    [expected],
                    "action {action}, navigates: {navigates}"
                );
            }
        }
    }

    #[tokio::test]
    async fn pressing_keys_pauses_in_full_only_for_a_typed_enter() {
        let driver = page(false);
        let sb = demo_case(&driver).await;
        sb.press_keys("#a", "abc").await.unwrap();
        sb.press_keys("#a", "abc\n").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Short, PauseKind::Full]);
    }

    #[tokio::test]
    async fn the_other_clicks_follow_the_url_rule() {
        let driver = page(false);
        let mut sb = demo_case(&driver).await;
        sb.double_click("#a").await.unwrap();
        sb.context_click("#a").await.unwrap();
        sb.click_active_element().await.unwrap();
        sb.click_link("Home").await.unwrap();
        sb.click_partial_link_text("Ho").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Short; 5]);
    }

    #[tokio::test]
    async fn the_actions_that_always_pause_in_full_do() {
        let driver = page(false);
        let mut sb = demo_case(&driver).await;
        sb.go_back().await.unwrap();
        sb.go_forward().await.unwrap();
        sb.js_click("#a").await.unwrap();
        sb.jquery_click("#a").await.unwrap();
        sb.jquery_click_all("#a").await.unwrap();
        sb.set_value("#a", "v").await.unwrap();
        sb.set_text("#a", "t").await.unwrap();
        sb.focus("#a").await.unwrap();
        sb.jquery_update_text("#a", "t").await.unwrap();
        sb.submit("#a").await.unwrap();
        assert_eq!(sb.demo_pauses(), [PauseKind::Full; 10]);
    }

    #[tokio::test]
    async fn resizing_the_window_pauses_short_in_demo_mode_and_in_full_in_slow_mode() {
        let driver = page(false);
        let demo = demo_case(&driver).await;
        demo.set_window_size(500, 400).await.unwrap();
        demo.maximize_window().await.unwrap();
        demo.minimize_window().await.unwrap();
        demo.set_window_rect(1, 2, 300, 200).await.unwrap();
        demo.set_window_position(3, 4).await.unwrap();
        assert_eq!(demo.demo_pauses(), [PauseKind::Short; 5]);

        // Python's slow mode pauses for the whole time whatever the action.
        let slow = slow_case(&driver).await;
        slow.set_window_size(500, 400).await.unwrap();
        assert_eq!(slow.demo_pauses(), [PauseKind::Short]);
        assert_eq!(slow.run_settings().pause_for(PauseKind::Short), PAUSE);
    }

    #[tokio::test]
    async fn a_drag_outlines_both_the_source_and_the_target_in_demo_mode() {
        // Python highlights the drag selector and the drop selector.
        let driver = page(false);
        let mut sb = demo_case(&driver).await;
        sb.drag_and_drop("#src", "#dst").await.unwrap();
        let scripts: Vec<String> = driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .filter(|script| script.contains("3px solid magenta"))
            .collect();
        for selector in ["#src", "#dst"] {
            assert!(
                scripts.iter().any(|script| script.contains(selector)),
                "{selector} was not outlined: {scripts:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_pause_lasts_what_the_run_settings_say() {
        let driver = page(false);
        let mut sb = driver.base_case().await;
        sb.set_run_settings(
            RunSettings::default()
                .with_demo_mode(true)
                .with_demo_sleep(Duration::from_millis(340)),
        );
        let started = std::time::Instant::now();
        sb.click("#a").await.unwrap();
        // A click that stays on the page pauses for 1/3.4 of 340 ms.
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "{:?}",
            started.elapsed()
        );
        let started = std::time::Instant::now();
        sb.open("https://example.test/").await.unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(340),
            "{:?}",
            started.elapsed()
        );
    }
}

mod tests_messages {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    fn headed() -> BrowserConfig {
        BrowserConfig::default().with_headless(false)
    }

    fn last_script(driver: &MockWebDriver) -> String {
        driver.requests("/execute/sync").last().unwrap().body["script"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_message_stays_for_the_run_settings_duration() {
        let sb = BaseCase::without_session(headed());
        assert_eq!(sb.message_time(None), Duration::from_millis(2550));
        assert_eq!(
            sb.message_time(Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );

        let sb = BaseCase::without_session(headed().with_run_settings(
            RunSettings::default().with_message_duration(Duration::from_millis(1500)),
        ));
        assert_eq!(sb.message_time(None), Duration::from_millis(1500));
    }

    #[test]
    fn a_message_in_a_headless_browser_stays_at_most_three_quarters_of_a_second() {
        let sb = BaseCase::without_session(headed().with_headless(true));
        assert_eq!(sb.message_time(None), Duration::from_millis(750));
        assert_eq!(
            sb.message_time(Some(Duration::from_secs(5))),
            Duration::from_millis(750)
        );
        // A shorter time is kept.
        assert_eq!(
            sb.message_time(Some(Duration::from_millis(100))),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn nobody_watches_a_headless_browser_or_one_on_a_virtual_display() {
        let sb = BaseCase::without_session(headed().with_headless(true));
        assert!(sb.nobody_watches());

        let sb = BaseCase::without_session(headed().with_xvfb(true));
        assert!(sb.nobody_watches());

        // Headed, with no virtual display, somebody watches.
        let sb = BaseCase::without_session(headed());
        assert!(!sb.nobody_watches());
        let sb = BaseCase::without_session(headed().with_xvfb(true).with_xvfb(false));
        assert!(!sb.nobody_watches());
    }

    #[test]
    fn a_message_on_a_virtual_display_stays_at_most_three_quarters_of_a_second() {
        let sb = BaseCase::without_session(headed().with_xvfb(true));
        assert_eq!(sb.message_time(None), Duration::from_millis(750));
        assert_eq!(
            sb.message_time(Some(Duration::from_secs(5))),
            Duration::from_millis(750)
        );
        assert_eq!(
            sb.message_time(Some(Duration::from_millis(100))),
            Duration::from_millis(100)
        );
    }


    /// A case on a mock where Messenger takes every message at once: each
    /// script answers true.
    async fn messenger_case(config: BrowserConfig) -> (MockWebDriver, BaseCase) {
        let driver = MockWebDriver::start(json!({}), |request| match request.path.as_str() {
            "/execute/sync" => Reply::Value(json!(true)),
            _ => Reply::Value(Value::Null),
        });
        let mut config = config.with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        let sb = BaseCase::new(config).await.unwrap();
        (driver, sb)
    }

    /// The scripts that touched Messenger, oldest first.
    fn messenger_scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .filter(|script| script.contains("Messenger"))
            .collect()
    }

    #[tokio::test]
    async fn post_message_posts_with_messenger_for_the_duration_and_waits_for_it() {
        let (driver, sb) = messenger_case(headed().with_run_settings(
            RunSettings::default().with_message_duration(Duration::from_millis(120)),
        ))
        .await;

        let started = std::time::Instant::now();
        sb.post_message("Hello").await.unwrap();
        // The duration and 0.15 s more, as Python's `pause=True` waits.
        assert!(
            started.elapsed() >= Duration::from_millis(270),
            "{:?}",
            started.elapsed()
        );
        let script = last_script(&driver);
        assert!(script.contains("window.Messenger().post("), "{script}");
        assert!(script.contains("message: \"Hello\","), "{script}");
        assert!(script.contains("type: \"info\","), "{script}");
        assert!(script.contains("hideAfter: 0.12,"), "{script}");
    }

    #[tokio::test]
    async fn the_keywords_reach_the_message_and_the_wait() {
        let (driver, sb) = messenger_case(headed()).await;

        let started = std::time::Instant::now();
        sb.post_message("x")
            .style(MessageStyle::Error)
            .duration(Duration::from_millis(40))
            .await
            .unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(190),
            "{:?}",
            started.elapsed()
        );
        let script = last_script(&driver);
        assert!(script.contains("type: \"error\","), "{script}");
        assert!(script.contains("hideAfter: 0.04,"), "{script}");
    }

    #[tokio::test]
    async fn without_a_pause_the_call_returns_at_once() {
        let (_driver, sb) = messenger_case(headed().with_run_settings(
            RunSettings::default().with_message_duration(Duration::from_secs(30)),
        ))
        .await;
        let started = std::time::Instant::now();
        sb.post_message("x").pause(false).await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    }

    #[tokio::test]
    async fn a_message_where_nobody_watches_is_cut_to_three_quarters_of_a_second() {
        let (driver, sb) = messenger_case(headed().with_headless(true)).await;
        sb.post_message("x")
            .duration(Duration::from_secs(5))
            .pause(false)
            .await
            .unwrap();
        assert!(last_script(&driver).contains("hideAfter: 0.75,"));

        // A shorter time is kept.
        sb.post_message("x")
            .duration(Duration::from_millis(100))
            .pause(false)
            .await
            .unwrap();
        assert!(last_script(&driver).contains("hideAfter: 0.1,"));
    }

    #[tokio::test]
    async fn the_success_and_error_messages_fix_the_style() {
        let (driver, sb) = messenger_case(headed()).await;
        let quick = Duration::from_millis(1);

        sb.post_success_message("ok")
            .duration(quick)
            .pause(false)
            .await
            .unwrap();
        assert!(last_script(&driver).contains("type: \"success\","));

        sb.post_error_message("bad")
            .duration(quick)
            .pause(false)
            .await
            .unwrap();
        assert!(last_script(&driver).contains("type: \"error\","));

        sb.post_success_message("odd")
            .style(MessageStyle::Info)
            .duration(quick)
            .pause(false)
            .await
            .unwrap();
        assert!(last_script(&driver).contains("type: \"info\","));
    }

    #[tokio::test]
    async fn the_message_is_html_as_in_python() {
        let (driver, sb) = messenger_case(headed()).await;
        sb.post_message("<b>Saved</b> & \"more\"")
            .pause(false)
            .await
            .unwrap();
        let script = last_script(&driver);
        assert!(
            script.contains(r#"message: "<b>Saved</b> & \"more\"","#),
            "{script}"
        );
    }

    /// A page that has no Messenger until a script that loads it has run.
    fn page_that_loads_messenger_on_request() -> MockWebDriver {
        let loaded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        MockWebDriver::start(json!({}), move |request| {
            if request.path != "/execute/sync" {
                return Reply::Value(Value::Null);
            }
            let script = request.body["script"].as_str().unwrap_or_default();
            if script.contains("(async () =>") {
                loaded.store(true, std::sync::atomic::Ordering::SeqCst);
                return Reply::Value(json!(true));
            }
            if script.contains("hideOnNavigate") {
                return Reply::Value(json!(loaded.load(std::sync::atomic::Ordering::SeqCst)));
            }
            Reply::Value(json!(true))
        })
    }

    async fn case_on(driver: &MockWebDriver, config: BrowserConfig) -> BaseCase {
        let mut config = config.with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        BaseCase::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn a_page_without_messenger_gets_it_loaded_themed_and_the_message_posted_again() {
        let driver = page_that_loads_messenger_on_request();
        let sb = case_on(&driver, headed()).await;

        sb.post_message("Hello").pause(false).await.unwrap();

        let scripts = messenger_scripts(&driver);
        assert_eq!(scripts.len(), 4, "{scripts:?}");
        assert!(scripts[0].contains("hideOnNavigate"), "first it posts: {}", scripts[0]);
        assert!(
            scripts[1].contains("(async () =>") && scripts[1].contains("messenger-theme-air.min.css"),
            "then it loads Messenger and its themes: {}",
            scripts[1]
        );
        assert!(
            scripts[2].contains("\"theme\":\"future\"")
                && scripts[2].contains("messenger-on-bottom messenger-on-right"),
            "then it themes it as Python does: {}",
            scripts[2]
        );
        assert!(scripts[3].contains("hideOnNavigate"), "and posts again: {}", scripts[3]);
    }

    #[tokio::test]
    async fn a_mobile_emulator_gets_messenger_at_the_top_centre() {
        let driver = page_that_loads_messenger_on_request();
        let sb = case_on(&driver, headed().with_mobile(true)).await;

        sb.post_message("Hello").pause(false).await.unwrap();

        let scripts = messenger_scripts(&driver);
        assert_eq!(scripts.len(), 4, "{scripts:?}");
        assert!(
            scripts[2].contains("\"extraClasses\":\"messenger-fixed messenger-on-top\""),
            "{}",
            scripts[2]
        );
    }

    #[tokio::test]
    async fn a_page_that_cannot_load_messenger_still_pauses_and_the_call_succeeds() {
        // Every script fails, as on a page whose policy blocks the files.
        let driver = MockWebDriver::start(json!({}), |request| match request.path.as_str() {
            "/execute/sync" => Reply::Error(500, "javascript error"),
            _ => Reply::Value(Value::Null),
        });
        let sb = case_on(
            &driver,
            headed().with_run_settings(
                RunSettings::default().with_message_duration(Duration::from_millis(120)),
            ),
        )
        .await;

        let started = std::time::Instant::now();
        sb.post_message("Hello").await.unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(270),
            "{:?}",
            started.elapsed()
        );
        // It tried to post, then to load Messenger, and went no further.
        let scripts = messenger_scripts(&driver);
        assert_eq!(scripts.len(), 2, "{scripts:?}");
    }

    #[tokio::test]
    async fn a_message_with_a_highlight_does_not_wait_for_the_message_to_go() {
        let (driver, sb) = messenger_case(headed().with_run_settings(
            RunSettings::default().with_message_duration(Duration::from_secs(30)),
        ))
        .await;
        let started = std::time::Instant::now();
        sb.post_message_and_highlight("Saved", "h1").await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
        // The message went first, as a success message of the full time.
        let scripts = messenger_scripts(&driver);
        assert!(scripts.iter().any(|script| {
            script.contains("hideAfter: 30.0,") && script.contains("type: \"success\",")
        }), "{scripts:?}");
    }
}
