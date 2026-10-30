//! Terminal colour for the crate's own output: one decision and one palette.
//!
//! Everything that writes coloured text for a person to read asks this module
//! two questions: the log lines of [`tracing_util`](crate::tracing_util), the
//! recorder's banners, the messages of `sbrs`, and later the debug prompts.
//!
//! 1. **May I colour?** [`color_allowed`] and [`use_color`] say yes only when
//!    the destination is a terminal, `NO_COLOR` is unset or empty
//!    (<https://no-color.org>) and `TERM` is not `dumb`. A pipe, a file, a CI
//!    capture and JSON logs (`SB_LOG_FORMAT=json`) therefore get plain text.
//! 2. **With what?** A [`Style`] is a foreground, a background and the
//!    attributes bold, dim and underline. The named styles in [`palette`] keep
//!    the colours in one place; the banner ones are the colorama pairs that
//!    Python SeleniumBase prints its `sbase mkrec` banners with, so the two
//!    tools look alike.
//!
//! Painting never decides for itself. [`Style::paint`] takes the answer to the
//! first question, so a caller that owns a stream decides once, and a test
//! forces either answer without a terminal.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::common::color::{palette, use_color};
//!
//! // False when standard output is piped, or when NO_COLOR is set.
//! let on = use_color(&std::io::stdout());
//! let shown = palette::BANNER_FILE.paint("login_rec.rs", on);
//! assert!(shown.contains("login_rec.rs"));
//!
//! // Forced on, it is the colorama pair that Python prints.
//! assert_eq!(
//!     palette::BANNER_FILE.paint("x.rs", true),
//!     "\x1b[31;103mx.rs\x1b[0m"
//! );
//! ```

use std::ffi::OsStr;
use std::fmt::{self, Display, Write as _};
use std::io::IsTerminal;

/// The escape sequence that resets every attribute.
pub const RESET: &str = "\x1b[0m";

/// Decides whether colour may be used, from the facts that matter.
///
/// `no_color` and `term` are the values of the `NO_COLOR` and `TERM`
/// environment variables, if set. An empty `NO_COLOR` does not count.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::color::color_allowed;
///
/// assert!(color_allowed(true, None, Some("xterm-256color".as_ref())));
/// assert!(!color_allowed(false, None, None), "a pipe or a file");
/// assert!(!color_allowed(true, Some("1".as_ref()), None), "NO_COLOR");
/// ```
#[must_use]
pub fn color_allowed(is_terminal: bool, no_color: Option<&OsStr>, term: Option<&OsStr>) -> bool {
    is_terminal
        && no_color.is_none_or(OsStr::is_empty)
        && term.is_none_or(|term| term != OsStr::new("dumb"))
}

/// Whether text written to `stream` should be coloured right now: it is a
/// terminal and the environment (`NO_COLOR`, `TERM`) allows colour.
///
/// `stream` is any handle that knows whether it is a terminal, usually
/// `&std::io::stdout()` or `&std::io::stderr()`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::color::use_color;
///
/// let on = use_color(&std::io::stderr());
/// println!("colour on standard error: {on}");
/// ```
#[must_use]
pub fn use_color(stream: &impl IsTerminal) -> bool {
    color_allowed(
        stream.is_terminal(),
        std::env::var_os("NO_COLOR").as_deref(),
        std::env::var_os("TERM").as_deref(),
    )
}

/// Replaces every control character except newline and tab with U+FFFD.
///
/// Terminals act on escape sequences, so text that came from outside the
/// program (a page title, a server's error body) must not reach one as is.
#[must_use]
pub fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
}

