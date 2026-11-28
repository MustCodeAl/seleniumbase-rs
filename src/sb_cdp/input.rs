//! Mouse and keyboard input, delivered as trusted browser events.
//!
//! Clicks and keystrokes go through `Input.dispatchMouseEvent` and
//! `Input.dispatchKeyEvent`, so the page sees real (`isTrusted`) events rather
//! than ones synthesised by script. Reach these through [`Page::mouse`] and
//! [`Page::keyboard`], or let a [`Locator`](super::Locator) do it for you.

use std::time::Duration;

use serde_json::{Value, json};

use super::page::Page;
use super::sync::locked;
use super::types::Point;
use crate::error::SeleniumBaseError;

/// A JavaScript modal dialog (`alert`, `confirm`, `prompt`) opened while
/// dispatching input on a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingDialog {
    kind: String,
    default_prompt: String,
    typed: Option<String>,
}

impl PendingDialog {
    fn from_event(params: &Value) -> Self {
        Self {
            kind: params["type"].as_str().unwrap_or("alert").to_owned(),
            default_prompt: params["defaultPrompt"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            typed: None,
        }
    }
}

/// Pause between key presses, long enough for the page's handlers to run.
const KEY_DELAY: Duration = Duration::from_millis(8);

/// How many intermediate pointer moves a drag makes. HTML5 drag-and-drop
/// handlers only start once the pointer has actually travelled.
const DRAG_STEPS: u32 = 5;

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    fn event(kind: &str, at: Point, button: Option<Button>, pressed: u8, count: u8) -> Value {
        json!({
            "type": kind,
            "x": at.x,
            "y": at.y,
            "button": button.map_or("none", Button::protocol_name),
            "buttons": pressed,
            "clickCount": count,
        })
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
                Self::event(kind, at, button, pressed, count),
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

    /// Moves to `at` and presses and releases `button` `times` times.
    ///
    /// Chrome holds a pointer move back until the next frame (about 33 ms in
    /// headless mode) before it answers, but it delivers that move ahead of
    /// the press that follows. So all the events go out together, in order, and
    /// the page sees the same sequence as before, in under a millisecond
    /// instead of waiting a frame for the move.
    async fn click_n(&self, at: Point, button: Button, times: u8) -> Result<(), SeleniumBaseError> {
        const METHOD: &str = "Input.dispatchMouseEvent";
        let mut commands = vec![(METHOD, Self::event("mouseMoved", at, None, 0, 0))];
        for count in 1..=times {
            commands.push((
                METHOD,
                Self::event("mousePressed", at, Some(button), button.mask(), count),
            ));
            commands.push((
                METHOD,
                Self::event("mouseReleased", at, Some(button), 0, count),
            ));
        }
        let mut events = self.page.events();
        let dialog_opened = async {
            while let Some(event) = events.next().await {
                if event.method == "Page.javascriptDialogOpening" {
                    return PendingDialog::from_event(&event.params);
                }
            }
            std::future::pending::<PendingDialog>().await
        };
        tokio::select! {
            res = self.page.execute_ordered(commands) => {
                res?;
            }
            dialog = dialog_opened => {
                self.page.record_open_dialog(dialog);
            }
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
    /// Insert.
    Insert,
    /// Space.
    Space,
    /// Pause.
    Pause,
    /// Shift.
    Shift,
    /// Control.
    Control,
    /// Alt / Option.
    Alt,
    /// Meta / Command / Windows key.
    Meta,
    /// Function key F1.
    F1,
    /// Function key F2.
    F2,
    /// Function key F3.
    F3,
    /// Function key F4.
    F4,
    /// Function key F5.
    F5,
    /// Function key F6.
    F6,
    /// Function key F7.
    F7,
    /// Function key F8.
    F8,
    /// Function key F9.
    F9,
    /// Function key F10.
    F10,
    /// Function key F11.
    F11,
    /// Function key F12.
    F12,
    /// Numpad 0.
    NumPad0,
    /// Numpad 1.
    NumPad1,
    /// Numpad 2.
    NumPad2,
    /// Numpad 3.
    NumPad3,
    /// Numpad 4.
    NumPad4,
    /// Numpad 5.
    NumPad5,
    /// Numpad 6.
    NumPad6,
    /// Numpad 7.
    NumPad7,
    /// Numpad 8.
    NumPad8,
    /// Numpad 9.
    NumPad9,
    /// Numpad Multiply (*).
    Multiply,
    /// Numpad Add (+).
    Add,
    /// Numpad Subtract (-).
    Subtract,
    /// Numpad Decimal (.).
    Decimal,
    /// Numpad Divide (/).
    Divide,
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
            Self::Insert => ("Insert", "Insert", 45),
            Self::Space => (" ", "Space", 32),
            Self::Pause => ("Pause", "Pause", 19),
            Self::Shift => ("Shift", "ShiftLeft", 16),
            Self::Control => ("Control", "ControlLeft", 17),
            Self::Alt => ("Alt", "AltLeft", 18),
            Self::Meta => ("Meta", "MetaLeft", 91),
            Self::F1 => ("F1", "F1", 112),
            Self::F2 => ("F2", "F2", 113),
            Self::F3 => ("F3", "F3", 114),
            Self::F4 => ("F4", "F4", 115),
            Self::F5 => ("F5", "F5", 116),
            Self::F6 => ("F6", "F6", 117),
            Self::F7 => ("F7", "F7", 118),
            Self::F8 => ("F8", "F8", 119),
            Self::F9 => ("F9", "F9", 120),
            Self::F10 => ("F10", "F10", 121),
            Self::F11 => ("F11", "F11", 122),
            Self::F12 => ("F12", "F12", 123),
            Self::NumPad0 => ("0", "Numpad0", 96),
            Self::NumPad1 => ("1", "Numpad1", 97),
            Self::NumPad2 => ("2", "Numpad2", 98),
            Self::NumPad3 => ("3", "Numpad3", 99),
            Self::NumPad4 => ("4", "Numpad4", 100),
            Self::NumPad5 => ("5", "Numpad5", 101),
            Self::NumPad6 => ("6", "Numpad6", 102),
            Self::NumPad7 => ("7", "Numpad7", 103),
            Self::NumPad8 => ("8", "Numpad8", 104),
            Self::NumPad9 => ("9", "Numpad9", 105),
            Self::Multiply => ("*", "NumpadMultiply", 106),
            Self::Add => ("+", "NumpadAdd", 107),
            Self::Subtract => ("-", "NumpadSubtract", 109),
            Self::Decimal => (".", "NumpadDecimal", 110),
            Self::Divide => ("/", "NumpadDivide", 111),
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
        if matches!(key, Key::Enter | Key::Escape)
            && let Some(dialog) = self.page.take_open_dialog()
        {
            let accept = key == Key::Enter;
            let mut params = json!({ "accept": accept });
            if accept && dialog.kind == "prompt" {
                params["promptText"] = json!(dialog.typed.unwrap_or(dialog.default_prompt));
            }
            match self
                .page
                .execute("Page.handleJavaScriptDialog", params)
                .await
            {
                Ok(_) => return Ok(()),
                Err(error) if error.to_string().contains("No dialog is showing") => {}
                Err(error) => return Err(error),
            }
        }
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
        if self.page.append_dialog_text(text.as_ref()) {
            return Ok(());
        }
        for ch in text.as_ref().chars() {
            self.type_char(ch).await?;
            tokio::time::sleep(KEY_DELAY).await;
        }
        Ok(())
    }

    /// Presses and releases the key that produces `ch`.
    ///
    /// A newline presses [`Key::Enter`] and a tab presses [`Key::Tab`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn type_char(&self, ch: char) -> Result<(), SeleniumBaseError> {
        if let Some(key) = crate::utils::keys::Keys::to_key(ch) {
            return self.press(key).await;
        }
        match ch {
            '\n' | '\r' => self.press(Key::Enter).await,
            '\t' => self.press(Key::Tab).await,
            _ => {
                let typed = ch.to_string();
                let is_upper = ch.is_uppercase();
                let unmodified = if ch.is_ascii_uppercase() {
                    ch.to_ascii_lowercase().to_string()
                } else {
                    typed.clone()
                };
                let modifiers = if is_upper { 8 } else { 0 };

                if is_upper {
                    self.page
                        .execute(
                            "Input.dispatchKeyEvent",
                            json!({
                                "type": "rawKeyDown",
                                "key": "Shift",
                                "code": "ShiftLeft",
                                "windowsVirtualKeyCode": 16,
                                "nativeVirtualKeyCode": 16,
                                "modifiers": 8,
                            }),
                        )
                        .await?;
                }

                self.page
                    .execute(
                        "Input.dispatchKeyEvent",
                        json!({
                            "type": "keyDown",
                            "key": typed,
                            "text": typed,
                            "unmodifiedText": unmodified,
                            "modifiers": modifiers,
                        }),
                    )
                    .await?;
                self.page
                    .execute(
                        "Input.dispatchKeyEvent",
                        json!({
                            "type": "keyUp",
                            "key": typed,
                            "modifiers": modifiers,
                        }),
                    )
                    .await?;

                if is_upper {
                    self.page
                        .execute(
                            "Input.dispatchKeyEvent",
                            json!({
                                "type": "keyUp",
                                "key": "Shift",
                                "code": "ShiftLeft",
                                "windowsVirtualKeyCode": 16,
                                "nativeVirtualKeyCode": 16,
                                "modifiers": 0,
                            }),
                        )
                        .await?;
                }

                Ok(())
            }
        }
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
        if self.page.append_dialog_text(text.as_ref()) {
            return Ok(());
        }
        self.page
            .execute("Input.insertText", json!({ "text": text.as_ref() }))
            .await?;
        Ok(())
    }
}

impl Page {
    pub(super) fn record_open_dialog(&self, dialog: PendingDialog) {
        locked(&self.browser().inner().open_dialogs).insert(self.id().to_owned(), dialog);
    }

