// Tests of the clicks at an offset from an element, against the mock
// WebDriver in `webdriver_support` (no browser needed). Included (for tests
// only) at the end of `base_case.rs` like the files beside it.

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
