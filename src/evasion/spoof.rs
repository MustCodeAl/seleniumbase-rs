//! The four states of one fingerprint setting.
//!
//! A [`Fingerprint`](crate::evasion::fingerprint::Fingerprint) setting is not
//! a value plus a flag that says whether to use it. It is one [`Spoof`], and the
//! state says everything: leave the browser's own value, draw a random one, use
//! this one, or turn the API off.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::evasion::spoof::Spoof;
//!
//! // A plain value is `Value`, so passing one is all the intent there is.
//! let zone: Spoof<String> = "Europe/Berlin".into();
//! assert_eq!(zone.as_str(), Some("Europe/Berlin"));
//!
//! // The other three states are named.
//! assert!(Spoof::<String>::default().is_natural());
//! assert!(Spoof::<String>::Random.is_random());
//! assert!(Spoof::<String>::Off.is_off());
//! ```

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One fingerprint setting: what the browser should report for it.
///
/// `Random` is resolved when a fingerprint is built (see
/// [`FingerprintBuilder::build`](crate::evasion::fingerprint::FingerprintBuilder::build)),
/// so a built fingerprint holds `Natural`, `Value` or `Off`.
///
/// In JSON a `Spoof` is `"natural"`, `"random"`, `"off"` or the value itself;
/// `null` reads as `Natural`. For a string setting, those three words are
/// reserved: a value spelled like one of them reads as the keyword.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Spoof<T> {
    /// The browser's own value, left alone. This is the default.
    Natural,
    /// A random valid value, coherent with the rest of the fingerprint (same
    /// operating system, region and locale), drawn from the builder's seed.
    Random,
    /// Exactly this value.
    Value(T),
    /// The API is turned off: unavailable, or denied. Only some settings have
    /// an off state; building a fingerprint with `Off` on any other is an error
    /// that names the setting.
    Off,
}

/// The default is [`Natural`](Spoof::Natural), whatever `T` is.
impl<T> Default for Spoof<T> {
    fn default() -> Self {
        Self::Natural
    }
}

impl<T> Spoof<T> {
    /// Whether this is [`Natural`](Spoof::Natural): the browser's own value.
    #[must_use]
    pub const fn is_natural(&self) -> bool {
        matches!(self, Self::Natural)
    }

    /// Whether this is [`Random`](Spoof::Random).
    #[must_use]
    pub const fn is_random(&self) -> bool {
        matches!(self, Self::Random)
    }

    /// Whether this is [`Value`](Spoof::Value).
    #[must_use]
    pub const fn is_value(&self) -> bool {
        matches!(self, Self::Value(_))
    }

    /// Whether this is [`Off`](Spoof::Off).
    #[must_use]
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }

    /// Whether the setting changes anything: every state but `Natural`.
    #[must_use]
    pub const fn is_set(&self) -> bool {
        !self.is_natural()
    }

    /// The value, if this is a [`Value`](Spoof::Value).
    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    /// The value by move, if this is a [`Value`](Spoof::Value).
    #[must_use]
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    /// The same state, borrowing the value.
    #[must_use]
    pub const fn as_ref(&self) -> Spoof<&T> {
        match self {
            Self::Natural => Spoof::Natural,
            Self::Random => Spoof::Random,
            Self::Value(value) => Spoof::Value(value),
            Self::Off => Spoof::Off,
        }
    }

    /// The same state with the value mapped through `f`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spoof<U> {
        match self {
            Self::Natural => Spoof::Natural,
            Self::Random => Spoof::Random,
            Self::Value(value) => Spoof::Value(f(value)),
            Self::Off => Spoof::Off,
        }
    }

    /// The word for the state without a value, or `None` for a value.
    pub(crate) const fn word(&self) -> Option<&'static str> {
        match self {
            Self::Natural => Some("natural"),
            Self::Random => Some("random"),
            Self::Off => Some("off"),
            Self::Value(_) => None,
        }
    }
}

impl Spoof<String> {
    /// The value as text, if this is a [`Value`](Spoof::Value).
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.value().map(String::as_str)
    }
}

/// A plain value is the intent to use it.
impl<T> From<T> for Spoof<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl From<&str> for Spoof<String> {
    fn from(value: &str) -> Self {
        Self::Value(value.to_owned())
    }
}

impl From<&String> for Spoof<String> {
    fn from(value: &String) -> Self {
        Self::Value(value.clone())
    }
}

impl From<Cow<'_, str>> for Spoof<String> {
    fn from(value: Cow<'_, str>) -> Self {
        Self::Value(value.into_owned())
    }
}

impl From<Vec<&str>> for Spoof<Vec<String>> {
    fn from(values: Vec<&str>) -> Self {
        Self::Value(values.into_iter().map(str::to_owned).collect())
    }
}

impl<const N: usize> From<[&str; N]> for Spoof<Vec<String>> {
    fn from(values: [&str; N]) -> Self {
        Self::Value(values.into_iter().map(str::to_owned).collect())
    }
}

impl<T: Serialize> Serialize for Spoof<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            other => serializer.serialize_str(other.word().unwrap_or("natural")),
        }
    }
}

/// The JSON Schema of a setting: a keyword, `null`, or the value. Only the library's
/// own tests write schemas (see `stealth::fingerprint::schema`).
#[cfg(test)]
impl<T: schemars::JsonSchema> schemars::JsonSchema for Spoof<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Spoof_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("{}::Spoof<{}>", module_path!(), T::schema_id()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                { "type": "string", "enum": ["natural", "random", "off"] },
                { "type": "null" },
                generator.subschema_for::<T>()
            ]
        })
    }
}

