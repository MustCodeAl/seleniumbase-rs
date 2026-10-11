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
//! A command that fails prints a report made by [`format_error_report`]:
//!
//! ```text
//! error: element not found: #buy (strategy: css selector)
//! hint: Nothing on the page matched the css selector `#buy` in time. ...
//! ```
//!
//! and exits with [`exit_code_for`] the error, which follows its category.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::rich_helper::{format_message, Level};
//!
//! assert_eq!(format_message(Level::Info, "ready", false), "[INFO] ready");
//! assert!(format_message(Level::Error, "failed", true).starts_with("\x1b[1;31m[ERROR]"));
//! ```

use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io::IsTerminal;

use crate::error::SeleniumBaseError;

const RESET: &str = "\x1b[0m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const BOLD_RED: &str = "\x1b[1;31m";
const BOLD_CYAN: &str = "\x1b[1;36m";

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

/// Formats a failed command's error for a person reading a terminal or a log.
///
/// The first line is `error: ` and what failed. Each underlying cause follows
/// on its own `caused by: ` line, leaving out a cause whose text the lines
/// above already contain. Last comes `hint: ` with the likely cause and the
/// fix, taken from the first of this crate's own errors in the chain. With
/// `color` the `error` and `hint` labels are coloured, and the text is
/// [sanitized](sanitize) either way.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::rich_helper::format_error_report;
/// use seleniumbase_rs::SeleniumBaseError;
///
/// let report = format_error_report(&SeleniumBaseError::element_not_found("#buy"), false);
/// assert!(report.starts_with("error: element not found: #buy (strategy: css selector)\n"));
/// assert!(report.contains("\nhint: "));
/// ```
#[must_use]
pub fn format_error_report(error: &(dyn Error + 'static), color: bool) -> String {
    let label = |name: &str, style: &str| {
        if color && !style.is_empty() {
            format!("{style}{name}:{RESET} ")
        } else {
            format!("{name}: ")
        }
    };
    let summary = error.to_string();
    let mut report = format!("{}{}\n", label("error", BOLD_RED), sanitize(&summary));
    let mut said = vec![summary];
    let mut cause = error.source();
    while let Some(inner) = cause {
        let text = inner.to_string();
        if !text.is_empty() && !said.iter().any(|line| line.contains(&text)) {
            let _ = writeln!(report, "  {}{}", label("caused by", ""), sanitize(&text));
            said.push(text);
        }
        cause = inner.source();
    }
    if let Some(hint) = own_error(error).and_then(SeleniumBaseError::hint) {
        let _ = writeln!(report, "{}{}", label("hint", BOLD_CYAN), sanitize(&hint));
    }
    report
}

/// The exit status for a command that failed with `error`.
///
/// The first of this crate's errors in the chain decides, through
/// [`SeleniumBaseError::exit_code`]; otherwise an I/O error uses the same code
/// as [`SeleniumBaseError::Io`], and anything else is 1.
#[must_use]
pub fn exit_code_for(error: &(dyn Error + 'static)) -> u8 {
    if let Some(error) = own_error(error) {
        error.exit_code()
    } else if error.is::<std::io::Error>() {
        SeleniumBaseError::Io(std::io::Error::other("")).exit_code()
    } else {
        1
    }
}

/// The first [`SeleniumBaseError`] in `error`'s chain of causes, itself included.
fn own_error<'a>(error: &'a (dyn Error + 'static)) -> Option<&'a SeleniumBaseError> {
    let mut next = Some(error);
    while let Some(current) = next {
        if let Some(own) = current.downcast_ref::<SeleniumBaseError>() {
            return Some(own);
        }
        next = current.source();
    }
    None
}

/// Prints [`format_error_report`] of `error` to standard error.
pub fn print_error_report(error: &(dyn Error + 'static)) {
    eprint!(
        "{}",
        format_error_report(error, use_color(Stream::Stderr))
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
    fn an_error_report_has_the_summary_the_causes_and_the_hint() {
        let error = SeleniumBaseError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "settings.toml is missing",
        ));

        let report = format_error_report(&error, false);

        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines[0], "error: I/O error: settings.toml is missing");
        assert!(
            !report.contains("caused by"),
            "a cause the summary already says is not repeated:\n{report}"
        );
        assert!(lines[1].starts_with("hint: A file or directory"), "{report}");
        assert_eq!(lines.len(), 2);
    }

    #[derive(Debug)]
    struct Wrapped(SeleniumBaseError);

    impl std::fmt::Display for Wrapped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("could not open the page")
        }
    }

    impl Error for Wrapped {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn a_cause_that_adds_something_gets_its_own_line() {
        let error = Wrapped(SeleniumBaseError::navigation("https://a.test", "dns"));

        let report = format_error_report(&error, false);

        assert!(
            report.contains("\n  caused by: navigation to 'https://a.test' failed: dns\n"),
            "{report}"
        );
        assert!(
            report.contains("\nhint: Check that 'https://a.test' is spelled right"),
            "the hint comes from the crate's error further down:\n{report}"
        );
        assert_eq!(exit_code_for(&error), 5);
    }

    #[test]
    fn a_coloured_report_colours_the_labels_and_defuses_the_text() {
        let error = SeleniumBaseError::assertion_failed("title was \x1b[2Jowned");

        let report = format_error_report(&error, true);

        assert!(report.starts_with("\x1b[1;31merror:\x1b[0m assertion failed: title was \u{fffd}[2Jowned"));
        assert!(report.contains("\x1b[1;36mhint:\x1b[0m "), "{report:?}");
        assert_eq!(report.matches('\x1b').count(), 4, "{report:?}");
    }

    #[test]
    fn the_exit_code_follows_the_error_category() {
        assert_eq!(
            exit_code_for(&SeleniumBaseError::element_not_found("#x")),
            3
        );
        assert_eq!(
            exit_code_for(&std::io::Error::from(std::io::ErrorKind::NotFound)),
            6
        );
        let other: Box<dyn Error> = "plain text".into();
        assert_eq!(exit_code_for(other.as_ref()), 1);
    }

    #[test]
    fn levels_have_distinct_tags() {
        let tags = [Level::Info, Level::Warning, Level::Error].map(Level::tag);
        assert_eq!(tags, ["[INFO]", "[WARN]", "[ERROR]"]);
    }
}
