//! The [`Locator`]: a lazy, auto-waiting handle to elements on a page.

use std::fmt;
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Value};

use super::expect::LocatorExpect;
use super::input::Key;
use super::page::Page;
use super::types::{ElementInfo, Point, Rect, SelectBy, State};
use crate::error::SeleniumBaseError;
use crate::utils::selectors::SelectorBuf;

/// One step of a locator chain: find within the previous step's matches.
///
/// Serialised as-is and resolved by the page helper, which is why the field
/// names match `helper.js`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Step {
    selector: SelectorBuf,
    visible: bool,
    nth: Option<usize>,
    last: bool,
}

impl Step {
    fn new(selector: SelectorBuf) -> Self {
        Self {
            selector,
            visible: false,
            nth: None,
            last: false,
        }
    }
}

/// Finds elements on a page, lazily, and acts on them.
///
/// A locator remembers *how* to find elements, not which element it found.
/// Every use looks again, so it keeps working when the page re-renders, and
/// actions wait for the element to be ready instead of failing at once.
///
/// Narrowing a locator returns a new one, so `page.locator("li").visible().nth(2)`
/// reads left to right. This replaces the many near-duplicate methods other
/// frameworks offer (click the nth visible element, click only if visible, click
/// a child of a parent, and so on): compose them instead.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::Page;
///
/// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// // Fill a form and submit it.
/// page.locator("#username").fill("demo_user").await?;
/// page.locator("#password").fill("secret_pass").await?;
/// page.locator("button[type=submit]").click().await?;
///
/// // The third visible item of a list.
/// let third = page.locator("ul.results > li").visible().nth(2);
/// println!("{}", third.text().await?);
///
/// // Click a button only when the banner is showing.
/// let dismiss = page.locator(".cookie-banner button");
/// if dismiss.is_visible().await? {
///     dismiss.click().await?;
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Locator {
    page: Page,
    chain: Vec<Step>,
    timeout: Duration,
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, step) in self.chain.iter().enumerate() {
            if index > 0 {
                f.write_str(" >> ")?;
            }
            write!(f, "{}", step.selector)?;
            let mut modifiers = Vec::new();
            if step.visible {
                modifiers.push("visible".to_owned());
            }
            if let Some(nth) = step.nth {
                modifiers.push(format!("nth={nth}"));
            }
            if step.last {
                modifiers.push("last".to_owned());
            }
            if !modifiers.is_empty() {
                write!(f, " [{}]", modifiers.join(", "))?;
            }
        }
        Ok(())
    }
}

impl Locator {
    pub(crate) fn new(page: Page, selector: SelectorBuf) -> Self {
        let timeout = page.timeout();
        Self {
            page,
            chain: vec![Step::new(selector)],
            timeout,
        }
    }

    /// The page this locator searches.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// How long actions on this locator wait for an element to be ready.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    // ------------------------------------------------------------------
    // Narrowing
    // ------------------------------------------------------------------

    /// Returns a locator that waits `timeout` instead of the page default.
    #[must_use]
    pub fn with_timeout(&self, timeout: Duration) -> Self {
        Self {
            timeout,
            ..self.clone()
        }
    }

    /// Searches *inside* the elements this locator matches.
    #[must_use]
    pub fn locator(&self, selector: impl Into<SelectorBuf>) -> Self {
        let mut next = self.clone();
        next.chain.push(Step::new(selector.into()));
        next
    }

    /// Keeps only the match at `index` (zero-based).
    #[must_use]
    pub fn nth(&self, index: usize) -> Self {
        self.narrowed(|step| {
            step.nth = Some(index);
            step.last = false;
        })
    }

    /// Keeps only the first match.
    #[must_use]
    pub fn first(&self) -> Self {
        self.nth(0)
    }

    /// Keeps only the last match.
    #[must_use]
    pub fn last(&self) -> Self {
        self.narrowed(|step| {
            step.last = true;
            step.nth = None;
        })
    }

