// ESC ends the test (Python's `--esc-end`, `--esc_end`, `--ee`): the key
// listener a page gets and the places that look whether ESC was pressed.
// Included in `base_case.rs` like the files beside it.

/// The page script that listens for ESC.
///
/// Python's script assigns `document.onkeydown`, which replaces any handler the
/// page has. This one adds a capturing listener once per document
/// (`sb_esc_armed`), so arming again after every click is harmless and the
/// page's own key handlers keep working. Either way, ESC sets
/// `document.sb_esc_end` to `"yes"`. The script has no caller-supplied text, so
/// nothing in it needs quoting.
const ESC_END_ARM_SCRIPT: &str = r#"
if (!document.sb_esc_armed) {
    document.sb_esc_armed = true;
    window.addEventListener("keydown", function (evt) {
        evt = evt || window.event;
        var isEscape = false;
        if ("key" in evt) {
            isEscape = (evt.key === "Escape" || evt.key === "Esc");
        } else {
            isEscape = (evt.keyCode === 27);
        }
        if (isEscape) {
            document.sb_esc_end = "yes";
        }
    }, true);
}
"#;

/// The page script that reads what the listener saw.
const ESC_END_CHECK_SCRIPT: &str = "return document.sb_esc_end;";

impl BaseCase {
    /// Whether pressing ESC in the browser ends the test
    /// ([`BrowserConfig::with_esc_end`]).
    #[must_use]
    pub fn esc_end(&self) -> bool {
        self.config.esc_end()
    }

    /// Puts the ESC listener in the page that is loaded now. Does nothing
    /// unless [`esc_end`](Self::esc_end) is on.
    ///
    /// A page that cannot run the script (no page yet, an alert that is open)
    /// is left alone: the listener is a convenience and must not fail a test.
    /// A new document loses the listener, so this runs again after the calls
    /// that load one.
    async fn arm_esc_end(&self) {
        if !self.esc_end() {
            return;
        }
        if let Err(error) = self.execute_script(ESC_END_ARM_SCRIPT).await {
            debug!(%error, "could not install the ESC listener");
        }
    }

