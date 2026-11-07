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
                } else if expression.contains("window.location.href") {
                    value(json!("https://example.com/app"))

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

    /// Two open tabs, `T1` and `T2`, as `Target.getTargets` lists them.
    fn two_tabs(mock: &MockCtrl) {
        mock.reply(
            "Target.getTargets",
            json!({ "targetInfos": [
                { "targetId": "T1", "type": "page", "url": "about:blank", "title": "" },
                { "targetId": "T2", "type": "page", "url": "about:blank", "title": "" },
            ]}),
        );
    }

    #[tokio::test]
    async fn window_handles_are_the_target_ids_of_the_tabs() {
        let (sb, mock) = case().await;
        two_tabs(&mock);

        assert_eq!(sb.get_all_window_handles().await.unwrap(), ["T1", "T2"]);
        assert_eq!(sb.get_current_window_handle().await.unwrap(), "T1");
    }

    #[tokio::test]
    async fn switching_to_a_window_makes_its_tab_the_one_the_case_drives() {
        let (sb, mock) = case().await;
        two_tabs(&mock);

        sb.switch_to_window("T2").await.unwrap();

        assert_eq!(sb.get_current_window_handle().await.unwrap(), "T2");
        assert!(
            !mock.calls_to("Page.bringToFront").is_empty(),
            "the tab is brought to the front"
        );
    }

    #[tokio::test]
    async fn a_new_tab_is_created_by_the_browser_and_switched_to() {
        let (mut sb, mock) = case().await;
        two_tabs(&mock);

        sb.open_new_tab().await.unwrap();

        let created = mock.calls_to("Target.createTarget");
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].params["newWindow"], json!(false));
        assert_eq!(sb.get_current_window_handle().await.unwrap(), "T2");
    }

    #[tokio::test]
    async fn a_new_tab_that_is_not_switched_to_leaves_the_case_where_it_was() {
        let (mut sb, mock) = case().await;
        two_tabs(&mock);

        sb.open_new_tab().switch_to(false).await.unwrap();

        assert_eq!(mock.calls_to("Target.createTarget").len(), 1);
        assert_eq!(sb.get_current_window_handle().await.unwrap(), "T1");
    }

    #[tokio::test]
    async fn the_window_is_measured_and_resized_through_the_browser() {
        let (sb, mock) = case().await;
        mock.reply(
            "Browser.getWindowForTarget",
            json!({ "windowId": 7, "bounds": {
                "left": 10, "top": 20, "width": 800, "height": 600, "windowState": "normal",
            }}),
        );

        assert_eq!(sb.get_window_rect().await.unwrap(), (10, 20, 800, 600));
        assert_eq!(sb.get_window_position().await.unwrap(), (10, 20));
        sb.set_window_size(1024, 768).await.unwrap();
        sb.maximize().await.unwrap();

        let set = mock.calls_to("Browser.setWindowBounds");
        let bounds: Vec<_> = set.iter().map(|call| call.params["bounds"].clone()).collect();
        assert!(set.iter().all(|call| call.params["windowId"] == json!(7)));
        assert!(
            bounds.contains(&json!({ "left": 0, "top": 0, "width": 1024, "height": 768 })),
            "{bounds:?}"
        );
        assert!(
            bounds.contains(&json!({ "windowState": "maximized" })),
            "{bounds:?}"
        );
    }

    #[tokio::test]
    async fn closing_the_window_closes_its_tab() {
        let (mut sb, mock) = case().await;

        sb.close_window().await.unwrap();

        assert_eq!(mock.calls_to("Target.closeTarget").len(), 1);
    }


    /// One cookie of the page, as `Network.getCookies` reports it.
    fn one_cookie(mock: &MockCtrl) {
        mock.reply(
            "Network.getCookies",
            json!({ "cookies": [{
                "name": "sid", "value": "abc", "domain": "example.com", "path": "/",
                "httpOnly": true, "secure": false, "expires": -1,
            }]}),
        );
    }

    #[tokio::test]
    async fn the_cookies_of_the_page_are_asked_for_by_its_address() {
        let (sb, mock) = case().await;
        Page::one().script(&mock);
        one_cookie(&mock);

        let cookies = sb.get_cookies().await.unwrap();

        assert_eq!(cookies.len(), 1);
        assert_eq!((cookies[0].name.as_str(), cookies[0].value.as_str()), ("sid", "abc"));
        assert!(cookies[0].http_only);
        assert_eq!(cookies[0].expires, None, "a session cookie has no expiry");
        let asked = mock.calls_to("Network.getCookies");
        assert_eq!(asked[0].params["urls"], json!(["https://example.com/app"]));
        assert_eq!(sb.get_cookie("sid").await.unwrap().unwrap().value, "abc");
        assert!(sb.get_cookie("other").await.unwrap().is_none());
        assert_eq!(sb.get_cookie_string().await.unwrap(), "sid=abc");
    }

    #[tokio::test]
    async fn a_cookie_without_a_domain_is_added_for_the_address_of_the_page() {
        let (sb, mock) = case().await;
        Page::one().script(&mock);
        mock.reply("Network.setCookie", json!({ "success": true }));

        sb.add_cookie(&Cookie::new("theme", "dark")).await.unwrap();

        let sent = mock.calls_to("Network.setCookie");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].params["name"], json!("theme"));
        assert_eq!(sent[0].params["url"], json!("https://example.com/app"));
    }

    #[tokio::test]
    async fn a_cookie_the_browser_refuses_is_an_error() {
        let (sb, mock) = case().await;
        Page::one().script(&mock);
        mock.reply("Network.setCookie", json!({ "success": false }));

        let error = sb
            .add_cookie(&Cookie::new("theme", "dark"))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("theme"), "{error}");
    }

    #[tokio::test]
    async fn cookies_are_deleted_by_name_and_all_of_the_page_by_what_the_page_has() {
        let (sb, mock) = case().await;
        Page::one().script(&mock);
        one_cookie(&mock);

        sb.delete_cookie("sid").await.unwrap();
        sb.clear_all_cookies().await.unwrap();
        sb.clear_browser_cookies().await.unwrap();

        let deleted: Vec<_> = mock
            .calls_to("Network.deleteCookies")
            .into_iter()
            .map(|call| call.params)
            .collect();
        assert_eq!(
            deleted,
            [
                json!({ "name": "sid", "url": "https://example.com/app" }),
                json!({ "name": "sid", "domain": "example.com", "path": "/" }),
            ]
        );
        assert_eq!(mock.calls_to("Network.clearBrowserCookies").len(), 1);
    }

    #[tokio::test]
    async fn saved_cookies_are_the_w3c_objects_python_writes() {
        let (sb, mock) = case().await;
        Page::one().script(&mock);
        one_cookie(&mock);
        let path = std::env::temp_dir().join(format!("sbrs-pure-cdp-cookies-{}.json", std::process::id()));

        sb.save_cookies_to_path(&path).await.unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(saved[0]["name"], json!("sid"));
        assert_eq!(saved[0]["value"], json!("abc"));
        assert_eq!(saved[0]["httpOnly"], json!(true));
        assert_eq!(saved[0]["domain"], json!("example.com"));
    }

    #[tokio::test]
    async fn the_devtools_helpers_go_to_the_page() {
        let (sb, mock) = case().await;

        sb.set_timezone("Europe/Paris").await.unwrap();
        sb.set_geolocation(48.85, 2.35, 10.0).await.unwrap();
        sb.clear_browser_cache().await.unwrap();
        sb.cdp_mouse_click(5.0, 6.0).await.unwrap();
        sb.cdp_insert_text("hi").await.unwrap();

        assert_eq!(mock.calls_to("Emulation.setTimezoneOverride").len(), 1);
        assert_eq!(mock.calls_to("Emulation.setGeolocationOverride").len(), 1);
        assert_eq!(mock.calls_to("Network.clearBrowserCache").len(), 1);
        assert_eq!(mouse(&mock).len(), 3, "move, press, release");
        assert_eq!(mock.calls_to("Input.insertText")[0].params["text"], json!("hi"));
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
