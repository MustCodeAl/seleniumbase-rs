// Tests of the `BaseCase` clicks and text input that go through DevTools
// (`cdp_click`, `cdp_insert_text`),
// against a mock WebDriver whose DevTools channel is scripted (no browser).
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_cdp_input {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use crate::browser::config::DriverMode;
    use serde_json::json;

    /// A page scrolled down 600 px with one button, 100 x 50, at (10, 20) in
    /// the viewport. WebDriver's element rectangle is in the document, so it
    /// says (10, 620): clicking there from a viewport click lands 600 px low.
    fn page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/goog/cdp/execute") => {
                    let expression = request.body["params"]["expression"]
                        .as_str()
                        .unwrap_or_default();
                    match request.body["cmd"].as_str().unwrap_or_default() {
                        "Runtime.evaluate" if expression.contains("getBoundingClientRect") => {
                            Reply::Value(json!({ "result": { "value": [10.0, 20.0, 100.0, 50.0] } }))
                        }
                        "Runtime.evaluate" if expression.starts_with("document.querySelector(") => {
                            Reply::Value(json!({
                                "result": { "type": "object", "subtype": "node", "objectId": "o1" }
                            }))
                        }
                        _ => Reply::Value(json!({})),
                    }
                }
                ("POST", "/element") => Reply::Value(element("button")),
                ("GET", "/element/button/rect") => Reply::Value(
                    json!({ "x": 10.0, "y": 620.0, "width": 100.0, "height": 50.0 }),
                ),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn case(driver: &MockWebDriver) -> BaseCase {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_mode(DriverMode::Cdp);
        config.auto_start_driver = false;
        BaseCase::new(config).await.expect("the mock accepts a session")
    }

    /// The parameters of each DevTools `command` that was sent.
    fn sent(driver: &MockWebDriver, command: &str) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .filter(|request| request.body["cmd"] == command)
            .map(|request| request.body["params"].clone())
            .collect()
    }

    fn clicks(driver: &MockWebDriver) -> Vec<(f64, f64)> {
        sent(driver, "Input.dispatchMouseEvent")
            .iter()
            .filter(|params| params["type"] == "mousePressed")
            .map(|params| (params["x"].as_f64().unwrap(), params["y"].as_f64().unwrap()))
            .collect()
    }

    #[tokio::test]
    async fn clicking_an_element_presses_where_it_is_in_the_viewport() {
        // `cdp_click` presses in the viewport. The rectangle that WebDriver
        // gives is in the document: 600 px below the button on a scrolled page.
        let driver = page();
        let mut sb = case(&driver).await;

        sb.cdp_click("#go").await.unwrap();

        assert_eq!(clicks(&driver), [(60.0, 45.0)]);
        assert!(
            driver.requests("/element/button/rect").is_empty(),
            "the document rectangle is not the place to click"
        );
    }

    #[tokio::test]
    async fn a_click_is_recorded_as_cdp_click() {
        let driver = page();
        let mut sb = case(&driver).await;
        sb.cdp_click("#go").await.unwrap();
        let recorded = sb.recorded_actions().unwrap();
        let names: Vec<_> = recorded
            .iter()
            .filter(|action| action.name.starts_with("cdp_click"))
            .map(|action| action.name.as_str())
            .collect();
        assert_eq!(names, ["cdp_click"], "{recorded:?}");
    }

    #[tokio::test]
    async fn inserted_text_goes_in_with_one_command_and_no_key_events() {
        let driver = page();
        let sb = case(&driver).await;

        sb.cdp_insert_text("hello").await.unwrap();

        let inserted: Vec<_> = sent(&driver, "Input.insertText")
            .iter()
            .map(|params| params["text"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(inserted, ["hello"]);
        assert!(sent(&driver, "Input.dispatchKeyEvent").is_empty());
    }
}
