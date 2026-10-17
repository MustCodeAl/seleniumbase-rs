//! The plugins a program turns on by name, without code: the `[plugins]` table
//! of the settings file and the `SB_PLUGINS` variable.
//!
//! ```toml
//! # sbase_config.toml
//! [plugins]
//! enable = ["shield", "block-resources"]   # the order is the insertion order
//!
//! [plugins.shield]                         # options of one plugin
//! level = "strict"
//! ```
//!
//! A [`PluginSelection`] only holds names and options; it is always compiled,
//! so a settings file with a `[plugins]` table parses in every build. Turning
//! the names into plugins needs the feature `plugins` and the plugin crates
//! (`plugins-builtin` for the official ones): see
//! `plugin_host` (feature `plugins`). A build without the feature refuses a
//! non-empty selection when it loads the settings, because a program that asked
//! for privacy hardening and silently runs without it is worse than one that
//! stops.
//!
//! The command-line flags `--plugin` and `--plugin-opt` are not part of this
//! module yet.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::SeleniumBaseError;

/// The environment variable that names the plugins to turn on, as a
/// comma-separated list such as `shield,block-resources`.
pub const PLUGINS_ENV: &str = "SB_PLUGINS";

/// The plugins to turn on, by name, and their options.
///
/// It is the `[plugins]` table of the settings file: `enable` lists the names
/// in the order their plugins are added, and a table named like a plugin holds
/// that plugin's options. The same value is built from `SB_PLUGINS`, or in
/// code with [`enable`](Self::enable) and [`with_options`](Self::with_options).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::plugins::PluginSelection;
///
/// let selection: PluginSelection = toml::from_str(
///     r#"
///     enable = ["shield"]
///
///     [shield]
///     level = "strict"
///     "#,
/// )?;
/// assert_eq!(selection.enabled(), ["shield"]);
/// assert_eq!(selection.options_of("shield").unwrap()["level"], "strict");
/// # Ok::<(), toml::de::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginSelection {
    /// The plugins to add, in order.
    #[serde(default)]
    enable: Vec<String>,
    /// The options of each plugin, by name. Every key of the table but `enable`.
    #[serde(flatten)]
    options: BTreeMap<String, Value>,
}

impl PluginSelection {
    /// A selection that turns nothing on.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The selection a comma-separated list names, as `SB_PLUGINS` holds it.
    /// Spaces around a name and empty entries are ignored.
    ///
    /// ```
    /// use seleniumbase_rs::config::plugins::PluginSelection;
    ///
    /// let selection = PluginSelection::from_list(" shield, ,block-resources,");
    /// assert_eq!(selection.enabled(), ["shield", "block-resources"]);
    /// ```
    #[must_use]
    pub fn from_list(list: &str) -> Self {
        let mut selection = Self::new();
        selection.set_enabled_from_list(list);
        selection
    }

    /// Adds `name` after the plugins already enabled.
    #[must_use]
    pub fn enable(mut self, name: impl Into<String>) -> Self {
        self.enable.push(name.into());
        self
    }

    /// Sets the options of the plugin called `name`, replacing earlier ones.
    /// `options` is what the plugin's own option table holds, usually a JSON
    /// object.
    #[must_use]
    pub fn with_options(mut self, name: impl Into<String>, options: Value) -> Self {
        self.options.insert(name.into(), options);
        self
    }

    /// Replaces the enabled names with those of a comma-separated list, as the
    /// `SB_PLUGINS` variable does. The options of the plugins the list still
    /// names stay; the options of the others are dropped, because a plugin that
    /// is no longer enabled has no use for them and they would otherwise fail
    /// the build as stray options. An empty list turns every plugin off.
    pub fn set_enabled_from_list(&mut self, list: &str) {
        self.enable = list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        let enable = &self.enable;
        self.options.retain(|name, _| enable.contains(name));
    }

    /// Adds the plugins of `other` after this selection's, and takes its
    /// options over those of the same name.
    pub fn extend(&mut self, other: &Self) {
        self.enable.extend(other.enable.iter().cloned());
        for (name, options) in &other.options {
            self.options.insert(name.clone(), options.clone());
        }
    }

    /// The names of the plugins to enable, in order.
    #[must_use]
    pub fn enabled(&self) -> &[String] {
        &self.enable
    }

    /// The options given for the plugin called `name`, if any.
    #[must_use]
    pub fn options_of(&self, name: &str) -> Option<&Value> {
        self.options.get(name)
    }

    /// The names that have an option table, in alphabetical order.
    pub(crate) fn option_names(&self) -> impl Iterator<Item = &str> {
        self.options.keys().map(String::as_str)
    }

