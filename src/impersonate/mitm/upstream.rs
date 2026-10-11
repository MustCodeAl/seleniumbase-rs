//! The upstream client behind the proxy, replaced whole when the egress or the profile changes.
//!
//! It is the impersonating client of `impersonate::client` with what a proxy needs on top: no
//! redirects (the browser follows them), no decompression (`Content-Encoding` passes through),
//! no default headers (the browser's own go out), verification always on, and bounded connect
//! and read phases. Beside it, a second client sends the measured WebSocket `ClientHello`
//! (HTTP/1.1 only) for WebSocket handshakes, with the same egress, resolver, roots and timeouts.
//! Of the proxy's files, only this one names wreq types.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use http::{HeaderMap, HeaderName, HeaderValue, Method, Response, StatusCode, Version};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use wreq::header::OrigHeaderMap;
use wreq::tls::trust::CertStore;

use super::body::Capped;
use super::destination::{self, PublicOnly};
use super::forward::ProxyBody;
use super::websocket::RelayIo;
use super::{MitmError, MitmOptions};
use crate::impersonate::client::{impersonation_client, ImpersonateError, ImpersonationProfile};
use crate::impersonate::convert;
use crate::sb_cdp::sync::locked;
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::wire::client_hello::ext_id;
use crate::wire::emulation;
use crate::wire::header_spec::RequestShape;
use crate::wire::tls_spec::TlsSpec;

/// How `Host` is spelled on HTTP/1.1, where the client writes it first.
const HOST_SPELLING: &str = "Host";
/// The HTTP/2-only priority header (RFC 9218).
const PRIORITY: &str = "priority";
/// The header the measured priority hangs on.
const SEC_FETCH_DEST: &str = "sec-fetch-dest";
/// Header values `Debug` must never print.
const SENSITIVE: [&str; 2] = ["cookie", "authorization"];
/// How many HTTP/1.1-only origins are remembered before the memory starts over.
const MAX_REMEMBERED_ORIGINS: usize = 1024;
/// How Chrome spells the fields of a WebSocket handshake that the measured lists may not hold
/// (Chromium's `websocket_handshake_constants`).
const WEBSOCKET_SPELLING: [&str; 9] = [
    "Connection",
    "Upgrade",
    "Pragma",
    "Cache-Control",
    "Origin",
    "Sec-WebSocket-Version",
    "Sec-WebSocket-Key",
    "Sec-WebSocket-Extensions",
    "Sec-WebSocket-Protocol",
];

/// The connection a request leaves on, which decides the spelling and the extra headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leg {
    /// HTTP/2: lower-case names and the measured `priority`.
    Http2,
    /// HTTP/1.1: `Host` first and the measured spelling.
    Http1,
    /// A WebSocket handshake: HTTP/1.1, with Chrome's spelling of the handshake fields.
    WebSocket,
}

/// What the upstream did with a WebSocket handshake.
pub(super) enum Handshake {
    /// It answered `101`: its headers, and the switched connection.
    Switched {
        /// The headers of the `101`.
        headers: HeaderMap,
        /// The connection, switched.
        stream: Box<dyn RelayIo>,
    },
    /// It answered anything else, which goes to the browser as an ordinary answer.
    Answered(Response<ProxyBody>),
}

/// One request as it leaves the proxy, before the wire spelling is applied.
pub(super) struct Outbound {
    /// The method.
    pub(super) method: Method,
    /// The full URL.
    pub(super) url: String,
    /// `scheme://host:port`, for the protocol memory.
    pub(super) origin: String,
    /// The browser's headers in its order, hop-by-hop fields removed.
    pub(super) headers: Vec<(HeaderName, HeaderValue)>,
    /// The body, unless the browser sent none.
    pub(super) body: Option<Capped<Incoming>>,
}

