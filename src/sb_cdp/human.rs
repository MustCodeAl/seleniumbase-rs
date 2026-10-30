//! Input with a person's pace: curved pointer paths, uneven typing.
//!
//! [`Page::human`] returns a [`Human`] that plays the plans of
//! [`Humanizer`] out on the page: the pointer travels along a curve that
//! speeds up and slows down, the click lands somewhere inside the element
//! rather than on its exact centre, and text arrives with a human rhythm. The
//! events are the same trusted DevTools input as [`Mouse`](super::Mouse) and
//! [`Keyboard`](super::Keyboard); only the timing and the path differ.
//!
//! It is slower than the direct API by design. Use it where a page watches
//! how input arrives.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::Page;
//! use seleniumbase_rs::evasion::behavior::Behavior;
//!
//! # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let person = page.human(Behavior::builder().typing_wpm(65.0).build()?);
//! person.type_text(&page.locator("#search"), "rust cdp").await?;
//! person.click(&page.locator("button[type=submit]")).await?;
//! # Ok(())
//! # }
//! ```

use std::future::Future;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::sync::locked;
use super::{Button, Key, Locator, Page, Point, Rect};
use crate::error::SeleniumBaseError;
use crate::evasion::behavior::{Behavior, Humanizer, KeyAction, Step};

/// The pointer events a person's click or drag is made of.
///
/// A seam shared by the Pure CDP [`Page`] and the WebDriver `BaseCase`, so
/// that both play the same plans (see [`plan_click`] and [`plan_drag`]).
/// `held` is whether the primary button is down while the pointer moves.
pub(crate) trait PointerSink {
    /// Moves the pointer to `at`.
    fn pointer_move(
        &self,
        at: Point,
        held: bool,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;
    /// Presses the primary button at `at`.
    fn pointer_down(&self, at: Point)
        -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;
    /// Releases the primary button at `at`.
    fn pointer_up(&self, at: Point) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;
}

impl PointerSink for Page {
    async fn pointer_move(&self, at: Point, held: bool) -> Result<(), SeleniumBaseError> {
        if held {
            self.execute(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseMoved", "x": at.x, "y": at.y,
                    "button": "left", "buttons": 1, "clickCount": 0,
                }),
            )
            .await?;
            Ok(())
        } else {
            self.mouse().move_to(at).await
        }
    }

    async fn pointer_down(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.mouse().down(at, Button::Left).await
    }

    async fn pointer_up(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.mouse().up(at, Button::Left).await
    }
}

/// A planned click: travel to the target, rest on it, press, hold, release.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClickPlan {
    /// The pointer's way to the landing point.
    pub(crate) path: Vec<Step>,
    /// Where the button goes down and up: near the aim, not exactly on it.
    pub(crate) landing: Point,
    /// How long the pointer rests on the target before the press.
    pub(crate) rest: Duration,
    /// How long the button stays down.
    pub(crate) dwell: Duration,
}

/// A planned drag: travel to the handle, rest, press, drag along a path,
/// let go.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DragPlan {
    /// The approach to the handle.
    pub(crate) approach: Vec<Step>,
    /// Where the button goes down.
    pub(crate) press: Point,
    /// How long the pointer rests on the handle before the press.
    pub(crate) rest: Duration,
    /// The way from the handle to the end, button held.
    pub(crate) stroke: Vec<Step>,
    /// Where the button comes up.
    pub(crate) release: Point,
    /// How long the pointer stays at the end before the button comes up.
    pub(crate) hold: Duration,
}

/// Plans a click on a target of about `tolerance` pixels across, centred on
/// `aim`, for a pointer now at `from`.
///
/// The landing point is normally distributed around `aim` and kept inside the
/// target, so two clicks differ.
pub(crate) fn plan_click(
    person: &mut Humanizer,
    from: Point,
    aim: Point,
    tolerance: f64,
) -> ClickPlan {
    let landing = person.target_point(Rect {
        x: aim.x - tolerance / 2.0,
        y: aim.y - tolerance / 2.0,
        width: tolerance,
        height: tolerance,
    });
    ClickPlan {
        path: person.mouse_path(from, landing, tolerance),
        landing,
        rest: person.reaction(),
        dwell: person.click_dwell(),
    }
}