    /// Whether the selection turns nothing on and holds no options.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.enable.is_empty() && self.options.is_empty()
    }

    /// Fails when this selection asks for plugins and this build cannot run
    /// any, because the feature `plugins` is off.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] with the cargo feature that turns
    /// plugin support on.
    pub fn ensure_supported(&self) -> Result<(), SeleniumBaseError> {
        if cfg!(feature = "plugins") || self.is_empty() {
            return Ok(());
        }
        let asked: Vec<&str> = self
            .enable
            .iter()
            .map(String::as_str)
            .chain(self.option_names())
            .collect();
        Err(SeleniumBaseError::invalid_config(format!(
            "plugins were asked for ({}) but this build of seleniumbase-rs cannot run plugins; \
             rebuild with --features plugins-builtin for the built-in ones (or --features plugins \
             for the API alone), or remove the `[plugins]` table and SB_PLUGINS",
            asked.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_toml_table_has_the_names_in_order_and_one_options_table_per_plugin() {
        let selection: PluginSelection = toml::from_str(
            r#"
            enable = ["shield", "block-resources"]

            [shield]
            level = "strict"
            extra_block_hosts = ["tracker.example"]

            [block-resources]
            kinds = ["image"]
            "#,
        )
        .unwrap();

        assert_eq!(selection.enabled(), ["shield", "block-resources"]);
        assert_eq!(
            selection.options_of("shield"),
            Some(&json!({ "level": "strict", "extra_block_hosts": ["tracker.example"] }))
        );
        assert_eq!(
            selection.options_of("block-resources"),
            Some(&json!({ "kinds": ["image"] }))
        );
        assert_eq!(selection.options_of("crx"), None);
    }

    #[test]
    fn a_json_object_reads_the_same_way() {
        let selection: PluginSelection =
            serde_json::from_str(r#"{"enable": ["shield"], "shield": {"level": "strict"}}"#)
                .unwrap();

        assert_eq!(selection.enabled(), ["shield"]);
        assert_eq!(selection.options_of("shield").unwrap()["level"], "strict");
    }

    #[test]
    fn an_empty_table_selects_nothing() {
        let selection: PluginSelection = toml::from_str("").unwrap();
        assert!(selection.is_empty());
        assert_eq!(selection, PluginSelection::new());
    }

    #[test]
    fn a_comma_list_is_trimmed_and_empty_entries_are_dropped() {
        let selection = PluginSelection::from_list(" shield , ,block-resources,,");
        assert_eq!(selection.enabled(), ["shield", "block-resources"]);
        assert!(PluginSelection::from_list("").is_empty());
        assert!(PluginSelection::from_list(" , ").is_empty());
    }

    #[test]
    fn the_list_replaces_the_names_and_keeps_the_options() {
        let mut selection = PluginSelection::new()
            .enable("crx")
            .with_options("shield", json!({ "level": "strict" }));

        selection.set_enabled_from_list("shield");

        assert_eq!(selection.enabled(), ["shield"]);
        assert!(selection.options_of("shield").is_some());
        selection.set_enabled_from_list("");
        assert!(
            selection.enabled().is_empty(),
            "an empty list turns all off"
        );
    }

    #[test]
    fn the_list_drops_the_options_of_plugins_it_no_longer_names() {
        let mut selection = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "strict" }))
            .with_options("crx", json!({}));

        selection.set_enabled_from_list("crx");
        assert_eq!(selection.enabled(), ["crx"]);
        assert!(selection.options_of("shield").is_none());
        assert!(selection.options_of("crx").is_some());

        selection.set_enabled_from_list("");
        assert!(selection.is_empty(), "SB_PLUGINS= leaves nothing behind");
    }

    #[test]
    fn extending_appends_names_and_replaces_options_of_the_same_name() {
        let mut selection = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "basic" }));
        let other = PluginSelection::new()
            .enable("crx")
            .with_options("shield", json!({ "level": "strict" }));

        selection.extend(&other);

        assert_eq!(selection.enabled(), ["shield", "crx"]);
        assert_eq!(selection.options_of("shield").unwrap()["level"], "strict");
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn a_selection_is_supported_when_the_feature_is_on() {
        PluginSelection::new()
            .enable("shield")
            .ensure_supported()
            .unwrap();
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn without_the_feature_a_selection_fails_closed_naming_the_feature() {
        PluginSelection::new().ensure_supported().unwrap();

        for selection in [
            PluginSelection::new().enable("shield"),
            PluginSelection::new().with_options("shield", json!({})),
        ] {
            let error = selection.ensure_supported().unwrap_err();
            assert!(
                matches!(&error, SeleniumBaseError::InvalidConfig(message)
                    if message.contains("--features plugins-builtin")
                        && message.contains("shield")),
                "{error:?}"
            );
        }
    }
}
