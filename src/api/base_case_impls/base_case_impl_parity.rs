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

/// What [`BaseCase::load_cookies_with`] does with the expiry of the cookies it
/// adds. Python's `expiry` argument of `load_cookies`, as a type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CookieExpiry {
    /// Drop the expiry, so the cookies last for the browser session (Python's
    /// `expiry=False`). The default.
    #[default]
    Session,
    /// Expire the cookies 24 hours from now (Python's `expiry=True`).
    Day,
    /// Keep the expiry the cookies were saved with (Python's `expiry=-1`).
    Saved,
    /// Expire the cookies this many minutes from now (Python's
    /// `expiry=minutes`). Zero drops the expiry, like [`Session`](Self::Session).
    Minutes(u32),
}

/// `<base>/saved_cookies/<file name>`: the file Python's cookie helpers use
/// for the cookies called `name`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not usable; see
/// [`crate::artifacts::saved_cookies_file_name`].
fn saved_cookies_path(base: &Path, name: &str) -> Result<PathBuf, SeleniumBaseError> {
    Ok(base
        .join(crate::artifacts::SAVED_COOKIES_FOLDER)
        .join(crate::artifacts::saved_cookies_file_name(name)?))
}

/// The JSON in the cookie file `path`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if the file cannot be read or
/// is not JSON.
fn read_saved_cookies(path: &Path) -> Result<Value, SeleniumBaseError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!(
            "Failed to read the cookies in {}: {e}",
            path.display()
        ))
    })?;
    serde_json::from_str(text.trim()).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!(
            "{} does not hold cookies as JSON: {e}",
            path.display()
        ))
    })
}

/// The host of `origin` (`scheme://host[:port]`) without its port: a cookie's
/// domain names a host, and a browser driver refuses one that carries a port.
fn host_of_origin(origin: &str) -> &str {
    let authority = origin.rsplit("://").next().unwrap_or(origin);
    match authority.strip_prefix('[') {
        // `[::1]:8080` is the address `::1` in brackets, then the port.
        Some(_) => authority.split_inclusive(']').next().unwrap_or(authority),
        None => authority.split(':').next().unwrap_or(authority),
    }
}

