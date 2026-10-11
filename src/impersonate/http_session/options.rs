//! The limits and the plumbing of an [`HttpSession`](super::HttpSession): timeouts, body and redirect
//! bounds, pacing, the event sink, the trust store and the resolver entries.
//!
//! Every field has a default that is safe on the open web and a setter that takes a custom value.
//! [`HttpOptions::validate`] refuses a value outside the documented range before a client exists.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wreq::tls::trust::CertStore;

use super::error::HttpError;
use crate::scrape::ledger::events::{EventSink, NoopEventSink};

/// How long one request may take in all, redirects included, unless told otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest total time per request [`HttpOptions::validate`] accepts.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(3600);
/// How long a connection may take to open, unless told otherwise.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The largest response body read into memory unless told otherwise, after decompression.
pub const DEFAULT_MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
/// The largest response body [`HttpOptions::validate`] accepts as a limit.
pub const MAX_BODY_LIMIT: usize = 1024 * 1024 * 1024;
/// The largest request body sent unless told otherwise.
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
/// How many redirects are followed, as in a browser.
pub const DEFAULT_MAX_REDIRECTS: usize = 10;
/// The most redirects [`HttpOptions::validate`] accepts as a limit.
pub const MAX_REDIRECT_LIMIT: usize = 30;
/// The most paced sites kept at once; past it the table starts over.
pub const MAX_PACED_SITES: usize = 4096;

/// A source of the time in seconds since the Unix epoch.
///
/// The session reads the time through this, so a test can move it.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The system clock.
pub(super) fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    })
}

/// The limits and plumbing of an HTTP session.
///
/// ```
/// use std::time::Duration;
/// use seleniumbase_rs::impersonate::http_session::HttpOptions;
///
/// let options = HttpOptions::new()
///     .timeout(Duration::from_secs(20))
///     .max_body_bytes(8 * 1024 * 1024)
///     .max_redirects(5)
///     .pacing(2, Duration::from_secs(1));
/// assert!(options.validate().is_ok());
/// ```
#[derive(Clone)]
pub struct HttpOptions {
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) max_body_bytes: usize,
    pub(super) max_request_bytes: usize,
    pub(super) max_redirects: usize,
    pub(super) follow_redirects: bool,
    pub(super) pacing: Option<(usize, Duration)>,
    pub(super) sink: Arc<dyn EventSink>,
    pub(super) trust: Option<CertStore>,
    pub(super) resolve: Vec<(String, SocketAddr)>,
    pub(super) bypass: Vec<String>,
    pub(super) installed_major: Option<u32>,
    pub(super) clock: Clock,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for HttpOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpOptions")
            .field("timeout", &self.timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("max_request_bytes", &self.max_request_bytes)
            .field("max_redirects", &self.max_redirects)
            .field("follow_redirects", &self.follow_redirects)
            .field("pacing", &self.pacing)
            .field("trust", &self.trust.as_ref().map(|_| "<store>"))
            .field("resolve", &self.resolve.len())
            .field("bypass", &self.bypass)
            .field("installed_major", &self.installed_major)
            .finish_non_exhaustive()
    }
}

