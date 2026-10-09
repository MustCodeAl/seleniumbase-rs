//! Mouse and keyboard input, delivered as trusted browser events.
//!
//! Clicks and keystrokes go through `Input.dispatchMouseEvent` and
//! `Input.dispatchKeyEvent`, so the page sees real (`isTrusted`) events rather
//! than ones synthesised by script. Reach these through [`Page::mouse`] and
//! [`Page::keyboard`], or let a [`Locator`](super::Locator) do it for you.

use std::time::Duration;

use serde_json::json;

use super::page::Page;
use super::types::Point;
use crate::error::SeleniumBaseError;

/// Pause between key presses, long enough for the page's handlers to run.
const KEY_DELAY: Duration = Duration::from_millis(8);

/// How many intermediate pointer moves a drag makes. HTML5 drag-and-drop
/// handlers only start once the pointer has actually travelled.
const DRAG_STEPS: u32 = 5;

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Button {
    /// The primary button.
    Left,
    /// The secondary (context menu) button.
    Right,
    /// The middle button.
    Middle,
}

impl Button {
    fn protocol_name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }

    /// The bit this button sets in the protocol's `buttons` mask.
    fn mask(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 4,
        }
    }
}

/// The pointer of one tab.
///
/// Coordinates are viewport pixels from the top-left corner.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Page, Point};
///
/// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// page.mouse().click(Point { x: 120.0, y: 80.0 }).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Mouse {
    page: Page,
}

impl Mouse {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    async fn dispatch(
        &self,
        kind: &str,
        at: Point,
        button: Option<Button>,
        pressed: u8,
        count: u8,
    ) -> Result<(), SeleniumBaseError> {
        self.page
            .execute(
                "Input.dispatchMouseEvent",
                json!({
                    "type": kind,
                    "x": at.x,
                    "y": at.y,
                    "button": button.map_or("none", Button::protocol_name),
                    "buttons": pressed,
                    "clickCount": count,
                }),
            )
            .await?;
        Ok(())
    }

    /// Moves the pointer.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn move_to(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.dispatch("mouseMoved", at, None, 0, 0).await
    }

    /// Presses a button at `at` and holds it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn down(&self, at: Point, button: Button) -> Result<(), SeleniumBaseError> {
        self.dispatch("mousePressed", at, Some(button), button.mask(), 1)
            .await
    }

    /// Releases a button at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn up(&self, at: Point, button: Button) -> Result<(), SeleniumBaseError> {
        self.dispatch("mouseReleased", at, Some(button), 0, 1).await
    }

    async fn click_n(&self, at: Point, button: Button, times: u8) -> Result<(), SeleniumBaseError> {
        self.move_to(at).await?;
        for count in 1..=times {
            self.dispatch("mousePressed", at, Some(button), button.mask(), count)
                .await?;
            self.dispatch("mouseReleased", at, Some(button), 0, count)
                .await?;
        }
        Ok(())
    }

    /// Clicks the primary button at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn click(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.click_n(at, Button::Left, 1).await
    }

    /// Double-clicks the primary button at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn double_click(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.click_n(at, Button::Left, 2).await
    }

    /// Clicks the secondary button at `at`, opening a context menu.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn right_click(&self, at: Point) -> Result<(), SeleniumBaseError> {
        self.click_n(at, Button::Right, 1).await
    }

    /// Drags from `from` to `to` with the primary button held.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn drag(&self, from: Point, to: Point) -> Result<(), SeleniumBaseError> {
        self.move_to(from).await?;
        self.down(from, Button::Left).await?;
        for step in 1..=DRAG_STEPS {
            let t = f64::from(step) / f64::from(DRAG_STEPS);
            let at = Point {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
            self.dispatch("mouseMoved", at, Some(Button::Left), Button::Left.mask(), 0)
                .await?;
        }
        self.up(to, Button::Left).await
    }

    /// Scrolls the wheel by `dx` / `dy` pixels with the pointer at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn wheel(&self, at: Point, dx: f64, dy: f64) -> Result<(), SeleniumBaseError> {
        self.page
            .execute(
                "Input.dispatchMouseEvent",
                json!({ "type": "mouseWheel", "x": at.x, "y": at.y, "deltaX": dx, "deltaY": dy }),
            )
            .await?;
        Ok(())
    }
}