    /// Keeps only matches that are currently visible.
    ///
    /// Applied before [`nth`](Self::nth), so `visible().nth(2)` is the third
    /// *visible* element.
    #[must_use]
    pub fn visible(&self) -> Self {
        self.narrowed(|step| step.visible = true)
    }

    /// Moves to the parent of each match.
    #[must_use]
    pub fn parent(&self) -> Self {
        self.locator(SelectorBuf::xpath(".."))
    }

    fn narrowed(&self, change: impl FnOnce(&mut Step)) -> Self {
        let mut next = self.clone();
        if let Some(step) = next.chain.last_mut() {
            change(step);
        }
        next
    }

    // ------------------------------------------------------------------
    // Resolution
    // ------------------------------------------------------------------

    fn chain_json(&self) -> Result<String, SeleniumBaseError> {
        Ok(serde_json::to_string(&self.chain)?)
    }

    /// Runs `body` with the first match bound to `el`.
    async fn with_element(&self, body: &str) -> Result<Value, SeleniumBaseError> {
        let script = format!(
            "(() => {{ const el = __sbcdp.one({}); {body} }})()",
            self.chain_json()?
        );
        self.page.eval_labelled(&self.to_string(), &script).await
    }

    /// How many elements match right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn count(&self) -> Result<usize, SeleniumBaseError> {
        let count = self
            .page
            .eval_labelled(
                &self.to_string(),
                &format!("__sbcdp.resolve({}).length", self.chain_json()?),
            )
            .await?;
        Ok(count
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_default())
    }

    /// Whether the locator is currently in `state`. Does not wait.
    pub(crate) async fn is_in_state(&self, state: State) -> Result<bool, SeleniumBaseError> {
        let probe = match state {
            State::Present => format!("__sbcdp.resolve({}).length > 0", self.chain_json()?),
            State::Absent => format!("__sbcdp.resolve({}).length === 0", self.chain_json()?),
            State::Visible => format!(
                "__sbcdp.resolve({}).some(__sbcdp.visible)",
                self.chain_json()?
            ),
            State::Hidden => format!(
                "!__sbcdp.resolve({}).some(__sbcdp.visible)",
                self.chain_json()?
            ),
        };
        Ok(self.page.eval_labelled(&self.to_string(), &probe).await? == Value::Bool(true))
    }

    /// Waits until the locator reaches `state`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if it does not in time.
    pub async fn wait_for(&self, state: State) -> Result<(), SeleniumBaseError> {
        let reached = self
            .page
            .poll(self.timeout, || async { self.is_in_state(state).await })
            .await?;
        if reached {
            Ok(())
        } else {
            Err(SeleniumBaseError::wait_timeout(
                format!("{self} {}", state.describe()),
                Some(self.timeout),
            ))
        }
    }

    /// Waits for a match to exist, so reads fail with "not found".
    async fn wait_present(&self) -> Result<(), SeleniumBaseError> {
        if self.wait_for(State::Present).await.is_ok() {
            Ok(())
        } else {
            Err(SeleniumBaseError::element_not_found(self.to_string()))
        }
    }

    /// Waits for a match to be visible, so actions fail with a useful reason.
    async fn wait_actionable(&self) -> Result<(), SeleniumBaseError> {
        if self.wait_for(State::Visible).await.is_ok() {
            return Ok(());
        }
        if self.is_in_state(State::Present).await? {
            Err(SeleniumBaseError::element_not_interactable(
                self.to_string(),
                format!("it was still not visible after {:?}", self.timeout),
            ))
        } else {
            Err(SeleniumBaseError::element_not_found(self.to_string()))
        }
    }

    /// Scrolls the first match into view and returns where its centre is.
    async fn actionable_center(&self) -> Result<Point, SeleniumBaseError> {
        self.wait_actionable().await?;
        let center = self.with_element("return __sbcdp.center(el);").await?;
        Ok(Point {
            x: center["x"].as_f64().unwrap_or_default(),
            y: center["y"].as_f64().unwrap_or_default(),
        })
    }

    /// One locator per current match, each pinned to its position.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn all(&self) -> Result<Vec<Self>, SeleniumBaseError> {
        Ok((0..self.count().await?)
            .map(|index| self.nth(index))
            .collect())
    }

    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    /// Everything about the first match at this moment.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn info(&self) -> Result<ElementInfo, SeleniumBaseError> {
        self.wait_present().await?;
        let value = self.with_element("return __sbcdp.info(el);").await?;
        Ok(parse_info(&value))
    }

    /// Snapshots every current match. Does not wait; empty if nothing matches.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn infos(&self) -> Result<Vec<ElementInfo>, SeleniumBaseError> {
        let value = self
            .page
            .eval_labelled(
                &self.to_string(),
                &format!("__sbcdp.resolve({}).map(__sbcdp.info)", self.chain_json()?),
            )
            .await?;
        Ok(value
            .as_array()
            .map(|items| items.iter().map(parse_info).collect())
            .unwrap_or_default())
    }

    /// The rendered text of the first match.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn text(&self) -> Result<String, SeleniumBaseError> {
        self.wait_present().await?;
        let value = self.with_element("return __sbcdp.textOf(el);").await?;
        Ok(value.as_str().unwrap_or_default().to_owned())
    }

    /// The outer HTML of the first match.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn html(&self) -> Result<String, SeleniumBaseError> {
        self.wait_present().await?;
        let value = self.with_element("return el.outerHTML;").await?;
        Ok(value.as_str().unwrap_or_default().to_owned())
    }

    /// The value of attribute `name`, or `None` if the element lacks it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn attribute(
        &self,
        name: impl AsRef<str>,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.wait_present().await?;
        let name = serde_json::to_string(name.as_ref())?;
        let value = self
            .with_element(&format!("return el.getAttribute({name});"))
            .await?;
        Ok(value.as_str().map(str::to_owned))
    }

    /// Position and size of the first match, relative to the document.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn bounding_box(&self) -> Result<Rect, SeleniumBaseError> {
        Ok(self.info().await?.rect)
    }

    /// Whether any match is visible right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn is_visible(&self) -> Result<bool, SeleniumBaseError> {
        self.is_in_state(State::Visible).await
    }

    /// Whether any element matches right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn exists(&self) -> Result<bool, SeleniumBaseError> {
        self.is_in_state(State::Present).await
    }

    /// Whether a checkbox or radio button is checked.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn is_checked(&self) -> Result<bool, SeleniumBaseError> {
        self.wait_present().await?;
        Ok(self.with_element("return !!el.checked;").await? == Value::Bool(true))
    }

    /// Whether an `<option>`, checkbox or radio button is selected.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn is_selected(&self) -> Result<bool, SeleniumBaseError> {
        self.wait_present().await?;
        Ok(self
            .with_element("return !!(el.selected || el.checked);")
            .await?
            == Value::Bool(true))
    }

    /// Whether the control accepts input.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn is_enabled(&self) -> Result<bool, SeleniumBaseError> {
        self.wait_present().await?;
        Ok(self.with_element("return !el.disabled;").await? == Value::Bool(true))
    }

    // ------------------------------------------------------------------
    // Pointer actions
    // ------------------------------------------------------------------

    /// Clicks the centre of the first match.
    ///
    /// Waits for the element to be visible, scrolls it into view, and clicks
    /// with real mouse events.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] or
    /// [`SeleniumBaseError::ElementNotInteractable`] if the element does not
    /// become ready in time.
    pub async fn click(&self) -> Result<(), SeleniumBaseError> {
        let at = self.actionable_center().await?;
        self.page.mouse().click(at).await
    }

    /// Double-clicks the first match.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn double_click(&self) -> Result<(), SeleniumBaseError> {
        let at = self.actionable_center().await?;
        self.page.mouse().double_click(at).await
    }

    /// Right-clicks the first match.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn right_click(&self) -> Result<(), SeleniumBaseError> {
        let at = self.actionable_center().await?;
        self.page.mouse().right_click(at).await
    }

    /// Moves the pointer over the first match.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn hover(&self) -> Result<(), SeleniumBaseError> {
        let at = self.actionable_center().await?;
        self.page.mouse().move_to(at).await
    }

    /// Drags the first match onto the first match of `target`.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click); applies to both locators.
    pub async fn drag_to(&self, target: &Locator) -> Result<(), SeleniumBaseError> {
        let from = self.actionable_center().await?;
        let to = target.actionable_center().await?;
        self.page.mouse().drag(from, to).await
    }

    // ------------------------------------------------------------------
    // Keyboard and form actions
    // ------------------------------------------------------------------

    /// Gives the first match keyboard focus.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn focus(&self) -> Result<(), SeleniumBaseError> {
        self.wait_actionable().await?;
        self.with_element("__sbcdp.focus(el); return null;").await?;
        Ok(())
    }

    /// Empties the field.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn clear(&self) -> Result<(), SeleniumBaseError> {
        self.wait_actionable().await?;
        self.with_element("__sbcdp.focus(el); __sbcdp.setValue(el, ''); return null;")
            .await?;
        Ok(())
    }

    /// Replaces the field's content with `text`, inserted in one step.
    ///
    /// Fast, and the page still receives genuine `input` events. It sends no
    /// per-key events; use [`type_text`](Self::type_text) where those matter.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn fill(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.clear().await?;
        self.page.keyboard().insert_text(text).await
    }

    /// Types `text` into the field one key at a time, after what is there.
    ///
    /// Call [`clear`](Self::clear) first to replace instead of append.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn type_text(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.focus().await?;
        self.page.keyboard().type_text(text).await
    }

    /// Sets the control's value directly and fires `input` and `change`.
    ///
    /// For controls that ignore key events, such as range sliders.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn set_value(&self, value: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.wait_actionable().await?;
        let value = serde_json::to_string(value.as_ref())?;
        self.with_element(&format!(
            "__sbcdp.focus(el); __sbcdp.setValue(el, {value}); return null;"
        ))
        .await?;
        Ok(())
    }

    /// Focuses the first match and presses `key`.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn press(&self, key: Key) -> Result<(), SeleniumBaseError> {
        self.focus().await?;
        self.page.keyboard().press(key).await
    }

    /// Chooses an option in a `<select>` dropdown.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if there is no such
    /// option.
    pub async fn select_option(&self, by: SelectBy<'_>) -> Result<(), SeleniumBaseError> {
        self.wait_actionable().await?;
        let (kind, wanted) = match by {
            SelectBy::Text(text) => ("text", text.to_owned()),
            SelectBy::Value(value) => ("value", value.to_owned()),
            SelectBy::Index(index) => ("index", index.to_string()),
        };
        let wanted = serde_json::to_string(&wanted)?;
        self.with_element(&format!(
            "__sbcdp.selectOption(el, '{kind}', {wanted}); return null;"
        ))
        .await?;
        Ok(())
    }

    /// Makes a checkbox or radio button checked, clicking only if needed.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn check(&self) -> Result<(), SeleniumBaseError> {
        self.set_checked(true).await
    }

    /// Makes a checkbox unchecked, clicking only if needed.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn uncheck(&self) -> Result<(), SeleniumBaseError> {
        self.set_checked(false).await
    }

    /// Sets a checkbox to `checked`, clicking only if it differs.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn set_checked(&self, checked: bool) -> Result<(), SeleniumBaseError> {
        if self.is_checked().await? != checked {
            self.click().await?;
        }
        Ok(())
    }

    /// Submits the form that contains the first match.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotInteractable`] if the element is
    /// not inside a form.
    pub async fn submit(&self) -> Result<(), SeleniumBaseError> {
        self.wait_present().await?;
        self.with_element(
            "const form = el.form || el.closest('form'); \
             if (!form) throw new Error('sbcdp:not-interactable:the element is not inside a form'); \
             form.requestSubmit(); return null;",
        )
        .await?;
        Ok(())
    }

    /// Scrolls the first match to the middle of the viewport.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn scroll_into_view(&self) -> Result<(), SeleniumBaseError> {
        self.wait_present().await?;
        self.with_element(
            "el.scrollIntoView({block:'center', inline:'center', behavior:'instant'}); return null;",
        )
        .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Page changes and capture
    // ------------------------------------------------------------------

    /// Sets an attribute on every current match.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn set_attribute(
        &self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let name = serde_json::to_string(name.as_ref())?;
        let value = serde_json::to_string(value.as_ref())?;
        self.page
            .eval_labelled(
                &self.to_string(),
                &format!(
                    "__sbcdp.resolve({}).forEach(e => e.setAttribute({name}, {value}))",
                    self.chain_json()?
                ),
            )
            .await?;
        Ok(())
    }

    /// Removes every current match from the page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the selector is invalid.
    pub async fn remove(&self) -> Result<(), SeleniumBaseError> {
        self.page
            .eval_labelled(
                &self.to_string(),
                &format!(
                    "__sbcdp.resolve({}).forEach(e => e.remove())",
                    self.chain_json()?
                ),
            )
            .await?;
        Ok(())
    }

    /// Briefly outlines the first match, to show where a script is acting.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn flash(&self) -> Result<(), SeleniumBaseError> {
        self.wait_present().await?;
        self.with_element("__sbcdp.flash(el); return null;").await?;
        Ok(())
    }

    /// Captures the first match as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in
    /// time, or [`SeleniumBaseError::CdpDriver`] if the browser cannot capture.
    pub async fn screenshot(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.scroll_into_view().await?;
        let rect = self.bounding_box().await?;
        let response = self
            .page
            .execute(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": true,
                    "clip": { "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height, "scale": 1 },
                }),
            )
            .await?;
        let data = response["data"]
            .as_str()
            .ok_or_else(|| SeleniumBaseError::screenshot("the browser returned no image data"))?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| {
                SeleniumBaseError::screenshot(format!("the image data was not valid base64: {e}"))
            })
    }

    /// Starts an assertion about this locator.
    ///
    /// ```no_run
    /// # use seleniumbase_rs::sb_cdp::Page;
    /// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// page.locator("h1").expect().to_have_text("Welcome").await?;
    /// page.locator(".spinner").expect().not().to_be_visible().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn expect(&self) -> LocatorExpect {
        LocatorExpect::new(self.clone())
    }
}

