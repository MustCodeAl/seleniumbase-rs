//! The report `sbrs` and `sbrs-mcp` print when a command fails, drawn by [`miette`].
//!
//! [`Failure::of`] reads any error and keeps what a person needs: the message, a code that names
//! the kind of failure (`sbrs::element_not_found`), the hint of the first `SeleniumBaseError` in
//! the chain as the help line, and the causes the message does not already say. [`render`] draws
//! it; [`print()`] writes it to standard error.
//!
//! Colour follows the rules of the rest of the command line (`rich_helper::use_color`: a
//! terminal, no `NO_COLOR`, `TERM` not `dumb`). With none, and always when standard error is a
//! pipe or a file, the report is plain ASCII and its lines are not wrapped, so a log or a test
//! can search it. The exit status is not decided here: it stays
//! `rich_helper::exit_code_for`.
//!
//! A command line the parser refuses keeps the page of the command and the `error:` line under
//! it (see `failure.rs`); this is the report of a command that ran and failed.

use std::error::Error;
use std::fmt;
use std::io::IsTerminal;

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};
use seleniumbase_rs::SeleniumBaseError;
use seleniumbase_rs::cli::scripts::rich_helper::{Stream, sanitize, use_color};

/// The width a report wraps at on a terminal that does not say how wide it is.
const DEFAULT_WIDTH: usize = 80;

/// The code of an error that is neither a `SeleniumBaseError` nor an I/O error.
const GENERIC_CODE: &str = "error";

/// An error as [`miette`] reads it: a message, a code, a help line, and the causes below it.
///
/// Only the first link of the chain has a code and a help line; the links under it are the
/// causes the message above does not already contain.
#[derive(Debug)]
pub(crate) struct Failure {
    message: String,
    code: Option<String>,
    help: Option<String>,
    cause: Option<Box<Failure>>,
}

