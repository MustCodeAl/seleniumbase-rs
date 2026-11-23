//! Special keys defined by W3C WebDriver and Python Selenium (`selenium.webdriver.common.keys.Keys`).
//!
//! Each constant is the canonical Unicode PUA (Private Use Area) string,
//! recognized by WebDriver drivers and translated natively by the pure CDP engine.

use crate::sb_cdp::Key;

/// Special keyboard keys matching Python Selenium's `selenium.webdriver.common.keys.Keys`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Keys;
///
/// assert_eq!(Keys::ENTER, "\u{e007}");
/// assert_eq!(Keys::RETURN, "\u{e006}");
/// assert_eq!(Keys::TAB, "\u{e004}");
/// assert_eq!(Keys::BACKSPACE, "\u{e003}");
/// assert_eq!(Keys::BACK_SPACE, Keys::BACKSPACE);
/// assert_eq!(Keys::ESCAPE, "\u{e00c}");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keys;

impl Keys {
    /// Null key (`\u{e000}`).
    pub const NULL: &'static str = "\u{e000}";
    /// Cancel key (`\u{e001}`).
    pub const CANCEL: &'static str = "\u{e001}";
    /// Help key (`\u{e002}`).
    pub const HELP: &'static str = "\u{e002}";
    /// Backspace key (`\u{e003}`).
    pub const BACKSPACE: &'static str = "\u{e003}";
    /// Alias for [`BACKSPACE`](Self::BACKSPACE).
    pub const BACK_SPACE: &'static str = Self::BACKSPACE;
    /// Tab key (`\u{e004}`).
    pub const TAB: &'static str = "\u{e004}";
    /// Clear key (`\u{e005}`).
    pub const CLEAR: &'static str = "\u{e005}";
    /// Return key (`\u{e006}`).
    pub const RETURN: &'static str = "\u{e006}";
    /// Enter key (`\u{e007}`).
    pub const ENTER: &'static str = "\u{e007}";
    /// Shift key (`\u{e008}`).
    pub const SHIFT: &'static str = "\u{e008}";
    /// Alias for [`SHIFT`](Self::SHIFT).
    pub const LEFT_SHIFT: &'static str = Self::SHIFT;
    /// Control key (`\u{e009}`).
    pub const CONTROL: &'static str = "\u{e009}";
    /// Alias for [`CONTROL`](Self::CONTROL).
    pub const LEFT_CONTROL: &'static str = Self::CONTROL;
    /// Alt key (`\u{e00a}`).
    pub const ALT: &'static str = "\u{e00a}";
    /// Alias for [`ALT`](Self::ALT).
    pub const LEFT_ALT: &'static str = Self::ALT;
    /// Pause key (`\u{e00b}`).
    pub const PAUSE: &'static str = "\u{e00b}";
    /// Escape key (`\u{e00c}`).
    pub const ESCAPE: &'static str = "\u{e00c}";
    /// Space key (`\u{e00d}`).
    pub const SPACE: &'static str = "\u{e00d}";
    /// Page Up key (`\u{e00e}`).
    pub const PAGE_UP: &'static str = "\u{e00e}";
    /// Page Down key (`\u{e00f}`).
    pub const PAGE_DOWN: &'static str = "\u{e00f}";
    /// End key (`\u{e010}`).
    pub const END: &'static str = "\u{e010}";
    /// Home key (`\u{e011}`).
    pub const HOME: &'static str = "\u{e011}";
    /// Left arrow key (`\u{e012}`).
    pub const LEFT: &'static str = "\u{e012}";
    /// Alias for [`LEFT`](Self::LEFT).
    pub const ARROW_LEFT: &'static str = Self::LEFT;
    /// Up arrow key (`\u{e013}`).
    pub const UP: &'static str = "\u{e013}";
    /// Alias for [`UP`](Self::UP).
    pub const ARROW_UP: &'static str = Self::UP;
    /// Right arrow key (`\u{e014}`).
    pub const RIGHT: &'static str = "\u{e014}";
    /// Alias for [`RIGHT`](Self::RIGHT).
    pub const ARROW_RIGHT: &'static str = Self::RIGHT;
    /// Down arrow key (`\u{e015}`).
    pub const DOWN: &'static str = "\u{e015}";
    /// Alias for [`DOWN`](Self::DOWN).
    pub const ARROW_DOWN: &'static str = Self::DOWN;
    /// Insert key (`\u{e016}`).
    pub const INSERT: &'static str = "\u{e016}";
    /// Delete key (`\u{e017}`).
    pub const DELETE: &'static str = "\u{e017}";
    /// Semicolon key (`\u{e018}`).
    pub const SEMICOLON: &'static str = "\u{e018}";
    /// Equals key (`\u{e019}`).
    pub const EQUALS: &'static str = "\u{e019}";
    /// Numpad 0 key (`\u{e01a}`).
    pub const NUMPAD0: &'static str = "\u{e01a}";
    /// Numpad 1 key (`\u{e01b}`).
    pub const NUMPAD1: &'static str = "\u{e01b}";
    /// Numpad 2 key (`\u{e01c}`).
    pub const NUMPAD2: &'static str = "\u{e01c}";
    /// Numpad 3 key (`\u{e01d}`).
    pub const NUMPAD3: &'static str = "\u{e01d}";
    /// Numpad 4 key (`\u{e01e}`).
    pub const NUMPAD4: &'static str = "\u{e01e}";
    /// Numpad 5 key (`\u{e01f}`).
    pub const NUMPAD5: &'static str = "\u{e01f}";
    /// Numpad 6 key (`\u{e020}`).
    pub const NUMPAD6: &'static str = "\u{e020}";
    /// Numpad 7 key (`\u{e021}`).
    pub const NUMPAD7: &'static str = "\u{e021}";
    /// Numpad 8 key (`\u{e022}`).
    pub const NUMPAD8: &'static str = "\u{e022}";
    /// Numpad 9 key (`\u{e023}`).
    pub const NUMPAD9: &'static str = "\u{e023}";
    /// Multiply key (`\u{e024}`).
    pub const MULTIPLY: &'static str = "\u{e024}";
    /// Add key (`\u{e025}`).
    pub const ADD: &'static str = "\u{e025}";
    /// Separator key (`\u{e026}`).
    pub const SEPARATOR: &'static str = "\u{e026}";
    /// Subtract key (`\u{e027}`).
    pub const SUBTRACT: &'static str = "\u{e027}";
    /// Decimal key (`\u{e028}`).
    pub const DECIMAL: &'static str = "\u{e028}";
    /// Divide key (`\u{e029}`).
    pub const DIVIDE: &'static str = "\u{e029}";
    /// F1 key (`\u{e031}`).
    pub const F1: &'static str = "\u{e031}";
    /// F2 key (`\u{e032}`).
    pub const F2: &'static str = "\u{e032}";
    /// F3 key (`\u{e033}`).
    pub const F3: &'static str = "\u{e033}";
    /// F4 key (`\u{e034}`).
    pub const F4: &'static str = "\u{e034}";
    /// F5 key (`\u{e035}`).
    pub const F5: &'static str = "\u{e035}";
    /// F6 key (`\u{e036}`).
    pub const F6: &'static str = "\u{e036}";
    /// F7 key (`\u{e037}`).
    pub const F7: &'static str = "\u{e037}";
    /// F8 key (`\u{e038}`).
    pub const F8: &'static str = "\u{e038}";
    /// F9 key (`\u{e039}`).
    pub const F9: &'static str = "\u{e039}";
    /// F10 key (`\u{e03a}`).
    pub const F10: &'static str = "\u{e03a}";
    /// F11 key (`\u{e03b}`).
    pub const F11: &'static str = "\u{e03b}";
    /// F12 key (`\u{e03c}`).
    pub const F12: &'static str = "\u{e03c}";
    /// Meta key (`\u{e03d}`).
    pub const META: &'static str = "\u{e03d}";
    /// Alias for [`META`](Self::META).
    pub const COMMAND: &'static str = Self::META;

