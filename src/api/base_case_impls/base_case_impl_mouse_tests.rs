// Tests of the clicks that depend on where or whether the element is (at an
// offset, only if visible), and of `get_locale_code`, against the mock
// WebDriver in `webdriver_support` (no browser needed). Included (for tests
// only) at the end of `base_case.rs` like the files beside it.

mod tests_conditional_click {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;
    use std::time::Instant;

    /// A page with one element, `#a`, that is displayed once `shown_after` has
    /// passed since the page was made.
    fn page(shown_after: Duration) -> MockWebDriver {
        let born = Instant::now();
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("GET", "/element/a/displayed") => {
                    Reply::Value(json!(born.elapsed() >= shown_after))
                }
                ("GET", "/element/a/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn click_if_visible_looks_once_like_pythons_default() {
        let driver = page(Duration::from_millis(300));
        let mut sb = driver.base_case().await;
        sb.click_if_visible("#a").await.unwrap();
        assert!(driver.requests("/element/a/click").is_empty());

        // Later, when it is displayed, the same call clicks.
        tokio::time::sleep(Duration::from_millis(350)).await;
        sb.click_if_visible("#a").await.unwrap();
        assert_eq!(driver.requests("/element/a/click").len(), 1);
    }

    #[tokio::test]
    async fn click_if_visible_with_a_timeout_waits_for_the_element() {
        let driver = page(Duration::from_millis(150));
        let mut sb = driver.base_case().await;

        sb.click_if_visible_with_timeout("#a", Duration::from_secs(3))
            .await
            .unwrap();

        assert_eq!(driver.requests("/element/a/click").len(), 1);
    }

    #[tokio::test]
    async fn click_if_visible_with_a_timeout_gives_up_without_an_error() {
        let driver = page(Duration::from_secs(3600));
        let mut sb = driver.base_case().await;

        let started = Instant::now();
        sb.click_if_visible_with_timeout("#a", Duration::from_millis(120))
            .await
            .unwrap();

        assert!(started.elapsed() >= Duration::from_millis(120));
        assert!(driver.requests("/element/a/click").is_empty());
    }

    #[tokio::test]
    async fn the_locale_is_what_the_browser_reports_and_nothing_made_up() {
        // A browser with no locale at all answers `undefined`, which is `null`.
        let driver = MockWebDriver::start(json!({}), |_| Reply::Value(Value::Null));
        let sb = driver.base_case().await;
        assert_eq!(sb.get_locale_code().await.unwrap(), "");
        let script = driver.requests("/execute/sync").last().unwrap().body["script"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            script.contains("navigator.languages[0]") && !script.contains("en-US"),
            "{script}"
        );

        let driver = MockWebDriver::start(json!({}), |_| Reply::Value(json!("de-DE")));
        let sb = driver.base_case().await;
        assert_eq!(sb.get_locale_code().await.unwrap(), "de-DE");
    }
}

mod tests_messenger_theme {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use crate::sb_cdp::{MessengerLocation, MessengerSettings, MessengerTheme};
    use serde_json::json;

