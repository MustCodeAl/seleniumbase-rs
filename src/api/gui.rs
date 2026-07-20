//! Native GUI automation wrapper around `enigo`.
//!
//! This module exposes a small, ergonomic `Gui` struct for simulating mouse and
//! keyboard input at the OS level. It is intentionally separate from the
//! WebDriver-based `BaseCase` API so it can be used standalone or composed into
//! higher-level helper methods.

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use crate::error::SeleniumBaseError;

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

        for m in &modifiers {
            self.enigo.key(*m, Direction::Press)?;
        }

        for k in &normal {
            self.enigo.key(*k, Direction::Click)?;
        }

        for m in modifiers.iter().rev() {
            self.enigo.key(*m, Direction::Release)?;
        }

        Ok(())
    }

    /// Type the given unicode text using the fastest available input method.
    pub fn write(&mut self, text: &str) -> Result<(), SeleniumBaseError> {
        self.enigo.text(text)?;
        Ok(())
    }

    /// Drag the left mouse button from `from` to `to`.
    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), SeleniumBaseError> {
        self.enigo.move_mouse(from.0, from.1, Coordinate::Abs)?;
        self.enigo.button(Button::Left, Direction::Press)?;
        self.enigo.move_mouse(to.0, to.1, Coordinate::Abs)?;
        self.enigo.button(Button::Left, Direction::Release)?;
        Ok(())
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
    use super::Gui;
    use enigo::Key;

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
}