/// A named key that is not typed as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
}

impl Key {
    /// The `key`, `code` and Windows virtual key code the protocol expects.
    fn descriptor(self) -> (&'static str, &'static str, u32) {
        match self {
            Self::Enter => ("Enter", "Enter", 13),
            Self::Tab => ("Tab", "Tab", 9),
            Self::Escape => ("Escape", "Escape", 27),
            Self::Backspace => ("Backspace", "Backspace", 8),
            Self::Delete => ("Delete", "Delete", 46),
            Self::ArrowUp => ("ArrowUp", "ArrowUp", 38),
            Self::ArrowDown => ("ArrowDown", "ArrowDown", 40),
            Self::ArrowLeft => ("ArrowLeft", "ArrowLeft", 37),
            Self::ArrowRight => ("ArrowRight", "ArrowRight", 39),
            Self::Home => ("Home", "Home", 36),
            Self::End => ("End", "End", 35),
            Self::PageUp => ("PageUp", "PageUp", 33),
            Self::PageDown => ("PageDown", "PageDown", 34),
        }
    }
}

/// The keyboard of one tab, acting on whatever has focus.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Key, Page};
///
/// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// page.locator("#search").focus().await?;
/// page.keyboard().type_text("rust").await?;
/// page.keyboard().press(Key::Enter).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Keyboard {
    page: Page,
}

impl Keyboard {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    /// Presses and releases one key.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn press(&self, key: Key) -> Result<(), SeleniumBaseError> {
        let (name, code, virtual_code) = key.descriptor();
        // Enter also produces a character, which is what submits a form.
        let text = (key == Key::Enter).then_some("\r");
        let mut down = json!({
            "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
            "key": name,
            "code": code,
            "windowsVirtualKeyCode": virtual_code,
            "nativeVirtualKeyCode": virtual_code,
        });
        if let Some(text) = text {
            down["text"] = json!(text);
        }
        self.page.execute("Input.dispatchKeyEvent", down).await?;
        self.page
            .execute(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyUp",
                    "key": name,
                    "code": code,
                    "windowsVirtualKeyCode": virtual_code,
                    "nativeVirtualKeyCode": virtual_code,
                }),
            )
            .await?;
        Ok(())
    }

    /// Types `text` one character at a time.
    ///
    /// Each character is a separate key press, so the page sees what a person
    /// typing would produce. A newline presses [`Key::Enter`] and a tab
    /// presses [`Key::Tab`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn type_text(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        for ch in text.as_ref().chars() {
            match ch {
                '\n' | '\r' => self.press(Key::Enter).await?,
                '\t' => self.press(Key::Tab).await?,
                _ => {
                    let typed = ch.to_string();
                    self.page
                        .execute(
                            "Input.dispatchKeyEvent",
                            json!({ "type": "keyDown", "key": typed, "text": typed, "unmodifiedText": typed }),
                        )
                        .await?;
                    self.page
                        .execute(
                            "Input.dispatchKeyEvent",
                            json!({ "type": "keyUp", "key": typed }),
                        )
                        .await?;
                }
            }
            tokio::time::sleep(KEY_DELAY).await;
        }
        Ok(())
    }

    /// Inserts `text` in one step, as a paste would.
    ///
    /// Much faster than [`type_text`](Self::type_text), but it produces no
    /// per-character key events, so avoid it where the page might notice.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn insert_text(&self, text: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        self.page
            .execute("Input.insertText", json!({ "text": text.as_ref() }))
            .await?;
        Ok(())
    }
}

impl Page {
    /// The pointer of this tab.
    #[must_use]
    pub fn mouse(&self) -> Mouse {
        Mouse::new(self.clone())
    }

    /// The keyboard of this tab.
    #[must_use]
    pub fn keyboard(&self) -> Keyboard {
        Keyboard::new(self.clone())
    }
}
