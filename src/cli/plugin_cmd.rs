//! `sbrs plugin list` and `sbrs plugin info NAME`, and the check of a plugin
//! selection that `sbrs` and `sbrs-mcp` make before they start a browser.
//!
//! The functions return the text to print and the error to report, so the
//! binaries only print them and the logic is tested without a process. A
//! mistake in what the person typed (an unknown plugin, an option the plugin
//! refuses, an option of a plugin nothing turns on) is
//! [`SeleniumBaseError::InvalidConfig`], which `sbrs` reports with exit status
//! 2, as it does for every other value that does not parse.
//!
//! Without the feature `plugins` there is nothing to list: [`list`] says so,
//! [`info`] fails, and [`check_selection`] refuses any selection.
//!
//! [`PluginChoice`] is the whole choice a program makes: the `[plugins]` table of the
//! settings file, `SB_PLUGINS` and the command-line flags, with the place each plugin was
//! named kept, so that the program can refuse what a project file may not turn on alone
//! and tell the operator what is active.

#[cfg(feature = "plugins")]
use std::fmt::Write as _;

#[cfg(feature = "plugins")]
use seleniumbase_rs_plugin::{
    CapabilitySet, Category, Descriptor, Enforcement, Metadata, Plugin, PluginSpec, Registry,
    Requirements, Seals,
};
#[cfg(feature = "plugins")]
use serde_json::Value;

use std::path::Path;

use crate::browser::config::BrowserConfig;
use crate::config::plugins::{PluginSelection, PLUGINS_ENV};
use crate::config::settings::Settings;
use crate::error::SeleniumBaseError;

/// What a build without the built-in plugins says, in place of a list.
const NO_PLUGINS: &str = "this build has no built-in plugin; rebuild with \
                          --features plugins-builtin (or one of plugin-shield, \
                          plugin-block-resources, plugin-crx)";

/// Checks `selection` the way a command line program should before it starts a
/// browser: this build can run plugins, every name is one it has, and the
/// plugins accept their options.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] naming the mistake. For an unknown
/// name it lists the known ones.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::plugin_cmd::check_selection;
/// use seleniumbase_rs::config::plugins::PluginSelection;
///
/// check_selection(&PluginSelection::new())?;
///
/// let error = check_selection(&PluginSelection::new().enable("no-such-plugin")).unwrap_err();
/// assert_eq!(error.exit_code(), 2);
/// assert!(error.to_string().contains("no-such-plugin"));
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn check_selection(selection: &PluginSelection) -> Result<(), SeleniumBaseError> {
    selection.ensure_supported()?;
    #[cfg(feature = "plugins")]
    check_selection_with(&crate::plugin_host::builtin_registry(), selection)?;
    Ok(())
}

/// Where the name of a plugin was given.
///
/// The precedence is the command line over `SB_PLUGINS` over the settings file; the file is
/// told apart by how it was found, because a file the operator names with `--config` is
/// theirs, and the `sbase_config.toml` of the current folder may be a cloned project's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginSource {
    /// The `[plugins]` table of the settings file found in the current folder.
    ProjectFile,
    /// The `[plugins]` table of the file that `--config` names.
    ConfigFile,
    /// The `SB_PLUGINS` variable.
    Environment,
    /// A `--plugin` flag.
    CommandLine,
}

impl PluginSource {
    /// The source in words, as the launch log and the errors say it.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::ProjectFile => "the settings file in the current folder",
            Self::ConfigFile => "the settings file named by --config",
            Self::Environment => "SB_PLUGINS",
            Self::CommandLine => "--plugin",
        }
    }
}

/// An enabled plugin and where its name was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOrigin {
    /// The plugin's name.
    pub name: String,
    /// Where the name came from. The strongest place wins: the command line, then
    /// `SB_PLUGINS`, then the file.
    pub source: PluginSource,
}

/// The plugins an operator chose for a program: what the settings file, `SB_PLUGINS` and the
/// command line turn on and configure, and where each name came from.
///
/// `sbrs`, `sbrs devtools` and `sbrs-mcp` build one so that the precedence is the same for
/// all of them. [`check`](Self::check) refuses a mistake, and a plugin with a power that a
/// project file may not grant on its own; [`log`](Self::log) says what is active.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// use seleniumbase_rs::cli::plugin_cmd::{PluginChoice, PluginSource};
///
/// // The settings file of the current folder, SB_PLUGINS, then `--plugin shield`.
/// let choice = PluginChoice::load(
///     None::<&Path>,
///     &["shield"],
///     &[r#"shield.level="strict""#],
/// )?;
/// assert_eq!(choice.selection().enabled(), ["shield"]);
/// assert_eq!(choice.origins()[0].source, PluginSource::CommandLine);
/// choice.check()?;
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PluginChoice {
    selection: PluginSelection,
    origins: Vec<PluginOrigin>,
}

impl PluginChoice {
    /// Reads the `[plugins]` table of the settings file `config` (the settings file of the
    /// current folder when `None`), replaces the names with `SB_PLUGINS` when it is set, and
    /// applies `--plugin` (`plugins`) and `--plugin-opt` (`options`) on top.
    ///
    /// Only the plugin part of the file is read, so a value that is wrong for a browser does
    /// not stop a program that starts none. The options of the file stay for a plugin the
    /// flags turn on again, even when `SB_PLUGINS` does not name it; those of plugins
    /// nothing turns on are dropped after the flags, as `SB_PLUGINS` has always dropped them.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] for a file that cannot be read, and for a flag
    /// that is not a plugin name or `NAME.KEY=VALUE`, or sets an option of a plugin that
    /// nothing turns on.
    pub fn load<P, N, O>(
        config: Option<P>,
        plugins: &[N],
        options: &[O],
    ) -> Result<Self, SeleniumBaseError>
    where
        P: AsRef<Path>,
        N: AsRef<str>,
        O: AsRef<str>,
    {
        let file_source = if config.is_some() {
            PluginSource::ConfigFile
        } else {
            PluginSource::ProjectFile
        };
        let table = Settings::load_plugin_table(config)?;
        Self::from_parts(
            table,
            file_source,
            plugins,
            options,
            std::env::var(PLUGINS_ENV).ok(),
        )
    }

    /// [`load`](Self::load) from the `[plugins]` table that was read (`table`, found as
    /// `file_source` says) and the value of `SB_PLUGINS`, when it is set.
    fn from_parts<N, O>(
        table: PluginSelection,
        file_source: PluginSource,
        plugins: &[N],
        options: &[O],
        list: Option<String>,
    ) -> Result<Self, SeleniumBaseError>
    where
        N: AsRef<str>,
        O: AsRef<str>,
    {
        let mut selection = table;
        // An empty variable is an answer, not an unset one: it turns the file's plugins off.
        let variable = list.as_deref().map(PluginSelection::from_list);
        if let Some(list) = &list {
            selection.set_enabled_keeping_options(list);
        }
        selection.apply_flags(plugins, options)?;
        if variable.is_some() {
            selection.prune_unused();
        }

        let origins = selection
            .enabled()
            .iter()
            .map(|name| {
                let source = if plugins.iter().any(|flag| flag.as_ref() == name) {
                    PluginSource::CommandLine
                } else if variable
                    .as_ref()
                    .is_some_and(|named| named.enabled().contains(name))
                {
                    PluginSource::Environment
                } else {
                    file_source
                };
                PluginOrigin {
                    name: name.clone(),
                    source,
                }
            })
            .collect();
        Ok(Self { selection, origins })
    }

