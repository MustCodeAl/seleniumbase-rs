//! Native GUI automation wrapper around `enigo`.
//!
//! This module exposes a small, ergonomic `Gui` struct for simulating mouse and
//! keyboard input at the OS level. It is intentionally separate from the
//! WebDriver-based `BaseCase` API so it can be used standalone or composed into
//! higher-level helper methods.
//!
//! The input is real: it moves the pointer on the user's screen and types into
//! whatever window has focus. It needs a desktop session (on Linux, X11 or
//! Wayland with the right permissions; on macOS, the Accessibility permission
//! for the process).
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::gui::Gui;
//!
//! # fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut gui = Gui::new()?;
//! gui.move_mouse(200, 300)?;
//! gui.click()?;
//! gui.press_keys(&["ctrl", "a"])?; // select all
//! gui.write("hello")?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use crate::error::SeleniumBaseError;

/// Time between two pointer moves while dragging. Drag handlers and HTML5
/// drag-and-drop need to see the pointer travel, not jump.
const DRAG_STEP_PAUSE: Duration = Duration::from_millis(8);

/// Roughly how many pixels the pointer moves per step while dragging.
const DRAG_PIXELS_PER_STEP: f64 = 15.0;

/// The fewest and most steps a drag is split into.
const DRAG_STEPS: std::ops::RangeInclusive<usize> = 5..=60;

/// The input operations the chord and drag logic needs.
///
/// A seam, so that logic can be tested against a recording fake instead of
/// moving the real pointer. The methods are named apart from `enigo`'s own
/// (`key`, `button`, ...) so that calls on an `Enigo` stay unambiguous.
trait InputDevice {
    fn send_key(&mut self, key: Key, direction: Direction) -> Result<(), SeleniumBaseError>;
    fn move_pointer(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError>;
    fn send_left_button(&mut self, direction: Direction) -> Result<(), SeleniumBaseError>;
    /// Waits between two steps of a gesture.
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

impl InputDevice for Enigo {
    fn send_key(&mut self, key: Key, direction: Direction) -> Result<(), SeleniumBaseError> {
        Keyboard::key(self, key, direction)?;
        Ok(())
    }

    fn move_pointer(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        Mouse::move_mouse(self, x, y, Coordinate::Abs)?;
        Ok(())
    }

    fn send_left_button(&mut self, direction: Direction) -> Result<(), SeleniumBaseError> {
        Mouse::button(self, Button::Left, direction)?;
        Ok(())
    }
}

/// Presses `modifiers`, clicks `keys`, then releases the modifiers in reverse.
///
/// The modifiers that were pressed are released even if a later key fails, so
/// a failed chord cannot leave Ctrl or Shift stuck down. The first error is
/// the one returned.
fn press_chord(
    device: &mut impl InputDevice,
    modifiers: &[Key],
    keys: &[Key],
) -> Result<(), SeleniumBaseError> {
    let mut held = Vec::with_capacity(modifiers.len());
    let mut result = Ok(());
    for modifier in modifiers {
        match device.send_key(*modifier, Direction::Press) {
            Ok(()) => held.push(*modifier),
            Err(error) => {
                result = Err(error);
                break;
            }
        }
    }
    if result.is_ok() {
        for key in keys {
            if let Err(error) = device.send_key(*key, Direction::Click) {
                result = Err(error);
                break;
            }
        }
    }
    for modifier in held.iter().rev() {
        let released = device.send_key(*modifier, Direction::Release);
        if result.is_ok() {
            result = released;
        }
    }
    result
}

/// The points between `from` and `to`, excluding both, that a drag passes
/// through. Empty when the two are the same point.
fn drag_waypoints(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let dx = f64::from(to.0) - f64::from(from.0);
    let dy = f64::from(to.1) - f64::from(from.1);
    let distance = dx.hypot(dy);
    if distance == 0.0 {
        return Vec::new();
    }
    let steps = ((distance / DRAG_PIXELS_PER_STEP).ceil() as usize)
        .clamp(*DRAG_STEPS.start(), *DRAG_STEPS.end());
    (1..steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            (
                (f64::from(from.0) + dx * t).round() as i32,
                (f64::from(from.1) + dy * t).round() as i32,
            )
        })
        .collect()
}

/// Drags the left button from `from` to `to` in small steps.
///
/// The button is released even if a move fails.
fn drag_between(
    device: &mut impl InputDevice,
    from: (i32, i32),
    to: (i32, i32),
) -> Result<(), SeleniumBaseError> {
    device.move_pointer(from.0, from.1)?;
    device.send_left_button(Direction::Press)?;
    let travelled = (|| {
        for (x, y) in drag_waypoints(from, to) {
            device.move_pointer(x, y)?;
            device.pause(DRAG_STEP_PAUSE);
        }
        device.move_pointer(to.0, to.1)
    })();
    let released = device.send_left_button(Direction::Release);
    travelled.and(released)
}

/// Wraps an [`enigo::Enigo`] instance and provides SeleniumBase-flavored
/// convenience methods.
///
/// `Gui` is intentionally not stored inside [`crate::BaseCase`] because
/// `enigo::Enigo` is not guaranteed to be `Send`/`Sync`. Instead, construct a
/// fresh `Gui` inside each method that needs native input.
pub struct Gui {
    enigo: Enigo,
}

impl Gui {
    /// Create a new `Gui` with default `enigo` settings.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Gui`] if there is no desktop session to
    /// send input to, or the operating system refuses it.
    pub fn new() -> Result<Self, SeleniumBaseError> {
        let enigo = Enigo::new(&Settings::default())?;
        Ok(Self { enigo })
    }

