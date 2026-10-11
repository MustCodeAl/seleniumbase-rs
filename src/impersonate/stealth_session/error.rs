//! The error of a stealth session: one enum over the layers the session combines.
//!
//! Nothing here adds a message of its own. Each variant carries the error of the layer that
//! failed, and each of those already keeps URLs redacted and credentials out of its text, so a
//! `StealthError` can be logged as it is.

use crate::error::SeleniumBaseError;
use crate::impersonate::client::ImpersonateError;
use crate::impersonate::http_session::HttpError;
use crate::impersonate::mitm::MitmError;
use crate::scrape::egress::geo::GeoError;
use crate::scrape::egress::route::EgressError;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::state::StateError;
use crate::scrape::passport::identity::IdentityError;

/// Why a [`StealthSession`](super::StealthSession) could not be made, moved or fetch a page.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StealthError {
    /// The browser leg, the site state, an identity or the mitigation policy failed. The
    /// challenge outcomes (`Unsolved`, `GaveUp`, `CoolingDown`) come as this variant.
    #[error(transparent)]
    Scrape(#[from] ScrapeError),
    /// The HTTP leg failed.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The MITM proxy that carries the browser's traffic failed.
    #[error(transparent)]
    Mitm(#[from] MitmError),
    /// The measured browser could not be turned into a profile, or the browser that runs is not
    /// the one that was measured.
    #[error(transparent)]
    Impersonate(#[from] ImpersonateError),
}

/// Wraps an error of the scrape layer the way [`ScrapeError`] does.
macro_rules! through_scrape {
    ($($source:ty),+ $(,)?) => {$(
        impl From<$source> for StealthError {
            fn from(error: $source) -> Self {
                Self::Scrape(ScrapeError::from(error))
            }
        }
    )+};
}

through_scrape!(
    EgressError,
    GeoError,
    StateError,
    IdentityError,
    SeleniumBaseError
);

impl From<StealthError> for SeleniumBaseError {
    /// A browser failure comes back as it was. Anything else becomes
    /// [`SeleniumBaseError::Stealth`] with the message of the error, which holds no credential.
    fn from(error: StealthError) -> Self {
        match error {
            StealthError::Scrape(inner) => Self::from(inner),
            other => Self::stealth(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::egress::redact::RedactedUrl;

    const SECRET_URL: &str = "https://alice:s3cret@shop.example/cart?token=hunter2&page=2";

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_error_can_cross_threads() {
        assert_send_sync::<StealthError>();
    }

    #[test]
    fn every_layer_converts_with_a_question_mark() {
        fn identity() -> Result<(), StealthError> {
            Err(IdentityError::Schema(9))?;
            Ok(())
        }
        fn egress() -> Result<(), StealthError> {
            crate::scrape::egress::route::EgressRef::parse("ftp://proxy.example:21", None)?;
            Ok(())
        }
        fn http() -> Result<(), StealthError> {
            Err(HttpError::Options("bad"))?;
            Ok(())
        }
        fn browser() -> Result<(), StealthError> {
            Err(SeleniumBaseError::cdp_driver("gone"))?;
            Ok(())
        }
        assert!(matches!(
            identity(),
            Err(StealthError::Scrape(ScrapeError::Identity(
                IdentityError::Schema(9)
            )))
        ));
        assert!(matches!(
            egress(),
            Err(StealthError::Scrape(ScrapeError::Egress(_)))
        ));
        assert!(matches!(
            http(),
            Err(StealthError::Http(HttpError::Options(_)))
        ));
        assert!(matches!(
            browser(),
            Err(StealthError::Scrape(ScrapeError::Browser(_)))
        ));
    }

    #[test]
    fn a_browser_failure_returns_to_the_crate_error_unchanged() {
        let back =
            SeleniumBaseError::from(StealthError::from(SeleniumBaseError::cdp_driver("gone")));
        assert!(matches!(back, SeleniumBaseError::CdpDriver(message) if message == "gone"));
    }

    #[test]
    fn every_other_error_becomes_a_stealth_error_with_its_message() {
        let error = StealthError::Http(HttpError::TooManyRedirects(10));
        let message = error.to_string();
        let back = SeleniumBaseError::from(error);
        assert!(
            matches!(&back, SeleniumBaseError::Stealth { reason } if *reason == message),
            "{back:?}"
        );
    }

    #[test]
    fn no_variant_prints_a_credential() {
        let url = RedactedUrl::new(SECRET_URL);
        let errors = [
            StealthError::Http(HttpError::Request {
                url: url.clone(),
                kind: "timed out",
            }),
            StealthError::Scrape(ScrapeError::Navigation {
                url,
                reason: "net::ERR_ABORTED".to_owned(),
            }),
        ];
        for error in &errors {
            let shown = format!("{error} | {error:?}");
            for secret in ["alice", "s3cret", "hunter2"] {
                assert!(!shown.contains(secret), "{secret} in {shown}");
            }
        }
    }
}
