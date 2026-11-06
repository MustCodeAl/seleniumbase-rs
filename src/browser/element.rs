//! [`Element`]: one element of the page that the WebDriver engine found.
//!
//! [`BaseCase::find_element`](crate::BaseCase::find_element) and the other
//! methods that hand back an element return this type, and an element has the
//! methods of the pure CDP engine's [`Locator`](crate::sb_cdp::Locator) for
//! the same ideas (`text`, `html`, `click`, `type_text`, ...), under the same
//! names.
//!
//! The two are different things. A `Locator` remembers how to find an element
//! and finds it again on every use. An `Element` is the node the browser found
//! at one moment, as Python's `WebElement` is: it does not look again and it
//! does not wait, so it goes stale when the page replaces the node.

use std::fmt;

use serde_json::json;
use thirtyfour::error::WebDriverErrorInner;
use thirtyfour::prelude::{WebDriver, WebElement};

use crate::browser::session::shown_text;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::Rect;
use crate::utils::selectors::SelectorBuf;

/// The page script of [`Element::attribute`]: the attribute `arguments[1]` of
/// the element `arguments[0]`, as the markup has it, or `null`.
const ATTRIBUTE: &str = "return arguments[0].getAttribute(arguments[1]);";

/// One element of the page, found by the WebDriver engine.
///
/// It is the node the browser had when it was found, not a way to find it
/// again: it does not wait and it does not look a second time. When the page
/// replaces the node, the element is stale and its methods fail with
/// [`SeleniumBaseError::StaleElement`], which names the selector the element
/// was found by. To work on whatever the selector matches now, use the
/// selector (`BaseCase::click("#go")`) or the pure CDP engine's
/// [`Locator`](crate::sb_cdp::Locator), which finds again on every use.
///
/// An element is cheap to clone, and the clone is the same node, not a new
/// lookup.
///
/// For a `thirtyfour` call this type has no method for, the raw element is
/// reachable with [`as_webdriver_element`](Self::as_webdriver_element) and
/// [`into_webdriver_element`](Self::into_webdriver_element).
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::{BaseCase, BrowserConfig};
///
/// # async fn demo() -> seleniumbase_rs::Result<()> {
/// let mut sb = BaseCase::new(BrowserConfig::default()).await?;
/// sb.open("https://example.com/cart").await?;
/// let row = sb.find_element("#cart .row").await?;
/// println!("{}", row.text().await?);
///
/// // The raw `thirtyfour` element, for a call this crate has no method for.
/// let raw: &seleniumbase_rs::drivers::WebElement = row.as_webdriver_element();
/// # let _ = raw;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Element {
    inner: WebElement,
    /// The session's driver, for the scripts and the action chains.
    driver: WebDriver,
    /// How the element was found: the selector, or for an element found from
    /// another one the chain (`#cart >> .row`), as a `Locator` writes it. It
    /// names the element in errors, `Debug` and `Display`.
    selector: SelectorBuf,
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Element")
            .field("selector", &self.selector.to_string())
            .field("id", &self.inner.element_id().to_string())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Element {
    /// The selector the element was found by.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.selector.fmt(f)
    }
}

impl Element {
    /// An element the session found with `selector`.
    pub(crate) fn new(inner: WebElement, driver: WebDriver, selector: SelectorBuf) -> Self {
        Self {
            inner,
            driver,
            selector,
        }
    }

    /// The error for a failed command on this element: the driver's
    /// stale-reference answer becomes [`SeleniumBaseError::StaleElement`]
    /// naming the selector, which the raw error cannot name, and any other
    /// error is passed on.
    fn failed(&self, error: impl Into<SeleniumBaseError>) -> SeleniumBaseError {
        match error.into() {
            SeleniumBaseError::WebDriver(inner)
                if matches!(
                    inner.as_inner(),
                    WebDriverErrorInner::StaleElementReference(_)
                ) =>
            {
                SeleniumBaseError::StaleElement {
                    selector: self.selector.to_string(),
                }
            }
            other => other,
        }
    }

    /// The raw `thirtyfour` element, for a call this type has no method for.
    #[must_use]
    pub fn as_webdriver_element(&self) -> &WebElement {
        &self.inner
    }

    /// The raw `thirtyfour` element, owned.
    #[must_use]
    pub fn into_webdriver_element(self) -> WebElement {
        self.inner
    }

