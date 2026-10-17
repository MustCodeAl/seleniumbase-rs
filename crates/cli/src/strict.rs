//! What usage 6 lets through that clap did not, refused before the real parse.
//!
//! usage 6.12 is looser than clap in three places that no attribute of the derives closes:
//!
//! - A scalar flag given twice. `args_override_self = false` is a setting of one command:
//!   the root and an `Args` struct can say it, an inline variant of the command enum
//!   cannot, so after a subcommand the last value won.
//! - A value written after a switch. The parser drops it (`--headless=false` would switch
//!   the option on). A switch declared `bool_value` hands the value over instead, so the
//!   parser can see it; the derive then accepts `true` and `false`, and clap accepted
//!   neither.
//! - `help` followed by a path that names no command. The parser stops at the last word
//!   that names one and prints that page, so `help nosuch` printed the main help.
//!
//! [`check`] reads the events the generated parser reads, from the same tables, and
//! refuses these three with the errors usage renders for everything else. Every other mistake,
//! and every help or version request, is left to the real parse.

use std::ffi::OsStr;

use usage::spec::{CommandMeta, FlagMeta, Spec};
use usage::{Command, Error, Event, Flag, InvalidValue, Parser};

/// Refuses a command line that clap would have refused for one of the three reasons above.
///
/// `argv` is the command line without the program name. `spec` and `root` are those of the
/// same CLI (`Cli::spec()` and `Cli::command()`). The first mistake comes back as the error
/// the parser itself would have returned for it, to be rendered the same way.
///
/// # Errors
///
/// [`Error::DuplicateFlag`] for a scalar flag given twice to a command whose declaration
/// lets the last one win while the root's does not, [`Error::InvalidValue`] for a value
/// after a switch, and [`Error::UnexpectedArg`] for the word after a `help` path that names
/// no command.
pub fn check<'v>(
    spec: &'static Spec<'static>,
    root: &'static Command<'static>,
    argv: &[&'v OsStr],
) -> Result<(), Error<'static, 'v>> {
    // The rule is the root's: a command that says nothing about it is held to it too.
    let strict = !spec.root.args_override_self;
    let mut parser = Parser::new(root, argv);
    // The metadata of the command the parser is in, and the flags it has been given.
    let mut meta = spec.root;
    let mut given: Vec<u64> = Vec::new();
    while let Some(event) = parser.next_event() {
        match event {
            Ok(Event::Command(command)) => {
                let Some(next) = subcommand_meta(meta, command) else {
                    return Ok(());
                };
                meta = next;
                given.clear();
            }
            Ok(Event::Flag { flag, value, .. }) => {
                // A request for help or the version is answered by the real parse, and
                // what follows it is never read, but a value written after it is a
                // mistake (`--help=1`), as it is after any other switch.
                if usage::is_help_flag(flag) || usage::is_version_flag(flag) {
                    return builtin_switch_value(flag, argv);
                }
                if let Some(value) = value.filter(|_| flag.bool_value && !flag.takes_value) {
                    return Err(switch_value(flag, value));
                }
                let Some(flag_meta) = flag_meta(meta, flag) else {
                    continue;
                };
                // Only a command whose own declaration lets the last value win needs the
                // check: the root and the `Args` structs refuse a repeat themselves.
                if strict
                    && meta.args_override_self
                    && !repeats(flag_meta)
                    && given.contains(&flag.key)
                {
                    return Err(Error::DuplicateFlag { name: flag.name });
                }
                given.push(flag.key);
            }
            Ok(_) => {}
            // The parser's own refusal of a value other than `true` or `false` after a
            // `bool_value` switch (`--uc=1`): the same mistake, said the same way.
            Err(Error::InvalidChoice { name, .. }) => {
                return match switch_named(meta, name) {
                    Some(flag) => Err(switch_value(flag, written_value(flag, argv))),
                    None => Ok(()),
                };
            }
            // The `help` word, which the parser answers where the path of commands ends.
            Err(Error::Help { cmd, .. }) => return help_path(&parser, cmd, argv),
            // Anything else is the real parse's to report.
            Err(_) => return Ok(()),
        }
    }
    Ok(())
}

/// The metadata of `command`, a subcommand of the one `meta` describes.
fn subcommand_meta(
    meta: &'static CommandMeta<'static>,
    command: &Command<'_>,
) -> Option<&'static CommandMeta<'static>> {
    meta.subcommands
        .iter()
        .copied()
        .find(|candidate| candidate.cmd.key == command.key)
}

