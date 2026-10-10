// Link-text helpers.

/// `links` without empty entries and duplicates, in first-seen order.
fn unique_links(links: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    links
        .into_iter()
        .filter(|link| !link.is_empty() && seen.insert(link.clone()))
        .collect()
}

/// Whether `link` is an `http://` or `https://` URL, the only kind that has an
/// HTTP status to report.
fn is_http_link(link: &str) -> bool {
    let scheme_end = link.find(':').unwrap_or(0);
    matches!(
        link[..scheme_end].to_ascii_lowercase().as_str(),
        "http" | "https"
    )
}

/// One line of `print_unique_links_with_status_codes` output.
fn format_link_status(link: &str, status: &Result<u16, SeleniumBaseError>) -> String {
    match status {
        Ok(code) => format!("{link} -> {code}"),
        Err(error) => format!("{link} -> error ({error})"),
    }
}

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

    /// Returns true if a link with exact `text` is in the DOM, visible or not.
    ///
    /// Use [`BaseCase::is_link_text_visible`] to require that it is displayed.
    ///
    /// # Errors
    ///
    /// Returns the WebDriver error if the lookup itself fails; a link that is
    /// simply absent is `Ok(false)`.
    pub async fn is_link_text_present(&self, text: &str) -> Result<bool, SeleniumBaseError> {
        let links = self.session.driver().find_all(By::LinkText(text)).await?;
        Ok(!links.is_empty())
    }

    /// Returns true if a link containing `text` is in the DOM, visible or not.
    ///
    /// Use [`BaseCase::is_partial_link_text_visible`] to require that it is
    /// displayed.
    ///
    /// # Errors
    ///
    /// Returns the WebDriver error if the lookup itself fails; a link that is
    /// simply absent is `Ok(false)`.
    pub async fn is_partial_link_text_present(&self, text: &str) -> Result<bool, SeleniumBaseError> {
        let links = self
            .session
            .driver()
            .find_all(By::PartialLinkText(text))
            .await?;
        Ok(!links.is_empty())
    }

    /// Waits up to `timeout_secs` for a link with exact `text` to be present.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no such link appears.
    pub async fn wait_for_link_text_present(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Link text '{text}' was not present"),
            || self.is_link_text_present(text),
        )
        .await
    }

    /// Waits up to `timeout_secs` for a link containing `text` to be present.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no such link appears.
    pub async fn wait_for_partial_link_text_present(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Partial link text '{text}' was not present"),
            || self.is_partial_link_text_present(text),
        )
        .await
    }

    /// Waits up to `timeout_secs` for a link with exact `text` to be visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no such link is displayed
    /// in time.
    pub async fn wait_for_link_text_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Link text '{text}' was not visible"),
            || self.is_link_text_visible(text),
        )
        .await
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

    /// Returns the HTTP status code for `url`.
    ///
    /// Redirects are not followed: a moved page reports its own `301`/`302`.
    /// This is the same as Python's `get_link_status_code`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Network`] if the request gets no answer.
    pub async fn get_link_status_code(&self, url: &str) -> Result<u16, SeleniumBaseError> {
        http_status_code(url).await
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

    /// Prints every unique link on the page together with its HTTP status code,
    /// one `link -> code` line per `http(s)` link, in page order.
    ///
    /// Links are checked eight at a time. A link that gets no answer prints
    /// `link -> error (reason)`. Links that are not `http(s)` (`mailto:`,
    /// `javascript:` and the like) have no status; they are listed after the
    /// others as `link -> skipped (not an HTTP link)`.
    pub async fn print_unique_links_with_status_codes(&self) -> Result<(), SeleniumBaseError> {
        let (http, other): (Vec<String>, Vec<String>) = unique_links(self.get_all_links().await?)
            .into_iter()
            .partition(|link| is_http_link(link));
        for (link, status) in check_links(http).await {
            println!("{}", format_link_status(&link, &status));
        }
        for link in other {
            println!("{link} -> skipped (not an HTTP link)");
        }
        Ok(())
    }

    /// Returns all `href` values from `<a>` tags on the current page.
    ///
    /// Anchors without an `href` attribute are left out.
    pub async fn get_all_links(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let result = self
            .execute_script(
                "return Array.from(document.querySelectorAll('a[href]')).map(a => a.href);",
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

#[cfg(test)]
mod tests_links {
    use super::*;

    #[test]
    fn unique_links_drops_blanks_and_repeats_but_keeps_page_order() {
        let links = ["https://b", "", "https://a", "https://b", "https://a", "https://c"]
            .map(String::from)
            .to_vec();
        assert_eq!(unique_links(links), ["https://b", "https://a", "https://c"]);
    }

    #[test]
    fn only_http_and_https_links_have_a_status() {
        for link in ["http://a/", "https://a/x", "HTTPS://A/"] {
            assert!(is_http_link(link), "{link}");
        }
        for link in ["mailto:a@b.c", "javascript:void(0)", "tel:+1", "ftp://a", "#top", "/relative", ""] {
            assert!(!is_http_link(link), "{link}");
        }
    }

    #[test]
    fn a_failed_check_prints_its_reason_instead_of_a_fake_status_zero() {
        assert_eq!(format_link_status("https://a/", &Ok(200)), "https://a/ -> 200");
        let failed = Err(SeleniumBaseError::network("https://a/", 0, "connection refused"));
        let line = format_link_status("https://a/", &failed);
        assert!(line.starts_with("https://a/ -> error ("), "{line}");
        assert!(line.contains("connection refused"), "{line}");
    }
}
