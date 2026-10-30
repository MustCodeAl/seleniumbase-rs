//! The [`Locator`]: a lazy, auto-waiting handle to elements on a page.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Value};

use super::expect::LocatorExpect;
use super::frames::{is_frame_gone, is_frames_request, Scope};
use super::input::{Button, Key};
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

/// How many frames a locator enters one inside the other, at most.
const MAX_FRAME_DEPTH: usize = 8;

/// How many times a locator looks again when a frame goes away (navigates or
/// closes) while it is being entered.
const FRAME_ATTEMPTS: usize = 3;

/// The frame a chain reaches into, and what is left of the chain inside it.
struct Located {
    scope: Scope,
    chain: Vec<Step>,
}

/// Finds elements on a page, lazily, and acts on them.
///
/// A locator remembers *how* to find elements, not which element it found.
/// Every use looks again, so it keeps working when the page re-renders, and
/// actions wait for the element to be ready instead of failing at once.
///
/// Narrowing a locator returns a new one, so `page.locator("li").visible().nth(2)`
/// reads left to right. This replaces the many near-duplicate methods other
/// frameworks offer (click the nth visible element, click a child of a parent,
/// and so on): compose them instead.
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
    ///
    /// Where those elements are frames (`iframe`, `frame`), it searches the
    /// frame's own document, whatever its origin: a frame of the page's own
    /// origin is read by the page script, and a `data:` frame, a sandboxed one,
    /// a frame of another origin or of another site is entered through the
    /// protocol. This is Python's `nested_click(parent, selector)` and
    /// `get_nested_element`. When several frames the page script cannot read
    /// match the locator so far, the first that holds a match is used.
    ///
    /// ```no_run
    /// # use seleniumbase_rs::sb_cdp::Page;
    /// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// // Python: sb.cdp.nested_click("iframe#myFrame3", ".fBox")
    /// page.locator("iframe#myFrame3").locator(".fBox").click().await?;
    /// # Ok(())
    /// # }
    /// ```
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
        let (value, _) = self.run_element(body, false).await?;
        Ok(value)
    }

    /// The script that binds the first match of the chain `chain` (JSON) to `el`
    /// and runs `body`.
    fn element_script(chain: &str, body: &str) -> String {
        format!("(() => {{ const el = __sbcdp.one({chain}); {body} }})()")
    }

    /// [`with_element`](Self::with_element), and, when `origin` is set, where the
    /// frame the element is in starts in the tab's viewport, for a frame the
    /// page script cannot see out of (zero for any other).
    async fn run_element(
        &self,
        body: &str,
        origin: bool,
    ) -> Result<(Value, Point), SeleniumBaseError> {
        self.eval(
            |chain| Self::element_script(chain, body),
            || Err(SeleniumBaseError::element_not_found(self.to_string())),
            origin,
        )
        .await
    }

    /// Runs the script `build` makes for this locator's chain (JSON).
    ///
    /// The page helper searches the frames it can read into, those of the page's
    /// own origin. When the chain matches nothing there and goes through a frame
    /// it cannot read into, the helper says so, and the frame is entered through
    /// the protocol (see the `frames` module): the script runs in the first frame
    /// the rest of the chain matches in. `empty` is the answer when it matches in
    /// none. With `origin`, the answer also holds where that frame's content
    /// starts in the tab's viewport.
    async fn eval(
        &self,
        build: impl Fn(&str) -> String,
        empty: impl FnOnce() -> Result<Value, SeleniumBaseError>,
        origin: bool,
    ) -> Result<(Value, Point), SeleniumBaseError> {
        let label = self.to_string();
        match self
            .page
            .eval_labelled(&label, &build(&self.chain_json()?))
            .await
        {
            Err(error) if is_frames_request(&error) => {}
            other => return other.map(|value| (value, Point::default())),
        }
        let mut gone = None;
        for _ in 0..FRAME_ATTEMPTS {
            match self.in_frame(&label, &build, origin).await {
                Ok(Some(found)) => return Ok(found),
                Ok(None) => return empty().map(|value| (value, Point::default())),
                Err(error) if is_frame_gone(&error) => gone = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(gone.unwrap_or_else(|| {
            SeleniumBaseError::cdp_driver("a frame changed while the locator entered it")
        }))
    }

    /// One try at [`eval`](Self::eval) inside a frame: `None` if the chain
    /// matches in no frame.
    async fn in_frame(
        &self,
        label: &str,
        build: &impl Fn(&str) -> String,
        origin: bool,
    ) -> Result<Option<(Value, Point)>, SeleniumBaseError> {
        let Some(found) = self
            .locate(self.page.main_scope(), self.chain.clone(), 0)
            .await?
        else {
            return Ok(None);
        };
        let ran = async {
            if origin {
                self.page.reveal_scope(&found.scope).await?;
            }
            let script = build(&serde_json::to_string(&found.chain)?);
            let value = self.page.eval_in(&found.scope, label, &script).await?;
            let at = if origin {
                self.page.settle_scope(&found.scope).await?;

                self.page.scope_origin(&found.scope).await?
            } else {
                Point::default()
            };
            Ok::<_, SeleniumBaseError>((value, at))
        }
        .await;
        if matches!(&ran, Err(error) if is_frame_gone(error)) {
            self.page.forget_scope(&found.scope);
        }
        self.page.release_scope(&found.scope).await;
        ran.map(Some)
    }

    /// Finds the first frame, in the page order of the frames the helper could not
    /// read into, in which the chain `chain` (what is left of this locator's, from
    /// the step that searches inside the frame) matches something; looks inside
    /// those frames in turn, as far as `MAX_FRAME_DEPTH`.
    fn locate(
        &self,
        scope: Scope,
        chain: Vec<Step>,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Located>, SeleniumBaseError>> + Send + '_>> {
        Box::pin(async move {
            if depth >= MAX_FRAME_DEPTH {
                return Ok(None);
            }
            let label = self.to_string();
            let chain_json = serde_json::to_string(&chain)?;
            let steps = self.page.blocked_steps(&scope, &label, &chain_json).await?;
            for (index, step) in steps.into_iter().enumerate() {
                let Some(object) = self.page.frame_element(&scope, &chain_json, index).await?
                else {
                    continue;
                };
                let Some(inside) = self.page.enter_frame(&scope, object).await? else {
                    continue;
                };
                let rest = chain.get(step..).unwrap_or_default().to_vec();
                let probe = format!("__sbcdp.resolve({}).length", serde_json::to_string(&rest)?);
                match self.page.eval_in(&inside, &label, &probe).await {
                    Ok(count) if count.as_u64().unwrap_or_default() > 0 => {
                        return Ok(Some(Located {
                            scope: inside,
                            chain: rest,
                        }));
                    }
                    Ok(_) => {}
                    Err(error) if is_frames_request(&error) => {
                        if let Some(found) = self.locate(inside.clone(), rest, depth + 1).await? {
                            return Ok(Some(found));
                        }
                    }
                    Err(error) if is_frame_gone(&error) => self.page.forget_scope(&inside),
                    Err(error) => return Err(error),
                }
                self.page.release_holder(&inside).await;
            }
            Ok(None)
        })
    }

    /// How many elements match right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn count(&self) -> Result<usize, SeleniumBaseError> {
        let (count, _) = self
            .eval(
                |chain| format!("__sbcdp.resolve({chain}).length"),
                || Ok(json!(0)),
                false,
            )
            .await?;
        Ok(count
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_default())
    }

    /// Whether the locator is currently in `state`. Does not wait.
    pub(crate) async fn is_in_state(&self, state: State) -> Result<bool, SeleniumBaseError> {
        let probe = |chain: &str| match state {
            State::Present => format!("__sbcdp.resolve({chain}).length > 0"),
            State::Absent => format!("__sbcdp.resolve({chain}).length === 0"),
            State::Visible => format!("__sbcdp.resolve({chain}).some(__sbcdp.visible)"),
            State::Hidden => format!("!__sbcdp.resolve({chain}).some(__sbcdp.visible)"),
        };
        // A frame the page script cannot read into that holds no match: nothing
        // is there, and nothing is visible.
        let none = matches!(state, State::Absent | State::Hidden);
        let (answer, _) = self.eval(probe, || Ok(Value::Bool(none)), false).await?;
        Ok(answer == Value::Bool(true))
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
    ///
    /// Only a timeout means "not found"; an invalid selector or a lost
    /// connection keeps its own error.
    async fn wait_present(&self) -> Result<(), SeleniumBaseError> {
        match self.wait_for(State::Present).await {
            Err(SeleniumBaseError::WaitTimeout(_)) => {
                Err(SeleniumBaseError::element_not_found(self.to_string()))
            }
            other => other,
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
    pub(super) async fn actionable_center(&self) -> Result<Point, SeleniumBaseError> {
        self.wait_actionable().await?;
        let (center, origin) = self.run_element("return __sbcdp.center(el);", true).await?;

        Ok(Point {
            x: center["x"].as_f64().unwrap_or_default() + origin.x,
            y: center["y"].as_f64().unwrap_or_default() + origin.y,
        })
    }

    /// One locator per current match, each pinned to its position.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn all(&self) -> Result<Vec<Self>, SeleniumBaseError> {
        Ok((0..self.count().await?)
            .map(|index| self.nth(index))
            .collect())
    }

    /// Waits up to the locator's timeout for a first match, then returns one
    /// locator per match, as [`all`](Self::all) does.
    ///
    /// This is what Python's `find_elements`, `find_all` and `select_all` do:
    /// they poll until something matches. Where they raise nothing and give an
    /// empty list when the time runs out, so does this. Use
    /// [`wait_for`](Self::wait_for) to get an error for that instead.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn all_waiting(&self) -> Result<Vec<Self>, SeleniumBaseError> {
        self.page
            .poll(self.timeout, || async {
                self.is_in_state(State::Present).await
            })
            .await?;
        self.all().await
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
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn infos(&self) -> Result<Vec<ElementInfo>, SeleniumBaseError> {
        let (value, _) = self
            .eval(
                |chain| format!("__sbcdp.resolve({chain}).map(__sbcdp.info)"),
                || Ok(json!([])),
                false,
            )
            .await?;
        Ok(value
            .as_array()
            .map(|items| items.iter().map(parse_info).collect())
            .unwrap_or_default())
    }

    /// The rendered text of the first match, or the value of an `<input>` or
    /// `<textarea>`, which have no rendered text (Python's `get_text` and
    /// `text_all` read the value too).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn text(&self) -> Result<String, SeleniumBaseError> {
        self.wait_present().await?;
        let value = self.with_element("return __sbcdp.fieldText(el);").await?;
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

    /// Position and size of the first match relative to the window's visible
    /// area, whatever the page is scrolled to.
    ///
    /// This is what Python's `get_element_rect` and `get_element_position`
    /// return. [`bounding_box`](Self::bounding_box) measures from the top of
    /// the document instead, so the two differ by how far the page is scrolled.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn viewport_rect(&self) -> Result<Rect, SeleniumBaseError> {
        self.wait_present().await?;
        let rect = self
            .with_element(
                "const r = el.getBoundingClientRect(); \
                 return { x: r.left, y: r.top, width: r.width, height: r.height };",
            )
            .await?;
        Ok(Rect {
            x: rect["x"].as_f64().unwrap_or_default(),
            y: rect["y"].as_f64().unwrap_or_default(),
            width: rect["width"].as_f64().unwrap_or_default(),
            height: rect["height"].as_f64().unwrap_or_default(),
        })
    }

    /// Whether any match is visible right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn is_visible(&self) -> Result<bool, SeleniumBaseError> {
        self.is_in_state(State::Visible).await
    }

    /// Whether any element matches right now. Does not wait.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
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

    /// Presses the primary button on the first match, holds it for `hold`, and
    /// releases it there.
    ///
    /// Corresponds to Python's `click_and_hold(selector, timeframe=2)`.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn click_and_hold(&self, hold: Duration) -> Result<(), SeleniumBaseError> {
        let at = self.actionable_center().await?;
        let mouse = self.page.mouse();
        mouse.move_to(at).await?;
        mouse.down(at, Button::Left).await?;
        tokio::time::sleep(hold).await;
        mouse.up(at, Button::Left).await
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
    /// Python's `send_keys`. Use [`type_text`](Self::type_text) to replace
    /// what the field holds instead.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn send_keys(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.focus().await?;
        self.page.keyboard().type_text(text).await
    }

    /// Empties the field, then types `text` into it one key at a time.
    ///
    /// Python's `type`, which "clears the text field first". Each character is
    /// a key event the page can see, unlike [`fill`](Self::fill), which inserts
    /// the text in one step. Use [`send_keys`](Self::send_keys) to type after
    /// what the field holds instead.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn type_text(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        // Clearing focuses the field.
        self.clear().await?;
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
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn set_attribute(
        &self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let name = serde_json::to_string(name.as_ref())?;
        let value = serde_json::to_string(value.as_ref())?;
        self.eval(
            |chain| {
                format!("__sbcdp.resolve({chain}).forEach(e => e.setAttribute({name}, {value}))")
            },
            || Ok(Value::Null),
            false,
        )
        .await?;
        Ok(())
    }

    /// Removes every current match from the page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is invalid.
    pub async fn remove(&self) -> Result<(), SeleniumBaseError> {
        self.eval(
            |chain| format!("__sbcdp.resolve({chain}).forEach(e => e.remove())"),
            || Ok(Value::Null),
            false,
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

    /// Outlines the first match in `color` (any CSS colour, such as `#44CC88`)
    /// for `duration`, then restores its style.
    ///
    /// The outline is removed in the page; this returns at once. Python's
    /// `flash(duration, color, pause)` waits `pause` seconds afterwards, which
    /// here is a `tokio::time::sleep`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches in time.
    pub async fn flash_with(
        &self,
        color: &str,
        duration: Duration,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_present().await?;
        let color = serde_json::to_string(color)?;
        let millis = duration.as_millis();
        self.with_element(&format!(
            "__sbcdp.flash(el, {color}, {millis}); return null;"
        ))
        .await?;
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
    /// Clicks `x` pixels right of and `y` pixels below a corner of the first
    /// match, or of its centre, scrolling it into view first.
    ///
    /// Python's `click_with_offset(selector, x, y, center=False)`: the offset
    /// is measured from the element's top-left corner, or, with
    /// `center == true`, from its centre. A negative offset goes left or up.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn click_with_offset(
        &self,
        x: f64,
        y: f64,
        center: bool,
    ) -> Result<(), SeleniumBaseError> {
        let middle = self.actionable_center().await?;
        let origin = if center {
            middle
        } else {
            let size = self.bounding_box().await?;
            Point {
                x: middle.x - size.width / 2.0,
                y: middle.y - size.height / 2.0,
            }
        };
        self.page
            .mouse()
            .click(Point {
                x: origin.x + x,
                y: origin.y + y,
            })
            .await
    }

    /// Clicks the first match if it is visible now, and says whether it
    /// clicked. It looks once and does not wait.
    ///
    /// Python's `click_if_visible(selector)` with its default `timeout=0`; use
    /// [`click_if_visible_with_timeout`](Self::click_if_visible_with_timeout)
    /// to wait for the element. It gives `false`, not an error, when the
    /// element is not visible or disappears before the click. Unlike Python,
    /// which suppresses every error, an invalid selector or a lost connection
    /// is still an error.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is
    /// invalid, or the browser's error if it fails.
    pub async fn click_if_visible(&self) -> Result<bool, SeleniumBaseError> {
        self.click_if_visible_with_timeout(Duration::ZERO).await
    }

    /// [`click_if_visible`](Self::click_if_visible), waiting up to `timeout`
    /// for the element to become visible.
    ///
    /// # Errors
    ///
    /// As [`click_if_visible`](Self::click_if_visible).
    pub async fn click_if_visible_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<bool, SeleniumBaseError> {
        match self.with_timeout(timeout).wait_for(State::Visible).await {
            Ok(()) => {}
            Err(SeleniumBaseError::WaitTimeout(_)) => return Ok(false),
            Err(error) => return Err(error),
        }
        // It was visible a moment ago; do not wait again if it is gone now.
        match self.with_timeout(Duration::ZERO).click().await {
            Ok(()) => Ok(true),
            Err(
                SeleniumBaseError::ElementNotFound { .. }
                | SeleniumBaseError::ElementNotInteractable { .. },
            ) => Ok(false),
            Err(error) => Err(error),
        }
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
