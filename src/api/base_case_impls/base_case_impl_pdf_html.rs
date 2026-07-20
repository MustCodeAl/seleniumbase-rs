// Additional BaseCase methods for PDF, HTML parsing, and themed tours.

impl BaseCase {
    /// Captures the current page as a PDF using CDP `Page.printToPDF`.
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

        pdf::save_pdf_bytes(&bytes, filename)?;
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

    /// Returns the configured browser locale.
    pub fn get_locale(&self) -> String {
        self.config.locale.clone().unwrap_or_else(|| "en-US".to_owned())
    }

    /// Asserts no JavaScript errors are present in recorded console logs.
    pub async fn assert_no_js_errors(&self) -> Result<(), SeleniumBaseError> {
        let logs = self.get_recorded_console_logs().await?;
        for entry in &logs {
            if entry.to_lowercase().contains("error") {
                return Err(SeleniumBaseError::AssertionFailed(format!(
                    "JS error detected: {entry}"
                )));
            }
        }
        Ok(())
    }

    /// Converts a CSS selector to a simple XPath expression.
    pub fn convert_css_to_xpath(&self, css: &str) -> Result<String, SeleniumBaseError> {
        // Handles basic tag, id, and class selectors only.
        let mut xpath = String::from("//");
        if let Some(id) = css.strip_prefix('#') {
            xpath.push_str(&format!("*[@id='{}']", html_escape_single(id)));
        } else if let Some(class) = css.strip_prefix('.') {
            xpath.push_str(&format!(
                "*[contains(concat(' ', normalize-space(@class), ' '), ' {} ')]",
                html_escape_single(class)
            ));
        } else if let Some(pos) = css.find('#') {
            let tag = &css[..pos];
            let id = &css[pos + 1..];
            xpath.push_str(&format!("{}[@id='{}']", tag, html_escape_single(id)));
        } else if let Some(pos) = css.find('.') {
            let tag = &css[..pos];
            let class = &css[pos + 1..];
            xpath.push_str(&format!(
                "{}[contains(concat(' ', normalize-space(@class), ' '), ' {} ')]",
                tag,
                html_escape_single(class)
            ));
        } else {
            xpath.push_str(css);
        }
        Ok(xpath)
    }
}

fn html_escape_single(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
