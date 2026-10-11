//! Country codes, the locale table, and the geo resolver trait that picks a locale from the egress country.

#![expect(
    clippy::module_name_repetitions,
    reason = "the public names GeoError, GeoInfo and GeoResolver are fixed by the design plan"
)]

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::route::{EgressKey, EgressRef};
use crate::stealth::fingerprint::Fingerprint;
pub use crate::stealth::randomize::{Locale, LOCALES};

/// An ISO 3166-1 alpha-2 country code, upper case. Only a valid one can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CountryCode([u8; 2]);

impl CountryCode {
    /// Validates a two-letter code and upper-cases it.
    ///
    /// # Errors
    ///
    /// Returns [`GeoError::InvalidCountry`] unless `code` is exactly two ASCII letters.
    pub fn new(code: &str) -> Result<Self, GeoError> {
        match code.as_bytes() {
            [a, b] if a.is_ascii_alphabetic() && b.is_ascii_alphabetic() => {
                Ok(Self([a.to_ascii_uppercase(), b.to_ascii_uppercase()]))
            }
            _ => Err(GeoError::InvalidCountry(code.to_owned())),
        }
    }

    /// The code as text, such as `DE`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // The bytes are ASCII letters: `new` is the only way to build a value.
        std::str::from_utf8(&self.0).unwrap_or("??")
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for CountryCode {
    type Error = GeoError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<CountryCode> for String {
    fn from(code: CountryCode) -> Self {
        code.as_str().to_owned()
    }
}

/// Why a geo lookup or a country code was refused.
#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    /// The text is not a two-letter country code.
    #[error("`{0}` is not an ISO 3166-1 alpha-2 country code")]
    InvalidCountry(String),
    /// The locale table has no entry for this country.
    #[error("no locale is known for {0}")]
    NoLocale(CountryCode),
    /// The egress has no country, so nothing can be looked up for it.
    #[error("no geo information for egress {0}")]
    Unknown(EgressKey),
}

/// The first locale of `country` in [`LOCALES`]. The US has four time zones under one country
/// code; the first of them is returned.
#[must_use]
pub fn locale_for_country(country: CountryCode) -> Option<&'static Locale> {
    LOCALES
        .iter()
        .find(|locale| locale.country == country.as_str())
}

/// The locale whose tag is `tag`, such as `de-DE`.
#[must_use]
pub fn locale_by_tag(tag: &str) -> Option<&'static Locale> {
    LOCALES.iter().find(|locale| locale.tag == tag)
}

/// Sets the locale fields of `fingerprint` from `locale`: the language list, Accept-Language, the
/// time zone and the position of the city.
pub fn apply_locale(locale: &Locale, fingerprint: &mut Fingerprint) {
    let languages = locale.accept_language();
    fingerprint.locale = Some(locale.tag.to_owned());
    fingerprint.languages = Some(languages.clone());
    fingerprint.accept_languages = Some(languages);
    fingerprint.timezone = Some(locale.timezone.to_owned());
    fingerprint.latitude = Some(locale.latitude);
    fingerprint.longitude = Some(locale.longitude);
}

/// What a geo lookup learned about an egress.
#[derive(Clone, Debug, PartialEq)]
pub struct GeoInfo {
    /// The country of the egress.
    pub country: CountryCode,
    /// The IANA time zone, when known.
    pub timezone: Option<String>,
    /// Latitude and longitude, when known.
    pub coordinates: Option<(f64, f64)>,
}

/// Resolves the country, time zone and position of an egress. The shipped implementation,
/// [`StaticGeoResolver`], answers from the proxy spec and the locale table, and never looks up an
/// address.
#[async_trait]
pub trait GeoResolver: Send + Sync {
    /// Looks up the geo information for `egress`.
    ///
    /// # Errors
    ///
    /// Returns [`GeoError`] when the egress has no country or the country has no locale.
    async fn resolve(&self, egress: &EgressRef) -> Result<GeoInfo, GeoError>;
}

