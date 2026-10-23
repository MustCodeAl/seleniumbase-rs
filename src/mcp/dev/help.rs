//! `cli_help`: the `sbrs` command line, from its usage spec.
//!
//! The spec is `completions/sbrs.usage.kdl`, embedded and read by
//! [`spec`](super::spec). The parser, the shell completions and the CLI reference
//! page are all generated from it, so this answer cannot drift from them, however
//! the page is laid out. Parsing the spec with the `usage` crates would put a
//! command-line crate into the library, which `tests/library_manifest.rs` forbids.

use std::fmt::Write as _;
use std::sync::OnceLock;

use super::spec::{Arg, Choice, Command, Flag, Spec, SpecError};
use super::NoBrowser;
use crate::error::{closest_match, SeleniumBaseError};
use crate::mcp::support::nonempty;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

/// The usage spec of `sbrs`, written by the CLI crate's test `usage_spec`.
const USAGE_SPEC: &str = include_str!("../../../completions/sbrs.usage.kdl");

/// The tools of this file.
pub(super) fn tools() -> Vec<ToolDef<NoBrowser>> {
    vec![ToolDef::new(
        "cli_help",
        "CLI Help",
        "Explain the `sbrs` command line. With no command: the global flags, one line each, and \
         every command with a one-line description. With a command (`open`, `wait-for-text`, \
         `plugin list`) or one of its aliases: its usage, arguments, flags with their defaults, \
         choices and aliases. With `global`: the global flags in full. It is read from the \
         usage spec of this version, the one `sbrs --help`, the shell completions and the CLI \
         reference page are generated from.",
        Effect::Inspect,
        Schema::new().optional(
            "command",
            Prop::string(
                "A command or alias, such as `mkdir`, `install` or `plugin list`; `global` for \
                 the flags that go before or after the command",
            ),
        ),
        cli_help,
    )]
}

/// The embedded spec, read once.
fn spec() -> Result<&'static Spec, ToolError> {
    static SPEC: OnceLock<Result<Spec, SpecError>> = OnceLock::new();
    SPEC.get_or_init(|| Spec::parse(USAGE_SPEC))
        .as_ref()
        .map_err(|error| {
            ToolError::Failed(SeleniumBaseError::mcp(
                "cli_help",
                format!("the usage spec of this build cannot be read: {error}"),
            ))
        })
}

// ----------------------------------------------------------------------
// Rendering
// ----------------------------------------------------------------------