    pub(super) fn take_open_dialog(&self) -> Option<PendingDialog> {
        locked(&self.browser().inner().open_dialogs).remove(self.id())
    }

    pub(super) fn append_dialog_text(&self, text: &str) -> bool {
        let mut guard = locked(&self.browser().inner().open_dialogs);
        if let Some(dialog) = guard.get_mut(self.id())
            && dialog.kind == "prompt"
        {
            dialog.typed.get_or_insert_with(String::new).push_str(text);
            return true;
        }
        false
    }

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

#[cfg(test)]
mod dialog_input_tests {
    use super::*;
    use crate::sb_cdp::Browser;

    #[tokio::test]
    async fn keyboard_handles_prompt_alert_and_confirm_dialogs_recorded_on_page() {
        let (browser, mock) = Browser::new_mocked();
        let page = browser.default_page().await.expect("mock page");

        // Alert accepted with Enter.
        page.record_open_dialog(PendingDialog::from_event(&json!({
            "type": "alert",
            "message": "I am a JS Alert"
        })));
        page.keyboard()
            .press(Key::Enter)
            .await
            .expect("accepts alert");

        // Confirm dismissed with Escape.
        page.record_open_dialog(PendingDialog::from_event(&json!({
            "type": "confirm",
            "message": "I am a JS Confirm"
        })));
        page.keyboard()
            .press(Key::Escape)
            .await
            .expect("dismisses confirm");

        // Prompt typed into and accepted with Enter.
        page.record_open_dialog(PendingDialog::from_event(&json!({
            "type": "prompt",
            "message": "I am a JS prompt"
        })));
        page.keyboard()
            .type_text("Here is my prompt answer")
            .await
            .expect("buffers prompt text");
        page.keyboard()
            .press(Key::Enter)
            .await
            .expect("submits prompt");

        let handled: Vec<Value> = mock
            .calls_to("Page.handleJavaScriptDialog")
            .into_iter()
            .map(|c| c.params)
            .collect();
        assert_eq!(
            handled,
            vec![
                json!({ "accept": true }),
                json!({ "accept": false }),
                json!({ "accept": true, "promptText": "Here is my prompt answer" }),
            ]
        );
    }