/// Plans a drag of a handle at `press` to `release`, for a pointer now at
/// `from`.
pub(crate) fn plan_drag(
    person: &mut Humanizer,
    from: Point,
    press: Point,
    release: Point,
    tolerance: f64,
) -> DragPlan {
    let start = person.target_point(Rect {
        x: press.x - tolerance / 2.0,
        y: press.y - tolerance / 2.0,
        width: tolerance,
        height: tolerance,
    });
    DragPlan {
        approach: person.mouse_path(from, start, tolerance),
        press: start,
        rest: person.reaction(),
        stroke: person.mouse_path(start, release, tolerance),
        release,
        hold: person.click_dwell(),
    }
}

/// Plays pointer moves on the planned timeline: each is due a planned wait
/// after the one before it, and the time the browser takes to take one in
/// does not push the rest back, so a long path does not become a slow one.
async fn play_steps<P: PointerSink>(
    sink: &P,
    steps: &[Step],
    held: bool,
) -> Result<(), SeleniumBaseError> {
    let start = tokio::time::Instant::now();
    let mut due = Duration::ZERO;
    for step in steps {
        due += step.wait;
        tokio::time::sleep_until(start + due).await;
        sink.pointer_move(step.at, held).await?;
    }
    Ok(())
}

/// Plays a [`ClickPlan`] out on `sink`.
pub(crate) async fn play_click<P: PointerSink>(
    sink: &P,
    plan: &ClickPlan,
) -> Result<(), SeleniumBaseError> {
    play_steps(sink, &plan.path, false).await?;
    tokio::time::sleep(plan.rest).await;
    sink.pointer_down(plan.landing).await?;
    tokio::time::sleep(plan.dwell).await;
    sink.pointer_up(plan.landing).await
}

/// Plays a [`DragPlan`] out on `sink`.
pub(crate) async fn play_drag<P: PointerSink>(
    sink: &P,
    plan: &DragPlan,
) -> Result<(), SeleniumBaseError> {
    play_steps(sink, &plan.approach, false).await?;
    tokio::time::sleep(plan.rest).await;
    sink.pointer_down(plan.press).await?;
    play_steps(sink, &plan.stroke, true).await?;
    tokio::time::sleep(plan.hold).await;
    sink.pointer_up(plan.release).await
}

/// A person at a [`Page`]'s mouse and keyboard.
///
/// Cloning gives another handle to the same person: they share the pointer's
/// position and the random state, so two tasks do not both start from where
/// the pointer used to be.
#[derive(Debug, Clone)]
pub struct Human {
    page: Page,
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    person: Humanizer,
    /// Where the pointer was last put, if it has been moved.
    pointer: Option<Point>,
}

impl Page {
    /// Input on this page with a person's pace; see the [module
    /// docs](crate::sb_cdp::human).
    #[must_use]
    pub fn human(&self, behavior: Behavior) -> Human {
        Human {
            page: self.clone(),
            state: Arc::new(Mutex::new(State {
                person: Humanizer::new(behavior),
                pointer: None,
            })),
        }
    }
}

