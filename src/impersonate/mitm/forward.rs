//! One request through the proxy: the checks, the header rewrite, the upstream call and the
//! answer, with the body streamed both ways.
//!
//! The browser's header order is kept: the headers go upstream in the order the browser wrote
//! them, without the ones that describe the browser-to-proxy hop (RFC 9110 section 7.6.1) and
//! without `Host`, which the upstream client writes itself (`:authority` on HTTP/2).

use std::time::Instant;

use bytes::Bytes;
use http::header::{CONNECTION, CONTENT_LENGTH, HOST, TE, UPGRADE, USER_AGENT};
use http::uri::{Authority, PathAndQuery, Uri};
use http::{
    Extensions, HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, Version,
};
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt as _, Empty};
use hyper::body::{Body as _, Incoming};
use url::Host;

use super::body::Capped;
use super::connect::refuse;
use super::upstream::Outbound;
use super::{Counters, MitmError, Shared};
use crate::scrape::challenge::mitigation::Mode;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::ledger::events::ScraperEvent;

/// The error of a body in either direction.
pub(super) type BoxError = Box<dyn std::error::Error + Send + Sync>;
/// The body of every answer to the browser.
pub(super) type ProxyBody = UnsyncBoxBody<Bytes, BoxError>;

/// Header fields that describe one hop, plus the proxy's own; never forwarded either way.
const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authorization",
    "proxy-authenticate",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];
/// The one `TE` value a request may carry upstream.
const TE_TRAILERS: &str = "trailers";
/// What precedes the major version in a Chrome user agent.
const CHROME_TOKEN: &str = "Chrome/";
/// The path of a request that names none.
const ROOT_PATH: &str = "/";
/// The default port of `http`.
const HTTP_PORT: u16 = 80;
/// The default port of `https`.
const HTTPS_PORT: u16 = 443;

/// The scheme requests to an [`Origin`] use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scheme {
    /// Plain HTTP, for absolute-form requests.
    Http,
    /// HTTPS, for everything inside a `CONNECT` tunnel.
    Https,
}

impl Scheme {
    /// The scheme as written in a URL.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    /// The port a URL of this scheme leaves out.
    fn default_port(self) -> u16 {
        match self {
            Self::Http => HTTP_PORT,
            Self::Https => HTTPS_PORT,
        }
    }
}

/// Where the requests of one tunnel, or one absolute-form request, go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Origin {
    /// `https` for a tunnel, `http` for an absolute-form request.
    scheme: Scheme,
    /// The host as a URL writes it: lower case, IPv6 in brackets.
    host: String,
    /// The port.
    port: u16,
}

impl Origin {
    /// The origin of a `CONNECT` request (`host:port`, port required), with its host for the
    /// leaf certificate.
    pub(super) fn from_connect(uri: &Uri) -> Result<(Self, Host<String>), MitmError> {
        let authority = uri
            .authority()
            .filter(|authority| uri.scheme().is_none() && authority.port_u16().is_some())
            .ok_or(MitmError::BadTarget("CONNECT takes host:port"))?;
        Self::from_authority(Scheme::Https, authority)
    }

    /// The origin of an absolute-form `http://` request.
    pub(super) fn from_absolute(uri: &Uri) -> Result<Self, MitmError> {
        if uri.scheme_str() != Some(Scheme::Http.as_str()) {
            return Err(MitmError::BadTarget(
                "only http:// requests are taken without CONNECT",
            ));
        }
        let authority = uri
            .authority()
            .ok_or(MitmError::BadTarget("the request names no host"))?;
        Self::from_authority(Scheme::Http, authority).map(|(origin, _host)| origin)
    }

    /// Reads host and port; user information, port 0 and an unparseable host are refused.
    fn from_authority(
        scheme: Scheme,
        authority: &Authority,
    ) -> Result<(Self, Host<String>), MitmError> {
        let (host, port) = parse_authority(authority, scheme.default_port()).ok_or(
            MitmError::BadTarget("the target is not a valid host and port"),
        )?;
        let origin = Self {
            scheme,
            host: host_text(&host),
            port,
        };
        Ok((origin, host))
    }