impl Failure {
    /// The failure `error` is, for the program `program` (`sbrs` or `sbrs_mcp`, the start of
    /// the code).
    pub(crate) fn of(program: &str, error: &(dyn Error + 'static)) -> Self {
        let summary = error.to_string();
        let mut said = vec![summary.clone()];
        let mut causes = Vec::new();
        let mut next = error.source();
        while let Some(inner) = next {
            let text = inner.to_string();
            if !text.is_empty() && !said.iter().any(|line| line.contains(&text)) {
                said.push(text.clone());
                causes.push(text);
            }
            next = inner.source();
        }
        let cause = causes.into_iter().rev().fold(None, |below, text| {
            Some(Box::new(Self {
                message: sanitize(&text),
                code: None,
                help: None,
                cause: below,
            }))
        });
        let own = own_error(error);
        Self {
            message: sanitize(&summary),
            code: Some(format!("{program}::{}", kind(error, own))),
            help: own
                .and_then(SeleniumBaseError::hint)
                .map(|hint| sanitize(&hint)),
            cause,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

impl Diagnostic for Failure {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.code
            .as_ref()
            .map(|code| Box::new(code) as Box<dyn fmt::Display + 'a>)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn fmt::Display + 'a>)
    }

    // `diagnostic_source` is left alone on purpose: a cause that is a `Diagnostic` is drawn as a
    // report of its own, with a cross and a blank line, where a plain error is one line.
}

/// The first `SeleniumBaseError` in `error`'s chain of causes, `error` itself included.
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

/// The word after `program::` in the code: the category of the library's error, `io` for an
/// I/O error, `error` for anything else.
fn kind(error: &(dyn Error + 'static), own: Option<&SeleniumBaseError>) -> &'static str {
    match own {
        Some(own) => own.category(),
        None if error.is::<std::io::Error>() => "io",
        None => GENERIC_CODE,
    }
}

/// The width of the terminal as `COLUMNS` says it, else 80.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|columns| columns.trim().parse::<usize>().ok())
        .map_or(DEFAULT_WIDTH, |columns| columns.clamp(40, 200))
}

/// `failure` drawn as a report: with colour and box-drawing characters when `color`, else in
/// plain ASCII. It wraps at `wrap_at` columns when that is given and keeps every line whole
/// when it is not.
pub(crate) fn render(failure: &Failure, color: bool, wrap_at: Option<usize>) -> String {
    let theme = if color {
        GraphicalTheme::unicode()
    } else {
        GraphicalTheme::none()
    };
    let handler = GraphicalReportHandler::new_themed(theme).with_links(false);
    let handler = match wrap_at {
        Some(width) => handler.with_width(width),
        None => handler.with_wrap_lines(false),
    };
    let mut text = String::new();
    // Writing to a `String` cannot fail.
    let _ = handler.render_report(&mut text, failure);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Formats a raw stack backtrace with syntax highlighting and coloring.
pub(crate) fn format_backtrace(trace: &str, color: bool) -> String {
    if !color {
        return trace.to_string();
    }

    let mut out = String::with_capacity(trace.len() + 512);
    for line in trace.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("stack backtrace:") {
            out.push_str("\x1b[1;33mstack backtrace:\x1b[0m\n");
        } else if let Some(rest) = trimmed.strip_prefix("at ") {
            let indent_len = line.len() - trimmed.len();
            out.push_str(&line[..indent_len]);
            out.push_str("\x1b[2mat\x1b[0m ");
            format_location(rest, &mut out);
            out.push('\n');
        } else if let Some((frame_num, rest)) = parse_frame_line(trimmed) {
            let indent_len = line.len() - trimmed.len();
            out.push_str(&line[..indent_len]);
            out.push_str("\x1b[36m");
            out.push_str(frame_num);
            out.push_str(":\x1b[0m");

            let symbol_part = rest.trim_start();
            if let Some((addr, sym)) = parse_address_and_symbol(symbol_part) {
                out.push_str(" \x1b[2m");
                out.push_str(addr);
                out.push_str("\x1b[0m ");
                format_symbol(sym, &mut out);
            } else {
                out.push(' ');
                format_symbol(symbol_part, &mut out);
            }
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !trace.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn parse_frame_line(s: &str) -> Option<(&str, &str)> {
    let colon = s.find(':')?;
    let prefix = &s[..colon];
    if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
        Some((prefix, &s[colon + 1..]))
    } else {
        None
    }
}

fn parse_address_and_symbol(s: &str) -> Option<(&str, &str)> {
    if s.starts_with("0x") || s.starts_with("0X") {
        let pos = s.find(" - ")?;
        return Some((&s[..pos + 2], &s[pos + 3..]));
    }
    None
}

fn format_symbol(sym: &str, out: &mut String) {
    if sym.contains("seleniumbase_rs") || sym.contains("sbrs") {
        out.push_str("\x1b[1;32m");
        out.push_str(sym);
        out.push_str("\x1b[0m");
    } else if sym.contains("tokio")
        || sym.contains("core::")
        || sym.contains("std::")
        || sym.contains("alloc::")
    {
        out.push_str("\x1b[2;34m");
        out.push_str(sym);
        out.push_str("\x1b[0m");
    } else if sym.contains("<unknown>") || sym.contains("???") {
        out.push_str("\x1b[2;37m");
        out.push_str(sym);
        out.push_str("\x1b[0m");
    } else {
        out.push_str("\x1b[37m");
        out.push_str(sym);
        out.push_str("\x1b[0m");
    }
}

fn format_location(loc: &str, out: &mut String) {
    let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
    if parts.len() == 3
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
    {
        let file = parts[2];
        let line = parts[1];
        let col = parts[0];
        out.push_str("\x1b[4;36m");
        out.push_str(file);
        out.push_str("\x1b[0m");
        out.push_str("\x1b[1;33m:");
        out.push_str(line);
        out.push(':');
        out.push_str(col);
        out.push_str("\x1b[0m");
    } else if parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
        let file = parts[1];
        let line = parts[0];
        out.push_str("\x1b[4;36m");
        out.push_str(file);
        out.push_str("\x1b[0m");
        out.push_str("\x1b[1;33m:");
        out.push_str(line);
        out.push_str("\x1b[0m");
    } else {
        out.push_str("\x1b[4;36m");
        out.push_str(loc);
        out.push_str("\x1b[0m");
    }
}

/// Formats a panic hook info with syntax highlighting and coloring.
pub(crate) fn format_panic_info(info: &std::panic::PanicHookInfo<'_>, color: bool) -> String {
    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        *s
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.as_str()
    } else {
        "Box<dyn Any>"
    };

    let location = info
        .location()
        .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));

    if color {
        let mut msg = format!(
            "\x1b[1;31merror\x1b[0m\x1b[1m: the application panicked at \x1b[0m\x1b[1;37m'{payload}'\x1b[0m"
        );
        if let Some(loc) = location {
            msg.push_str("\n  \x1b[2mat\x1b[0m ");
            format_location(&loc, &mut msg);
        }
        msg
    } else {
        let mut msg = format!("error: the application panicked at '{payload}'");
        if let Some(loc) = location {
            msg.push_str(&format!("\n  at {loc}"));
        }
        msg
    }
}

/// Installs a panic hook that formats the panic location and stacktrace with syntax highlighting.
pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let color = use_color(Stream::Stderr);
        eprintln!("{}", format_panic_info(info, color));
        if std::env::var("RUST_BACKTRACE")
            .map(|v| v != "0")
            .unwrap_or(false)
        {
            let backtrace = std::backtrace::Backtrace::capture();
            if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
                eprintln!("\n{}", format_backtrace(&format!("{backtrace}"), color));
            }
        } else if color {
            eprintln!(
                "\x1b[2mnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\x1b[0m"
            );
        } else {
            eprintln!(
                "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
            );
        }
    }));
}