impl Human {
    /// Moves the pointer to `to`, along a human path.
    ///
    /// `target_width` is how big the target is, in pixels: a small target takes
    /// longer to reach.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn move_to(&self, to: Point, target_width: f64) -> Result<(), SeleniumBaseError> {
        let from = self.pointer().await?;
        let path = locked(&self.state)
            .person
            .mouse_path(from, to, target_width);
        self.play_path(path).await
    }

    /// Scrolls the element into view, moves to a point inside it and clicks.
    ///
    /// The point is normally distributed around the element's centre, so
    /// repeated clicks differ. The button is held for a few tens of
    /// milliseconds, as a finger holds it.
    ///
    /// # Errors
    ///
    /// Returns an error if the element does not become actionable in time, or
    /// if the browser rejects the input.
    pub async fn click(&self, target: &Locator) -> Result<(), SeleniumBaseError> {
        let center = target.actionable_center().await?;
        let size = target.bounding_box().await?;
        let area = Rect {
            x: center.x - size.width / 2.0,
            y: center.y - size.height / 2.0,
            width: size.width,
            height: size.height,
        };
        let (point, dwell) = {
            let mut state = locked(&self.state);
            (state.person.target_point(area), state.person.click_dwell())
        };

        self.move_to(point, size.width.min(size.height)).await?;
        let mouse = self.page.mouse();
        mouse.down(point, Button::Left).await?;
        tokio::time::sleep(dwell).await;
        mouse.up(point, Button::Left).await
    }

    /// Moves to a point near `aim` along a human path, rests on it for a
    /// moment, then presses and releases the primary button.
    ///
    /// `tolerance` is how big the target is, in pixels: the button goes down
    /// at a point normally distributed around `aim` and kept within a square of
    /// that size, and a small target takes longer to reach. Returns the point
    /// that was pressed. Use it where the target is not an element of the page
    /// that a [`Locator`] can name, such as the box inside a frame from another
    /// origin.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn click_at(&self, aim: Point, tolerance: f64) -> Result<Point, SeleniumBaseError> {
        let from = self.pointer().await?;
        let plan = {
            let mut state = locked(&self.state);
            plan_click(&mut state.person, from, aim, tolerance)
        };
        play_click(&self.page, &plan).await?;
        locked(&self.state).pointer = Some(plan.landing);
        Ok(plan.landing)
    }

    /// Drags a handle from a point near `press` to `release` along human paths:
    /// the pointer travels to the handle, rests, presses, moves with the
    /// button held and lets go.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn drag(
        &self,
        press: Point,
        release: Point,
        tolerance: f64,
    ) -> Result<(), SeleniumBaseError> {
        let from = self.pointer().await?;
        let plan = {
            let mut state = locked(&self.state);
            plan_drag(&mut state.person, from, press, release, tolerance)
        };
        play_drag(&self.page, &plan).await?;
        locked(&self.state).pointer = Some(plan.release);
        Ok(())
    }

    /// Clicks the element, then types `text` into it at a human pace.
    ///
    /// A newline presses Enter and a tab presses Tab. With a non-zero
    /// [`typo_rate`](crate::evasion::behavior::BehaviorBuilder::typo_rate),
    /// some letters are typed wrong and corrected; the field always ends up
    /// holding exactly `text` (plus what it held before).
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn type_text(
        &self,
        target: &Locator,
        text: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        self.click(target).await?;
        let reaction = locked(&self.state).person.reaction();
        tokio::time::sleep(reaction).await;
        self.type_focused(text).await
    }

    /// Types `text` into whatever has keyboard focus, at a human pace.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn type_focused(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let plan = locked(&self.state).person.typing_plan(text.as_ref());
        let keyboard = self.page.keyboard();
        for key in plan {
            tokio::time::sleep(key.wait).await;
            match key.action {
                KeyAction::Type(ch) => keyboard.type_char(ch).await?,
                KeyAction::Backspace => keyboard.press(Key::Backspace).await?,
            }
        }
        Ok(())
    }

    /// Scrolls by `dy` pixels (negative is up) with the mouse wheel, in
    /// notches that start and end gently.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn scroll_by(&self, dy: f64) -> Result<(), SeleniumBaseError> {
        let at = self.pointer().await?;
        let notches = locked(&self.state).person.scroll_plan(dy);
        let mouse = self.page.mouse();
        for notch in notches {
            tokio::time::sleep(notch.wait).await;
            mouse.wheel(at, 0.0, notch.dy).await?;
        }
        Ok(())
    }

    /// Where the pointer is now, placing it somewhere plausible on the first
    /// call.
    async fn pointer(&self) -> Result<Point, SeleniumBaseError> {
        if let Some(point) = locked(&self.state).pointer {
            return Ok(point);
        }
        let [width, height]: [f64; 2] = self
            .page
            .evaluate_as("[window.innerWidth, window.innerHeight]")
            .await?;
        let start = locked(&self.state).person.resting_point(width, height);
        self.page.mouse().move_to(start).await?;
        locked(&self.state).pointer = Some(start);
        Ok(start)
    }

    async fn play_path(&self, path: Vec<Step>) -> Result<(), SeleniumBaseError> {
        let mouse = self.page.mouse();
        let mut last = None;
        for step in path {
            if step.wait > Duration::ZERO {
                tokio::time::sleep(step.wait).await;
            }
            mouse.move_to(step.at).await?;
            last = Some(step.at);
        }
        if let Some(point) = last {
            locked(&self.state).pointer = Some(point);
        }
        Ok(())
    }
}