    /// Move the mouse cursor to absolute screen coordinates `(x, y)`.
    pub fn move_mouse(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        Ok(())
    }

    /// Click the left mouse button at the current cursor position.
    pub fn click(&mut self) -> Result<(), SeleniumBaseError> {
        self.enigo.button(Button::Left, Direction::Click)?;
        Ok(())
    }

    /// Click the right mouse button at the current cursor position.
    pub fn right_click(&mut self) -> Result<(), SeleniumBaseError> {
        self.enigo.button(Button::Right, Direction::Click)?;
        Ok(())
    }

    /// Double-click the left mouse button at the current cursor position.
    pub fn double_click(&mut self) -> Result<(), SeleniumBaseError> {
        self.click()?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.click()?;
        Ok(())
    }

    /// Scroll the mouse wheel horizontally by `x` clicks and vertically by `y`
    /// clicks. Positive values scroll right/down.
    pub fn scroll(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        if x != 0 {
            self.enigo.scroll(x, Axis::Horizontal)?;
        }
        if y != 0 {
            self.enigo.scroll(y, Axis::Vertical)?;
        }
        Ok(())
    }

    /// Press and release a single key identified by name.
    ///
    /// Names are case-insensitive and include `enter`, `return`, `tab`, `esc`,
    /// `escape`, `backspace`, `delete`, `space`, `ctrl`, `alt`, `shift`, `meta`,
    /// `command`, arrow keys, function keys, and single unicode characters.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Gui`] for an unknown key name or if the
    /// system rejects the input.
    pub fn press_key(&mut self, key: &str) -> Result<(), SeleniumBaseError> {
        let k = Self::key_from_str(key)
            .ok_or_else(|| SeleniumBaseError::Gui(format!("unknown key: {key}")))?;
        self.enigo.key(k, Direction::Click)?;
        Ok(())
    }

    /// Press a chord of keys. Modifier keys (`ctrl`, `alt`, `shift`, `meta`,
    /// `command`) are held down while non-modifier keys are clicked.
    ///
    /// Example: `gui.press_keys(&["ctrl", "a"])` selects all.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Gui`] for an unknown key name (before any
    /// key is pressed) or if the system rejects the input. Modifiers already
    /// pressed are released before an input error is returned.
    pub fn press_keys(&mut self, keys: &[&str]) -> Result<(), SeleniumBaseError> {
        let mut modifiers = Vec::new();
        let mut normal = Vec::new();

        for key in keys {
            let k = Self::key_from_str(key)
                .ok_or_else(|| SeleniumBaseError::Gui(format!("unknown key: {key}")))?;
            if Self::is_modifier(k) {
                modifiers.push(k);
            } else {
                normal.push(k);
            }
        }

        press_chord(&mut self.enigo, &modifiers, &normal)
    }

    /// Type the given unicode text using the fastest available input method.
    pub fn write(&mut self, text: &str) -> Result<(), SeleniumBaseError> {
        self.enigo.text(text)?;
        Ok(())
    }

    /// Drag the left mouse button from `from` to `to`.
    ///
    /// The pointer travels in small steps rather than jumping, so that drag
    /// handlers see it move. The button is released even if a move fails.
    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), SeleniumBaseError> {
        drag_between(&mut self.enigo, from, to)
    }

    /// Return the size of the main display in pixels as `(width, height)`.
    pub fn screen_size(&self) -> Result<(i32, i32), SeleniumBaseError> {
        Ok(self.enigo.main_display()?)
    }

    /// Press the left mouse button without releasing it.
    pub fn mouse_down(&mut self) -> Result<(), SeleniumBaseError> {
        self.enigo.button(Button::Left, Direction::Press)?;
        Ok(())
    }

    /// Release the left mouse button.
    pub fn mouse_up(&mut self) -> Result<(), SeleniumBaseError> {
        self.enigo.button(Button::Left, Direction::Release)?;
        Ok(())
    }