/// Answers from the country given with the proxy spec, through the locale table. No network lookup
/// ships.
#[derive(Clone, Copy, Debug, Default)]
pub struct StaticGeoResolver;

#[async_trait]
impl GeoResolver for StaticGeoResolver {
    async fn resolve(&self, egress: &EgressRef) -> Result<GeoInfo, GeoError> {
        let key = egress.key();
        let country = key.country.ok_or_else(|| GeoError::Unknown(key.clone()))?;
        let locale = locale_for_country(country).ok_or(GeoError::NoLocale(country))?;
        Ok(GeoInfo {
            country,
            timezone: Some(locale.timezone.to_owned()),
            coordinates: Some((locale.latitude, locale.longitude)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::OsType;
    use futures_util::FutureExt;

    #[test]
    fn a_country_code_is_two_letters_upper_cased() {
        assert_eq!(CountryCode::new("de").expect("valid").as_str(), "DE");
        for bad in ["", "D", "DEU", "1A", "D-"] {
            assert!(CountryCode::new(bad).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn a_country_code_round_trips_as_its_text() {
        let code: CountryCode = serde_json::from_str("\"de\"").expect("valid");
        assert_eq!(serde_json::to_string(&code).expect("serializes"), "\"DE\"");
        assert!(serde_json::from_str::<CountryCode>("\"DEU\"").is_err());
    }

    #[test]
    fn every_locale_has_a_valid_country_tag_and_iana_zone() {
        for locale in &LOCALES {
            assert!(
                CountryCode::new(locale.country).is_ok(),
                "{}",
                locale.country
            );
            assert!(
                locale.tag.contains('-'),
                "{} is not a BCP 47 tag",
                locale.tag
            );
            assert!(
                locale.timezone.contains('/'),
                "{} is not IANA",
                locale.timezone
            );
        }
    }

    #[test]
    fn the_table_is_looked_up_by_country_and_by_tag() {
        let de = CountryCode::new("DE").expect("valid");
        assert_eq!(
            locale_for_country(de).map(|l| l.timezone),
            Some("Europe/Berlin")
        );
        assert_eq!(locale_by_tag("ja-JP").map(|l| l.country), Some("JP"));
        let nowhere = CountryCode::new("ZZ").expect("valid");
        assert!(locale_for_country(nowhere).is_none());
    }

    #[test]
    fn applying_a_locale_sets_the_language_zone_and_position() {
        let mut fingerprint = Fingerprint::randomized(OsType::Windows, 3);
        let locale = locale_by_tag("de-DE").expect("present");
        apply_locale(locale, &mut fingerprint);
        assert_eq!(fingerprint.locale.as_deref(), Some("de-DE"));
        assert_eq!(fingerprint.timezone.as_deref(), Some("Europe/Berlin"));
        assert_eq!(
            fingerprint.accept_languages.as_deref(),
            Some("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")
        );
        assert!(fingerprint
            .latitude
            .is_some_and(|lat| (lat - 52.52).abs() < 1e-9));
    }

    // The resolver awaits nothing, so one poll finishes it. No runtime is needed, which keeps this
    // pure module free of tokio, even in its tests.
    #[test]
    fn the_static_resolver_answers_from_the_spec_country() {
        let de = CountryCode::new("DE").expect("valid");
        let egress = EgressRef::parse("proxy.example:8080", Some(de)).expect("valid spec");
        let info = StaticGeoResolver
            .resolve(&egress)
            .now_or_never()
            .expect("resolves in one poll")
            .expect("resolves");
        assert_eq!(info.country, de);
        assert_eq!(info.timezone.as_deref(), Some("Europe/Berlin"));
    }

    #[test]
    fn the_static_resolver_refuses_an_egress_without_a_country() {
        let egress = EgressRef::parse("proxy.example:8080", None).expect("valid spec");
        let err = StaticGeoResolver
            .resolve(&egress)
            .now_or_never()
            .expect("resolves in one poll")
            .expect_err("no country");
        assert!(matches!(err, GeoError::Unknown(_)));
    }
}