    /// The plugins to turn on and their options.
    #[must_use]
    pub fn selection(&self) -> &PluginSelection {
        &self.selection
    }

    /// The selection, to hand to a builder.
    #[must_use]
    pub fn into_selection(self) -> PluginSelection {
        self.selection
    }

    /// The enabled plugins in order, each with where its name was given.
    #[must_use]
    pub fn origins(&self) -> &[PluginOrigin] {
        &self.origins
    }

    /// Checks the choice the way a program should before it starts a browser:
    /// [`check_selection`], and a plugin that declares a power the settings file of the
    /// current folder may not grant alone (`launch.proxy`, `host.*`, `artifacts.write`) has
    /// to be named by the operator too, with `--plugin` or in `SB_PLUGINS`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] naming the mistake.
    pub fn check(&self) -> Result<(), SeleniumBaseError> {
        check_selection(&self.selection)?;
        #[cfg(feature = "plugins")]
        check_origins_with(&crate::plugin_host::builtin_registry(), &self.origins)?;
        Ok(())
    }

    /// Logs one `INFO` line for each enabled plugin with the place that enabled it, so that
    /// the operator can see what a file in the folder turned on.
    pub fn log(&self) {
        for origin in &self.origins {
            tracing::info!(
                plugin = %origin.name,
                source = origin.source.describe(),
                "plugin enabled"
            );
        }
    }
}

/// Gives `config` the plugins `selection` names. The same as
/// [`BrowserConfig::with_plugin_selection`], except that a build without the feature
/// `plugins` refuses a selection that is not empty instead of leaving it out of the
/// program, whichever features the caller has.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when this build cannot run plugins, and
/// [`SeleniumBaseError::Plugin`] for an unknown name or options a plugin refuses.
pub fn apply_to_config(
    config: BrowserConfig,
    selection: &PluginSelection,
) -> Result<BrowserConfig, SeleniumBaseError> {
    #[cfg(feature = "plugins")]
    {
        config.with_plugin_selection(selection)
    }
    #[cfg(not(feature = "plugins"))]
    {
        selection.ensure_supported()?;
        Ok(config)
    }
}

/// What `sbrs plugin list` prints: the built-in plugins of this build, with
/// their category, whether `selection` turns them on, and a summary.
#[must_use]
pub fn list(selection: &PluginSelection) -> String {
    #[cfg(feature = "plugins")]
    {
        list_with(&crate::plugin_host::builtin_registry(), selection)
    }
    #[cfg(not(feature = "plugins"))]
    {
        let _ = selection;
        format!("No plugins: {NO_PLUGINS}.\n")
    }
}

/// What `sbrs plugin info NAME` prints: what the plugin declares about itself
/// and what `selection` does with it.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when this build has no plugin called
/// `name`; the message lists the ones it has.
pub fn info(name: &str, selection: &PluginSelection) -> Result<String, SeleniumBaseError> {
    #[cfg(feature = "plugins")]
    {
        info_with(&crate::plugin_host::builtin_registry(), name, selection)
    }
    #[cfg(not(feature = "plugins"))]
    {
        let _ = selection;
        Err(SeleniumBaseError::invalid_config(format!(
            "no plugin named `{name}`: {NO_PLUGINS}"
        )))
    }
}

/// [`check_selection`] against the plugins of `registry`.
///
/// # Errors
///
/// As [`check_selection`].
#[cfg(feature = "plugins")]
pub fn check_selection_with(
    registry: &Registry,
    selection: &PluginSelection,
) -> Result<(), SeleniumBaseError> {
    if let Some(name) = selection
        .enabled()
        .iter()
        .find(|name| registry.get(name).is_none())
    {
        return Err(unknown(name, registry));
    }
    crate::plugin_host::build_with(registry, selection)
        .map(drop)
        .map_err(as_usage_error)
}

/// Whether a capability is one the operator has to grant: the settings file of a project that
/// is cloned must not give it on its own.
#[cfg(feature = "plugins")]
fn needs_the_operator(capability: &str) -> bool {
    capability == "launch.proxy"
        || capability == "artifacts.write"
        || capability.starts_with("host.")
}

/// The check of [`PluginChoice::check`] that depends on where a name came from, against
/// the plugins of `registry`: a plugin that declares `launch.proxy`, `artifacts.write` or a
/// `host.*` capability, and was named only by the settings file of the current folder, is
/// refused. A name nothing in `registry` has is left to [`check_selection_with`].
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] naming the plugin, the capabilities and how to name
/// the plugin yourself.
#[cfg(feature = "plugins")]
pub fn check_origins_with(
    registry: &Registry,
    origins: &[PluginOrigin],
) -> Result<(), SeleniumBaseError> {
    for origin in origins
        .iter()
        .filter(|origin| origin.source == PluginSource::ProjectFile)
    {
        let Some(descriptor) = registry.get(&origin.name) else {
            continue;
        };
        let meta = descriptor.metadata();
        let powers: Vec<String> = meta
            .capabilities
            .iter()
            .chain(meta.optional.iter())
            .map(ToString::to_string)
            .filter(|capability| needs_the_operator(capability))
            .collect();
        if !powers.is_empty() {
            let name = &origin.name;
            return Err(SeleniumBaseError::invalid_config(format!(
                "plugin `{name}` asks for {}, which {} cannot grant by itself: a project you \
                 clone would decide. Name the plugin yourself, with `--plugin {name}` or \
                 `SB_PLUGINS={name}`",
                powers.join(", "),
                origin.source.describe(),
            )));
        }
    }
    Ok(())
}

/// [`list`] for the plugins of `registry`.
#[cfg(feature = "plugins")]
#[must_use]
pub fn list_with(registry: &Registry, selection: &PluginSelection) -> String {
    let descriptors: Vec<&Descriptor> = registry
        .names()
        .into_iter()
        .filter_map(|name| registry.get(name))
        .collect();
    if descriptors.is_empty() {
        return format!("No plugins: {NO_PLUGINS}.\n");
    }

    let header = ["NAME", "CATEGORY", "ENABLED", "SUMMARY"].map(str::to_owned);
    let mut rows = vec![header];
    for descriptor in &descriptors {
        let meta = descriptor.metadata();
        let enabled = selection
            .enabled()
            .iter()
            .any(|name| name == descriptor.name());
        rows.push([
            descriptor.name().to_owned(),
            category_of(meta),
            if enabled { "yes" } else { "no" }.to_owned(),
            dash_if_empty(&meta.summary),
        ]);
    }
    let width = |column: usize| rows.iter().map(|row| row[column].len()).max().unwrap_or(0);
    let widths = [width(0), width(1), width(2)];

    let mut text = String::new();
    for row in &rows {
        for (cell, width) in row.iter().zip(widths) {
            let _ = write!(text, "{cell:<width$}  ");
        }
        // The last column is not padded: the summary ends the line.
        text.push_str(&row[3]);
        text.push('\n');
    }

    let missing: Vec<&str> = selection
        .enabled()
        .iter()
        .map(String::as_str)
        .filter(|name| registry.get(name).is_none())
        .collect();
    if !missing.is_empty() {
        let _ = write!(
            text,
            "\nPlugins enabled but not in this build: {}\n",
            missing.join(", ")
        );
    }
    text.push_str(
        "\nTurn one on with --plugin NAME, SB_PLUGINS, or `enable` in the [plugins] table of the \
         settings file. `sbrs plugin info NAME` shows what a plugin may do.\n",
    );
    text
}

