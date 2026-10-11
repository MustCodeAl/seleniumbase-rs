//! Country codes, the locale table, and the geo resolver trait that picks a locale from the egress country.

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::route::{EgressKey, EgressRef};
use crate::stealth::fingerprint::Fingerprint;
use crate::stealth::humanize::Rng;
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

impl Locale {
    /// The first locale of `country` in [`LOCALES`]. The US has four time zones under one
    /// country code; the first of them is returned.
    #[must_use]
    pub fn for_country(country: CountryCode) -> Option<&'static Locale> {
        LOCALES
            .iter()
            .find(|locale| locale.country == country.as_str())
    }

    /// The locale whose tag is `tag`, such as `de-DE`. BCP 47 tags are case-insensitive, so
    /// `de-de` finds it too.
    #[must_use]
    pub fn by_tag(tag: &str) -> Option<&'static Locale> {
        LOCALES
            .iter()
            .find(|locale| locale.tag.eq_ignore_ascii_case(tag))
    }

    /// Sets the locale fields of `fingerprint` from this locale: the language list, the
    /// `Accept-Language` header, the time zone and a position near the city. The position is a
    /// function of the fingerprint's seed, so the same identity always reports the same spot.
    pub fn apply_to(&self, fingerprint: &mut Fingerprint) {
        let languages = self.accept_language();
        let place = self.position(&mut Rng::new(fingerprint.seed.unwrap_or_default()));
        fingerprint.locale = Some(self.tag.to_owned());
        fingerprint.languages = Some(languages.clone());
        fingerprint.accept_languages = Some(languages);
        fingerprint.timezone = Some(self.timezone.to_owned());
        fingerprint.latitude = Some(place.latitude);
        fingerprint.longitude = Some(place.longitude);
        fingerprint.accuracy = Some(place.accuracy);
    }
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
        let locale = Locale::for_country(country).ok_or(GeoError::NoLocale(country))?;
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

    fn country(code: &str) -> CountryCode {
        CountryCode::new(code).expect("valid code")
    }

    fn resolve(egress: &EgressRef) -> Result<GeoInfo, GeoError> {
        // The resolver awaits nothing, so one poll finishes it. No runtime is needed, which
        // keeps this pure module free of tokio, even in its tests.
        StaticGeoResolver
            .resolve(egress)
            .now_or_never()
            .expect("resolves in one poll")
    }

    #[test]
    fn a_country_code_is_two_letters_upper_cased() {
        assert_eq!(country("de").as_str(), "DE");
        assert_eq!(country("Gb").to_string(), "GB");
        for bad in ["", "D", "DEU", "1A", "D-", "é", "ÄÖ", " D", "D\n"] {
            assert!(CountryCode::new(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn a_rejected_code_is_named_in_the_error() {
        let err = CountryCode::new("DEU").expect_err("three letters");
        assert!(matches!(&err, GeoError::InvalidCountry(code) if code == "DEU"));
        assert!(err.to_string().contains("DEU"));
    }

    #[test]
    fn a_country_code_round_trips_as_its_text() {
        let code: CountryCode = serde_json::from_str("\"de\"").expect("valid");
        assert_eq!(serde_json::to_string(&code).expect("serializes"), "\"DE\"");
        assert!(serde_json::from_str::<CountryCode>("\"DEU\"").is_err());
        assert!(serde_json::from_str::<CountryCode>("42").is_err());
    }

    #[test]
    fn every_locale_has_a_valid_country_tag_and_iana_zone() {
        for locale in LOCALES {
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
    fn every_country_in_the_table_is_found_at_its_first_entry() {
        for locale in LOCALES {
            let found = Locale::for_country(country(locale.country)).expect("listed");
            let first = LOCALES
                .iter()
                .find(|l| l.country == locale.country)
                .expect("listed");
            assert_eq!(found, first, "{}", locale.country);
        }
    }

    #[test]
    fn the_table_is_looked_up_by_country_and_by_tag() {
        assert_eq!(
            Locale::for_country(country("DE")).map(|l| l.timezone),
            Some("Europe/Berlin")
        );
        assert_eq!(
            Locale::for_country(country("kr")).map(|l| l.timezone),
            Some("Asia/Seoul")
        );
        assert_eq!(Locale::by_tag("ja-JP").map(|l| l.country), Some("JP"));
        assert_eq!(Locale::by_tag("ja-jp").map(|l| l.country), Some("JP"));
        assert!(Locale::by_tag("xx-XX").is_none());
        assert!(Locale::for_country(country("ZZ")).is_none());
    }

    #[test]
    fn the_us_is_found_in_new_york_first() {
        let us = Locale::for_country(country("US")).expect("listed");
        assert_eq!(us.timezone, "America/New_York");
    }

    #[test]
    fn applying_a_locale_sets_the_language_zone_and_position() {
        let mut fingerprint = Fingerprint::randomized(OsType::Windows, 3);
        let locale = Locale::by_tag("de-DE").expect("present");
        locale.apply_to(&mut fingerprint);
        assert_eq!(fingerprint.locale.as_deref(), Some("de-DE"));
        assert_eq!(fingerprint.timezone.as_deref(), Some("Europe/Berlin"));
        assert_eq!(
            fingerprint.accept_languages.as_deref(),
            Some("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")
        );
        assert_eq!(fingerprint.languages, fingerprint.accept_languages);
        let (lat, long) = (
            fingerprint.latitude.expect("set"),
            fingerprint.longitude.expect("set"),
        );
        assert!((lat - 52.52).abs() <= 0.05, "{lat}");
        assert!((long - 13.405).abs() <= 0.05, "{long}");
        assert!(fingerprint.accuracy.is_some());
        assert!(fingerprint.validate().is_coherent());
    }

    #[test]
    fn the_position_is_a_function_of_the_seed() {
        let locale = Locale::by_tag("fr-FR").expect("present");
        let place = |seed| {
            let mut fingerprint = Fingerprint::randomized(OsType::Linux, seed);
            fingerprint.seed = Some(seed);
            locale.apply_to(&mut fingerprint);
            (fingerprint.latitude, fingerprint.longitude)
        };
        assert_eq!(place(11), place(11));
        assert_ne!(place(11), place(12));
    }

    #[test]
    fn a_locale_can_move_an_identity_to_any_listed_country() {
        for locale in LOCALES {
            let mut fingerprint = Fingerprint::randomized(OsType::Macos, 5);
            locale.apply_to(&mut fingerprint);
            assert_eq!(fingerprint.locale.as_deref(), Some(locale.tag));
            assert_eq!(fingerprint.timezone.as_deref(), Some(locale.timezone));
        }
    }

    #[test]
    fn the_static_resolver_answers_from_the_spec_country() {
        let de = country("DE");
        let egress = EgressRef::parse("proxy.example:8080", Some(de)).expect("valid spec");
        let info = resolve(&egress).expect("resolves");
        assert_eq!(info.country, de);
        assert_eq!(info.timezone.as_deref(), Some("Europe/Berlin"));
        assert_eq!(info.coordinates, Some((52.52, 13.405)));
    }

    #[test]
    fn the_static_resolver_refuses_an_egress_without_a_country() {
        let egress = EgressRef::parse("proxy.example:8080", None).expect("valid spec");
        let err = resolve(&egress).expect_err("no country");
        assert!(matches!(err, GeoError::Unknown(_)));
    }

    #[test]
    fn the_static_resolver_refuses_a_country_with_no_locale() {
        let zz = country("ZZ");
        let egress = EgressRef::parse("proxy.example:8080", Some(zz)).expect("valid spec");
        let err = resolve(&egress).expect_err("no locale");
        assert!(matches!(err, GeoError::NoLocale(code) if code == zz));
    }

    #[test]
    fn a_geo_error_names_the_egress_but_not_its_credentials() {
        let egress =
            EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("valid spec");
        let err = resolve(&egress).expect_err("no country");
        let shown = format!("{err} {err:?}");
        assert!(shown.contains("proxy.example"), "{shown}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
    }
}
