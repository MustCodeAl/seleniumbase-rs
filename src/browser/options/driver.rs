//! Which version of the browser driver to use.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// A Chrome release channel, as Chrome for Testing publishes them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ReleaseChannel {
    /// The newest stable release.
    Stable,
    /// The newest beta.
    Beta,
    /// The newest dev build.
    Dev,
    /// The newest canary build.
    Canary,
}

impl ReleaseChannel {
    /// The channel's name in the Chrome for Testing listings.
    #[must_use]
    pub const fn listing_name(self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Beta => "Beta",
            Self::Dev => "Dev",
            Self::Canary => "Canary",
        }
    }
}

/// Which chromedriver to run.
///
/// Python SeleniumBase takes it as `--driver-version=VERSION` (and
/// `driver_version=` in its Python API). A chromedriver only drives the Chrome
/// of its own major version, so the choice matters.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::{DriverVersion, ReleaseChannel};
///
/// assert_eq!("browser".parse::<DriverVersion>()?, DriverVersion::MatchBrowser);
/// assert_eq!("beta".parse::<DriverVersion>()?, DriverVersion::Channel(ReleaseChannel::Beta));
/// assert_eq!("126".parse::<DriverVersion>()?, DriverVersion::Milestone(126));
/// assert_eq!(
///     "126.0.6478.126".parse::<DriverVersion>()?,
///     DriverVersion::Exact("126.0.6478.126".into())
/// );
/// assert!("v126".parse::<DriverVersion>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum DriverVersion {
    /// The driver for the major version of the Chrome that is installed
    /// (`browser`, also `chrome`).
    MatchBrowser,
    /// The newest driver of a release channel (`stable` or `latest`, `beta`,
    /// `dev`, `canary`).
    Channel(ReleaseChannel),
    /// The newest driver of one Chrome major version (`126`).
    Milestone(u32),
    /// One build, by its full version (`126.0.6478.126`).
    Exact(String),
}

impl fmt::Display for DriverVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatchBrowser => f.write_str("browser"),
            Self::Channel(ReleaseChannel::Stable) => f.write_str("stable"),
            Self::Channel(ReleaseChannel::Beta) => f.write_str("beta"),
            Self::Channel(ReleaseChannel::Dev) => f.write_str("dev"),
            Self::Channel(ReleaseChannel::Canary) => f.write_str("canary"),
            Self::Milestone(major) => write!(f, "{major}"),
            Self::Exact(version) => f.write_str(version),
        }
    }
}

impl FromStr for DriverVersion {
    type Err = SeleniumBaseError;

    /// Reads a Python `--driver-version` value: `browser`, `latest`,
    /// `stable`, `beta`, `dev`, `canary`, a major version such as `126`, or a
    /// full version of four dot-separated numbers.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wanted = text.trim().to_ascii_lowercase();
        match wanted.as_str() {
            "browser" | "chrome" => return Ok(Self::MatchBrowser),
            "latest" | "stable" => return Ok(Self::Channel(ReleaseChannel::Stable)),
            "beta" => return Ok(Self::Channel(ReleaseChannel::Beta)),
            "dev" => return Ok(Self::Channel(ReleaseChannel::Dev)),
            "canary" => return Ok(Self::Channel(ReleaseChannel::Canary)),
            _ => {}
        }
        let invalid = || {
            SeleniumBaseError::parse(
                text,
                "browser, latest, stable, beta, dev, canary, a major version like 126, \
                 or a full version like 126.0.6478.126",
            )
        };
        let numbers: Vec<Option<u32>> = wanted
            .split('.')
            .map(|part| {
                (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| part.parse().ok())
                    .flatten()
            })
            .collect();
        match numbers.as_slice() {
            [Some(major)] => Ok(Self::Milestone(*major)),
            [Some(_), Some(_), Some(_), Some(_)] => Ok(Self::Exact(wanted)),
            _ => Err(invalid()),
        }
    }
}

impl TryFrom<String> for DriverVersion {
    type Error = SeleniumBaseError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<DriverVersion> for String {
    fn from(version: DriverVersion) -> Self {
        version.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_python_documents_are_read() {
        let cases = [
            ("browser", DriverVersion::MatchBrowser),
            ("Chrome", DriverVersion::MatchBrowser),
            ("latest", DriverVersion::Channel(ReleaseChannel::Stable)),
            ("stable", DriverVersion::Channel(ReleaseChannel::Stable)),
            ("BETA", DriverVersion::Channel(ReleaseChannel::Beta)),
            ("dev", DriverVersion::Channel(ReleaseChannel::Dev)),
            (" canary ", DriverVersion::Channel(ReleaseChannel::Canary)),
            ("114", DriverVersion::Milestone(114)),
            (
                "126.0.6478.126",
                DriverVersion::Exact("126.0.6478.126".into()),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(text.parse::<DriverVersion>().unwrap(), expected, "{text:?}");
        }
    }

    #[test]
    fn anything_else_is_refused() {
        for text in [
            "",
            "v126",
            "126.0",
            "126.0.6478",
            "126.0.6478.126.1",
            "126.x.1.2",
            "-5",
            "1..2.3",
            "99999999999",
        ] {
            assert!(
                text.parse::<DriverVersion>().is_err(),
                "{text:?} was accepted"
            );
        }
    }

    #[test]
    fn display_reads_back_the_same() {
        for version in [
            DriverVersion::MatchBrowser,
            DriverVersion::Channel(ReleaseChannel::Stable),
            DriverVersion::Channel(ReleaseChannel::Canary),
            DriverVersion::Milestone(126),
            DriverVersion::Exact("126.0.6478.126".into()),
        ] {
            assert_eq!(
                version.to_string().parse::<DriverVersion>().unwrap(),
                version
            );
        }
    }

    #[test]
    fn it_is_stored_as_the_text_python_takes() {
        let version = DriverVersion::Milestone(126);
        assert_eq!(serde_json::to_string(&version).unwrap(), r#""126""#);
        assert_eq!(
            serde_json::from_str::<DriverVersion>(r#""beta""#).unwrap(),
            DriverVersion::Channel(ReleaseChannel::Beta)
        );
        assert!(serde_json::from_str::<DriverVersion>(r#""nope""#).is_err());
    }

    #[test]
    fn channels_are_named_as_the_chrome_for_testing_listing_names_them() {
        assert_eq!(ReleaseChannel::Stable.listing_name(), "Stable");
        assert_eq!(ReleaseChannel::Canary.listing_name(), "Canary");
    }
}
