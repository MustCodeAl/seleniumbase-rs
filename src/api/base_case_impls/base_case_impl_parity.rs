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
fn prepare_cookie(cookie: &mut Cookie, origin: &str, expiry: CookieExpiry, now: i64) {
    if !cookie.domain.is_empty() && !origin.contains(cookie.domain.as_str()) {
        cookie.domain = host_of_origin(origin).to_owned();
    }
    cookie.expires = match expiry {
        CookieExpiry::Session | CookieExpiry::Minutes(0) => None,
        CookieExpiry::Day => Some((now + 86_400) as f64),
        CookieExpiry::Saved => cookie.expires,
        CookieExpiry::Minutes(minutes) => Some((now + i64::from(minutes) * 60) as f64),
    };
}

/// The cookie in one entry of a saved cookie file: the W3C cookie object that Python
/// SeleniumBase writes.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] naming `path` if the entry is not an object
/// with a string `name` and `value`, or holds a field that is not a cookie's.
fn saved_cookie(entry: Value, path: &Path) -> Result<Cookie, SeleniumBaseError> {
    if !entry.is_object() {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "{} holds a cookie that is not an object",
            path.display()
        )));
    }
    serde_json::from_value(entry).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!(
            "{} holds a cookie that cannot be used: {e}",
            path.display()
        ))
    })
}