/// Prints the report of `error` to standard error: coloured and wrapped on a terminal, plain
/// and unwrapped anywhere else.
pub(crate) fn print(program: &str, error: &(dyn Error + 'static)) {
    let color = use_color(Stream::Stderr);
    let wrap_at = std::io::stderr().is_terminal().then(terminal_width);
    eprint!("{}", render(&Failure::of(program, error), color, wrap_at));
    if std::env::var("RUST_BACKTRACE")
        .map(|v| v != "0")
        .unwrap_or(false)
    {
        let backtrace = std::backtrace::Backtrace::capture();
        if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            eprintln!("\n{}", format_backtrace(&format!("{backtrace}"), color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An error with a cause below it, as `anyhow`-style wrappers make.
    #[derive(Debug)]
    struct Wrapped(SeleniumBaseError);

    impl fmt::Display for Wrapped {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("could not open the page")
        }
    }

    impl Error for Wrapped {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    fn plain(error: &(dyn Error + 'static)) -> String {
        render(&Failure::of("sbrs", error), false, None)
    }

    #[test]
    fn a_library_error_shows_its_category_as_the_code_and_its_hint_as_help() {
        let text = plain(&SeleniumBaseError::element_not_found("#buy"));

        assert!(text.contains("sbrs::element_not_found"), "{text}");
        assert!(
            text.contains("element not found: #buy (strategy: css selector)"),
            "{text}"
        );
        assert!(
            text.contains("help: Nothing on the page matched the css selector `#buy`"),
            "{text}"
        );
    }

    #[test]
    fn the_report_of_a_pipe_is_plain_ascii_on_whole_lines() {
        let hint_error = SeleniumBaseError::element_not_found("#buy");
        let text = plain(&hint_error);

        assert!(text.is_ascii(), "{text}");
        assert!(!text.contains('\x1b'), "{text}");
        assert!(text.ends_with('\n'), "{text:?}");
        let help = text
            .lines()
            .find(|line| line.contains("help:"))
            .expect("a help line");
        assert!(
            help.len() > 120,
            "a long hint stays on one line when nothing wraps it: {help}"
        );
    }

    #[test]
    fn a_terminal_gets_colour_and_a_wrapped_report() {
        let failure = Failure::of("sbrs", &SeleniumBaseError::element_not_found("#buy"));

        let coloured = render(&failure, true, Some(60));

        assert!(coloured.contains('\x1b'), "{coloured:?}");
        assert!(
            coloured.contains('\u{2715}') || coloured.contains('\u{d7}'),
            "{coloured:?}"
        );
        let uncoloured = render(&failure, false, Some(60));
        assert!(!uncoloured.contains('\x1b'), "{uncoloured:?}");
        assert!(
            uncoloured.lines().all(|line| line.len() <= 60),
            "wrapped at 60 columns:\n{uncoloured}"
        );
    }

    #[test]
    fn a_cause_the_message_does_not_say_gets_its_own_line_under_it() {
        let error = Wrapped(SeleniumBaseError::navigation("https://a.test", "dns"));

        let text = plain(&error);

        assert!(text.contains("could not open the page"), "{text}");
        assert!(
            text.contains("navigation to 'https://a.test' failed: dns"),
            "{text}"
        );
        assert!(
            text.contains("sbrs::navigation"),
            "the code is the kind of the library's error further down the chain:\n{text}"
        );
        assert!(
            text.contains("help: Check that 'https://a.test' is spelled right"),
            "the hint comes from the library's error further down the chain:\n{text}"
        );
    }

    #[test]
    fn the_plain_report_is_the_code_the_message_the_cause_and_the_help() {
        let inner = SeleniumBaseError::navigation("https://a.test", "dns");
        let help = inner.hint().expect("a navigation error has a hint");
        let error = Wrapped(inner);

        let text = plain(&error);

        assert_eq!(
            text.trim_end(),
            format!(
                "sbrs::navigation\n\n  x could not open the page\n  `-> navigation to \
                 'https://a.test' failed: dns\n  help: {help}"
            )
        );
    }

    #[test]

    fn a_cause_the_message_already_contains_is_not_repeated() {
        let error = SeleniumBaseError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "settings.toml is missing",
        ));

        let text = plain(&error);

        assert_eq!(
            text.matches("settings.toml is missing").count(),
            1,
            "{text}"
        );
        assert!(text.contains("sbrs::io"), "{text}");
    }

    #[test]
    fn an_io_error_and_a_plain_error_have_codes_of_their_own() {
        let io = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(plain(&io).contains("sbrs::io"));

        let other: Box<dyn Error> = "plain text".into();
        let text = plain(other.as_ref());
        assert!(text.contains("sbrs::error"), "{text}");
        assert!(text.contains("plain text"), "{text}");
        assert!(
            !text.contains("help:"),
            "a plain message has no help: {text}"
        );
    }

    #[test]
    fn the_code_starts_with_the_program() {
        let failure = Failure::of("sbrs_mcp", &SeleniumBaseError::invalid_config("bad"));
        let text = render(&failure, false, None);

        assert!(text.contains("sbrs_mcp::invalid_config"), "{text}");
    }

    #[test]
    fn an_escape_sequence_in_the_message_cannot_reach_the_terminal() {
        let error = SeleniumBaseError::assertion_failed("title was \x1b[2Jowned");

        let text = plain(&error);

        assert!(!text.contains('\x1b'), "{text:?}");
        assert!(text.contains("title was \u{fffd}[2Jowned"), "{text:?}");
    }

    #[test]
    fn backtrace_formatting_respects_color_flag() {
        let sample = "stack backtrace:\n   0: 0x102345678 - seleniumbase_rs::browser::session::connect\n             at src/browser/session/connect.rs:42:15\n   1: 0x102345890 - std::sys::backtrace\n             at library/std/src/sys/backtrace.rs:20:5";
        let uncolored = format_backtrace(sample, false);
        assert_eq!(uncolored, sample);

        let colored = format_backtrace(sample, true);
        assert!(colored.contains('\x1b'), "{colored}");
        // Check frame 0 styled
        assert!(colored.contains("\x1b[36m0:\x1b[0m"), "{colored}");
        // Check crate code highlighted in green
        assert!(
            colored.contains("\x1b[1;32mseleniumbase_rs::browser::session::connect\x1b[0m"),
            "{colored}"
        );
        // Check std code muted in blue
        assert!(
            colored.contains("\x1b[2;34mstd::sys::backtrace\x1b[0m"),
            "{colored}"
        );
        // Check location line highlighted
        assert!(
            colored
                .contains("\x1b[4;36msrc/browser/session/connect.rs\x1b[0m\x1b[1;33m:42:15\x1b[0m"),
            "{colored}"
        );
    }
}
