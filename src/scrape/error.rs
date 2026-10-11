//! The `ScrapeError` type, the one error enum the scrape layer returns, mapped to `SeleniumBaseError::Stealth` at the crate boundary.
//!
//! Every URL an error carries is a [`RedactedUrl`], so a password or a token in a query cannot
//! reach a log through an error. A browser failure that names the URL it was loading is turned
//! into [`ScrapeError::Navigation`] for the same reason; any other browser failure is kept as it
//! came, because it names no address the caller did not already hold.

use crate::error::SeleniumBaseError;
use crate::scrape::challenge::detect::ChallengeKind;
use crate::scrape::challenge::mitigation::GiveUpReason;
use crate::scrape::egress::geo::GeoError;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::egress::route::EgressError;
use crate::scrape::ledger::state::StateError;
use crate::scrape::passport::identity::IdentityError;

/// Why a scrape did not produce a page.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ScrapeError {
    /// A proxy spec was refused.
    #[error(transparent)]
    Egress(#[from] EgressError),
    /// A country or locale could not be resolved.
    #[error(transparent)]
    Geo(#[from] GeoError),
    /// The per-site state could not be read or written.
    #[error(transparent)]
    State(#[from] StateError),
    /// An identity was refused.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// A challenge was met and the attempts allowed ran out before it was passed.
    #[error("a {kind:?} challenge at {url} was not passed in {attempts} attempts")]
    Unsolved {
        /// The URL that was requested, redacted.
        url: RedactedUrl,
        /// The challenge that stood in the way.
        kind: ChallengeKind,
        /// How many attempts were made.
        attempts: u32,
    },
    /// The mitigation policy stopped.
    #[error("gave up on {url}: {reason:?}")]
    GaveUp {
        /// The URL that was requested, redacted.
        url: RedactedUrl,
        /// Why the policy stopped.
        reason: GiveUpReason,
    },
    /// The site is to be left alone for a while, so it was not requested.
    #[error("{site} is to be left alone until {until} (seconds since the Unix epoch)")]
    CoolingDown {
        /// The site, as a registrable domain.
        site: String,
        /// When it may be tried again, in seconds since the Unix epoch.
        until: u64,
    },
    /// The browser could not load the page.
    #[error("navigation to {url} failed: {reason}")]
    Navigation {
        /// The URL that was requested, redacted.
        url: RedactedUrl,
        /// What the browser said.
        reason: String,
    },
    /// An egress change was asked for and there is no other egress to change to.
    #[error("no other egress is available")]
    NoEgressAvailable,
    /// The browser failed in some other way.
    #[error(transparent)]
    Browser(SeleniumBaseError),
}

impl From<SeleniumBaseError> for ScrapeError {
    /// A navigation failure becomes [`ScrapeError::Navigation`], which holds the URL redacted;
    /// every other failure is wrapped as it is.
    fn from(error: SeleniumBaseError) -> Self {
        match error {
            SeleniumBaseError::Navigation { url, reason } => Self::Navigation {
                url: RedactedUrl::new(&url),
                reason,
            },
            other => Self::Browser(other),
        }
    }
}

impl From<ScrapeError> for SeleniumBaseError {
    /// A browser failure comes back as it was. Anything else becomes
    /// [`SeleniumBaseError::Stealth`] with the message of the error, which holds no credential.
    fn from(error: ScrapeError) -> Self {
        match error {
            ScrapeError::Browser(inner) => inner,
            other => Self::stealth(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::egress::route::EgressKey;

    const SECRET_URL: &str = "https://alice:s3cret@shop.example/cart?token=hunter2&page=2";

    fn assert_send_sync<T: Send + Sync>() {}

    fn shown(error: &ScrapeError) -> String {
        format!("{error} | {error:?}")
    }

    #[test]
    fn the_error_can_cross_threads() {
        assert_send_sync::<ScrapeError>();
    }

    #[test]
    fn a_navigation_failure_keeps_the_url_only_redacted() {
        let error = ScrapeError::from(SeleniumBaseError::navigation(
            SECRET_URL,
            "net::ERR_CONNECTION_RESET",
        ));
        assert!(matches!(error, ScrapeError::Navigation { .. }));
        let text = shown(&error);
        for secret in ["alice", "s3cret", "hunter2"] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
        assert!(text.contains("page=2") && text.contains("ERR_CONNECTION_RESET"));
    }

    #[test]
    fn any_other_browser_failure_is_wrapped_as_it_came() {
        let error = ScrapeError::from(SeleniumBaseError::cdp_driver("target closed"));
        assert!(matches!(
            &error,
            ScrapeError::Browser(SeleniumBaseError::CdpDriver(message)) if message == "target closed"
        ));
        assert_eq!(error.to_string(), "CDP driver error: target closed");
    }

    #[test]
    fn a_browser_failure_returns_to_the_crate_error_unchanged() {
        let back =
            SeleniumBaseError::from(ScrapeError::Browser(SeleniumBaseError::cdp_driver("gone")));
        assert!(matches!(back, SeleniumBaseError::CdpDriver(message) if message == "gone"));
    }

    #[test]
    fn every_other_error_becomes_a_stealth_error_with_its_message() {
        let unsolved = ScrapeError::Unsolved {
            url: RedactedUrl::new(SECRET_URL),
            kind: ChallengeKind::Interactive,
            attempts: 3,
        };
        let message = unsolved.to_string();
        let back = SeleniumBaseError::from(unsolved);
        assert!(
            matches!(&back, SeleniumBaseError::Stealth { reason } if *reason == message),
            "{back:?}"
        );
        assert!(message.contains("Interactive") && message.contains("3 attempts"));
    }

    #[test]
    fn no_variant_prints_a_credential() {
        let url = RedactedUrl::new(SECRET_URL);
        let errors = [
            ScrapeError::Unsolved {
                url: url.clone(),
                kind: ChallengeKind::JsChallenge,
                attempts: 2,
            },
            ScrapeError::GaveUp {
                url: url.clone(),
                reason: GiveUpReason::Blocked,
            },
            ScrapeError::Navigation {
                url,
                reason: "net::ERR_ABORTED".to_owned(),
            },
            ScrapeError::CoolingDown {
                site: "shop.example".to_owned(),
                until: 1_700_000_900,
            },
            ScrapeError::NoEgressAvailable,
        ];
        for error in &errors {
            let text = shown(error);
            for secret in ["alice", "s3cret", "hunter2"] {
                assert!(!text.contains(secret), "{secret} in {text}");
            }
        }
    }

    #[test]
    fn the_domain_errors_convert_with_a_question_mark() {
        fn egress() -> Result<(), ScrapeError> {
            crate::scrape::egress::route::EgressRef::parse("ftp://proxy.example:21", None)?;
            Ok(())
        }
        fn geo() -> Result<(), ScrapeError> {
            crate::scrape::egress::geo::CountryCode::new("Germany")?;
            Ok(())
        }
        fn identity() -> Result<(), ScrapeError> {
            Err(IdentityError::Schema(9))?
        }
        assert!(matches!(egress(), Err(ScrapeError::Egress(_))));
        assert!(matches!(geo(), Err(ScrapeError::Geo(_))));
        assert!(matches!(identity(), Err(ScrapeError::Identity(_))));
        let key = EgressKey {
            scheme: "http".to_owned(),
            host: "proxy.example".to_owned(),
            port: 8080,
            country: None,
        };
        let unknown = ScrapeError::from(GeoError::Unknown(key));
        assert!(unknown.to_string().contains("http://proxy.example:8080"));
    }
}
