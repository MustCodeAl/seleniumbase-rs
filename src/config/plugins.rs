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
//! The command-line flags `--plugin NAME` and `--plugin-opt NAME.KEY=VALUE` of
//! `sbrs` and `sbrs-mcp` are applied to a selection read from the file and the
//! variable by [`PluginSelection::apply_flags`], so the precedence is the same
//! for every program: the command line over `SB_PLUGINS` over the file.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
        self.set_enabled_keeping_options(list);
        self.prune_unused();
    }

    /// [`set_enabled_from_list`](Self::set_enabled_from_list) that keeps every
    /// option table, including those of the plugins the list no longer names.
    ///
    /// A program that applies command-line flags after the variable uses this,
    /// so that a plugin the flags turn on again keeps the options of the
    /// settings file, and calls [`prune_unused`](Self::prune_unused) when the
    /// flags are applied.
    pub fn set_enabled_keeping_options(&mut self, list: &str) {
        self.enable = list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
    }

    /// Drops the options of the plugins that are not enabled: a plugin nothing
    /// turns on has no use for them, and they would otherwise fail the build
    /// as stray options.
    pub fn prune_unused(&mut self) {
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

    /// Adds `name` after the plugins already enabled, unless it is enabled
    /// already, in which case it keeps its place. [`enable`](Self::enable)
    /// adds a name twice, which the build then refuses.
    pub fn enable_once(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.enable.contains(&name) {
            self.enable.push(name);
        }
    }

    /// Applies what the command line asked for to a selection that was read
    /// from the settings file and `SB_PLUGINS`: the names of every `--plugin
    /// NAME`, then every `--plugin-opt NAME.KEY=VALUE`.
    ///
    /// The precedence is the command line over `SB_PLUGINS` over the file.
    /// A `--plugin` name is added after the plugins already enabled (one that
    /// is enabled already stays where it is), and a `--plugin-opt` sets one
    /// option of one plugin, replacing the same option from the file and
    /// keeping the plugin's other options. `VALUE` is a TOML value, so a
    /// string is written with quotes: `shield.level="strict"`,
    /// `block-resources.preset="media"`, `shield.extra_block_hosts=["a.example"]`.
    /// `KEY` is the option's name; everything after the first `.` and before
    /// the `=`.
    ///
    /// ```
    /// use seleniumbase_rs::config::plugins::PluginSelection;
    ///
    /// let mut selection = PluginSelection::from_list("shield");
    /// selection.apply_flags(&["crx"], &[r#"shield.level="strict""#])?;
    /// assert_eq!(selection.enabled(), ["shield", "crx"]);
    /// assert_eq!(selection.options_of("shield").unwrap()["level"], "strict");
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] for a plugin name that is empty or
    /// has a space or a comma in it, an option that is not `NAME.KEY=VALUE`
    /// with a TOML `VALUE`, an option of a plugin that nothing turns on, and
    /// the same option given twice.
    pub fn apply_flags<N, O>(
        &mut self,
        plugins: &[N],
        options: &[O],
    ) -> Result<(), SeleniumBaseError>
    where
        N: AsRef<str>,
        O: AsRef<str>,
    {
        for name in plugins {
            let name = name.as_ref();
            if name.is_empty() || name.contains([',', ' ', '\t']) {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "`--plugin {name}` is not a plugin name: give one name per --plugin and \
                     repeat the flag for more (`--plugin shield --plugin crx`)"
                )));
            }
            self.enable_once(name);
        }
        let mut given = BTreeSet::new();
        for text in options {
            let text = text.as_ref();
            let (name, key, value) = parse_option_flag(text)?;
            if !self.enable.contains(&name) {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "`--plugin-opt {text}` sets an option of `{name}`, which nothing turns on: add \
                     `--plugin {name}`, list it in SB_PLUGINS, or put it in `enable` under [plugins]"
                )));
            }
            if !given.insert((name.clone(), key.clone())) {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "`--plugin-opt {name}.{key}=...` was given twice; give each option once"
                )));
            }
            self.set_option(&name, key, value)?;
        }
        Ok(())
    }

    /// Sets the option `key` of the plugin called `name`, keeping its other
    /// options.
    fn set_option(
        &mut self,
        name: &str,
        key: String,
        value: Value,
    ) -> Result<(), SeleniumBaseError> {
        let options = self
            .options
            .entry(name.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if options.is_null() {
            *options = Value::Object(Map::new());
        }
        match options {
            Value::Object(map) => {
                map.insert(key, value);
                Ok(())
            }
            other => Err(SeleniumBaseError::invalid_config(format!(
                "the options of `{name}` are {other}, not a table, so `--plugin-opt` cannot set \
                 `{key}` in them"
            ))),
        }
    }

    /// The enabled names as a comma-separated list: the value of
    /// `SB_PLUGINS` that enables the same plugins, and the inverse of
    /// [`from_list`](Self::from_list).
    #[must_use]
    pub fn to_list(&self) -> String {
        self.enable.join(",")
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

/// The plugin, option and value of one `--plugin-opt NAME.KEY=VALUE`.
///
/// `NAME` ends at the first `.` and the text before the first `=` ends `KEY`;
/// `VALUE` is a TOML value.
fn parse_option_flag(text: &str) -> Result<(String, String, Value), SeleniumBaseError> {
    let malformed = || {
        SeleniumBaseError::invalid_config(format!(
            "`--plugin-opt {text}` is not NAME.KEY=VALUE: give the plugin, the option and a TOML \
             value, such as --plugin-opt 'shield.level=\"strict\"'"
        ))
    };
    let (target, raw) = text.split_once('=').ok_or_else(malformed)?;
    let (name, key) = target.split_once('.').ok_or_else(malformed)?;
    if name.is_empty() || key.is_empty() || raw.trim().is_empty() {
        return Err(malformed());
    }
    let value = parse_toml_value(raw).map_err(|why| {
        // A bare word is the usual mistake: the shell strips the quotes of
        // `shield.level="strict"` unless the whole flag is in single quotes.
        let bare = raw.trim();
        let hint = if bare
            .chars()
            .all(|c| c.is_alphanumeric() || "_-./:@+".contains(c))
        {
            format!("; write a string with quotes, as --plugin-opt '{name}.{key}=\"{bare}\"'")
        } else {
            String::new()
        };
        SeleniumBaseError::invalid_config(format!(
            "the value of `--plugin-opt {text}` is not a TOML value ({why}){hint}"
        ))
    })?;
    Ok((name.to_owned(), key.to_owned(), value))
}

/// The TOML value `raw` holds, as the JSON value options are kept as.
///
/// `raw` is parsed as the right-hand side of `value = ...`, and anything that
/// would add a second key or a table to that document is refused.
fn parse_toml_value(raw: &str) -> Result<Value, String> {
    let mut document: toml::Table =
        toml::from_str(&format!("value = {raw}")).map_err(|error| error.message().to_owned())?;
    match (document.remove("value"), document.is_empty()) {
        (Some(value), true) => json_of(value),
        _ => Err("it has more than one value".to_owned()),
    }
}

/// `value` as JSON, which has no dates and no `nan` or infinity.
fn json_of(value: toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(number) => Value::from(number),
        toml::Value::Float(number) => serde_json::Number::from_f64(number)
            .map(Value::Number)
            .ok_or_else(|| format!("{number} cannot be an option"))?,
        toml::Value::Boolean(flag) => Value::Bool(flag),
        toml::Value::Datetime(date) => return Err(format!("a date ({date}) cannot be an option")),
        toml::Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(json_of)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        toml::Value::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(key, value)| json_of(value).map(|value| (key, value)))
                .collect::<Result<Map<_, _>, _>>()?,
        ),
    })
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
    fn keeping_the_options_until_the_flags_are_applied_lets_a_flag_bring_them_back() {
        let mut selection: PluginSelection = toml::from_str(
            r#"
            enable = ["shield"]

            [shield]
            level = "strict"
            "#,
        )
        .unwrap();

        selection.set_enabled_keeping_options("crx");
        assert_eq!(selection.enabled(), ["crx"]);
        assert!(
            selection.options_of("shield").is_some(),
            "the options wait for the flags"
        );

        selection.apply_flags(&["shield"], &[] as &[&str]).unwrap();
        selection.prune_unused();
        assert_eq!(selection.enabled(), ["crx", "shield"]);
        assert_eq!(
            selection.options_of("shield").unwrap()["level"],
            "strict",
            "the flag turned shield on again with the file's level"
        );
    }

    #[test]
    fn pruning_drops_the_options_of_plugins_nothing_turned_on() {
        let mut selection = PluginSelection::new()
            .enable("crx")
            .with_options("shield", json!({ "level": "strict" }))
            .with_options("crx", json!({}));

        selection.prune_unused();

        assert!(selection.options_of("shield").is_none());
        assert!(selection.options_of("crx").is_some());
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

    #[test]
    fn enabling_once_keeps_the_first_place_and_adds_a_new_name_last() {
        let mut selection = PluginSelection::from_list("shield,crx");

        selection.enable_once("shield");
        selection.enable_once("block-resources");
        selection.enable_once("block-resources");

        assert_eq!(selection.enabled(), ["shield", "crx", "block-resources"]);
    }

    #[test]
    fn flags_add_names_after_the_selection_and_override_options_key_by_key() {
        let mut selection: PluginSelection = toml::from_str(
            r#"
            enable = ["shield"]

            [shield]
            level = "basic"
            extra_block_hosts = ["tracker.example"]
            "#,
        )
        .unwrap();

        selection
            .apply_flags(
                &["crx", "shield"],
                &[r#"shield.level="strict""#, "crx.keep=true"],
            )
            .unwrap();

        assert_eq!(selection.enabled(), ["shield", "crx"]);
        assert_eq!(
            selection.options_of("shield"),
            Some(&json!({ "level": "strict", "extra_block_hosts": ["tracker.example"] })),
            "the flag replaces `level` and leaves the other option of the file"
        );
        assert_eq!(
            selection.options_of("crx"),
            Some(&json!({ "keep": true })),
            "a plugin the file did not configure gets a table of its own"
        );
    }

    #[test]
    fn no_flags_change_nothing() {
        let mut selection = PluginSelection::from_list("shield");
        let before = selection.clone();
        selection.apply_flags::<&str, &str>(&[], &[]).unwrap();
        assert_eq!(selection, before);
    }

    #[test]
    fn an_option_flag_takes_any_toml_value() {
        let mut selection = PluginSelection::from_list("p");
        selection
            .apply_flags(
                &[] as &[&str],
                &[
                    r#"p.text="two words = ok""#,
                    "p.count=3",
                    "p.ratio=1.5",
                    "p.on=false",
                    r#"p.list=["a.example", "b.example"]"#,
                    r#"p.table={ kind = "x", depth = 2 }"#,
                    "p.empty=[]",
                    "p.negative=-4",
                ],
            )
            .unwrap();

        assert_eq!(
            selection.options_of("p"),
            Some(&json!({
                "text": "two words = ok",
                "count": 3,
                "ratio": 1.5,
                "on": false,
                "list": ["a.example", "b.example"],
                "table": { "kind": "x", "depth": 2 },
                "empty": [],
                "negative": -4,
            }))
        );
    }

    #[test]
    fn an_option_flag_splits_at_the_first_dot_and_the_first_equals_sign() {
        let mut selection = PluginSelection::from_list("block-resources");
        selection
            .apply_flags(&[] as &[&str], &[r#"block-resources.a.b="x=y""#])
            .unwrap();
        assert_eq!(
            selection.options_of("block-resources"),
            Some(&json!({ "a.b": "x=y" })),
            "the key is everything between the first dot and the first equals sign"
        );
    }

    fn flag_error(selection: &mut PluginSelection, plugins: &[&str], options: &[&str]) -> String {
        match selection.apply_flags(plugins, options).unwrap_err() {
            SeleniumBaseError::InvalidConfig(message) => message,
            other => panic!("expected an invalid configuration, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_option_flag_is_refused_with_what_was_wrong() {
        for (flag, wanted) in [
            ("shield.level", "NAME.KEY=VALUE"),
            ("level=1", "NAME.KEY=VALUE"),
            (".level=1", "NAME.KEY=VALUE"),
            ("shield.=1", "NAME.KEY=VALUE"),
            ("shield.level=", "NAME.KEY=VALUE"),
            ("=1", "NAME.KEY=VALUE"),
        ] {
            let mut selection = PluginSelection::from_list("shield");
            let message = flag_error(&mut selection, &[], &[flag]);
            assert!(message.contains(wanted), "{flag}: {message}");
            assert!(message.contains(flag), "{flag}: {message}");
        }
    }

    #[test]
    fn a_value_that_is_not_toml_is_refused_and_a_bare_word_is_told_to_use_quotes() {
        let mut selection = PluginSelection::from_list("shield");
        let message = flag_error(&mut selection, &[], &["shield.level=strict"]);
        assert!(message.contains("not a TOML value"), "{message}");
        assert!(
            message.contains(r#"shield.level="strict""#),
            "the message shows the quoted spelling: {message}"
        );

        for flag in [
            "shield.a=1 2",
            "shield.a=[1,",
            "shield.a=1\nb = 2",
            "shield.a=1\n[table]",
        ] {
            let mut selection = PluginSelection::from_list("shield");
            let message = flag_error(&mut selection, &[], &[flag]);
            assert!(message.contains("not a TOML value"), "{flag:?}: {message}");
            assert_eq!(
                selection.options_of("shield"),
                None,
                "{flag:?} changed the selection"
            );
        }
    }

    #[test]
    fn values_json_cannot_hold_are_refused() {
        for flag in ["p.when=2026-10-17", "p.x=nan", "p.x=inf", "p.x=-inf"] {
            let mut selection = PluginSelection::from_list("p");
            let message = flag_error(&mut selection, &[], &[flag]);
            assert!(message.contains(flag), "{flag}: {message}");
        }
    }

    #[test]
    fn an_option_of_a_plugin_nothing_turns_on_says_how_to_turn_it_on() {
        let mut selection = PluginSelection::from_list("crx");

        let message = flag_error(&mut selection, &[], &[r#"shield.level="strict""#]);

        assert!(message.contains("`shield`"), "{message}");
        assert!(message.contains("--plugin shield"), "{message}");
        assert!(message.contains("SB_PLUGINS"), "{message}");
        assert_eq!(selection.options_of("shield"), None);

        // The same flags with the plugin named: fine, and the order does not matter.
        let mut selection = PluginSelection::new();
        selection
            .apply_flags(&["shield"], &[r#"shield.level="strict""#])
            .unwrap();
    }

    #[test]
    fn the_same_option_twice_in_the_flags_is_refused() {
        let mut selection = PluginSelection::from_list("shield");
        let message = flag_error(
            &mut selection,
            &[],
            &[r#"shield.level="basic""#, r#"shield.level="strict""#],
        );
        assert!(message.contains("shield.level"), "{message}");
        assert!(message.contains("twice"), "{message}");
    }

    #[test]
    fn a_plugin_flag_names_one_plugin() {
        for name in ["", "shield,crx", "shield crx"] {
            let mut selection = PluginSelection::new();
            let message = flag_error(&mut selection, &[name], &[]);
            assert!(message.contains("--plugin"), "{name:?}: {message}");
            assert!(selection.is_empty(), "{name:?} changed the selection");
        }
    }

    #[test]
    fn options_that_are_not_a_table_cannot_take_a_flag() {
        let mut selection =
            PluginSelection::from_list("shield").with_options("shield", json!("not a table"));
        let message = flag_error(&mut selection, &[], &["shield.level=1"]);
        assert!(message.contains("not a table"), "{message}");
    }

    #[test]
    fn the_list_form_is_the_inverse_of_from_list() {
        let selection = PluginSelection::from_list("shield, crx");
        assert_eq!(selection.to_list(), "shield,crx");
        assert_eq!(PluginSelection::from_list(&selection.to_list()), selection);
        assert_eq!(PluginSelection::new().to_list(), "");
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