/// `text` with every line indented by `spaces`; blank lines stay empty.
fn indent(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a long help says beyond the short one. The `usage` crate starts every long
/// help with the short help, so that paragraph is not repeated.
fn beyond(help: &str, long_help: Option<&str>) -> Option<String> {
    let long = long_help?.trim();
    let rest = long.strip_prefix(help.trim()).unwrap_or(long).trim();
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// A flag as it is listed: its short spellings, its first long one and the value it
/// takes, `-n, --threads <THREADS>`.
fn flag_label(flag: &Flag) -> String {
    let mut names = flag.shorts();
    names.extend(flag.longs().first());
    if names.is_empty() {
        names.extend(flag.names.first().map(String::as_str));
    }
    let mut label = names.join(", ");
    if let Some(value) = &flag.value {
        label.push(' ');
        label.push_str(value);
    }
    label
}

/// A flag as the usage line writes it: `--css <CSS>`, with `...` if it repeats.
fn flag_usage(flag: &Flag) -> String {
    let name = flag
        .longs()
        .first()
        .copied()
        .or_else(|| flag.names.first().map(String::as_str))
        .unwrap_or_default();
    let mut usage = name.to_owned();
    if let Some(value) = &flag.value {
        usage.push(' ');
        usage.push_str(value);
    }
    if flag.repeatable {
        usage.push_str("...");
    }
    usage
}

/// The facts about a flag that do not fit its help: whether it is required or may
/// repeat, its default, its other spellings and the flags it excludes.
fn flag_notes(flag: &Flag) -> Vec<String> {
    let mut notes = Vec::new();
    if flag.required {
        notes.push("required".to_owned());
    }
    if flag.repeatable && !flag.help.contains("repeatable") {
        notes.push("repeatable".to_owned());
    }
    if let Some(default) = &flag.default {
        notes.push(format!("default: {default}"));
    }
    let aliases: Vec<&str> = flag.longs().into_iter().skip(1).collect();
    if !aliases.is_empty() {
        notes.push(format!("also {}", aliases.join(", ")));
    }
    if !flag.conflicts.is_empty() {
        notes.push(format!("not with {}", flag.conflicts.join(", ")));
    }
    notes
}

/// The values a flag or an argument takes, with the help of those that have one.
fn write_choices(out: &mut String, choices: &[Choice]) {
    if choices.is_empty() {
        return;
    }
    let list: Vec<String> = choices
        .iter()
        .map(|choice| match &choice.help {
            Some(help) => format!("{} ({help})", choice.name),
            None => choice.name.clone(),
        })
        .collect();
    let _ = writeln!(out, "    Choices: {}", list.join(", "));
}

fn write_long_help(out: &mut String, help: &str, long_help: Option<&str>) {
    if let Some(more) = beyond(help, long_help) {
        let _ = writeln!(out, "{}", indent(&more, 4));
    }
}

/// One line for a flag, and, with `detail`, its notes, choices and long help.
fn write_flag(out: &mut String, flag: &Flag, detail: bool) {
    let _ = write!(out, "- {}", flag_label(flag));
    if !flag.help.is_empty() {
        let _ = write!(out, ": {}", flag.help);
    }
    let notes = flag_notes(flag);
    if detail && !notes.is_empty() {
        let _ = write!(out, " [{}]", notes.join("; "));
    }
    out.push('\n');
    if detail {
        write_choices(out, &flag.choices);
        write_long_help(out, &flag.help, flag.long_help.as_deref());
    }
}

fn write_arg(out: &mut String, arg: &Arg) {
    let _ = write!(out, "- {}", arg.name);
    if !arg.help.is_empty() {
        let _ = write!(out, ": {}", arg.help);
    }
    if let Some(default) = &arg.default {
        let _ = write!(out, " [default: {default}]");
    }
    out.push('\n');
    write_choices(out, &arg.choices);
    write_long_help(out, &arg.help, arg.long_help.as_deref());
}

/// How a command is called: the program, the command, its flags (optional ones in
/// brackets), its arguments, and `<SUBCOMMAND>` if it has any.
fn usage_line(spec: &Spec, command: &Command) -> String {
    let mut usage = spec.bin.clone();
    if !command.path.is_empty() {
        usage.push(' ');
        usage.push_str(&command.path);
    }
    for flag in command.flags.iter().filter(|flag| !flag.builtin) {
        let piece = flag_usage(flag);
        if flag.required {
            let _ = write!(usage, " {piece}");
        } else {
            let _ = write!(usage, " [{piece}]");
        }
    }
    for arg in &command.args {
        let _ = write!(usage, " {}", arg.name);
    }
    if !command.subcommands.is_empty() {
        usage.push_str(" <SUBCOMMAND>");
    }
    usage
}

/// The program's name and version, what it is, and how it is called.
fn program_header(spec: &Spec) -> String {
    let mut out = format!(
        "{} {}\n",
        spec.bin,
        spec.version.as_deref().unwrap_or_default()
    );
    let about = spec.long_about.as_deref().unwrap_or(&spec.about);
    if !about.is_empty() {
        let _ = writeln!(out, "{about}");
    }
    let _ = writeln!(out, "Usage: {} [FLAGS] <SUBCOMMAND>", spec.bin);
    out
}

/// The overview: the global flags, one line each, and every command.
fn overview(spec: &Spec) -> String {
    let mut out = program_header(spec);
    out.push_str(
        "\nGlobal flags (they go before or after the command; cli_help with command `global` gives \
         their choices, aliases and long help):\n",
    );
    for flag in &spec.root.flags {
        write_flag(&mut out, flag, false);
    }
    let commands = spec.commands();
    let _ = writeln!(
        out,
        "\nCommands ({}); cli_help with a command gives its arguments, flags, defaults and \
         aliases:",
        commands.len()
    );
    for command in commands {
        let _ = write!(out, "- {}", command.path);
        if !command.aliases.is_empty() {
            let _ = write!(out, " (alias {})", command.aliases.join(", "));
        }
        if !command.help.is_empty() {
            let _ = write!(out, ": {}", command.help);
        }
        out.push('\n');
    }
    out
}

/// The program with its global flags in full.
fn render_root(spec: &Spec) -> String {
    let mut out = program_header(spec);
    out.push_str("\nGlobal flags:\n");
    for flag in &spec.root.flags {
        write_flag(&mut out, flag, true);
    }
    out
}

/// One command: what it does, how it is called, and its arguments and flags.
fn render_command(spec: &Spec, command: &Command) -> String {
    let mut out = format!("{} {}\n", spec.bin, command.path);
    if !command.help.is_empty() {
        let _ = writeln!(out, "{}", command.help);
    }
    if let Some(more) = beyond(&command.help, command.long_help.as_deref()) {
        let _ = write!(out, "\n{more}\n");
    }
    let _ = write!(out, "\nUsage: {}\n", usage_line(spec, command));
    if !command.aliases.is_empty() {
        let _ = writeln!(out, "Aliases: {}", command.aliases.join(", "));
    }
    if !command.subcommands.is_empty() {
        out.push_str("\nSubcommands:\n");
        for sub in &command.subcommands {
            let _ = writeln!(out, "- {}: {}", sub.path, sub.help);
        }
    }
    if !command.args.is_empty() {
        out.push_str("\nArguments:\n");
        for arg in &command.args {
            write_arg(&mut out, arg);
        }
    }
    if !command.flags.is_empty() {
        out.push_str("\nFlags:\n");
        for flag in &command.flags {
            write_flag(&mut out, flag, true);
        }
    }
    out
}

// ----------------------------------------------------------------------
// Finding a command
// ----------------------------------------------------------------------

/// `command` as the spec names a command: lowercase, with `-` for `_`, and without a
/// leading `sbrs`. The program itself (`sbrs`, `global`) is the empty name.
fn bare(command: &str) -> String {
    let lower = command.trim().to_lowercase().replace('_', "-");
    let name = match lower.strip_prefix("sbrs") {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => rest.trim(),
        _ => lower.as_str(),
    };
    if matches!(name, "global" | "global flags" | "flags") {
        String::new()
    } else {
        name.to_owned()
    }
}

/// The commands `wanted` (a [`bare`] name) names: the command itself, or the one an
/// alias stands for, and every command below it.
fn find<'a>(spec: &'a Spec, wanted: &str) -> Vec<&'a Command> {
    let all = spec.commands();
    let exact = all
        .iter()
        .copied()
        .find(|command| command.path == wanted)
        .or_else(|| {
            all.iter()
                .copied()
                .find(|command| command.aliases.iter().any(|alias| alias == wanted))
        });
    let below = format!("{wanted} ");
    let children = all
        .iter()
        .copied()
        .filter(|command| command.path.starts_with(&below));
    exact.into_iter().chain(children).collect()
}

