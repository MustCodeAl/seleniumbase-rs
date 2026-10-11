//! `HttpSession`: fetches over HTTP with the profile's client, following redirects and storing cookies by hand.
//!
//! A browser spends a second or more on a page; an HTTP request spends milliseconds. Once a page
//! has earned its cookies (a login, a passed check), an [`HttpSession`] carries them on in plain
//! requests that look like the same Chrome: the measured TLS `ClientHello`, the measured HTTP/2
//! start, the header names, order and values of that Chrome on that system, and the user agent,
//! client hints and `Accept-Language` of the identity. Nothing is typed by hand: every wire value
//! comes from the measurement `wire::emulation` embeds, and a browser or system that was not
//! measured is refused instead of approximated.
//!
//! ```no_run
//! use seleniumbase_rs::impersonate::http_session::{HttpOptions, HttpSession};
//! use seleniumbase_rs::sb_cdp::Page;
//!
//! # async fn demo(page: Page) -> Result<(), Box<dyn std::error::Error>> {
//! let session = HttpSession::from_page(&page, None, HttpOptions::new()).await?;
//! let reply = session.get("https://example.com/api/orders").await?;
//! let orders: serde_json::Value = reply.json()?;
//! println!("{orders}");
//! session.sync_cookies_back(&page).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # What a session does for you
//!
//! - **Cookies** live in an RFC 6265 jar (`scrape::jar::store::CookieJar`) with the public suffix
//!   list. The session writes the `Cookie` header itself and reads `Set-Cookie` itself; wreq's
//!   own cookie store is off. A cookie set for a public suffix, for another host, or against the
//!   `Secure`, `__Host-` and `SameSite` rules is refused exactly as a browser refuses it.
//! - **Redirects** are followed by hand, up to a limit, with the cookies of each hop stored and
//!   sent per URL, `303` turned into a `GET`, and the headers the caller set dropped when a hop
//!   leaves the origin.
//! - **Limits** bound the whole request: time, body size after decompression, redirects, request
//!   body size, and requests per window per site ([`HttpOptions`]).
//! - **Challenges** are detected on every reply ([`HttpReply::detect`]) and judged by the
//!   mitigation and session policies ([`HttpSession::fetch`], [`HttpSession::assess`]), so a
//!   caller learns whether to carry on, wait, rotate, or hand the URL to a browser.
//! - **Identity**: [`HttpSession::export_identity`] puts the session's cookies back into a
//!   `StealthIdentity`, and [`HttpSession::from_identity`] refuses an identity that belongs to
//!   another egress or another Chrome version.
//!
//! # What it does not do
//!
//! A matching TLS and HTTP/2 start is one signal among many. It does not run JavaScript, solve a
//! challenge, or pass canvas, WebGL or behavioural checks, and it does not make automation
//! permitted by a site's terms. The headers `User-Agent`, `Sec-CH-UA*`, `Accept-Language`,
//! `Accept-Encoding`, `Cookie` and `priority` belong to the profile; a request cannot set them.
//!
//! A redirect is followed where it points, as a browser follows it, including to a private
//! address: a session that fetches URLs for someone else must filter them before it asks. The
//! first request to an origin writes the headers of HTTP/2, which nearly every site speaks; if
//! the origin answers on HTTP/1.1 the session remembers it and writes those headers from then on.
//!
//! Credentials, cookies and proxy passwords never reach `Debug`, logs, events or errors: URLs are
//! redacted, a failure of the HTTP client is reduced to a word, and header values are not quoted.

mod cookies;
mod error;
mod fetch;
mod network;
mod options;
mod outcome;
mod pace;
mod page;
mod redirect;
mod reply;
mod request;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use wreq::Method;

pub use self::error::HttpError;
pub use self::options::{
    Clock, HttpOptions, DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_BODY_BYTES, DEFAULT_MAX_REDIRECTS,
    DEFAULT_MAX_REQUEST_BYTES, DEFAULT_TIMEOUT, MAX_BODY_LIMIT, MAX_REDIRECT_LIMIT, MAX_TIMEOUT,
};
pub use self::outcome::{Attempt, Fetched, Rules, Verdict};
pub use self::reply::{Headers, HttpReply, ReplyProtocol};
pub use self::request::HttpRequest;