    /// The text the checks of Python SeleniumBase compare: the value of an
    /// `<input>` or a `<textarea>`, whose rendered text is empty, and the
    /// rendered text of any other element.
    ///
    /// Like [`Locator::text`](crate::sb_cdp::Locator::text).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn text(&self) -> Result<String, SeleniumBaseError> {
        shown_text(&self.inner)
            .await
            .map_err(|error| self.failed(error))
    }

    /// The element's outer HTML.
    ///
    /// Like [`Locator::html`](crate::sb_cdp::Locator::html).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn html(&self) -> Result<String, SeleniumBaseError> {
        self.inner
            .outer_html()
            .await
            .map_err(|error| self.failed(error))
    }

    /// The tag name, in lower case (`div`, `input`).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn tag(&self) -> Result<String, SeleniumBaseError> {
        self.inner
            .tag_name()
            .await
            .map(|tag| tag.to_ascii_lowercase())
            .map_err(|error| self.failed(error))
    }

    /// The attribute `name` as the markup has it, or `None` if the element
    /// lacks it.
    ///
    /// Like [`Locator::attribute`](crate::sb_cdp::Locator::attribute). It is
    /// not what [`BaseCase::get_attribute`](crate::BaseCase::get_attribute)
    /// answers, which follows Selenium and prefers the DOM property: use
    /// [`property`](Self::property) for what a field holds now.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn attribute(
        &self,
        name: impl AsRef<str>,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let arguments = vec![
            self.inner.to_json().map_err(|error| self.failed(error))?,
            json!(name.as_ref()),
        ];
        let value = self
            .driver
            .execute(ATTRIBUTE, arguments)
            .await
            .map_err(|error| self.failed(error))?;
        Ok(value.json().as_str().map(str::to_owned))
    }

    /// The DOM property `name`, as a string, or `None` if the element has no
    /// such property. A field's `value` is what it holds now, not the
    /// `value=""` of its markup.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn property(
        &self,
        name: impl AsRef<str>,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.inner
            .prop(name.as_ref())
            .await
            .map_err(|error| self.failed(error))
    }

    /// Position and size, relative to the document.
    ///
    /// Like [`Locator::bounding_box`](crate::sb_cdp::Locator::bounding_box).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn bounding_box(&self) -> Result<Rect, SeleniumBaseError> {
        let rect = self
            .inner
            .rect()
            .await
            .map_err(|error| self.failed(error))?;
        Ok(Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })
    }

    /// Whether the element is displayed.
    ///
    /// Like [`Locator::is_visible`](crate::sb_cdp::Locator::is_visible).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn is_visible(&self) -> Result<bool, SeleniumBaseError> {
        self.inner
            .is_displayed()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Whether the element is enabled.
    ///
    /// Like [`Locator::is_enabled`](crate::sb_cdp::Locator::is_enabled).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn is_enabled(&self) -> Result<bool, SeleniumBaseError> {
        self.inner
            .is_enabled()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Whether an `<option>`, a checkbox or a radio button is selected.
    ///
    /// Like [`Locator::is_selected`](crate::sb_cdp::Locator::is_selected).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub async fn is_selected(&self) -> Result<bool, SeleniumBaseError> {
        self.inner
            .is_selected()
            .await
            .map_err(|error| self.failed(error))
    }
}

