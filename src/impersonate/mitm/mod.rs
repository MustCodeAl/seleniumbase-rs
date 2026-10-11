//! The MITM proxy: a loopback forward proxy that ends the browser's TLS and replays each request
//! with the profile's fingerprint.
//!
//! [`MitmProxy::start`] binds `127.0.0.1` on a port the system picks and serves Chrome as its HTTP
//! proxy. For each `CONNECT` it:
//!
//! 1. requires `Proxy-Authorization: Basic` with the proxy's random 128-bit token, compared in
//!    constant time, and answers `407` otherwise. Chrome gets the token from `ProxyAuth` over
//!    `Fetch.authRequired`, so it never appears on Chrome's command line;
//! 2. refuses a target it cannot intercept with an empty `502` and closes the connection. Nothing
//!    is ever tunnelled blind: bytes that are not a TLS handshake for the `CONNECT` host end the
//!    connection;
//! 3. answers `200` and ends the browser's TLS with a leaf of the ephemeral CA for exactly the
//!    `CONNECT` host (ALPN `http/1.1`; the SNI must name the same host);
//! 4. reads each request, drops the hop-by-hop and proxy headers, keeps Chrome's header order,
//!    adds the HTTP/2-only `priority` header from the measured table, and sends the request
//!    upstream through the impersonating client (measured `ClientHello`, HTTP/2 start and header
//!    spelling), with certificate verification on;
//! 5. streams the answer back without buffering it whole. An upstream failure, a certificate
//!    the upstream client does not trust included, is an empty `502`.
//!
//! An absolute-form `http://` request is checked the same way and forwarded without `CONNECT`.
//!
//! # What Chrome must be told
//!
//! [`MitmProxy::configure_launch`] sets the proxy and adds [`MitmProxy::chrome_args`]: the
//! `--ignore-certificate-errors-spki-list` switch with the leaf pin only (never the CA pin),
//! `--proxy-bypass-list=<-loopback>` so loopback hosts go through the proxy too, and
//! `--disable-quic`. Chrome trusts any chain that carries the pinned key, so the pin is safe only
//! while every connection of that profile ends here; without the last two switches a loopback
//! server or a QUIC connection would reach Chrome directly.
//!
//! # Bounds
//!
//! Connections ([`MitmOptions::max_connections`], 256 by default; further clients wait in the
//! listen queue), the request head ([`MitmOptions::max_header_bytes`] and
//! [`MitmOptions::max_headers`]), each body in either direction
//! ([`MitmOptions::max_body_bytes`]), the idle time of a connection waiting for its next request
//! ([`MitmOptions::idle_timeout`]), the browser's TLS handshake, and the upstream connect and read
//! phases all have limits.
//!
//! # Hot-swappable upstream
//!
//! [`MitmProxy::swap_upstream`] and [`MitmProxy::swap_profile`] build the new upstream client
//! first and then replace the old one; requests already under way finish on the old one and the
//! listener, its port and the browser's open connections stay.
//!
//! # Not intercepted
//!
//! Protocol upgrades (WebSocket) are refused with an empty `502`. QUIC is off in Chrome, so there
//! is no HTTP/3. Requests whose `User-Agent` claims a Chrome major other than the profile's are
//! refused, because the measured TLS and HTTP/2 would contradict the header.
//!
//! # Secrets
//!
//! The token, the `Proxy-Authorization` value, the egress credentials, leaf certificates and keys
//! never reach `Debug`, `Display`, logs, events or files. Logs carry redacted URLs and egress keys.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! use seleniumbase_rs::impersonate::client::ImpersonationProfile;
//! use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs::scrape::ledger::events::NoopEventSink;
//! use seleniumbase_rs::wire::BrowserKind;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let profile = ImpersonationProfile::measured(BrowserKind::Chrome)?;
//! let mitm = MitmProxy::start(profile, None, MitmOptions::default(), Arc::new(NoopEventSink))
//!     .await?;
//! let options = mitm.configure_launch(LaunchOptions::builder()).build()?;
//! let browser = Browser::launch(options).await?;
//! let page = browser.new_page(Some("https://example.com/")).await?;
//! # drop(page);
//! browser.close().await?;
//! mitm.shutdown(Duration::from_secs(5)).await;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