/// The metadata of `flag`, a flag of the command `meta` describes. A flag the command does
/// not declare itself (one of an ancestor) has none here.
fn flag_meta<'m>(meta: &'m CommandMeta<'static>, flag: &Flag<'_>) -> Option<&'m FlagMeta<'static>> {
    meta.flags
        .iter()
        .find(|candidate| candidate.flag.key == flag.key)
}

/// Whether the flag may be given more than once: a collection, or a counter.
fn repeats(flag: &FlagMeta<'_>) -> bool {
    flag.repeatable || flag.count
}

/// The switch of the command `meta` describes that the parser calls `name`.
fn switch_named(meta: &'static CommandMeta<'static>, name: &str) -> Option<&'static Flag<'static>> {
    meta.flags
        .iter()
        .map(|candidate| candidate.flag)
        .find(|flag| flag.name == name && flag.bool_value && !flag.takes_value)
}

/// What `argv` writes after `--flag=` for the first spelling of the switch it finds, or
/// nothing when it cannot be found.
fn written_value<'v>(flag: &Flag<'_>, argv: &[&'v OsStr]) -> &'v [u8] {
    argv.iter()
        .find_map(|word| {
            let body = word.as_encoded_bytes().strip_prefix(b"--")?;
            let at = body.iter().position(|byte| *byte == b'=')?;
            let spelling = &body[..at];
            flag.longs
                .iter()
                .any(|long| long.as_bytes() == spelling)
                .then_some(&body[at + 1..])
        })
        .unwrap_or_default()
}

/// The error for `value` written after the switch `flag`: `--headless=true`.
fn switch_value<'v>(flag: &Flag<'static>, value: &'v [u8]) -> Error<'static, 'v> {
    // The name is what the message calls the flag, and a name with no flag of that name
    // in the command is printed as it is. Leaked: the process is about to exit with the
    // message, and the error type wants a `'static` name.
    let long = flag.longs.first().copied().unwrap_or(flag.name);
    let name: &'static str = Box::leak(format!("--{long}").into_boxed_str());
    Error::InvalidValue(Box::new(InvalidValue {
        name,
        value: String::from_utf8_lossy(value).into_owned(),
        reason: "a switch takes no value".to_owned(),
    }))
}

/// Refuses a value written after the built-in `--help` or `--version`.
///
/// The parser drops it and answers the request, so the word has to be found in `argv`: the
/// first spelling of a help or version request is the one the parser just answered.
fn builtin_switch_value<'v>(
    flag: &Flag<'static>,
    argv: &[&'v OsStr],
) -> Result<(), Error<'static, 'v>> {
    for word in argv {
        let bytes = word.as_encoded_bytes();
        if matches!(bytes, b"--" | b"-h" | b"-V") {
            break;
        }
        let Some(body) = bytes.strip_prefix(b"--") else {
            continue;
        };
        let (spelling, value) = match body.iter().position(|byte| *byte == b'=') {
            Some(at) => (&body[..at], Some(&body[at + 1..])),
            None => (body, None),
        };
        if matches!(spelling, b"help" | b"version") {
            return value.map_or(Ok(()), |value| Err(switch_value(flag, value)));
        }
    }
    Ok(())
}

/// Whether the words after a `help` path are all part of the path.
///
/// `parser` has just answered the `help` word, so it knows where the path of commands
/// ended and `cmd` is the command it ended at: the first word after it, if there is one,
/// names no command. Two words are allowed there all the same: a request for help for the
/// page itself (`help mkdir --help`), and `help` where `cmd` has the `help` subcommand of
/// its own (`help help`).
fn help_path<'v>(
    parser: &Parser<'static, '_, 'v>,
    cmd: &Command<'_>,
    argv: &[&'v OsStr],
) -> Result<(), Error<'static, 'v>> {
    let (from, to) = parser.help_span();
    // `(0, 0)` is no `help` word: the parser did not answer one.
    if (from, to) == (0, 0) {
        return Ok(());
    }
    match argv.get(to).copied() {
        Some(word) if matches!(word.as_encoded_bytes(), b"-h" | b"--help") => Ok(()),
        Some(word)
            if word.as_encoded_bytes() == b"help"
                && !cmd.subcommands.is_empty()
                && !cmd.disable_help_subcommand =>
        {
            Ok(())
        }
        Some(word) => Err(Error::UnexpectedArg {
            token: word.as_encoded_bytes(),
        }),
        None => Ok(()),
    }
}
