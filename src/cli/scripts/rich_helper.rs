//! Styled terminal messages for `sbase`.
//!
//! The Python SeleniumBase console scripts print through the `rich` library.
//! This module is the Rust counterpart: a handful of message kinds with a
//! fixed tag and colour, written so that they are safe to pipe and to log.
//!
//! - Colour is used only when the destination is a terminal, `NO_COLOR` is
//!   unset or empty (<https://no-color.org>) and `TERM` is not `dumb`.
//! - Control characters in the message, such as an escape sequence copied from
//!   a web page into an error text, are replaced, so a message can never
//!   reprogram the terminal.
//! - Informational output goes to standard output; warnings and errors go to
//!   standard error.
//!
//! The `format_*` functions are pure and are what the tests exercise; the
//! `print_*` functions add the stream and the colour decision.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::rich_helper::{format_message, Level};
//!
//! assert_eq!(format_message(Level::Info, "ready", false), "[INFO] ready");
//! assert!(format_message(Level::Error, "failed", true).starts_with("\x1b[1;31m[ERROR]"));
//! ```

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io::IsTerminal;

const RESET: &str = "\x1b[0m";
const BOLD_GREEN: &str = "\x1b[1;32m";

/// What a message says about the run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    /// Progress or a result the user asked for.
    Info,
    /// Something is wrong but the command carries on.
    Warning,
    /// The command could not do what was asked.
    Error,
}

impl Level {
    /// The bracketed tag that starts the message.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Info => "[INFO]",
            Self::Warning => "[WARN]",
            Self::Error => "[ERROR]",
        }
    }

    const fn style(self) -> &'static str {
        match self {
            Self::Info => "\x1b[1;34m",
            Self::Warning => "\x1b[1;33m",
            Self::Error => "\x1b[1;31m",
        }
    }
}

/// The standard stream a message is printed to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

impl Stream {
    fn is_terminal(self) -> bool {
        match self {
            Self::Stdout => std::io::stdout().is_terminal(),
            Self::Stderr => std::io::stderr().is_terminal(),
        }
    }
}

/// Decides whether colour may be used, from the facts that matter.
///
/// `no_color` and `term` are the values of the `NO_COLOR` and `TERM`
/// environment variables, if set. An empty `NO_COLOR` does not count.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::rich_helper::color_allowed;
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

/// Whether messages to `stream` should be coloured right now.
#[must_use]
pub fn use_color(stream: Stream) -> bool {
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

/// Formats `text` as a message of the given kind, with its tag in front.
///
/// With `color` the tag is bold and coloured; the text itself never is.
#[must_use]
pub fn format_message(level: Level, text: &str, color: bool) -> String {
    let mut line = String::with_capacity(text.len() + 24);
    if color {
        let _ = write!(line, "{}{}{RESET}", level.style(), level.tag());
    } else {
        line.push_str(level.tag());
    }
    line.push(' ');
    line.push_str(&sanitize(text));
    line
}

/// Formats `text` as a highlighted heading: bold green when `color` is set.
#[must_use]
pub fn format_highlight(text: &str, color: bool) -> String {
    let text = sanitize(text);
    if color {
        format!("{BOLD_GREEN}{text}{RESET}")
    } else {
        text
    }
}

/// Prints a highlighted line to standard output.
pub fn print_rich_text(text: &str) {
    println!("{}", format_highlight(text, use_color(Stream::Stdout)));
}

/// Prints an informational message to standard output.
pub fn print_info(text: &str) {
    println!(
        "{}",
        format_message(Level::Info, text, use_color(Stream::Stdout))
    );
}

/// Prints an informational message to standard error.
///
/// Use this, not [`print_info`], for progress a command reports while its
/// standard output carries data that someone may be piping elsewhere.
pub fn print_notice(text: &str) {
    eprintln!(
        "{}",
        format_message(Level::Info, text, use_color(Stream::Stderr))
    );
}

/// Prints a warning to standard error.
pub fn print_warning(text: &str) {
    eprintln!(
        "{}",
        format_message(Level::Warning, text, use_color(Stream::Stderr))
    );
}

/// Prints an error to standard error.
pub fn print_error(text: &str) {
    eprintln!(
        "{}",
        format_message(Level::Error, text, use_color(Stream::Stderr))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_message_is_the_tag_and_the_text() {
        assert_eq!(format_message(Level::Info, "ready", false), "[INFO] ready");
        assert_eq!(format_message(Level::Warning, "slow", false), "[WARN] slow");
        assert_eq!(
            format_message(Level::Error, "failed", false),
            "[ERROR] failed"
        );
    }

    #[test]
    fn a_coloured_message_colours_only_the_tag() {
        assert_eq!(
            format_message(Level::Error, "failed", true),
            "\x1b[1;31m[ERROR]\x1b[0m failed"
        );
        assert_eq!(
            format_message(Level::Info, "ready", true),
            "\x1b[1;34m[INFO]\x1b[0m ready"
        );
        assert_eq!(
            format_message(Level::Warning, "slow", true),
            "\x1b[1;33m[WARN]\x1b[0m slow"
        );
    }

    #[test]
    fn highlighting_wraps_the_whole_text() {
        assert_eq!(format_highlight("Done", false), "Done");
        assert_eq!(format_highlight("Done", true), "\x1b[1;32mDone\x1b[0m");
    }

    #[test]
    fn escape_sequences_in_the_text_are_defused() {
        let hostile = "title\x1b]0;owned\x07 and \x1b[2J";
        let shown = format_message(Level::Error, hostile, false);
        assert!(!shown.contains('\x1b'), "{shown:?}");
        assert!(!shown.contains('\x07'), "{shown:?}");
        assert!(shown.contains("title"), "{shown:?}");
        assert_eq!(
            format_highlight(hostile, true).matches('\x1b').count(),
            2,
            "only the opening and closing style codes remain"
        );
    }

    #[test]
    fn newlines_and_tabs_survive() {
        assert_eq!(sanitize("a\tb\nc"), "a\tb\nc");
        assert_eq!(
            sanitize("a\rb"),
            "a\u{fffd}b",
            "a bare CR can overwrite a line"
        );
    }

    #[test]
    fn colour_needs_a_terminal() {
        assert!(color_allowed(true, None, None));
        assert!(!color_allowed(false, None, None));
    }

    #[test]
    fn no_color_turns_colour_off_unless_it_is_empty() {
        assert!(!color_allowed(true, Some(OsStr::new("1")), None));
        assert!(!color_allowed(true, Some(OsStr::new("anything")), None));
        assert!(color_allowed(true, Some(OsStr::new("")), None));
    }

    #[test]
    fn a_dumb_terminal_gets_no_colour() {
        assert!(!color_allowed(true, None, Some(OsStr::new("dumb"))));
        assert!(color_allowed(true, None, Some(OsStr::new("xterm"))));
    }

    #[test]
    fn levels_have_distinct_tags() {
        let tags = [Level::Info, Level::Warning, Level::Error].map(Level::tag);
        assert_eq!(tags, ["[INFO]", "[WARN]", "[ERROR]"]);
    }
}
