// Additional BaseCase methods for PDF, HTML parsing, and themed tours.

/// Browser log messages from these libraries are not page errors, so
/// `assert_no_js_errors` ignores them (Python SeleniumBase does the same).
const IGNORED_ERROR_SOURCES: [&str; 2] = [
    "//cdnjs.cloudflare.com/ajax/libs/messenger",
    "//cdnjs.cloudflare.com/ajax/libs/underscore",
];

/// The `SEVERE` browser log entries that count as JavaScript errors.
fn severe_js_errors(entries: &[thirtyfour::BrowserLogEntry]) -> Vec<&thirtyfour::BrowserLogEntry> {
    entries
        .iter()
        .filter(|entry| entry.level == "SEVERE")
        .filter(|entry| {
            !IGNORED_ERROR_SOURCES
                .iter()
                .any(|source| entry.message.contains(source))
        })
        .collect()
}

impl BaseCase {
    /// Captures the current page as a PDF using CDP `Page.printToPDF`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if the session has no CDP
    /// connection or the response has no PDF data, and an I/O error if the file
    /// cannot be written.
    pub async fn print_to_pdf(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let response = self
            .session
            .execute_cdp_with_params(
                "Page.printToPDF",
                serde_json::json!({
                    "printBackground": true,
                    "preferCSSPageSize": true,
                }),
            )
            .await?;

        let data = response
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SeleniumBaseError::Unsupported("PDF data missing from CDP response".to_owned()))?;

        let bytes = BASE64_STANDARD
            .decode(data)
            .map_err(|e| SeleniumBaseError::Unsupported(format!("Failed to decode PDF: {e}")))?;

