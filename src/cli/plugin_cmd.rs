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

#[cfg(feature = "plugins")]
use std::fmt::Write as _;

#[cfg(feature = "plugins")]
use seleniumbase_rs_plugin::{
    CapabilitySet, Category, Descriptor, Enforcement, Metadata, Plugin, PluginSpec, Registry,
    Requirements, Seals,
};
#[cfg(feature = "plugins")]
use serde_json::Value;

use crate::config::plugins::PluginSelection;
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
}