    /// The URL of `path_and_query` at this origin; the default port is left out, as a browser
    /// leaves it out of `:authority` and `Host`.
    pub(super) fn url(&self, path_and_query: &str) -> String {
        let scheme = self.scheme.as_str();
        if self.port == self.scheme.default_port() {
            format!("{scheme}://{}{path_and_query}", self.host)
        } else {
            format!("{scheme}://{}:{}{path_and_query}", self.host, self.port)
        }
    }

    /// `scheme://host:port`, the key the upstream remembers protocols under.
    pub(super) fn key(&self) -> String {
        format!("{}://{}:{}", self.scheme.as_str(), self.host, self.port)
    }

    /// Whether a `Host` header names exactly this origin.
    pub(super) fn accepts_host_header(&self, value: &HeaderValue) -> bool {
        value
            .to_str()
            .ok()
            .and_then(|text| text.parse::<Authority>().ok())
            .and_then(|authority| parse_authority(&authority, self.scheme.default_port()))
            .is_some_and(|(host, port)| port == self.port && host_text(&host) == self.host)
    }
}

/// Host and port of an authority, `None` for user information, port 0 or a bad host.
fn parse_authority(authority: &Authority, default_port: u16) -> Option<(Host<String>, u16)> {
    if authority.as_str().contains('@') {
        return None;
    }
    let port = authority.port_u16().unwrap_or(default_port);
    if port == 0 {
        return None;
    }
    let host = Host::parse(authority.host()).ok()?;
    Some((host, port))
}

/// A host as a URL writes it.
fn host_text(host: &Host<String>) -> String {
    match host {
        Host::Domain(domain) => domain.clone(),
        Host::Ipv4(ip) => ip.to_string(),
        Host::Ipv6(ip) => format!("[{ip}]"),
    }
}

/// Serves one request: checks it, sends it upstream, answers with the upstream's answer, and
/// reports it to the event sink.
pub(super) async fn forward(
    shared: &Shared,
    origin: &Origin,
    request: Request<Incoming>,
) -> Response<ProxyBody> {
    let started = Instant::now();
    let path = request
        .uri()
        .path_and_query()
        .map_or(ROOT_PATH, PathAndQuery::as_str);
    let url = origin.url(path);
    let redacted = RedactedUrl::new(&url);
    let response = match check(shared, origin, request.headers()) {
        Err((status, error)) => refuse(shared, status, &error, true),
        Ok(()) => relay(shared, origin, url, &redacted, request).await,
    };
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    shared.sink.emit(&ScraperEvent::Request {
        url: redacted,
        status: response.status().as_u16(),
        mode: Mode::Browser,
        elapsed_ms,
    });
    response
}

/// The checks before a request may leave: no protocol upgrade, one `Host` naming the origin, an
/// announced body within the limit, and no user agent that contradicts the profile.
fn check(
    shared: &Shared,
    origin: &Origin,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, MitmError)> {
    if headers.contains_key(UPGRADE) {
        return Err((
            StatusCode::BAD_GATEWAY,
            MitmError::NotIntercepted("protocol upgrades, WebSocket included, are not forwarded"),
        ));
    }
    let mut hosts = headers.get_all(HOST).iter();
    let host_ok = hosts
        .next()
        .is_some_and(|value| origin.accepts_host_header(value))
        && hosts.next().is_none();
    if !host_ok {
        return Err((StatusCode::BAD_REQUEST, MitmError::HostMismatch));
    }
    let limit = shared.options.max_body_bytes;
    if exceeds(headers, limit) {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            MitmError::BodyTooLarge { limit },
        ));
    }
    let measured = shared.upstream.current().major();
    if let Some(browser) = claimed_chrome_major(headers).filter(|claimed| *claimed != measured) {
        return Err((
            StatusCode::BAD_GATEWAY,
            MitmError::VersionMismatch { browser, measured },
        ));
    }
    Ok(())
}

