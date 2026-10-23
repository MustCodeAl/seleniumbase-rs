//! `docs/help/cli_reference.md`, the usage spec rendered as a page of the Docs7 site.
//!
//! The page lists every command in a table, then gives each one a section: its help,
//! its usage in a titled `bash` block, and a `ParamField` for each argument and flag,
//! with the flag's other spellings, its value, choices, default and environment
//! variable. Built only for the test that keeps the committed page current.

use usage_lib::{Spec, SpecArg, SpecCommand, SpecFlag};

/// The front matter, a note on how the page is made, then the reference.
pub(crate) fn render(spec: &Spec) -> String {
    let mut out = String::from(
        "---\n\
         title: \"CLI reference\"\n\
         sidebarTitle: \"CLI reference\"\n\
         description: \"Every sbrs command with its arguments and flags, generated from the parser.\"\n\
         ---\n\n\
         <Info>\n\n\
         Generated from the usage spec of `sbrs` (`completions/sbrs.usage.kdl`) by \
         `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`. \
         Do not edit it by hand. For the common commands by task, see the \
         [CLI TL;DR](/help/cli_tldr).\n\n\
         </Info>\n\n",
    );
    let root = &spec.cmd;
    if let Some(version) = &spec.version {
        out.push_str(&format!(
            "<Badge>{} {}</Badge>\n\n",
            spec.bin,
            prose(version)
        ));
    }
    if let Some(about) = spec.about_long.as_ref().or(spec.about.as_ref()) {
        out.push_str(&prose(about.trim()));
        out.push_str("\n\n");
    }
    out.push_str(&usage_block(&spec.bin, root));

    let commands = all_commands(root);
    out.push_str("## Commands\n\n| Command | What it does |\n|---|---|\n");
    for cmd in &commands {
        let name = full_name(&spec.bin, cmd);
        let help = cmd.help.as_deref().unwrap_or_default();
        let first_line = help.lines().next().unwrap_or_default();
        out.push_str(&format!(
            "| [`{name}`](#{}) | {} |\n",
            anchor(&name),
            prose(first_line).replace('|', "\\|")
        ));
    }
    out.push('\n');

    let flags: Vec<&SpecFlag> = root.flags.iter().filter(|f| !f.hide).collect();
    if !flags.is_empty() {
        out.push_str(
            "## Global flags\n\nEach applies to every command and goes before the command or after it, but not both: `sbrs --headless open URL` and `sbrs open URL --headless` are the same.\n\n",
        );
        for flag in flags {
            out.push_str(&flag_field(flag));
        }
    }

    for cmd in &commands {
        out.push_str(&command_section(&spec.bin, cmd));
    }
    out
}

/// Every visible command below `root`, each followed by its own subcommands, in
/// alphabetical order at each level as `sbrs --help` lists them.
fn all_commands(root: &SpecCommand) -> Vec<&SpecCommand> {
    let mut children: Vec<&SpecCommand> = root.subcommands.values().filter(|c| !c.hide).collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = Vec::new();
    for child in children {
        out.push(child);
        out.extend(all_commands(child));
    }
    out
}

fn full_name(bin: &str, cmd: &SpecCommand) -> String {
    let mut name = bin.to_owned();
    for part in &cmd.full_cmd {
        name.push(' ');
        name.push_str(part);
    }
    name
}

/// The heading id the site gives `## sbrs plugin list`: `sbrs-plugin-list`.
fn anchor(name: &str) -> String {
    name.replace(' ', "-")
}

fn usage_block(bin: &str, cmd: &SpecCommand) -> String {
    format!(
        "```bash Usage\n{}\n```\n\n",
        format!("{bin} {}", cmd.usage).trim()
    )
}

