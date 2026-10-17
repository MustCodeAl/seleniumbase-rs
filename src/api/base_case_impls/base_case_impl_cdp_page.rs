// Additional BaseCase methods for CDP-backed page automation.
// Included at the end of `base_case.rs` so the code lives in the same module
// and can access the private `session` field directly.

use crate::api::cdp_page::{CdpNode, CdpPage};

impl BaseCase {
    /// Navigates to `url` using CDP `Page.navigate`.
    pub async fn cdp_open(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).open(url).await
    }

    /// Finds the first element matching `selector` by running `document.querySelector` in the
    /// page through `Runtime.evaluate`. The `DOM` protocol domain is never switched on, because a
    /// page can tell that a client is attached to it.
    ///
    /// The returned [`CdpNode`] has a `node_id` of 0 and carries an `object_id`, which is how it
    /// is used with `Runtime.callFunctionOn`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches and
    /// [`SeleniumBaseError::InvalidSelector`] if the browser rejects the selector.
    pub async fn cdp_find_element(&self, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
        CdpPage::new(&self.session).find_element(selector).await
    }

    /// Finds all elements matching `selector` by running `document.querySelectorAll` in the page
    /// through `Runtime.evaluate` and `Runtime.getProperties`, in document order. The `DOM`
    /// protocol domain is never switched on.
    ///
    /// Each returned [`CdpNode`] has a `node_id` of 0 and carries an `object_id`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the browser rejects the selector.
    pub async fn cdp_find_all(&self, selector: &str) -> Result<Vec<CdpNode>, SeleniumBaseError> {
        CdpPage::new(&self.session).find_all(selector).await
    }

    /// Clicks the center of the first element matching `selector` using CDP
    /// input events, after waiting up to 7 seconds for it and scrolling it into
    /// view.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches, and
    /// the browser's error if it refuses an input event.
    pub async fn cdp_click(&mut self, selector: &str) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).click(selector).await
    }

    /// Focuses the element matching `selector`, clears it, and types `text`.
    pub async fn cdp_type(&mut self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).type_text(selector, text).await
    }

    /// Focuses the element matching `selector`, clears it, and inserts `text`
    /// in one operation.
    ///
    /// Faster than [`cdp_type`][Self::cdp_type], but it emits no per-character
    /// key events, so it is the wrong choice under stealth.
    pub async fn cdp_fast_type(
        &mut self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).fast_type(selector, text).await
    }

    /// Waits up to `timeout` for an element matching `selector` to be present
    /// in the DOM.
    ///
    /// # Errors
    ///
    /// Returns an error if the element is still absent when `timeout` expires.
    pub async fn cdp_wait_for_element_present(
        &self,
        selector: &str,
        timeout: Duration,
    ) -> Result<CdpNode, SeleniumBaseError> {
        CdpPage::new(&self.session)
            .wait_for_element_present(selector, timeout)
            .await
    }

    /// Returns the visible text of the first element matching `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn cdp_get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        CdpPage::new(&self.session).get_text(selector).await
    }

    /// Returns the named HTML attribute of the first matching element, or
    /// `None` if it has no such attribute.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn cdp_get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        CdpPage::new(&self.session)
            .get_attribute(selector, attribute)
            .await
    }

    /// Returns the named DOM property of the first matching element.
    pub async fn cdp_get_property(
        &self,
        selector: &str,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        CdpPage::new(&self.session)
            .get_property(selector, property)
            .await
    }

    /// Returns the current page title via CDP.
    pub async fn cdp_get_title(&self) -> Result<String, SeleniumBaseError> {
        CdpPage::new(&self.session).get_title().await
    }

    /// Returns the current page URL via CDP.
    pub async fn cdp_get_url(&self) -> Result<String, SeleniumBaseError> {
        CdpPage::new(&self.session).get_url().await
    }

    /// Evaluates `expression` in the page context via CDP `Runtime.evaluate`
    /// and returns its value, as Python's `sb.cdp.evaluate` does. A `return `
    /// that starts the last line is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the script throws.
    pub async fn cdp_evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        CdpPage::new(&self.session).evaluate(expression).await
    }

    /// Navigates back in browser history via CDP.
    pub async fn cdp_go_back(&mut self) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).go_back().await
    }

    /// Navigates forward in browser history via CDP.
    pub async fn cdp_go_forward(&mut self) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).go_forward().await
    }

    /// Reloads the current page via CDP.
    pub async fn cdp_refresh(&mut self) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).refresh().await
    }

    /// Selects the `<option>` with matching visible text.
    pub async fn cdp_select_option_by_text(
        &mut self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session)
            .select_option_by_text(selector, text)
            .await
    }

    /// Selects the `<option>` with the given value.
    pub async fn cdp_select_option_by_value(
        &mut self,
        selector: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session)
            .select_option_by_value(selector, value)
            .await
    }

    /// Selects the `<option>` at `index` (zero-based) inside the `<select>`
    /// matching `selector`.
    pub async fn cdp_select_option_by_index(
        &mut self,
        selector: &str,
        index: usize,
    ) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session)
            .select_option_by_index(selector, index)
            .await
    }

    /// Captures a PNG screenshot via CDP and writes it to `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the session has no CDP connection, the capture
    /// fails, or the file cannot be written.
    pub async fn cdp_screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        CdpPage::new(&self.session).screenshot(path).await
    }
}
