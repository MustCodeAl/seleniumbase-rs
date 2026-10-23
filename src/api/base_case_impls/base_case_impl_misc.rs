// Miscellaneous remaining helpers.

/// The selector of the link `generate_referral` builds and clicks.
const REFERRAL_LINK: &str = "a.analytics.referral.test";

/// The style Python gives the link `generate_referral` builds.
const REFERRAL_LINK_STYLE: &str = "font-family: Arial,sans-serif; font-size: 30px; color: #18a2cd";

/// The script that replaces the page's body with one link to `destination`.
///
/// The link is built with the DOM, not pasted into markup, and `destination`
/// goes in as a string literal, so a URL with a quote in it stays a URL.
fn referral_link_script(destination: &str) -> String {
    format!(
        "var link = document.createElement('a'); \
         link.className = 'analytics referral test'; \
         link.setAttribute('href', {href}); \
         link.setAttribute('style', {style}); \
         link.textContent = 'Magic Link Button'; \
         var body = document.createElement('body'); \
         body.appendChild(link); \
         document.documentElement.replaceChild(body, document.body);",
        href = quote(destination),
        style = quote(REFERRAL_LINK_STYLE),
    )
}

/// The folder of the baseline `check_window` keeps for the check `name`: for
/// a test called `test`, `visual_baseline/<test>/<name>`, where the test
/// name's path separators and other unsafe characters become `_`; without one,
/// the folder of the thread that runs the test (the function name, as in
/// Python).
fn check_window_folder(
    test: Option<&str>,
    name: &str,
) -> Result<std::path::PathBuf, SeleniumBaseError> {
    let Some(test) = test else {
        return crate::api::run_settings::baseline_dir(name);
    };
    let name = if name.is_empty() { "default" } else { name };
    crate::artifacts::confined_path(
        &std::path::Path::new("visual_baseline").join(crate::artifacts::safe_stem(test, "default")),
        name,
    )
}

/// Checks that `url` is one Python's `is_valid_url` accepts, for the
/// referral helpers, which refuse a page they could not navigate to before
/// they touch the browser.
fn require_page_url(url: &str, role: &str) -> Result<(), SeleniumBaseError> {
    if is_valid_url_text(url) {
        Ok(())
    } else {
        Err(SeleniumBaseError::Parse {
            input: url.to_owned(),
            expected: format!("a valid URL for {role}"),
        })
    }
}

impl BaseCase {
    // Deferred / delayed assertion aliases.