    /// Ends the test if ESC was pressed since the listener was installed:
    /// returns [`SeleniumBaseError::Skipped`]. Does nothing unless
    /// [`esc_end`](Self::esc_end) is on.
    ///
    /// A script that fails is read as "not pressed", so a closed page or an
    /// open alert never turns this check into the reason a test fails.
    async fn skip_if_esc(&self) -> Result<(), SeleniumBaseError> {
        if !self.esc_end() {
            return Ok(());
        }
        let pressed = self
            .execute_script(ESC_END_CHECK_SCRIPT)
            .await
            .is_ok_and(|seen| seen.as_str() == Some("yes"));
        if pressed {
            return Err(SeleniumBaseError::Skipped(
                "ESC was pressed in the browser (--esc-end)".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_esc_end {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A browser page in which ESC has been pressed once `pressed` is set.
    fn page(pressed: &Arc<AtomicBool>) -> MockWebDriver {
        let pressed = Arc::clone(pressed);
        MockWebDriver::start(json!({}), move |request| {
            let script = request.body["script"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") if script.contains("return document.sb_esc_end") => {
                    if pressed.load(Ordering::SeqCst) {
                        Reply::Value(json!("yes"))
                    } else {
                        Reply::Value(Value::Null)
                    }
                }
                ("POST", "/execute/sync") if script.contains("readyState") => {
                    Reply::Value(json!("complete"))
                }
                ("POST", "/element") => Reply::Value(element("e1")),
                ("GET", "/element/e1/displayed" | "/element/e1/enabled") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn pressed(on: bool) -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(on))
    }

    async fn case(driver: &MockWebDriver, esc_end: bool) -> BaseCase {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_esc_end(esc_end);
        config.auto_start_driver = false;
        BaseCase::new(config)
            .await
            .expect("the mock WebDriver accepts a session")
    }

    /// Every script the page was asked to run, oldest first.
    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .collect()
    }

    fn arms_the_listener(script: &str) -> bool {
        script.contains("addEventListener(\"keydown\"")
    }

    fn is_skip(result: Result<(), SeleniumBaseError>) -> bool {
        matches!(result, Err(SeleniumBaseError::Skipped(message)) if message.contains("ESC"))
    }

    #[tokio::test]
    async fn nothing_is_injected_or_checked_unless_it_is_asked_for() {
        // The page says ESC was pressed, but the setting is off.
        let driver = page(&pressed(true));
        let mut sb = case(&driver, false).await;
        assert!(!sb.esc_end());

        sb.open("https://example.test/").await.unwrap();
        sb.wait_for_ready_state_complete().await.unwrap();
        sb.click("#a").await.unwrap();
        sb.type_text("#a", "x").await.unwrap();
        sb.highlight("#a").await.unwrap();
        sb.wait_for_element_visible("#a", 1).await.unwrap();
        sb.click_link_text("Home").await.unwrap();

        let all = scripts(&driver);
        assert!(!all.is_empty(), "the other calls ran scripts");
        assert!(
            all.iter().all(|script| !script.contains("sb_esc_")),
            "{all:?}"
        );
    }

    #[tokio::test]
    async fn the_listener_sets_the_flag_that_the_checks_read() {
        let driver = page(&pressed(false));
        let sb = case(&driver, true).await;
        assert!(sb.esc_end());

        sb.wait_for_ready_state_complete().await.unwrap();

        let last = scripts(&driver).pop().expect("a script was sent");
        assert!(arms_the_listener(&last), "{last}");
        assert!(last.contains("evt.key === \"Escape\""), "{last}");
        assert!(last.contains("evt.keyCode === 27"), "{last}");
        assert!(last.contains("document.sb_esc_end = \"yes\""), "{last}");
        assert!(
            last.contains("if (!document.sb_esc_armed)"),
            "arming twice must not add a second listener: {last}"
        );
    }

    #[tokio::test]
    async fn a_page_that_was_just_opened_gets_the_listener() {
        let driver = page(&pressed(false));
        let mut sb = case(&driver, true).await;

        sb.open("https://example.test/").await.unwrap();

        assert!(scripts(&driver).iter().any(|script| arms_the_listener(script)));
    }

    #[tokio::test]
    async fn esc_ends_the_test_at_the_next_wait_for_the_page() {
        let escape = pressed(false);
        let driver = page(&escape);
        let sb = case(&driver, true).await;
        sb.wait_for_ready_state_complete().await.unwrap();

        escape.store(true, Ordering::SeqCst);
        let result = sb.wait_for_ready_state_complete().await;

        assert!(is_skip(result));
        let polls = scripts(&driver)
            .iter()
            .filter(|script| script.contains("readyState"))
            .count();
        assert_eq!(polls, 1, "the second call stopped before it waited");
    }

    #[tokio::test]
    async fn esc_stops_a_click_before_it_clicks() {
        let driver = page(&pressed(true));
        let mut sb = case(&driver, true).await;

        assert!(is_skip(sb.click("#a").await));

        assert!(driver.requests("/element/e1/click").is_empty());
    }

    #[tokio::test]
    async fn esc_stops_typing_before_it_types() {
        let driver = page(&pressed(true));
        let mut sb = case(&driver, true).await;

        assert!(is_skip(sb.type_text("#a", "hello").await));

        assert!(driver.requests("/element/e1/value").is_empty());
    }

    #[tokio::test]
    async fn esc_stops_the_other_places_python_checks() {
        let driver = page(&pressed(true));
        let mut sb = case(&driver, true).await;

        assert!(is_skip(sb.highlight("#a").await));
        assert!(is_skip(sb.wait_for_element_visible("#a", 1).await));
        assert!(is_skip(sb.click_link_text("Home").await));
        assert!(driver.requests("/element").is_empty(), "no element was looked up");
    }

    #[tokio::test]
    async fn a_click_that_ran_puts_the_listener_back_for_the_page_it_loaded() {
        let driver = page(&pressed(false));
        let mut sb = case(&driver, true).await;

        sb.click("#a").await.unwrap();

        let requests = driver.all_requests();
        let click = requests
            .iter()
            .position(|request| request.path == "/element/e1/click")
            .expect("the element was clicked");
        let armed_after = requests[click..].iter().any(|request| {
            request.body["script"]
                .as_str()
                .is_some_and(arms_the_listener)
        });
        assert!(armed_after, "{requests:?}");
    }

    #[tokio::test]
    async fn a_script_that_fails_is_not_an_escape() {
        // An open alert, or a page that is gone, makes every script fail.
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Error(500, "javascript error"),
                ("POST", "/element") => Reply::Value(element("e1")),
                ("GET", "/element/e1/displayed" | "/element/e1/enabled") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = case(&driver, true).await;

        sb.click("#a").await.expect("the click is not blocked");

        assert_eq!(driver.requests("/element/e1/click").len(), 1);
    }
}