/// The client requests leave through, with the profile and egress it was built from.
pub(super) struct Upstream {
    /// The impersonating client.
    client: wreq::Client,
    /// The client of WebSocket handshakes, `None` when no WebSocket hello is measured for the
    /// profile's browser.
    websocket: Option<wreq::Client>,
    /// The profile whose TLS, HTTP/2 and spelling the client sends.
    profile: ImpersonationProfile,
    /// The egress, `None` for a direct connection.
    egress: Option<EgressRef>,
    /// Measured spelling of header names, for HTTP/1.1 upstreams.
    spelling: HashMap<HeaderName, String>,
    /// Origins that answered over HTTP/1.1, which get no `priority` header.
    http1_origins: Mutex<HashSet<String>>,
}

impl fmt::Debug for Upstream {
    /// The profile source and the egress key; never a credential.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upstream")
            .field("profile", &self.profile.source())
            .field("egress", &self.egress_key())
            .finish_non_exhaustive()
    }
}

impl Upstream {
    /// Builds the client for `profile` through `egress`.
    pub(super) fn build(
        profile: ImpersonationProfile,
        egress: Option<EgressRef>,
        options: &MitmOptions,
    ) -> Result<Self, MitmError> {
        let client = client(&profile, egress.as_ref(), options)?;
        let websocket = websocket_spec(&profile)
            .map(|spec| websocket_client(&spec, egress.as_ref(), options))
            .transpose()?;
        let spelling = spelling_of(&profile);
        Ok(Self {
            client,
            websocket,
            profile,
            egress,
            spelling,
            http1_origins: Mutex::default(),
        })
    }

    /// The profile the client sends.
    pub(super) fn profile(&self) -> &ImpersonationProfile {
        &self.profile
    }

    /// The Chrome major the profile was measured as.
    pub(super) fn major(&self) -> u32 {
        self.profile.major()
    }

    /// The egress key, `None` for a direct connection.
    pub(super) fn egress_key(&self) -> Option<EgressKey> {
        self.egress.as_ref().map(|egress| egress.key().clone())
    }

    /// Whether WebSocket handshakes can leave with a measured hello.
    pub(super) fn speaks_websocket(&self) -> bool {
        self.websocket.is_some()
    }

