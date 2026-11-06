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

use serde_json::{json, Value};
use thirtyfour::common::keys::Key as WireKey;
use thirtyfour::components::SelectElement;
use thirtyfour::error::WebDriverErrorInner;
use thirtyfour::prelude::{By, WebDriver, WebElement};

use crate::browser::session::shown_text;
use crate::browser::shadow_find;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Key, Rect, SelectBy};
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

    /// The element as the page scripts take it in their arguments.
    pub(crate) fn reference(&self) -> Result<Value, SeleniumBaseError> {
        self.inner.to_json().map_err(|error| self.failed(error))
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

    /// The text the driver renders for the element, as `BaseCase::get_text`
    /// answers: empty for an `<input>` or a `<textarea>`, whose value
    /// [`text`](Self::text) reads.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error.
    pub(crate) async fn rendered_text(&self) -> Result<String, SeleniumBaseError> {
        self.inner.text().await.map_err(|error| self.failed(error))
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
        let arguments = vec![self.reference()?, json!(name.as_ref())];
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

/// The key a WebDriver session types for `key`.
fn wire_key(key: Key) -> WireKey {
    match key {
        Key::Enter => WireKey::Enter,
        Key::Tab => WireKey::Tab,
        Key::Escape => WireKey::Escape,
        Key::Backspace => WireKey::Backspace,
        Key::Delete => WireKey::Delete,
        Key::ArrowUp => WireKey::Up,
        Key::ArrowDown => WireKey::Down,
        Key::ArrowLeft => WireKey::Left,
        Key::ArrowRight => WireKey::Right,
        Key::Home => WireKey::Home,
        Key::End => WireKey::End,
        Key::PageUp => WireKey::PageUp,
        Key::PageDown => WireKey::PageDown,
    }
}

/// The actions: what a test does with the element.
///
/// None of them waits for the element to be ready, since an `Element` is
/// already found: an element that is covered, hidden or disabled fails with
/// the driver's error. [`BaseCase`](crate::BaseCase) methods by selector wait
/// first.
impl Element {
    /// Clicks the element.
    ///
    /// Like [`Locator::click`](crate::sb_cdp::Locator::click), without the
    /// wait.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::StaleElement`] if the page replaced the element,
    /// or the driver's error, such as a click another element intercepts.
    pub async fn click(&self) -> Result<(), SeleniumBaseError> {
        self.inner.click().await.map_err(|error| self.failed(error))
    }

    /// Double-clicks the element.
    ///
    /// Like [`Locator::double_click`](crate::sb_cdp::Locator::double_click).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn double_click(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .action_chain()
            .double_click_element(&self.inner)
            .perform()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Right-clicks the element (opens its context menu).
    ///
    /// Like [`Locator::right_click`](crate::sb_cdp::Locator::right_click).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn right_click(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .action_chain()
            .context_click_element(&self.inner)
            .perform()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Moves the pointer over the centre of the element.
    ///
    /// Like [`Locator::hover`](crate::sb_cdp::Locator::hover).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn hover(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .action_chain()
            .move_to_element_center(&self.inner)
            .perform()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Empties the field.
    ///
    /// Like [`Locator::clear`](crate::sb_cdp::Locator::clear).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn clear(&self) -> Result<(), SeleniumBaseError> {
        self.inner.clear().await.map_err(|error| self.failed(error))
    }

    /// Types `text` into the element, after what is there.
    ///
    /// Python's `send_keys`. Like
    /// [`Locator::send_keys`](crate::sb_cdp::Locator::send_keys). Use
    /// [`type_text`](Self::type_text) to replace what the field holds instead.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn send_keys(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.inner
            .send_keys(text.as_ref())
            .await
            .map_err(|error| self.failed(error))
    }

    /// Empties the field, then types `text` into it.
    ///
    /// Python's `type`. Like
    /// [`Locator::type_text`](crate::sb_cdp::Locator::type_text).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn type_text(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.clear().await?;
        self.send_keys(text).await
    }

    /// Presses `key` with the element focused.
    ///
    /// Like [`Locator::press`](crate::sb_cdp::Locator::press), and it takes
    /// the same [`Key`].
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn press(&self, key: Key) -> Result<(), SeleniumBaseError> {
        self.inner
            .send_keys(wire_key(key))
            .await
            .map_err(|error| self.failed(error))
    }

    /// Chooses an option of a `<select>` element.
    ///
    /// Like [`Locator::select_option`](crate::sb_cdp::Locator::select_option),
    /// and it takes the same [`SelectBy`].
    ///
    /// # Errors
    ///
    /// The driver's error, `no such element`, if the `<select>` has no such
    /// option; and see [`click`](Self::click).
    pub async fn select_option(&self, by: SelectBy<'_>) -> Result<(), SeleniumBaseError> {
        let select = SelectElement::new(&self.inner)
            .await
            .map_err(|error| self.failed(error))?;
        let chosen = match by {
            SelectBy::Text(text) => select.select_by_visible_text(text).await,
            SelectBy::Value(value) => select.select_by_value(value).await,
            SelectBy::Index(index) => {
                let index = u32::try_from(index).map_err(|_| {
                    SeleniumBaseError::InvalidSelector(format!("option index {index} is too large"))
                })?;
                select.select_by_index(index).await
            }
        };
        chosen.map_err(|error| self.failed(error))
    }

    /// Submits the form that contains the element.
    ///
    /// Like [`Locator::submit`](crate::sb_cdp::Locator::submit).
    ///
    /// # Errors
    ///
    /// The driver's error if the element is not inside a form; and see
    /// [`click`](Self::click).
    pub async fn submit(&self) -> Result<(), SeleniumBaseError> {
        self.driver
            .execute(
                "arguments[0].closest('form').submit()",
                vec![self.reference()?],
            )
            .await
            .map_err(|error| self.failed(error))?;
        Ok(())
    }

    /// Scrolls the element into view.
    ///
    /// Like
    /// [`Locator::scroll_into_view`](crate::sb_cdp::Locator::scroll_into_view).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn scroll_into_view(&self) -> Result<(), SeleniumBaseError> {
        self.inner
            .scroll_into_view()
            .await
            .map_err(|error| self.failed(error))
    }

    /// Drags the element onto `target`.
    ///
    /// Like [`Locator::drag_to`](crate::sb_cdp::Locator::drag_to).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click); a stale `target` is reported with the
    /// selector of the element being dragged.
    pub async fn drag_to(&self, target: &Element) -> Result<(), SeleniumBaseError> {
        self.driver
            .action_chain()
            .drag_and_drop_element(&self.inner, &target.inner)
            .perform()
            .await
            .map_err(|error| self.failed(error))
    }
}

