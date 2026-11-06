//! The layers of the settings: what the two providers [`EnvVars`](super::env_vars::EnvVars)
//! and [`SettingsFile`](super::settings_file::SettingsFile) have in common, and the one place
//! where [`figment`] merges them.
//!
//! The settings of a run come from up to four sources, each over the one before: the defaults,
//! the settings file, the `SB_*` variables, and explicit code (the pairs of `SettingsOverrides`,
//! or a plain assignment to the `Settings` that `load` returns). A source is read into a
//! [`Layer`], a dictionary with the settings it sets at the paths the serialized
//! [`Settings`] has (`browser.proxy.server`, `session.grid.port`). figment merges the layers
//! key by key, so a higher source wins for each setting it gives and leaves the rest alone, and
//! deserializes the result into a [`Settings`]. A layer also knows its name and how to give a
//! key path back as its source writes it, so an error can say where the value came from.
//!
//! Nothing in this module is public: no figment type is part of the API of the crate.

use std::collections::BTreeMap;

use figment::value::{Dict, Empty, Map, Tag, Value};
use figment::{Figment, Metadata, Profile, Provider};

use crate::config::settings::{Settings, SettingsProblem};
use crate::config::text;
use crate::error::SeleniumBaseError;

/// The settings one source gives, as a dictionary at the paths of the serialized [`Settings`],
/// and what the source is called in an error.
#[derive(Clone)]
pub(crate) struct Layer {
    metadata: Metadata,
    dict: Dict,
    /// The variables the source gave that no path holds by itself, to be worked out when the
    /// layers are merged.
    inputs: Inputs,
}

impl Layer {
    /// A layer named by `metadata` with the settings in `dict`.
    pub(crate) fn new(metadata: Metadata, dict: Dict) -> Self {
        Self {
            metadata,
            dict,
            inputs: Inputs::default(),
        }
    }

    /// This layer with the variables `inputs` that the source gave.
    pub(crate) fn with_inputs(mut self, inputs: Inputs) -> Self {
        self.inputs = inputs;
        self
    }

    /// The layer of the defaults: no setting, so that every group takes its `Default`.
    pub(crate) fn defaults() -> Self {
        Self::new(Metadata::named("defaults"), Dict::new())
    }

    /// The settings `settings` already have, as one layer: the lowest source when variables or
    /// explicit pairs are laid over a `Settings` that code built.
    pub(crate) fn of_settings(settings: &Settings) -> Result<Self, SeleniumBaseError> {
        match Value::serialize(settings).map_err(unserializable)? {
            Value::Dict(_, dict) => Ok(Self::new(Metadata::named("code"), dict)),
            other => Err(SeleniumBaseError::InvalidConfig(format!(
                "the settings serialize as {}, not as a table",
                other.to_actual()
            ))),
        }
    }

    /// Forgets the setting at `path`, or the nearest table above it that holds one, so that a
    /// value that was refused is left out of the next extraction. `false` when nothing at or
    /// above `path` is in the layer.
    pub(crate) fn forget(&mut self, path: &[String]) -> bool {
        (1..=path.len())
            .rev()
            .any(|len| forget_at(&mut self.dict, &path[..len]))
    }

    /// The setting at `path`, when it is a string.
    pub(crate) fn text_at(&self, path: &[String]) -> Option<&str> {
        let (last, parents) = path.split_last()?;
        let mut dict = &self.dict;
        for key in parents {
            dict = dict.get(key)?.as_dict()?;
        }
        dict.get(last)?.as_str()
    }

    /// Whether the layer holds no setting.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.dict.is_empty()
    }
}

impl Provider for Layer {
    fn metadata(&self) -> Metadata {
        self.metadata.clone()
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        Ok(Profile::Default.collect(self.dict.clone()))
    }
}

/// What a source gave for a variable of [`Inputs`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Given {
    /// The text of the variable, or of the flat key of a settings file.
    Text(String),
    /// The items of the array form of a flat key of a settings file
    /// (`chromium_arg = ["--a", "--b"]`).
    Items(Vec<String>),
}

