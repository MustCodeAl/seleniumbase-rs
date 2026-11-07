// Tests of `BaseCase` on a direct DevTools attachment (the pure CDP engine, no
// WebDriver), against a scripted browser (`Browser::new_mocked`): what each
// group of methods sends, and how the answers become results and errors.
// See `docs/design/basecase_over_cdp.md`.
// Included (for tests only) at the end of `base_case.rs` like the files beside
// it.

mod tests_pure_cdp {
    use super::*;
    use crate::sb_cdp::{Browser, MockCtrl};
    use serde_json::json;

    /// Wraps a script result the way `Runtime.evaluate` returns it.
    fn value(v: Value) -> Value {
        json!({ "result": { "value": v } })
    }

    /// A case on a mocked tab with a short wait for the failure paths.
    async fn case() -> (BaseCase, MockCtrl) {
        let (browser, mock) = Browser::new_mocked();
        let page = browser
            .default_page()
            .await
            .expect("the mock has a tab")
            .with_timeout(Duration::from_millis(200));
        let session = BrowserSession::attach_page(page);
        (BaseCase::with_session(BrowserConfig::default(), session), mock)
    }

    /// What the page helper answers for a page that has `matches` elements for
    /// every selector, `visible` or not, whose text is `text` and whose
    /// attributes read `attribute`.
    struct Page {
        matches: usize,
        visible: bool,
        text: &'static str,
        attribute: &'static str,
        tag: &'static str,
    }

    impl Page {
        fn one() -> Self {
            Self {
                matches: 1,
                visible: true,
                text: "Hello",
                attribute: "attribute-value",
                tag: "div",
            }
        }

        fn script(self, mock: &MockCtrl) {
            mock.on("Runtime.evaluate", move |params| {
                let expression = params["expression"].as_str().unwrap_or_default();
                Ok(if expression.contains("__sbcdp.center(") {
                    value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
                } else if expression.contains("__sbcdp.fieldText(") {
                    value(json!(self.text))
                } else if expression.contains("/* getAttribute */") {
                    value(json!(self.attribute))
                } else if expression.contains("el.tagName.toLowerCase()") {
                    value(json!(self.tag))
                } else if expression.contains("el.innerText") {
                    value(json!(self.text))
                } else if expression.contains(".some(__sbcdp.visible)") {
                    value(json!(self.visible && self.matches > 0))
                } else if expression.contains("resolve(") && expression.contains(".length > 0") {
                    value(json!(self.matches > 0))
                } else if expression.contains("resolve(") && expression.contains(".length === 0") {
                    value(json!(self.matches == 0))
                } else if expression.contains("resolve(") && expression.contains(".length") {
                    value(json!(self.matches))
                } else if expression.contains("return !el.disabled;") {
                    value(json!(true))
                } else {
                    value(Value::Null)
                })
            });
        }
    }