use self::cookies::SessionJar;
use self::pace::SitePacer;
use crate::impersonate::client::{ImpersonationProfile, WireProtocol};
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::jar::store::CookieJar;
use crate::scrape::ledger::events::{EventSink, ScraperEvent};
use crate::scrape::passport::identity::{EmulationRef, IdentityError, StealthIdentity};

/// The most origins whose protocol is remembered. Past it the table starts over.
const MAX_REMEMBERED_ORIGINS: usize = 1024;

/// The bounds a request is held to.
#[derive(Clone, Copy, Debug)]
struct Limits {
    timeout: Duration,
    max_body_bytes: usize,
    max_request_bytes: usize,
    max_redirects: usize,
    follow_redirects: bool,
}

/// What the clones of a session share.
struct Inner {
    client: wreq::Client,
    profile: ImpersonationProfile,
    jar: SessionJar,
    egress: Option<EgressRef>,
    limits: Limits,
    pacer: Option<SitePacer>,
    sink: Arc<dyn EventSink>,
    clock: Clock,
    /// The protocol each origin settled on, so the next request carries the headers of that
    /// protocol (the `priority` header is for HTTP/2 only).
    protocols: Mutex<HashMap<String, WireProtocol>>,
}

/// Plain HTTP requests that continue a browser's session; see the [module](self) documentation.
///
/// Cloning gives another handle to the same session: the same cookies, client and limits.
#[derive(Clone)]
pub struct HttpSession {
    inner: Arc<Inner>,
}

impl fmt::Debug for HttpSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never the cookies or the proxy password.
        f.debug_struct("HttpSession")
            .field("profile", &self.inner.profile.source())
            .field("egress", &self.inner.egress)
            .field("cookies", &self.inner.jar.len())
            .field("limits", &self.inner.limits)
            .finish_non_exhaustive()
    }
}

/// Maps the mismatch of an identity's emulation to the session's version error.
fn emulation_error(error: IdentityError) -> HttpError {
    match error {
        IdentityError::EmulationMismatch { identity, measured } => HttpError::VersionMismatch {
            browser: identity,
            measured,
        },
        other => HttpError::Identity(other),
    }
}

impl HttpSession {
    /// A session for an identity: its fingerprint becomes the user agent, client hints and
    /// languages of the requests, and its cookies fill the jar.
    ///
    /// `egress` is the exit the requests leave through, and it must be the one the identity
    /// belongs to: a clearance is bound to the address that earned it.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Identity`] when the identity is incoherent or belongs to another
    /// egress, [`HttpError::VersionMismatch`] when it claims another Chrome major than the
    /// measured one, [`HttpError::Impersonate`] for a system that was not measured, and
    /// [`HttpError::Options`] or [`HttpError::Client`] when the options or the proxy are refused.
    pub fn from_identity(
        identity: &StealthIdentity,
        egress: Option<EgressRef>,
        options: HttpOptions,
    ) -> Result<Self, HttpError> {
        options.validate()?;
        identity.verify()?;
        identity.check_egress(egress.as_ref().map(EgressRef::key))?;
        let profile = ImpersonationProfile::from_fingerprint(&identity.fingerprint)?;
        identity
            .check_emulation(&EmulationRef {
                kind: profile.kind(),
                major: profile.major(),
                os: profile.os(),
            })
            .map_err(emulation_error)?;
        let session = Self::assemble(profile, identity.cookies.clone(), egress, options)?;
        session.emit(&ScraperEvent::IdentityRestored {
            cookies: session.inner.jar.len(),
        });
        Ok(session)
    }

