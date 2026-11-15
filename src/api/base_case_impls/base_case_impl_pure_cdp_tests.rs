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
                } else if expression.contains("x: r.left") {
                    // `Locator::viewport_rect`: a 40 by 20 box whose centre is (120, 60).
                    value(json!({ "x": 100.0, "y": 50.0, "width": 40.0, "height": 20.0 }))

                } else if expression.contains("__sbcdp.fieldText(") {
                    value(json!(self.text))
                } else if expression.contains("/* getAttribute */") {
                    value(json!(self.attribute))
                } else if expression.contains("document.title") {
                    value(json!("Welcome page"))

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

        assert_eq!(sb.text("#go").await.unwrap(), "Hello");
        assert_eq!(
            sb.attribute("#go", "href").await.unwrap().as_deref(),
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

        assert_eq!(sb.all_window_handles().await.unwrap(), ["T1", "T2"]);
        assert_eq!(sb.current_window_handle().await.unwrap(), "T1");
    }

    #[tokio::test]
    async fn switching_to_a_window_makes_its_tab_the_one_the_case_drives() {
        let (sb, mock) = case().await;
        two_tabs(&mock);

        sb.switch_to_window("T2").await.unwrap();

        assert_eq!(sb.current_window_handle().await.unwrap(), "T2");
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
        assert_eq!(sb.current_window_handle().await.unwrap(), "T2");
    }

    #[tokio::test]
    async fn a_new_tab_that_is_not_switched_to_leaves_the_case_where_it_was() {
        let (mut sb, mock) = case().await;
        two_tabs(&mock);

        sb.open_new_tab().switch_to(false).await.unwrap();

        assert_eq!(mock.calls_to("Target.createTarget").len(), 1);
        assert_eq!(sb.current_window_handle().await.unwrap(), "T1");
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

        assert_eq!(sb.window_rect().await.unwrap(), (10, 20, 800, 600));
        assert_eq!(sb.window_position().await.unwrap(), (10, 20));
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

        let cookies = sb.cookies().await.unwrap();

        assert_eq!(cookies.len(), 1);
        assert_eq!((cookies[0].name.as_str(), cookies[0].value.as_str()), ("sid", "abc"));
        assert!(cookies[0].http_only);
        assert_eq!(cookies[0].expires, None, "a session cookie has no expiry");
        let asked = mock.calls_to("Network.getCookies");
        assert_eq!(asked[0].params["urls"], json!(["https://example.com/app"]));
        assert_eq!(sb.cookie("sid").await.unwrap().unwrap().value, "abc");
        assert!(sb.cookie("other").await.unwrap().is_none());
        assert_eq!(sb.cookie_string().await.unwrap(), "sid=abc");
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


    /// A dialog the page opened, as `Page.javascriptDialogOpening` reports it.
    fn open_dialog(mock: &MockCtrl, kind: &str, message: &str) {
        mock.emit(
            "Page.javascriptDialogOpening",
            json!({
                "url": "https://example.com/app", "message": message, "type": kind,
                "hasBrowserHandler": false, "defaultPrompt": "initial",
            }),
            Some("S1"),
        );
    }

    #[tokio::test]
    async fn an_alert_the_page_opened_is_read_and_answered() {
        let (sb, mock) = case().await;
        open_dialog(&mock, "confirm", "Sure?");

        let alert = sb.switch_to_alert().await.unwrap();
        assert_eq!(alert.text().await.unwrap(), "Sure?");
        assert!(sb.is_alert_present().await.unwrap());
        alert.accept().await.unwrap();

        let handled = mock.calls_to("Page.handleJavaScriptDialog");
        assert_eq!(handled.len(), 1);
        assert_eq!(handled[0].params, json!({ "accept": true }));
        assert!(!sb.is_alert_present().await.unwrap());
    }

    #[tokio::test]
    async fn a_prompt_takes_the_typed_text_and_a_dismissed_alert_is_cancelled() {
        let (sb, mock) = case().await;
        open_dialog(&mock, "prompt", "Name?");
        let prompt = sb.switch_to_alert().await.unwrap();
        prompt.send_keys("Ada").await.unwrap();
        prompt.accept().await.unwrap();

        open_dialog(&mock, "alert", "Bye");
        sb.switch_to_alert().await.unwrap().dismiss().await.unwrap();

        let handled: Vec<_> = mock
            .calls_to("Page.handleJavaScriptDialog")
            .into_iter()
            .map(|call| call.params)
            .collect();
        assert_eq!(
            handled,
            [
                json!({ "accept": true, "promptText": "Ada" }),
                json!({ "accept": false }),
            ]
        );
    }

    #[tokio::test]
    async fn an_alert_command_without_an_alert_fails_as_no_such_alert() {
        let (sb, mock) = case().await;

        let error = sb.accept_alert().await.unwrap_err();

        assert!(error.to_string().contains("no such alert"), "{error}");
        assert!(mock.calls_to("Page.handleJavaScriptDialog").is_empty());
        assert!(!sb.is_alert_present().await.unwrap());
    }

    #[tokio::test]
    async fn an_action_is_refused_while_a_dialog_is_open() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);
        open_dialog(&mock, "alert", "Hold on");
        sb.switch_to_alert().await.unwrap();

        let error = sb.click("#go").await.unwrap_err();

        assert!(error.to_string().contains("unexpected alert open"), "{error}");
        assert!(mouse(&mock).is_empty());
    }

    #[tokio::test]
    async fn a_screenshot_is_the_png_the_browser_captured() {
        let (sb, mock) = case().await;
        mock.reply("Page.captureScreenshot", json!({ "data": "iVBORw0KGgo=" }));

        let png = sb.screenshot_as_png().await.unwrap();

        assert_eq!(png, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[tokio::test]
    async fn scripts_run_in_the_frame_the_case_is_in() {
        let (mut sb, mock) = case().await;
        Page {
            tag: "iframe",
            ..Page::one()
        }
        .script(&mock);

        sb.execute_script("return 1;").await.unwrap();
        let top = last_expression(&mock);
        sb.switch_to_frame("#frame").await.unwrap();
        sb.execute_script("return 2;").await.unwrap();
        let inside = last_expression(&mock);

        assert!(top.starts_with("(function() {\nreturn 1;"), "{top}");
        assert!(!top.contains("#frame"), "{top}");
        assert!(inside.contains("#frame") && inside.contains("return 2;"), "{inside}");
    }

    #[tokio::test]
    async fn an_async_script_gets_its_callback_as_the_last_argument() {
        let (sb, mock) = case().await;

        sb.execute_async_script("arguments[arguments.length - 1](7);")
            .await
            .unwrap();

        let sent = last_expression(&mock);
        assert!(sent.starts_with("new Promise("), "{sent}");
        assert!(sent.contains(".call(window, resolve)"), "{sent}");
    }

    #[tokio::test]
    async fn a_named_key_is_a_real_key_event_and_links_are_found_by_their_text() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.press_key("enter").await.unwrap();
        sb.click_link("Docs").await.unwrap();

        let keys: Vec<_> = mock
            .calls_to("Input.dispatchKeyEvent")
            .into_iter()
            .filter_map(|call| call.params["key"].as_str().map(str::to_owned))
            .collect();
        assert!(keys.iter().any(|key| key == "Enter"), "{keys:?}");
        assert!(
            mock.calls_to("Runtime.evaluate").into_iter().any(|call| {
                call.params["expression"]
                    .as_str()
                    .is_some_and(|expression| expression.contains("link_text"))
            }),
            "the link is looked up by its text"
        );
        assert!(!mouse(&mock).is_empty(), "the link is clicked");
        sb.wait_for_title("Welcome", 1).await.unwrap();
    }

    #[tokio::test]
    async fn an_action_that_opens_a_dialog_is_let_go_instead_of_waited_for() {
        let (browser, mock) = Browser::new_mocked();
        let page = browser.default_page().await.unwrap();
        let watch = crate::browser::session::DialogWatch::start(&page);
        let emitter = mock.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            open_dialog(&emitter, "alert", "Hi");
        });

        // A command that stalls for ever, as a click does that opens an alert.
        let outcome = watch
            .unless_dialog(std::future::pending::<Result<(), SeleniumBaseError>>())
            .await;

        assert!(matches!(outcome, Ok(None)), "{outcome:?}");
        assert_eq!(watch.current().unwrap().message, "Hi");
    }


    /// The mouse events sent, as `(type, button, click count)`.
    fn mouse_buttons(mock: &MockCtrl) -> Vec<(String, String, u64)> {
        mock.calls_to("Input.dispatchMouseEvent")
            .into_iter()
            .map(|call| {
                (
                    call.params["type"].as_str().unwrap_or_default().to_owned(),
                    call.params["button"].as_str().unwrap_or_default().to_owned(),
                    call.params["clickCount"].as_u64().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn hover_double_click_and_context_click_are_real_mouse_events() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.hover("#go").await.unwrap();
        assert_eq!(mouse(&mock), [("mouseMoved".to_owned(), 120.0, 80.0)]);

        sb.double_click("#go").await.unwrap();
        let double = mouse_buttons(&mock).split_off(1);
        let expected = [
            ("mouseMoved", "none", 0),
            ("mousePressed", "left", 1),
            ("mouseReleased", "left", 1),
            ("mousePressed", "left", 2),
            ("mouseReleased", "left", 2),
        ];
        assert_eq!(
            double,
            expected.map(|(kind, button, count)| (kind.to_owned(), button.to_owned(), count))
        );

        sb.context_click("#go").await.unwrap();
        let right = mouse_buttons(&mock).split_off(6);
        assert!(
            right
                .iter()
                .filter(|(kind, _, _)| kind == "mousePressed")
                .all(|(_, button, _)| button == "right"),
            "{right:?}"
        );
        assert_eq!(right.len(), 3, "{right:?}");
    }

    #[tokio::test]
    async fn a_click_at_an_offset_lands_that_far_from_the_centre_of_the_element() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.click_with_offset("#go", 10, 5).center(true).await.unwrap();
        sb.double_click_with_offset("#go", -20, 0)
            .center(true)
            .await
            .unwrap();

        // The element is 40 by 20 pixels and its centre is (120, 60).
        let presses: Vec<_> = mouse(&mock)
            .into_iter()
            .filter(|(kind, _, _)| kind == "mousePressed")
            .map(|(_, x, y)| (x, y))
            .collect();
        assert_eq!(presses, [(130.0, 65.0), (100.0, 60.0), (100.0, 60.0)]);
    }

    #[tokio::test]
    async fn a_drag_by_an_offset_holds_the_button_from_the_centre_to_the_target() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.drag_and_drop_with_offset("#go", 30, -10).await.unwrap();

        let events = mouse(&mock);
        assert_eq!(events.first(), Some(&("mouseMoved".to_owned(), 120.0, 60.0)));
        assert_eq!(events.get(1), Some(&("mousePressed".to_owned(), 120.0, 60.0)));
        assert_eq!(events.last(), Some(&("mouseReleased".to_owned(), 150.0, 50.0)));
        assert!(events.len() > 3, "the pointer travels between the two: {events:?}");
    }

    #[tokio::test]
    async fn a_click_chain_clicks_each_element_in_turn() {
        let (mut sb, mock) = case().await;
        Page::one().script(&mock);

        sb.click_chain(&["#first", "#second"]).await.unwrap();

        let pressed = mouse(&mock)
            .into_iter()
            .filter(|(kind, _, _)| kind == "mousePressed")
            .count();
        assert_eq!(pressed, 2);
    }

    #[tokio::test]
    async fn a_method_that_needs_a_driver_refuses_by_name_and_names_the_alternative() {
        let (sb, _mock) = case().await;

        let version = sb.chromedriver_version().unwrap_err();
        let events = sb.session.cdp_events().await.unwrap_err();
        let addon = sb.install_addon("dark-mode.xpi", true).await.unwrap_err();

        for (error, method, alternative) in [
            (version, "chromedriver_version", "browser_version"),
            (events, "cdp_events", "Page::events"),
            (addon, "install_addon", "with_extension_dir"),
        ] {
            assert!(
                matches!(error, SeleniumBaseError::Unsupported(_)),
                "{method}: {error:?}"
            );
            let text = error.to_string();
            assert!(
                text.contains(method) && text.contains(alternative),
                "{method}: {text}"
            );
        }
    }


    /// The expression of the last `Runtime.evaluate`.
    fn last_expression(mock: &MockCtrl) -> String {
        mock.calls_to("Runtime.evaluate")
            .into_iter()
            .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
            .next_back()
            .unwrap_or_default()
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