/// Makes a saved cookie fit the page `origin` and `expiry`, as Python's
/// `load_cookies` does: a domain that is not part of the origin becomes the
/// origin's host, and the expiry is dropped, kept or set from `now` (seconds
/// since the epoch).
fn prepare_cookie(
    cookie: &mut serde_json::Map<String, Value>,
    origin: &str,
    expiry: CookieExpiry,
    now: i64,
) {
    let foreign = cookie
        .get("domain")
        .and_then(Value::as_str)
        .is_some_and(|domain| !origin.contains(domain));
    if foreign {
        cookie.insert("domain".to_owned(), Value::from(host_of_origin(origin)));
    }
    match expiry {
        CookieExpiry::Session | CookieExpiry::Minutes(0) => {
            cookie.remove("expiry");
        }
        CookieExpiry::Day => {
            cookie.insert("expiry".to_owned(), Value::from(now + 86_400));
        }
        CookieExpiry::Saved => {}
        CookieExpiry::Minutes(minutes) => {
            cookie.insert(
                "expiry".to_owned(),
                Value::from(now + i64::from(minutes) * 60),
            );
        }
    }
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
        self.check_time_limit()?;
        let by = Selector::auto(css).to_by()?;
        let timeout = self.default_wait_secs();
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

    /// Shows `message` in the page and waits for it to go. Corresponds to
    /// Python's `post_message(message)`: it stays for the run's message
    /// duration, 2.55 s unless `RunSettings::with_message_duration` says
    /// otherwise, cut to 0.75 s in a headless browser; the call returns 0.15 s
    /// after it has gone (Python's default `pause=True`). For another time, see
    /// [`post_message_for`](Self::post_message_for).
    ///
    /// # Errors
    ///
    /// Returns an error if the page script fails.
    pub async fn post_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.show_message(message, INFO_MESSAGE_COLOR, None, true)
            .await
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

    /// Makes the browser this case started with the active one, however many
    /// others it has opened since and however often this is called. The
    /// browser that was active stays open. Corresponds to Python's
    /// `switch_to_default_driver`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the default browser has
    /// been quit.
    pub async fn switch_to_default_driver(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_driver(DriverId::DEFAULT).await
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

    /// Reads the cookies saved in `saved_cookies/<name>.txt` (the file
    /// [`save_cookies`](Self::save_cookies) wrote) without adding them to the
    /// browser. Corresponds to Python's `get_saved_cookies(name)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not a usable
    /// file name or the file is missing or is not JSON.
    pub fn get_saved_cookies(&self, name: &str) -> Result<serde_json::Value, SeleniumBaseError> {
        read_saved_cookies(&saved_cookies_path(&std::env::current_dir()?, name)?)
    }

    /// Deletes `saved_cookies/<name>.txt`, if it is there. Corresponds to
    /// Python's `delete_saved_cookies(name)`; it does not delete the cookies
    /// from the browser.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not a usable
    /// file name, and the I/O error if the file exists but cannot be removed.
    pub async fn delete_saved_cookies(&self, name: &str) -> Result<(), SeleniumBaseError> {
        let path = saved_cookies_path(&std::env::current_dir()?, name)?;
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
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
        std::fs::write(&path, "  [{\"name\":\"sid\",\"value\":\"abc\"}]\n").unwrap();

        let cookies = read_saved_cookies(&path).unwrap();

        assert_eq!(cookies[0]["name"], "sid");
        assert_eq!(cookies[0]["value"], "abc");
    }

    #[test]
    fn a_missing_or_malformed_cookie_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.txt");
        assert!(matches!(
            read_saved_cookies(&missing),
            Err(SeleniumBaseError::InvalidConfig(m)) if m.contains("missing.txt")
        ));

        let bad = dir.path().join("bad.txt");
        std::fs::write(&bad, "not json").unwrap();
        assert!(matches!(
            read_saved_cookies(&bad),
            Err(SeleniumBaseError::InvalidConfig(m)) if m.contains("bad.txt")
        ));
    }

    #[test]
    fn cookie_files_live_in_saved_cookies_under_the_last_part_of_the_name() {
        // Python's rule: keep the last path part and add `.txt` if missing.
        let base = Path::new("work");
        let file = |name: &str| saved_cookies_path(base, name).unwrap();
        let expected = base.join("saved_cookies").join("passwd.txt");
        assert_eq!(file("../../etc/passwd"), expected);
        assert_eq!(file("passwd.txt"), expected);
        assert_eq!(file(r"C:\\evil\\passwd"), expected);
        assert_eq!(
            file("cookies"),
            base.join("saved_cookies").join("cookies.txt")
        );
        for bad in ["", "a/", r"a\\", "..", "."] {
            assert!(
                matches!(
                    saved_cookies_path(base, bad),
                    Err(SeleniumBaseError::InvalidConfig(_))
                ),
                "{bad:?}"
            );
        }
    }

    fn cookie() -> serde_json::Map<String, Value> {
        let Value::Object(fields) = serde_json::json!({
            "name": "sid", "value": "abc", "domain": ".example.test",
            "path": "/", "expiry": 4_102_444_800_i64
        }) else {
            unreachable!("a JSON object");
        };
        fields
    }

    #[test]
    fn a_cookie_of_another_domain_gets_the_domain_of_the_page() {
        let mut foreign = cookie();
        prepare_cookie(
            &mut foreign,
            "https://www.other.test:8443",
            CookieExpiry::Saved,
            0,
        );
        // The port is not part of a cookie's domain.
        assert_eq!(foreign["domain"], "www.other.test");

        // A domain the origin contains is left alone.
        let mut same = cookie();
        prepare_cookie(&mut same, "https://www.example.test", CookieExpiry::Saved, 0);
        assert_eq!(same["domain"], ".example.test");

        // A cookie with no domain gets none.
        let mut bare = cookie();
        bare.remove("domain");
        prepare_cookie(&mut bare, "https://www.other.test", CookieExpiry::Saved, 0);
        assert!(!bare.contains_key("domain"));
    }

    #[test]
    fn the_domain_of_an_origin_has_no_port() {
        assert_eq!(host_of_origin("https://www.example.test"), "www.example.test");
        assert_eq!(host_of_origin("http://localhost:8000"), "localhost");
        assert_eq!(host_of_origin("http://127.0.0.1:9"), "127.0.0.1");
        assert_eq!(host_of_origin("http://[::1]:8080"), "[::1]");
        assert_eq!(host_of_origin("example.test"), "example.test");
    }

    #[test]
    fn the_expiry_is_dropped_kept_or_set_as_python_does() {
        let origin = "https://www.example.test";
        let after = |expiry| {
            let mut fields = cookie();
            prepare_cookie(&mut fields, origin, expiry, 1_000);
            fields.get("expiry").cloned()
        };
        assert_eq!(after(CookieExpiry::default()), None, "dropped by default");
        assert_eq!(after(CookieExpiry::Session), None);
        assert_eq!(after(CookieExpiry::Minutes(0)), None);
        assert_eq!(after(CookieExpiry::Saved), Some(Value::from(4_102_444_800_i64)));
        assert_eq!(after(CookieExpiry::Day), Some(Value::from(1_000 + 86_400)));
        assert_eq!(
            after(CookieExpiry::Minutes(5)),
            Some(Value::from(1_000 + 300))
        );

        // A cookie with no expiry gets one only when asked to.
        let mut session = cookie();
        session.remove("expiry");
        prepare_cookie(&mut session, origin, CookieExpiry::Day, 1_000);
        assert_eq!(session["expiry"], 1_000 + 86_400);
    }
}