fn command_section(bin: &str, cmd: &SpecCommand) -> String {
    let name = full_name(bin, cmd);
    // `## sbrs open`, `### sbrs plugin list`.
    let level = "#".repeat(cmd.full_cmd.len() + 1);
    let mut out = format!("{level} {name}\n\n");
    if !cmd.aliases.is_empty() {
        let aliases: Vec<String> = cmd.aliases.iter().map(|a| format!("`{a}`")).collect();
        out.push_str(&format!("Also: {}\n\n", aliases.join(", ")));
    }
    if let Some(deprecated) = &cmd.deprecated {
        out.push_str(&format!(
            "<Warning>\n\nDeprecated: {}\n\n</Warning>\n\n",
            prose(deprecated)
        ));
    }
    for text in [
        cmd.before_help_long.as_ref().or(cmd.before_help.as_ref()),
        cmd.help_long.as_ref().or(cmd.help.as_ref()),
    ]
    .into_iter()
    .flatten()
    {
        out.push_str(&prose(text.trim()));
        out.push_str("\n\n");
    }
    out.push_str(&usage_block(bin, cmd));

    let args: Vec<&SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    if !args.is_empty() {
        out.push_str("**Arguments**\n\n");
        for arg in args {
            out.push_str(&arg_field(arg));
        }
    }
    // Every command has `-h, --help`; the page says so once, under the global flags.
    let flags: Vec<&SpecFlag> = cmd
        .flags
        .iter()
        .filter(|f| !f.hide && !f.global && !f.long.iter().any(|l| l == "help"))
        .collect();
    if !flags.is_empty() {
        out.push_str("**Flags**\n\n");
        for flag in flags {
            out.push_str(&flag_field(flag));
        }
    }
    out.push_str(&examples_section(cmd));
    if let Some(text) = cmd.after_help_long.as_ref().or(cmd.after_help.as_ref()) {
        out.push_str(&prose(text.trim()));
        out.push_str("\n\n");
    }
    out
}

