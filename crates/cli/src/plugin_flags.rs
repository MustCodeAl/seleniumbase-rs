//! The `--plugin` flag of `sbrs`, in a struct of its own.
//!
//! A completer is a function of the partial parse of its own command, which exists only for an
//! `Args` struct, not for the root `Cli`. So the flag lives here and the root flattens it in
//! (the way `RecorderFlags` is), and a Tab at `sbrs --plugin ` or `sbrs open URL --plugin ` offers
//! the built-in plugins of the binary ([`crate::runtime_values::plugin_candidates`]).

/// The flag that turns a built-in plugin on.
#[derive(Debug, Default, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct PluginFlags {
    /// Turn on a built-in plugin in the browsers the command starts (SB_PLUGINS; repeatable)
    ///
    /// Adds to the plugins the settings file (the [plugins] table) and SB_PLUGINS turn on.
    /// Precedence, highest first: the command line, then SB_PLUGINS (which replaces the names
    /// the file enables), then the settings file. `sbrs plugin list` shows the plugins this
    /// build has. `sbrs test` passes the names on to the tests as SB_PLUGINS.
    #[allow(rustdoc::broken_intra_doc_links)] // `[plugins]` is a TOML table in help text, not a link
    #[usage(long, global, value_name = "NAME", complete = crate::runtime_values::plugin_candidates)]
    pub(crate) plugin: Vec<String>,
}