/// A keyword of the JSON form.
enum Word {
    Natural,
    Random,
    Off,
}

impl<'de> Deserialize<'de> for Word {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match text.to_ascii_lowercase().as_str() {
            "natural" => Ok(Self::Natural),
            "random" => Ok(Self::Random),
            "off" => Ok(Self::Off),
            _ => Err(serde::de::Error::custom("not a spoof keyword")),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Spoof<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire<T> {
            Word(Word),
            Value(T),
        }
        Ok(match Option::<Wire<T>>::deserialize(deserializer)? {
            None | Some(Wire::Word(Word::Natural)) => Self::Natural,
            Some(Wire::Word(Word::Random)) => Self::Random,
            Some(Wire::Word(Word::Off)) => Self::Off,
            Some(Wire::Value(value)) => Self::Value(value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_natural_until_it_is_told_otherwise() {
        let setting: Spoof<u32> = Spoof::default();

        assert!(setting.is_natural());
        assert!(!setting.is_set());
        assert_eq!(setting.value(), None);
    }

    #[test]
    fn a_plain_value_is_the_intent_to_use_it() {
        let cores: Spoof<u32> = 8.into();
        let zone: Spoof<String> = "Europe/Berlin".into();
        let owned: Spoof<String> = String::from("Asia/Tokyo").into();
        let fonts: Spoof<Vec<String>> = ["Arial", "Calibri"].into();

        assert_eq!(cores, Spoof::Value(8));
        assert_eq!(zone.as_str(), Some("Europe/Berlin"));
        assert_eq!(owned.as_str(), Some("Asia/Tokyo"));
        assert_eq!(fonts.value().map(Vec::len), Some(2));
        assert!(cores.is_set() && cores.is_value());
    }

    #[test]
    fn the_four_states_are_told_apart() {
        let states: [Spoof<u32>; 4] = [Spoof::Natural, Spoof::Random, Spoof::Value(1), Spoof::Off];

        let kinds: Vec<_> = states
            .iter()
            .map(|s| (s.is_natural(), s.is_random(), s.is_value(), s.is_off()))
            .collect();

        assert_eq!(
            kinds,
            vec![
                (true, false, false, false),
                (false, true, false, false),
                (false, false, true, false),
                (false, false, false, true),
            ]
        );
        assert_eq!(
            states.iter().map(Spoof::is_set).collect::<Vec<_>>(),
            vec![false, true, true, true]
        );
    }

    #[test]
    fn map_keeps_the_state_and_changes_the_value() {
        assert_eq!(Spoof::Value(2).map(|n| n * 2), Spoof::Value(4));
        assert_eq!(Spoof::<u32>::Off.map(|n| n * 2), Spoof::Off);
        assert_eq!(Spoof::Value(3).as_ref(), Spoof::Value(&3));
        assert_eq!(Spoof::Value(3).into_value(), Some(3));
        assert_eq!(Spoof::<u32>::Random.into_value(), None);
    }

    #[test]
    fn json_writes_a_keyword_or_the_value() {
        let natural = serde_json::to_string(&Spoof::<String>::Natural).unwrap();
        let random = serde_json::to_string(&Spoof::<String>::Random).unwrap();
        let off = serde_json::to_string(&Spoof::<String>::Off).unwrap();
        let value = serde_json::to_string(&Spoof::Value("Europe/Berlin".to_owned())).unwrap();
        let pair = serde_json::to_string(&Spoof::Value(vec![1, 2])).unwrap();

        assert_eq!(natural, r#""natural""#);
        assert_eq!(random, r#""random""#);
        assert_eq!(off, r#""off""#);
        assert_eq!(value, r#""Europe/Berlin""#);
        assert_eq!(pair, "[1,2]");
    }

    #[test]
    fn json_reads_a_keyword_in_any_case_or_the_value_or_null() {
        let read = |text: &str| serde_json::from_str::<Spoof<String>>(text).unwrap();

        assert_eq!(read(r#""natural""#), Spoof::Natural);
        assert_eq!(read(r#""Random""#), Spoof::Random);
        assert_eq!(read(r#""OFF""#), Spoof::Off);
        assert_eq!(read("null"), Spoof::Natural);
        assert_eq!(
            read(r#""Europe/Berlin""#),
            Spoof::Value("Europe/Berlin".into())
        );
    }

    #[test]
    fn json_reads_numbers_lists_and_objects_as_values() {
        let cores: Spoof<u32> = serde_json::from_str("8").unwrap();
        let ratio: Spoof<f64> = serde_json::from_str("2.5").unwrap();
        let fonts: Spoof<Vec<String>> = serde_json::from_str(r#"["Arial"]"#).unwrap();
        let none: Spoof<Vec<String>> = serde_json::from_str(r#""off""#).unwrap();

        assert_eq!(cores, Spoof::Value(8));
        assert_eq!(ratio, Spoof::Value(2.5));
        assert_eq!(fonts, Spoof::Value(vec!["Arial".to_owned()]));
        assert_eq!(none, Spoof::Off);
    }

    #[test]
    fn json_round_trips_every_state() {
        for state in [
            Spoof::Natural,
            Spoof::Random,
            Spoof::Value("x".to_owned()),
            Spoof::Off,
        ] {
            let text = serde_json::to_string(&state).unwrap();
            assert_eq!(serde_json::from_str::<Spoof<String>>(&text).unwrap(), state);
        }
    }

    #[test]
    fn a_string_that_is_not_a_keyword_is_never_mistaken_for_one() {
        let read: Spoof<String> = serde_json::from_str(r#""offline""#).unwrap();

        assert_eq!(read, Spoof::Value("offline".into()));
    }
}