/// One of the sixteen ANSI colours, named as colorama names them
/// (`Fore.LIGHTRED_EX` is [`Color::LightRed`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    /// ANSI 30.
    Black,
    /// ANSI 31.
    Red,
    /// ANSI 32.
    Green,
    /// ANSI 33.
    Yellow,
    /// ANSI 34.
    Blue,
    /// ANSI 35.
    Magenta,
    /// ANSI 36.
    Cyan,
    /// ANSI 37.
    White,
    /// ANSI 90, a dark grey.
    LightBlack,
    /// ANSI 91.
    LightRed,
    /// ANSI 92.
    LightGreen,
    /// ANSI 93.
    LightYellow,
    /// ANSI 94.
    LightBlue,
    /// ANSI 95.
    LightMagenta,
    /// ANSI 96.
    LightCyan,
    /// ANSI 97.
    LightWhite,
}

impl Color {
    /// The code of the colour as a foreground; a background is 10 more.
    const fn foreground(self) -> u8 {
        match self {
            Self::Black => 30,
            Self::Red => 31,
            Self::Green => 32,
            Self::Yellow => 33,
            Self::Blue => 34,
            Self::Magenta => 35,
            Self::Cyan => 36,
            Self::White => 37,
            Self::LightBlack => 90,
            Self::LightRed => 91,
            Self::LightGreen => 92,
            Self::LightYellow => 93,
            Self::LightBlue => 94,
            Self::LightMagenta => 95,
            Self::LightCyan => 96,
            Self::LightWhite => 97,
        }
    }
}

/// A foreground, a background and the attributes bold, dim and underline.
///
/// Built from [`Style::PLAIN`] with the `const` builders, so a palette can be
/// made of constants:
///
/// ```
/// use seleniumbase_rs::common::color::{Color, Style};
///
/// const WARNING: Style = Style::PLAIN.fg(Color::Yellow).bold();
/// assert_eq!(WARNING.paint("slow", true), "\x1b[1;33mslow\x1b[0m");
/// assert_eq!(WARNING.paint("slow", false), "slow");
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    dim: bool,
    underline: bool,
}

impl Style {
    /// No colour and no attribute: painting with it changes nothing.
    pub const PLAIN: Self = Self {
        fg: None,
        bg: None,
        bold: false,
        dim: false,
        underline: false,
    };

    /// This style with the foreground `color`.
    #[must_use]
    pub const fn fg(self, color: Color) -> Self {
        Self {
            fg: Some(color),
            ..self
        }
    }

    /// This style with the background `color`.
    #[must_use]
    pub const fn on(self, color: Color) -> Self {
        Self {
            bg: Some(color),
            ..self
        }
    }

    /// This style, bold.
    #[must_use]
    pub const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// This style, dim (faint).
    #[must_use]
    pub const fn dim(self) -> Self {
        Self { dim: true, ..self }
    }

    /// This style, underlined.
    #[must_use]
    pub const fn underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }

    /// Whether the style has no colour and no attribute.
    #[must_use]
    pub const fn is_plain(self) -> bool {
        self.fg.is_none() && self.bg.is_none() && !self.bold && !self.dim && !self.underline
    }

    /// The escape sequence that starts the style (one sequence, such as
    /// `\x1b[1;34;106m`), or an empty string for [`Style::PLAIN`].
    #[must_use]
    pub fn escape(self) -> String {
        if self.is_plain() {
            return String::new();
        }
        let mut codes: Vec<u8> = Vec::with_capacity(5);
        if self.bold {
            codes.push(1);
        }
        if self.dim {
            codes.push(2);
        }
        if self.underline {
            codes.push(4);
        }
        if let Some(color) = self.fg {
            codes.push(color.foreground());
        }
        if let Some(color) = self.bg {
            codes.push(color.foreground() + 10);
        }
        let mut sequence = String::from("\x1b[");
        for (index, code) in codes.iter().enumerate() {
            if index > 0 {
                sequence.push(';');
            }
            let _ = write!(sequence, "{code}");
        }
        sequence.push('m');
        sequence
    }

    /// `text` in this style when `on` is set, else `text` as it is.
    ///
    /// `on` is the answer of [`use_color`] (or of a test). Nothing is
    /// sanitized: pass text that came from outside through [`sanitize`] first.
    #[must_use]
    pub fn paint(self, text: impl Display, on: bool) -> String {
        let mut out = String::new();
        let _ = self.write(&mut out, text, on);
        out
    }

    /// Writes `text` in this style to `out` when `on` is set, else as it is.
    ///
    /// # Errors
    ///
    /// Returns the error of `out`.
    pub fn write(self, out: &mut impl fmt::Write, text: impl Display, on: bool) -> fmt::Result {
        if on && !self.is_plain() {
            write!(out, "{}{text}{RESET}", self.escape())
        } else {
            write!(out, "{text}")
        }
    }
}