    /// A page whose scripts all succeed, whether Messenger is loaded or not,
    /// and that says whether it has Messenger when asked to apply a theme.
    fn page(has_messenger: bool) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| match request.path.as_str() {
            "/execute/async" => Reply::Value(json!(true)),
            "/execute/sync" => Reply::Value(json!(has_messenger)),
            _ => Reply::Value(Value::Null),
        })
    }

    fn last_script(driver: &MockWebDriver) -> String {
        driver.requests("/execute/sync").last().unwrap().body["script"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    async fn the_theme_the_location_and_the_limit_reach_messenger() {
        let driver = page(true);
        let sb = driver.base_case().await;

        sb.set_messenger_theme(MessengerSettings {
            theme: MessengerTheme::Ice,
            location: MessengerLocation::TopLeft,
            max_messages: 3,
        })
        .await
        .unwrap();

        let script = last_script(&driver);
        assert!(script.starts_with("return "), "{script}");
        assert!(script.contains("\"theme\":\"ice\""), "{script}");
        assert!(script.contains("messenger-on-top"), "{script}");
        assert!(script.contains("messenger-on-left"), "{script}");
        assert!(script.contains("\"maxMessages\":3"), "{script}");
        // Messenger is loaded first (jQuery, the style sheet, the script).
        assert!(driver.requests("/execute/async").len() >= 3);
    }

    #[tokio::test]
    async fn a_theme_or_a_location_alone_keeps_pythons_defaults_for_the_rest() {
        let driver = page(true);
        let sb = driver.base_case().await;

        sb.set_messenger_theme(MessengerTheme::Air).await.unwrap();
        let script = last_script(&driver);
        assert!(script.contains("\"theme\":\"air\""), "{script}");
        assert!(script.contains("messenger-on-bottom"), "{script}");
        assert!(script.contains("\"maxMessages\":8"), "{script}");

        sb.set_messenger_theme(MessengerLocation::TopCenter)
            .await
            .unwrap();
        let script = last_script(&driver);
        assert!(script.contains("\"theme\":\"flat\""), "{script}");
        assert!(script.contains("messenger-on-top"), "{script}");
    }

    #[tokio::test]
    async fn a_page_that_still_lacks_messenger_is_an_error() {
        let driver = page(false);
        let sb = driver.base_case().await;
        let error = sb
            .set_messenger_theme(MessengerSettings::default())
            .await
            .unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
    }

    #[tokio::test]
    async fn the_colour_setter_is_what_set_messenger_theme_used_to_be() {
        let driver = page(true);
        let sb = driver.base_case().await;

        sb.set_messenger_color("#336699").await.unwrap();
        assert!(
            last_script(&driver).contains(".messenger-message { background: #336699 !important; }"),
            "{}",
            last_script(&driver)
        );
        assert!(matches!(
            sb.set_messenger_color("red; } body { display: none").await,
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }
}

mod tests_offset_clicks {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A page with one element, `#a`, `width` pixels wide and 40 high.
    fn page(width: f64) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("GET", "/element/a/displayed" | "/element/a/enabled") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/element/a/rect") => Reply::Value(
                    json!({ "x": 0.0, "y": 0.0, "width": width, "height": 40.0 }),
                ),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// Each pointer action of `kind` in the actions sent, anywhere in them.
    fn pointer_actions(driver: &MockWebDriver, kind: &str) -> Vec<Value> {
        fn collect(value: &Value, kind: &str, found: &mut Vec<Value>) {
            match value {
                Value::Object(map) => {
                    if map.get("type").and_then(Value::as_str) == Some(kind) {
                        found.push(value.clone());
                    }
                    map.values().for_each(|inner| collect(inner, kind, found));
                }
                Value::Array(items) => items.iter().for_each(|inner| collect(inner, kind, found)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        for request in driver.requests("/actions") {
            collect(&request.body, kind, &mut found);
        }
        found
    }

    /// Where the pointer was moved to, relative to the element.
    fn moved_to(driver: &MockWebDriver) -> Vec<(i64, i64)> {
        pointer_actions(driver, "pointerMove")
            .iter()
            .map(|action| {
                (
                    action["x"].as_f64().unwrap() as i64,
                    action["y"].as_f64().unwrap() as i64,
                )
            })
            .collect()
    }

    #[test]
    fn an_offset_from_the_top_left_corner_becomes_one_from_the_centre() {
        // 100 x 40: the top-left corner is (-50, -20) from the centre.
        assert_eq!(offset_from_center((0, 0), (100.0, 40.0), false), (-50, -20));
        assert_eq!(offset_from_center((10, 5), (100.0, 40.0), false), (-40, -15));
        // The driver truncates, so half a pixel goes toward zero, as Python's does.
        assert_eq!(offset_from_center((10, 5), (101.0, 41.0), false), (-40, -15));
        assert_eq!(offset_from_center((0, 0), (101.0, 41.0), false), (-50, -20));
        // From the centre, nothing changes.
        assert_eq!(offset_from_center((10, -5), (100.0, 40.0), true), (10, -5));
    }

    #[tokio::test]
    async fn a_click_counts_from_the_top_left_corner_unless_it_says_centre() {
        let driver = page(100.0);
        let mut sb = driver.base_case().await;

        sb.click_with_offset("#a", 10, 5, false).await.unwrap();
        sb.click_with_offset("#a", 10, 5, true).await.unwrap();

        assert_eq!(moved_to(&driver), [(-40, -15), (10, 5)]);
        assert_eq!(pointer_actions(&driver, "pointerDown").len(), 2);
    }

    #[tokio::test]
    async fn a_double_click_counts_the_same_way_and_presses_twice() {
        let driver = page(100.0);
        let mut sb = driver.base_case().await;

        sb.double_click_with_offset("#a", 10, 5, false)
            .await
            .unwrap();

        assert_eq!(moved_to(&driver), [(-40, -15)]);
        assert!(pointer_actions(&driver, "pointerDown").len() >= 2);
    }

    #[tokio::test]
    async fn the_recording_says_where_the_offset_counts_from() {
        let driver = page(100.0);
        let mut sb = driver.base_case().await;
        sb.click_with_offset("#a", 10, -5, false).await.unwrap();
        sb.double_click_with_offset("#a", 1, 2, true).await.unwrap();

        let recorded = sb.recorded_actions().unwrap();
        let offsets: Vec<_> = recorded
            .iter()
            .filter(|action| action.name.ends_with("click_with_offset"))
            .map(|action| (action.name.as_str(), action.value.as_deref()))
            .collect();
        assert_eq!(
            offsets,
            [
                ("click_with_offset", Some("10, -5")),
                ("double_click_with_offset", Some("1, 2, center")),
            ]
        );
        // ... and the generated program passes it on.
        let mut recorder = ActionRecorder::default();
        for action in &recorded {
            recorder.record(&action.name, action.target.as_deref(), action.value.as_deref());
        }
        let script = recorder.to_rust_script();
        assert!(
            script.contains("sb.click_with_offset(\"#a\", 10, -5, false).await?;"),
            "{script}"
        );
        assert!(
            script.contains("sb.double_click_with_offset(\"#a\", 1, 2, true).await?;"),
            "{script}"
        );
    }
}
