// Helpers that complete the BaseCase surface of the Python framework.

/// What an action needs from its element before it can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ready {
    /// The element exists in the page.
    Present,
    /// The element exists and is displayed.
    Visible,
    /// The element is displayed and enabled.
    Clickable,
}

impl std::fmt::Display for Ready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Present => "present",
            Self::Visible => "visible",
            Self::Clickable => "clickable",
        })
    }
}

impl BaseCase {
    /// Waits, polling, until `css` is ready for an action, for up to the
    /// configured timeout ([`set_timeout`](Self::set_timeout), 10 seconds by
    /// default).
    ///
    /// Actions call this first so they do not race a page that is still
    /// rendering. A WebDriver-wide implicit wait is deliberately not used: it
    /// would also make every "is it absent?" check wait the full time.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the selector and the
    /// state it never reached.
    async fn await_ready(&self, css: &str, ready: Ready) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let timeout = self.timeout_secs;
        let reached = match ready {
            Ready::Present => self.session.wait_for_element(by, timeout).await.map(drop),
            Ready::Visible => self.session.wait_for_element_visible(by, timeout).await.map(drop),
            Ready::Clickable => self.session.wait_for_element_clickable(by, timeout).await.map(drop),
        };
        reached.map_err(|_| {
            SeleniumBaseError::wait_timeout(
                format!("'{css}' to be {ready}"),
                Some(Duration::from_secs(timeout)),
            )
        })
    }

    /// Replaces an input's content with `text` in one step, with no per-key
    /// events. A trailing newline presses Enter. Corresponds to Python's
    /// `fast_type`; use [`type_text`](Self::type_text) where the page listens
    /// for key presses.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if the element is missing.
    pub async fn fast_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let (body, enter) = match text.strip_suffix('\n') {
            Some(body) => (body, true),
            None => (text, false),
        };
        self.set_value(css, body).await?;
        if enter {
            self.send_keys(css, "\n").await?;
        }
        Ok(())
    }

    /// Clicks the element with a script, only if it is visible. Does nothing
    /// otherwise. Corresponds to Python's `js_click_if_visible`.
    ///
    /// # Errors
    ///
    /// Returns an error if the page script fails.
    pub async fn js_click_if_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_visible(css).await? {
            self.js_click(css).await?;
        }
        Ok(())
    }

    /// The element's rectangle in screen coordinates, as a desktop automation
    /// tool such as `enigo` needs it. Corresponds to Python's
    /// `get_gui_element_rect`.
    ///
    /// The browser's toolbar height is estimated from the window's outer and
    /// inner sizes, so the result is exact for a normal window and approximate
    /// when the toolbar and a docked panel are both open.
    ///
    /// # Errors
    ///
    /// Returns an error if the element is missing or the page script fails.
    pub async fn get_gui_element_rect(
        &mut self,
        css: &str,
    ) -> Result<crate::sb_cdp::Rect, SeleniumBaseError> {
        let element = self.find_element(css).await?;
        let rect = element.rect().await?;
        let metrics = self
            .execute_script(&format!("return {};", crate::utils::geometry::WINDOW_METRICS_SCRIPT))
            .await?;
        let metrics: [f64; 6] = serde_json::from_value(metrics)?;
        Ok(crate::utils::geometry::screen_rect(
            crate::sb_cdp::Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            },
            metrics,
        ))
    }

    /// The centre of the element in screen coordinates. Corresponds to
    /// Python's `get_gui_element_center`; see
    /// [`get_gui_element_rect`](Self::get_gui_element_rect).
    ///
    /// # Errors
    ///
    /// See [`get_gui_element_rect`](Self::get_gui_element_rect).
    pub async fn get_gui_element_center(
        &mut self,
        css: &str,
    ) -> Result<crate::sb_cdp::Point, SeleniumBaseError> {
        let rect = self.get_gui_element_rect(css).await?;
        Ok(crate::sb_cdp::Point {
            x: rect.x + rect.width / 2.0,
            y: rect.y + rect.height / 2.0,
        })
    }

    /// Escapes `code` so it can sit inside a single-quoted JavaScript string.
    /// Corresponds to Python's `jq_format`.
    #[must_use]
    pub fn jq_format(code: &str) -> String {
        js_escape(code)
    }

    /// Shows `message` in the page for a few seconds. Corresponds to Python's
    /// `post_message` with its default duration.
    ///
    /// # Errors
    ///
    /// Returns an error if the page script fails.
    pub async fn post_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.post_message_for(message, 3).await
    }

    /// Saves the page's HTML as `name` in the logs folder and returns its
    /// path. Only the last path part of `name` is used. Corresponds to
    /// Python's `save_as_html_to_logs`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub async fn save_as_html_to_logs(&self, name: &str) -> Result<PathBuf, SeleniumBaseError> {
        let file = Path::new(name).file_name().ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!("{name:?} is not a usable file name"))
        })?;
        let path = ensure_latest_logs_dir()?.join(file);
        self.save_page_source_to_path(&path).await?;
        Ok(path)
    }

    /// Saves a screenshot to the logs folder for a failed test's report.
    /// Corresponds to Python's `save_teardown_screenshot`.
    ///
    /// # Errors
    ///
    /// Returns an error if the screenshot cannot be taken or written.
    pub async fn save_teardown_screenshot(&self) -> Result<PathBuf, SeleniumBaseError> {
        let path = artifact_path(&ensure_latest_logs_dir()?, "teardown_screenshot", "png");
        self.save_screenshot_to_path(&path).await?;
        Ok(path)
    }

    /// Switches back to the first browser opened by this test. Corresponds to
    /// Python's `switch_to_default_driver`.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver cannot be switched.
    pub async fn switch_to_default_driver(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_driver(0).await
    }

    /// Waits until the page has no AngularJS requests in flight. Returns at
    /// once on a page that does not use AngularJS. Corresponds to Python's
    /// `wait_for_angularjs`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if requests are still
    /// pending after `timeout_secs`.
    pub async fn wait_for_angularjs(&self, timeout_secs: u64) -> Result<(), SeleniumBaseError> {
        const SCRIPT: &str = "if (typeof angular === 'undefined') return true; \
            const injector = angular.element(document.body).injector(); \
            return !injector || injector.get('$http').pendingRequests.length === 0;";
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if self.execute_script(SCRIPT).await?.as_bool().unwrap_or(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::wait_timeout(
                    "AngularJS requests to finish",
                    Some(Duration::from_secs(timeout_secs)),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Reads a cookie file written by [`save_cookies`](Self::save_cookies)
    /// without loading it into the browser. Corresponds to Python's
    /// `get_saved_cookies`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is missing or is not valid JSON.
    pub fn get_saved_cookies(file_path: &str) -> Result<serde_json::Value, SeleniumBaseError> {
        let text = std::fs::read_to_string(file_path)?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;


    #[test]
    fn a_wait_names_the_state_in_the_timeout_message() {
        assert_eq!(Ready::Present.to_string(), "present");
        assert_eq!(Ready::Visible.to_string(), "visible");
        assert_eq!(Ready::Clickable.to_string(), "clickable");
        let message = SeleniumBaseError::wait_timeout(
            format!("'#go' to be {}", Ready::Clickable),
            Some(Duration::from_secs(10)),
        )
        .to_string();
        assert!(message.contains("#go") && message.contains("clickable"), "{message}");
    }

    #[test]
    fn jq_format_escapes_what_would_end_a_quoted_script_string() {
        assert_eq!(BaseCase::jq_format("it's\na\\b"), "it\\'s\\na\\\\b");
    }

    #[test]
    fn saved_cookies_are_read_back_without_a_browser() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.txt");
        std::fs::write(&path, r#"[{"name":"sid","value":"abc"}]"#).unwrap();

        let cookies = BaseCase::get_saved_cookies(path.to_str().unwrap()).unwrap();

        assert_eq!(cookies[0]["name"], "sid");
        assert_eq!(cookies[0]["value"], "abc");
    }

    #[test]
    fn a_missing_or_malformed_cookie_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.txt");
        assert!(BaseCase::get_saved_cookies(missing.to_str().unwrap()).is_err());

        let bad = dir.path().join("bad.txt");
        std::fs::write(&bad, "not json").unwrap();
        assert!(BaseCase::get_saved_cookies(bad.to_str().unwrap()).is_err());
    }

}
