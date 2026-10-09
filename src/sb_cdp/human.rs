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
//! use seleniumbase_rs::stealth::behavior::Behavior;
//!
//! # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let person = page.human(Behavior::builder().typing_wpm(65.0).build()?);
//! person.type_text(&page.locator("#search"), "rust cdp").await?;
//! person.click(&page.locator("button[type=submit]")).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::sync::locked;
use super::{Button, Key, Locator, Page, Point, Rect};
use crate::error::SeleniumBaseError;
use crate::stealth::behavior::{Behavior, Humanizer, KeyAction, Step};

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

    /// Clicks the element, then types `text` into it at a human pace.
    ///
    /// A newline presses Enter and a tab presses Tab. With a non-zero
    /// [`typo_rate`](crate::stealth::behavior::BehaviorBuilder::typo_rate),
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