/// Finding other elements from this one.
///
/// Each looks once, since the element is already found: a search that finds
/// nothing is an error for [`find`](Self::find) and an empty list for
/// [`find_all`](Self::find_all). The elements found are named by the chain
/// that led to them (`#cart >> .row`), as a
/// [`Locator`](crate::sb_cdp::Locator) writes its chain, so a stale one says
/// where it came from.
impl Element {
    /// The name of what `selector` finds inside this element: the chain.
    fn below(&self, selector: &SelectorBuf) -> SelectorBuf {
        SelectorBuf::css(format!("{self} >> {selector}"))
    }

    /// The locator for a search inside this element.
    ///
    /// A selector that crosses a shadow root is cut and followed from the
    /// page, which an element cannot do from where it is: it is refused here
    /// with a hint, not left to the driver.
    fn lookup(selector: &SelectorBuf) -> Result<By, SeleniumBaseError> {
        let by = shadow_find::by_of(selector)?;
        if shadow_find::pieces(&by)?.is_some() {
            return Err(SeleniumBaseError::InvalidSelector(format!(
                "'{selector}' crosses a shadow root, which a search inside an element does not \
                 enter: look it up from the page, as `BaseCase::find_element` does"
            )));
        }
        Ok(by)
    }

    /// The first match of `selector` inside this element, looked for once.
    ///
    /// `selector` is anything that converts to a [`SelectorBuf`]: a string,
    /// whose kind is read as everywhere else (`.remove`, `//td`, `link=Edit`),
    /// or a [`SelectorBuf`] that names its kind. Like
    /// [`Locator::locator`](crate::sb_cdp::Locator::locator), without the
    /// laziness.
    ///
    /// # Errors
    ///
    /// The driver's `no such element` error if nothing matches,
    /// [`SeleniumBaseError::InvalidSelector`] for an empty selector or one
    /// that crosses a shadow root, and
    /// [`SeleniumBaseError::StaleElement`] if this element went stale.
    pub async fn find(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<Element, SeleniumBaseError> {
        let selector = selector.into();
        let by = Self::lookup(&selector)?;
        let found = self
            .inner
            .find(by)
            .await
            .map_err(|error| self.failed(error))?;
        Ok(Self::new(found, self.driver.clone(), self.below(&selector)))
    }

    /// Every match of `selector` inside this element, possibly none.
    ///
    /// # Errors
    ///
    /// As [`find`](Self::find), except that nothing matching is an empty list.
    pub async fn find_all(
        &self,
        selector: impl Into<SelectorBuf>,
    ) -> Result<Vec<Element>, SeleniumBaseError> {
        let selector = selector.into();
        let by = Self::lookup(&selector)?;
        let found = self
            .inner
            .find_all(by)
            .await
            .map_err(|error| self.failed(error))?;
        let name = self.below(&selector);
        Ok(found
            .into_iter()
            .map(|inner| Self::new(inner, self.driver.clone(), name.clone()))
            .collect())
    }

    /// The element this one is in.
    ///
    /// Like [`Locator::parent`](crate::sb_cdp::Locator::parent), without the
    /// laziness.
    ///
    /// # Errors
    ///
    /// The driver's error if there is no parent, as on the document element;
    /// and see [`find`](Self::find).
    pub async fn parent(&self) -> Result<Element, SeleniumBaseError> {
        let parent = self
            .inner
            .parent()
            .await
            .map_err(|error| self.failed(error))?;
        let name = self.below(&SelectorBuf::xpath(".."));
        Ok(Self::new(parent, self.driver.clone(), name))
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
    async fn found(driver: &MockWebDriver, css: &str) -> (BrowserSession, Element) {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        let session = BrowserSession::connect(config).await.expect("connects");
        let webdriver = session.webdriver().expect("a WebDriver session").clone();
        let inner = webdriver
            .find(By::Css(css))
            .await
            .expect("the mock finds #a");
        let element = Element::new(inner, webdriver, SelectorBuf::css(css));
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

    /// A page with the elements `#a` (`e1`), `#b` (`e2`) and the `<select>`
    /// `#s` (`s1`), whose option `o1` is not selected.
    fn form() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request: &Seen| {
            let asked = request.body["value"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(match asked {
                    "#b" => element("e2"),
                    "#s" => element("s1"),
                    _ => element("e1"),
                }),
                ("POST", "/element/s1/elements") => Reply::Value(json!([element("o1")])),
                ("POST", "/element/s1/element") => Reply::Value(element("o1")),
                ("GET", "/element/o1/selected") => Reply::Value(json!(false)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The body of the one request the driver got for `path`.
    fn only_request(driver: &MockWebDriver, path: &str) -> Value {
        let sent = driver.requests(path);
        assert_eq!(sent.len(), 1, "{path}: {sent:?}");
        sent[0].body.clone()
    }

    #[tokio::test]
    async fn click_clear_and_scroll_send_the_commands_for_the_element() {
        let driver = form();
        let (_session, field) = found(&driver, "#a").await;

        field.click().await.unwrap();
        field.clear().await.unwrap();
        field.scroll_into_view().await.unwrap();

        only_request(&driver, "/element/e1/click");
        only_request(&driver, "/element/e1/clear");
        let script = only_request(&driver, "/execute/sync");
        assert!(
            script["script"]
                .as_str()
                .is_some_and(|script| script.contains("scrollIntoView")),
            "{script}"
        );
    }

    #[tokio::test]
    async fn send_keys_types_after_what_is_there_and_type_text_clears_first() {
        let driver = form();
        let (_session, field) = found(&driver, "#a").await;

        field.send_keys("abc").await.unwrap();
        field.type_text("xyz").await.unwrap();

        let commands: Vec<_> = driver
            .all_requests()
            .into_iter()
            .filter(|request| request.path.ends_with("/clear") || request.path.ends_with("/value"))
            .map(|request| (request.path, request.body["text"].clone()))
            .collect();
        assert_eq!(
            commands,
            [
                ("/element/e1/value".to_owned(), json!("abc")),
                ("/element/e1/clear".to_owned(), Value::Null),
                ("/element/e1/value".to_owned(), json!("xyz")),
            ]
        );
    }

    #[tokio::test]
    async fn press_types_the_key_the_driver_knows() {
        let driver = form();
        let (_session, field) = found(&driver, "#a").await;

        field.press(Key::Enter).await.unwrap();
        field.press(Key::ArrowDown).await.unwrap();

        let typed: Vec<_> = driver
            .requests("/element/e1/value")
            .into_iter()
            .map(|request| request.body["text"].clone())
            .collect();
        assert_eq!(typed, [json!("\u{e007}"), json!("\u{e015}")]);
    }

    #[tokio::test]
    async fn the_pointer_actions_are_action_chains_on_the_element() {
        let driver = form();
        let (_session, field) = found(&driver, "#a").await;

        field.hover().await.unwrap();
        let hovered = only_request(&driver, "/actions").to_string();
        assert!(hovered.contains("pointerMove"), "{hovered}");
        assert!(hovered.contains("e1"), "{hovered}");
        assert!(!hovered.contains("pointerDown"), "{hovered}");

        field.double_click().await.unwrap();
        field.right_click().await.unwrap();
        let chains: Vec<_> = driver
            .requests("/actions")
            .into_iter()
            .map(|request| request.body.to_string())
            .collect();
        assert_eq!(chains[1].matches("pointerDown").count(), 2, "{}", chains[1]);
        assert!(chains[2].contains("\"button\":2"), "{}", chains[2]);
    }

    #[tokio::test]
    async fn dragging_moves_from_the_element_to_the_target() {
        let driver = form();
        let (_session, source) = found(&driver, "#a").await;
        let (_other, target) = found(&driver, "#b").await;

        source.drag_to(&target).await.unwrap();

        let chain = only_request(&driver, "/actions").to_string();
        let (from, to) = (chain.find("e1"), chain.find("e2"));
        assert!(from.is_some() && from < to, "{chain}");
    }

    #[tokio::test]
    async fn submit_submits_the_form_of_the_element() {
        let driver = form();
        let (_session, field) = found(&driver, "#a").await;

        field.submit().await.unwrap();

        let script = only_request(&driver, "/execute/sync");
        assert!(
            script["script"]
                .as_str()
                .is_some_and(|script| script.contains("closest('form').submit()")),
            "{script}"
        );
        assert_eq!(script["args"][0], element("e1"));
    }

    #[tokio::test]
    async fn select_option_clicks_the_option_each_kind_of_choice_names() {
        let driver = form();
        let (_session, dropdown) = found(&driver, "#s").await;

        dropdown.select_option(SelectBy::Text("Two")).await.unwrap();
        dropdown.select_option(SelectBy::Value("b")).await.unwrap();
        dropdown.select_option(SelectBy::Index(1)).await.unwrap();

        assert_eq!(driver.requests("/element/o1/click").len(), 3);
        let looked_for: Vec<_> = driver
            .all_requests()
            .into_iter()
            .filter(|request| {
                request.method == "POST" && request.path.starts_with("/element/s1/element")
            })
            .map(|request| {
                request.body["value"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        assert!(
            looked_for[0].contains("normalize-space(.) = \"Two\""),
            "{looked_for:?}"
        );
        assert!(
            looked_for[1].contains("option[value=\"b\"]"),
            "{looked_for:?}"
        );
        assert_eq!(looked_for[2], "option:nth-of-type(2)");
    }

    /// A cart: `#a` (`e1`) holds the rows `r1`, which has gone stale, and `r2`,
    /// and its parent is `p1`. `#b` (`e2`) has gone stale itself.
    fn cart() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request: &Seen| {
            let asked = request.body["value"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(match asked {
                    "#b" => element("e2"),
                    _ => element("e1"),
                }),
                ("POST", "/element/e1/element") => match asked {
                    ".row" => Reply::Value(element("r1")),
                    "./.." | ".." => Reply::Value(element("p1")),
                    _ => Reply::Error(404, "no such element"),
                },
                ("POST", "/element/e1/elements") => match asked {
                    ".row" => Reply::Value(json!([element("r1"), element("r2")])),
                    _ => Reply::Value(json!([])),
                },
                ("POST", "/element/e2/element" | "/element/e2/elements") => {
                    Reply::Error(404, "stale element reference")
                }
                ("POST", "/element/r1/element") => Reply::Value(element("c1")),
                ("POST", "/execute/sync") => Reply::Value(element("p1")),
                ("GET", "/element/r1/displayed") => Reply::Error(404, "stale element reference"),
                ("GET", "/element/r2/displayed" | "/element/p1/displayed") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn find_looks_once_inside_the_element_and_names_the_chain() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;

        let row = cart.find(".row").await.unwrap();

        assert_eq!(row.to_string(), "#a >> .row");
        assert_eq!(row.as_webdriver_element().element_id().to_string(), "r1");
        let asked = only_request(&driver, "/element/e1/element");
        assert_eq!(asked["using"], "css selector");
        assert_eq!(asked["value"], ".row");
        // The chain goes on from there.
        let cell = row.find("td").await.unwrap();
        assert_eq!(cell.to_string(), "#a >> .row >> td");
    }

    #[tokio::test]
    async fn a_search_reads_the_kind_of_its_selector() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;

        // Nothing matches, and the driver's error is passed on.
        let error = cart.find("//td").await.unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::WebDriver(_)),
            "{error:?}"
        );
        cart.find("link=Edit").await.unwrap_err();

        let kinds: Vec<_> = driver
            .requests("/element/e1/element")
            .into_iter()
            .map(|request| {
                request.body["using"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        assert_eq!(kinds, ["xpath", "link text"]);
    }

    #[tokio::test]
    async fn find_all_returns_every_match_and_no_match_is_an_empty_list() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;

        let rows = cart.find_all(".row").await.unwrap();
        let ids: Vec<_> = rows
            .iter()
            .map(|row| row.as_webdriver_element().element_id().to_string())
            .collect();
        assert_eq!(ids, ["r1", "r2"]);
        assert!(rows.iter().all(|row| row.to_string() == "#a >> .row"));

        assert!(cart.find_all(".nothing").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn parent_is_the_element_above_and_is_named_so() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;

        let parent = cart.parent().await.unwrap();

        assert_eq!(parent.to_string(), "#a >> ..");
        assert_eq!(parent.as_webdriver_element().element_id().to_string(), "p1");
        assert!(parent.is_visible().await.unwrap());
    }

    #[tokio::test]
    async fn an_element_found_from_another_goes_stale_naming_the_chain() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;
        let rows = cart.find_all(".row").await.unwrap();

        // The mock says `r1` is stale and `r2` is not; both are `#a >> .row`.
        match rows[0].is_visible().await {
            Err(SeleniumBaseError::StaleElement { selector }) => {
                assert_eq!(selector, "#a >> .row");
            }
            other => panic!("expected StaleElement, got {other:?}"),
        }
        assert!(rows[1].is_visible().await.unwrap());
    }

    #[tokio::test]
    async fn a_search_from_a_stale_element_names_that_element() {
        let driver = cart();
        let (_session, gone) = found(&driver, "#b").await;

        for error in [
            gone.find(".row").await.unwrap_err(),
            gone.find_all(".row").await.unwrap_err(),
        ] {
            match error {
                SeleniumBaseError::StaleElement { selector } => assert_eq!(selector, "#b"),
                other => panic!("expected StaleElement, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn a_search_that_cannot_work_is_refused_before_the_driver_is_asked() {
        let driver = cart();
        let (_session, cart) = found(&driver, "#a").await;

        for selector in ["", "  ", "host ::shadow", "host::shadow .row"] {
            let error = cart.find(selector).await.unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidSelector(_)),
                "{selector:?}: {error:?}"
            );
            cart.find_all(selector).await.unwrap_err();
        }
        let message = cart
            .find("host::shadow .row")
            .await
            .unwrap_err()
            .to_string();
        assert!(message.contains("find_element"), "{message}");
        assert!(driver.requests("/element/e1/element").is_empty());
        assert!(driver.requests("/element/e1/elements").is_empty());
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
        let (_session, input) = found(&driver, "#a").await;

        assert_eq!(input.text().await.unwrap(), "typed");
    }

    #[tokio::test]
    async fn the_text_of_anything_else_is_the_rendered_text() {
        let driver = page("H1", "Welcome", "ignored");
        let (_session, heading) = found(&driver, "#a").await;

        assert_eq!(heading.text().await.unwrap(), "Welcome");
        assert!(driver.requests("/element/e1/property/value").is_empty());
    }

    #[tokio::test]
    async fn the_reading_methods_ask_the_driver_for_what_they_name() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver, "#a").await;

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
        let (_session, link) = found(&driver, "#a").await;

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
        let (_session, link) = found(&driver, "#a").await;

        assert_eq!(link.to_string(), "#a");
        let debug = format!("{link:?}");
        assert!(debug.contains("selector: \"#a\""), "{debug}");
        assert!(debug.contains("id: \"e1\""), "{debug}");
    }

    #[tokio::test]
    async fn the_raw_element_stays_reachable() {
        let driver = page("A", "Go", "");
        let (_session, link) = found(&driver, "#a").await;

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
        let (_session, link) = found(&driver, "#a").await;

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