    /// Convert a case-insensitive key name to an `enigo::Key`.
    ///
    /// Returns `None` when the name cannot be mapped.
    pub fn key_from_str(name: &str) -> Option<Key> {
        if name.chars().count() == 1 {
            let ch = name.chars().next().unwrap();
            return Some(Key::Unicode(ch));
        }
        match name.to_lowercase().as_str() {
            "enter" | "return" => Some(Key::Return),
            "tab" => Some(Key::Tab),
            "esc" | "escape" => Some(Key::Escape),
            "backspace" => Some(Key::Backspace),
            "delete" | "del" => Some(Key::Delete),
            "space" => Some(Key::Space),
            "ctrl" | "control" => Some(Key::Control),
            "alt" => Some(Key::Alt),
            "shift" => Some(Key::Shift),
            "meta" | "command" | "cmd" | "win" | "windows" | "super" => Some(Key::Meta),
            "home" => Some(Key::Home),
            "end" => Some(Key::End),
            "pageup" => Some(Key::PageUp),
            "pagedown" => Some(Key::PageDown),
            "up" | "uparrow" => Some(Key::UpArrow),
            "down" | "downarrow" => Some(Key::DownArrow),
            "left" | "leftarrow" => Some(Key::LeftArrow),
            "right" | "rightarrow" => Some(Key::RightArrow),
            "capslock" => Some(Key::CapsLock),
            "f1" => Some(Key::F1),
            "f2" => Some(Key::F2),
            "f3" => Some(Key::F3),
            "f4" => Some(Key::F4),
            "f5" => Some(Key::F5),
            "f6" => Some(Key::F6),
            "f7" => Some(Key::F7),
            "f8" => Some(Key::F8),
            "f9" => Some(Key::F9),
            "f10" => Some(Key::F10),
            "f11" => Some(Key::F11),
            "f12" => Some(Key::F12),
            _ => None,
        }
    }

    fn is_modifier(key: Key) -> bool {
        matches!(key, Key::Control | Key::Alt | Key::Shift | Key::Meta)
    }
}

impl From<enigo::InputError> for SeleniumBaseError {
    fn from(err: enigo::InputError) -> Self {
        SeleniumBaseError::Gui(err.to_string())
    }
}

