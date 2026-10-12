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
//!    is ever tunnelled blind: a tunnel whose first bytes are neither a TLS handshake for the
//!    `CONNECT` host nor a plain-text WebSocket handshake (`ws://`, see [WebSocket](#websocket))
//!    is closed;
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
//! ([`MitmOptions::idle_timeout`]), the browser's TLS handshake, the upstream connect and read
//! phases, and a relayed WebSocket's idle time and bytes in each direction
//! ([`MitmOptions::websocket_idle_timeout`], [`MitmOptions::max_websocket_bytes`]) all have
//! limits.
//!
//! # Destinations
//!
//! A page must not be able to use the proxy to reach the machine itself or its network. A direct
//! proxy (no egress) connects only to public addresses: a name is resolved by the proxy, every
//! internal address in the answer (loopback, private, link-local, carrier-grade NAT, multicast,
//! reserved, cloud metadata, and the IPv6 forms that wrap them) is dropped, and the connection
//! goes to one of the addresses that passed, so DNS rebinding has nothing to swap. Literal
//! addresses and `localhost` names are refused at the `CONNECT` or in the request line, with or
//! without an egress. [`MitmOptions::allow_private_destinations`] turns the rule off.
//!
//! # Hot-swappable upstream
//!
//! [`MitmProxy::swap_upstream`] and [`MitmProxy::swap_profile`] build the new upstream client
//! first and then replace the old one; requests already under way finish on the old one and the
//! listener, its port and the browser's open connections stay.
//!
//! # WebSocket
//!
//! A `wss://` connection reaches the proxy as a `CONNECT` whose TLS the proxy ends, followed by the
//! RFC 6455 opening handshake (`GET` with `Upgrade: websocket` and `Connection: Upgrade`). The
//! handshake passes the same checks as any request (token, destination, `Host`, user agent) and
//! goes upstream over HTTP/1.1 with the measured WebSocket `ClientHello` (Chrome offers only
//! `http/1.1` there, so its JA4 differs from the ordinary one), in the browser's header order with
//! the hop-by-hop fields dropped except the `Connection: Upgrade` and `Upgrade: websocket` pair,
//! which the proxy writes itself. The upstream's `101 Switching Protocols` is relayed as it is;
//! checking `Sec-WebSocket-Accept` stays with the browser. From then on the proxy copies bytes in
//! both directions without reading frames, until a side closes, the connection has carried no byte
//! for [`MitmOptions::websocket_idle_timeout`], one direction has carried more than
//! [`MitmOptions::max_websocket_bytes`], or the proxy stops; then both sides are shut down.
//!
//! A `ws://` connection reaches the proxy as a `CONNECT` too (measured on Chrome 155: to port 80
//! when the URL names none), but the tunnel then carries the opening handshake in plain text, with
//! the same fields in the same order. The proxy reads the tunnel's first bytes: a TLS handshake is
//! ended as above; `GET ` is read as HTTP/1.1, and a WebSocket handshake in it takes exactly the
//! path of a `wss://` one (the same checks, `Host` against the `CONNECT` target, the same header
//! rewrite, bounds and relay), sent upstream as `http://` over a plain connection, so no
//! `ClientHello` is involved. Any other plain-text request is refused with an empty `502` (`400`
//! for half an upgrade) and the tunnel closed, and bytes that start neither way close it
//! unanswered: plain text is relayed only after an upgrade the upstream accepted, as for `wss://`.
//! Through an egress, the plain handshake leaves the way the upstream client sends any `http://`
//! request, so an HTTP egress has to relay the upgrade itself; a `ws://` WebSocket through an
//! egress is not tested.
//!
//! A custom profile's WebSocket connections send the measured WebSocket hello of its base browser.
//! A profile without a measured WebSocket hello gets an empty `502` for every handshake, `ws://`
//! included.
//!
//! # The priming page
//!
//! Chrome gets the token over `Fetch.authRequired`, which `ProxyAuth` answers for the requests of
//! a tab. A WebSocket's `CONNECT` never reaches it: measured on Chrome 155 (headless), a WebSocket
//! that is the first request through the proxy gets the `407` and is not retried with the token,
//! so it fails, and `Fetch.enable` on the browser target sees nothing of it either. Credentials
//! Chrome was given are kept in a cache that every tab of the browser context shares, and are sent
//! with each later `CONNECT`, a WebSocket's included (measured: one tab's navigation, then another
//! tab's first-request WebSocket goes through). A browser context created later does not share
//! the default context's cache (measured as well).
//!
//! So the proxy owns one page, [`MitmProxy::prime_url`]: `https://` [`PRIME_HOST`] and a random
//! path. After the token check, a `GET` of exactly that is answered by the proxy itself with an
//! empty page that is never cached. Any other request for that host (another path, the browser's
//! `favicon.ico`) gets an empty `404`; nothing for that host goes upstream, and no event is
//! emitted. [`MitmProxy::configure_launch`] makes the priming page the start page, so
//! `Browser::launch` loads it in the first tab (through the `407` and `ProxyAuth`) before it
//! returns, and from then on a WebSocket may be a new tab's first request. A context created with
//! [`MitmProxy::browser_proxy`] takes the credentials the same way, by loading the priming page in
//! its first tab. The priming URL holds no token, and the token still reaches Chrome only through
//! `ProxyAuth`.
//!
//! # Not intercepted
//!
//! Every protocol upgrade other than WebSocket (`h2c`, `TLS/1.2`, a list of protocols) is refused
//! with an empty `502`, and a request that only half asks for one (`Connection: upgrade` without
//! `Upgrade`, `Upgrade` without `Connection: upgrade`, a method other than `GET`, a body) with an
//! empty `400`. A WebSocket handshake outside a `CONNECT` tunnel (an absolute-form `http://`
//! request, which Chrome never sends for `ws://`) is refused with an empty `502`. QUIC is off in
//! Chrome, so there is no HTTP/3. Requests whose `User-Agent` claims a Chrome major other than
//! the profile's are refused, because the measured TLS and HTTP/2 would contradict the header.
//!
//! # HTTP/1.1 origins
//!
//! The upstream protocol is settled by ALPN when the connection opens, after the request was
//! prepared. The first request to an origin is prepared for HTTP/2, so when that origin turns out
//! to speak HTTP/1.1 only, its first request carries the HTTP/2-only `priority` header; the proxy
//! then remembers the origin and its later requests do not. On HTTP/1.1 the header names take the
//! measured spelling; a name outside the measured lists keeps the lower case the browser leg
//! delivers.
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
mod destination;
mod forward;
mod sniff;
mod upstream;
mod websocket;

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
/// The host of the proxy's priming page ([`MitmProxy::prime_url`]). Under `.invalid` (RFC 6761),
/// so it never names a real site.
pub const PRIME_HOST: &str = "sb-mitm.invalid";
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
/// How long a relayed WebSocket may carry no byte in either direction by default.
pub const DEFAULT_WEBSOCKET_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// The most bytes a relayed WebSocket carries in each direction by default.
pub const DEFAULT_MAX_WEBSOCKET_BYTES: u64 = 512 * 1024 * 1024;

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
    /// How long a relayed WebSocket may carry no byte in either direction.
    websocket_idle_timeout: Duration,
    /// The most bytes a relayed WebSocket carries in each direction.
    max_websocket_bytes: u64,
    /// DER roots that replace the built-in web roots for upstream verification, when not empty.
    upstream_roots: Vec<Vec<u8>>,
    /// Host names the upstream client resolves to fixed addresses.
    resolve: Vec<(String, SocketAddr)>,
    /// The CA to issue leaves with, instead of a new one.
    ca: Option<Arc<EphemeralCa>>,
    /// Whether loopback, private, link-local and similar addresses may be destinations.
    allow_private_destinations: bool,
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
            websocket_idle_timeout: DEFAULT_WEBSOCKET_IDLE_TIMEOUT,
            max_websocket_bytes: DEFAULT_MAX_WEBSOCKET_BYTES,
            upstream_roots: Vec::new(),
            resolve: Vec::new(),
            ca: None,
            allow_private_destinations: false,
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
            .field("websocket_idle_timeout", &self.websocket_idle_timeout)
            .field("max_websocket_bytes", &self.max_websocket_bytes)
            .field("upstream_roots", &self.upstream_roots.len())
            .field("resolve", &self.resolve.len())
            .field("ca", &self.ca)
            .field(
                "allow_private_destinations",
                &self.allow_private_destinations,
            )
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

    /// How long a relayed WebSocket may carry no byte in either direction before both of its
    /// sides are closed. Not zero. Chrome sends no keep-alive of its own on a WebSocket, so a
    /// site that pings less often than this loses its idle connections.
    #[must_use]
    pub fn websocket_idle_timeout(mut self, timeout: Duration) -> Self {
        self.websocket_idle_timeout = timeout;
        self
    }

    /// The most bytes a relayed WebSocket carries in each direction, at least 1. The read that
    /// crosses the limit is not passed on, and both sides are closed; frames are not read, so a
    /// single frame larger than the limit ends the connection.
    #[must_use]
    pub fn max_websocket_bytes(mut self, bytes: u64) -> Self {
        self.max_websocket_bytes = bytes;
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
    ///
    /// An address that is loopback, private or otherwise internal is refused like any other
    /// answer, unless [`MitmOptions::allow_private_destinations`] is on.
    #[must_use]
    pub fn resolve(mut self, host: impl Into<String>, addr: SocketAddr) -> Self {
        self.resolve.push((host.into(), addr));
        self
    }

    /// Lets requests go to loopback, private, link-local, shared (carrier-grade NAT), multicast
    /// and reserved addresses, the cloud metadata addresses, and to `localhost`. Off by default.
    ///
    /// Chrome never resolves a name it gives a proxy, so only this proxy can tell that a page
    /// is asking for the machine's own services or for the local network. With the switch off,
    /// a direct proxy (no egress) connects to public addresses only: it checks every address a
    /// name resolves to and connects to the checked address itself, so a DNS answer that
    /// changes between a check and the connection gains nothing, and it refuses literal internal
    /// addresses and `localhost` names with an empty `502`. Through an egress the names are
    /// resolved on the egress's side, so only the literal addresses and `localhost` names are
    /// refused.
    ///
    /// Turn it on for a site on the local network, a test server on loopback, or addresses named
    /// with [`MitmOptions::resolve`] that are internal. Everything the browser loads can then
    /// reach those addresses: do it only for a browser that loads nothing else.
    #[must_use]
    pub fn allow_private_destinations(mut self, allow: bool) -> Self {
        self.allow_private_destinations = allow;
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
        } else if self.max_websocket_bytes == 0 {
            Some("max_websocket_bytes must be at least 1")
        } else if [
            self.idle_timeout,
            self.handshake_timeout,
            self.connect_timeout,
            self.read_timeout,
            self.websocket_idle_timeout,
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
/// an empty status (`407`, `400`, `403`, `404`, `413`, `431` or `502`). No variant carries a
/// token, a credential, a header value or key material.
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
    /// The target is a local, private or reserved address, or a `localhost` name, and
    /// [`MitmOptions::allow_private_destinations`] is off.
    #[error("the destination is not allowed: {0}")]
    ForbiddenDestination(&'static str),
    /// A connection or request cannot be intercepted, so it is refused instead of tunnelled.
    #[error("the request cannot be intercepted: {0}")]
    NotIntercepted(&'static str),
    /// The `Host` header of a request does not name its `CONNECT` target.
    #[error("the Host header does not name the CONNECT target")]
    HostMismatch,
    /// A request asks for a protocol upgrade in a malformed way: `Connection: upgrade` without
    /// `Upgrade`, `Upgrade` without `Connection: upgrade`, or a WebSocket handshake that is not
    /// a bodiless HTTP/1.1 `GET`.
    #[error("the upgrade request is malformed: {0}")]
    MalformedUpgrade(&'static str),
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
    /// WebSocket handshakes the upstream accepted, whose bytes the proxy then relayed; each is
    /// counted in `forwarded` too.
    pub websockets: u64,
    /// Loads of the priming page ([`MitmProxy::prime_url`]), answered by the proxy itself.
    pub primed: u64,
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
    websockets: AtomicU64,
    primed: AtomicU64,
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
            websockets: read(&self.websockets),
            primed: read(&self.primed),
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
    /// The path of the priming page on [`PRIME_HOST`]: `/` and 128 random bits in hex.
    prime_path: String,
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
        let prime_path = format!("/{}", random_hex()?);
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
            prime_path,
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

    /// The address of the proxy's priming page: `https://` [`PRIME_HOST`] and a path of 128
    /// random bits drawn when the proxy started.
    ///
    /// The proxy answers a `GET` of it itself, with an empty page that is never cached, and sends
    /// nothing for that host upstream (any other request for it gets an empty `404`). Loading it is
    /// how a browser context takes the proxy's credentials before its first WebSocket: Chrome hands
    /// the token over only when a tab's request meets the `407`, and a WebSocket's own `CONNECT` is
    /// not such a request (see the module documentation). [`MitmProxy::configure_launch`] makes it
    /// the start page; a context created with [`MitmProxy::browser_proxy`] loads it in its first
    /// tab.
    ///
    /// The path holds no token and is not a secret, but it cannot be guessed, so a page that was
    /// not given it only ever sees the `404`.
    #[must_use]
    pub fn prime_url(&self) -> String {
        format!("https://{PRIME_HOST}{}", self.inner.shared.prime_path)
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
    /// `Fetch.authRequired`, never put on the command line), [`MitmProxy::chrome_args`], and
    /// [`MitmProxy::prime_url`] as the start page, so that the browser holds the proxy's
    /// credentials before any WebSocket.
    ///
    /// A start page already set on `builder` is kept; an `http://` or `https://` one takes the
    /// credentials as well, as any page through the proxy does, but an `about:` or `data:` one does
    /// not. A start page set after this call replaces the priming page.
    #[must_use]
    pub fn configure_launch(&self, builder: LaunchOptionsBuilder) -> LaunchOptionsBuilder {
        // The builder has no getter, so its start page is read from a copy of what it builds.
        let has_start_page = builder
            .clone()
            .build()
            .is_ok_and(|options| options.url.is_some());
        let builder = self
            .chrome_args()
            .into_iter()
            .fold(builder.proxy(self.inner.spec.expose()), |builder, arg| {
                builder.arg(arg)
            });
        if has_start_page {
            builder
        } else {
            builder.url(self.prime_url())
        }
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
    random_hex().map(Secret::new)
}

/// 128 random bits, hex encoded.
fn random_hex() -> Result<String, MitmError> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_unspecified| MitmError::Random)?;
    Ok(hex(&bytes))
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