/// What an element answers, against the mock WebDriver in `webdriver_support`.
#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use thirtyfour::prelude::By;

    use super::*;
    use crate::api::base_case::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use crate::browser::config::BrowserConfig;
    use crate::browser::session::BrowserSession;

    /// The session on `driver` and the element `#a` as it finds it.
    async fn found(driver: &MockWebDriver) -> (BrowserSession, Element) {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        let session = BrowserSession::connect(config).await.expect("connects");
        let webdriver = session.webdriver().expect("a WebDriver session").clone();
        let inner = webdriver
            .find(By::Css("#a"))
            .await
            .expect("the mock finds #a");
        let element = Element::new(inner, webdriver, SelectorBuf::css("#a"));
        (session, element)
    }

    /// A page whose `#a` is the element `e1`, a `tag` with `text` and a
    /// `value` property, and which answers `commands` the way the table says.
    fn page(tag: &'static str, text: &'static str, value: &'static str) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request: &Seen| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("GET", "/element/e1/name") => Reply::Value(json!(tag)),
                ("GET", "/element/e1/text") => Reply::Value(json!(text)),
                ("GET", "/element/e1/property/value") => Reply::Value(json!(value)),
                ("GET", "/element/e1/property/outerHTML") => {
                    Reply::Value(json!("<a id=\"a\">Go</a>"))
                }
                ("GET", "/element/e1/rect") => {
                    Reply::Value(json!({ "x": 10.0, "y": 20.0, "width": 30.0, "height": 40.0 }))
                }
                ("GET", "/element/e1/displayed" | "/element/e1/enabled") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/element/e1/selected") => Reply::Value(json!(false)),
                ("POST", "/execute/sync") => Reply::Value(json!("primary")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn assert_send_sync_clone<T: Send + Sync + Clone>() {}

    #[test]
    fn an_element_can_be_shared_between_tasks_and_cloned() {
        assert_send_sync_clone::<Element>();
    }

    #[tokio::test]
    async fn the_text_of_an_input_is_its_value() {
        // The rendered text of a field is empty; Python compares the value.
        let driver = page("INPUT", "", "typed");
        let (_session, input) = found(&driver).await;

        assert_eq!(input.text().await.unwrap(), "typed");
    }

    #[tokio::test]
    async fn the_text_of_anything_else_is_the_rendered_text() {
        let driver = page("H1", "Welcome", "ignored");
        let (_session, heading) = found(&driver).await;

        assert_eq!(heading.text().await.unwrap(), "Welcome");
        assert!(driver.requests("/element/e1/property/value").is_empty());
    }

    #[tokio::test]
    async fn the_reading_methods_ask_the_driver_for_what_they_name() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver).await;

        assert_eq!(link.html().await.unwrap(), "<a id=\"a\">Go</a>");
        assert_eq!(link.tag().await.unwrap(), "a", "the tag is in lower case");
        assert_eq!(link.property("value").await.unwrap().as_deref(), Some(""));
        let rect = link.bounding_box().await.unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (10.0, 20.0, 30.0, 40.0)
        );
        assert!(link.is_visible().await.unwrap());
        assert!(link.is_enabled().await.unwrap());
        assert!(!link.is_selected().await.unwrap());
    }

    #[tokio::test]
    async fn an_attribute_is_read_as_the_markup_has_it() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver).await;

        assert_eq!(
            link.attribute("data-kind").await.unwrap().as_deref(),
            Some("primary")
        );
        let scripts = driver.requests("/execute/sync");
        let script = scripts.last().expect("a script ran");
        assert!(
            script.body["script"]
                .as_str()
                .is_some_and(|script| script.contains("getAttribute(arguments[1])")),
            "{:?}",
            script.body
        );
        assert_eq!(script.body["args"][1], json!("data-kind"));
        assert!(
            driver
                .requests("/element/e1/attribute/data-kind")
                .is_empty(),
            "the driver's attribute command prefers the property"
        );
    }

    #[tokio::test]
    async fn debug_and_display_show_the_selector_and_the_id() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver).await;

        assert_eq!(link.to_string(), "#a");
        let debug = format!("{link:?}");
        assert!(debug.contains("selector: \"#a\""), "{debug}");
        assert!(debug.contains("id: \"e1\""), "{debug}");
    }

    #[tokio::test]
    async fn the_raw_element_stays_reachable() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver).await;

        assert_eq!(link.as_webdriver_element().element_id().to_string(), "e1");
        assert_eq!(link.into_webdriver_element().element_id().to_string(), "e1");
    }

    #[tokio::test]
    async fn a_stale_element_names_the_selector_it_was_found_by() {
        let driver = MockWebDriver::start(json!({}), |request: &Seen| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("GET", "/element/e1/displayed") => Reply::Error(404, "stale element reference"),
                ("GET", "/element/e1/enabled") => Reply::Error(404, "element not interactable"),
                _ => Reply::Value(Value::Null),
            }
        });
        let (_session, link) = found(&driver).await;

        match link.is_visible().await {
            Err(SeleniumBaseError::StaleElement { selector }) => assert_eq!(selector, "#a"),
            other => panic!("expected StaleElement, got {other:?}"),
        }
        // Any other driver error is passed on as it is.
        assert!(matches!(
            link.is_enabled().await,
            Err(SeleniumBaseError::WebDriver(_))
        ));
    }
}
