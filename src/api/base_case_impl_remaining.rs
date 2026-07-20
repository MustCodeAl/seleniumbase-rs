// Remaining public BaseCase helpers to close the parity gap with Python SeleniumBase.
// This file is included at the end of `base_case.rs` so it shares the same scope.

use regex::Regex;

// -------------------------------------------------------------------------
// Internal helpers
// -------------------------------------------------------------------------

/// Best-effort HTTP status code for `url`.
async fn http_status_code(url: &str) -> Result<u16, SeleniumBaseError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?;
    let response = client.head(url).send().await;
    match response {
        Ok(resp) => Ok(resp.status().as_u16()),
        Err(e) if e.is_status() => Ok(e.status().unwrap_or(reqwest::StatusCode::BAD_REQUEST).as_u16()),
        Err(e) => Err(SeleniumBaseError::InvalidConfig(e.to_string())),
    }
}

fn regex_or_err(pattern: &str) -> Result<Regex, SeleniumBaseError> {
    Regex::new(pattern).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))
}

impl BaseCase {
    // =====================================================================
    // Generic / assertion helpers
    // =====================================================================

    /// Fails the test immediately with `message`.
    pub fn fail(&self, message: &str) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
    }

    /// Alias for sleeping `seconds`.
    pub async fn wait(&self, seconds: u64) {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
    }

    /// Sets the default soft timeout used by wait helpers.
    pub fn set_default_timeout(&mut self, seconds: u64) {
        self.time_limit_secs = Some(seconds);
    }

    /// Clears the default soft timeout.
    pub fn reset_default_timeout(&mut self) {
        self.time_limit_secs = None;
    }

    /// Alias for `get_property`.
    pub async fn get_property_value(
        &mut self,
        css: &str,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.get_property(css, property).await
    }

    /// Asserts that `css` is present in the DOM.
    pub async fn assert_element_present(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_element_present(css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Element '{}' was not present",
                css
            )));
        }
        Ok(())
    }

    /// Asserts that every selector in `selectors` is present.
    pub async fn assert_elements(&self, selectors: &[&str]) -> Result<(), SeleniumBaseError> {
        self.assert_elements_present(selectors).await
    }

    /// Asserts that every selector in `selectors` is visible.
    pub async fn assert_elements_visible(
        &mut self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            self.assert_element_visible(css).await?;
        }
        Ok(())
    }

    /// Asserts that at least one selector in `selectors` is present.
    pub async fn assert_any_of_elements_present(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            if self.is_element_present(css).await.unwrap_or(false) {
                return Ok(());
            }
        }
        Err(SeleniumBaseError::AssertionFailed(
            "None of the selectors were present".to_owned(),
        ))
    }

    /// Asserts that `text` is not visible inside `css`.
    pub async fn assert_exact_text_not_visible(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        if self.is_exact_text_visible(text, css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Exact text '{}' was unexpectedly visible in '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that `css` has non-empty visible text.
    pub async fn assert_non_empty_text(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_non_empty_text_visible(css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Element '{}' did not have non-empty visible text",
                css
            )));
        }
        Ok(())
    }

    // =====================================================================
    // Wait helpers
    // =====================================================================

    /// Waits up to `timeout_secs` for `css` to leave the DOM.
    pub async fn wait_for_element_not_present(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if !self.is_element_present(css).await.unwrap_or(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Element '{}' was still present",
                    css
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for exact `text` to be visible inside `css`.
    pub async fn wait_for_exact_text_visible(
        &self,
        css: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_exact_text_visible(text, css).await.unwrap_or(false) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Exact text '{}' was not visible in '{}'",
                    text, css
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for exact `text` to not be visible inside `css`.
    pub async fn wait_for_exact_text_not_visible(
        &self,
        css: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if !self.is_exact_text_visible(text, css).await.unwrap_or(true) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Exact text '{}' was still visible in '{}'",
                    text, css
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for `css` to contain non-empty visible text.
    pub async fn wait_for_non_empty_text(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_non_empty_text_visible(css).await.unwrap_or(false) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Non-empty text was not visible in '{}'",
                    css
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Alias for `wait_for_non_empty_text`.
    pub async fn wait_for_non_empty_text_visible(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_non_empty_text(css, timeout_secs).await
    }

    /// Waits up to `timeout_secs` for `text` to be visible anywhere on the page.
    pub async fn wait_for_text_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        let script = format!(
            "return document.body && document.body.innerText.includes('{}');",
            js_escape(text)
        );
        loop {
            match self.execute_script(&script).await {
                Ok(v) if v.as_bool() == Some(true) => return Ok(()),
                _ => {}
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Text '{}' was not visible",
                    text
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for `text` to not be visible anywhere.
    pub async fn wait_for_text_not_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        let script = format!(
            "return document.body && document.body.innerText.includes('{}');",
            js_escape(text)
        );
        loop {
            match self.execute_script(&script).await {
                Ok(v) if v.as_bool() != Some(true) => return Ok(()),
                _ => {}
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Text '{}' was still visible",
                    text
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
}