mod body;
mod connect;
mod forward;
mod upstream;

#[cfg(test)]
mod tests;

use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btls::ssl::SslAcceptor;
use ring::rand::{SecureRandom, SystemRandom};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use self::connect::ProxyCredentials;
use self::upstream::{Upstream, UpstreamSlot};
use super::client::{ImpersonateError, ImpersonationProfile};
use super::tls_server::{self, EphemeralCa, TlsServerError, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT};
use crate::common::shutdown::{cleanup_within, spawn_cleanup, timeout_from_env};
use crate::sb_cdp::sync::locked;
use crate::sb_cdp::{LaunchOptionsBuilder, Proxy};
use crate::scrape::egress::redact::{RedactedUrl, Secret};
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::ledger::events::{EventSink, ScraperEvent};

/// The user name the browser gives the proxy; the proxy's token is the password.
pub const PROXY_USER: &str = "sb";
/// The Chrome switch that removes the implicit bypass of loopback hosts, so `localhost` and
/// `127.0.0.1` go through the proxy as well.
pub const PROXY_EVERYTHING_ARG: &str = "--proxy-bypass-list=<-loopback>";
/// The Chrome switch that turns QUIC off, so every connection is TCP through the proxy.
pub const DISABLE_QUIC_ARG: &str = "--disable-quic";
/// How many browser connections are served at once by default.
pub const DEFAULT_MAX_CONNECTIONS: usize = 256;
/// The most browser connections [`MitmOptions::max_connections`] accepts.
pub const MAX_CONNECTIONS: usize = 4096;
/// The largest request head (request line and headers) by default, in bytes.
pub const DEFAULT_MAX_HEADER_BYTES: usize = 64 * 1024;
/// The smallest request-head limit accepted; the HTTP/1 parser needs this much buffer.
pub const MIN_HEADER_BYTES: usize = 8 * 1024;
/// The largest request-head limit accepted.
pub const MAX_HEADER_BYTES: usize = 1024 * 1024;
/// The most header fields in one request by default.
pub const DEFAULT_MAX_HEADERS: usize = 128;
/// The most header fields [`MitmOptions::max_headers`] accepts.
pub const MAX_HEADERS: usize = 1024;
/// The largest request or response body by default, in bytes. Bodies stream; this only cuts a
/// body that grows past it.
pub const DEFAULT_MAX_BODY_BYTES: usize = 512 * 1024 * 1024;
/// How long a browser connection may wait for its next request by default.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the upstream connection (TCP, egress tunnel and TLS) may take by default.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long one upstream read may take by default.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Random bytes in a proxy token: 128 bits.
const TOKEN_BYTES: usize = 16;
/// What the proxy URL starts with.
const PROXY_SCHEME: &str = "http";

/// Limits and custom inputs of a [`MitmProxy`].
///
/// Every field has a builder method; [`MitmProxy::start`] checks the bounds named on each.
/// `Debug` shows the limits, the number of extra roots and overrides, and the CA's `Debug`,
/// which never shows a key.
#[derive(Clone)]
pub struct MitmOptions {
    /// Browser connections served at once.
    max_connections: usize,
    /// The largest request head, in bytes.
    max_header_bytes: usize,
    /// The most header fields in one request.
    max_headers: usize,
    /// The largest body in either direction, in bytes.
    max_body_bytes: usize,
    /// How long a browser connection may wait for its next request.
    idle_timeout: Duration,
    /// How long the browser's TLS handshake may take.
    handshake_timeout: Duration,
    /// How long the upstream connection may take.
    connect_timeout: Duration,
    /// How long one upstream read may take.
    read_timeout: Duration,
    /// DER roots that replace the built-in web roots for upstream verification, when not empty.
    upstream_roots: Vec<Vec<u8>>,
    /// Host names the upstream client resolves to fixed addresses.
    resolve: Vec<(String, SocketAddr)>,
    /// The CA to issue leaves with, instead of a new one.
    ca: Option<Arc<EphemeralCa>>,
}

impl Default for MitmOptions {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_CONNECTIONS,
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            max_headers: DEFAULT_MAX_HEADERS,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            upstream_roots: Vec::new(),
            resolve: Vec::new(),
            ca: None,
        }
    }
}

