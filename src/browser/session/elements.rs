//! Finding elements and acting on them: clicks, typing, selects, drags and
//! the questions asked of an element.

use serde_json::json;
use thirtyfour::prelude::{By, WebElement};

use crate::browser::shadow_find;
use crate::error::SeleniumBaseError;

use super::BrowserSession;

/// Selenium's `getAttribute` as a page script, a port of its atom: the value of
/// `arguments[1]` on the element `arguments[0]`, the DOM property first (what a
/// field holds now, not the `value=""` of its markup) and the attribute where the
/// property is missing, an object or a function; `selected`, `checked` and the
/// other boolean attributes give `"true"` or `null`. The `/* getAttribute */`
/// marker tells tests which script this is.
const GET_ATTRIBUTE: &str = "/* getAttribute */ \
    const element = arguments[0]; const name = String(arguments[1]).toLowerCase(); \
    const alias = { class: 'className', readonly: 'readOnly' }; \
    const booleans = ['allowfullscreen', 'allowpaymentrequest', 'allowusermedia', 'async', \
        'autofocus', 'autoplay', 'checked', 'compact', 'complete', 'controls', 'declare', \
        'default', 'defaultchecked', 'defaultselected', 'defer', 'disabled', 'ended', \
        'formnovalidate', 'hidden', 'indeterminate', 'iscontenteditable', 'ismap', \
        'itemscope', 'loop', 'multiple', 'muted', 'nohref', 'nomodule', 'noresize', \
        'noshade', 'novalidate', 'nowrap', 'open', 'paused', 'playsinline', 'pubdate', \
        'readonly', 'required', 'reversed', 'scoped', 'seamless', 'seeking', 'selected', \
        'truespeed', 'typemustmatch', 'willvalidate']; \
    const tag = element.tagName ? element.tagName.toUpperCase() : ''; \
    const attribute = element.getAttribute(name); \
    if (name === 'style') { \
        return element.style ? (typeof element.style === 'string' ? element.style \
            : element.style.cssText) : null; \
    } \
    const selectable = tag === 'OPTION' || (tag === 'INPUT' \
        && ['checkbox', 'radio'].includes(String(element.type).toLowerCase())); \
    if ((name === 'selected' || name === 'checked') && selectable) { \
        return (tag === 'OPTION' ? element.selected : element.checked) ? 'true' : null; \
    } \
    if ((tag === 'IMG' && name === 'src') || (tag === 'A' && name === 'href')) { \
        return attribute ? String(element[name]) : attribute; \
    } \
    if (name === 'spellcheck') { \
        const lower = attribute === null ? null : attribute.toLowerCase(); \
        return lower === 'false' || lower === 'true' ? lower : String(element[name]); \
    } \
    const key = alias[name] || String(arguments[1]); \
    if (booleans.includes(name)) { return attribute !== null || !!element[key] ? 'true' : null; } \
    if (name === 'value' && tag === 'LI') { return attribute; } \
    let property; \
    try { property = element[key]; } catch (error) { property = undefined; } \
    if (property === undefined || property === null \
        || typeof property === 'object' || typeof property === 'function') { return attribute; } \
    return String(property);";

impl BrowserSession {
    /// Clicks the element located by `locator`.
    pub async fn click(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.click().await?;
        Ok(())
    }