/// [`info`] for the plugins of `registry`.
///
/// # Errors
///
/// As [`info`].
#[cfg(feature = "plugins")]
pub fn info_with(
    registry: &Registry,
    name: &str,
    selection: &PluginSelection,
) -> Result<String, SeleniumBaseError> {
    let descriptor = registry.get(name).ok_or_else(|| unknown(name, registry))?;
    let meta = descriptor.metadata();
    let name = descriptor.name();
    let enabled = selection.enabled().iter().any(|enabled| enabled == name);

    let mut text = String::new();
    if meta.version.is_empty() {
        let _ = writeln!(text, "{name}");
    } else {
        let _ = writeln!(text, "{name} {}", meta.version);
    }
    if !meta.summary.is_empty() {
        let _ = writeln!(text, "{}", meta.summary);
    }
    text.push('\n');

    let phase = meta
        .phase
        .or_else(|| meta.category.map(Category::default_phase))
        .map_or_else(
            || "-".to_owned(),
            |phase| format!("{phase:?}").to_lowercase(),
        );
    row(&mut text, "category", [category_of(meta)]);
    row(
        &mut text,
        "enabled",
        [if enabled { "yes" } else { "no" }.to_owned()],
    );
    row(&mut text, "phase", [phase]);

    let (plugin, refusal) = build_for_info(registry, name, selection);
    row(
        &mut text,
        "needs",
        [plugin.map_or_else(|| "-".to_owned(), |plugin| needs_of(plugin.requirements()))],
    );

    let capabilities = |set: &CapabilitySet| {
        let names: Vec<String> = set.iter().map(ToString::to_string).collect();
        let width = names.iter().map(String::len).max().unwrap_or(0);
        set.iter()
            .zip(names)
            .map(|(capability, name)| {
                let risk = format!("{:?}", capability.risk()).to_lowercase();
                let enforcement = match capability.enforcement(false) {
                    Enforcement::Enforced => "enforced",
                    _ => "declared only",
                };
                format!("{name:<width$}  {risk:<8}{enforcement}")
            })
            .collect::<Vec<_>>()
    };
    let declared_only = meta
        .capabilities
        .iter()
        .chain(meta.optional.iter())
        .any(|capability| capability.enforcement(false) != Enforcement::Enforced);
    row(&mut text, "capabilities", capabilities(&meta.capabilities));
    row(&mut text, "optional", capabilities(&meta.optional));
    let names = |names: &[std::borrow::Cow<'static, str>]| {
        names.iter().map(ToString::to_string).collect::<Vec<_>>()
    };
    row(&mut text, "requires", names(&meta.requires));
    row(&mut text, "conflicts", names(&meta.conflicts));
    row(&mut text, "runs after", names(&meta.after));
    row(&mut text, "runs before", names(&meta.before));
    let seals: Vec<String> = [
        (Seals::PROXY, "proxy"),
        (Seals::CONTEXT_PROXY, "context-proxy"),
        (Seals::WEBRTC, "webrtc"),
        (Seals::HEADLESS, "headless"),
        (Seals::FULFILL, "fulfill"),
    ]
    .into_iter()
    .filter(|(seal, _)| meta.seals.contains(*seal))
    .map(|(_, label)| label.to_owned())
    .collect();
    row(&mut text, "seals", seals);

    let given = selection.options_of(name);
    let mut lines: Vec<String> = match given {
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| format!("{key} = {value}"))
            .collect(),
        Some(other) => vec![other.to_string()],
        None => Vec::new(),
    };
    if lines.is_empty() {
        lines.push(format!(
            "none set; give them as [plugins.{name}] in the settings file, or on the command line \
             as --plugin-opt '{name}.KEY=VALUE'"
        ));
    }
    if let Some(refusal) = refusal {
        lines.push(if given.is_some() {
            format!("refused by the plugin: {refusal}")
        } else {
            format!("the plugin does not start without options: {refusal}")
        });
    }
    row(&mut text, "options", lines);

    if declared_only {
        text.push_str(
            "\nA capability marked `declared only` is a promise: native Rust plugins are not \
             sandboxed, so the host cannot stop one that goes beyond it.\n",
        );
    }
    Ok(text)
}

/// The error for a plugin name `registry` does not have: the names it does,
/// and where to see them.
#[cfg(feature = "plugins")]
fn unknown(name: &str, registry: &Registry) -> SeleniumBaseError {
    let known = registry.names();
    if known.is_empty() {
        SeleniumBaseError::invalid_config(format!("no plugin named `{name}`: {NO_PLUGINS}"))
    } else {
        SeleniumBaseError::invalid_config(format!(
            "no plugin named `{name}` (known: {}); `sbrs plugin list` shows them",
            known.join(", ")
        ))
    }
}

/// A mistake in what the person typed, which the plugin host reports as a
/// plugin error, as a usage error: exit status 2 and no advice about hooks.
#[cfg(feature = "plugins")]
fn as_usage_error(error: SeleniumBaseError) -> SeleniumBaseError {
    match error {
        SeleniumBaseError::Plugin { plugin, reason } if plugin.is_empty() => {
            SeleniumBaseError::invalid_config(reason)
        }
        SeleniumBaseError::Plugin { plugin, reason } => {
            SeleniumBaseError::invalid_config(format!("plugin `{plugin}`: {reason}"))
        }
        other => other,
    }
}

/// The plugin `name` built with the options `selection` gives it, and, when it
/// refuses them, what it says about them and the plugin built with its
/// defaults instead.
#[cfg(feature = "plugins")]
fn build_for_info(
    registry: &Registry,
    name: &str,
    selection: &PluginSelection,
) -> (Option<std::sync::Arc<dyn Plugin>>, Option<String>) {
    let spec = match selection.options_of(name) {
        Some(options) => PluginSpec::new(name).options(options.clone()),
        None => PluginSpec::new(name),
    };
    match registry.build(&spec) {
        Ok(plugin) => (Some(plugin), None),
        Err(error) => (
            registry.build(&PluginSpec::new(name)).ok(),
            Some(error.to_string()),
        ),
    }
}

/// The conditions in `requirements` in words, or a dash.
#[cfg(feature = "plugins")]
fn needs_of(requirements: Requirements) -> String {
    let needs: Vec<&str> = [
        (Requirements::CHROMIUM_ONLY, "a Chromium-based browser"),
        (
            Requirements::LAUNCH_ONLY,
            "a browser it starts itself, not one it attaches to",
        ),
        (Requirements::HEADFUL, "a window, not headless"),
        (
            Requirements::RUN_LAST,
            "to run after the plugins without this flag",
        ),
    ]
    .into_iter()
    .filter(|(flag, _)| requirements.contains(*flag))
    .map(|(_, words)| words)
    .collect();
    if needs.is_empty() {
        "-".to_owned()
    } else {
        needs.join("; ")
    }
}