    /// Checks now that `css` is in the page, visible or not, and records a
    /// failure for [`process_deferred_asserts`](Self::process_deferred_asserts)
    /// to report. Corresponds to Python's `deferred_assert_element_present`.
    pub async fn deferred_assert_element_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        deferred.check_element_present(self, css).await;
        self.deferred = deferred;
        Ok(())
    }

    /// Checks now that the text of `css` is exactly `text`, and records a
    /// failure for [`process_deferred_asserts`](Self::process_deferred_asserts)
    /// to report. The text comes first and the selector second, as in
    /// Python's `deferred_assert_exact_text(text, selector)`.
    pub async fn deferred_assert_exact_text(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        deferred.check_exact_text(self, text, css).await;
        self.deferred = deferred;
        Ok(())
    }

    /// Checks now that `css` shows some text, and records a failure for
    /// [`process_deferred_asserts`](Self::process_deferred_asserts) to report.
    /// Corresponds to Python's `deferred_assert_non_empty_text`.
    pub async fn deferred_assert_non_empty_text(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        deferred.check_non_empty_text(self, css).await;
        self.deferred = deferred;
        Ok(())
    }

    /// Alias for `deferred_assert_element`.
    pub async fn delayed_assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_element(css).await
    }

    /// Alias for `deferred_assert_element_present`.
    pub async fn delayed_assert_element_present(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_element_present(css).await
    }

    /// Alias for `deferred_assert_exact_text`: the text comes first and the
    /// selector second.
    pub async fn delayed_assert_exact_text(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_exact_text(text, css).await
    }

    /// Alias for `deferred_assert_non_empty_text`.
    pub async fn delayed_assert_non_empty_text(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_non_empty_text(css).await
    }

    /// Alias for `deferred_assert_text`: the text comes first and the
    /// selector second.
    pub async fn delayed_assert_text(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_text(text, css).await
    }

    /// Alias for `process_deferred_asserts`.
    pub async fn process_delayed_asserts(&mut self) -> Result<(), SeleniumBaseError> {
        self.process_deferred_asserts().await
    }

    /// Runs [`check_window`](Self::check_window) and, if the window differs
    /// from its baseline, records the failure, with the page's URL, for
    /// [`process_deferred_asserts`](Self::process_deferred_asserts) to report
    /// instead of returning it. Corresponds to Python's `deferred_check_window`.
    pub async fn deferred_check_window(
        &mut self,
        name: &str,
        level: u8,
    ) -> Result<(), SeleniumBaseError> {
        if let Err(error) = self.check_window(name, level).await {
            let url = self.get_current_url().await.ok();
            self.deferred.record_failure(url, error.to_string());
        }
        Ok(())
    }

    /// Alias for `deferred_check_window`.
    pub async fn delayed_check_window(
        &mut self,
        name: &str,
        level: u8,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_check_window(name, level).await
    }

    // Console / messaging helpers.

    /// Logs the value of the JavaScript expression `script` to the browser's
    /// console, as `console.log(script)`. Corresponds to Python's
    /// `console_log_script`. To capture console output, see
    /// [`start_recording_console_logs`](Self::start_recording_console_logs).
    pub async fn console_log_script(&self, script: &str) -> Result<(), SeleniumBaseError> {
        self.execute_script(&page_scripts::console_log_expression_script(script))
            .await?;
        Ok(())
    }

    /// Logs the string `text` to the browser's console. Corresponds to
    /// Python's `console_log_string`.
    pub async fn console_log_string(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.execute_script(&page_scripts::console_log_string_script(text))
            .await?;
        Ok(())
    }

    /// Shows `message` in the page as a success message with Messenger.
    /// Corresponds to Python's `post_success_message(message, duration=None,
    /// pause=True)`: `.duration(..)` and `.pause(..)` are the keywords. It
    /// behaves as [`post_message`](Self::post_message) does, with the style
    /// fixed to success.
    pub fn post_success_message<'a>(&'a self, message: &'a str) -> PostMessage<'a> {
        PostMessage::new(self, message, MessageStyle::Success)
    }

    /// Shows `message` in the page as an error message with Messenger.
    /// Corresponds to Python's `post_error_message(message, duration=None,
    /// pause=True)`; see [`post_success_message`](Self::post_success_message).
    pub fn post_error_message<'a>(&'a self, message: &'a str) -> PostMessage<'a> {
        PostMessage::new(self, message, MessageStyle::Error)
    }

    /// Shows `message` as a success message and highlights the element `css`.
    /// Corresponds to Python's `post_message_and_highlight(message,
    /// selector)`.
    ///
    /// # Errors
    ///
    /// Returns an error if `css` cannot be highlighted (it is not on the page,
    /// for example).
    pub async fn post_message_and_highlight(
        &self,
        message: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        // The message stays while the element is outlined, so this does not
        // wait for it to go first.
        self.post_success_message(message).pause(false).await?;
        self.highlight(css).await
    }

    // Referral / traffic generators.

    /// Opens `start_page`, builds a link to `destination_page` in it and
    /// clicks the link, so the destination receives a real `Referer` header
    /// naming the start page. Corresponds to Python's `generate_referral`.
    ///
    /// An empty `start_page` starts from the page the browser is on. With a
    /// `selector`, that element is clicked on the destination page too, which
    /// keeps the visit from counting as a bounce.
    ///
    /// The link replaces the whole body of the start page, so the page is
    /// left on the destination. The browser's referrer policy decides how much
    /// of the start page's address the destination sees: by default Chrome
    /// sends all of it to a page of the same origin and the origin alone to
    /// a page of another origin.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Parse`] if either page is not a URL that
    /// [`is_valid_url`](Self::is_valid_url) accepts, before anything is sent
    /// to the browser, and the usual wait and driver errors for the steps.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.generate_referral("https://example.com/blog", "https://example.com/shop", None)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate_referral(
        &mut self,
        start_page: &str,
        destination_page: &str,
        selector: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        require_page_url(destination_page, "destination_page")?;
        if !start_page.is_empty() {
            require_page_url(start_page, "start_page")?;
            self.open(start_page).await?;
            self.sleep(0.08).await;
            self.wait_for_ready_state_complete().await?;
        }
        self.execute_script(&referral_link_script(destination_page))
            .await?;
        self.click(REFERRAL_LINK).await?;
        self.sleep(0.15).await;
        // Python moves on after a pause; waiting for the page the click led to
        // keeps the next hop of a chain from running on the page being left.
        self.wait_for_ready_state_complete().await?;
        if let Some(selector) = selector {
            self.click(selector).await?;
            self.sleep(0.15).await;
        }
        Ok(())
    }

    /// Runs [`generate_referral`](Self::generate_referral) `loops` times, so
    /// the destination receives that many visits from the start page.
    /// Corresponds to Python's `generate_traffic`.
    ///
    /// # Errors
    ///
    /// See [`generate_referral`](Self::generate_referral); the first loop that
    /// fails ends the run.
    pub async fn generate_traffic(
        &mut self,
        start_page: &str,
        destination_page: &str,
        loops: u32,
        selector: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        for _ in 0..loops {
            self.generate_referral(start_page, destination_page, selector)
                .await?;
            self.sleep(0.05).await;
        }
        Ok(())
    }

    /// Clicks through `pages` in order: a link to the first page is built in
    /// the page the browser is on and clicked, then a link to the second in the
    /// first, and so on, so every page after the first sees the one before it
    /// as its referrer. A visit that goes on to a second page of the site this
    /// way does not count as a bounce. Corresponds to Python's
    /// `generate_referral_chain`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if fewer than two pages are
    /// given and [`SeleniumBaseError::Parse`] if any is not a valid URL; both
    /// are checked before the first click.
    pub async fn generate_referral_chain(&mut self, pages: &[&str]) -> Result<(), SeleniumBaseError> {
        if pages.len() < 2 {
            return Err(SeleniumBaseError::InvalidConfig(
                "a referral chain needs at least two website pages".to_owned(),
            ));
        }
        for page in pages {
            require_page_url(page, "a page of the chain")?;
        }
        for page in pages {
            self.generate_referral("", page, None).await?;
        }
        Ok(())
    }

    /// Runs [`generate_referral_chain`](Self::generate_referral_chain)
    /// `loops` times. Corresponds to Python's `generate_traffic_chain`.
    ///
    /// # Errors
    ///
    /// See [`generate_referral_chain`](Self::generate_referral_chain); the
    /// first loop that fails ends the run.
    pub async fn generate_traffic_chain(
        &mut self,
        pages: &[&str],
        loops: u32,
    ) -> Result<(), SeleniumBaseError> {
        for _ in 0..loops {
            self.generate_referral_chain(pages).await?;
            self.sleep(0.05).await;
        }
        Ok(())
    }

    // Selector / soup / CDP aliases.

    /// Converts any kind of selector to CSS, for the jQuery and JavaScript
    /// helpers that need a CSS selector. Corresponds to Python's
    /// `convert_to_css_selector(selector, by)`, with the strategy read from the
    /// selector as the other methods do: CSS is returned as it is, XPath is
    /// converted as [`convert_xpath_to_css`](Self::convert_xpath_to_css) does,
    /// and a bare element id (`Selector::Id`) becomes `[id='x']`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] for XPath that has no CSS
    /// equivalent and for link text (`link=Home`), which CSS cannot select.
    pub fn convert_to_css_selector(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        match Selector::auto(selector) {
            Selector::Css(css) => Ok(css.to_owned()),
            Selector::XPath(xpath) => self.convert_xpath_to_css(xpath),
            Selector::Id(id) => Ok(format!("[id='{}']", id.replace('\\', "\\\\").replace('\'', "\\'"))),
            Selector::LinkText(_) | Selector::PartialLinkText(_) => {
                Err(SeleniumBaseError::InvalidSelector(format!(
                    "'{selector}' selects a link by its text, which has no CSS equivalent"
                )))
            }
        }
    }

    /// Alias for `get_beautiful_soup_object`.
    pub async fn get_beautiful_soup(&self) -> Result<BeautifulSoup, SeleniumBaseError> {
        self.get_beautiful_soup_object().await
    }

    /// Executes a CDP command with the given parameters.
    pub async fn execute_cdp_cmd(
        &self,
        command: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SeleniumBaseError> {
        self.session.execute_cdp_with_params(command, params).await
    }

    // File / recorder helpers.

    /// Saves recorded actions to a JSON file in the logs directory.
    pub fn save_recorded_actions(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        let actions = self.recorded_actions()?;
        fs::write(&path, serde_json::to_string_pretty(&actions)?)?;
        Ok(path)
    }

    /// Ensures hidden file inputs are visible for automation.
    pub async fn show_file_choosers(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            "document.querySelectorAll('input[type=file]').forEach(el => { el.style.display = 'block'; el.style.visibility = 'visible'; });",
        )
        .await?;
        Ok(())
    }

    // Test control helpers (message-overload variants to avoid collisions with
    // the sync helpers in `base_case_impl_extra.rs` / `base_case_impl_common.rs`).

    /// Fails the test unless `condition` is true, with a custom message.
    pub fn assert_true_msg(
        &self,
        condition: bool,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        if condition {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
        }
    }

    /// Fails the test unless `condition` is false, with a custom message.
    pub fn assert_false_msg(
        &self,
        condition: bool,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.assert_true_msg(!condition, message)
    }

    /// Fails the test unless `actual` equals `expected`, with a custom message.
    pub fn assert_equal_msg<A: std::fmt::Debug + PartialEq<B>, B: std::fmt::Debug>(
        &self,
        actual: A,
        expected: B,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        if actual == expected {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "{}: expected {:?}, got {:?}",
                message, expected, actual
            )))
        }
    }

    /// Fails the test unless `actual` does not equal `not_expected`, with a custom message.
    pub fn assert_not_equal_msg<A: std::fmt::Debug + PartialEq<B>, B: std::fmt::Debug>(
        &self,
        actual: A,
        not_expected: B,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        if actual != not_expected {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "{}: unexpected value {:?}",
                message, actual
            )))
        }
    }

    /// Fails the test if `value` is None, with a custom message, returning the value.
    pub fn assert_not_none_msg<T: std::fmt::Debug>(
        &self,
        value: Option<T>,
        message: &str,
    ) -> Result<T, SeleniumBaseError> {
        value.ok_or_else(|| SeleniumBaseError::AssertionFailed(message.to_owned()))
    }

    /// Fails the test if `value` is Some, with a custom message.
    pub fn assert_none_msg<T: std::fmt::Debug>(
        &self,
        value: Option<T>,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        if value.is_none() {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
        }
    }

    /// Evaluates a closure and returns its result, or fails with `message`.
    pub fn assert_with<T, F>(&self, message: &str, f: F) -> Result<T, SeleniumBaseError>
    where
        F: FnOnce() -> Result<T, SeleniumBaseError>,
    {
        f().map_err(|e| SeleniumBaseError::AssertionFailed(format!("{}: {}", message, e)))
    }

    /// Marks the current test as skipped with `message`.
    pub fn skip_test(&self, message: &str) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::Skipped(message.to_owned()))
    }
}

