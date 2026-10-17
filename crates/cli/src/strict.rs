//! What usage 6 lets through that clap did not, refused before the real parse.
//!
//! usage 6.12 is looser than clap in places that no attribute of the derives closes:
//!
//! - A value written after a switch. The parser drops it (`--headless=false` would switch
//!   the option on). A switch declared `bool_value` hands the value over instead, so the
//!   parser can see it; the derive then accepts `true` and `false`, and clap accepted
//!   neither.
//!
//! [`check`] reads the events the generated parser reads, from the same tables, and
//! refuses these with the errors usage renders for everything else. Every other mistake,
//! and every help or version request, is left to the real parse.

use std::ffi::OsStr;

use usage::spec::{CommandMeta, Spec};
use usage::{Command, Error, Event, Flag, InvalidValue, Parser};

/// Whether `argv`, the command line without the program name, is one clap would have
/// refused for one of the reasons above.
///
/// `spec` and `root` are those of the same CLI (`Cli::spec()` and `Cli::command()`).
/// Returns the first mistake, as the error the parser itself would have returned for it.
///
/// # Errors
///
/// [`Error::InvalidValue`] for a value after a switch.
pub fn check<'v>(
    spec: &'static Spec<'static>,
    root: &'static Command<'static>,
    argv: &[&'v OsStr],
) -> Result<(), Error<'static, 'v>> {
    let mut parser = Parser::new(root, argv);
    // The metadata of the command the parser is in.
    let mut meta = spec.root;
    while let Some(event) = parser.next_event() {
        match event {
            Ok(Event::Command(command)) => {
                let Some(next) = subcommand_meta(meta, command) else {
                    return Ok(());
                };
                meta = next;
            }
            Ok(Event::Flag { flag, value, .. }) => {
                // A request for help or the version is answered by the real parse, and
                // what follows it is never read.
                if usage::is_help_flag(flag) || usage::is_version_flag(flag) {
                    return Ok(());
                }
                if let Some(value) = value.filter(|_| flag.bool_value && !flag.takes_value) {
                    return Err(switch_value(flag, value));
                }
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