/// The variables that no path of the settings holds by itself, as the layers gave them.
///
/// A setting that two variables write together (the virtual display and its size, the
/// DevTools events of every session and of the undetected ones, the Chromium switches and
/// the features they name) cannot be read one source at a time: half of it may come from the
/// file and half from the environment, and what a source's half shows in the settings (a size
/// with no display) may be nothing. So each source keeps the variables it gave, as it was
/// given, the source nearest the top that gives a variable decides it, and the setting is
/// worked out from the variables of all the sources once the layers are merged
/// ([`text::replay`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Inputs {
    given: BTreeMap<&'static str, Given>,
}

impl Inputs {
    /// Records what a source gave for `variable`; a later record replaces an earlier one.
    pub(crate) fn give(&mut self, variable: &'static str, given: Given) {
        self.given.insert(variable, given);
    }

    /// What the layers gave for `variable`, if anything.
    pub(crate) fn get(&self, variable: &str) -> Option<&Given> {
        self.given.get(variable)
    }

    /// The inputs `over` laid over these: for each variable `over` gives, its value replaces
    /// ours.
    pub(crate) fn lay(&mut self, over: &Self) {
        for (variable, given) in &over.given {
            self.given.insert(variable, given.clone());
        }
    }
}

/// A setting that a source refused, and the row of the option table it belongs to. The
/// problems of all sources are listed in the order of the rows, whichever source gave each.
pub(crate) struct Refused {
    /// The index of the row in the option table; the rows run in table order.
    pub(crate) row: usize,
    /// What is wrong and where it came from.
    pub(crate) problem: SettingsProblem,
}

/// The settings of `layers`, the first layer lowest, deserialized.
///
/// # Errors
///
/// The figment error of the first value that is not a setting of its type; it carries the
/// metadata of the layer the value came from and the path of the key.
pub(crate) fn extract(layers: &[&Layer]) -> Result<Settings, Box<figment::Error>> {
    let mut figment = Figment::new();
    let mut inputs = Inputs::default();
    for layer in layers {
        figment = figment.merge(*layer);
        inputs.lay(&layer.inputs);
    }
    let mut settings: Settings = figment.extract().map_err(Box::new)?;
    // A switch that a higher layer turned off is no entry: the map holds the switches that are on.
    settings.browser.switches.retain(|_, on| *on);
    // The settings that several variables write are worked out from the variables of all the
    // layers, now that the layers are merged.
    text::replay(&mut settings, &inputs)
        .map_err(|error| Box::new(figment::Error::from(error.to_string())))?;
    Ok(settings)
}

/// What a figment error says, naming its source: the layer's name, the key as that source
/// writes it (the interpolater of the layer's metadata), and what is wrong with the value.
pub(crate) fn describe(error: &figment::Error) -> String {
    let what = error.kind.to_string();
    match &error.metadata {
        Some(metadata) if !error.path.is_empty() => {
            let key = metadata.interpolate(&Profile::Default, &error.path);
            format!("{}: {key}: {what}", metadata.name)
        }
        Some(metadata) => format!("{}: {what}", metadata.name),
        None => what,
    }
}

/// Sets `path` (`browser.proxy.server`) in `dict` to `value`, making the tables on the way.
/// A value already at `path` is replaced; the tables beside it are kept.
pub(crate) fn put(dict: &mut Dict, path: &str, value: Value) {
    match path.split_once('.') {
        None => {
            dict.insert(path.to_owned(), value);
        }
        Some((head, rest)) => {
            let slot = dict
                .entry(head.to_owned())
                .or_insert_with(|| Value::Dict(Tag::Default, Dict::new()));
            if !matches!(slot, Value::Dict(..)) {
                *slot = Value::Dict(Tag::Default, Dict::new());
            }
            if let Value::Dict(_, inner) = slot {
                put(inner, rest, value);
            }
        }
    }
}

/// Lays `over` on `dict`, key by key: two tables are merged, anything else is replaced.
pub(crate) fn merge_into(dict: &mut Dict, over: Dict) {
    for (key, value) in over {
        match (dict.get_mut(&key), value) {
            (Some(Value::Dict(_, below)), Value::Dict(_, above)) => merge_into(below, above),
            (_, value) => {
                dict.insert(key, value);
            }
        }
    }
}

/// The value that stands for "nothing", which an `Option` reads as `None`.
pub(crate) fn nothing() -> Value {
    Value::Empty(Tag::Default, Empty::None)
}