async fn cli_help(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let spec = spec()?;
    let Some(command) = nonempty(&args, "command")? else {
        return Ok(overview(spec).into());
    };
    let wanted = bare(command);
    if wanted.is_empty() {
        return Ok(render_root(spec).into());
    }
    let found = find(spec, &wanted);
    if found.is_empty() {
        let mut reason = format!("no command `{command}`; cli_help without a command lists them");
        let names = spec
            .commands()
            .into_iter()
            .map(|command| command.path.as_str());
        if let Some(near) = closest_match(&wanted, names) {
            let _ = write!(reason, "; did you mean `{near}`?");
        }
        return Err(ToolError::invalid("command", reason));
    }
    let text = found
        .iter()
        .map(|command| render_command(spec, command))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text.into())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::mcp::Settings;

    async fn call(arguments: Value) -> Result<Output, ToolError> {
        super::super::host(Settings::new("out"))
            .call("cli_help", arguments.as_object().cloned().unwrap())
            .await
    }

    async fn text(arguments: Value) -> String {
        call(arguments).await.unwrap().as_text().unwrap().to_owned()
    }

    #[test]
    fn the_embedded_spec_reads_and_lists_the_commands_of_sbrs() {
        let spec = spec().unwrap();

        assert_eq!(spec.bin, "sbrs");
        assert!(spec.commands().len() > 100, "{}", spec.commands().len());
        let paths: Vec<&str> = spec.commands().iter().map(|c| c.path.as_str()).collect();
        for expected in ["open", "wait-for-text", "install", "plugin", "plugin list"] {
            assert!(paths.contains(&expected), "{expected}");
        }
    }

    #[tokio::test]
    async fn without_a_command_the_help_lists_the_global_flags_and_every_command() {
        let overview = text(json!({})).await;

        assert!(overview.starts_with("sbrs "), "{overview}");
        assert!(overview.contains("Global flags"), "{overview}");
        assert!(
            overview.contains("- --webdriver <URL>: WebDriver server URL"),
            "{overview}"
        );
        assert!(
            overview.contains("\n- open: Open a URL in the browser.\n"),
            "{overview}"
        );
        assert!(
            overview.contains("\n- encrypt (alias obfuscate): "),
            "{overview}"
        );
        assert!(overview.contains("\n- plugin list: "), "{overview}");
        let spec = spec().unwrap();
        assert!(
            overview.contains(&format!("Commands ({});", spec.commands().len())),
            "{overview}"
        );
        for command in spec.commands() {
            assert!(
                overview.contains(&format!("\n- {}", command.path)),
                "{} is missing",
                command.path
            );
        }
    }

    #[tokio::test]
    async fn the_overview_keeps_one_line_for_each_global_flag_and_global_gives_them_in_full() {
        let overview = text(json!({})).await;
        assert!(!overview.contains("Choices:"), "{overview}");
        assert!(!overview.contains("Adds to the plugins"), "{overview}");

        for given in ["global", "sbrs", "Global Flags"] {
            let full = text(json!({ "command": given })).await;
            assert!(full.starts_with("sbrs "), "{given}: {full}");
            assert!(full.contains("\nGlobal flags:\n"), "{given}");
            assert!(
                full.contains("Choices: chrome, chromium, edge, firefox"),
                "{given}"
            );
            assert!(
                full.contains("Adds to the plugins the settings file"),
                "{given}"
            );
            assert!(full.contains("- -h, --help: Print help"), "{given}");
        }
    }

    #[tokio::test]
    async fn a_command_shows_how_it_is_called_and_its_flags_with_their_defaults() {
        let help = text(json!({ "command": "wait-for-text" })).await;

        assert!(
            help.starts_with("sbrs wait-for-text\nWait until an element contains"),
            "{help}"
        );
        assert!(
            help.contains(
                "\nUsage: sbrs wait-for-text --css <CSS> --text <TEXT> [--timeout <TIMEOUT>]\n"
            ),
            "{help}"
        );
        assert!(
            help.contains("- --css <CSS>: CSS selector for the target element [required]\n"),
            "{help}"
        );
        assert!(
            help.contains("- --timeout <TIMEOUT>: Maximum wait time in seconds [default: 10]\n"),
            "{help}"
        );
        assert!(help.contains("- -h, --help: Print help"), "{help}");
    }

    #[tokio::test]
    async fn arguments_choices_repeats_and_other_spellings_are_shown() {
        let open = text(json!({ "command": "open" })).await;
        assert!(open.contains("\nUsage: sbrs open <URL>\n"), "{open}");
        assert!(
            open.contains("\nArguments:\n- <URL>: URL to open in the browser.\n"),
            "{open}"
        );

        let completions = text(json!({ "command": "completions" })).await;
        assert!(
            completions.contains("    Choices: bash, zsh, fish"),
            "{completions}"
        );

        let global = text(json!({ "command": "global" })).await;
        assert!(
            !global.contains("[repeatable]"),
            "the help says it already: {global}"
        );
        // A repeatable flag whose help does not say so gets the note.
        let deferred = text(json!({ "command": "deferred" })).await;
        assert!(
            deferred
                .lines()
                .any(|line| line.starts_with("- --assert <SPEC>: ")
                    && line.ends_with(" [repeatable]")),
            "{deferred}"
        );
        assert!(
            global.contains("- --reuse-session: Share browsers among the tests of the run")
                && global.contains("[also --reuse_session, --rs]"),
            "{global}"
        );
        assert!(global.contains("- -n, --threads <THREADS>:"), "{global}");
    }

    #[tokio::test]
    async fn an_alias_the_sbrs_prefix_and_capitals_find_the_command() {
        for given in ["obfuscate", "sbrs encrypt", "Encrypt", "ENCRYPT"] {
            let help = text(json!({ "command": given })).await;
            assert!(help.starts_with("sbrs encrypt\n"), "{given}: {help}");
            assert!(help.contains("Aliases: obfuscate"), "{given}");
        }
        let underscored = text(json!({ "command": "wait_for_text" })).await;
        assert!(
            underscored.starts_with("sbrs wait-for-text\n"),
            "{underscored}"
        );
    }

    #[tokio::test]
    async fn the_long_help_of_a_command_is_not_said_twice() {
        let help = text(json!({ "command": "encrypt" })).await;

        assert_eq!(
            help.matches("Encrypt text with a passphrase").count(),
            1,
            "{help}"
        );
        assert!(
            help.contains("The passphrase comes from the SB_ENCRYPTION_KEY"),
            "{help}"
        );
    }

    #[tokio::test]
    async fn a_group_of_commands_shows_itself_and_its_subcommands() {
        let help = text(json!({ "command": "plugin" })).await;

        assert!(help.starts_with("sbrs plugin\n"), "{help}");
        assert!(help.contains("\nSubcommands:\n- plugin list: "), "{help}");
        assert!(help.contains("\nsbrs plugin list\n"), "{help}");
        assert!(help.contains("\nsbrs plugin info\n"), "{help}");
        assert!(help.contains("Usage: sbrs plugin info"), "{help}");
    }

    #[tokio::test]
    async fn every_command_of_the_spec_can_be_asked_for() {
        let spec = spec().unwrap();
        for command in spec.commands() {
            let help = text(json!({ "command": command.path })).await;
            assert!(
                help.starts_with(&format!("sbrs {}\n", command.path)),
                "{}",
                command.path
            );
            assert!(
                help.contains(&format!("Usage: sbrs {}", command.path)),
                "{}",
                command.path
            );
        }
    }

    #[tokio::test]
    async fn an_unknown_command_is_an_error_with_the_nearest_name() {
        let error = call(json!({ "command": "opn" })).await.unwrap_err();

        assert!(
            matches!(&error, ToolError::InvalidArgument { name, reason }
                if name == "command" && reason.contains("did you mean `open`?")),
            "{error}"
        );
        let far = call(json!({ "command": "frobnicate" })).await.unwrap_err();
        assert!(!far.to_string().contains("did you mean"), "{far}");
    }

    #[test]
    fn a_command_is_read_as_the_spec_names_it() {
        assert_eq!(bare("  SBRS Wait_For_Text "), "wait-for-text");
        assert_eq!(bare("sbrs plugin list"), "plugin list");
        assert_eq!(bare("sbrs"), "");
        assert_eq!(bare("global"), "");
        // Only the word `sbrs` is dropped, not letters a command starts with.
        assert_eq!(bare("sbrsx"), "sbrsx");
    }

    #[test]
    fn the_usage_line_puts_required_flags_bare_and_optional_ones_in_brackets() {
        let spec = Spec::parse(
            "bin demo\n\
             cmd run help=\"Run it.\" {\n\
             \x20   flag --must help=\"m\" required=#true {\n        arg <M>\n    }\n\
             \x20   flag \"-o --opt\" help=\"o\" {\n        arg <O>\n    }\n\
             \x20   flag --many help=\"x\" var=#true {\n        arg <X>\n    }\n\
             \x20   flag --quiet help=\"q\" bool_value=#true\n\
             \x20   flag \"-h --help\" help=\"Print help\" action=help builtin=#true\n\
             \x20   arg <FILE> help=\"f\"\n\
             \x20   arg \"[REST]...\" help=\"r\"\n\
             }\n",
        )
        .unwrap();

        let usage = usage_line(&spec, spec.commands()[0]);

        assert_eq!(
            usage,
            "demo run --must <M> [--opt <O>] [--many <X>...] [--quiet] <FILE> [REST]..."
        );
    }

    #[test]
    fn what_a_long_help_adds_is_what_follows_the_short_one() {
        assert_eq!(
            beyond("Short.", Some("Short.\n\nMore.")),
            Some("More.".to_owned())
        );
        assert_eq!(beyond("Short.", Some("Short.")), None);
        assert_eq!(
            beyond("Short.", Some("Other words.")),
            Some("Other words.".to_owned())
        );
        assert_eq!(beyond("Short.", None), None);
    }

    #[test]
    fn a_spec_that_cannot_be_read_is_an_error_a_model_can_read() {
        let error = Spec::parse("cmd open {\n").unwrap_err();

        assert_eq!(error.line, 2);
        assert!(error.to_string().contains("never closed"), "{error}");
    }
}
