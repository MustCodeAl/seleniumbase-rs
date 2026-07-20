// Link-text helpers.

impl BaseCase {
    /// Clicks the first `<a>` element whose full visible text equals `text`.
    pub async fn click_link(&mut self, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("click_link", Some(text), None);
        let element = self.session.driver().find(By::LinkText(text)).await?;
        element.click().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Clicks the first `<a>` element whose visible text contains `text`.
    pub async fn click_partial_link(&mut self, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("click_partial_link", Some(text), None);
        let element = self.session.driver().find(By::PartialLinkText(text)).await?;
        element.click().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns true if a link with exact `text` exists.
    pub async fn is_link_text_present(&self, text: &str) -> Result<bool, SeleniumBaseError> {
        match self.session.driver().find(By::LinkText(text)).await {
            Ok(el) => Ok(el.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Returns true if a link containing `text` exists.
    pub async fn is_partial_link_text_present(&self, text: &str) -> Result<bool, SeleniumBaseError> {
        match self.session.driver().find(By::PartialLinkText(text)).await {
            Ok(el) => Ok(el.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Waits up to `timeout_secs` for a link with exact `text` to be present.
    pub async fn wait_for_link_text_present(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_link_text_present(text).await.unwrap_or(false) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Link text '{}' was not present",
                    text
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for a link containing `text` to be present.
    pub async fn wait_for_partial_link_text_present(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(self.effective_timeout(timeout_secs));
        loop {
            if self.is_partial_link_text_present(text).await.unwrap_or(false) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Partial link text '{}' was not present",
                    text
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Waits up to `timeout_secs` for a link with exact `text` to be visible.
    pub async fn wait_for_link_text_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_link_text_present(text, timeout_secs).await
    }

    /// Returns the `attribute` of the first link with exact `text`.
    pub async fn get_link_attribute(
        &mut self,
        text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let el = self.session.driver().find(By::LinkText(text)).await?;
        el.attr(attribute).await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Alias for `get_link_attribute`.
    pub async fn get_link_text_attribute(
        &mut self,
        text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.get_link_attribute(text, attribute).await
    }

    /// Returns the `attribute` of the first link containing `text`.
    pub async fn get_partial_link_text_attribute(
        &mut self,
        text: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let el = self.session.driver().find(By::PartialLinkText(text)).await?;
        el.attr(attribute).await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns the `href` of the first link with exact `text`.
    pub async fn get_href_from_link_text(&mut self, text: &str) -> Result<String, SeleniumBaseError> {
        self.get_link_attribute(text, "href")
            .await?
            .ok_or_else(|| SeleniumBaseError::AssertionFailed(format!("Link '{}' has no href", text)))
    }

    /// Asserts that a link with exact `text` exists.
    pub async fn assert_link(&mut self, text: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_link_text_present(text).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Link '{}' was not found",
                text
            )));
        }
        Ok(())
    }

    /// Asserts that the link at `url` does not return HTTP 404.
    pub async fn assert_link_status_code_is_not_404(&self, url: &str) -> Result<(), SeleniumBaseError> {
        let code = http_status_code(url).await?;
        if code == 404 {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Link '{}' returned 404",
                url
            )));
        }
        Ok(())
    }

    /// Prints every unique link on the page together with its HTTP status code.
    pub async fn print_unique_links_with_status_codes(&self) -> Result<(), SeleniumBaseError> {
        let links = self.get_all_links().await?;
        let mut seen = std::collections::HashSet::new();
        for link in links {
            if !seen.insert(link.clone()) {
                continue;
            }
            let code = http_status_code(&link).await.unwrap_or(0);
            println!("{} -> {}", link, code);
        }
        Ok(())
    }

    /// Returns all `href` values from `<a>` tags on the current page.
    pub async fn get_all_links(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let result = self
            .execute_script(
                "return Array.from(document.querySelectorAll('a')).map(a => a.href);",
            )
            .await?;
        match result {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }
}
