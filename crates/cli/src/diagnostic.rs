//! The report `sbrs` and `sbrs-mcp` print when a command fails, drawn by [`miette`].
//!
//! [`Failure::of`] reads any error and keeps what a person needs: the message, a code that names
//! the kind of failure (`sbrs::element_not_found`), the hint of the first `SeleniumBaseError` in
//! the chain as the help line, and the causes the message does not already say. [`render`] draws
//! it; [`print`] writes it to standard error.
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
use seleniumbase_rs::cli::scripts::rich_helper::{sanitize, use_color, Stream};
use seleniumbase_rs::SeleniumBaseError;

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

/// Prints the report of `error` to standard error: coloured and wrapped on a terminal, plain
/// and unwrapped anywhere else.
pub(crate) fn print(program: &str, error: &(dyn Error + 'static)) {
    let wrap_at = std::io::stderr().is_terminal().then(terminal_width);
    eprint!(
        "{}",
        render(
            &Failure::of(program, error),
            use_color(Stream::Stderr),
            wrap_at
        )
    );
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
}
