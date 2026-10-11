//! The ephemeral CA and the TLS acceptor for the MITM proxy: one leaf per host, pinned by SPKI,
//! never written to disk.
//!
//! [`EphemeralCa`] creates a P-256 certificate authority in memory for the life of one process,
//! plus one shared P-256 key that every leaf certifies. [`EphemeralCa::leaf_for`] issues a leaf
//! for one CONNECT host (a DNS name or an IP address, never a wildcard) and keeps it in a
//! count-bounded LRU cache until it nears expiry. [`acceptor`] builds the `BoringSSL` server
//! context (TLS 1.2 and 1.3, ALPN from a fixed preference list, no session resumption) and
//! [`accept`] runs the handshake of one connection with the leaf of its CONNECT host, then
//! refuses the connection when the client's SNI names another host.
//!
//! Chrome trusts the CA only through `--ignore-certificate-errors-spki-list` with the two pins of
//! [`EphemeralCa::spki_pins`] (see [`EphemeralCa::chrome_spki_arg`]). The CA is never installed
//! in a trust store, and `--ignore-certificate-errors` is never used.
//!
//! # Loopback only
//!
//! The CA exists so that a local browser can talk to a proxy on `127.0.0.1`. Nothing here binds
//! a socket; the caller binds loopback only. The private keys live in `BoringSSL` memory and never
//! appear in `Debug`, logs, errors or files. They are freed, not wiped, when the CA drops
//! (`BoringSSL` offers no zeroize hook), which is acceptable for a loopback CA that lives no
//! longer than its process.
//!
//! # Example
//!
//! ```no_run
//! use std::time::SystemTime;
//! use seleniumbase_rs::impersonate::tls_server::{accept, acceptor, EphemeralCa, ALPN_HTTP1,
//!     DEFAULT_HANDSHAKE_TIMEOUT};
//!
//! # async fn demo(tcp: tokio::net::TcpStream) -> Result<(), Box<dyn std::error::Error>> {
//! let ca = EphemeralCa::generate()?;
//! let chrome_flag = ca.chrome_spki_arg(); // pass to Chrome together with --user-data-dir
//! let server = acceptor(&[ALPN_HTTP1])?;
//! let host = url::Host::parse("example.com")?;
//! let leaf = ca.leaf_for(&host, SystemTime::now())?;
//! let tls = accept(&server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await?;
//! # drop((chrome_flag, tls));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

mod ca;
mod cache;
mod host;

use std::pin::Pin;
use std::time::Duration;

use btls::error::ErrorStack;
use btls::ssl::{
    AlpnError, NameType, Ssl, SslAcceptor, SslMethod, SslOptions, SslSessionCacheMode, SslVersion,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_btls::SslStream;

pub use ca::{spki_pin_of, EphemeralCa, Leaf};

/// The ALPN id of HTTP/1.1, the only protocol the browser-to-proxy leg negotiates.
pub const ALPN_HTTP1: &[u8] = b"http/1.1";
/// How long [`accept`] waits for a client to finish its handshake, unless told otherwise.
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// How many leaves the cache holds by default before it evicts the least recently used one.
pub const DEFAULT_LEAF_CAPACITY: usize = 256;
/// The largest leaf cache [`CaOptions`] accept.
pub const MAX_LEAF_CAPACITY: usize = 4096;
/// How long the CA certificate is valid by default, counted from its creation.
pub const DEFAULT_CA_VALIDITY: Duration = Duration::from_secs(7 * SECONDS_PER_DAY);
/// The longest CA validity [`CaOptions`] accept: the CA is per process, so it stays short.
pub const MAX_CA_VALIDITY: Duration = Duration::from_secs(90 * SECONDS_PER_DAY);
/// How long a leaf is valid by default, counted from its issue and capped at the CA's expiry.
pub const DEFAULT_LEAF_VALIDITY: Duration = Duration::from_secs(SECONDS_PER_DAY);
/// A cached leaf is issued again once it is this close to expiry, by default.
pub const DEFAULT_LEAF_RENEW_MARGIN: Duration = Duration::from_secs(SECONDS_PER_HOUR);
/// Every certificate starts this long before its issue, so a slightly slow client clock accepts
/// it.
pub const CLOCK_SKEW_ALLOWANCE: Duration = Duration::from_secs(SECONDS_PER_HOUR);

/// Seconds in one hour.
const SECONDS_PER_HOUR: u64 = 3600;
/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;
/// The longest protocol id an ALPN list can carry (its length is one byte).
const MAX_ALPN_PROTOCOL_LEN: usize = 255;

/// Lifetimes and the cache bound of an [`EphemeralCa`]. Every field is a custom input, checked
/// by [`EphemeralCa::generate_at`]. The durations count in whole seconds: a certificate is
/// issued for the whole seconds of its validity, and the bounds below are checked on those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaOptions {
    /// How long the CA certificate is valid, from creation. At most [`MAX_CA_VALIDITY`].
    pub ca_validity: Duration,
    /// How long a leaf is valid, from issue, capped at the CA's expiry. At most `ca_validity`.
    pub leaf_validity: Duration,
    /// A cached leaf this close to expiry is issued again. Shorter than `leaf_validity`.
    pub leaf_renew_margin: Duration,
    /// How many leaves the cache holds. From 1 to [`MAX_LEAF_CAPACITY`].
    pub leaf_capacity: usize,
}