/// A boolean value.
pub(crate) fn flag(on: bool) -> Value {
    Value::Bool(Tag::Default, on)
}

/// An empty list.
pub(crate) fn no_items() -> Value {
    Value::Array(Tag::Default, Vec::new())
}

fn forget_at(dict: &mut Dict, path: &[String]) -> bool {
    match path {
        [] => false,
        [last] => dict.remove(last).is_some(),
        [head, rest @ ..] => {
            let Some(Value::Dict(_, inner)) = dict.get_mut(head) else {
                return false;
            };
            let removed = forget_at(inner, rest);
            if removed && inner.is_empty() {
                dict.remove(head);
            }
            removed
        }
    }
}

fn unserializable(error: figment::Error) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!("the settings cannot be layered: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Value {
        Value::String(Tag::Default, value.to_owned())
    }

    #[test]
    fn a_path_makes_the_tables_on_the_way_and_keeps_the_ones_beside_it() {
        let mut dict = Dict::new();
        put(&mut dict, "browser.proxy.server", text("a"));
        put(&mut dict, "browser.proxy.pac_url", text("b"));
        put(&mut dict, "browser.lang", text("fr"));
        let browser = dict
            .get("browser")
            .and_then(Value::as_dict)
            .expect("a table");
        assert_eq!(browser.len(), 2);
        let proxy = browser
            .get("proxy")
            .and_then(Value::as_dict)
            .expect("a table");
        assert_eq!(proxy.len(), 2);
        // A whole table written over its parts replaces them.
        put(&mut dict, "browser.proxy", text("whole"));
        let browser = dict
            .get("browser")
            .and_then(Value::as_dict)
            .expect("a table");
        assert_eq!(browser.get("proxy").and_then(Value::as_str), Some("whole"));
    }

    #[test]
    fn a_higher_layer_wins_key_by_key_and_leaves_the_rest() {
        let mut low = Dict::new();
        put(&mut low, "browser.window_size.width", Value::from(800_u32));
        put(&mut low, "browser.window_size.height", Value::from(600_u32));
        let mut high = Dict::new();
        put(
            &mut high,
            "browser.window_size.width",
            Value::from(1024_u32),
        );
        merge_into(&mut low, high);
        let size = low
            .get("browser")
            .and_then(Value::as_dict)
            .and_then(|browser| browser.get("window_size"))
            .and_then(Value::as_dict)
            .expect("a table");
        assert_eq!(size.get("width").and_then(Value::to_u128), Some(1024));
        assert_eq!(size.get("height").and_then(Value::to_u128), Some(600));
    }

    #[test]
    fn the_inputs_of_a_higher_layer_replace_the_lower_ones_variable_by_variable() {
        let given = |text: &str| Given::Text(text.to_owned());
        let mut inputs = Inputs::default();
        inputs.give("SB_XVFB", given("true"));
        inputs.give("SB_XVFB_METRICS", given("800,600"));
        let mut over = Inputs::default();
        over.give("SB_XVFB_METRICS", given("1920,1080"));
        over.give("SB_CHROMIUM_ARG", Given::Items(vec!["--a".to_owned()]));
        inputs.lay(&over);
        // A variable the higher layer leaves alone is the lower layer's.
        assert_eq!(inputs.get("SB_XVFB"), Some(&given("true")));
        assert_eq!(inputs.get("SB_XVFB_METRICS"), Some(&given("1920,1080")));
        assert_eq!(
            inputs.get("SB_CHROMIUM_ARG"),
            Some(&Given::Items(vec!["--a".to_owned()]))
        );
        assert_eq!(inputs.get("SB_DISABLE_FEATURES"), None);
    }

    #[test]
    fn a_refused_key_is_forgotten_with_the_table_that_it_leaves_empty() {
        let mut dict = Dict::new();
        put(&mut dict, "run.demo_mode", text("later"));
        let mut layer = Layer::new(Metadata::named("test"), dict);
        assert!(layer.forget(&["run".to_owned(), "demo_mode".to_owned()]));
        assert!(layer.is_empty());
        assert!(!layer.forget(&["run".to_owned(), "demo_mode".to_owned()]));
    }
}
