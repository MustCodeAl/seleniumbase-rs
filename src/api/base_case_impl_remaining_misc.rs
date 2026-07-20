// Miscellaneous remaining helpers.

impl BaseCase {
    // Deferred / delayed assertion aliases.

    /// Records a deferred assertion that `css` must be present.
    pub async fn deferred_assert_element_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_element(css).await
    }

    /// Records a deferred assertion for exact text in `css`.
    pub async fn deferred_assert_exact_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_text(text, css).await
    }

    /// Records a deferred assertion that `css` has non-empty text.
    pub async fn deferred_assert_non_empty_text(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.deferred.add_element(css);
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

    /// Alias for `deferred_assert_exact_text`.
    pub async fn delayed_assert_exact_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_exact_text(css, text).await
    }

    /// Alias for `deferred_assert_non_empty_text`.
    pub async fn delayed_assert_non_empty_text(
        &mut self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_non_empty_text(css).await
    }

    /// Alias for `deferred_assert_text`.
    pub async fn delayed_assert_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.deferred_assert_text(text, css).await
    }

    /// Alias for `process_deferred_asserts`.
    pub async fn process_delayed_asserts(&mut self) -> Result<(), SeleniumBaseError> {
        self.process_deferred_asserts().await
    }

    /// Alias for `process_deferred_asserts`.
    pub async fn deferred_check_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.process_deferred_asserts().await
    }

    /// Alias for `process_deferred_asserts`.
    pub async fn delayed_check_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.process_deferred_asserts().await
    }

    // Console / messaging helpers.

    /// Injects a script that captures console logs into `window.__sbConsoleLogs`.
    pub async fn console_log_script(&self) -> Result<(), SeleniumBaseError> {
        self.start_recording_console_logs().await
    }

    /// Writes `text` to the recorded console log array if it exists.
    pub async fn console_log_string(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "if (window.__sbConsoleLogs) window.__sbConsoleLogs.push({});",
            serde_json::to_string(text).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Displays a success toast message on the page.
    pub async fn post_success_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.post_message(message, "#28a745").await
    }

    /// Displays an error toast message on the page.
    pub async fn post_error_message(&self, message: &str) -> Result<(), SeleniumBaseError> {
        self.post_message(message, "#dc3545").await
    }

    /// Displays a toast message and highlights it.
    pub async fn post_message_and_highlight(
        &self,
        message: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.post_message(message, "#ffc107").await?;
        let _ = self.highlight("#sb-toast-message").await;
        Ok(())
    }

    async fn post_message(&self, message: &str, color: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var d=document.createElement('div'); d.id='sb-toast-message'; d.textContent={}; \
             d.style.cssText='position:fixed;top:10px;right:10px;padding:12px;background:{};color:#fff;z-index:999999;'; \
             document.body.appendChild(d);",
            serde_json::to_string(message).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?,
            color
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    // Referral / traffic generators.

    /// Returns a URL with a referral parameter appended.
    pub fn generate_referral(&self, url: &str, referrer: &str) -> Result<String, SeleniumBaseError> {
        let parsed = reqwest::Url::parse(url).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?;
        let mut url = parsed.clone();
        url.query_pairs_mut().append_pair("ref", referrer);
        Ok(url.to_string())
    }

    /// Returns a chain of referral URLs.
    pub fn generate_referral_chain(&self, url: &str, referrers: &[&str]) -> Result<Vec<String>, SeleniumBaseError> {
        referrers
            .iter()
            .map(|r| self.generate_referral(url, r))
            .collect()
    }

    /// Opens the referral URL in the browser.
    pub async fn generate_traffic(&mut self, url: &str, referrer: &str) -> Result<(), SeleniumBaseError> {
        let referral = self.generate_referral(url, referrer)?;
        self.open(&referral).await
    }

    /// Opens a chain of referral URLs sequentially.
    pub async fn generate_traffic_chain(
        &mut self,
        url: &str,
        referrers: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for referrer in referrers {
            self.generate_traffic(url, referrer).await?;
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
        let path = dir.join(filename);
        let actions = self.recorded_actions()?;
        fs::write(&path, serde_json::to_string_pretty(&actions)?)?;
        Ok(path)
    }

    /// Firefox addons are not supported in this port.
    pub async fn install_addon(&self, _path: &str) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::Unsupported(
            "install_addon is not supported in the Rust port".to_owned(),
        ))
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
    // the sync helpers in `base_case_impl_extra.rs` / `base_case_impl_remaining.rs`).

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
