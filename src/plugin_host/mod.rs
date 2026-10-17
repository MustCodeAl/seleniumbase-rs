//! Turning plugin names and options into a [`PluginSet`] (feature `plugins`).
//!
//! A program that wants a built-in plugin without writing code lists it by
//! name, in the `[plugins]` table of the settings file or in `SB_PLUGINS`
//! (see [`PluginSelection`]), and the launch builders resolve the names here
//! through a [`Registry`]:
//!
//! * [`builtin_registry`] holds the official plugins whose cargo feature is on
//!   (`plugin-shield`, `plugin-block-resources`, `plugin-crx`, or
//!   `plugins-builtin` for all of them). A build with none has an empty
//!   registry, and every name is then unknown.
//! * [`build`] and [`build_with`] return a *strict* [`PluginSet`]: it refuses a
//!   plugin that declares no metadata, and grants each plugin what it declares
//!   except Critical capabilities, as the command line will.
//! * A plugin from another crate joins by name through
//!   `Registry::with(Descriptor::of::<P>())` and [`build_with`], or goes into a
//!   set directly with `PluginSet::with`.
//!
//! An unknown name, options for a plugin that is not enabled, options the
//! plugin does not accept, and a name enabled twice are all errors
//! ([`SeleniumBaseError::Plugin`]) before any browser starts. Nothing is
//! silently ignored.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::plugins::PluginSelection;
//! use seleniumbase_rs::plugin_host;
//!
//! // With no plugin feature on, the registry is empty and a name is unknown;
//! // the error says which names would have been accepted.
//! let selection = PluginSelection::new().enable("no-such-plugin");
//! let error = plugin_host::build(&selection).unwrap_err();
//! assert!(error.to_string().contains("no-such-plugin"));
//! ```

use seleniumbase_rs_plugin::{PluginError, PluginSet, PluginSpec, Registry};

use crate::config::plugins::PluginSelection;
use crate::error::SeleniumBaseError;

pub(crate) mod convert;
pub(crate) mod guard;

/// The cargo features that add a built-in plugin, for messages.
const BUILTIN_FEATURES: &str = "plugin-shield, plugin-block-resources, plugin-crx \
                                (plugins-builtin turns them all on)";

/// The built-in plugins this build contains, by name.
///
/// The names are those whose cargo feature is on; the registry is empty when
/// none is.
#[must_use]
pub fn builtin_registry() -> Registry {
    #[cfg(any(
        feature = "plugin-shield",
        feature = "plugin-block-resources",
        feature = "plugin-crx"
    ))]
    {
        seleniumbase_rs_plugins_builtin::registry()
    }
    #[cfg(not(any(
        feature = "plugin-shield",
        feature = "plugin-block-resources",
        feature = "plugin-crx"
    )))]
    {
        Registry::new()
    }
}

/// The plugins `selection` names, built from the built-in registry, as a
/// strict set.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] for an unknown name, options of a plugin that
/// is not enabled, options the plugin refuses, or a name enabled twice.
pub fn build(selection: &PluginSelection) -> Result<PluginSet, SeleniumBaseError> {
    build_with(&builtin_registry(), selection)
}

/// [`build`] with a registry of your own, such as the built-in one plus the
/// plugins of other crates.
///
/// # Errors
///
/// As [`build`].
pub fn build_with(
    registry: &Registry,
    selection: &PluginSelection,
) -> Result<PluginSet, SeleniumBaseError> {
    add_selection(PluginSet::strict(), registry, selection)
}

/// Adds the plugins `selection` names to `set`, after those it holds.
///
/// A set that is empty becomes the registry's strict set, so a program that
/// uses names only gets the strict checks; a set that already holds plugins
/// keeps its own strictness.
///
/// # Errors
///
/// As [`build`].
pub(crate) fn add_selection(
    set: PluginSet,
    registry: &Registry,
    selection: &PluginSelection,
) -> Result<PluginSet, SeleniumBaseError> {
    let specs = specs_of(selection)?;
    if specs.is_empty() {
        return Ok(set);
    }
    for spec in &specs {
        if registry.get(&spec.name).is_none() {
            return Err(unknown(&spec.name, registry));
        }
    }
    // A set that holds nothing yet becomes strict; one with plugins keeps its own mode.
    let mut set = if set.is_empty() {
        PluginSet::strict()
    } else {
        set
    };
    for spec in &specs {
        let plugin = registry
            .build(spec)
            .map_err(|error| named(&spec.name, error))?;
        set = set
            .with_shared(plugin)
            .map_err(|error| named(&spec.name, error))?;
    }
    Ok(set)
}