impl fmt::Debug for MitmOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MitmOptions")
            .field("max_connections", &self.max_connections)
            .field("max_header_bytes", &self.max_header_bytes)
            .field("max_headers", &self.max_headers)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("idle_timeout", &self.idle_timeout)
            .field("handshake_timeout", &self.handshake_timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("read_timeout", &self.read_timeout)
            .field("upstream_roots", &self.upstream_roots.len())
            .field("resolve", &self.resolve.len())
            .field("ca", &self.ca)
            .finish()
    }
}

impl MitmOptions {
    /// Browser connections served at once, from 1 to [`MAX_CONNECTIONS`]. Clients past it wait
    /// in the listen queue until a connection ends.
    #[must_use]
    pub fn max_connections(mut self, count: usize) -> Self {
        self.max_connections = count;
        self
    }

    /// The largest request head in bytes, from [`MIN_HEADER_BYTES`] to [`MAX_HEADER_BYTES`]. A
    /// larger head is answered `431`.
    #[must_use]
    pub fn max_header_bytes(mut self, bytes: usize) -> Self {
        self.max_header_bytes = bytes;
        self
    }

    /// The most header fields in one request, from 1 to [`MAX_HEADERS`].
    #[must_use]
    pub fn max_headers(mut self, count: usize) -> Self {
        self.max_headers = count;
        self
    }

    /// The largest body in either direction, in bytes, at least 1. A request that announces a
    /// larger body is answered `413`; a body that grows past it is cut and its connection closed.
    #[must_use]
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// How long a browser connection may wait for its next request (and for the head of its
    /// first one) before it is closed. Not zero.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// How long the browser's TLS handshake may take. Not zero.
    #[must_use]
    pub fn handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// How long the upstream connection (TCP, egress tunnel, TLS) may take. Not zero.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// How long one upstream read may take. Not zero.
    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Trusts a DER root for upstream verification. Once one is added, the upstream client
    /// trusts the added roots only, not the built-in web roots: use it for a private site.
    /// Verification is never turned off.
    #[must_use]
    pub fn trust_upstream_root(mut self, der: impl Into<Vec<u8>>) -> Self {
        self.upstream_roots.push(der.into());
        self
    }

    /// Makes the upstream client resolve `host` to `addr`. The port of `addr` is ignored: the
    /// request's own port is used, as the browser asked for it.
    #[must_use]
    pub fn resolve(mut self, host: impl Into<String>, addr: SocketAddr) -> Self {
        self.resolve.push((host.into(), addr));
        self
    }

    /// Issues the leaves with `ca` instead of a CA made for this proxy alone. The CA's key stays
    /// in memory; sharing it shares the pin Chrome trusts.
    #[must_use]
    pub fn ca(mut self, ca: Arc<EphemeralCa>) -> Self {
        self.ca = Some(ca);
        self
    }

    /// Checks the documented bounds.
    fn validate(&self) -> Result<(), MitmError> {
        let problem = if !(1..=MAX_CONNECTIONS).contains(&self.max_connections) {
            Some("max_connections must be from 1 to MAX_CONNECTIONS")
        } else if !(MIN_HEADER_BYTES..=MAX_HEADER_BYTES).contains(&self.max_header_bytes) {
            Some("max_header_bytes must be from MIN_HEADER_BYTES to MAX_HEADER_BYTES")
        } else if !(1..=MAX_HEADERS).contains(&self.max_headers) {
            Some("max_headers must be from 1 to MAX_HEADERS")
        } else if self.max_body_bytes == 0 {
            Some("max_body_bytes must be at least 1")
        } else if [
            self.idle_timeout,
            self.handshake_timeout,
            self.connect_timeout,
            self.read_timeout,
        ]
        .contains(&Duration::ZERO)
        {
            Some("a timeout must not be zero")
        } else if self.resolve.iter().any(|(host, _)| host.trim().is_empty()) {
            Some("a resolve override needs a host name")
        } else {
            None
        };
        problem.map_or(Ok(()), |reason| Err(MitmError::InvalidOptions(reason)))
    }
}

