//! The tasks of the built-in plugins as commands of `sbrs plugin`, which no declaration holds.
//!
//! Which plugins offer a task depends on the build, so `sbrs plugin crawler` is not declared in
//! `sbrs.rs`: the declaration of `plugin` ends in a catch-all (`PluginCommand::External`), and a
//! `usage-dynamic` [`Catalog`] gives the words it captures their meaning when they are met, one
//! command for each plugin of this build that has a task. The catalog adds those commands to the
//! help page of `plugin`, to the completion of `sbrs plugin <Tab>` and to the parse of the words
//! after the name; the usage spec, the reference page and the man page, which come from the
//! declarations, never hold them.
//!
//! `sbrs plugin NAME ...` is `sbrs plugin run NAME ...`: once the catalog has accepted the words
//! ([`dispatch`] answers [`Dispatch::Run`]), the caller parses the command line again with `run`
//! in front of the name, so the task is started by the one code path that `plugin run` has.
//!
//! A build whose plugins offer no task has an empty catalog, and then nothing here is used: the
//! parser's own answers apply.

use std::ffi::OsString;

use usage::complete::{CompletionRequest, render};
use usage_dynamic::{Catalog, Outcome, Spec, SpecArgBuilder, SpecCommandBuilder, SpecFlagBuilder};

use crate::Cli;
use crate::runtime_values;

/// The command the catalog's commands go under: `sbrs plugin`.
pub(crate) const PARENT: &str = "plugin";

/// What the catalog made of the words after `sbrs plugin`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Dispatch {
    /// The words are the options of a plugin's task: run `sbrs plugin run` with them.
    Run,
    /// The words asked for a page (`--help`), which is to be printed on standard output.
    Page(String),
    /// The words are no command of the plugin, or are refused by it: the message of the error.
    Refused(String),
}

/// The spec of the task of the plugin `name`, with its summary as help.
///
/// Every task takes the same two options, the ones of `sbrs plugin run`.
fn task_spec(name: &str, summary: &str) -> Spec {
    let option = |long: &str, help: &str| {
        SpecFlagBuilder::new()
            .name(long)
            .long(long)
            .arg(SpecArgBuilder::new().name("FILE").build())
            .help(help)
            .build()
    };
    SpecCommandBuilder::new()
        .name(name)
        .help(format!(
            "{summary}. The same as `sbrs plugin run {name}`; the global options go before `plugin`"
        ))
        .flag(option(
            "input",
            "A JSON file with the task's input, or `-` to read it from standard input",
        ))
        .flag(option(
            "out",
            "Write the results to this file, one JSON value per line (default: standard output)",
        ))
        .build()
        .into()
}

/// The catalog of this build: one command under `plugin` for each plugin that offers a task, or
/// `None` when none does (or the catalog cannot be built, and the parser's own answers apply).
pub(crate) fn catalog() -> Option<Catalog<'static>> {
    let tasks = runtime_values::plugin_summaries(true);
    if tasks.is_empty() {
        return None;
    }
    let mut builder = Catalog::builder(Cli::app());
    for (name, summary) in &tasks {
        builder = builder.under(PARENT, task_spec(name, summary.trim_end_matches('.')));
    }
    builder.build().ok()
}

/// What the catalog makes of `captured`, the words `PluginCommand::External` took: the name of
/// a command of the catalog and what follows it.
pub(crate) fn dispatch(captured: &[OsString]) -> Dispatch {
    let name = captured
        .first()
        .map(|word| word.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unknown = || Dispatch::Refused(format!("unrecognized subcommand '{name}'"));
    let Some(catalog) = catalog() else {
        return unknown();
    };
    match catalog.parse_external(PARENT, captured) {
        Ok(Some(Outcome::Parsed(_))) => Dispatch::Run,
        Ok(Some(Outcome::Help(help))) => Dispatch::Page(help.page),
        Ok(Some(_) | None) => unknown(),
        Err(error) => Dispatch::Refused(error.to_string()),
    }
}

/// The help page of the command `path` (the words after the program name, `plugin` for the
/// command that holds the catalog), with the commands of the catalog on it. `None` when there is
/// no catalog.
pub(crate) fn help_page(path: &str, long: bool) -> Option<String> {
    catalog()?.app().ok()?.help(path, long)
}

/// The answer to the completion request in `argv` (the words after the program name), with the
/// commands of the catalog among the candidates, or `None` when `argv` is no request or the build
/// has no catalog.
///
/// The answer is made by a task on a runtime of its own, so this is called before the main
/// runtime starts.
pub(crate) fn completion(argv: &[OsString]) -> Option<String> {
    let request = CompletionRequest::parse(argv)?;
    let catalog = catalog()?;
    let app = catalog.app().ok()?;
    let runtime = tokio::runtime::Builder::new_current_thread().build().ok()?;
    let answer = runtime.block_on(app.complete_request(&request));
    Some(render(&answer, request.shell))
}

#[cfg(all(test, feature = "plugins-builtin"))]
mod tests {
    use super::*;

    /// The words after `sbrs plugin`, as `PluginCommand::External` captures them.
    fn captured(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_catalog_holds_the_plugins_with_a_task() {
        let catalog = catalog().expect("the default build has plugins with a task");
        let page = catalog
            .app()
            .expect("the catalog has an app")
            .help(PARENT, false)
            .expect("the page of plugin");
        for name in ["fetcher", "extractor", "batcher", "crawler"] {
            assert!(page.contains(name), "{name} in:\n{page}");
        }
        // The commands the declarations hold stay on the page; a plugin with no task is no command.
        for name in ["list", "info", "run"] {
            assert!(page.contains(name), "{name} in:\n{page}");
        }
        assert!(!page.contains("shield"), "{page}");
    }

    #[test]
    fn the_options_of_a_task_make_plugin_run() {
        let words = captured(&["crawler", "--input", "crawl.json", "--out", "pages.jsonl"]);
        assert_eq!(dispatch(&words), Dispatch::Run);
        assert_eq!(dispatch(&captured(&["crawler"])), Dispatch::Run);
    }

    #[test]
    fn a_request_for_help_is_a_page_with_the_summary_and_the_options() {
        let Dispatch::Page(page) = dispatch(&captured(&["crawler", "--help"])) else {
            panic!("--help is a page");
        };
        for expected in ["--input", "--out", "sbrs plugin run crawler"] {
            assert!(page.contains(expected), "{expected:?} in:\n{page}");
        }
    }

    #[test]
    fn an_option_the_task_does_not_have_is_refused() {
        let Dispatch::Refused(message) = dispatch(&captured(&["crawler", "--nosuch"])) else {
            panic!("--nosuch is refused");
        };
        assert!(message.contains("--nosuch"), "{message}");
    }

    #[test]
    fn a_plugin_without_a_task_or_no_plugin_is_an_unknown_command() {
        for name in ["shield", "no-such-plugin"] {
            assert_eq!(
                dispatch(&captured(&[name])),
                Dispatch::Refused(format!("unrecognized subcommand '{name}'"))
            );
        }
    }

    #[test]
    fn a_completion_request_lists_the_tasks_after_plugin() {
        let argv = captured(&[
            crate::failure::COMPLETE_WORD,
            "--shell",
            "bash",
            "--line",
            "sbrs plugin ",
        ]);
        let answer = completion(&argv).expect("a request is answered");
        for name in ["list", "info", "run", "crawler"] {
            assert!(
                answer.lines().any(|line| line == name),
                "{name} in:\n{answer}"
            );
        }
        assert_eq!(
            completion(&captured(&["open", "https://example.com"])),
            None
        );
    }
}