impl Default for CaOptions {
    fn default() -> Self {
        Self {
            ca_validity: DEFAULT_CA_VALIDITY,
            leaf_validity: DEFAULT_LEAF_VALIDITY,
            leaf_renew_margin: DEFAULT_LEAF_RENEW_MARGIN,
            leaf_capacity: DEFAULT_LEAF_CAPACITY,
        }
    }
}

impl CaOptions {
    /// Checks the documented bounds of every field. Certificates are issued in whole seconds,
    /// dropping any sub-second part, so the durations are compared the same way: a leaf valid
    /// for 3600.5 s with a renew margin of 3600 s is refused, as the leaf would last 3600 s.
    fn validate(&self) -> Result<(), TlsServerError> {
        let ca_validity = self.ca_validity.as_secs();
        let leaf_validity = self.leaf_validity.as_secs();
        let problem = if self.leaf_capacity == 0 || self.leaf_capacity > MAX_LEAF_CAPACITY {
            Some("leaf_capacity must be from 1 to MAX_LEAF_CAPACITY")
        } else if ca_validity > MAX_CA_VALIDITY.as_secs() {
            Some("ca_validity exceeds MAX_CA_VALIDITY")
        } else if leaf_validity > ca_validity {
            Some("leaf_validity exceeds ca_validity")
        } else if self.leaf_renew_margin.as_secs() >= leaf_validity {
            Some("leaf_renew_margin must be shorter than leaf_validity")
        } else {
            None
        };
        problem.map_or(Ok(()), |reason| Err(TlsServerError::InvalidOptions(reason)))
    }
}