#[cfg(test)]
mod tests_cookies {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A page at `https://www.example.test/` holding one cookie of another
    /// domain, which expires in 2100.
    fn page_with_a_cookie() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/cookie") => Reply::Value(json!([{
                    "name": "sid", "value": "abc", "domain": ".other.test", "path": "/",
                    "secure": false, "httpOnly": true, "expiry": 4_102_444_800_i64,
                    "sameSite": "Lax"
                }])),
                ("GET", "/url") => Reply::Value(json!("https://www.example.test/home")),
                // `document.readyState`, which loading cookies waits for.
                ("POST", "/execute/sync") => Reply::Value(json!("complete")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The cookies the page was asked to add, oldest first.
    fn added(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/cookie")
            .into_iter()
            .filter(|request| request.method == "POST")
            .map(|request| request.body["cookie"].clone())
            .collect()
    }

    #[tokio::test]
    async fn saved_cookies_are_loaded_the_way_python_loads_them() {
        let driver = page_with_a_cookie();
        let sb = driver.base_case().await;
        let dir = tempfile::tempdir().unwrap();
        // The folder of the file is made.
        let file = dir.path().join("saved").join("cookies.txt");

        sb.save_cookies_to_path(&file).await.unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(saved[0]["name"], "sid");
        assert_eq!(saved[0]["expiry"], 4_102_444_800_i64);

        // By default the domain becomes the page's and the expiry goes. Like
        // Python's `load_cookies`, it waits for the page to finish loading.
        sb.load_cookies_from_path(&file, CookieExpiry::default())
            .await
            .unwrap();
        let asked_ready_state = driver.requests("/execute/sync").iter().any(|request| {
            request.body["script"]
                .as_str()
                .is_some_and(|script| script.contains("document.readyState"))
        });
        assert!(asked_ready_state, "the page was not asked if it had loaded");
        let cookie = &added(&driver)[0];
        assert_eq!(cookie["name"], "sid");
        assert_eq!(cookie["domain"], "www.example.test");
        assert_eq!(cookie["httpOnly"], true);
        assert!(cookie.get("expiry").is_none(), "{cookie}");

        // The saved expiry can be kept.
        sb.load_cookies_from_path(&file, CookieExpiry::Saved)
            .await
            .unwrap();
        assert_eq!(added(&driver)[1]["expiry"], 4_102_444_800_i64);
    }

    #[tokio::test]
    async fn a_file_that_is_not_a_list_of_cookies_is_refused() {
        let driver = page_with_a_cookie();
        let sb = driver.base_case().await;
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in [
            ("object.txt", r#"{"name": "sid"}"#),
            ("scalar.txt", "[1]"),
            ("nameless.txt", r#"[{"value": "abc"}]"#),
        ] {
            let file = dir.path().join(name);
            std::fs::write(&file, text).unwrap();
            let error = sb
                .load_cookies_from_path(&file, CookieExpiry::Saved)
                .await
                .expect_err(name);
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(ref m) if m.contains(name)),
                "{error:?}"
            );
        }
        assert!(added(&driver).is_empty(), "nothing was added");
    }
}