    /// Sends a WebSocket handshake over HTTP/1.1 with the measured WebSocket hello. A `101`
    /// comes back switched; any other answer with its body capped at `max_body`. The error names
    /// what failed in words, as [`Upstream::send`] does.
    pub(super) async fn open_websocket(
        &self,
        url: String,
        headers: Vec<(HeaderName, HeaderValue)>,
        max_body: usize,
    ) -> Result<Handshake, &'static str> {
        let client = self
            .websocket
            .as_ref()
            .ok_or("no WebSocket ClientHello is measured for this profile")?;
        let (headers, order) = self.wire_headers(headers, Leg::WebSocket);
        let response = client
            .get(url)
            .version(Version::HTTP_11)
            .default_headers(false)
            .headers(headers)
            .orig_headers(order)
            .send()
            .await
            .map_err(|error| failure_kind(&error))?;
        if response.status() != StatusCode::SWITCHING_PROTOCOLS {
            let response: Response<wreq::Body> = response.into();
            return Ok(Handshake::Answered(
                response.map(|body| Capped::new(body, max_body).boxed_unsync()),
            ));
        }
        let headers = response.headers().clone();
        let stream = response
            .upgrade()
            .await
            .map_err(|_error| "the upstream connection was not handed over after its 101")?;
        Ok(Handshake::Switched {
            headers,
            stream: Box::new(stream),
        })
    }

    /// Sends one request and returns the answer with its body capped at `max_body`. The error
    /// names what failed in words; the client's own error text, which can hold the URL, is
    /// never kept.
    pub(super) async fn send(
        &self,
        outbound: Outbound,
        max_body: usize,
    ) -> Result<Response<ProxyBody>, &'static str> {
        let leg = if locked(&self.http1_origins).contains(&outbound.origin) {
            Leg::Http1
        } else {
            Leg::Http2
        };
        let (headers, order) = self.wire_headers(outbound.headers, leg);
        let mut request = self
            .client
            .request(outbound.method, outbound.url)
            .default_headers(false)
            .headers(headers)
            .orig_headers(order);
        if let Some(body) = outbound.body {
            request = request.body(wreq::Body::wrap(body));
        }
        let response = request.send().await.map_err(|error| failure_kind(&error))?;
        self.remember(outbound.origin, response.version());
        let response: Response<wreq::Body> = response.into();
        Ok(response.map(|body| Capped::new(body, max_body).boxed_unsync()))
    }

    /// The headers and their wire order: the browser's list with the measured spelling,
    /// `Host` first on HTTP/1.1 (the client fills it in), on HTTP/2 the measured `priority` for
    /// the request's destination at the end, as Chrome sends it, and on a WebSocket handshake
    /// Chrome's spelling of the handshake fields the measured lists do not hold.
    fn wire_headers(
        &self,
        list: Vec<(HeaderName, HeaderValue)>,
        leg: Leg,
    ) -> (HeaderMap, OrigHeaderMap) {
        let mut map = HeaderMap::with_capacity(list.len() + 1);
        let mut order = OrigHeaderMap::with_capacity(list.len() + 2);
        if leg != Leg::Http2 {
            order.insert(HOST_SPELLING);
        }
        let priority = if leg == Leg::Http2 {
            self.priority_for(&list)
        } else {
            None
        };
        for (name, mut value) in list {
            if SENSITIVE.contains(&name.as_str()) {
                value.set_sensitive(true);
            }
            let spelled = match self.spelling.get(&name) {
                Some(measured) => measured.clone(),
                None if leg == Leg::WebSocket => websocket_spelling(&name)
                    .map_or_else(|| name.as_str().to_owned(), str::to_owned),
                None => name.as_str().to_owned(),
            };
            order.insert(spelled);
            map.append(name, value);
        }
        if let Some(value) = priority {
            order.insert(PRIORITY);
            map.append(HeaderName::from_static(PRIORITY), value);
        }
        (map, order)
    }

    /// The measured `priority` for the request's `Sec-Fetch-Dest`, unless it has one already.
    fn priority_for(&self, list: &[(HeaderName, HeaderValue)]) -> Option<HeaderValue> {
        if list.iter().any(|(name, _)| name.as_str() == PRIORITY) {
            return None;
        }
        let (_, dest) = list
            .iter()
            .find(|(name, _)| name.as_str() == SEC_FETCH_DEST)?;
        let measured = self
            .profile
            .header_spec()
            .priority_for(dest.to_str().ok()?)?;
        HeaderValue::from_str(measured).ok()
    }

    /// Remembers whether `origin` answered over HTTP/1.1.
    fn remember(&self, origin: String, version: Version) {
        let mut origins = locked(&self.http1_origins);
        if version == Version::HTTP_11 || version == Version::HTTP_10 {
            if origins.len() >= MAX_REMEMBERED_ORIGINS && !origins.contains(&origin) {
                origins.clear();
            }
            origins.insert(origin);
        } else {
            origins.remove(&origin);
        }
    }
}

/// The impersonating client for a proxy.
fn client(
    profile: &ImpersonationProfile,
    egress: Option<&EgressRef>,
    options: &MitmOptions,
) -> Result<wreq::Client, MitmError> {
    finish(impersonation_client(profile)?, egress, options)
}

/// The measured WebSocket hello of the profile's browser and system, without the known gaps,
/// when one is measured for the profile's major version.
pub(super) fn websocket_spec(profile: &ImpersonationProfile) -> Option<TlsSpec> {
    let entry = emulation::get(profile.kind(), profile.os()).ok()?;
    if entry.major != profile.major() {
        return None;
    }
    let spec = entry.tls_websocket.as_ref()?;
    Some(spec.without_known_gaps(&entry.provenance.known_gaps))
}