/// The category in the words the settings use (`privacy`), or a dash for a
/// plugin that declares none.
#[cfg(feature = "plugins")]
fn category_of(meta: &Metadata) -> String {
    meta.category
        .and_then(|category| serde_json::to_value(category).ok())
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "-".to_owned())
}

/// `text`, or a dash when it is empty.
#[cfg(feature = "plugins")]
fn dash_if_empty(text: &str) -> String {
    let shown = if text.is_empty() { "-" } else { text };
    shown.to_owned()
}

/// Writes `label` and its `lines`: the first on the label's line, the others
/// indented under it. No line at all is a dash.
#[cfg(feature = "plugins")]
fn row(text: &mut String, label: &str, lines: impl IntoIterator<Item = String>) {
    let mut lines = lines.into_iter();
    let first = lines.next().unwrap_or_else(|| "-".to_owned());
    let _ = writeln!(text, "{label:<14}{first}");
    for line in lines {
        let _ = writeln!(text, "{:14}{line}", "");
    }
}

// ----------------------------------------------------------------------
// `sbrs plugin run NAME`
// ----------------------------------------------------------------------

/// The names of the plugins of `registry` that offer a task, in registry order.
///
/// A plugin that refuses its default options is not listed, because its task cannot be seen
/// without them.
#[cfg(feature = "plugins")]
#[must_use]
pub fn task_names_with(registry: &Registry) -> Vec<String> {
    registry
        .names()
        .into_iter()
        .filter(|name| {
            registry
                .build(&PluginSpec::new(*name))
                .is_ok_and(|plugin| plugin.task().is_some())
        })
        .map(|name| name.to_string())
        .collect()
}

/// The error for a name that is not a plugin with a task: what is wrong, and which plugins do
/// offer one.
#[cfg(feature = "plugins")]
fn no_task(name: &str, registry: &Registry) -> SeleniumBaseError {
    let with_task = task_names_with(registry);
    let known = if with_task.is_empty() {
        "no plugin of this build offers a task".to_owned()
    } else {
        format!("plugins with a task: {}", with_task.join(", "))
    };
    let problem = if registry.get(name).is_none() {
        format!("no plugin named `{name}`")
    } else {
        format!("plugin `{name}` offers no task")
    };
    SeleniumBaseError::invalid_config(format!(
        "{problem} ({known}); `sbrs plugin list` shows the plugins"
    ))
}

/// The launch options for `sbrs plugin run NAME`: the browser `settings` describe, with the
/// plugins `settings` selects and the plugin `name` on top (enabled implicitly), all built from
/// the built-in registry.
///
/// Everything that can be wrong with the name, the options and the capabilities is found here,
/// before a browser starts.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when no built-in plugin called `name` offers a task (the
/// message lists the ones that do), when a plugin or its options are refused, and what
/// [`Settings::to_launch_options`] returns.
#[cfg(feature = "plugins")]
pub fn task_launch_options(
    settings: Settings,
    name: &str,
) -> Result<crate::sb_cdp::LaunchOptions, SeleniumBaseError> {
    task_launch_options_with(&crate::plugin_host::builtin_registry(), settings, name)
}

/// [`task_launch_options`] against the plugins of `registry`.
///
/// # Errors
///
/// As [`task_launch_options`].
#[cfg(feature = "plugins")]
pub fn task_launch_options_with(
    registry: &Registry,
    settings: Settings,
    name: &str,
) -> Result<crate::sb_cdp::LaunchOptions, SeleniumBaseError> {
    if registry.get(name).is_none() {
        return Err(no_task(name, registry));
    }
    let mut selection = settings.plugins.clone();
    selection.enable_once(name);
    let set = crate::plugin_host::build_with(registry, &selection).map_err(as_usage_error)?;
    if !set.task_names().contains(&name) {
        return Err(no_task(name, registry));
    }
    crate::plugin_host::check_task(&set, name).map_err(as_usage_error)?;
    // The set above replaces the one the settings would build from the built-in registry.
    let mut options = Settings {
        plugins: PluginSelection::new(),
        ..settings
    }
    .to_launch_options()?;
    options.plugins = set;
    Ok(options)
}

/// Reads the input of a task: JSON from the file `source`, from standard input when it is `-`,
/// and an empty object when there is none.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when the input cannot be read or is not JSON.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::plugin_cmd::read_task_input;
///
/// assert_eq!(read_task_input(None)?, serde_json::json!({}));
/// assert!(read_task_input(Some("no/such/input.json")).is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[cfg(feature = "plugins")]
pub fn read_task_input(source: Option<&str>) -> Result<Value, SeleniumBaseError> {
    read_task_input_from(source, std::io::stdin().lock())
}

/// [`read_task_input`] with the reader that `-` means.
#[cfg(feature = "plugins")]
fn read_task_input_from(
    source: Option<&str>,
    mut stdin: impl std::io::Read,
) -> Result<Value, SeleniumBaseError> {
    let Some(source) = source else {
        return Ok(Value::Object(serde_json::Map::new()));
    };
    let (text, from) = if source == "-" {
        let mut text = String::new();
        stdin.read_to_string(&mut text).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "cannot read the task input from standard input: {error}"
            ))
        })?;
        (text, "standard input".to_owned())
    } else {
        let text = std::fs::read_to_string(source).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "cannot read the task input file {source}: {error}"
            ))
        })?;
        (text, format!("file {source}"))
    };
    serde_json::from_str(&text).map_err(|error| {
        SeleniumBaseError::invalid_config(format!(
            "the task input from {from} is not JSON: {error}"
        ))
    })
}

/// Where the results of `sbrs plugin run` go: one JSON value per line, on standard output or in
/// the file `--out` names. Every line is flushed, so a task that is stopped leaves what it found.
#[cfg(feature = "plugins")]

pub struct JsonlSink {
    out: std::sync::Mutex<Box<dyn std::io::Write + Send>>,
}

#[cfg(feature = "plugins")]
impl JsonlSink {
    /// A sink over any writer.
    #[must_use]
    pub fn new(out: impl std::io::Write + Send + 'static) -> Self {
        Self {
            out: std::sync::Mutex::new(Box::new(out)),
        }
    }

    /// The sink `--out` asks for: the file `path` (created, with its folders; an existing file
    /// is replaced), or standard output when there is none.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when the file cannot be created.
    pub fn open(path: Option<&Path>) -> Result<Self, SeleniumBaseError> {
        let Some(path) = path else {
            return Ok(Self::new(std::io::stdout()));
        };
        let refused = |error: std::io::Error| {
            SeleniumBaseError::invalid_config(format!(
                "cannot create the output file {}: {error}",
                path.display()
            ))
        };
        if let Some(folder) = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
        {
            std::fs::create_dir_all(folder).map_err(refused)?;
        }
        std::fs::File::create(path).map(Self::new).map_err(refused)
    }
}