/// The time now, in whole seconds since the Unix epoch.
fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
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
    async fn await_ready(&self, css: &Selector, ready: Ready) -> Result<(), SeleniumBaseError> { self.await_ready_within(css, ready, self.default_wait_secs())
        .await }
    
    /// [`await_ready`](Self::await_ready) for up to `timeout` seconds, for the
    /// calls that take a timeout of their own.
    async fn await_ready_within(&self, css: &Selector, ready: Ready,
    timeout: u64,) -> Result<(), SeleniumBaseError> { self.check_time_limit()?;
    let selector = shadow_find::checked(css)?;
    let reached = match ready {
        Ready::Present => self.session.wait_for_element(selector, timeout).await.map(drop),
        Ready::Visible => self.session.wait_for_element_visible(selector, timeout).await.map(drop),
        Ready::Clickable => self.session.wait_for_element_clickable(selector, timeout).await.map(drop),
    };
    reached.map_err(|_| {
        SeleniumBaseError::wait_timeout(
            format!("'{css}' to be {ready}"),
            Some(Duration::from_secs(timeout)),
        )
    }) }

    /// Replaces an input's content with `text` in one step, with no per-key
    /// events. A trailing newline presses Enter. Corresponds to Python's
    /// `fast_type`; use [`type_text`](Self::type_text) where the page listens
    /// for key presses.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if the element is missing.
    pub async fn fast_type(&mut self, selector: impl Into<Selector>, text: &str) -> Result<(), SeleniumBaseError> {
        let css: &Selector = &selector.into();
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
    pub async fn js_click_if_visible(&mut self, selector: impl Into<Selector>) -> Result<(), SeleniumBaseError> {
        let css: &Selector = &selector.into();
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
    pub async fn gui_element_rect(&mut self, selector: impl Into<Selector>) -> Result<crate::sb_cdp::Rect, SeleniumBaseError> {
        let css: &Selector = &selector.into();
        let element = self.find_element(css).await?;
        let rect = element.bounding_box().await?;
        let metrics = self
            .execute_script(&format!("return {};", crate::utils::geometry::WINDOW_METRICS_SCRIPT))
            .await?;
        let metrics: [f64; 6] = serde_json::from_value(metrics)?;
        Ok(crate::utils::geometry::screen_rect(rect, metrics))
    }

    /// The centre of the element in screen coordinates. Corresponds to
    /// Python's `get_gui_element_center`; see
    /// [`gui_element_rect`](Self::gui_element_rect).
    ///
    /// # Errors
    ///
    /// See [`gui_element_rect`](Self::gui_element_rect).
    pub async fn gui_element_center(&mut self, selector: impl Into<Selector>) -> Result<crate::sb_cdp::Point, SeleniumBaseError> {
        let css: &Selector = &selector.into();
        let rect = self.gui_element_rect(css).await?;
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

    /// Shows `message` in the page with Messenger and, by default, waits for it
    /// to go. Python's `post_message(message, duration=None, pause=True,
    /// style="info")`: the message is the argument, and `.duration(..)`,
    /// `.pause(..)` and `.style(..)` are the keywords; see [`PostMessage`].
    /// The message is HTML, as in Python. It stays for the run's message
    /// duration (2.55 s unless `RunSettingsBuilder::message_duration` says
    /// otherwise), at most 0.75 s where nobody watches, and the call returns
    /// 0.15 s after it has gone. Where Messenger cannot be loaded the message
    /// is printed instead, as in Python, so awaiting the builder does not fail
    /// because of the page.
    pub fn post_message<'a>(&'a self, message: &'a str) -> PostMessage<'a> {
        PostMessage::new(self, message, MessageStyle::Info)
    }

    /// Shows the message `call` describes and waits as it asks: the work of
    /// awaiting a [`PostMessage`]. The message stays for `call`'s duration, else
    /// the run's message duration, at most 0.75 s where nobody watches.
    pub(crate) async fn show_messenger_message(&self, call: &PostMessage<'_>) {
        let shown = self.message_time(call.duration);
        let options = MessageOptions {
            duration: shown,
            pause: call.pause,
            style: call.style,
        };
        if !self.post_with_messenger(call.message, &options).await {
            println!("{}", fallback_line(call.style, call.message));
        }
        if call.pause {
            tokio::time::sleep(shown + MESSAGE_SETTLE).await;
        }
    }

    /// Posts `markup` with Messenger as Python's `post_message` does: try
    /// at once; if that fails, load Messenger, give it its starting theme and
    /// try again. False when the message could not be shown.
    async fn post_with_messenger(&self, markup: &str, options: &MessageOptions) -> bool {
        let script = format!("return {};", post_markup_script(markup, options));
        if self.messenger_posted(&script).await {
            return true;
        }
        if self.activate_messenger().await.is_err() {
            return false;
        }
        if self
            .apply_messenger_settings(self.messenger_defaults())
            .await
            .is_err()
        {
            return false;
        }
        self.messenger_posted(&script).await
    }

    /// Runs a script that answers true when Messenger took the message.
    async fn messenger_posted(&self, script: &str) -> bool {
        self.execute_script(script)
            .await
            .is_ok_and(|answer| answer.as_bool() == Some(true))
    }

    /// The theme and place Messenger starts with when a message loads it: the
    /// future theme at the bottom right, but at the top centre in a mobile
    /// emulator (Python's `set_messenger_theme` defaults).
    fn messenger_defaults(&self) -> MessengerSettings {
        let location = if self.config.browser.mobile {
            MessengerLocation::TopCenter
        } else {
            MessengerLocation::BottomRight
        };
        MessengerSettings {
            location,
            ..MessengerSettings::default()
        }
    }

    /// Saves the page's HTML as `name` in the log folder of the running test and returns its
    /// path. Only the last path part of `name` is used. Corresponds to
    /// Python's `save_as_html_to_logs`. The folder is the one
    /// [`save_screenshot_to_logs`](Self::save_screenshot_to_logs) writes to.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub async fn save_as_html_to_logs(&self, name: &str) -> Result<PathBuf, SeleniumBaseError> {
        let file = Path::new(name).file_name().ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!("{name:?} is not a usable file name"))
        })?;
        let path = self.test_logs_dir()?.join(file);
        self.save_page_source_to_path(&path).await?;
        Ok(path)
    }

    /// Saves a screenshot to the log folder of the running test (as
    /// [`save_screenshot_to_logs`](Self::save_screenshot_to_logs) does) for a
    /// failed test's report.
    /// Corresponds to Python's `save_teardown_screenshot`.
    ///
    /// # Errors
    ///
    /// Returns an error if the screenshot cannot be taken or written.
    pub async fn save_teardown_screenshot(&self) -> Result<PathBuf, SeleniumBaseError> {
        let path = artifact_path(&self.test_logs_dir()?, "teardown_screenshot", "png");
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
    pub fn saved_cookies(&self, name: &str) -> Result<serde_json::Value, SeleniumBaseError> {
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

    // ------------------------------------------------------------------
    // Python SeleniumBase method_summary parity getters and aliases
    // ------------------------------------------------------------------

    /// Gets the text of the element matching `selector`.
    /// Alias for [`text`](Self::text) matching Python SeleniumBase `get_text(selector)`.
    pub async fn get_text(&self, selector: impl Into<Selector>) -> Result<String, SeleniumBaseError> {
        self.text(selector).await
    }

    /// Gets the current page title.
    /// Alias for [`title`](Self::title) matching Python SeleniumBase `get_title()`.
    pub async fn get_title(&self) -> Result<String, SeleniumBaseError> {
        self.title().await
    }

    /// Alias for [`get_title`](Self::get_title) matching Python SeleniumBase `get_page_title()`.
    pub async fn get_page_title(&self) -> Result<String, SeleniumBaseError> {
        self.title().await
    }

    /// Gets the current browser URL.
    /// Alias for [`current_url`](Self::current_url) matching Python SeleniumBase `get_current_url()`.
    pub async fn get_current_url(&self) -> Result<String, SeleniumBaseError> {
        self.current_url().await
    }

    /// Gets the current page HTML source.
    /// Alias for [`page_source`](Self::page_source) matching Python SeleniumBase `get_page_source()`.
    pub async fn get_page_source(&self) -> Result<String, SeleniumBaseError> {
        self.page_source().await
    }

    /// Gets an attribute value of an element.
    /// Alias for [`attribute`](Self::attribute) matching Python SeleniumBase `get_attribute(selector, attribute)`.
    pub async fn get_attribute(
        &mut self,
        selector: impl Into<Selector>,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.attribute(selector, attribute).await
    }

    /// Gets a property of an element.
    /// Alias for [`property`](Self::property) matching Python SeleniumBase `get_property(selector, property)`.
    pub async fn get_property(
        &mut self,
        selector: impl Into<Selector>,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.property(selector, property).await
    }

    /// Alias for [`get_property`](Self::get_property) matching Python SeleniumBase `get_property_value(selector, property)`.
    pub async fn get_property_value(
        &mut self,
        selector: impl Into<Selector>,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.property(selector, property).await
    }

    /// Gets the HTML of the current page.
    /// Alias for [`html`](Self::html) matching Python SeleniumBase `get_html()`.
    pub async fn get_html(&self) -> Result<String, SeleniumBaseError> {
        self.html().await
    }

    /// Gets the outer HTML of the element matching `selector`.
    /// Corresponds to Python's `get_element_html(selector)`.
    pub async fn get_element_html(&mut self, selector: impl Into<Selector>) -> Result<String, SeleniumBaseError> {
        let el = self.find_element(selector).await?;
        el.html().await
    }

    /// Gets the text content of an element.
    /// Alias for [`text_content`](Self::text_content) matching Python SeleniumBase `get_text_content(selector)`.
    pub async fn get_text_content(&mut self, selector: impl Into<Selector>) -> Result<String, SeleniumBaseError> {
        self.text_content(selector).await
    }

    /// Gets all cookies from the current page.
    /// Alias for [`cookies`](Self::cookies) matching Python SeleniumBase `get_cookies()`.
    pub async fn get_cookies(&self) -> Result<Vec<crate::Cookie>, SeleniumBaseError> {
        self.cookies().await
    }

    /// Gets a cookie by name from the current page.
    /// Alias for [`cookie`](Self::cookie) matching Python SeleniumBase `get_cookie(name)`.
    pub async fn get_cookie(&self, name: &str) -> Result<Option<crate::Cookie>, SeleniumBaseError> {
        self.cookie(name).await
    }

    /// Gets the cookie string formatted as an HTTP `Cookie` header.
    /// Alias for [`cookie_string`](Self::cookie_string) matching Python SeleniumBase `get_cookie_string()`.
    pub async fn get_cookie_string(&self) -> Result<String, SeleniumBaseError> {
        self.cookie_string().await
    }

    /// Finds and returns the first element matching `selector`.
    /// Alias for [`find_element`](Self::find_element) matching Python SeleniumBase `get_element(selector)`.
    pub async fn get_element(&mut self, selector: impl Into<Selector>) -> Result<Element, SeleniumBaseError> {
        self.find_element(selector).await
    }

    /// Returns the parent of the element matching `selector`.
    /// Alias for [`parent`](Self::parent) matching Python SeleniumBase `get_parent(selector)`.
    pub async fn get_parent(&mut self, selector: impl Into<Selector>) -> Result<Option<Element>, SeleniumBaseError> {
        self.parent(selector).await
    }

    /// Gets the value of an input element.
    /// Alias for [`value`](Self::value) matching Python SeleniumBase `get_value(selector)`.
    pub async fn get_value(&mut self, selector: impl Into<Selector>) -> Result<String, SeleniumBaseError> {
        self.value(selector).await
    }

    /// Gets the browser user agent.
    /// Alias for [`user_agent`](Self::user_agent) matching Python SeleniumBase `get_user_agent()`.
    pub async fn get_user_agent(&self) -> Result<String, SeleniumBaseError> {
        self.user_agent().await
    }

    /// Gets the origin of the current page.
    /// Alias for [`origin`](Self::origin) matching Python SeleniumBase `get_origin()`.
    pub async fn get_origin(&mut self) -> Result<String, SeleniumBaseError> {
        self.origin().await
    }

    /// Extracts the domain from `url`.
    /// Alias for [`domain_url`](Self::domain_url) matching Python SeleniumBase `get_domain_url(url)`.
    pub fn get_domain_url(&self, url: &str) -> Result<String, SeleniumBaseError> {
        self.domain_url(url)
    }

    /// Gets the browser downloads folder path.
    /// Alias for [`downloads_folder`](Self::downloads_folder) matching Python SeleniumBase `get_downloads_folder()`.
    pub fn get_downloads_folder(&self) -> PathBuf {
        self.downloads_folder()
    }

    /// Alias for [`get_downloads_folder`](Self::get_downloads_folder) matching Python SeleniumBase `get_browser_downloads_folder()`.
    pub fn get_browser_downloads_folder(&self) -> PathBuf {
        self.downloads_folder()
    }

    /// Gets a list of downloaded files.
    /// Alias for [`downloaded_files`](Self::downloaded_files) matching Python SeleniumBase `get_downloaded_files()`.
    pub fn get_downloaded_files(&self) -> Result<Vec<PathBuf>, SeleniumBaseError> {
        self.downloaded_files()
    }

    /// Gets the full path of a downloaded file.
    /// Alias for [`path_of_downloaded_file`](Self::path_of_downloaded_file) matching Python SeleniumBase `get_path_of_downloaded_file(file)`.
    pub fn get_path_of_downloaded_file(&self, filename: impl AsRef<Path>) -> Result<PathBuf, SeleniumBaseError> {
        self.path_of_downloaded_file(filename)
    }

    /// Reads data from a downloaded file.
    /// Alias for [`data_from_downloaded_file`](Self::data_from_downloaded_file) matching Python SeleniumBase `get_data_from_downloaded_file(file)`.
    pub fn get_data_from_downloaded_file(&self, filename: impl AsRef<Path>) -> Result<String, SeleniumBaseError> {
        self.data_from_downloaded_file(filename)
    }

    /// Reads text data from a file.
    /// Alias for [`file_data`](Self::file_data) matching Python SeleniumBase `get_file_data(file)`.
    pub fn get_file_data(&self, filename: impl AsRef<Path>) -> Result<String, SeleniumBaseError> {
        self.file_data(filename)
    }

    #[cfg(feature = "pdf")]
    /// Extracts text from a PDF file.
    /// Alias for [`pdf_text`](Self::pdf_text) matching Python SeleniumBase `get_pdf_text(pdf)`.
    pub fn get_pdf_text(&self, pdf: &str) -> Result<String, SeleniumBaseError> {
        self.pdf_text(pdf)
    }

    /// Gets the screen rectangle as `(width, height)`.
    /// Alias for [`screen_rect`](Self::screen_rect) matching Python SeleniumBase `get_screen_rect()`.
    pub async fn get_screen_rect(&self) -> Result<(i64, i64), SeleniumBaseError> {
        self.screen_rect().await
    }

    /// Gets the browser window bounds as `(x, y, width, height)`.
    /// Alias for [`window_rect`](Self::window_rect) matching Python SeleniumBase `get_window_rect()`.
    pub async fn get_window_rect(&self) -> Result<(i64, i64, i64, i64), SeleniumBaseError> {
        self.window_rect().await
    }

    /// Gets the browser window size as `(width, height)`.
    /// Alias for [`window_size`](Self::window_size) matching Python SeleniumBase `get_window_size()`.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        self.window_size().await
    }

    /// Gets the browser window position as `(x, y)`.
    /// Alias for [`window_position`](Self::window_position) matching Python SeleniumBase `get_window_position()`.
    pub async fn get_window_position(&self) -> Result<(i64, i64), SeleniumBaseError> {
        self.window_position().await
    }

    /// Gets the browser locale code.
    /// Alias for [`locale_code`](Self::locale_code) matching Python SeleniumBase `get_locale_code()`.
    pub async fn get_locale_code(&self) -> Result<String, SeleniumBaseError> {
        self.locale_code().await
    }

    /// Generates a time-based one-time password (TOTP) MFA code.
    /// Alias for [`mfa_code`](Self::mfa_code) matching Python SeleniumBase `get_mfa_code(totp_key)`.
    pub fn get_mfa_code(&self, totp_key: &str) -> Result<String, SeleniumBaseError> {
        self.mfa_code(totp_key)
    }

    /// Generates a time-based one-time password (TOTP) code.
    /// Alias for [`totp_code`](Self::totp_code) matching Python SeleniumBase `get_totp_code(totp_key)`.
    pub fn get_totp_code(&self, totp_key: &str) -> Result<String, SeleniumBaseError> {
        self.totp_code(totp_key)
    }

    /// Generates a Google Authenticator verification code.
    /// Alias for [`google_auth_code`](Self::google_auth_code) matching Python SeleniumBase `get_google_auth_code(secret)`.
    pub fn get_google_auth_code(&self, secret: &str) -> Result<String, SeleniumBaseError> {
        self.google_auth_code(secret)
    }

    /// Alias for [`get_google_auth_code`](Self::get_google_auth_code) matching Python SeleniumBase `get_google_auth_password(secret)`.
    pub fn get_google_auth_password(&self, secret: &str) -> Result<String, SeleniumBaseError> {
        self.google_auth_code(secret)
    }

    /// Gets the installed Chrome version.
    /// Alias for [`chrome_version`](Self::chrome_version) matching Python SeleniumBase `get_chrome_version()`.
    pub async fn get_chrome_version(&self) -> Result<String, SeleniumBaseError> {
        self.chrome_version().await
    }

    /// Gets the installed ChromeDriver version.
    /// Alias for [`chromedriver_version`](Self::chromedriver_version) matching Python SeleniumBase `get_chromedriver_version()`.
    pub fn get_chromedriver_version(&self) -> Result<String, SeleniumBaseError> {
        self.chromedriver_version()
    }

    /// Alias for [`get_chrome_version`](Self::get_chrome_version) matching Python SeleniumBase `get_chromium_version()`.
    pub async fn get_chromium_version(&self) -> Result<String, SeleniumBaseError> {
        self.chrome_version().await
    }

    /// Alias for [`get_chromedriver_version`](Self::get_chromedriver_version) matching Python SeleniumBase `get_chromium_driver_version()`.
    pub fn get_chromium_driver_version(&self) -> Result<String, SeleniumBaseError> {
        self.chromedriver_version()
    }

    /// Gets a local storage item value by key.
    /// Alias for [`local_storage_item`](Self::local_storage_item) matching Python SeleniumBase `get_local_storage_item(key)`.
    pub async fn get_local_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        self.local_storage_item(key).await
    }

    /// Gets all local storage items.
    /// Alias for [`local_storage_items`](Self::local_storage_items) matching Python SeleniumBase `get_local_storage_items()`.
    pub async fn get_local_storage_items(&self) -> Result<std::collections::HashMap<String, String>, SeleniumBaseError> {
        self.local_storage_items().await
    }

    /// Gets all local storage keys.
    /// Alias for [`local_storage_keys`](Self::local_storage_keys) matching Python SeleniumBase `get_local_storage_keys()`.
    pub async fn get_local_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        self.local_storage_keys().await
    }

    /// Gets a session storage item value by key.
    /// Alias for [`session_storage_item`](Self::session_storage_item) matching Python SeleniumBase `get_session_storage_item(key)`.
    pub async fn get_session_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        self.session_storage_item(key).await
    }

    /// Gets all session storage items.
    /// Alias for [`session_storage_items`](Self::session_storage_items) matching Python SeleniumBase `get_session_storage_items()`.
    pub async fn get_session_storage_items(&self) -> Result<std::collections::HashMap<String, String>, SeleniumBaseError> {
        self.session_storage_items().await
    }

    /// Gets all session storage keys.
    /// Alias for [`session_storage_keys`](Self::session_storage_keys) matching Python SeleniumBase `get_session_storage_keys()`.
    pub async fn get_session_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        self.session_storage_keys().await
    }

    /// Gets the CSS selector of `document.activeElement`.
    /// Alias for [`active_element_css`](Self::active_element_css) matching Python SeleniumBase `get_active_element_css()`.
    pub async fn get_active_element_css(&self) -> Result<String, SeleniumBaseError> {
        self.active_element_css().await
    }

    /// Gets the option texts of a `<select>` element.
    /// Alias for [`select_options`](Self::select_options) matching Python SeleniumBase `get_select_options(selector)`.
    pub async fn get_select_options(&mut self, selector: &str) -> Result<Vec<String>, SeleniumBaseError> {
        self.select_options(selector).await
    }

    /// Gets all unique links (`href` attributes) on the page.
    /// Alias for [`unique_links`](Self::unique_links) matching Python SeleniumBase `get_unique_links()`.
    pub async fn get_unique_links(&self) -> Result<Vec<String>, SeleniumBaseError> {
        self.unique_links().await
    }

    /// Gets an attribute of a link identified by its exact link text.
    /// Alias for [`link_attribute`](Self::link_attribute) matching Python SeleniumBase `get_link_attribute(link_text, attribute)`.
    pub async fn get_link_attribute(
        &mut self,
        link_text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.link_attribute(link_text, attribute).await
    }

    /// Alias for [`get_link_attribute`](Self::get_link_attribute) matching Python SeleniumBase `get_link_text_attribute(link_text, attribute)`.
    pub async fn get_link_text_attribute(
        &mut self,
        link_text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.link_attribute(link_text, attribute).await
    }

    /// Gets an attribute of a link identified by partial link text.
    /// Alias for [`partial_link_text_attribute`](Self::partial_link_text_attribute) matching Python SeleniumBase `get_partial_link_text_attribute(partial_link_text, attribute)`.
    pub async fn get_partial_link_text_attribute(
        &mut self,
        partial_link_text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.partial_link_text_attribute(partial_link_text, attribute).await
    }

    /// Checks the HTTP status code of a link identified by its exact link text.
    /// Alias for [`link_status_code`](Self::link_status_code) matching Python SeleniumBase `get_link_status_code(link_text)`.
    pub async fn get_link_status_code(&self, link_text: &str) -> Result<u16, SeleniumBaseError> {
        self.link_status_code(link_text).await
    }

    /// Reads saved cookies from disk.
    /// Alias for [`saved_cookies`](Self::saved_cookies) matching Python SeleniumBase `get_saved_cookies(name)`.
    pub fn get_saved_cookies(&self, name: &str) -> Result<serde_json::Value, SeleniumBaseError> {
        self.saved_cookies(name)
    }

    /// Types text into an input field (clearing it first).
    /// Alias for [`type_text`](Self::type_text) matching Python SeleniumBase `type(selector, text)`.
    pub async fn r#type(
        &mut self,
        selector: impl Into<Selector>,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.type_text(selector, text).await
    }

    /// Opens a URL in UC (undetected) mode.
    /// Alias for [`open`](Self::open) matching Python SeleniumBase `uc_open(url)`.
    pub async fn uc_open(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    /// Opens a URL in a new tab in UC (undetected) mode.
    /// Alias for [`open_new_tab_url`](Self::open_new_tab_url) matching Python SeleniumBase `uc_open_with_tab(url)`.
    pub async fn uc_open_with_tab(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open_new_tab_url(url).await
    }

    /// Skips the test with a message.
    /// Alias for [`skip_test`](Self::skip_test) matching Python SeleniumBase `skip(reason)`.
    pub fn skip(&self, reason: &str) -> Result<(), SeleniumBaseError> {
        self.skip_test(reason)
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

    fn cookie() -> Cookie {
        Cookie::new("sid", "abc")
            .domain(".example.test")
            .expires(4_102_444_800.0)
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
        assert_eq!(foreign.domain, "www.other.test");
        // A domain the origin contains is left alone.
        let mut same = cookie();
        prepare_cookie(&mut same, "https://www.example.test", CookieExpiry::Saved, 0);
        assert_eq!(same.domain, ".example.test");
        // A cookie with no domain gets none.
        let mut bare = Cookie::new("sid", "abc");
        prepare_cookie(&mut bare, "https://www.other.test", CookieExpiry::Saved, 0);
        assert_eq!(bare.domain, "");
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
            let mut cookie = cookie();
            prepare_cookie(&mut cookie, origin, expiry, 1_000);
            cookie.expires
        };
        assert_eq!(after(CookieExpiry::default()), None, "dropped by default");
        assert_eq!(after(CookieExpiry::Session), None);
        assert_eq!(after(CookieExpiry::Minutes(0)), None);
        assert_eq!(after(CookieExpiry::Saved), Some(4_102_444_800.0));
        assert_eq!(after(CookieExpiry::Day), Some(1_000.0 + 86_400.0));
        assert_eq!(after(CookieExpiry::Minutes(5)), Some(1_000.0 + 300.0));
        // A cookie with no expiry gets one only when asked to.
        let mut session = Cookie::new("sid", "abc");
        prepare_cookie(&mut session, origin, CookieExpiry::Saved, 1_000);
        assert_eq!(session.expires, None);
        prepare_cookie(&mut session, origin, CookieExpiry::Day, 1_000);
        assert_eq!(session.expires, Some(1_000.0 + 86_400.0));
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

    #[tokio::test]
    async fn a_good_cookie_before_a_bad_one_is_not_added() {
        let driver = page_with_a_cookie();
        let sb = driver.base_case().await;
        let dir = tempfile::tempdir().unwrap();
        // The first entry of each file is fine. The whole file is read before the first
        // cookie goes in, so the bad second entry stops the load with nothing added.
        for (name, text) in [
            (
                "second_nameless.txt",
                r#"[{"name": "a", "value": "1"}, {"value": "2"}]"#,
            ),
            ("second_scalar.txt", r#"[{"name": "a", "value": "1"}, 7]"#),
            (
                // Python writes `Lax`, `Strict` and `None`; another spelling is not a policy.
                "second_same_site.txt",
                r#"[{"name": "a", "value": "1"}, {"name": "b", "value": "2", "sameSite": "sometimes"}]"#,
            ),
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
            assert!(added(&driver).is_empty(), "{name} added a cookie");
        }
    }

}