/// Sends the request upstream and turns the answer into one for the browser; an upstream
/// failure is an empty `502`.
async fn relay(
    shared: &Shared,
    origin: &Origin,
    url: String,
    redacted: &RedactedUrl,
    request: Request<Incoming>,
) -> Response<ProxyBody> {
    let limit = shared.options.max_body_bytes;
    let (parts, body) = request.into_parts();
    let outbound = Outbound {
        method: parts.method,
        url,
        origin: origin.key(),
        headers: forwarded_request_headers(&parts.headers),
        body: (!body.is_end_stream()).then(|| Capped::new(body, limit)),
    };
    let upstream = shared.upstream.current();
    match upstream.send(outbound, limit).await {
        Ok(response) if exceeds(response.headers(), limit) => {
            Counters::bump(&shared.counters.upstream_errors);
            let error = MitmError::BodyTooLarge { limit };
            tracing::debug!(%error, url = %redacted, "upstream answer refused");
            empty(StatusCode::BAD_GATEWAY)
        }
        Ok(response) => {
            Counters::bump(&shared.counters.forwarded);
            for_browser(response)
        }
        Err(kind) => {
            Counters::bump(&shared.counters.upstream_errors);
            let error = MitmError::UpstreamRequest {
                url: redacted.clone(),
                kind,
            };
            tracing::debug!(%error, "upstream request failed");
            empty(StatusCode::BAD_GATEWAY)
        }
    }
}

/// The upstream answer as the browser gets it: HTTP/1.1, no hop-by-hop headers.
fn for_browser(response: Response<ProxyBody>) -> Response<ProxyBody> {
    let (mut parts, body) = response.into_parts();
    parts.headers = forwarded_response_headers(&parts.headers);
    parts.version = Version::HTTP_11;
    parts.extensions = Extensions::new();
    Response::from_parts(parts, body)
}

/// An answer with `status` and no body.
pub(super) fn empty(status: StatusCode) -> Response<ProxyBody> {
    let body = Empty::<Bytes>::new()
        .map_err(|never| -> BoxError { match never {} })
        .boxed_unsync();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
}

/// The request headers that go upstream, in the browser's order: everything but `Host`, the
/// hop-by-hop and proxy fields, the fields `Connection` names, and a `TE` other than
/// `trailers`.
pub(super) fn forwarded_request_headers(headers: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let listed = connection_tokens(headers);
    headers
        .iter()
        .filter(|(name, value)| keeps_request_header(name, value, &listed))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Whether one request header goes upstream.
fn keeps_request_header(name: &HeaderName, value: &HeaderValue, listed: &[String]) -> bool {
    if *name == HOST || listed.iter().any(|token| token == name.as_str()) {
        return false;
    }
    if *name == TE {
        return value
            .as_bytes()
            .eq_ignore_ascii_case(TE_TRAILERS.as_bytes());
    }
    !HOP_BY_HOP.contains(&name.as_str())
}

/// The response headers that go to the browser: everything but the hop-by-hop and proxy
/// fields and the fields `Connection` names.
pub(super) fn forwarded_response_headers(headers: &HeaderMap) -> HeaderMap {
    let listed = connection_tokens(headers);
    let mut kept = HeaderMap::with_capacity(headers.len());
    for (name, value) in headers {
        let hop = HOP_BY_HOP.contains(&name.as_str())
            || listed.iter().any(|token| token == name.as_str());
        if !hop {
            kept.append(name.clone(), value.clone());
        }
    }
    kept
}

/// The field names a `Connection` header lists, lower case.
fn connection_tokens(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

/// Whether a message announces a body longer than `limit` bytes.
fn exceeds(headers: &HeaderMap, limit: usize) -> bool {
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    announced_length(headers).is_some_and(|length| length > limit)
}

/// The body length a message announces, when it announces one that parses.
fn announced_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The Chrome major version a request's `User-Agent` claims, when it claims one.
pub(super) fn claimed_chrome_major(headers: &HeaderMap) -> Option<u32> {
    let agent = headers.get(USER_AGENT)?.to_str().ok()?;
    let (_, rest) = agent.split_once(CHROME_TOKEN)?;
    let digits = rest
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .filter(|digits| !digits.is_empty())?;
    digits.parse().ok()
}
