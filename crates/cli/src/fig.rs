//! The Fig completion spec of `sbrs completions fig`, written from the usage spec.
//!
//! usage generates shell completions, the spec and its documents from one declaration,
//! but not Fig specs (the format of the withfig/autocomplete repository that Warp's
//! built-in completions, inshellisense and Amazon Q read). This walks the tables the
//! parser and the help page read, so the Fig spec lists exactly the commands, flags,
//! aliases and choices of the command line.

use std::fmt::Write as _;

use usage::spec::{ArgMeta, CommandMeta, FlagMeta, Spec};

/// The Fig spec, a TypeScript module, of the program `spec` describes.
pub fn spec(spec: &Spec<'_>) -> String {
    let mut out = String::from("const completionSpec: Fig.Spec = {\n");
    let name = spec.bin.unwrap_or(spec.name);
    push_line(&mut out, 1, &format!("name: {},", quote(name)));
    if let Some(about) = spec.about.or(spec.root.about) {
        push_line(
            &mut out,
            1,
            &format!("description: {},", quote(first_line(about))),
        );
    }
    push_command_body(&mut out, spec.root, 1, true);
    out.push_str("};\n\nexport default completionSpec;\n");
    out
}

/// The `subcommands`, `options` and `args` of `command`, at `depth` levels in. The root
/// also lists the `help` command and the `--version` flag the parser answers to.
fn push_command_body(out: &mut String, command: &CommandMeta<'_>, depth: usize, root: bool) {
    let subcommands: Vec<_> = command
        .subcommands
        .iter()
        .filter(|subcommand| !subcommand.hide)
        .collect();
    if !subcommands.is_empty() {
        push_line(out, depth, "subcommands: [");
        for subcommand in subcommands {
            push_line(out, depth + 1, "{");
            push_line(
                out,
                depth + 2,
                &format!(
                    "name: {},",
                    names(subcommand.cmd.name, subcommand.cmd.aliases)
                ),
            );
            if let Some(about) = subcommand.about {
                push_line(
                    out,
                    depth + 2,
                    &format!("description: {},", quote(first_line(about))),
                );
            }
            push_command_body(out, subcommand, depth + 2, false);
            push_line(out, depth + 1, "},");
        }
        if root {
            push_line(out, depth + 1, "{");
            push_line(out, depth + 2, "name: \"help\",");
            push_line(
                out,
                depth + 2,
                "description: \"Print this message or the help of the given subcommand(s)\",",
            );
            push_line(
                out,
                depth + 2,
                "args: { name: \"subcommand\", isOptional: true, isVariadic: true },",
            );
            push_line(out, depth + 1, "},");
        }
        push_line(out, depth, "],");
    }

    push_line(out, depth, "options: [");
    for flag in command.flags.iter().filter(|flag| !flag.hide) {
        push_flag(out, flag, depth + 1);
    }
    push_builtin(out, &["-h", "--help"], "Print help", depth + 1);
    if root {
        push_builtin(out, &["-V", "--version"], "Print version", depth + 1);
    }
    push_line(out, depth, "],");

    let args: Vec<_> = command.args.iter().filter(|arg| !arg.hide).collect();
    if !args.is_empty() {
        push_line(out, depth, "args: [");
        for arg in args {
            push_arg(out, arg, depth + 1);
        }
        push_line(out, depth, "],");
    }
}

/// One option: its spellings, what it does and the value it takes.
fn push_flag(out: &mut String, flag: &FlagMeta<'_>, depth: usize) {
    push_line(out, depth, "{");
    let mut spellings: Vec<String> = flag
        .flag
        .shorts
        .iter()
        .map(|short| format!("-{}", char::from(*short)))
        .collect();
    spellings.extend(flag.flag.longs.iter().map(|long| format!("--{long}")));
    let spellings = match spellings.as_slice() {
        [one] => quote(one),
        many => list(many),
    };
    push_line(out, depth + 1, &format!("name: {spellings},"));
    if let Some(help) = flag.help {
        push_line(
            out,
            depth + 1,
            &format!("description: {},", quote(first_line(help))),
        );
    }
    if flag.repeatable {
        push_line(out, depth + 1, "isRepeatable: true,");
    }
    if flag.flag.global {
        // Declared once, on the root, and valid below every command.
        push_line(out, depth + 1, "isPersistent: true,");
    }

    if flag.required {
        push_line(out, depth + 1, "isRequired: true,");
    }
    if flag.flag.takes_value {
        let value = flag
            .value_name
            .or_else(|| flag.flag.longs.first().copied())
            .unwrap_or("value");
        push_line(out, depth + 1, "args: {");
        push_line(
            out,
            depth + 2,
            &format!("name: {},", quote(&value.to_lowercase())),
        );
        if flag.value_optional {
            push_line(out, depth + 2, "isOptional: true,");
        }
        push_template(out, flag.complete_type(), depth + 2);
        push_suggestions(out, flag.choices, depth + 2);
        push_line(out, depth + 1, "},");
    }
    push_line(out, depth, "},");
}

