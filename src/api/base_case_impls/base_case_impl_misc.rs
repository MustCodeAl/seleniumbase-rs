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
        level: f64,
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
        level: f64,
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

    /// Displays a success toast message on the page for the run's message
    /// duration ([`RunSettings`](crate::api::run_settings::RunSettings)). It
    /// is a plain box, not Python's Messenger widget, and replaces the toast
    /// already showing.
    pub async fn post_success_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.post_styled_message(message, "#28a745").await
    }

    /// Displays an error toast message on the page; see
    /// [`post_success_message`](Self::post_success_message).
    pub async fn post_error_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.post_styled_message(message, "#dc3545").await
    }

    /// Displays a toast message and highlights it.
    pub async fn post_message_and_highlight(
        &self,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.post_styled_message(message, "#ffc107").await?;
        let _ = self.highlight("#sb-toast-message").await;
        Ok(())
    }

    async fn post_styled_message(&self, message: &str, color: &str) -> Result<(), SeleniumBaseError> {
        let seconds = self.run_settings.message_seconds();
        self.execute_script(&page_scripts::toast_script(message, color, seconds))
            .await?;
        Ok(())
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

    /// Alias for `convert_xpath_to_css`.
    pub fn convert_to_css_selector(&self, xpath: &str) -> Result<String, SeleniumBaseError> {
        self.convert_xpath_to_css(xpath)
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
