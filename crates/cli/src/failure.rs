//! The page `sbrs` prints when a command line is wrong.
//!
//! usage 6 answers a command line it cannot parse with the clap-shaped error and one line of
//! `Usage:`. Python's `sbase` prints more: the usage of the command the words reached, its
//! examples and its options, and then what was wrong with the call. [`render`] does the same:
//! the help page of the deepest command the words reached (about line, `Usage:`, arguments,
//! flags, `Examples:`), a blank line, and last the parser's error with its `tip:`, so the
//! error is the line above the prompt. At the root there is no 100-line page: [`root_page`]
//! lists the commands by group, names only.
//!
//! A request for help or the version is no failure and is left to `Cli::parse`, which prints
//! it on standard output and exits with 0. The completion request (`sbrs __complete_word__`)
//! never comes here: see [`COMPLETE_WORD`].
//!
//! A mistake that only shows after parsing (a start page a command refuses, a missing page) is an
//! [`ArgumentError`] returned from the command; [`render_argument_error`] prints it the same way.

use std::ffi::OsStr;
use std::fmt::Write as _;

use usage::help::Style;
use usage::spec::Spec;
use usage::{Command, Error, Parser};

use crate::Cli;

/// The first word of a completion request, which the completion scripts send to the binary.
/// `Cli::parse` answers it; it must reach no failure page.
pub(crate) const COMPLETE_WORD: &str = "__complete_word__";

/// The program name the pages print.
const PROGRAM: &str = "sbrs";

/// The heading of a command that has none of its own.
const OTHER_COMMANDS: &str = "Other commands";

/// The width of the compact root page when the terminal's is not known.
const DEFAULT_WIDTH: usize = 80;

/// The help headings of the commands, in the order the compact root page lists them. A heading
/// that is not here follows them in the order the commands declare it.
const HEADING_ORDER: [&str; 7] = [
    "Page commands",
    "Files and generators",
    "Record and replay",
    "Tests",
    "Drivers, grid and servers",
    "Security",
    "Info and setup",
];

/// The heading of the options that every command accepts, which a failure page leaves out.
const GLOBAL_FLAGS_HEADING: &str = "Global flags:";

/// An argument the parser accepted but the command refuses.
///
/// It is printed as a parse failure is: the page of the command, then `error: <message>`, with
/// status 2 and before any browser starts. Failures while a command runs are not this type and
/// keep their error report.
#[derive(Debug)]
pub(crate) struct ArgumentError {
    /// The command path without the program name, such as `["get-text"]`.
    command: Vec<String>,
    /// What is wrong, without the `error:` prefix.
    message: String,
}

