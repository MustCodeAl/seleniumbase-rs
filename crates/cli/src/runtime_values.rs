//! The words only the running program knows, offered at a value position.
//!
//! Every completion script asks `sbrs` for the candidates each time Tab is pressed (the hidden
//! `sbrs __complete_word__` request), so a candidate can be computed when it is asked for. The
//! static spec (`completions/sbrs.usage.kdl`) holds what is fixed in the source: the commands,
//! the flags, a closed set of `choices`. What depends on the build or the machine is not in it:
//! which built-in plugins this build has, which of them offer a task, which fingerprint presets
//! the library ships. A completer is a plain function named in the declaration
//! (`#[usage(complete = plugin_candidates)]`); the answer is asked when the word is typed and
//! never written into the spec, the reference page or the man page.

use usage::complete::{Candidate, CompleteCtx};

/// What each built-in plugin of this build is called and what it does, for the words a completer
/// offers and for the commands of the plugin catalog (`plugin_catalog.rs`); only the plugins
/// that offer a task when `only_with_task` is set.
#[cfg(feature = "plugins")]
pub(crate) fn plugin_summaries(only_with_task: bool) -> Vec<(String, String)> {
    let registry = seleniumbase_rs::plugin_host::builtin_registry();
    let with_task = seleniumbase_rs::cli::plugin_cmd::task_names_with(&registry);
    registry
        .names()
        .into_iter()
        .filter_map(|name| registry.get(name))
        .filter(|descriptor| !only_with_task || with_task.iter().any(|n| n == descriptor.name()))
        .map(|descriptor| {
            (
                descriptor.name().to_owned(),
                descriptor.metadata().summary.to_string(),
            )
        })
        .collect()
}

/// A build without the plugin API has no plugin to name.
#[cfg(not(feature = "plugins"))]
pub(crate) fn plugin_summaries(_only_with_task: bool) -> Vec<(String, String)> {
    Vec::new()
}

fn described(words: Vec<(String, String)>) -> Vec<Candidate<'static>> {
    words
        .into_iter()
        .map(|(word, about)| {
            if about.is_empty() {
                Candidate::new(word)
            } else {
                Candidate::described(word, about)
            }
        })
        .collect()
}

/// The completer of `--plugin NAME` and `sbrs plugin info NAME`: the built-in plugins of this
/// build, each with its summary.
pub(crate) fn plugin_candidates<P>(
    _partial: &P,
    _ctx: &CompleteCtx<'_>,
) -> Vec<Candidate<'static>> {
    described(plugin_summaries(false))
}

/// The completer of `sbrs plugin run NAME`: the plugins of this build that offer a task.
pub(crate) fn task_plugin_candidates<P>(
    _partial: &P,
    _ctx: &CompleteCtx<'_>,
) -> Vec<Candidate<'static>> {
    described(plugin_summaries(true))
}

/// The completer of `sbrs fingerprint export --preset NAME`: the presets the library ships.
#[allow(dead_code)]
pub(crate) fn preset_candidates<P>(
    _partial: &P,
    _ctx: &CompleteCtx<'_>,
) -> Vec<Candidate<'static>> {
    ["windows", "edge", "macos", "linux", "android", "ios"]
        .iter()
        .map(|name| Candidate::new(*name))
        .collect()
}
