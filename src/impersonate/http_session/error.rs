//! Why an HTTP session could not be made, or a request produced no reply.
//!
//! Nothing here can carry a credential. A URL is a [`RedactedUrl`]; a failure of the HTTP client
//! is reduced to a fixed word (`kind`) and the text of the client's own error is dropped, because
//! it can name the URL, the proxy and its password. A header is named, never quoted: its value
//! may be a token.

use crate::error::SeleniumBaseError;
use crate::impersonate::client::ImpersonateError;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::passport::identity::IdentityError;

/// Why an [`HttpSession`](super::HttpSession) could not be made or a request failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HttpError {
    /// The measured browser could not be turned into a client.
    #[error(transparent)]
    Impersonate(ImpersonateError),
    /// The browser is another Chrome than the one the TLS and HTTP/2 were measured on.
    ///
    /// A request that claims Chrome `browser` over the hello of Chrome `measured` is a
    /// contradiction a detector can see, so none is sent until the capture tools run again.
    #[error("the browser is Chrome {browser}, but the measured TLS and HTTP/2 are Chrome {measured}; run the capture tools again")]
    VersionMismatch {
        /// The major version the browser or identity claims.
        browser: u32,
        /// The major version that was measured.
        measured: u32,
    },
    /// The identity was refused: another egress, another emulation, or an incoherent fingerprint.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// A header the browser profile owns was set on a request.
    #[error("the header `{0}` belongs to the browser profile and cannot be set on a request")]
    ReservedHeader(String),
    /// A header name or value is not valid, or there are too many headers.
    #[error("the header `{0}` is not valid")]
    InvalidHeader(String),
    /// A method a browser never sends from a page.
    #[error("the method `{0}` is not allowed")]
    ForbiddenMethod(String),
    /// A URL that cannot be requested. The URL is not echoed: it may hold a credential.
    #[error("the URL cannot be requested: {reason}")]
    Url {
        /// What is wrong with it.
        reason: &'static str,
    },
    /// The chain of redirects is longer than the limit.
    #[error("more than {0} redirects")]
    TooManyRedirects(usize),
    /// A redirect target that is not followed.
    #[error("the redirect to {url} was refused: {reason}")]
    RedirectRefused {
        /// The target, redacted.
        url: RedactedUrl,
        /// Why it was refused.
        reason: &'static str,
    },
    /// The response body is larger than the limit (in bytes), after decompression.
    #[error("the response body is larger than {0} bytes")]
    BodyTooLarge(usize),
    /// The request body is larger than the limit (in bytes).
    #[error("the request body is larger than {0} bytes")]
    RequestTooLarge(usize),
    /// The request failed on the way: no connection, a TLS failure, a timeout.
    #[error("the request to {url} failed: {kind}")]
    Request {
        /// The URL requested, redacted.
        url: RedactedUrl,
        /// What kind of failure, in a word or two.
        kind: &'static str,
    },
    /// The HTTP client could not be set up (a proxy, a trust store, a resolver entry).
    #[error("the HTTP client could not be set up: {kind}")]
    Client {
        /// Which part was refused.
        kind: &'static str,
    },
    /// An option is out of range.
    #[error("invalid HTTP options: {0}")]
    Options(&'static str),
    /// The body is not UTF-8 text.
    #[error("the body is not UTF-8 text")]
    NotUtf8,
    /// The body is not the JSON the caller asked for. The text of the body is not echoed.
    #[error("the body is not the JSON that was asked for: {kind}")]
    Json {
        /// Whether the text is not JSON, ends too early, or has another layout.
        kind: &'static str,
    },
    /// The page could not be read or written.
    #[error(transparent)]
    Browser(#[from] SeleniumBaseError),
}

impl From<ImpersonateError> for HttpError {
    /// A version mismatch keeps its own variant; every other failure is wrapped.
    fn from(error: ImpersonateError) -> Self {
        match error {
            ImpersonateError::VersionMismatch { browser, measured } => {
                Self::VersionMismatch { browser, measured }
            }
            other => Self::Impersonate(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_error_can_cross_threads() {
        assert_send_sync::<HttpError>();
    }

    #[test]
    fn an_impersonation_version_mismatch_becomes_the_session_variant() {
        let error = HttpError::from(ImpersonateError::VersionMismatch {
            browser: 156,
            measured: 155,
        });
        assert!(matches!(
            error,
            HttpError::VersionMismatch {
                browser: 156,
                measured: 155
            }
        ));
        let other = HttpError::from(ImpersonateError::Build("x".to_owned()));
        assert!(matches!(other, HttpError::Impersonate(_)));
    }

    #[test]
    fn a_request_error_shows_the_url_redacted() {
        let error = HttpError::Request {
            url: RedactedUrl::new("https://alice:s3cret@shop.example/a?token=hunter2&page=2"),
            kind: "timed out",
        };
        let shown = format!("{error} | {error:?}");
        for secret in ["alice", "s3cret", "hunter2"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
        assert!(shown.contains("page=2") && shown.contains("timed out"));
    }
}