        tokio::fs::write(filename, bytes).await?;
        Ok(())
    }

    /// Alias for `print_to_pdf`.
    pub async fn save_as_pdf(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        self.print_to_pdf(filename).await
    }

    /// Extracts text from a PDF file on disk.
    pub fn get_pdf_text(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        pdf::extract_text_from_file(filename)
    }

    /// Asserts that `text` appears inside the given PDF.
    pub fn assert_pdf_text(&self, filename: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let content = self.get_pdf_text(filename)?;
        if !content.contains(text) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "PDF '{}' did not contain text '{}'",
                filename, text
            )));
        }
        Ok(())
    }

    /// Returns a BeautifulSoup-style parser for the current page source.
    pub async fn get_beautiful_soup_object(&self) -> Result<BeautifulSoup, SeleniumBaseError> {
        let source = self.get_page_source().await?;
        Ok(BeautifulSoup::parse(&source))
    }

    /// Creates a tour and assigns a visual theme.
    pub async fn create_tour_with_theme(
        &mut self,
        name: &str,
        theme: TourTheme,
    ) -> Result<(), SeleniumBaseError> {
        self.tour = Some(crate::api::tour::Tour::new(name).with_theme(theme));
        Ok(())
    }

    /// Full-screens the browser window.
    pub async fn fullscreen_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().fullscreen_window().await?;
        Ok(())
    }

    /// Returns the screen width via JavaScript.
    pub async fn get_screen_width(&self) -> Result<i64, SeleniumBaseError> {
        let value = self.execute_script("return window.screen.width;").await?;
        value
            .as_i64()
            .ok_or_else(|| SeleniumBaseError::Unsupported("screen width not a number".to_owned()))
    }

    /// Returns the screen height via JavaScript.
    pub async fn get_screen_height(&self) -> Result<i64, SeleniumBaseError> {
        let value = self.execute_script("return window.screen.height;").await?;
        value
            .as_i64()
            .ok_or_else(|| SeleniumBaseError::Unsupported("screen height not a number".to_owned()))
    }

    /// Returns the locale the browser was configured with, if one was.
    ///
    /// `None` means no locale was set, so the browser uses its own default.
    /// Use [`BaseCase::get_locale_code`] to ask the browser what it is using.
    pub fn configured_locale(&self) -> Option<&str> {
        self.config.locale.as_deref()
    }

    /// Returns the configured browser locale, or `en-US` when none was
    /// configured.
    ///
    /// The `en-US` fallback is a guess, not what the browser reports: a browser
    /// started without a locale uses the operating system's. Call
    /// [`BaseCase::configured_locale`] to tell the two apart, or
    /// [`BaseCase::get_locale_code`] for the locale the browser is using.
    pub fn get_locale(&self) -> String {
        self.configured_locale().unwrap_or("en-US").to_owned()
    }

    /// Asserts that the browser reported no severe JavaScript errors.
    ///
    /// This reads (and so clears) the browser's log, like Python SeleniumBase's
    /// `assert_no_js_errors`, and counts only entries of level `SEVERE`:
    /// uncaught exceptions, failed resource loads and `console.error`. A message
    /// that merely contains the word "error" is not one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] listing the errors, or a
    /// WebDriver error if the browser has no log to read (only Chromium-based
    /// browsers do).
    pub async fn assert_no_js_errors(&self) -> Result<(), SeleniumBaseError> {
        let entries = self.session.driver().browser_log().await?;
        let errors = severe_js_errors(&entries);
        if errors.is_empty() {
            return Ok(());
        }
        let listed = errors
            .iter()
            .map(|entry| entry.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let url = match self.session.driver().current_url().await {
            Ok(url) => url.to_string(),
            Err(_) => "the page".to_owned(),
        };
        Err(SeleniumBaseError::AssertionFailed(format!(
            "JavaScript errors found on {url} =>\n{listed}"
        )))
    }

    /// Converts a CSS selector to an equivalent XPath expression.
    ///
    /// Supports type, `*`, `#id`, `.class`, attribute selectors (`[a]`, `[a=v]`
    /// and the `~=`, `|=`, `^=`, `$=`, `*=` forms), the descendant, `>`, `+` and
    /// `~` combinators, comma-separated lists, and the pseudo-classes
    /// `:first-child`, `:last-child`, `:only-child`, `:first-of-type`,
    /// `:last-of-type`, `:nth-child(n)`, `:nth-of-type(n)` (a number, `odd` or
    /// `even`), `:empty` and `:contains("text")`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] for a selector it cannot
    /// convert, rather than an XPath expression that is not valid.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn demo(sb: &seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// assert_eq!(sb.convert_css_to_xpath("form > input#user")?, "//form/input[@id='user']");
    /// # Ok(())
    /// # }
    /// ```
    pub fn convert_css_to_xpath(&self, css: &str) -> Result<String, SeleniumBaseError> {
        css_to_xpath(css)
    }
}

#[cfg(test)]
mod tests_pdf_html {
    use super::*;

    fn entry(level: &str, message: &str) -> thirtyfour::BrowserLogEntry {
        serde_json::from_value(serde_json::json!({
            "level": level, "message": message, "timestamp": 0
        }))
        .unwrap()
    }

    #[test]
    fn only_severe_entries_are_js_errors() {
        let entries = vec![
            entry("INFO", "0 errors found"),
            entry("WARNING", "error-prone but only a warning"),
            entry("SEVERE", "https://x/app.js 1:1 Uncaught TypeError: x is not a function"),
            entry("SEVERE", "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/m.js"),
            entry("SEVERE", "see //cdnjs.cloudflare.com/ajax/libs/underscore/1.9.1/u.js"),
        ];
        let errors = severe_js_errors(&entries);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Uncaught TypeError"));
    }

    #[test]
    fn an_unconfigured_locale_is_distinguishable_from_the_fallback() {
        let config = BrowserConfig {
            locale: None,
            ..BrowserConfig::default()
        };
        let sb = BaseCase::with_session(config.clone(), BrowserSession::disconnected());
        assert_eq!(sb.configured_locale(), None);
        assert_eq!(sb.get_locale(), "en-US");
        let config = BrowserConfig {
            locale: Some("de-DE".to_owned()),
            ..config
        };
        let sb = BaseCase::with_session(config, BrowserSession::disconnected());
        assert_eq!(sb.configured_locale(), Some("de-DE"));
        assert_eq!(sb.get_locale(), "de-DE");
    }
}