/// Why the proxy could not start, swap its upstream, or serve one request.
///
/// The per-request variants are what the proxy logs when it refuses; the browser only ever sees
/// an empty status (`407`, `400`, `413` or `502`). No variant carries a token, a credential, a
/// header value or key material.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MitmError {
    /// The loopback listener could not be bound.
    #[error("the loopback listener cannot be bound: {0}")]
    Bind(#[source] io::Error),
    /// The CA, the TLS server context or a browser handshake failed. A client SNI that names
    /// another host than the `CONNECT` target is [`TlsServerError::SniMismatch`].
    #[error("the TLS side of the proxy failed: {0}")]
    Tls(#[from] TlsServerError),
    /// [`MitmOptions`] break one of their documented bounds.
    #[error("invalid MITM options: {0}")]
    InvalidOptions(&'static str),
    /// The system random generator failed to make a token.
    #[error("the system random generator failed")]
    Random,
    /// The upstream client cannot be built from the profile.
    #[error("the upstream client cannot be built: {0}")]
    Upstream(#[from] ImpersonateError),
    /// The egress cannot be used by the upstream client.
    #[error("the egress cannot be used: {0}")]
    Egress(&'static str),
    /// A request came without the proxy token, or with a wrong one.
    #[error("the proxy credentials are missing or wrong")]
    Unauthorized,
    /// A `CONNECT` or absolute-form target cannot be intercepted, so it is refused.
    #[error("the target cannot be intercepted: {0}")]
    BadTarget(&'static str),
    /// A connection or request cannot be intercepted, so it is refused instead of tunnelled.
    #[error("the request cannot be intercepted: {0}")]
    NotIntercepted(&'static str),
    /// The `Host` header of a request does not name its `CONNECT` target.
    #[error("the Host header does not name the CONNECT target")]
    HostMismatch,
    /// A request announced a body larger than the limit.
    #[error("the body is larger than {limit} bytes")]
    BodyTooLarge {
        /// The limit in force.
        limit: usize,
    },
    /// The request's `User-Agent` claims another Chrome major than the profile was measured as.
    #[error("the request claims Chrome {browser}, but the profile is Chrome {measured}; run the capture tools again")]
    VersionMismatch {
        /// The major version the user agent claims.
        browser: u32,
        /// The major version the profile was measured as.
        measured: u32,
    },
    /// The upstream request failed (connection, egress, certificate, timeout).
    #[error("the upstream request to {url} failed: {kind}")]
    UpstreamRequest {
        /// The URL, redacted.
        url: RedactedUrl,
        /// What failed, in words; never the upstream error's own text.
        kind: &'static str,
    },
}

/// Counts of what the proxy did since it started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MitmStats {
    /// Browser connections accepted.
    pub connections: u64,
    /// `CONNECT` tunnels whose TLS was ended by the proxy.
    pub tunnels: u64,
    /// Requests answered `407`.
    pub unauthorized: u64,
    /// Targets, handshakes and requests refused.
    pub refused: u64,
    /// Requests answered by the upstream.
    pub forwarded: u64,
    /// Requests answered `502` because the upstream failed.
    pub upstream_errors: u64,
}

/// The live counters behind [`MitmStats`].
#[derive(Debug, Default)]
struct Counters {
    connections: AtomicU64,
    tunnels: AtomicU64,
    unauthorized: AtomicU64,
    refused: AtomicU64,
    forwarded: AtomicU64,
    upstream_errors: AtomicU64,
}

impl Counters {
    /// Adds one to `counter`.
    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// The counts now.
    fn snapshot(&self) -> MitmStats {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        MitmStats {
            connections: read(&self.connections),
            tunnels: read(&self.tunnels),
            unauthorized: read(&self.unauthorized),
            refused: read(&self.refused),
            forwarded: read(&self.forwarded),
            upstream_errors: read(&self.upstream_errors),
        }
    }
}

/// What every connection task shares.
struct Shared {
    /// Issues the leaf of each `CONNECT` host.
    ca: Arc<EphemeralCa>,
    /// The TLS server context of the browser leg (ALPN `http/1.1`).
    acceptor: SslAcceptor,
    /// The expected `Proxy-Authorization`.
    credentials: ProxyCredentials,
    /// The client requests leave through.
    upstream: UpstreamSlot,
    /// The limits.
    options: MitmOptions,
    /// Where request and egress events go.
    sink: Arc<dyn EventSink>,
    /// What happened so far.
    counters: Counters,
}

/// The state behind every clone of a [`MitmProxy`].
struct Inner {
    /// What the connection tasks share.
    shared: Arc<Shared>,
    /// The bound loopback address.
    addr: SocketAddr,
    /// `http://sb:<token>@127.0.0.1:<port>`.
    spec: Secret,
    /// The same, parsed, for a browser context.
    browser_proxy: Proxy,
    /// Tells the accept loop and the connections to stop.
    stop: watch::Sender<bool>,
    /// The accept loop; it owns every connection task.
    task: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        let Some(task) = locked(&self.task).take() else {
            return;
        };
        let abort = task.abort_handle();
        if !spawn_cleanup(finish(task, timeout_from_env())) {
            abort.abort();
        }
    }
}

/// A loopback MITM proxy for one Chrome profile.
///
/// Clones share one listener. It stops with [`MitmProxy::shutdown`], or when the last clone is
/// dropped (the stop then runs through `common::shutdown::spawn_cleanup`, so
/// `drain_cleanups` waits for it). Either way the accept loop and every connection task end:
/// they all belong to the loop, which is aborted when the grace period runs out.
#[doc(alias = "TlsSpoofingProxy")]
#[derive(Clone)]
pub struct MitmProxy {
    inner: Arc<Inner>,
}

impl fmt::Debug for MitmProxy {
    /// Shows the address, the profile, the egress key and the counts; never the token.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let upstream = self.inner.shared.upstream.current();
        f.debug_struct("MitmProxy")
            .field("addr", &self.inner.addr)
            .field("profile", &upstream.profile().source())
            .field("egress", &upstream.egress_key())
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl MitmProxy {
    /// Binds `127.0.0.1` on a port the system picks and starts serving.
    ///
    /// Requests leave with `profile`'s TLS, HTTP/2 and header spelling, through `egress` when one
    /// is given. Each answered request is reported to `sink` as a redacted
    /// [`ScraperEvent::Request`]; each egress change as [`ScraperEvent::EgressRotated`].
    ///
    /// # Errors
    ///
    /// [`MitmError::InvalidOptions`] when `options` break a bound, [`MitmError::Tls`] when the CA
    /// or the TLS context cannot be made, [`MitmError::Upstream`] or [`MitmError::Egress`] when
    /// the upstream client cannot be built, [`MitmError::Bind`] when the port cannot be bound,
    /// [`MitmError::Random`] when no token can be drawn.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn start(
        profile: ImpersonationProfile,
        egress: Option<EgressRef>,
        options: MitmOptions,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, MitmError> {
        options.validate()?;
        let ca = match &options.ca {
            Some(ca) => Arc::clone(ca),
            None => Arc::new(EphemeralCa::generate()?),
        };
        let acceptor = tls_server::acceptor(&[ALPN_HTTP1])?;
        let upstream = UpstreamSlot::new(Upstream::build(profile, egress, &options)?);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(MitmError::Bind)?;
        let addr = listener.local_addr().map_err(MitmError::Bind)?;
        let token = random_token()?;
        let spec = Secret::new(format!(
            "{PROXY_SCHEME}://{PROXY_USER}:{}@{addr}",
            token.expose()
        ));
        let browser_proxy = Proxy::parse(spec.expose())
            .map_err(|_error| MitmError::InvalidOptions("the proxy address does not parse"))?;
        let shared = Arc::new(Shared {
            ca,
            acceptor,
            credentials: ProxyCredentials::new(PROXY_USER, &token),
            upstream,
            options,
            sink,
            counters: Counters::default(),
        });
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(connect::run(listener, Arc::clone(&shared), stopped));
        tracing::debug!(%addr, "MITM proxy listening");
        Ok(Self {
            inner: Arc::new(Inner {
                shared,
                addr,
                spec,
                browser_proxy,
                stop,
                task: Mutex::new(Some(task)),
            }),
        })
    }

    /// The loopback port the proxy listens on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.inner.addr.port()
    }

    /// The loopback address the proxy listens on; its IP is always `127.0.0.1`.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.addr
    }

    /// `http://sb:<token>@127.0.0.1:<port>`, for a launch or a client that sets its own proxy.
    /// It holds the token: pass it on, never print it.
    #[must_use]
    pub fn proxy_spec(&self) -> Secret {
        self.inner.spec.clone()
    }

    /// The proxy for a browser context (`ContextOptions::proxy`). Its `Debug` hides the token.
    #[must_use]
    pub fn browser_proxy(&self) -> Proxy {
        self.inner.browser_proxy.clone()
    }

    /// The Chrome switches the proxy needs besides `--proxy-server`: the leaf pin (never the CA
    /// pin), [`PROXY_EVERYTHING_ARG`] and [`DISABLE_QUIC_ARG`].
    #[must_use]
    pub fn chrome_args(&self) -> Vec<String> {
        chrome_args_for(&self.inner.shared.ca)
    }

    /// Points a Chrome launch at this proxy: the proxy with its token (answered over
    /// `Fetch.authRequired`, never put on the command line) and [`MitmProxy::chrome_args`].
    #[must_use]
    pub fn configure_launch(&self, builder: LaunchOptionsBuilder) -> LaunchOptionsBuilder {
        self.chrome_args()
            .into_iter()
            .fold(builder.proxy(self.inner.spec.expose()), |builder, arg| {
                builder.arg(arg)
            })
    }

    /// The counts so far.
    #[must_use]
    pub fn stats(&self) -> MitmStats {
        self.inner.shared.counters.snapshot()
    }

    /// The egress requests leave through now, `None` for a direct connection.
    #[must_use]
    pub fn egress(&self) -> Option<EgressKey> {
        self.inner.shared.upstream.current().egress_key()
    }

    /// Sends the next requests through `egress` (or directly with `None`).
    ///
    /// The new upstream client is built first; on an error nothing changes. Requests under way
    /// finish through the old egress, the listener and the browser's connections stay, and
    /// [`ScraperEvent::EgressRotated`] is emitted.
    ///
    /// # Errors
    ///
    /// [`MitmError::Egress`] or [`MitmError::Upstream`] when the new client cannot be built.
    pub fn swap_upstream(&self, egress: Option<EgressRef>) -> Result<(), MitmError> {
        let shared = &self.inner.shared;
        let (from, to) = shared.upstream.swap_egress(egress, &shared.options)?;
        tracing::info!(?from, ?to, "MITM upstream egress swapped");
        shared.sink.emit(&ScraperEvent::EgressRotated { from, to });
        Ok(())
    }

    /// Sends the next requests with `profile`'s fingerprint, through the same egress.
    ///
    /// # Errors
    ///
    /// [`MitmError::Upstream`] when the new client cannot be built; nothing changes then.
    pub fn swap_profile(&self, profile: ImpersonationProfile) -> Result<(), MitmError> {
        let shared = &self.inner.shared;
        shared.upstream.swap_profile(profile, &shared.options)
    }

    /// Stops accepting, lets requests under way finish for up to `grace`, then aborts what is
    /// left. Every clone stops with it.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn shutdown(self, grace: Duration) {
        self.inner.stop.send_replace(true);
        let task = locked(&self.inner.task).take();
        if let Some(task) = task {
            finish(task, grace).await;
        }
    }
}

/// The Chrome switches for a proxy whose leaves `ca` issues.
fn chrome_args_for(ca: &EphemeralCa) -> Vec<String> {
    vec![
        ca.chrome_spki_arg(),
        PROXY_EVERYTHING_ARG.to_owned(),
        DISABLE_QUIC_ARG.to_owned(),
    ]
}

/// Waits up to `grace` for the accept loop, then aborts it, which drops and so aborts every
/// connection task it owns.
async fn finish(mut task: JoinHandle<()>, grace: Duration) {
    let ended = cleanup_within(grace, async {
        let _joined = (&mut task).await;
    })
    .await;
    if !ended {
        task.abort();
        let _aborted = task.await;
    }
}

/// A new proxy password: 128 random bits, hex encoded.
fn random_token() -> Result<Secret, MitmError> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_unspecified| MitmError::Random)?;
    Ok(Secret::new(hex(&bytes)))
}

/// Lower-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}
