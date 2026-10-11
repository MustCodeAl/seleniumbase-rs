//! The upstream client behind the proxy, replaced whole when the egress or the profile changes.
//!
//! It is the impersonating client of `impersonate::client` with what a proxy needs on top: no
//! redirects (the browser follows them), no decompression (`Content-Encoding` passes through),
//! no default headers (the browser's own go out), verification always on, and bounded connect
//! and read phases. Of the proxy's files, only this one names wreq types.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use http::{HeaderMap, HeaderName, HeaderValue, Method, Response, Version};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use wreq::header::OrigHeaderMap;
use wreq::tls::trust::CertStore;

use super::body::Capped;
use super::destination::{self, PublicOnly};
use super::forward::ProxyBody;
use super::{MitmError, MitmOptions};
use crate::impersonate::client::{impersonation_client, ImpersonateError, ImpersonationProfile};
use crate::sb_cdp::sync::locked;
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::wire::header_spec::RequestShape;

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
        let spelling = spelling_of(&profile);
        Ok(Self {
            client,
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

    /// Sends one request and returns the answer with its body capped at `max_body`. The error
    /// names what failed in words; the client's own error text, which can hold the URL, is
    /// never kept.
    pub(super) async fn send(
        &self,
        outbound: Outbound,
        max_body: usize,
    ) -> Result<Response<ProxyBody>, &'static str> {
        let http1 = locked(&self.http1_origins).contains(&outbound.origin);
        let (headers, order) = self.wire_headers(outbound.headers, http1);
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
    /// `Host` first on HTTP/1.1 (the client fills it in), and on HTTP/2 the measured `priority`
    /// for the request's destination at the end, as Chrome sends it.
    fn wire_headers(
        &self,
        list: Vec<(HeaderName, HeaderValue)>,
        http1: bool,
    ) -> (HeaderMap, OrigHeaderMap) {
        let mut map = HeaderMap::with_capacity(list.len() + 1);
        let mut order = OrigHeaderMap::with_capacity(list.len() + 2);
        if http1 {
            order.insert(HOST_SPELLING);
        }
        let priority = if http1 {
            None
        } else {
            self.priority_for(&list)
        };
        for (name, mut value) in list {
            if SENSITIVE.contains(&name.as_str()) {
                value.set_sensitive(true);
            }
            let spelled = self
                .spelling
                .get(&name)
                .cloned()
                .unwrap_or_else(|| name.as_str().to_owned());
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
    let mut builder = impersonation_client(profile)?
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

/// The egress as a client proxy, credentials attached; they never leave this function.
fn egress_proxy(egress: &EgressRef) -> Result<wreq::Proxy, MitmError> {
    let proxy = egress.proxy();
    let target = wreq::Proxy::all(proxy.server())
        .map_err(|_error| MitmError::Egress("the egress address is not a proxy URL"))?;
    Ok(match proxy.credentials() {
        Some((user, password)) => target.basic_auth(user, password),
        None => target,
    })
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