    /// A session from a profile, a jar of cookies and an egress.
    fn assemble(
        profile: ImpersonationProfile,
        jar: CookieJar,
        egress: Option<EgressRef>,
        options: HttpOptions,
    ) -> Result<Self, HttpError> {
        if let Some(installed) = options.installed_major {
            if installed != profile.major() {
                return Err(HttpError::VersionMismatch {
                    browser: installed,
                    measured: profile.major(),
                });
            }
        }
        let client = network::build_client(&profile, egress.as_ref(), &options)?;
        let limits = Limits {
            timeout: options.timeout,
            max_body_bytes: options.max_body_bytes,
            max_request_bytes: options.max_request_bytes,
            max_redirects: options.max_redirects,
            follow_redirects: options.follow_redirects,
        };
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                profile,
                jar: SessionJar::new(jar),
                egress,
                limits,
                pacer: options
                    .pacing
                    .map(|(calls, window)| SitePacer::new(calls, window)),
                sink: options.sink,
                clock: options.clock,
                protocols: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Starts a request with an explicit method.
    ///
    /// The default headers are those of a navigation for `GET` and `HEAD` and those of a script's
    /// request for every other method; [`HttpRequest::shape`] chooses.
    pub fn request(&self, method: Method, url: &str) -> HttpRequest<'_> {
        HttpRequest::new(self, method, url)
    }

    /// Sends a `GET` request.
    ///
    /// # Errors
    ///
    /// See [`HttpRequest::send`].
    pub async fn get(&self, url: &str) -> Result<HttpReply, HttpError> {
        self.request(Method::GET, url).send().await
    }

    /// The profile the requests are made with.
    #[must_use]
    pub fn profile(&self) -> &ImpersonationProfile {
        &self.inner.profile
    }

    /// The exit the requests leave through; `None` for a direct connection. It carries no
    /// credentials.
    #[must_use]
    pub fn egress(&self) -> Option<&EgressKey> {
        self.inner.egress.as_ref().map(EgressRef::key)
    }

    /// How many cookies the jar holds.
    #[must_use]
    pub fn cookie_count(&self) -> usize {
        self.inner.jar.len()
    }

    /// The time now, in seconds since the Unix epoch, as the session reads it.
    fn now(&self) -> u64 {
        (self.inner.clock)()
    }

    /// Hands an event to the sink.
    fn emit(&self, event: &ScraperEvent) {
        self.inner.sink.emit(event);
    }

    fn inner(&self) -> &Inner {
        &self.inner
    }

    fn limits(&self) -> &Limits {
        &self.inner.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::BrowserKind;

    #[test]
    fn an_emulation_mismatch_of_an_identity_is_a_version_mismatch_and_nothing_else_is() {
        let version = emulation_error(IdentityError::EmulationMismatch {
            identity: 154,
            measured: 155,
        });
        assert!(matches!(
            version,
            HttpError::VersionMismatch {
                browser: 154,
                measured: 155
            }
        ));
        let other = emulation_error(IdentityError::Schema(9));
        assert!(matches!(
            other,
            HttpError::Identity(IdentityError::Schema(9))
        ));
    }

    #[test]
    fn a_session_from_a_profile_shows_counts_and_the_egress_key_only() {
        let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
        let egress =
            EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("a spec");
        let session =
            HttpSession::assemble(profile, CookieJar::new(), Some(egress), HttpOptions::new())
                .expect("a session");
        let shown = format!("{session:?}");
        assert!(
            shown.contains("proxy.example") && shown.contains("cookies: 0"),
            "{shown}"
        );
        for secret in ["s3cret", "alice"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
        assert_eq!(session.egress().map(|key| key.port), Some(8080));
        assert_eq!(session.cookie_count(), 0);
    }

    #[test]
    fn the_installed_chrome_must_be_the_measured_one_when_a_profile_is_used_directly() {
        let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
        let measured = profile.major();
        let stale = HttpSession::assemble(
            profile.clone(),
            CookieJar::new(),
            None,
            HttpOptions::new().installed_chrome_major(measured + 3),
        )
        .expect_err("stale");
        assert!(
            matches!(stale, HttpError::VersionMismatch { browser, .. } if browser == measured + 3)
        );
        let fresh = HttpSession::assemble(
            profile,
            CookieJar::new(),
            None,
            HttpOptions::new().installed_chrome_major(measured),
        );
        assert!(fresh.is_ok());
    }
}