#[cfg(feature = "plugins")]
impl std::fmt::Debug for JsonlSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlSink").finish_non_exhaustive()
    }
}

#[cfg(feature = "plugins")]
impl seleniumbase_rs_plugin::Sink for JsonlSink {
    fn record(&self, line: &Value) -> seleniumbase_rs_plugin::Result<()> {
        use std::io::Write as _;

        let mut text = serde_json::to_string(line)
            .map_err(|error| seleniumbase_rs_plugin::PluginError::other(error.to_string()))?;
        text.push('\n');
        let mut out = self
            .out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        out.write_all(text.as_bytes())
            .and_then(|()| out.flush())
            .map_err(|error| seleniumbase_rs_plugin::PluginError::other(error.to_string()))
    }
}

/// How a task asks a person for help on the command line: it says what to do on standard error
/// and waits for Enter. Without a terminal there is nobody to ask, and waiting fails.
#[cfg(feature = "plugins")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalPrompt;

#[cfg(feature = "plugins")]
#[seleniumbase_rs_plugin::async_trait]
impl seleniumbase_rs_plugin::Prompt for TerminalPrompt {
    async fn wait(
        &self,
        message: &str,
        _page: &dyn seleniumbase_rs_plugin::PageHandle,
    ) -> seleniumbase_rs_plugin::Result<()> {
        use std::io::{IsTerminal as _, Write as _};

        if !std::io::stdin().is_terminal() {
            return Err(seleniumbase_rs_plugin::PluginError::unsupported(
                "waiting for a person: standard input is not a terminal",
            ));
        }
        // A closed standard error is nothing to stop the wait for.
        let _ = writeln!(
            std::io::stderr().lock(),
            "{message}\nPress Enter when you are done."
        );
        tokio::task::spawn_blocking(|| std::io::stdin().read_line(&mut String::new()))
            .await
            .map_err(|error| seleniumbase_rs_plugin::PluginError::other(error.to_string()))?
            .map(drop)
            .map_err(|error| seleniumbase_rs_plugin::PluginError::other(error.to_string()))
    }
}

#[cfg(all(test, feature = "plugins"))]
mod tests {
    use seleniumbase_rs_plugin::{
        async_trait, Capability, Category, Configurable, Descriptor, Metadata, PathScope, Phase,
        Plugin, PluginError, Registry, Requirements,
    };
    use serde_json::{json, Value};

    use super::*;

    /// A plugin that declares everything `info` shows, and accepts `{"mode": "..."}`.
    struct Probe;

    #[async_trait]
    impl Plugin for Probe {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }

        fn requirements(&self) -> Requirements {
            Requirements::CHROMIUM_ONLY | Requirements::LAUNCH_ONLY
        }
    }

    impl Configurable for Probe {
        const NAME: &'static str = "probe";

        fn describe() -> Metadata {
            Metadata::new(Category::Privacy)
                .version("1.2.3")
                .summary("Probes the page")
                .capability(Capability::LaunchArgs)
                .capability(Capability::Cdp("Network".into()))
                .capability(Capability::HostFsWrite(PathScope::Temp))
                .optional(Capability::PageEvaluate)
                .requires("base")
                .conflicts_with("rival")
                .after("early")
                .before("late")
                .phase(Phase::Last)
        }

        fn configure(options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            match options {
                Value::Null => Ok(Self),
                Value::Object(map) if map.keys().all(|key| key == "mode") => Ok(Self),
                other => Err(PluginError::Config(format!("probe: unexpected {other}"))),
            }
        }
    }

    /// A plugin that declares little: no version, no summary, no requirements.
    struct Plain;

    #[async_trait]
    impl Plugin for Plain {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }
    }

    impl Configurable for Plain {
        const NAME: &'static str = "plain";

        fn describe() -> Metadata {
            Metadata::new(Category::Core)
        }

        fn configure(_options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            Ok(Self)
        }
    }

    fn registry() -> Registry {
        Registry::new()
            .with(Descriptor::of::<Probe>())
            .with(Descriptor::of::<Plain>())
    }

    /// A plugin with a task: it writes its input to the sink and says it is done.
    struct Worker;

    #[async_trait]
    impl Plugin for Worker {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }

        fn task(&self) -> Option<&dyn seleniumbase_rs_plugin::Task> {
            Some(self)
        }
    }

    impl Configurable for Worker {
        const NAME: &'static str = "worker";

        fn describe() -> Metadata {
            Metadata::new(Category::Core).capability(Capability::Task)
        }

        fn configure(_options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            Ok(Self)
        }
    }

    #[async_trait]
    impl seleniumbase_rs_plugin::Task for Worker {
        async fn run(
            &self,
            cx: &seleniumbase_rs_plugin::TaskContext<'_>,
        ) -> seleniumbase_rs_plugin::Result<Value> {
            cx.sink.record(&json!({ "input": cx.input }))?;
            Ok(json!({ "done": true }))
        }
    }

    fn registry_with_a_task() -> Registry {
        registry().with(Descriptor::of::<Worker>())
    }

    #[test]
    fn the_plugins_that_offer_a_task_are_found_by_building_them() {
        assert_eq!(task_names_with(&registry_with_a_task()), ["worker"]);
        assert!(task_names_with(&registry()).is_empty());
    }

    #[test]
    fn a_name_that_has_no_task_is_a_usage_error_that_lists_the_names_that_have() {
        let with_task = registry_with_a_task();
        for (name, problem) in [
            ("ghost", "no plugin named `ghost`"),
            ("plain", "plugin `plain` offers no task"),
        ] {
            let error = task_launch_options_with(&with_task, Settings::default(), name)
                .map(drop)
                .unwrap_err();
            assert_eq!(error.exit_code(), 2, "{name}");
            let text = error.to_string();
            assert!(text.contains(problem), "{text}");
            assert!(text.contains("plugins with a task: worker"), "{text}");
        }
        let error = task_launch_options_with(&registry(), Settings::default(), "plain")
            .map(drop)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no plugin of this build offers a task"),
            "{error}"
        );
    }

    #[test]
    fn the_named_plugin_is_turned_on_by_the_name_alone_next_to_the_selected_ones() {
        let settings = Settings {
            plugins: PluginSelection::new().enable("plain"),
            ..Settings::default()
        };

        let options = task_launch_options_with(&registry_with_a_task(), settings, "worker")
            .expect("the options");

        assert_eq!(options.plugins.task_names(), ["worker"]);
        let names = options.plugins.names();
        assert!(
            names.contains(&"plain") && names.contains(&"worker"),
            "{names:?}"
        );
    }

    #[test]
    fn a_name_the_selection_already_has_is_not_enabled_twice() {
        let settings = Settings {
            plugins: PluginSelection::new().enable("worker"),
            ..Settings::default()
        };
        task_launch_options_with(&registry_with_a_task(), settings, "worker").expect("one worker");
    }

    #[test]
    fn options_for_a_plugin_that_is_not_enabled_are_a_usage_error_before_a_browser_starts() {
        let settings = Settings {
            plugins: PluginSelection::new().with_options("probe", json!({ "mode": 3, "bad": 1 })),
            ..Settings::default()
        };
        // `probe` is not in the selection's `enable`: the options alone are refused.
        let error = task_launch_options_with(&registry_with_a_task(), settings, "worker")
            .map(drop)
            .unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[tokio::test]
    async fn the_options_run_the_task_on_a_scripted_browser_and_the_sink_gets_its_lines() {
        use crate::common::shutdown::{Outcome, Shutdown};
        use crate::sb_cdp::{Browser, MockCtrl};

        let options =
            task_launch_options_with(&registry_with_a_task(), Settings::default(), "worker")
                .expect("the options");
        let browser = Browser::launch_mocked(options, &MockCtrl::new())
            .await
            .expect("the mocked launch");
        let buffer = SharedBuffer::default();
        let sink = JsonlSink::new(buffer.clone());
        let (mut shutdown, _trigger) = Shutdown::manual();

        let outcome = crate::plugin_host::run_task_on(
            &mut shutdown,
            browser,
            "worker",
            &json!({ "jobs": [1, 2] }),
            &sink,
            &seleniumbase_rs_plugin::NoPrompt,
        )
        .await
        .expect("the task runs");

        assert_eq!(outcome, Outcome::Completed(json!({ "done": true })));
        assert_eq!(buffer.text(), "{\"input\":{\"jobs\":[1,2]}}\n");
    }

    /// A writer whose text the test can read back.
    #[derive(Clone, Default)]
    struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl SharedBuffer {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl std::io::Write for SharedBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_sink_writes_one_json_value_per_line() {
        use seleniumbase_rs_plugin::Sink as _;

        let buffer = SharedBuffer::default();
        let sink = JsonlSink::new(buffer.clone());
        sink.record(&json!({ "a": 1 })).unwrap();
        sink.record(&json!("two")).unwrap();
        assert_eq!(buffer.text(), "{\"a\":1}\n\"two\"\n");
    }

    #[test]
    fn a_file_sink_creates_its_folders_and_replaces_an_old_file() {
        use seleniumbase_rs_plugin::Sink as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep/er/found.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "old\n").unwrap();

        let sink = JsonlSink::open(Some(&path)).unwrap();
        sink.record(&json!({ "n": 1 })).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"n\":1}\n");
        let fresh = dir.path().join("new/folder/out.jsonl");
        JsonlSink::open(Some(&fresh)).unwrap();
        assert!(fresh.exists(), "its folders were created");
        let blocked = dir.path().join("deep/er/found.jsonl/inside");
        let error = JsonlSink::open(Some(&blocked)).unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn the_task_input_is_a_file_standard_input_or_an_empty_object() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("jobs.json");
        std::fs::write(&file, r#"{"jobs": [1]}"#).unwrap();
        let file = file.to_str().unwrap();
        let nothing: &[u8] = b"";

        assert_eq!(read_task_input_from(None, nothing).unwrap(), json!({}));
        assert_eq!(
            read_task_input_from(Some(file), nothing).unwrap(),
            json!({ "jobs": [1] })
        );
        assert_eq!(
            read_task_input_from(Some("-"), &br#"[1, 2]"#[..]).unwrap(),
            json!([1, 2])
        );
    }

    #[test]
    fn a_task_input_that_cannot_be_used_is_a_usage_error_naming_where_it_came_from() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{not json").unwrap();
        let bad = bad.to_str().unwrap();
        let nothing: &[u8] = b"";

        for (source, stdin, needle) in [
            (bad, nothing, "file ".to_owned() + bad),
            ("no/such/file.json", nothing, "no/such/file.json".to_owned()),
            ("-", &b"oops"[..], "standard input".to_owned()),
        ] {
            let error = read_task_input_from(Some(source), stdin).unwrap_err();
            assert_eq!(error.exit_code(), 2, "{source}");
            assert!(error.to_string().contains(&needle), "{source}: {error}");
        }
    }

    /// The text of the line that starts with `label`, without the label, and
    /// the lines indented under it.
    fn value_of(text: &str, label: &str) -> Vec<String> {
        let mut lines = text.lines();
        let first = lines
            .find(|line| line.starts_with(label))
            .unwrap_or_else(|| panic!("no `{label}` row in:\n{text}"));
        let mut value = vec![first[label.len()..].trim().to_owned()];
        value.extend(
            lines
                .take_while(|line| line.starts_with("  "))
                .map(|line| line.trim().to_owned()),
        );
        value
    }

    fn words(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    #[test]
    fn the_list_has_a_header_and_a_row_per_plugin_in_registry_order() {
        let text = list_with(&registry(), &PluginSelection::from_list("probe"));

        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            &words(lines[0])[..4],
            ["NAME", "CATEGORY", "ENABLED", "SUMMARY"]
        );
        assert_eq!(&words(lines[1])[..3], ["probe", "privacy", "yes"]);
        assert!(lines[1].ends_with("Probes the page"), "{}", lines[1]);
        assert_eq!(&words(lines[2])[..3], ["plain", "core", "no"]);
        assert!(
            lines[2].ends_with('-'),
            "no summary shows a dash: {}",
            lines[2]
        );
        // The columns line up.
        assert_eq!(lines[0].find("CATEGORY"), lines[1].find("privacy"));
        assert_eq!(lines[0].find("SUMMARY"), lines[1].find("Probes"));
        assert!(
            text.contains("--plugin NAME"),
            "it says how to turn one on:\n{text}"
        );
        assert!(text.contains("sbrs plugin info NAME"), "{text}");
    }

    #[test]
    fn nothing_is_enabled_unless_the_selection_says_so() {
        let text = list_with(&registry(), &PluginSelection::new());
        assert_eq!(
            &words(text.lines().nth(1).unwrap())[..3],
            ["probe", "privacy", "no"]
        );
        assert_eq!(
            &words(text.lines().nth(2).unwrap())[..3],
            ["plain", "core", "no"]
        );
        assert!(!text.contains("not available"), "{text}");
    }

    #[test]
    fn a_name_the_build_does_not_have_is_reported_below_the_table() {
        let text = list_with(&registry(), &PluginSelection::from_list("probe,sheild"));
        assert!(
            text.contains("enabled but not in this build: sheild"),
            "{text}"
        );
    }

    #[test]
    fn an_empty_registry_says_how_to_get_plugins() {
        let text = list_with(&Registry::new(), &PluginSelection::new());
        assert!(text.contains("no built-in plugin"), "{text}");
        assert!(text.contains("--features plugins-builtin"), "{text}");
        assert!(!text.contains("NAME"), "there is no table: {text}");
    }

    #[test]
    fn info_shows_what_the_plugin_declares() {
        let text = info_with(&registry(), "probe", &PluginSelection::from_list("probe")).unwrap();

        assert_eq!(text.lines().next(), Some("probe 1.2.3"));
        assert_eq!(text.lines().nth(1), Some("Probes the page"));
        assert_eq!(value_of(&text, "category"), ["privacy"]);
        assert_eq!(value_of(&text, "enabled"), ["yes"]);
        assert_eq!(value_of(&text, "phase"), ["last"]);
        assert_eq!(value_of(&text, "requires"), ["base"]);
        assert_eq!(value_of(&text, "conflicts"), ["rival"]);
        assert_eq!(value_of(&text, "runs after"), ["early"]);
        assert_eq!(value_of(&text, "runs before"), ["late"]);
        assert_eq!(value_of(&text, "seals"), ["-"]);

        let needs = value_of(&text, "needs").join(" ");
        assert!(needs.contains("Chromium"), "{needs}");
        assert!(needs.contains("starts itself"), "{needs}");

        let capabilities = value_of(&text, "capabilities");
        assert_eq!(capabilities.len(), 3, "{capabilities:?}");
        assert_eq!(words(&capabilities[0]), ["launch.args", "low", "enforced"]);
        assert_eq!(
            words(&capabilities[1]),
            ["cdp:Network", "medium", "enforced"]
        );
        assert_eq!(
            &words(&capabilities[2])[..3],
            ["host.fs.write:temp", "high", "declared"],
            "host file access is a promise for native code"
        );
        assert!(text.contains("not sandboxed"), "{text}");
        let optional = value_of(&text, "optional");
        assert_eq!(words(&optional[0]), ["page.evaluate", "medium", "enforced"]);
    }

    #[test]
    fn info_of_a_plugin_that_declares_little_shows_dashes() {
        let text = info_with(&registry(), "plain", &PluginSelection::new()).unwrap();

        assert_eq!(text.lines().next(), Some("plain"));
        assert_eq!(value_of(&text, "category"), ["core"]);
        assert_eq!(value_of(&text, "enabled"), ["no"]);
        assert_eq!(value_of(&text, "needs"), ["-"]);
        assert_eq!(value_of(&text, "capabilities"), ["-"]);
        assert_eq!(value_of(&text, "requires"), ["-"]);
        assert!(!text.contains("not sandboxed"), "{text}");
    }

    #[test]
    fn info_says_how_to_give_options_and_shows_the_ones_in_effect() {
        let none = info_with(&registry(), "probe", &PluginSelection::new()).unwrap();
        let options = value_of(&none, "options").join(" ");
        assert!(options.contains("[plugins.probe]"), "{options}");
        assert!(options.contains("--plugin-opt"), "{options}");

        let selection =
            PluginSelection::from_list("probe").with_options("probe", json!({ "mode": "strict" }));
        let some = info_with(&registry(), "probe", &selection).unwrap();
        assert_eq!(value_of(&some, "options"), [r#"mode = "strict""#]);
    }

    #[test]
    fn info_shows_options_the_plugin_refuses() {
        let selection =
            PluginSelection::from_list("probe").with_options("probe", json!({ "mdoe": 1 }));

        let text = info_with(&registry(), "probe", &selection).unwrap();

        let options = value_of(&text, "options");
        assert_eq!(options[0], "mdoe = 1");
        assert!(
            options
                .iter()
                .any(|line| line.contains("refused") && line.contains("unexpected")),
            "{options:?}"
        );
        // The rest of the page is still there.
        assert_eq!(value_of(&text, "category"), ["privacy"]);
    }

    /// A plugin that cannot be built without options.
    struct Needy;

    #[async_trait]
    impl Plugin for Needy {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }
    }

    impl Configurable for Needy {
        const NAME: &'static str = "needy";

        fn describe() -> Metadata {
            Metadata::new(Category::Extension)
        }

        fn configure(options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            match options.get("files") {
                Some(_) => Ok(Self),
                None => Err(PluginError::Config("`files` is required".to_owned())),
            }
        }
    }

    #[test]
    fn info_says_when_a_plugin_does_not_start_without_options_and_when_it_refuses_the_given_ones() {
        let registry = Registry::new().with(Descriptor::of::<Needy>());

        let none = info_with(&registry, "needy", &PluginSelection::new()).unwrap();
        let options = value_of(&none, "options");
        assert!(options[0].starts_with("none set"), "{options:?}");
        assert!(
            options
                .iter()
                .any(|line| line.contains("does not start without options")
                    && line.contains("`files` is required")),
            "the page tells that options are required, not that they were refused: {options:?}"
        );
        assert_eq!(value_of(&none, "category"), ["extension"]);

        let wrong = PluginSelection::from_list("needy").with_options("needy", json!({ "x": 1 }));
        let refused = value_of(&info_with(&registry, "needy", &wrong).unwrap(), "options");
        assert_eq!(refused[0], "x = 1");
        assert!(
            refused
                .iter()
                .any(|line| line.contains("refused by the plugin")),
            "{refused:?}"
        );

        let right =
            PluginSelection::from_list("needy").with_options("needy", json!({ "files": [] }));
        let fine = value_of(&info_with(&registry, "needy", &right).unwrap(), "options");
        assert_eq!(fine, ["files = []"]);
    }

    #[test]
    fn info_of_an_unknown_name_is_a_usage_error_that_lists_the_known_ones() {
        let error = info_with(&registry(), "sheild", &PluginSelection::new()).unwrap_err();

        assert_eq!(error.exit_code(), 2, "{error:?}");
        let text = error.to_string();
        assert!(text.contains("`sheild`"), "{text}");
        assert!(text.contains("known: probe, plain"), "{text}");
        assert!(text.contains("sbrs plugin list"), "{text}");

        let none = info_with(&Registry::new(), "x", &PluginSelection::new()).unwrap_err();
        assert!(none.to_string().contains("no built-in plugin"), "{none}");
    }

    #[test]
    fn a_selection_is_checked_name_by_name_and_option_by_option() {
        check_selection_with(&registry(), &PluginSelection::new()).unwrap();
        check_selection_with(
            &registry(),
            &PluginSelection::from_list("probe,plain")
                .with_options("probe", json!({ "mode": "x" })),
        )
        .unwrap();

        let unknown =
            check_selection_with(&registry(), &PluginSelection::from_list("probe,sheild"))
                .unwrap_err();
        assert_eq!(unknown.exit_code(), 2);
        assert!(unknown.to_string().contains("`sheild`"), "{unknown}");
        assert!(
            unknown.to_string().contains("known: probe, plain"),
            "{unknown}"
        );

        let refused = check_selection_with(
            &registry(),
            &PluginSelection::from_list("probe").with_options("probe", json!({ "mdoe": 1 })),
        )
        .unwrap_err();
        assert_eq!(refused.exit_code(), 2);
        assert!(refused.to_string().contains("probe"), "{refused}");
        assert!(refused.to_string().contains("mdoe"), "{refused}");

        let stray = check_selection_with(
            &registry(),
            &PluginSelection::from_list("plain").with_options("probe", json!({})),
        )
        .unwrap_err();
        assert_eq!(stray.exit_code(), 2);
        assert!(stray.to_string().contains("not in `enable`"), "{stray}");

        let twice = check_selection_with(
            &registry(),
            &PluginSelection::new().enable("probe").enable("probe"),
        )
        .unwrap_err();
        assert_eq!(twice.exit_code(), 2);
        assert!(twice.to_string().contains("already in the set"), "{twice}");
    }

    fn origin(name: &str, source: PluginSource) -> PluginOrigin {
        PluginOrigin {
            name: name.to_owned(),
            source,
        }
    }

    #[test]
    fn a_power_over_the_host_needs_the_operator_to_name_the_plugin() {
        // `probe` declares `host.fs.write:temp`; `plain` declares nothing.
        let error = check_origins_with(
            &registry(),
            &[
                origin("plain", PluginSource::ProjectFile),
                origin("probe", PluginSource::ProjectFile),
            ],
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 2, "{error:?}");
        let text = error.to_string();
        for wanted in [
            "`probe`",
            "host.fs.write:temp",
            "--plugin probe",
            "SB_PLUGINS=probe",
        ] {
            assert!(text.contains(wanted), "{wanted:?} in: {text}");
        }
        assert!(
            !text.contains("`plain`"),
            "only the plugin with the power is named: {text}"
        );

        for source in [
            PluginSource::CommandLine,
            PluginSource::Environment,
            PluginSource::ConfigFile,
        ] {
            check_origins_with(&registry(), &[origin("probe", source)])
                .unwrap_or_else(|error| panic!("{source:?}: {error}"));
        }
    }

    #[test]
    fn a_name_the_build_does_not_have_is_left_to_the_name_check() {
        check_origins_with(&registry(), &[origin("sheild", PluginSource::ProjectFile)]).unwrap();
    }

    #[test]
    fn a_plugin_that_only_adjusts_the_launch_may_come_from_the_project_file() {
        check_origins_with(&registry(), &[origin("plain", PluginSource::ProjectFile)]).unwrap();
        assert!(needs_the_operator("launch.proxy"));
        assert!(needs_the_operator("artifacts.write"));
        assert!(needs_the_operator("host.process"));
        assert!(needs_the_operator("host.net:any"));
        for capability in [
            "launch.args",
            "launch.network",
            "launch.extensions",
            "page.script:*",
            "cdp:Network",
        ] {
            assert!(!needs_the_operator(capability), "{capability}");
        }
    }
}

#[cfg(test)]
mod choice_tests {
    use serde_json::json;

    use super::*;

    fn table(text: &str) -> PluginSelection {
        toml::from_str(text).expect("a plugin table")
    }

    fn choice(
        file: &str,
        variable: Option<&str>,
        plugins: &[&str],
        options: &[&str],
    ) -> PluginChoice {
        PluginChoice::from_parts(
            table(file),
            PluginSource::ProjectFile,
            plugins,
            options,
            variable.map(str::to_owned),
        )
        .expect("a valid choice")
    }

    fn sources(choice: &PluginChoice) -> Vec<(&str, PluginSource)> {
        choice
            .origins()
            .iter()
            .map(|origin| (origin.name.as_str(), origin.source))
            .collect()
    }

    #[test]
    fn each_name_remembers_the_strongest_place_that_gave_it() {
        let file = r#"enable = ["a", "b"]"#;

        let only_the_file = choice(file, None, &[], &[]);
        assert_eq!(
            sources(&only_the_file),
            [
                ("a", PluginSource::ProjectFile),
                ("b", PluginSource::ProjectFile)
            ]
        );

        // The variable replaces the file's names; the flags add to them.
        let all = choice(file, Some("b, c"), &["c", "d"], &[]);
        assert_eq!(all.selection().enabled(), ["b", "c", "d"]);
        assert_eq!(
            sources(&all),
            [
                ("b", PluginSource::Environment),
                ("c", PluginSource::CommandLine),
                ("d", PluginSource::CommandLine)
            ]
        );

        // A name the file and the flag both give is the flag's.
        let both = choice(file, None, &["b"], &[]);
        assert_eq!(
            sources(&both),
            [
                ("a", PluginSource::ProjectFile),
                ("b", PluginSource::CommandLine)
            ]
        );
    }

    #[test]
    fn a_file_the_config_flag_names_is_not_a_project_file() {
        let selection = PluginChoice::from_parts(
            table(r#"enable = ["a"]"#),
            PluginSource::ConfigFile,
            &[] as &[&str],
            &[] as &[&str],
            None,
        )
        .unwrap();
        assert_eq!(sources(&selection), [("a", PluginSource::ConfigFile)]);
    }

    #[test]
    fn an_empty_variable_turns_the_files_plugins_off_and_a_flag_still_adds() {
        let file = "enable = [\"a\"]\n[a]\nlevel = 1\n";

        let off = choice(file, Some(""), &[], &[]);
        assert!(off.selection().is_empty(), "{:?}", off.selection());
        assert!(off.origins().is_empty());

        let then_flag = choice(file, Some(""), &["b"], &[]);
        assert_eq!(then_flag.selection().enabled(), ["b"]);
    }

    #[test]
    fn a_flag_brings_back_the_file_options_of_a_plugin_the_variable_left_out() {
        let file = "enable = [\"shield\"]\n[shield]\nlevel = \"strict\"\nextra = 1\n";

        // The variable does not name shield, so its options are dropped ...
        let dropped = choice(file, Some("crx"), &[], &[]);
        assert_eq!(dropped.selection().enabled(), ["crx"]);
        assert!(dropped.selection().options_of("shield").is_none());

        // ... unless `--plugin shield` turns it on again: the level the file chose stays.
        let back = choice(file, Some("crx"), &["shield"], &[]);
        assert_eq!(back.selection().enabled(), ["crx", "shield"]);
        assert_eq!(
            back.selection().options_of("shield"),
            Some(&json!({ "level": "strict", "extra": 1 }))
        );

        // A `--plugin-opt` still wins key by key.
        let tuned = choice(file, Some("crx"), &["shield"], &[r#"shield.level="basic""#]);
        assert_eq!(
            tuned.selection().options_of("shield"),
            Some(&json!({ "level": "basic", "extra": 1 }))
        );
    }

    #[test]
    fn options_of_a_plugin_the_file_never_enabled_stay_for_the_build_to_refuse() {
        // No variable: nothing was replaced, so the stray table is a mistake to report.
        let stray = choice("[shield]\nlevel = 1\n", None, &[], &[]);
        assert!(stray.selection().options_of("shield").is_some());
    }

    #[test]
    fn a_flag_mistake_is_an_invalid_config_error() {
        let error = PluginChoice::from_parts(
            PluginSelection::new(),
            PluginSource::ProjectFile,
            &[] as &[&str],
            &[r#"shield.level="strict""#],
            None,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 2, "{error:?}");
        assert!(error.to_string().contains("nothing turns on"), "{error}");
    }

    #[test]
    fn the_sources_are_told_in_words() {
        assert_eq!(PluginSource::CommandLine.describe(), "--plugin");
        assert_eq!(PluginSource::Environment.describe(), "SB_PLUGINS");
        assert!(PluginSource::ProjectFile
            .describe()
            .contains("current folder"));
        assert!(PluginSource::ConfigFile.describe().contains("--config"));
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn a_build_without_plugins_refuses_to_apply_a_selection() {
        let config = BrowserConfig::default();
        apply_to_config(config.clone(), &PluginSelection::new()).unwrap();
        let error = apply_to_config(config, &PluginSelection::from_list("shield")).unwrap_err();
        assert!(
            error.to_string().contains("--features plugins-builtin"),
            "{error}"
        );
    }
}