#[cfg(test)]
mod tests_check_window {
    use super::*;
    use std::path::Path;

    #[test]
    fn tests_of_the_same_function_in_other_modules_keep_separate_baselines() {
        let cart = check_window_folder(Some("shop::cart::tests::renders"), "home").unwrap();
        let blog = check_window_folder(Some("blog::tests::renders"), "home").unwrap();
        assert_ne!(cart, blog);
        assert_eq!(
            cart,
            Path::new("visual_baseline")
                .join("shop__cart__tests__renders")
                .join("home")
        );
    }

    #[test]
    fn a_test_name_cannot_name_a_folder_outside_the_baselines() {
        let folder = check_window_folder(Some("../../etc/passwd"), "home").unwrap();
        assert_eq!(
            folder,
            Path::new("visual_baseline")
                .join("______etc_passwd")
                .join("home")
        );
        check_window_folder(Some("shop::cart"), "../home").unwrap_err();
        check_window_folder(Some("shop::cart"), "a/b").unwrap_err();
    }

    #[test]
    fn any_selector_converts_to_css_where_css_can_say_it() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        let css = |selector: &str| sb.convert_to_css_selector(selector);
        assert_eq!(css("#a > .b").unwrap(), "#a > .b");
        assert_eq!(css("//ul/li[2]").unwrap(), "ul > li:nth-of-type(2)");
        // A link text has no CSS; so does XPath that CSS cannot say.
        assert!(matches!(
            css("link=Home"),
            Err(SeleniumBaseError::InvalidSelector(_))
        ));
        assert!(matches!(
            css("//a[text()='Home']"),
            Err(SeleniumBaseError::InvalidSelector(_))
        ));
    }

    #[test]
    fn an_empty_check_name_is_the_default_one() {
        let folder = check_window_folder(Some("shop::cart"), "").unwrap();
        assert!(folder.ends_with("shop__cart/default"), "{folder:?}");
    }

    #[test]
    fn without_a_test_name_the_folder_is_named_after_the_running_test_function() {
        // The test harness names this thread `…::tests_check_window::without_a_…`.
        let folder = check_window_folder(None, "home").unwrap();
        assert_eq!(
            folder,
            Path::new("visual_baseline")
                .join("without_a_test_name_the_folder_is_named_after_the_running_test_function")
                .join("home")
        );
    }

    #[test]
    fn a_case_keeps_the_name_it_was_given() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        assert_eq!(sb.test_name(), None);
        let sb = sb.with_test_name("shop::cart::renders");
        assert_eq!(sb.test_name(), Some("shop::cart::renders"));
    }

    #[test]
    fn a_case_starts_with_the_run_settings_of_its_configuration() {
        let demo = RunSettings::default()
            .with_demo_mode(true)
            .with_demo_sleep(Duration::from_millis(30));
        let sb =
            BaseCase::without_session(BrowserConfig::default().with_run_settings(demo.clone()));
        assert_eq!(sb.run_settings(), &demo);
        let plain = BaseCase::without_session(BrowserConfig::default());
        assert_eq!(plain.run_settings(), &RunSettings::default());
    }

    #[test]
    fn a_default_timeout_of_the_run_settings_is_the_timeout_of_the_case_rounded_up() {
        let config = BrowserConfig::default().with_run_settings(
            RunSettings::default().with_default_timeout(Duration::from_millis(7_500)),
        );
        assert_eq!(BaseCase::without_session(config).timeout_secs(), 8);
        // Without one the case keeps Python's ten seconds.
        let plain = BaseCase::without_session(BrowserConfig::default());
        assert_eq!(plain.timeout_secs(), 10);
    }

    #[test]
    fn a_case_that_exists_can_be_named_too() {
        // What `sb_test!` does, since the runner builds the case.
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        sb.set_test_name("shop::cart::renders");
        assert_eq!(sb.test_name(), Some("shop::cart::renders"));
        sb.set_test_name(String::from("shop::cart::empties"));
        assert_eq!(sb.test_name(), Some("shop::cart::empties"));
    }
}
