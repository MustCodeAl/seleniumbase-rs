//! Finding elements and acting on them: clicks, typing, selects, drags and
//! the questions asked of an element.

use thirtyfour::prelude::WebElement;

use crate::browser::element::Element;
use crate::sb_cdp::SelectBy;
use crate::utils::selectors::SelectorBuf;
use serde_json::json;

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
    /// Clicks the element `selector` finds.
    pub async fn click(
        &mut self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector).await?.click().await
    }

    /// Clears and types `text` into the element `selector` finds.
    pub async fn type_text(
        &mut self,
        selector: impl Into<SelectorBuf>,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector).await?.type_text(text).await
    }

    /// Clears the value of the element `selector` finds.
    pub async fn clear(
        &mut self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector).await?.clear().await
    }

    /// Submits the form containing the element `selector` finds.
    pub async fn submit(
        &mut self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector).await?.submit().await
    }

    /// Returns the text the driver renders for the element `selector` finds.
    ///
    /// That is the driver's own answer, which is empty for an `<input>`; the
    /// text Python's checks compare is [`Element::text`].
    pub async fn text(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<String, SeleniumBaseError> {
        let element = self.find(selector).await?;
        element.rendered_text().await
    }

    /// Hovers over the element `selector` finds.
    pub async fn hover(
        &mut self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector).await?.hover().await
    }

    /// Selects the option with visible text `text`.
    pub async fn select_option_by_text(
        &mut self,
        selector: impl Into<SelectorBuf>,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector)
            .await?
            .select_option(SelectBy::Text(text))
            .await
    }

    /// Selects the option whose `value` matches.
    pub async fn select_option_by_value(
        &mut self,
        selector: impl Into<SelectorBuf>,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.find(selector)
            .await?
            .select_option(SelectBy::Value(value))
            .await
    }

    /// Drags the element `source` finds onto the element `target` finds.
    pub async fn drag_and_drop(
        &mut self,
        source: impl Into<SelectorBuf>,
        target: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        let source = self.find(source).await?;
        let target = self.find(target).await?;
        source.drag_to(&target).await
    }

    /// Returns the first element `selector` finds. A CSS selector that crosses
    /// shadow roots (`host::shadow inner`) is looked up through each host's shadow
    /// root, and every lookup of the session goes through here.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidSelector`] for an empty selector, or one that
    /// crosses a shadow root and cannot be cut; the driver's `no such element`
    /// error if nothing matches.
    pub async fn find(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<Element, SeleniumBaseError> {
        let driver = self.webdriver()?;
        let selector = selector.into();
        let by = shadow_find::by_of(&selector)?;
        let found = shadow_find::find(driver, by).await?;
        Ok(Element::new(found, driver.clone(), selector))
    }

    /// Returns all elements `selector` finds, which may cross shadow roots as
    /// [`find`](Self::find) says. A host that is missing or has no shadow root
    /// gives an empty list.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidSelector`], as for [`find`](Self::find).
    pub async fn find_all(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<Vec<Element>, SeleniumBaseError> {
        let driver = self.webdriver()?;
        let selector = selector.into();
        let by = shadow_find::by_of(&selector)?;
        let found = shadow_find::find_all(driver, by).await?;
        Ok(found
            .into_iter()
            .map(|inner| Element::new(inner, driver.clone(), selector.clone()))
            .collect())
    }

    /// WebDriver interaction: `element_present`.
    pub async fn element_present(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<bool, SeleniumBaseError> {
        let elements = self.find_all(selector).await?;
        Ok(!elements.is_empty())
    }

    /// WebDriver interaction: `get_attribute`.
    pub async fn get_attribute(
        &mut self,
        selector: impl Into<SelectorBuf>,
        attribute_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.find(selector).await?;
        self.attribute_or_property(element.as_webdriver_element(), attribute_name)
            .await
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
        element: &Element,
    ) -> Result<(), SeleniumBaseError> {
        let reference = element.reference()?;
        self.webdriver()?
            .execute("arguments[0].click();", vec![reference])
            .await?;
        Ok(())
    }

    /// WebDriver interaction: `get_property`.
    pub async fn get_property(
        &mut self,
        selector: impl Into<SelectorBuf>,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.find(selector).await?.property(property_name).await
    }

    /// Double-clicks the element `selector` finds, once it is clickable.
    pub async fn double_click(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_clickable(selector, 10)
            .await?
            .double_click()
            .await
    }

    /// Right-clicks the element `selector` finds, once it is clickable.
    pub async fn context_click(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_clickable(selector, 10)
            .await?
            .right_click()
            .await
    }

    /// Returns `true` if the element `selector` finds is enabled.
    pub async fn is_enabled(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<bool, SeleniumBaseError> {
        self.find(selector).await?.is_enabled().await
    }

    /// Returns `true` if the element `selector` finds is selected.
    pub async fn is_selected(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<bool, SeleniumBaseError> {
        self.find(selector).await?.is_selected().await
    }

    /// WebDriver interaction: `is_displayed`. An element that is not there is
    /// not displayed.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidSelector`] for a selector that can never
    /// match.
    pub async fn is_displayed(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<bool, SeleniumBaseError> {
        let selector = shadow_find::checked(selector)?;
        match self.find(selector).await {
            Ok(element) => element.is_visible().await,
            _ => Ok(false),
        }
    }

    /// WebDriver interaction: `find_elements`.
    pub async fn find_elements(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<Vec<Element>, SeleniumBaseError> {
        self.find_all(selector).await
    }
}

/// What the session does with a selector and what it hands back, against the mock
/// WebDriver in `webdriver_support`.
#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::api::base_case::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use crate::browser::config::BrowserConfig;
    use crate::utils::selectors::Selector;

    /// A page whose `#a` is the element `e1`, an `<input>` with no rendered text and the
    /// value `typed`; `.row` lists `r1` and `r2`; anything else is not there.
    fn page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request: &Seen| {
            let asked = request.body["value"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if asked == "#a" || asked == "[id=\"a b\"]" => {
                    Reply::Value(element("e1"))
                }
                ("POST", "/elements") if asked == ".row" => {
                    Reply::Value(json!([element("r1"), element("r2")]))
                }
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") => Reply::Value(json!([])),
                ("GET", "/element/e1/name") => Reply::Value(json!("input")),
                ("GET", "/element/e1/text") => Reply::Value(json!("")),
                ("GET", "/element/e1/property/value") => Reply::Value(json!("typed")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn session(driver: &MockWebDriver) -> BrowserSession {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        BrowserSession::connect(config).await.expect("connects")
    }

    #[tokio::test]
    async fn find_hands_back_an_element_named_by_its_selector() {
        let driver = page();
        let session = session(&driver).await;

        let found = session.find("#a").await.unwrap();

        assert_eq!(found.to_string(), "#a");
        assert_eq!(found.as_webdriver_element().element_id().to_string(), "e1");
        let rows = session.find_all(".row").await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.to_string() == ".row"));
        assert!(session.find_all("#nothing").await.unwrap().is_empty());
        assert!(session.find("#nothing").await.is_err());
    }

    #[tokio::test]
    async fn the_kind_of_a_string_is_read_once_by_the_rules_of_every_method() {
        let driver = page();
        let session = session(&driver).await;

        session.find("//div").await.unwrap_err();
        session.find("link=Home").await.unwrap_err();
        session
            .find(SelectorBuf::css("//not-xpath"))
            .await
            .unwrap_err();

        let kinds: Vec<_> = driver
            .requests("/element")
            .into_iter()
            .map(|request| {
                request.body["using"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        assert_eq!(kinds, ["xpath", "link text", "css selector"]);
    }

    #[tokio::test]
    async fn an_id_selector_reaches_the_driver_as_a_working_css_lookup() {
        let driver = page();
        let session = session(&driver).await;

        // `Selector::Id` has no `SelectorBuf` variant: it is written as an attribute selector,
        // which needs no escaping whatever characters the id holds.
        let found = session.find(Selector::Id("a b")).await.unwrap();

        assert_eq!(found.to_string(), "[id=\"a b\"]");
        let asked = driver.requests("/element");
        assert_eq!(asked[0].body["using"], "css selector");
        assert_eq!(asked[0].body["value"], "[id=\"a b\"]");
    }

    #[tokio::test]
    async fn a_selector_that_can_never_match_is_refused_before_the_driver_is_asked() {
        let driver = page();
        let mut session = session(&driver).await;

        for selector in ["", "   ", "host ::shadow", "link="] {
            let error = session.find(selector).await.unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidSelector(_)),
                "{selector:?}: {error:?}"
            );
            session.click(selector).await.unwrap_err();
            session.is_displayed(selector).await.unwrap_err();
            session.wait_for_element(selector, 30).await.unwrap_err();
        }
        assert!(driver.requests("/element").is_empty());
        assert!(driver.requests("/elements").is_empty());
    }

    #[tokio::test]
    async fn the_text_of_the_session_is_still_the_rendered_text_of_the_driver() {
        // `BaseCase::get_text` reads this: the rendered text of an `<input>` is empty,
        // and only `Element::text` reads the field's value.
        let driver = page();
        let session = session(&driver).await;

        assert_eq!(session.text("#a").await.unwrap(), "");
        assert_eq!(
            session.find("#a").await.unwrap().text().await.unwrap(),
            "typed"
        );
    }
}