/// The specs for `selection`: its names, in order, each with its options.
fn specs_of(selection: &PluginSelection) -> Result<Vec<PluginSpec>, SeleniumBaseError> {
    if let Some(stray) = selection
        .option_names()
        .find(|name| !selection.enabled().iter().any(|enabled| enabled == name))
    {
        return Err(SeleniumBaseError::plugin(
            stray,
            format!(
                "options were given for `{stray}`, which is not in `enable` ([plugins.{stray}]); \
                 add it to `enable`, or remove the options (a misspelled `enable` looks like \
                 this too)"
            ),
        ));
    }
    Ok(selection
        .enabled()
        .iter()
        .map(|name| {
            let spec = PluginSpec::new(name.clone());
            match selection.options_of(name) {
                Some(options) => spec.options(options.clone()),
                None => spec,
            }
        })
        .collect())
}

/// The error for a name `registry` does not know: the names it does, and how
/// the others get in.
fn unknown(name: &str, registry: &Registry) -> SeleniumBaseError {
    let known = registry.names();
    let known = if known.is_empty() {
        "none: this build has no built-in plugin".to_owned()
    } else {
        known.join(", ")
    };
    SeleniumBaseError::plugin(
        name,
        format!(
            "no plugin named `{name}` is available (known: {known}). The built-in plugins are \
             cargo features of seleniumbase-rs: {BUILTIN_FEATURES}. A plugin from another crate \
             is added with PluginSet::with, or by name through \
             Registry::with(Descriptor::of::<P>()) and plugin_host::build_with"
        ),
    )
}