/// The named styles.
///
/// The `BANNER_*` ones are the colorama pairs of Python SeleniumBase's
/// `console_scripts/sb_mkrec.py` (`c0` to `c7`); the rest colour the log lines
/// and are for any message that means the same thing.
pub mod palette {
    use super::{Color, Style};

    /// `c0`: blue on light cyan. The label `* RECORDING initialized:`.
    pub const BANNER_LABEL: Style = Style::PLAIN.fg(Color::Blue).on(Color::LightCyan);
    /// `c1` and `c5`: red on light yellow. A file name in a banner, and an
    /// `ERROR: ...` message.
    pub const BANNER_FILE: Style = Style::PLAIN.fg(Color::Red).on(Color::LightYellow);
    /// `c2`: light red on light yellow. The `>>>` and `***` that start a banner.
    pub const BANNER_MARK: Style = Style::PLAIN.fg(Color::LightRed).on(Color::LightYellow);
    /// `c7`: black on magenta. The `>>` and `<<` around an invalid option.
    pub const BANNER_OPTION: Style = Style::PLAIN.fg(Color::Black).on(Color::Magenta);

    /// Faint text that is context, not content: a time, a module path, a field name.
    pub const DIM: Style = Style::PLAIN.dim();
    /// A file or folder path.
    pub const PATH: Style = Style::PLAIN.fg(Color::Cyan);
    /// A test, a scenario or a step.
    pub const STEP: Style = Style::PLAIN.fg(Color::Magenta).bold();
    /// Something went wrong and the run stops.
    pub const ERROR: Style = Style::PLAIN.fg(Color::Red).bold();
    /// Something is wrong but the run goes on.
    pub const WARNING: Style = Style::PLAIN.fg(Color::Yellow).bold();
    /// Progress, or a result that was asked for.
    pub const SUCCESS: Style = Style::PLAIN.fg(Color::Green).bold();
    /// Detail that helps when something goes wrong.
    pub const DETAIL: Style = Style::PLAIN.fg(Color::Blue).bold();
    /// The finest detail.
    pub const TRACE: Style = Style::PLAIN.fg(Color::Magenta);
}