    #[tokio::test]
    async fn mouse_and_keyboard_dispatch_all_event_shapes_and_dialog_fallbacks() {
        let (browser, mock) = Browser::new_mocked();
        let page = browser.default_page().await.expect("mock page");
        let at = Point { x: 40.0, y: 80.0 };

        page.mouse().move_to(at).await.expect("move_to");
        page.mouse()
            .down(at, Button::Middle)
            .await
            .expect("down middle");
        page.mouse()
            .up(at, Button::Middle)
            .await
            .expect("up middle");
        page.mouse().click(at).await.expect("click");
        page.mouse().double_click(at).await.expect("double_click");
        page.mouse().right_click(at).await.expect("right_click");
        page.mouse()
            .drag(at, Point { x: 140.0, y: 180.0 })
            .await
            .expect("drag");
        page.mouse().wheel(at, 10.0, -20.0).await.expect("wheel");

        let mouse_calls = mock.calls_to("Input.dispatchMouseEvent");
        assert!(mouse_calls.iter().any(|c| c.params["type"] == "mouseWheel"));
        assert!(mouse_calls.iter().any(|c| c.params["button"] == "right"));
        assert!(mouse_calls.iter().any(|c| c.params["button"] == "middle"));
        assert!(mouse_calls.iter().any(|c| c.params["clickCount"] == 2));

        // Keyboard: type_text with uppercase, lowercase, newline, tab, and insert_text.
        page.keyboard()
            .type_text("Ab\t\n")
            .await
            .expect("type_text");
        page.keyboard()
            .insert_text("pasted")
            .await
            .expect("insert_text");
        assert_eq!(mock.calls_to("Input.insertText").len(), 1);

        // Prompt insert_text buffers into the open dialog without sending Input.insertText.
        page.record_open_dialog(PendingDialog::from_event(&json!({
            "type": "prompt",
            "defaultPrompt": "fallback"
        })));
        page.keyboard()
            .insert_text("inserted-answer")
            .await
            .expect("buffers into prompt");
        assert_eq!(mock.calls_to("Input.insertText").len(), 1);
        page.keyboard()
            .press(Key::Enter)
            .await
            .expect("submits prompt");
        let handled = mock.calls_to("Page.handleJavaScriptDialog");
        assert_eq!(
            handled.last().unwrap().params,
            json!({ "accept": true, "promptText": "inserted-answer" })
        );

        // Non-prompt open dialog does not swallow insert_text.
        page.record_open_dialog(PendingDialog::from_event(&json!({ "type": "alert" })));
        assert!(!page.append_dialog_text("ignored"));
        let _ = page.take_open_dialog();

        // If Page.handleJavaScriptDialog reports "No dialog is showing", fall back to dispatchKeyEvent.
        mock.fail("Page.handleJavaScriptDialog", "No dialog is showing");
        page.record_open_dialog(PendingDialog::from_event(&json!({ "type": "alert" })));
        let keys_before = mock.calls_to("Input.dispatchKeyEvent").len();
        page.keyboard()
            .press(Key::Enter)
            .await
            .expect("falls back to key dispatch");
        assert!(mock.calls_to("Input.dispatchKeyEvent").len() > keys_before);
    }

    #[test]
    fn all_key_descriptors_are_non_empty() {
        for key in [
            Key::Enter,
            Key::Tab,
            Key::Escape,
            Key::Backspace,
            Key::Delete,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
            Key::Insert,
            Key::Space,
            Key::Pause,
            Key::Shift,
            Key::Control,
            Key::Alt,
            Key::Meta,
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
            Key::NumPad0,
            Key::NumPad1,
            Key::NumPad2,
            Key::NumPad3,
            Key::NumPad4,
            Key::NumPad5,
            Key::NumPad6,
            Key::NumPad7,
            Key::NumPad8,
            Key::NumPad9,
            Key::Multiply,
            Key::Add,
            Key::Subtract,
            Key::Decimal,
            Key::Divide,
        ] {
            let (name, code, vk) = key.descriptor();
            assert!(!name.is_empty() && !code.is_empty() && vk > 0);
        }
    }
}