/// The client of WebSocket handshakes: the measured WebSocket hello, HTTP/1.1 only, and the
/// same proxy settings as [`client`].
fn websocket_client(
    spec: &TlsSpec,
    egress: Option<&EgressRef>,
    options: &MitmOptions,
) -> Result<wreq::Client, MitmError> {
    let tls = convert::tls_options(spec).map_err(ImpersonateError::from)?;
    let builder = wreq::Client::builder()
        .tls_options(tls)
        .tls_sni(spec.has_extension(ext_id::SERVER_NAME))
        .http1_only();
    finish(builder, egress, options)
}

/// Adds what a proxy needs to a started client and builds it: no redirects, no decompression,
/// bounded phases, the egress or none, the upstream roots, and the destination filter.
fn finish(
    builder: wreq::ClientBuilder,
    egress: Option<&EgressRef>,
    options: &MitmOptions,
) -> Result<wreq::Client, MitmError> {
    let mut builder = builder
        .redirect(wreq::redirect::Policy::none())
        .referer(false)
        .gzip(false)
        .brotli(false)
        .deflate(false)
        .zstd(false)
        .connect_timeout(options.connect_timeout)
        .read_timeout(options.read_timeout)
        .pool_idle_timeout(options.idle_timeout);
    builder = match egress {
        Some(egress) => builder.proxy(egress_proxy(egress)?),
        None => builder.no_proxy(),
    };
    // Without an egress this proxy is the one that resolves names and connects, so it is the
    // one that must keep a page away from the machine and its network. Through an egress the
    // names are resolved on the other side.
    let guarded = egress.is_none() && !options.allow_private_destinations;
    if !options.upstream_roots.is_empty() {
        let roots = options.upstream_roots.iter().map(Vec::as_slice);
        let store = CertStore::from_der_certs(roots).map_err(|_error| {
            MitmError::InvalidOptions("an upstream root is not a DER certificate")
        })?;
        builder = builder.tls_cert_store(store);
    }
    if guarded {
        // The fixed answers go through the resolver too, so they get the same check.
        builder = builder.dns_resolver(PublicOnly::system(&options.resolve));
    } else {
        for (host, addr) in &options.resolve {
            builder = builder.resolve(host.clone(), *addr);
        }
    }
    builder.build().map_err(|_error| {
        MitmError::Upstream(ImpersonateError::Build(
            "the client refused the proxy settings".to_owned(),
        ))
    })
}

/// The scheme the client must use for an egress. A `socks5` proxy resolves names the way a
/// browser does, at the proxy; the client's own `socks5` resolves them on this machine, which
/// tells this machine's resolver every site visited and can answer differently from the proxy,
/// so it gets `socks5h`.
fn proxy_scheme(scheme: &str) -> &str {
    if scheme == "socks5" {
        "socks5h"
    } else {
        scheme
    }
}

/// The `Proxy-Authorization` value of a user name and password, marked sensitive.
fn basic_auth(user: &str, password: &str) -> Result<HeaderValue, MitmError> {
    let token = STANDARD.encode(format!("{user}:{password}"));
    let mut value = HeaderValue::from_str(&format!("Basic {token}"))
        .map_err(|_error| MitmError::Egress("the egress credentials were refused"))?;
    value.set_sensitive(true);
    Ok(value)
}