impl ArgumentError {
    /// An error about `command`, a top-level command such as `get-text`.
    pub(crate) fn new(command: &str, message: impl Into<String>) -> Self {
        Self {
            command: vec![command.to_owned()],
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ArgumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ArgumentError {}

/// Whether `error` is a request usage answers itself (help, every command's help, the version),
/// not a failure.
pub(crate) fn is_request(error: &Error<'_, '_>) -> bool {
    matches!(
        error,
        Error::Help { .. } | Error::HelpAll { .. } | Error::Version { .. }
    )
}

/// The commands the words reached before the parser stopped, the root first.
pub(crate) fn route<'t>(root: &'t Command<'t>, argv: &[&OsStr]) -> Vec<&'t Command<'t>> {
    let mut parser = Parser::new(root, argv);
    while let Some(event) = parser.next_event() {
        if event.is_err() {
            break;
        }
    }
    parser
        .command_path()
        .into_iter()
        .map(|(command, _)| command)
        .collect()
}

/// The page of the deepest command in `route`, or the compact root page for the root.
fn page(route: &[&Command<'_>]) -> String {
    if route.len() > 1 {
        if let Some(page) =
            usage::help::render_at_styled(Cli::spec(), route, false, Style::auto_stderr())
        {
            return without_global_flags(&page);
        }
    }
    root_page(Cli::spec(), terminal_width())
}

/// `page` without its `Global flags:` section, which lists every option that comes before the
/// command and would push the command's own examples a hundred lines down. A line says where
/// to find them.
fn without_global_flags(page: &str) -> String {
    let mut text = String::with_capacity(page.len());
    let mut skipping = false;
    for line in page.split_inclusive('\n') {
        let plain = strip_ansi(line);
        // A section heading starts in the first column and ends with a colon.
        if !plain.starts_with(' ') && plain.trim_end().ends_with(':') {
            skipping = plain.trim_end() == GLOBAL_FLAGS_HEADING;
            if skipping {
                text.push_str(&format!(
                    "{GLOBAL_FLAGS_HEADING} written before or after the command; see '{PROGRAM} --help'\n\n"
                ));
                continue;
            }
        }
        if !skipping {
            text.push_str(line);
        }
    }
    text
}

/// The failure page of a command line the parser refused: the page of the command the words
/// reached, a blank line, then the error. `argv` is the command line without the program name.
pub(crate) fn render(argv: &[&OsStr], error: &Error<'static, '_>) -> String {
    let route = route(Cli::command(), argv);
    let mut text = page(&route);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    // A command that wants arguments and got none is answered with its page alone.
    if !matches!(error, Error::MissingArgsHelp { .. }) {
        text.push('\n');
        text.push_str(&error_text(argv, error));
    }
    text
}

/// The failure page of an [`ArgumentError`] found in `error`, or its message as a plain line
/// when `error` is another kind.
pub(crate) fn render_argument_error(error: &(dyn std::error::Error + 'static)) -> String {
    let Some(error) = error.downcast_ref::<ArgumentError>() else {
        return format!("error: {error}\n");
    };
    let mut route = vec![Cli::command()];
    for word in &error.command {
        let Some(next) = route.last().and_then(|command| {
            command.subcommands.iter().copied().find(|candidate| {
                candidate.name == word || candidate.aliases.contains(&word.as_str())
            })
        }) else {
            break;
        };
        route.push(next);
    }
    let mut text = page(&route);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let _ = write!(text, "\nerror: {}\n", error.message);
    text
}

/// What usage says of `error` (`error: ...` and any `tip:`), without the `Usage:` line and the
/// `For more information` line it appends, which the page above already covers.
fn error_text(argv: &[&OsStr], error: &Error<'static, '_>) -> String {
    without_usage_block(&Cli::render_failure(argv, error))
}

/// `text` up to the first line that starts with `Usage:`, with no blank line at its end.
fn without_usage_block(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if strip_ansi(line).trim_start().starts_with("Usage:") {
            break;
        }
        kept.push_str(line);
    }
    let kept = kept.trim_end();
    if kept.is_empty() {
        return text.to_owned();
    }
    format!("{kept}\n")
}

/// `text` without its terminal colour escapes (`ESC [ ... letter`).
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(letter) = chars.next() {
        if letter != '\u{1b}' {
            plain.push(letter);
        } else if chars.as_str().starts_with('[') {
            chars.next();
            for next in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&next) {
                    break;
                }
            }
        }
    }
    plain
}

/// The width of the terminal as `COLUMNS` says it, else 80.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|columns| columns.trim().parse::<usize>().ok())
        .map_or(DEFAULT_WIDTH, |columns| columns.clamp(40, 200))
}

/// The compact root page: the usage line and the command names under their help headings,
/// wrapped to `width`, with no logo and no descriptions.
pub(crate) fn root_page(spec: &Spec<'_>, width: usize) -> String {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for meta in spec.root.subcommands.iter().filter(|meta| !meta.hide) {
        let heading = meta.extra.help_heading.unwrap_or(OTHER_COMMANDS);
        match groups.iter_mut().find(|(known, _)| *known == heading) {
            Some((_, names)) => names.push(meta.cmd.name),
            None => groups.push((heading, vec![meta.cmd.name])),
        }
    }
    // The known headings first, in their order, then the others as declared, the commands
    // with no heading last. The sort is stable.
    groups.sort_by_key(|(heading, _)| {
        HEADING_ORDER
            .iter()
            .position(|known| known == heading)
            .unwrap_or(HEADING_ORDER.len() + usize::from(*heading == OTHER_COMMANDS))
    });
    let mut page = format!("Usage: {PROGRAM} [FLAGS] <COMMAND>\n");
    for (heading, mut names) in groups {
        names.sort_unstable();
        page.push_str(&wrapped(&format!("{heading}:"), &names, width));
    }
    let _ = write!(
        page,
        "\nRun '{PROGRAM} help <COMMAND>' for one command, '{PROGRAM} --help' for everything.\n"
    );
    page
}