/// `error` for the plugin called `name`, as the error of this crate.
fn named(name: &str, error: PluginError) -> SeleniumBaseError {
    match error {
        PluginError::Config(message) => {
            SeleniumBaseError::plugin(name, format!("its options were refused: {message}"))
        }
        other => SeleniumBaseError::plugin(name, other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use seleniumbase_rs_plugin::{
        async_trait, Capability, Category, Configurable, Descriptor, Metadata, Plugin,
    };
    use serde_json::{json, Value};

    use super::*;

    /// A plugin that is built from `{"mode": "..."}` and refuses anything else.
    struct Dummy {
        mode: String,
    }

    #[async_trait]
    impl Plugin for Dummy {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }
    }

    impl Configurable for Dummy {
        const NAME: &'static str = "dummy";

        fn describe() -> Metadata {
            Metadata::new(Category::Core).capability(Capability::LaunchArgs)
        }

        fn configure(options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            let mode = match options {
                Value::Null => "default".to_owned(),
                Value::Object(map) if map.keys().all(|key| key == "mode") => map
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_owned(),
                other => {
                    return Err(PluginError::Config(format!("dummy: unexpected {other}")));
                }
            };
            Ok(Self { mode })
        }
    }

    /// A plugin that declares no metadata, which a strict set refuses.
    struct Legacy;

    #[async_trait]
    impl Plugin for Legacy {
        fn name(&self) -> &str {
            "legacy"
        }
    }

    fn registry() -> Registry {
        Registry::new().with(Descriptor::of::<Dummy>())
    }

    fn error_text(result: Result<PluginSet, SeleniumBaseError>) -> String {
        result.expect_err("an error").to_string()
    }

    #[test]
    fn names_become_a_strict_set_in_the_order_given() {
        let registry = Registry::new()
            .with(Descriptor::of::<Dummy>())
            .with(Descriptor::of::<Second>());
        let selection = PluginSelection::new().enable("second").enable("dummy");

        let set = build_with(&registry, &selection).unwrap();

        assert_eq!(set.names(), ["second", "dummy"]);
        let with_legacy = set.with(Legacy).unwrap();
        let context = seleniumbase_rs_plugin::LaunchContext {
            engine: seleniumbase_rs_plugin::Engine::Cdp,
            family: seleniumbase_rs_plugin::BrowserFamily::Chromium,
            headless: true,
            connecting: false,
        };
        assert!(
            with_legacy.check(&context).is_err(),
            "the set built from names is strict"
        );
    }

    /// A second declared plugin, for the order.
    struct Second;

    #[async_trait]
    impl Plugin for Second {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }
    }

    impl Configurable for Second {
        const NAME: &'static str = "second";

        fn describe() -> Metadata {
            Metadata::new(Category::Core)
        }

        fn configure(_options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            Ok(Self)
        }
    }

    #[test]
    fn nothing_selected_gives_the_set_back_untouched() {
        let set = build_with(&registry(), &PluginSelection::new()).unwrap();
        assert!(set.is_empty());

        let existing = PluginSet::new().with(Legacy).unwrap();
        let same = add_selection(existing, &registry(), &PluginSelection::new()).unwrap();
        assert_eq!(same.names(), ["legacy"]);
    }

    #[test]
    fn an_unknown_name_is_an_error_that_names_the_known_ones_and_the_way_in() {
        let selection = PluginSelection::new().enable("sheild");

        let text = error_text(build_with(&registry(), &selection));

        assert!(text.contains("`sheild`"), "{text}");
        assert!(text.contains("known: dummy"), "{text}");
        assert!(text.contains("plugin-shield"), "{text}");
        assert!(text.contains("plugins-builtin"), "{text}");
        let error = build_with(&registry(), &selection).unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "sheild"),
            "{error:?}"
        );
    }

    #[test]
    fn with_no_plugin_in_the_registry_the_error_says_so() {
        let text = error_text(build_with(
            &Registry::new(),
            &PluginSelection::new().enable("shield"),
        ));
        assert!(
            text.contains("none: this build has no built-in plugin"),
            "{text}"
        );
    }

    #[test]
    fn options_reach_the_plugin_and_a_wrong_option_is_an_error_naming_it() {
        let ok = PluginSelection::new()
            .enable("dummy")
            .with_options("dummy", json!({ "mode": "strict" }));
        assert_eq!(build_with(&registry(), &ok).unwrap().names(), ["dummy"]);

        let typo = PluginSelection::new()
            .enable("dummy")
            .with_options("dummy", json!({ "mdoe": "strict" }));
        let error = build_with(&registry(), &typo).unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "dummy"),
            "{error:?}"
        );
        assert!(error.to_string().contains("mdoe"), "{error}");
    }

    #[test]
    fn options_for_a_plugin_that_is_not_enabled_are_an_error() {
        let selection = PluginSelection::new()
            .enable("dummy")
            .with_options("enabled", json!(["dummy"]));

        let error = build_with(&registry(), &selection).unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "enabled"),
            "{error:?}"
        );
        assert!(error.to_string().contains("not in `enable`"), "{error}");
    }

    #[test]
    fn a_name_enabled_twice_is_an_error() {
        let selection = PluginSelection::new().enable("dummy").enable("dummy");

        let error = build_with(&registry(), &selection).unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "dummy"),
            "{error:?}"
        );
        assert!(error.to_string().contains("already in the set"), "{error}");
    }

    #[test]
    fn names_are_added_after_the_plugins_a_set_already_has() {
        let existing = PluginSet::new().with(Legacy).unwrap();
        let selection = PluginSelection::new().enable("dummy");

        let set = add_selection(existing, &registry(), &selection).unwrap();

        assert_eq!(set.names(), ["legacy", "dummy"]);
        let again = add_selection(set, &registry(), &selection).unwrap_err();
        assert!(again.to_string().contains("already in the set"), "{again}");
    }

    #[test]
    fn the_dummy_keeps_the_mode_it_was_given() {
        let plugin = Dummy::configure(&json!({ "mode": "strict" })).unwrap();
        assert_eq!(plugin.mode, "strict");
        assert_eq!(Dummy::configure(&Value::Null).unwrap().mode, "default");
        let shared: Arc<dyn Plugin> = Arc::new(plugin);
        assert_eq!(shared.name(), "dummy");
    }

    #[cfg(feature = "plugin-shield")]
    #[test]
    fn the_builtin_registry_builds_shield_with_its_options() {
        assert!(builtin_registry().names().contains(&"shield"));

        let selection = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "strict" }));
        assert_eq!(build(&selection).unwrap().names(), ["shield"]);

        let bad = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "nope" }));
        let error = build(&bad).unwrap_err();
        assert!(error.to_string().contains("shield"), "{error}");
    }

    #[cfg(all(
        feature = "plugin-shield",
        feature = "plugin-block-resources",
        feature = "plugin-crx"
    ))]
    #[test]
    fn plugins_builtin_knows_the_three_official_plugins() {
        let mut names = builtin_registry().names();
        names.sort_unstable();
        assert_eq!(names, ["block-resources", "crx", "shield"]);
    }

    #[cfg(not(any(
        feature = "plugin-shield",
        feature = "plugin-block-resources",
        feature = "plugin-crx"
    )))]
    #[test]
    fn without_a_builtin_feature_the_registry_is_empty() {
        assert!(builtin_registry().names().is_empty());
    }
}