/// A banner as Python SeleniumBase prints it: `lead` in `lead_style`, then
/// `rest`, a space, and `subject` (a file name) in the file style.
///
/// ```text
/// >>> RECORDING SAVED as: recordings/login_rec.rs
/// ```
///
/// is `banner(">>>", palette::BANNER_MARK, " RECORDING SAVED as:", path, on)`:
/// only `>>>` and the path are coloured. With `on` unset the text is the
/// same without escape sequences.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::color::{banner, palette};
///
/// let plain = banner("***", palette::BANNER_MARK, " RECORDING COPIED to:", "x.rs", false);
/// assert_eq!(plain, "*** RECORDING COPIED to: x.rs");
/// ```
#[must_use]
pub fn banner(lead: &str, lead_style: Style, rest: &str, subject: &str, on: bool) -> String {
    format!(
        "{}{rest} {}",
        lead_style.paint(lead, on),
        palette::BANNER_FILE.paint(subject, on)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handle that is a file, never a terminal.
    fn not_a_terminal() -> std::fs::File {
        tempfile::tempfile().unwrap()
    }

    #[test]
    fn a_style_is_one_escape_sequence_with_the_attributes_first() {
        assert_eq!(palette::BANNER_LABEL.escape(), "\x1b[34;106m");
        assert_eq!(palette::BANNER_FILE.escape(), "\x1b[31;103m");
        assert_eq!(palette::BANNER_MARK.escape(), "\x1b[91;103m");
        assert_eq!(palette::BANNER_OPTION.escape(), "\x1b[30;45m");
        assert_eq!(palette::STEP.escape(), "\x1b[1;35m");
        assert_eq!(palette::DIM.escape(), "\x1b[2m");
        let all = Style::PLAIN
            .bold()
            .dim()
            .underline()
            .fg(Color::LightWhite)
            .on(Color::Black);
        assert_eq!(all.escape(), "\x1b[1;2;4;97;40m");
    }

    #[test]
    fn the_plain_style_has_no_escape_and_paints_nothing() {
        assert!(Style::PLAIN.is_plain());
        assert_eq!(Style::PLAIN.escape(), "");
        assert_eq!(Style::PLAIN.paint("x", true), "x");
        assert_eq!(Style::default(), Style::PLAIN);
    }

    #[test]
    fn painting_off_gives_the_text_alone() {
        for style in [palette::BANNER_FILE, palette::ERROR, palette::DIM] {
            let plain = style.paint("text 42", false);
            assert_eq!(plain, "text 42");
            assert!(!plain.contains('\x1b'));
        }
    }

    #[test]
    fn painting_on_wraps_the_text_and_resets() {
        assert_eq!(palette::ERROR.paint("boom", true), "\x1b[1;31mboom\x1b[0m");
        assert_eq!(palette::PATH.paint(7, true), "\x1b[36m7\x1b[0m");
    }

    #[test]
    fn colour_needs_a_terminal_and_no_opt_out() {
        let xterm = Some(OsStr::new("xterm-256color"));
        assert!(color_allowed(true, None, xterm));
        assert!(color_allowed(true, None, None));
        assert!(!color_allowed(false, None, xterm), "not a terminal");
        assert!(
            !color_allowed(true, Some(OsStr::new("1")), xterm),
            "NO_COLOR is set"
        );
        assert!(
            color_allowed(true, Some(OsStr::new("")), xterm),
            "an empty NO_COLOR does not count"
        );
        assert!(
            !color_allowed(true, None, Some(OsStr::new("dumb"))),
            "TERM=dumb"
        );
    }

    #[test]
    fn a_stream_that_is_not_a_terminal_never_gets_colour() {
        // Whatever NO_COLOR and TERM are in this process.
        assert!(!use_color(&not_a_terminal()));
    }

    #[test]
    fn control_characters_cannot_reach_a_terminal() {
        let hostile = "title\x1b]0;owned\x07 and \x1b[2J\nnext\tline";
        let clean = sanitize(hostile);
        assert!(
            !clean.contains('\x1b') && !clean.contains('\x07'),
            "{clean:?}"
        );
        assert!(clean.contains("\nnext\tline"), "{clean:?}");
    }

    #[test]
    fn a_banner_colours_the_lead_and_the_subject_only() {
        let painted = banner(
            ">>>",
            palette::BANNER_MARK,
            " RECORDING SAVED as:",
            "recordings/x_rec.rs",
            true,
        );
        assert_eq!(
            painted,
            "\x1b[91;103m>>>\x1b[0m RECORDING SAVED as: \x1b[31;103mrecordings/x_rec.rs\x1b[0m"
        );
        let plain = banner(
            ">>>",
            palette::BANNER_MARK,
            " RECORDING SAVED as:",
            "recordings/x_rec.rs",
            false,
        );
        assert_eq!(plain, ">>> RECORDING SAVED as: recordings/x_rec.rs");
    }

    #[test]
    fn the_initialized_banner_is_a_label_then_the_file() {
        let painted = banner(
            "* RECORDING initialized:",
            palette::BANNER_LABEL,
            "",
            "new_test.rs",
            true,
        );
        assert_eq!(
            painted,
            "\x1b[34;106m* RECORDING initialized:\x1b[0m \x1b[31;103mnew_test.rs\x1b[0m"
        );
    }
}