/// The egress as a client proxy, credentials attached; they never leave this function.
///
/// The credentials of an HTTP or HTTPS proxy travel in a sensitive `Proxy-Authorization` header
/// and never in the proxy address, which the client prints in its trace logs. A SOCKS proxy
/// takes them in the address, because its handshake has no header; that address is never logged
/// by this crate.
pub(super) fn egress_proxy(egress: &EgressRef) -> Result<wreq::Proxy, MitmError> {
    let key = egress.key();
    let host = if key.host.contains(':') {
        format!("[{}]", key.host)
    } else {
        key.host.clone()
    };
    let address = format!("{}://{host}:{}", proxy_scheme(&key.scheme), key.port);
    let target = wreq::Proxy::all(address.as_str())
        .map_err(|_error| MitmError::Egress("the egress address is not a proxy URL"))?;
    Ok(match egress.proxy().credentials() {
        Some((user, password)) if key.scheme.starts_with("socks") => {
            target.basic_auth(user, password)
        }
        Some((user, password)) => target.custom_http_auth(basic_auth(user, password)?),
        None => target,
    })
}

/// Chrome's spelling of a WebSocket handshake field, when `name` is one.
fn websocket_spelling(name: &HeaderName) -> Option<&'static str> {
    WEBSOCKET_SPELLING
        .iter()
        .find(|spelled| spelled.eq_ignore_ascii_case(name.as_str()))
        .copied()
}

/// The measured spelling of every header name the profile's lists hold.
fn spelling_of(profile: &ImpersonationProfile) -> HashMap<HeaderName, String> {
    let spec = profile.header_spec();
    [RequestShape::Document, RequestShape::Fetch]
        .into_iter()
        .flat_map(|shape| spec.headers(shape))
        .filter_map(|(name, _)| {
            HeaderName::from_bytes(name.as_bytes())
                .ok()
                .map(|key| (key, name.clone()))
        })
        .collect()
}

/// What failed, in words.
fn failure_kind(error: &wreq::Error) -> &'static str {
    if destination::is_internal_destination(error) {
        "the destination is a local, private or reserved address"
    } else if error.is_timeout() {
        "the upstream timed out"
    } else if error.is_proxy_connect() {
        "the egress proxy refused the tunnel"
    } else if error.is_tls() {
        "TLS with the upstream failed (certificate verification included)"
    } else if error.is_dns() {
        "the upstream host does not resolve"
    } else if error.is_connect() {
        "the upstream connection failed"
    } else if error.is_body() {
        "a body failed or grew past the limit"
    } else {
        "the upstream request failed"
    }
}

/// The current upstream, behind a lock that is only held to clone or replace the `Arc`.
pub(super) struct UpstreamSlot {
    /// The upstream new requests take.
    current: RwLock<Arc<Upstream>>,
    /// Serialises swaps, so two at once cannot lose one.
    swapping: Mutex<()>,
}

impl UpstreamSlot {
    /// A slot holding `upstream`.
    pub(super) fn new(upstream: Upstream) -> Self {
        Self {
            current: RwLock::new(Arc::new(upstream)),
            swapping: Mutex::new(()),
        }
    }

    /// The upstream new requests take.
    pub(super) fn current(&self) -> Arc<Upstream> {
        Arc::clone(&self.current.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Puts `upstream` in place; requests that hold the old one finish on it.
    fn replace(&self, upstream: Upstream) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(upstream);
    }

    /// Builds an upstream for `egress` with the current profile and puts it in place. Returns
    /// the egress keys before and after.
    pub(super) fn swap_egress(
        &self,
        egress: Option<EgressRef>,
        options: &MitmOptions,
    ) -> Result<(Option<EgressKey>, Option<EgressKey>), MitmError> {
        let _swapping = locked(&self.swapping);
        let old = self.current();
        let next = Upstream::build(old.profile.clone(), egress, options)?;
        let to = next.egress_key();
        self.replace(next);
        Ok((old.egress_key(), to))
    }

    /// Builds an upstream for `profile` with the current egress and puts it in place.
    pub(super) fn swap_profile(
        &self,
        profile: ImpersonationProfile,
        options: &MitmOptions,
    ) -> Result<(), MitmError> {
        let _swapping = locked(&self.swapping);
        let old = self.current();
        let next = Upstream::build(profile, old.egress.clone(), options)?;
        self.replace(next);
        Ok(())
    }
}