impl From<enigo::NewConError> for SeleniumBaseError {
    fn from(err: enigo::NewConError) -> Self {
        SeleniumBaseError::Gui(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{drag_between, drag_waypoints, press_chord, Gui, InputDevice, DRAG_STEPS};
    use crate::error::SeleniumBaseError;
    use enigo::{Direction, Key};
    use std::time::Duration;

    /// Records what it is asked to do, and fails on the `fail_on`th call.
    #[derive(Default)]
    struct Recorder {
        log: Vec<String>,
        fail_on: Option<usize>,
        calls: usize,
    }

    impl Recorder {
        fn record(&mut self, entry: String) -> Result<(), SeleniumBaseError> {
            self.calls += 1;
            if self.fail_on == Some(self.calls) {
                return Err(SeleniumBaseError::Gui(format!("failed: {entry}")));
            }
            self.log.push(entry);
            Ok(())
        }
    }

    impl InputDevice for Recorder {
        fn send_key(&mut self, key: Key, direction: Direction) -> Result<(), SeleniumBaseError> {
            self.record(format!("{key:?} {direction:?}"))
        }
        fn move_pointer(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
            self.record(format!("move {x},{y}"))
        }
        fn send_left_button(&mut self, direction: Direction) -> Result<(), SeleniumBaseError> {
            self.record(format!("button {direction:?}"))
        }
        fn pause(&mut self, _: Duration) {}
    }

    #[test]
    fn key_mapping_common_keys() {
        assert_eq!(Gui::key_from_str("enter"), Some(Key::Return));
        assert_eq!(Gui::key_from_str("return"), Some(Key::Return));
        assert_eq!(Gui::key_from_str("tab"), Some(Key::Tab));
        assert_eq!(Gui::key_from_str("Escape"), Some(Key::Escape));
        assert_eq!(Gui::key_from_str("BACKSPACE"), Some(Key::Backspace));
        assert_eq!(Gui::key_from_str("delete"), Some(Key::Delete));
        assert_eq!(Gui::key_from_str("space"), Some(Key::Space));
    }

    #[test]
    fn key_mapping_modifiers() {
        assert_eq!(Gui::key_from_str("ctrl"), Some(Key::Control));
        assert_eq!(Gui::key_from_str("alt"), Some(Key::Alt));
        assert_eq!(Gui::key_from_str("shift"), Some(Key::Shift));
        assert_eq!(Gui::key_from_str("meta"), Some(Key::Meta));
        assert_eq!(Gui::key_from_str("command"), Some(Key::Meta));
        assert_eq!(Gui::key_from_str("win"), Some(Key::Meta));
    }

    #[test]
    fn key_mapping_unicode_single_character() {
        assert_eq!(Gui::key_from_str("a"), Some(Key::Unicode('a')));
        assert_eq!(Gui::key_from_str("A"), Some(Key::Unicode('A')));
        assert_eq!(Gui::key_from_str("1"), Some(Key::Unicode('1')));
        assert_eq!(Gui::key_from_str("é"), Some(Key::Unicode('é')));
    }

    #[test]
    fn key_mapping_unknown_returns_none() {
        assert_eq!(Gui::key_from_str("not-a-key"), None);
    }

    #[test]
    fn key_mapping_function_and_arrow_keys() {
        assert_eq!(Gui::key_from_str("f1"), Some(Key::F1));
        assert_eq!(Gui::key_from_str("f12"), Some(Key::F12));
        assert_eq!(Gui::key_from_str("up"), Some(Key::UpArrow));
        assert_eq!(Gui::key_from_str("leftarrow"), Some(Key::LeftArrow));
    }

    #[test]
    fn a_chord_holds_modifiers_while_it_clicks_and_releases_them_in_reverse() {
        let mut device = Recorder::default();
        press_chord(
            &mut device,
            &[Key::Control, Key::Shift],
            &[Key::Unicode('a')],
        )
        .unwrap();
        assert_eq!(
            device.log,
            [
                "Control Press",
                "Shift Press",
                "Unicode('a') Click",
                "Shift Release",
                "Control Release",
            ]
        );
    }

    #[test]
    fn a_failed_key_does_not_leave_the_modifiers_pressed() {
        // Calls: 1 = Control down, 2 = Shift down, 3 = 'a' click (fails).
        let mut device = Recorder {
            fail_on: Some(3),
            ..Recorder::default()
        };
        let result = press_chord(
            &mut device,
            &[Key::Control, Key::Shift],
            &[Key::Unicode('a')],
        );
        assert!(matches!(result, Err(SeleniumBaseError::Gui(m)) if m.contains("Unicode('a')")));
        assert_eq!(
            device.log,
            [
                "Control Press",
                "Shift Press",
                "Shift Release",
                "Control Release"
            ],
            "both modifiers are released after the failure"
        );
    }

    #[test]
    fn a_modifier_that_never_went_down_is_not_released() {
        let mut device = Recorder {
            fail_on: Some(2),
            ..Recorder::default()
        };
        assert!(press_chord(&mut device, &[Key::Control, Key::Shift], &[Key::Tab]).is_err());
        assert_eq!(device.log, ["Control Press", "Control Release"]);
    }

    #[test]
    fn a_drag_travels_between_its_end_points() {
        let points = drag_waypoints((0, 0), (300, 0));
        assert_eq!(points.len(), 19, "300px at 15px a step");
        assert_eq!(points.first(), Some(&(15, 0)));
        assert_eq!(points.last(), Some(&(285, 0)));
        assert!(
            points.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "moves one way only"
        );
    }

    #[test]
    fn a_drag_has_a_few_steps_however_short_and_a_bounded_number_however_long() {
        assert_eq!(
            drag_waypoints((10, 10), (12, 10)).len(),
            DRAG_STEPS.start() - 1
        );
        assert!(drag_waypoints((5, 5), (5, 5)).is_empty());
        assert_eq!(
            drag_waypoints((0, 0), (100_000, 0)).len(),
            DRAG_STEPS.end() - 1
        );
    }

    #[test]
    fn a_drag_presses_moves_and_releases_in_order() {
        let mut device = Recorder::default();
        drag_between(&mut device, (0, 0), (60, 0)).unwrap();
        assert_eq!(device.log.first().map(String::as_str), Some("move 0,0"));
        assert_eq!(device.log[1], "button Press");
        assert_eq!(device.log[device.log.len() - 2], "move 60,0");
        assert_eq!(
            device.log.last().map(String::as_str),
            Some("button Release")
        );
        assert!(
            device.log.iter().filter(|e| e.starts_with("move")).count() > 3,
            "the pointer moves in steps, not in one jump: {:?}",
            device.log
        );
    }

    #[test]
    fn a_failed_move_still_releases_the_button() {
        // Calls: 1 = move to start, 2 = press, 3 = first waypoint (fails).
        let mut device = Recorder {
            fail_on: Some(3),
            ..Recorder::default()
        };
        assert!(drag_between(&mut device, (0, 0), (60, 0)).is_err());
        assert_eq!(
            device.log.last().map(String::as_str),
            Some("button Release")
        );
    }
}