/// Everything that can go wrong creating the CA, issuing a leaf or serving a handshake.
///
/// No variant carries key material: `BoringSSL` error stacks name the failing call only.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TlsServerError {
    /// `BoringSSL` refused an operation (key generation, certificate building, context setup).
    #[error("BoringSSL refused the operation: {0}")]
    Ssl(#[from] ErrorStack),
    /// The TLS handshake with the client failed.
    #[error("TLS handshake failed: {0}")]
    Handshake(#[source] btls::ssl::Error),
    /// The client did not finish its handshake in time.
    #[error("TLS handshake did not finish within {limit:?}")]
    HandshakeTimeout {
        /// The time the client had.
        limit: Duration,
        /// The timer that ran out.
        #[source]
        source: tokio::time::error::Elapsed,
    },
    /// The host cannot be named in a leaf certificate.
    #[error("the host cannot be named in a leaf certificate: {0}")]
    InvalidHost(&'static str),
    /// The ALPN list given to [`acceptor`] is empty or malformed.
    #[error("invalid ALPN protocol list: {0}")]
    InvalidAlpn(&'static str),
    /// [`CaOptions`] break one of their documented bounds.
    #[error("invalid CA options: {0}")]
    InvalidOptions(&'static str),
    /// The clock reads before 1970, or too far ahead for a certificate.
    #[error("the clock is outside the range a certificate can carry")]
    ClockOutOfRange,
    /// The CA expires within the renew margin, so no useful leaf can be issued.
    #[error("the ephemeral CA is about to expire; create a new one")]
    CaExpired,
    /// The client's SNI names another host than the CONNECT target, or is missing for a DNS host.
    #[error("TLS SNI {sni:?} does not match the CONNECT host {expected}")]
    SniMismatch {
        /// The CONNECT host the leaf was issued for.
        expected: String,
        /// The SNI the client sent, cut to the longest DNS name.
        sni: Option<String>,
    },
}

/// Builds the server context for intercepted connections.
///
/// TLS 1.2 and 1.3, the `BoringSSL` intermediate cipher set, no session tickets and no session
/// cache (every connection presents the leaf of its own CONNECT host). `alpn` lists the
/// protocols to accept in order of preference; the browser-to-proxy leg passes
/// `&[ALPN_HTTP1]`. A client whose ALPN offer shares no protocol with the list gets a fatal
/// `no_application_protocol` alert; a client that offers no ALPN at all is served.
///
/// # Errors
///
/// [`TlsServerError::InvalidAlpn`] when `alpn` is empty or holds an id that is empty or longer
/// than 255 bytes; [`TlsServerError::Ssl`] when `BoringSSL` refuses the configuration.
pub fn acceptor(alpn: &[&[u8]]) -> Result<SslAcceptor, TlsServerError> {
    let preferences = alpn_preferences(alpn)?;
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls())?;
    builder.set_min_proto_version(Some(SslVersion::TLS1_2))?;
    builder.set_options(SslOptions::NO_TICKET);
    builder.set_session_cache_mode(SslSessionCacheMode::OFF);
    builder.set_alpn_select_callback(move |_ssl, offered| {
        select_alpn(&preferences, offered).ok_or(AlpnError::ALERT_FATAL)
    });
    Ok(builder.build())
}

/// Runs the server side of one TLS handshake with `leaf` and checks the client's SNI.
///
/// The leaf, the shared leaf key and the CA certificate (as the chain) are set on this
/// connection only, so one [`acceptor`] serves every host, IP hosts included. After the
/// handshake, a DNS host requires an SNI equal to it (ASCII case-insensitive) and an IP host
/// requires no SNI; otherwise the stream is dropped before any application byte is read.
///
/// # Errors
///
/// [`TlsServerError::HandshakeTimeout`] when the client takes longer than `handshake_timeout`,
/// [`TlsServerError::Handshake`] when the handshake fails, [`TlsServerError::SniMismatch`] when
/// the SNI does not name the leaf's host, [`TlsServerError::Ssl`] when the connection cannot be
/// set up.
#[tracing::instrument(level = "debug", skip_all)]
pub async fn accept<S>(
    acceptor: &SslAcceptor,
    leaf: &Leaf,
    stream: S,
    handshake_timeout: Duration,
) -> Result<SslStream<S>, TlsServerError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut ssl = Ssl::new(acceptor.context())?;
    leaf.install(&mut ssl)?;
    let mut tls = SslStream::new(ssl, stream)?;
    tokio::time::timeout(handshake_timeout, Pin::new(&mut tls).accept())
        .await
        .map_err(|source| TlsServerError::HandshakeTimeout {
            limit: handshake_timeout,
            source,
        })?
        .map_err(TlsServerError::Handshake)?;
    leaf.name()
        .check_sni(tls.ssl().servername(NameType::HOST_NAME))?;
    Ok(tls)
}

/// Copies and checks the server's ALPN preference list.
fn alpn_preferences(alpn: &[&[u8]]) -> Result<Vec<Vec<u8>>, TlsServerError> {
    if alpn.is_empty() {
        return Err(TlsServerError::InvalidAlpn("no protocol given"));
    }
    alpn.iter()
        .map(|id| {
            if id.is_empty() || id.len() > MAX_ALPN_PROTOCOL_LEN {
                Err(TlsServerError::InvalidAlpn(
                    "a protocol id must be 1 to 255 bytes long",
                ))
            } else {
                Ok(id.to_vec())
            }
        })
        .collect()
}

/// Picks the first of `preferences` that the client offers, borrowing it from the client's
/// ALPN wire list (length-prefixed ids). A malformed tail of the list is ignored.
fn select_alpn<'a>(preferences: &[Vec<u8>], offered: &'a [u8]) -> Option<&'a [u8]> {
    preferences
        .iter()
        .find_map(|wanted| alpn_ids(offered).find(|id| *id == wanted.as_slice()))
}

/// Iterates the ids of an ALPN wire list, stopping at the first malformed entry.
fn alpn_ids(wire: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = wire;
    std::iter::from_fn(move || {
        let (&len, tail) = rest.split_first()?;
        let len = usize::from(len);
        if len == 0 || len > tail.len() {
            rest = &[];
            return None;
        }
        let (id, after) = tail.split_at(len);
        rest = after;
        Some(id)
    })
}

#[cfg(test)]
mod tests;