    /// Converts a character (such as a W3C WebDriver PUA code point) to a [`Key`],
    /// or `None` if the character is not a special key.
    #[must_use]
    pub fn to_key(ch: char) -> Option<Key> {
        match ch {
            '\n' | '\r' | '\u{e006}' | '\u{e007}' => Some(Key::Enter),
            '\t' | '\u{e004}' => Some(Key::Tab),
            '\u{e003}' => Some(Key::Backspace),
            '\u{e00c}' => Some(Key::Escape),
            '\u{e017}' => Some(Key::Delete),
            '\u{e016}' => Some(Key::Insert),
            '\u{e00d}' => Some(Key::Space),
            '\u{e00b}' => Some(Key::Pause),
            '\u{e008}' => Some(Key::Shift),
            '\u{e009}' => Some(Key::Control),
            '\u{e00a}' => Some(Key::Alt),
            '\u{e03d}' => Some(Key::Meta),
            '\u{e010}' => Some(Key::End),
            '\u{e011}' => Some(Key::Home),
            '\u{e012}' => Some(Key::ArrowLeft),
            '\u{e013}' => Some(Key::ArrowUp),
            '\u{e014}' => Some(Key::ArrowRight),
            '\u{e015}' => Some(Key::ArrowDown),
            '\u{e00e}' => Some(Key::PageUp),
            '\u{e00f}' => Some(Key::PageDown),
            '\u{e01a}' => Some(Key::NumPad0),
            '\u{e01b}' => Some(Key::NumPad1),
            '\u{e01c}' => Some(Key::NumPad2),
            '\u{e01d}' => Some(Key::NumPad3),
            '\u{e01e}' => Some(Key::NumPad4),
            '\u{e01f}' => Some(Key::NumPad5),
            '\u{e020}' => Some(Key::NumPad6),
            '\u{e021}' => Some(Key::NumPad7),
            '\u{e022}' => Some(Key::NumPad8),
            '\u{e023}' => Some(Key::NumPad9),
            '\u{e024}' => Some(Key::Multiply),
            '\u{e025}' => Some(Key::Add),
            '\u{e027}' => Some(Key::Subtract),
            '\u{e028}' => Some(Key::Decimal),
            '\u{e029}' => Some(Key::Divide),
            '\u{e031}' => Some(Key::F1),
            '\u{e032}' => Some(Key::F2),
            '\u{e033}' => Some(Key::F3),
            '\u{e034}' => Some(Key::F4),
            '\u{e035}' => Some(Key::F5),
            '\u{e036}' => Some(Key::F6),
            '\u{e037}' => Some(Key::F7),
            '\u{e038}' => Some(Key::F8),
            '\u{e039}' => Some(Key::F9),
            '\u{e03a}' => Some(Key::F10),
            '\u{e03b}' => Some(Key::F11),
            '\u{e03c}' => Some(Key::F12),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_keys_match_selenium_spec() {
        assert_eq!(Keys::NULL, "\u{e000}");
        assert_eq!(Keys::CANCEL, "\u{e001}");
        assert_eq!(Keys::HELP, "\u{e002}");
        assert_eq!(Keys::BACKSPACE, "\u{e003}");
        assert_eq!(Keys::BACK_SPACE, "\u{e003}");
        assert_eq!(Keys::TAB, "\u{e004}");
        assert_eq!(Keys::CLEAR, "\u{e005}");
        assert_eq!(Keys::RETURN, "\u{e006}");
        assert_eq!(Keys::ENTER, "\u{e007}");
        assert_eq!(Keys::SHIFT, "\u{e008}");
        assert_eq!(Keys::LEFT_SHIFT, "\u{e008}");
        assert_eq!(Keys::CONTROL, "\u{e009}");
        assert_eq!(Keys::LEFT_CONTROL, "\u{e009}");
        assert_eq!(Keys::ALT, "\u{e00a}");
        assert_eq!(Keys::LEFT_ALT, "\u{e00a}");
        assert_eq!(Keys::PAUSE, "\u{e00b}");
        assert_eq!(Keys::ESCAPE, "\u{e00c}");
        assert_eq!(Keys::SPACE, "\u{e00d}");
        assert_eq!(Keys::PAGE_UP, "\u{e00e}");
        assert_eq!(Keys::PAGE_DOWN, "\u{e00f}");
        assert_eq!(Keys::END, "\u{e010}");
        assert_eq!(Keys::HOME, "\u{e011}");
        assert_eq!(Keys::LEFT, "\u{e012}");
        assert_eq!(Keys::ARROW_LEFT, "\u{e012}");
        assert_eq!(Keys::UP, "\u{e013}");
        assert_eq!(Keys::ARROW_UP, "\u{e013}");
        assert_eq!(Keys::RIGHT, "\u{e014}");
        assert_eq!(Keys::ARROW_RIGHT, "\u{e014}");
        assert_eq!(Keys::DOWN, "\u{e015}");
        assert_eq!(Keys::ARROW_DOWN, "\u{e015}");
        assert_eq!(Keys::INSERT, "\u{e016}");
        assert_eq!(Keys::DELETE, "\u{e017}");
        assert_eq!(Keys::SEMICOLON, "\u{e018}");
        assert_eq!(Keys::EQUALS, "\u{e019}");
        assert_eq!(Keys::NUMPAD0, "\u{e01a}");
        assert_eq!(Keys::MULTIPLY, "\u{e024}");
        assert_eq!(Keys::ADD, "\u{e025}");
        assert_eq!(Keys::SUBTRACT, "\u{e027}");
        assert_eq!(Keys::DECIMAL, "\u{e028}");
        assert_eq!(Keys::DIVIDE, "\u{e029}");
        assert_eq!(Keys::F1, "\u{e031}");
        assert_eq!(Keys::F12, "\u{e03c}");
        assert_eq!(Keys::META, "\u{e03d}");
        assert_eq!(Keys::COMMAND, "\u{e03d}");
    }

    #[test]
    fn to_key_translation() {
        assert_eq!(Keys::to_key('\n'), Some(Key::Enter));
        assert_eq!(Keys::to_key('\r'), Some(Key::Enter));
        assert_eq!(Keys::to_key('\u{e007}'), Some(Key::Enter));
        assert_eq!(Keys::to_key('\u{e006}'), Some(Key::Enter));
        assert_eq!(Keys::to_key('\t'), Some(Key::Tab));
        assert_eq!(Keys::to_key('\u{e004}'), Some(Key::Tab));
        assert_eq!(Keys::to_key('\u{e003}'), Some(Key::Backspace));
        assert_eq!(Keys::to_key('\u{e00c}'), Some(Key::Escape));
        assert_eq!(Keys::to_key('\u{e015}'), Some(Key::ArrowDown));
        assert_eq!(Keys::to_key('a'), None);
    }
}