/// The examples of a command: those with no header and no text of their own in one `bash`
/// block titled "Examples", one command per line, then each of the others under its header.
fn examples_section(cmd: &SpecCommand) -> String {
    let examples = &cmd.examples;
    let mut out = String::new();
    let plain: Vec<_> = examples
        .iter()
        .filter(|example| example.header.is_none() && example.help.is_none())
        .collect();
    if !plain.is_empty() {
        out.push_str("```bash Examples\n");
        for example in plain {
            out.push_str(example.code.trim_end());
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    for example in examples
        .iter()
        .filter(|example| example.header.is_some() || example.help.is_some())
    {
        if let Some(header) = &example.header {
            out.push_str(&format!("**{}**\n\n", prose(header)));
        }
        if let Some(help) = &example.help {
            out.push_str(&prose(help.trim()));
            out.push_str("\n\n");
        }
        let lang = if example.lang.is_empty() {
            "bash"
        } else {
            example.lang.as_str()
        };
        out.push_str(&format!(
            "```{lang} Example\n{}\n```\n\n",
            example.code.trim_end()
        ));
    }
    out
}

fn arg_field(arg: &SpecArg) -> String {
    let mut attributes = format!(
        "path=\"{}\" type=\"{}\"",
        attribute(&arg.name),
        if arg.var { "string[]" } else { "string" }
    );
    if arg.required {
        attributes.push_str(" required");
    }
    if !arg.default.is_empty() {
        attributes.push_str(&format!(
            " default=\"{}\"",
            attribute(&arg.default.join(", "))
        ));
    }
    let mut body = help_of(arg.help_long.as_ref().or(arg.help.as_ref()));
    body.push_str(&choices_and_env(arg, arg.env.as_deref()));
    field(&attributes, &body)
}

fn flag_field(flag: &SpecFlag) -> String {
    let mut spellings: Vec<String> = flag.long.iter().map(|l| format!("--{l}")).collect();
    spellings.extend(flag.short.iter().map(|s| format!("-{s}")));
    let mut name = spellings
        .first()
        .cloned()
        .unwrap_or_else(|| flag.name.clone());
    if let Some(arg) = &flag.arg {
        name.push(' ');
        name.push_str(&arg.usage);
    }
    let kind = if flag.count {
        "count"
    } else if flag.arg.is_none() {
        "boolean"
    } else if flag.var {
        "string[]"
    } else {
        "string"
    };
    let mut attributes = format!("query=\"{}\" type=\"{kind}\"", attribute(&name));
    if flag.required {
        attributes.push_str(" required");
    }
    let default = if flag.default.is_empty() {
        flag.arg
            .as_ref()
            .map(|a| a.default.join(", "))
            .unwrap_or_default()
    } else {
        flag.default.join(", ")
    };
    if !default.is_empty() {
        attributes.push_str(&format!(" default=\"{}\"", attribute(&default)));
    }

    let mut body = help_of(flag.help_long.as_ref().or(flag.help.as_ref()));
    if spellings.len() > 1 {
        let others: Vec<String> = spellings[1..].iter().map(|s| format!("`{s}`")).collect();
        body.push_str(&format!("Also: {}\n\n", others.join(", ")));
    }
    if let Some(arg) = &flag.arg {
        body.push_str(&choices_and_env(
            arg,
            flag.env.as_deref().or(arg.env.as_deref()),
        ));
    } else if let Some(env) = &flag.env {
        body.push_str(&format!("Environment: `{env}`\n\n"));
    }
    if let Some(deprecated) = &flag.deprecated {
        body.push_str(&format!("Deprecated: {}\n\n", prose(deprecated)));
    }
    field(&attributes, &body)
}

fn help_of(help: Option<&String>) -> String {
    help.map(|h| format!("{}\n\n", prose(h.trim())))
        .unwrap_or_default()
}

fn choices_and_env(arg: &SpecArg, env: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(choices) = arg.choices.as_ref().filter(|c| !c.choices.is_empty()) {
        let choices: Vec<String> = choices.choices.iter().map(|c| format!("`{c}`")).collect();
        out.push_str(&format!("One of: {}\n\n", choices.join(", ")));
    }
    if let Some(env) = env {
        out.push_str(&format!("Environment: `{env}`\n\n"));
    }
    out
}

fn field(attributes: &str, body: &str) -> String {
    format!("<ParamField {attributes}>\n\n{body}</ParamField>\n\n")
}

/// A value inside a double-quoted JSX attribute.
fn attribute(value: &str) -> String {
    value.replace('"', "&quot;")
}

/// Help text as MDX prose: `<` outside code would open a tag and braces outside
/// code an expression, so both are escaped; code spans keep them.
fn prose(text: &str) -> String {
    let mut lt_escaped = String::with_capacity(text.len());
    let mut in_code = false;
    for ch in text.chars() {
        match ch {
            '`' => in_code = !in_code,
            '<' if !in_code => {
                lt_escaped.push_str("&lt;");
                continue;
            }
            _ => {}
        }
        lt_escaped.push(ch);
    }
    escape_mdx_braces(&lt_escaped)
}

/// Braces outside code are JavaScript expressions in MDX (the Docs7 site), so a
/// help text such as `{browser} {major}` would fail there; `\{` is a literal brace
/// in MDX and in CommonMark (mdBook) alike. Code spans and fenced blocks keep theirs.
fn escape_mdx_braces(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_fence = false;
    for line in markdown.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        let mut in_code = false;
        for ch in line.chars() {
            match ch {
                '`' => in_code = !in_code,
                '{' | '}' if !in_code => out.push('\\'),
                _ => {}
            }
            out.push(ch);
        }
    }
    out
}

#[test]
fn braces_outside_code_are_escaped_for_mdx() {
    let text = "placeholders {browser}, {{ and }}\n`{kept}` and\n```\n{fenced}\n```\n";
    assert_eq!(
        escape_mdx_braces(text),
        "placeholders \\{browser\\}, \\{\\{ and \\}\\}\n`{kept}` and\n```\n{fenced}\n```\n"
    );
}

#[test]
fn angle_brackets_outside_code_are_escaped() {
    assert_eq!(
        prose("open <URL> with `<URL>`"),
        "open &lt;URL> with `<URL>`"
    );
}

#[test]
fn a_command_heading_links_from_the_table() {
    assert_eq!(anchor("sbrs plugin list"), "sbrs-plugin-list");
}

/// The pieces the page renders: a global flag, and a command with two plain examples and
/// one that has a header and a text of its own.
const SAMPLE: &str = r#"name sbrs
bin sbrs
flag --headless help="Run the browser in headless mode" global=#true
cmd open help="Open a URL." {
    arg <URL>
    example "sbrs open https://a.test"
    example "sbrs open https://b.test"
    example "sbrs --headless open https://c.test" header="Headless" help="Without a window."
}
"#;

#[test]
fn the_plain_examples_of_a_command_are_one_bash_block_titled_examples() {
    let spec: Spec = SAMPLE.parse().expect("the sample spec parses");
    let page = render(&spec);
    assert!(
        page.contains(
            "```bash Examples\nsbrs open https://a.test\nsbrs open https://b.test\n```\n"
        ),
        "{page}"
    );
    assert!(
        page.contains(
            "**Headless**\n\nWithout a window.\n\n```bash Example\nsbrs --headless open https://c.test\n```\n"
        ),
        "{page}"
    );
}

#[test]
fn the_global_flags_go_before_or_after_the_command() {
    let spec: Spec = SAMPLE.parse().expect("the sample spec parses");
    let page = render(&spec);
    assert!(page.contains("## Global flags"), "{page}");
    assert!(
        page.contains("goes before the command or after it"),
        "{page}"
    );
    assert!(!page.contains("They go before the command"), "{page}");
}