    /// Clears and types `text` into the element located by `locator`.
    pub async fn type_text(&mut self, locator: By, text: &str) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.clear().await?;
        element.send_keys(text).await?;
        Ok(())
    }

    /// Clears the value of the element located by `locator`.
    pub async fn clear(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        element.clear().await?;
        Ok(())
    }

    /// Submits the form containing the element located by `locator`.
    pub async fn submit(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let element_json = element.to_json()?;
        // Form submit via script
        self.webdriver()?
            .execute("arguments[0].closest('form').submit()", vec![element_json])
            .await?;
        Ok(())
    }

    /// Returns the visible text of the element located by `locator`.
    pub async fn text(&self, locator: By) -> Result<String, SeleniumBaseError> {
        let element = self.find(locator).await?;
        Ok(element.text().await?)
    }

    /// Hovers over the element located by `locator`.
    pub async fn hover(&mut self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        self.webdriver()?
            .action_chain()
            .move_to_element_center(&element)
            .perform()
            .await?;
        Ok(())
    }

    /// Selects the option with visible text `text`.
    pub async fn select_option_by_text(
        &mut self,
        locator: By,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_visible_text(text).await?;
        Ok(())
    }

    /// Selects the option whose `value` matches.
    pub async fn select_option_by_value(
        &mut self,
        locator: By,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.find(locator).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        select.select_by_value(value).await?;
        Ok(())
    }

    /// Drags the source element onto the target element.
    pub async fn drag_and_drop(
        &mut self,
        source_locator: By,
        target_locator: By,
    ) -> Result<(), SeleniumBaseError> {
        let source = self.find(source_locator).await?;
        let target = self.find(target_locator).await?;
        self.webdriver()?
            .action_chain()
            .drag_and_drop_element(&source, &target)
            .perform()
            .await?;
        Ok(())
    }

    /// Returns the first element matching `locator`. A CSS locator that crosses
    /// shadow roots (`host::shadow inner`) is looked up through each host's shadow
    /// root, and every lookup of the session goes through here.
    pub async fn find(&self, locator: By) -> Result<WebElement, SeleniumBaseError> {
        shadow_find::find(self.webdriver()?, locator).await
    }

    /// Returns all elements matching `locator`, which may cross shadow roots as
    /// [`find`](Self::find) says. A host that is missing or has no shadow root
    /// gives an empty list.
    pub async fn find_all(&self, locator: By) -> Result<Vec<WebElement>, SeleniumBaseError> {
        shadow_find::find_all(self.webdriver()?, locator).await
    }

    /// WebDriver interaction: `element_present`.
    pub async fn element_present(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let elements = self.find_all(locator).await?;
        Ok(!elements.is_empty())
    }

    /// WebDriver interaction: `get_attribute`.
    pub async fn get_attribute(
        &mut self,
        locator: By,
        attribute_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.find(locator).await?;
        self.attribute_or_property(&element, attribute_name).await
    }

    /// The value of `name` on `element`, as Selenium's `get_attribute` gives it:
    /// the DOM property of that name if the element has one (so the text typed
    /// into an input, not the `value=""` its markup has), and otherwise the attribute.
    /// A `<select>` has no `value` attribute, but its property says which option
    /// is chosen; an `<option>` has an `index` property and no such attribute.
    ///
    /// The page script is Selenium's atom, as Python's `get_attribute` runs it, not
    /// the driver's attribute command, which answers with the markup's attribute
    /// alone. `selected`, `checked` and the other boolean attributes are `"true"`
    /// or `None`, as in Selenium.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver fails.
    pub(crate) async fn attribute_or_property(
        &self,
        element: &WebElement,
        name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let value = self
            .webdriver()?
            .execute(GET_ATTRIBUTE, vec![element.to_json()?, json!(name)])
            .await?;
        Ok(value.json().as_str().map(str::to_owned))
    }

    /// Clicks `element` with a script (`element.click()`), which reaches an
    /// element that is not displayed or that another element covers.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver fails.
    pub(crate) async fn js_click_element(
        &self,
        element: &WebElement,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .execute("arguments[0].click();", vec![element.to_json()?])
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `get_property`.
    pub async fn get_property(
        &mut self,
        locator: By,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.find(locator).await?;
        Ok(element.prop(property_name).await?)
    }

    /// Double-clicks the element located by `locator`.
    pub async fn double_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.webdriver()?
            .action_chain()
            .double_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Right-clicks the element located by `locator`.
    pub async fn context_click(&self, locator: By) -> Result<(), SeleniumBaseError> {
        let element = self.wait_for_element_clickable(locator.clone(), 10).await?;
        self.webdriver()?
            .action_chain()
            .context_click_element(&element)
            .perform()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is enabled.
    pub async fn is_enabled(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self.find(locator).await?;
        element
            .is_enabled()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Returns `true` if the element located by `locator` is selected.
    pub async fn is_selected(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        let element = self.find(locator).await?;
        element
            .is_selected()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `is_displayed`.
    pub async fn is_displayed(&self, locator: By) -> Result<bool, SeleniumBaseError> {
        if let Ok(element) = self.find(locator).await {
            element
                .is_displayed()
                .await
                .map_err(SeleniumBaseError::WebDriver)
        } else {
            Ok(false)
        }
    }

    /// WebDriver interaction: `find_elements`.
    pub async fn find_elements(
        &self,
        locator: thirtyfour::By,
    ) -> Result<Vec<thirtyfour::WebElement>, SeleniumBaseError> {
        self.find_all(locator).await
    }
}
