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
    pub async fn cdp_find_element(&self, selector: impl Into<Selector>) -> Result<CdpNode, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).find_element(&selector.to_string()).await
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
    pub async fn cdp_find_all(&self, selector: impl Into<Selector>) -> Result<Vec<CdpNode>, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).find_all(&selector.to_string()).await
    }

    /// Clicks the center of the first element matching `selector` using CDP
    /// input events. The selector is a CSS selector, as for every `cdp_*`
    /// method; XPath and `link=` forms are for [`click`](Self::click).
    pub async fn cdp_click(&mut self, selector: impl Into<Selector>) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        self.record_selector("cdp_click", selector, None);
        CdpPage::new(&self.session).click(&selector.to_string()).await
    }

    /// Focuses the element matching `selector`, clears it, and types `text`.
    pub async fn cdp_type(&mut self, selector: impl Into<Selector>, text: &str) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).type_text(&selector.to_string(), text).await
    }

    /// Focuses the element matching `selector`, clears it, and inserts `text`
    /// in one operation.
    ///
    /// Faster than [`cdp_type`][Self::cdp_type], but it emits no per-character
    /// key events, so it is the wrong choice under stealth.
    pub async fn cdp_fast_type(&mut self, selector: impl Into<Selector>, text: &str,) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).fast_type(&selector.to_string(), text).await
    }

    /// Waits up to `timeout` for an element matching `selector` to be present
    /// in the DOM.
    ///
    /// # Errors
    ///
    /// Returns an error if the element is still absent when `timeout` expires.
    pub async fn cdp_wait_for_element_present(&self, selector: impl Into<Selector>, timeout: Duration,) -> Result<CdpNode, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).wait_for_element_present(&selector.to_string(), timeout)
            .await
    }

    /// Returns the text of the first element matching `selector`: an input's or a textarea's
    /// value, as Python's `sb.cdp.get_text` gives it, else the rendered text.
    pub async fn cdp_get_text(&self, selector: impl Into<Selector>) -> Result<String, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).text(&selector.to_string()).await
    }

    /// Returns the named HTML attribute of the first matching element.
    pub async fn cdp_get_attribute(&self, selector: impl Into<Selector>, attribute: &str,) -> Result<Option<String>, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).attribute(&selector.to_string(), attribute)
            .await
    }

    /// Returns the named DOM property of the first matching element.
    pub async fn cdp_get_property(&self, selector: impl Into<Selector>, property: &str,) -> Result<Option<String>, SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).property(&selector.to_string(), property)
            .await
    }

    /// Returns the current page title via CDP.
    pub async fn cdp_get_title(&self) -> Result<String, SeleniumBaseError> {
        CdpPage::new(&self.session).title().await
    }

    /// Returns the current page URL via CDP.
    pub async fn cdp_get_url(&self) -> Result<String, SeleniumBaseError> {
        CdpPage::new(&self.session).url().await
    }

    /// Evaluates `expression` in the page context via CDP `Runtime.evaluate`.
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
    pub async fn cdp_select_option_by_text(&mut self, selector: impl Into<Selector>, text: &str,) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).select_option_by_text(&selector.to_string(), text)
            .await
    }

    /// Selects the `<option>` with the given value.
    pub async fn cdp_select_option_by_value(&mut self, selector: impl Into<Selector>, value: &str,) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).select_option_by_value(&selector.to_string(), value)
            .await
    }

    /// Selects the `<option>` at `index` (zero-based) inside the `<select>`
    /// matching `selector`.
    pub async fn cdp_select_option_by_index(&mut self, selector: impl Into<Selector>, index: usize,) -> Result<(), SeleniumBaseError> {
        let selector: &Selector = &selector.into();
        CdpPage::new(&self.session).select_option_by_index(&selector.to_string(), index)
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