    /// The mouse events sent, as `(type, x, y)`.
    fn mouse(mock: &MockCtrl) -> Vec<(String, f64, f64)> {
        mock.calls_to("Input.dispatchMouseEvent")
            .into_iter()
            .map(|call| {
                (
                    call.params["type"].as_str().unwrap_or_default().to_owned(),
                    call.params["x"].as_f64().unwrap_or_default(),
                    call.params["y"].as_f64().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn find_element_hands_back_an_element_of_the_page() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        let element = sb.find_element("#go").await.unwrap();

        assert_eq!(element.to_string(), "#go");
        assert!(element.as_webdriver_element().is_none());
        assert!(element.as_locator().is_some());
        assert_eq!(element.text().await.unwrap(), "Hello");
    }

    #[tokio::test]
    async fn find_element_for_a_selector_that_matches_nothing_times_out_naming_it() {
            let (mut sb, mock) = case().await;
            Page {
                matches: 0,
                ..Page::one()
            }
            .script(&mock);
            sb.set_timeout(1).await.unwrap();
    
            let error = sb.find_element("#nowhere").await.unwrap_err();
    
            assert!(
                matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("#nowhere")),
                "{error:?}"
            );
        }
    
        #[tokio::test]
        async fn a_session_find_of_a_selector_that_matches_nothing_is_not_found() {
            let (sb, mock) = case().await;
            Page {
                matches: 0,
                ..Page::one()
            }
            .script(&mock);
    
            let error = sb.session.find("#nowhere").await.unwrap_err();
    
            assert!(
                matches!(&error, SeleniumBaseError::ElementNotFound { selector, .. } if selector == "#nowhere"),
                "{error:?}"
            );
        }

    #[tokio::test]
    async fn find_elements_pins_each_match_to_its_position() {
        let (mut sb, mock) = case().await;
        Page {
            matches: 3,
            ..Page::one()
        }
        .script(&mock);

        let elements = sb.find_elements("li").await.unwrap();

        assert_eq!(elements.len(), 3);
        let chains: Vec<_> = elements
            .iter()
            .map(|element| element.as_locator().unwrap().to_string())
            .collect();
        assert_eq!(chains, ["li [nth=0]", "li [nth=1]", "li [nth=2]"]);
    }

    #[tokio::test]
    async fn a_click_presses_at_the_centre_of_the_element() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.click("#go").await.unwrap();

        assert_eq!(
            mouse(&mock),
            [
                ("mouseMoved".to_owned(), 120.0, 80.0),
                ("mousePressed".to_owned(), 120.0, 80.0),
                ("mouseReleased".to_owned(), 120.0, 80.0),
            ]
        );
    }

    #[tokio::test]
    async fn text_and_attributes_are_read_through_the_page() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        assert_eq!(sb.get_text("#go").await.unwrap(), "Hello");
        assert_eq!(
            sb.get_attribute("#go", "href").await.unwrap().as_deref(),
            Some("attribute-value")
        );
        assert!(sb.is_element_visible("#go").await.unwrap());
        sb.assert_text("Hello", "#go").await.unwrap();
    }

    #[tokio::test]
    async fn a_hidden_element_is_not_visible_and_not_clickable() {
        let (mut sb, mock) = case().await;
        Page {
            visible: false,
            ..Page::one()
        }
        .script(&mock);

        assert!(!sb.is_element_visible("#go").await.unwrap());
        let error = sb.wait_for_element_clickable("#go", 1).await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
        assert!(mouse(&mock).is_empty());
    }

    #[tokio::test]
    async fn typing_clears_the_field_and_sends_key_events() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.type_text("#name", "ab").await.unwrap();

        let cleared = mock
            .calls_to("Runtime.evaluate")
            .into_iter()
            .any(|call| {
                call.params["expression"]
                    .as_str()
                    .is_some_and(|expression| expression.contains("__sbcdp.setValue(el, '')"))
            });
        assert!(cleared, "the field is emptied first");
        let keys = mock.calls_to("Input.dispatchKeyEvent");
        assert!(keys.len() >= 2, "a key event per character: {keys:?}");
    }

    #[tokio::test]
    async fn set_value_runs_the_value_script_with_the_element_and_the_value() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.set_value("#slider", "42").await.unwrap();

        let expressions: Vec<String> = mock
            .calls_to("Runtime.evaluate")
            .into_iter()
            .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
            .collect();
        assert!(
            expressions.iter().any(|expression| {
                expression.contains("arguments[0].value = arguments[1]")
                    && expression.contains("[el, ...[\"42\"]]")
            }),
            "{expressions:#?}"
        );
    }

    #[tokio::test]
    async fn a_frame_scopes_the_searches_until_the_session_leaves_it() {
        let (mut sb, mock) = case().await;
        Page {
            tag: "iframe",
            ..Page::one()
        }
        .script(&mock);

        sb.switch_to_frame("#frame").await.unwrap();
        sb.find_element("#inner").await.unwrap();
        let inside = last_chain(&mock);
        sb.switch_to_default_content().await.unwrap();
        sb.find_element("#inner").await.unwrap();
        let outside = last_chain(&mock);

        assert!(inside.contains("#frame") && inside.contains("#inner"), "{inside}");
        assert!(!outside.contains("#frame") && outside.contains("#inner"), "{outside}");
    }

    #[tokio::test]
    async fn something_that_is_not_a_frame_cannot_be_switched_into() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        let error = sb.switch_to_frame("#go").await.unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::InvalidSelector(message) if message.contains("<div>")),
            "{error:?}"
        );
    }

    /// The expression of the last `Runtime.evaluate` that searched with a chain.
    fn last_chain(mock: &MockCtrl) -> String {
        mock.calls_to("Runtime.evaluate")
            .into_iter()
            .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
            .rfind(|expression| expression.contains("__sbcdp.resolve("))
            .unwrap_or_default()
    }
}