/// A flag the parser answers to without it being declared.
fn push_builtin(out: &mut String, spellings: &[&str], description: &str, depth: usize) {
    push_line(out, depth, "{");
    push_line(out, depth + 1, &format!("name: {},", list(spellings)));
    push_line(
        out,
        depth + 1,
        &format!("description: {},", quote(description)),
    );
    push_line(out, depth, "},");
}

/// One positional argument.
fn push_arg(out: &mut String, arg: &ArgMeta<'_>, depth: usize) {
    push_line(out, depth, "{");
    push_line(
        out,
        depth + 1,
        &format!("name: {},", quote(&arg.arg.name.to_lowercase())),
    );
    if let Some(help) = arg.help {
        push_line(
            out,
            depth + 1,
            &format!("description: {},", quote(first_line(help))),
        );
    }
    if !arg.required {
        push_line(out, depth + 1, "isOptional: true,");
    }
    if arg.arg.var {
        push_line(out, depth + 1, "isVariadic: true,");
    }
    push_template(out, arg.complete_type, depth + 1);
    push_suggestions(out, arg.choices, depth + 1);
    push_line(out, depth, "},");
}

/// Fig completes file names and folders from a template; usage calls them `path` (with the
/// extensions it is limited to after a colon, `path:json,yaml`) and `dir`.
fn push_template(out: &mut String, complete_type: Option<&str>, depth: usize) {
    let template = match complete_type {
        Some(kind) if kind == "path" || kind.starts_with("path:") => "filepaths",
        Some("dir") => "folders",
        _ => return,
    };
    push_line(out, depth, &format!("template: {},", quote(template)));
}

/// The values a flag or argument accepts, as Fig suggestions.
fn push_suggestions(out: &mut String, choices: &[&str], depth: usize) {
    if !choices.is_empty() {
        push_line(out, depth, &format!("suggestions: {},", list(choices)));
    }
}

/// `name` alone, or `name` and its `aliases` as a list.
fn names(name: &str, aliases: &[&str]) -> String {
    if aliases.is_empty() {
        quote(name)
    } else {
        let all: Vec<&str> = std::iter::once(name)
            .chain(aliases.iter().copied())
            .collect();
        list(&all)
    }
}

/// `items` as a TypeScript array of strings.
fn list<S: AsRef<str>>(items: &[S]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| quote(item.as_ref())).collect();
    format!("[{}]", quoted.join(", "))
}

/// `text` as a string literal, which is a valid TypeScript one.
fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("a string is always valid JSON")
}

/// The first line of a description: Fig shows one line.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

fn push_line(out: &mut String, depth: usize, line: &str) {
    // Writing to a `String` cannot fail.
    let _ = writeln!(out, "{:indent$}{line}", "", indent = depth * 2);
}

#[cfg(test)]
mod tests {
    use super::{first_line, list, names, quote};

    /// The first lines of the option or argument that starts with `name:` in the spec of the CLI.
    fn entry(spec: &str, name: &str) -> String {
        let start = spec
            .find(&format!("name: {name},"))
            .unwrap_or_else(|| panic!("{name} is listed"));
        spec[start..].lines().take(8).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn a_global_option_is_persistent_and_a_local_one_is_not() {
        let spec = super::spec(crate::Cli::spec());
        assert!(entry(&spec, "\"--headless\"").contains("isPersistent: true,"));
        assert!(entry(&spec, "[\"-c\", \"--config\"]").contains("isPersistent: true,"));
        assert!(!entry(&spec, "\"--title-contains\"").contains("isPersistent"));
    }

    #[test]
    fn a_path_limited_to_extensions_keeps_its_file_template() {
        let spec = super::spec(crate::Cli::spec());
        // `--config` takes `path:toml,json` in the usage spec; Fig has no extension filter.
        assert!(entry(&spec, "[\"-c\", \"--config\"]").contains("template: \"filepaths\""));
        assert!(entry(&spec, "\"--user-data-dir\"").contains("template: \"folders\""));
    }

    #[test]
    fn strings_are_quoted_for_typescript() {
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(list(&["a", "b"]), r#"["a", "b"]"#);
    }

    #[test]
    fn a_name_with_aliases_becomes_a_list() {
        assert_eq!(names("get", &[]), r#""get""#);
        assert_eq!(names("install", &["get"]), r#"["install", "get"]"#);
    }

    #[test]
    fn a_description_is_one_line() {
        assert_eq!(first_line("first\nsecond"), "first");
        assert_eq!(first_line(""), "");
    }
}