impl HttpOptions {
    /// The defaults: [`DEFAULT_TIMEOUT`], [`DEFAULT_MAX_BODY_BYTES`], [`DEFAULT_MAX_REDIRECTS`]
    /// redirects followed, no pacing, no events, the system trust roots.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            follow_redirects: true,
            pacing: None,
            sink: Arc::new(NoopEventSink),
            trust: None,
            resolve: Vec::new(),
            bypass: Vec::new(),
            installed_major: None,
            clock: system_clock(),
        }
    }

    /// How long one request may take in all, redirects included.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// How long a connection may take to open. A value above the timeout of the request is cut
    /// to it.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// The largest response body read into memory, counted after decompression. A longer one
    /// fails the request with [`HttpError::BodyTooLarge`].
    #[must_use]
    pub fn max_body_bytes(mut self, limit: usize) -> Self {
        self.max_body_bytes = limit;
        self
    }

    /// The largest request body a request may carry.
    #[must_use]
    pub fn max_request_bytes(mut self, limit: usize) -> Self {
        self.max_request_bytes = limit;
        self
    }

    /// How many redirects one request follows before it fails with
    /// [`HttpError::TooManyRedirects`].
    #[must_use]
    pub fn max_redirects(mut self, limit: usize) -> Self {
        self.max_redirects = limit;
        self
    }

    /// Whether redirects are followed. Off, a `3xx` answer is the reply (its `Set-Cookie` is
    /// still stored).
    #[must_use]
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    /// At most `calls` requests to one site (a registrable domain) in `window`, every redirect
    /// hop counted. The limit is per site, not global.
    #[must_use]
    pub fn pacing(mut self, calls: usize, window: Duration) -> Self {
        self.pacing = Some((calls, window));
        self
    }

    /// Where the events of the session go.
    #[must_use]
    pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = sink;
        self
    }

    /// The certificate roots the server must chain to, in place of the system roots, as a
    /// `wreq::tls::trust::CertStore`. Verification stays on: this is how a private root is
    /// trusted.
    #[must_use]
    pub fn trust(mut self, store: CertStore) -> Self {
        self.trust = Some(store);
        self
    }

    /// Resolves `host` to `addr` without asking DNS, as `curl --resolve` does. The TLS server name
    /// stays `host`.
    #[must_use]
    pub fn resolve(mut self, host: impl Into<String>, addr: SocketAddr) -> Self {
        self.resolve.push((host.into(), addr));
        self
    }

    /// A host that skips the proxy, in the notation of a `no_proxy` list (`example.com`,
    /// `.example.com`). Only the egress of a session uses it.
    #[must_use]
    pub fn bypass(mut self, host: impl Into<String>) -> Self {
        self.bypass.push(host.into());
        self
    }

    /// The major version of the Chrome installed on this machine. A session refuses to start when
    /// it differs from the measured one: the measurement is stale.
    #[must_use]
    pub fn installed_chrome_major(mut self, major: u32) -> Self {
        self.installed_major = Some(major);
        self
    }

    /// The source of the time. The default reads the system clock.
    #[must_use]
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Checks every limit against its documented range.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Options`] naming the first value out of range.
    pub fn validate(&self) -> Result<(), HttpError> {
        let problem = if self.timeout.is_zero() || self.timeout > MAX_TIMEOUT {
            Some("timeout must be above zero and at most an hour")
        } else if self.connect_timeout.is_zero() || self.connect_timeout > MAX_TIMEOUT {
            Some("connect_timeout must be above zero and at most an hour")
        } else if self.max_body_bytes == 0 || self.max_body_bytes > MAX_BODY_LIMIT {
            Some("max_body_bytes must be from 1 to 1 GiB")
        } else if self.max_request_bytes > MAX_BODY_LIMIT {
            Some("max_request_bytes must be at most 1 GiB")
        } else if self.max_redirects > MAX_REDIRECT_LIMIT {
            Some("max_redirects must be at most 30")
        } else if self.pacing.is_some_and(|(_, window)| window.is_zero()) {
            Some("the pacing window must be above zero")
        } else {
            None
        };
        problem.map_or(Ok(()), |reason| Err(HttpError::Options(reason)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_named() {
        let options = HttpOptions::new();
        assert!(options.validate().is_ok());
        assert_eq!(options.timeout, DEFAULT_TIMEOUT);
        assert_eq!(options.max_body_bytes, DEFAULT_MAX_BODY_BYTES);
        assert_eq!(options.max_redirects, DEFAULT_MAX_REDIRECTS);
        assert!(options.follow_redirects);
        assert!(options.pacing.is_none());
    }

    #[test]
    fn every_limit_out_of_range_is_refused() {
        let bad = [
            HttpOptions::new().timeout(Duration::ZERO),
            HttpOptions::new().timeout(MAX_TIMEOUT + Duration::from_secs(1)),
            HttpOptions::new().connect_timeout(Duration::ZERO),
            HttpOptions::new().connect_timeout(MAX_TIMEOUT + Duration::from_secs(1)),
            HttpOptions::new().max_body_bytes(0),
            HttpOptions::new().max_body_bytes(MAX_BODY_LIMIT + 1),
            HttpOptions::new().max_request_bytes(MAX_BODY_LIMIT + 1),
            HttpOptions::new().max_redirects(MAX_REDIRECT_LIMIT + 1),
            HttpOptions::new().pacing(1, Duration::ZERO),
        ];
        for options in bad {
            assert!(
                matches!(options.validate(), Err(HttpError::Options(_))),
                "{options:?}"
            );
        }
    }

    #[test]
    fn the_edges_of_the_ranges_are_accepted() {
        let options = HttpOptions::new()
            .timeout(MAX_TIMEOUT)
            .connect_timeout(MAX_TIMEOUT)
            .max_body_bytes(MAX_BODY_LIMIT)
            .max_request_bytes(0)
            .max_redirects(0)
            .follow_redirects(false)
            .pacing(0, Duration::from_millis(1));
        assert!(options.validate().is_ok());
    }

    #[test]
    fn the_debug_view_hides_the_resolver_addresses_and_the_store() {
        let options = HttpOptions::new().resolve("a.test", "127.0.0.1:8443".parse().expect("addr"));
        let shown = format!("{options:?}");
        assert!(!shown.contains("127.0.0.1"), "{shown}");
        assert!(shown.contains("resolve: 1"), "{shown}");
    }

    #[test]
    fn the_system_clock_reads_a_plausible_time() {
        let now = (system_clock())();
        // After 2024-01-01 and before 2200.
        assert!((1_704_067_200..7_258_118_400).contains(&now), "{now}");
    }
}