/// `names` in lines of at most `width` columns, each line indented by two spaces.
fn wrapped(label: &str, names: &[&str], width: usize) -> String {
    const INDENT: &str = "  ";
    let mut text = String::new();
    // The first line starts with the label; the lines after it are indented.
    let mut line = label.to_owned();
    let mut first_on_line = true;
    for name in names {
        let separator = if first_on_line { " " } else { "  " };
        if !first_on_line && line.len() + separator.len() + name.len() > width {
            text.push_str(&line);
            text.push('\n');
            line = format!("{INDENT}{name}");
            continue;
        }
        line.push_str(separator);
        line.push_str(name);
        first_on_line = false;
    }
    text.push_str(&line);
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use usage::spec::CommandMeta;

    use super::*;

    /// The words of `line` as the parser is given them.
    fn words<'a>(line: &'a [&'a str]) -> Vec<&'a OsStr> {
        line.iter().map(OsStr::new).collect()
    }

    /// The names of the commands a command line reached.
    fn reached(line: &[&str]) -> Vec<&'static str> {
        route(Cli::command(), &words(line))
            .into_iter()
            .map(|command| command.name)
            .collect()
    }

    /// The failure `sbrs <line>` is refused with.
    fn failure(line: &[&str]) -> String {
        let argv = words(line);
        let error = Cli::parse_from(&argv).expect_err("the line is refused");
        assert!(!is_request(&error), "{line:?} is a request");
        render(&argv, &error)
    }

    #[test]
    fn the_route_of_a_command_with_an_unknown_flag_ends_at_the_command() {
        assert_eq!(
            reached(&["mkrec", "--nosuch", "--url=google.com"]),
            ["sbrs", "mkrec"]
        );
    }

    #[test]
    fn the_route_of_a_nested_command_names_both_levels() {
        assert_eq!(reached(&["plugin"]), ["sbrs", "plugin"]);
        assert_eq!(reached(&["plugin", "info"]), ["sbrs", "plugin", "info"]);
    }

    #[test]
    fn the_route_of_an_unknown_command_is_the_root() {
        assert_eq!(reached(&["nosuch"]), ["sbrs"]);
        assert_eq!(reached(&[]), ["sbrs"]);
    }

    #[test]
    fn the_page_comes_before_the_error() {
        let text = failure(&["mkfile"]);
        let usage = text.find("Usage: sbrs mkfile").expect("the usage line");
        let error = text.find("error: ").expect("the error line");
        assert!(usage < error, "{text}");
        assert!(text.contains("the following required arguments"), "{text}");
    }

    #[test]
    fn only_the_page_has_a_usage_line() {
        let text = failure(&["mkfile"]);
        assert_eq!(text.matches("Usage:").count(), 1, "{text}");
        assert!(!text.contains("For more information"), "{text}");
    }

    #[test]
    fn the_error_is_the_last_thing_printed() {
        let text = failure(&["mkfile"]);
        let error = text.find("error: ").expect("the error line");
        // Nothing of the page follows it: a page after the error would push it off the screen.
        assert!(!text[error..].contains("Usage:"), "{text}");
        assert!(!text[error..].contains("Flags:"), "{text}");
        assert!(text[error..].lines().count() <= 6, "{text}");
        assert!(text.ends_with('\n'), "{text:?}");
    }

    #[test]
    fn an_unknown_command_gets_the_root_page_and_the_error() {
        let text = failure(&["nosuch"]);
        assert!(text.starts_with("Usage: sbrs [FLAGS] <COMMAND>"), "{text}");
        assert!(
            text.contains("error: unrecognized subcommand 'nosuch'"),
            "{text}"
        );
        assert!(text.contains("sbrs help <COMMAND>"), "{text}");
    }

    #[test]
    fn a_command_line_with_no_words_is_the_root_page_alone() {
        let argv = words(&[]);
        let error = Cli::parse_from(&argv).expect_err("no command is given");
        let text = render(&argv, &error);
        assert!(text.starts_with("Usage: sbrs [FLAGS] <COMMAND>"), "{text}");
    }

    #[test]
    fn the_usage_block_is_cut_with_or_without_colour() {
        let plain =
            "error: bad\n\n  tip: x\n\nUsage: sbrs a\n\nFor more information, try '--help'.\n";
        assert_eq!(without_usage_block(plain), "error: bad\n\n  tip: x\n");
        let coloured = "\u{1b}[31merror:\u{1b}[0m bad\n\n\u{1b}[1mUsage:\u{1b}[0m sbrs a\n";
        assert_eq!(
            without_usage_block(coloured),
            "\u{1b}[31merror:\u{1b}[0m bad\n"
        );
    }

    #[test]
    fn a_text_with_no_usage_line_is_kept_whole() {
        assert_eq!(without_usage_block("error: bad\n"), "error: bad\n");
    }

    #[test]
    fn escapes_are_stripped_from_a_line() {
        assert_eq!(strip_ansi("\u{1b}[1;4mUsage:\u{1b}[0m x"), "Usage: x");
    }

    #[test]
    fn the_root_page_names_every_visible_command_within_the_width() {
        let page = root_page(Cli::spec(), 80);
        for meta in Cli::spec()
            .root
            .subcommands
            .iter()
            .filter(|meta| !meta.hide)
        {
            assert!(
                page.split_whitespace().any(|word| word == meta.cmd.name),
                "{} is missing from:\n{page}",
                meta.cmd.name
            );
        }
        assert!(page.lines().all(|line| line.len() <= 80), "{page}");
    }

    #[test]
    fn a_name_that_does_not_fit_starts_a_new_line() {
        assert_eq!(
            wrapped("A:", &["aaaa", "bbbb", "cccc"], 14),
            "A: aaaa  bbbb\n  cccc\n"
        );
    }

    #[test]
    fn the_root_page_lists_the_groups_in_their_order_with_the_names_after_the_heading() {
        let page = root_page(Cli::spec(), 80);
        let mut at = 0;
        for heading in HEADING_ORDER {
            let found = page
                .find(&format!("{heading}: "))
                .unwrap_or_else(|| panic!("no group {heading} in:\n{page}"));
            assert!(found >= at, "{heading} is out of order in:\n{page}");
            at = found;
        }
    }

    #[test]
    fn the_root_page_leaves_room_for_the_error_in_thirty_lines() {
        // The page, a blank line and the one-line error of an unknown command.
        let page = root_page(Cli::spec(), 80);
        assert!(page.lines().count() + 2 <= 30, "{page}");
    }

    #[test]
    fn the_global_flags_section_is_replaced_by_a_line() {
        let page =
            "Make a file.\n\nUsage: sbrs mkfile <FILE>\n\nFlags:\n  --url <URL>  A page.\n\n\
                        Global flags:\n  --headless  Run headless.\n\n  --uc  Evade.\n\n\
                        Examples:\n    $ sbrs mkfile a.rs\n";
        let text = without_global_flags(page);
        assert!(text.contains("Flags:\n  --url <URL>"), "{text}");
        assert!(!text.contains("--headless"), "{text}");
        assert!(
            text.contains("Global flags: written before or after the command"),
            "{text}"
        );
        assert!(
            text.ends_with("Examples:\n    $ sbrs mkfile a.rs\n"),
            "{text}"
        );
    }

    #[test]
    fn a_failure_page_has_the_examples_of_its_command_and_no_global_flags_list() {
        let text = failure(&["get-text"]);
        assert!(text.contains("Examples:"), "{text}");
        assert!(
            text.contains("$ sbrs --url https://example.com get-text --css p"),
            "{text}"
        );
        assert!(!text.contains("--proxy-pac-url"), "{text}");
        assert!(text.lines().count() < 25, "{text}");
    }

    #[test]
    fn an_argument_error_prints_the_page_of_its_command_then_the_message() {
        let error = ArgumentError::new("get-text", "get-text acts on a page");
        let text = render_argument_error(&error);
        let usage = text.find("Usage: sbrs get-text").expect("the usage line");
        let message = text
            .find("error: get-text acts on a page")
            .expect("the message");
        assert!(usage < message, "{text}");
        assert!(text.ends_with("error: get-text acts on a page\n"), "{text}");
    }

    #[test]
    fn an_argument_error_for_an_unknown_command_falls_back_to_the_root_page() {
        let error = ArgumentError::new("nosuch", "refused");
        let text = render_argument_error(&error);
        assert!(text.starts_with("Usage: sbrs [FLAGS] <COMMAND>"), "{text}");
    }

    #[test]
    fn the_completion_word_is_the_one_usage_answers() {
        // Pinned by `cli_failure_pages`, which runs the binary: a mismatch there means usage
        // renamed its hidden command.
        assert_eq!(COMPLETE_WORD, "__complete_word__");
    }

    /// Every command a person can see, with the commands that reach it, the root first.
    fn visible_commands() -> Vec<Vec<&'static Command<'static>>> {
        fn walk(
            meta: &'static CommandMeta<'static>,
            route: &mut Vec<&'static Command<'static>>,
            found: &mut Vec<Vec<&'static Command<'static>>>,
        ) {
            for sub in meta.subcommands.iter().copied().filter(|sub| !sub.hide) {
                route.push(sub.cmd);
                found.push(route.clone());
                walk(sub, route, found);
                route.pop();
            }
        }
        let mut found = Vec::new();
        walk(Cli::spec().root, &mut vec![Cli::command()], &mut found);
        found
    }

    /// The command line a route names, such as `plugin info`.
    fn name_of(route: &[&Command<'_>]) -> String {
        route
            .iter()
            .skip(1)
            .map(|command| command.name)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The examples on the help page of the command at the end of `route`: the `$ ...` lines
    /// under `Examples:`, without the `$ `.
    fn examples(route: &[&Command<'_>]) -> Vec<String> {
        let page =
            usage::help::render_at(Cli::spec(), route, false).expect("the command has a page");
        let mut in_examples = false;
        let mut found = Vec::new();
        for line in page.lines() {
            let plain = strip_ansi(line);
            if !plain.is_empty() && !plain.starts_with(' ') {
                in_examples = plain.trim_end() == "Examples:";
            } else if in_examples {
                if let Some(example) = plain.trim().strip_prefix("$ ") {
                    found.push(example.to_owned());
                }
            }
        }
        found
    }

    /// The words of `line` as a shell reads them: spaces separate, single and double quotes
    /// group.
    fn shell_words(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quote: Option<char> = None;
        let mut started = false;
        for letter in line.chars() {
            match quote {
                Some(open) if letter == open => quote = None,
                Some(_) => word.push(letter),
                None if letter == '\'' || letter == '"' => {
                    quote = Some(letter);
                    started = true;
                }
                None if letter.is_whitespace() => {
                    if started {
                        words.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                None => {
                    word.push(letter);
                    started = true;
                }
            }
        }
        if started {
            words.push(word);
        }
        words
    }

    #[test]
    fn shell_words_group_quoted_text() {
        assert_eq!(
            shell_words("sbrs get-text --css 'a b'  --text \"c d\" e"),
            ["sbrs", "get-text", "--css", "a b", "--text", "c d", "e"]
        );
    }

    #[test]
    fn every_visible_command_has_an_example() {
        let missing: Vec<String> = visible_commands()
            .iter()
            .filter(|route| examples(route).is_empty())
            .map(|route| name_of(route))
            .collect();
        assert!(missing.is_empty(), "no example for: {missing:?}");
    }

    #[test]
    fn every_example_is_accepted_by_the_parser() {
        let mut refused = Vec::new();
        for route in visible_commands() {
            for example in examples(&route) {
                let words = shell_words(&example);
                if words.first().map(String::as_str) != Some(PROGRAM) {
                    refused.push(format!("{example}: does not start with {PROGRAM}"));
                    continue;
                }
                let argv: Vec<&OsStr> = words[1..].iter().map(OsStr::new).collect();
                if let Err(error) = crate::strict::check(Cli::spec(), Cli::command(), &argv) {
                    refused.push(format!(
                        "{example}: {}",
                        strip_ansi(&Cli::render_failure(&argv, &error))
                    ));
                } else if let Err(error) = Cli::parse_from(&argv) {
                    refused.push(format!(
                        "{example}: {}",
                        strip_ansi(&Cli::render_failure(&argv, &error))
                    ));
                }
            }
        }
        assert!(
            refused.is_empty(),
            "examples the parser refuses:\n{}",
            refused.join("\n")
        );
    }

    /// example.com lost its `<h1>` in 2026, so an example that selects one fails as soon as
    /// somebody pastes it from `--help`: the paragraph is the element both pages have.
    #[test]
    fn no_example_selects_the_h1_example_com_no_longer_serves() {
        let stale: Vec<String> = visible_commands()
            .iter()
            .flat_map(|route| examples(route))
            .filter(|example| example.contains("example.com"))
            .filter(|example| {
                shell_words(example)
                    .iter()
                    .any(|word| word == "h1" || word.contains("=h1"))
            })
            .collect();
        assert!(stale.is_empty(), "examples that select an h1: {stale:?}");
    }
}