fn parse_info(value: &Value) -> ElementInfo {
    let rect = &value["rect"];
    ElementInfo {
        tag: value["tag"].as_str().unwrap_or_default().to_owned(),
        text: value["text"].as_str().unwrap_or_default().to_owned(),
        html: value["html"].as_str().unwrap_or_default().to_owned(),
        attributes: value["attributes"]
            .as_object()
            .map(|attrs| {
                attrs
                    .iter()
                    .map(|(name, value)| {
                        (name.clone(), value.as_str().unwrap_or_default().to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        rect: Rect {
            x: rect["x"].as_f64().unwrap_or_default(),
            y: rect["y"].as_f64().unwrap_or_default(),
            width: rect["width"].as_f64().unwrap_or_default(),
            height: rect["height"].as_f64().unwrap_or_default(),
        },
        visible: value["visible"].as_bool().unwrap_or_default(),
    }
}

impl Locator {
    /// Clicks at `offset` pixels right of and below the element's top-left
    /// corner, scrolling it into view first. Corresponds to Python's
    /// `click_with_offset`; to click relative to the centre, add half of
    /// [`bounding_box`](Self::bounding_box)'s width and height to `offset`.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn click_at(&self, offset: Point) -> Result<(), SeleniumBaseError> {
        let center = self.actionable_center().await?;
        let size = self.bounding_box().await?;
        let top_left = Point {
            x: center.x - size.width / 2.0,
            y: center.y - size.height / 2.0,
        };
        self.page
            .mouse()
            .click(Point {
                x: top_left.x + offset.x,
                y: top_left.y + offset.y,
            })
            .await
    }

    /// The first match's rectangle in screen coordinates, as a desktop
    /// automation tool needs it. Corresponds to Python's
    /// `get_gui_element_rect`.
    ///
    /// The browser's toolbar height is estimated from the window's outer and
    /// inner sizes, so the result is exact for a normal window and approximate
    /// when the toolbar and a docked panel are both open.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches, or
    /// [`SeleniumBaseError::CdpDriver`] if the page cannot be measured.
    pub async fn screen_rect(&self) -> Result<Rect, SeleniumBaseError> {
        let element = self.bounding_box().await?;
        let metrics: [f64; 6] = self
            .page
            .evaluate_as(crate::utils::geometry::WINDOW_METRICS_SCRIPT)
            .await?;
        Ok(crate::utils::geometry::screen_rect(element, metrics))
    }

    /// The absolute `http(s)`, `ftp` and `file` URLs in the `href` and `src`
    /// attributes of the first match and everything inside it, in page order
    /// and without repeats. Relative links are resolved against the page URL.
    /// Corresponds to Python's `get_all_urls`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches.
    pub async fn urls(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let base = url::Url::parse(&self.page.url().await?).ok();
        let mut found = vec![self.info().await?];
        found.extend(self.locator("[href], [src]").infos().await?);

        let mut seen = std::collections::BTreeSet::new();
        let mut urls = Vec::new();
        for info in found {
            for attribute in ["href", "src"] {
                let Some(url) = info
                    .attributes
                    .get(attribute)
                    .and_then(|raw| absolute_url(raw, base.as_ref()))
                else {
                    continue;
                };
                if seen.insert(url.clone()) {
                    urls.push(url);
                }
            }
        }
        Ok(urls)
    }
}

/// `raw` resolved against `base`, if it is a link a browser could follow.
fn absolute_url(raw: &str, base: Option<&url::Url>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let url = match base {
        Some(base) => base.join(raw).ok()?,
        None => url::Url::parse(raw).ok()?,
    };
    matches!(url.scheme(), "http" | "https" | "ftp" | "file").then(|| url.into())
}

#[cfg(test)]
mod url_tests {
    use super::*;

    fn base() -> url::Url {
        url::Url::parse("https://example.com/dir/page").unwrap()
    }

    #[test]
    fn relative_links_resolve_against_the_page() {
        let base = base();
        assert_eq!(
            absolute_url("/about", Some(&base)).as_deref(),
            Some("https://example.com/about")
        );
        assert_eq!(
            absolute_url("next", Some(&base)).as_deref(),
            Some("https://example.com/dir/next")
        );
        assert_eq!(
            absolute_url("//cdn.test/a.js", Some(&base)).as_deref(),
            Some("https://cdn.test/a.js")
        );
    }

    #[test]
    fn fragments_scripts_and_blanks_are_not_urls() {
        let base = base();
        for skip in [
            "",
            "  ",
            "#top",
            "javascript:void(0)",
            "mailto:a@b.test",
            "data:text/plain,hi",
        ] {
            assert_eq!(absolute_url(skip, Some(&base)), None, "{skip:?}");
        }
    }

    #[test]
    fn without_a_page_url_only_absolute_links_survive() {
        assert_eq!(
            absolute_url("https://a.test/x", None).as_deref(),
            Some("https://a.test/x")
        );
        assert_eq!(absolute_url("/x", None), None);
    }
}
