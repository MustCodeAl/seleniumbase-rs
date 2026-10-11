//! The hop loop: sends a request, stores its cookies, follows redirects by hand, and reads the
//! body within the limits.
//!
//! One request runs under one deadline that covers every hop and the body. Each hop waits for its
//! site's pace, sends the headers of the profile for the protocol the origin speaks with the
//! `Cookie` header the jar gives for that hop's URL, stores the `Set-Cookie` headers of the
//! answer, and either follows the redirect (see [`redirect`](super::redirect)) or reads the body.
//! The text of an error from the HTTP client is dropped: it can name the URL or the proxy.

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::{timeout_at, Instant};
use url::Url;
use wreq::header::SET_COOKIE;
use wreq::Method;

use super::error::HttpError;
use super::options::MAX_TIMEOUT;
use super::redirect::{site_name, target_of, Hop};
use super::reply::{Headers, HttpReply, ReplyProtocol};
use super::request::{merge_headers, Prepared};
use super::{HttpSession, MAX_REMEMBERED_ORIGINS};
use crate::impersonate::client::{ImpersonateError, WireProtocol};
use crate::impersonate::convert;
use crate::scrape::challenge::mitigation::Mode;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::ledger::events::ScraperEvent;
use crate::wire::header_spec::RequestShape;

/// The largest buffer reserved for a body before its bytes arrive, whatever the server claims.
const MAX_PRESIZE_BYTES: usize = 1024 * 1024;
/// How much of the body of a redirect is read so that its connection can be used again.
const MAX_REDIRECT_DRAIN_BYTES: usize = 64 * 1024;
/// The header the transport writes first on HTTP/1.1.
const HOST_HEADER: &str = "Host";
/// What a failure to meet the deadline says.
const TIMED_OUT: &str = "timed out";

/// What a hop got back, before its body is read.
struct Answer {
    status: u16,
    headers: Headers,
    protocol: ReplyProtocol,
    response: wreq::Response,
}

/// Runs a prepared request to its reply.
///
/// # Errors
///
/// See [`HttpRequest::send`](super::HttpRequest::send).
pub(super) async fn run(session: &HttpSession, prepared: Prepared) -> Result<HttpReply, HttpError> {
    let limits = *session.limits();
    let started = Instant::now();
    let budget = prepared.timeout.unwrap_or(limits.timeout).min(MAX_TIMEOUT);
    let deadline = started + budget;
    let shape = prepared.shape;
    let mut hop = Hop::first(prepared);
    let mut chain: Vec<RedactedUrl> = Vec::new();
    loop {
        let answer = exchange(session, &hop, shape, deadline).await?;
        let target = target_of(
            answer.status,
            &answer.headers,
            &hop.url,
            limits.follow_redirects,
        )?;
        let Some(next) = target else {
            let body = read_body(&hop, answer.response, limits.max_body_bytes, deadline).await?;
            let reply = HttpReply {
                status: answer.status,
                url: hop.url,
                headers: answer.headers,
                body,
                protocol: answer.protocol,
                redirects: chain,
                elapsed: started.elapsed(),
            };
            announce(session, &reply);
            return Ok(reply);
        };
        if chain.len() >= limits.max_redirects {
            return Err(HttpError::TooManyRedirects(limits.max_redirects));
        }
        drain(answer.response, deadline).await;
        chain.push(RedactedUrl::new(next.as_str()));
        hop = hop.follow(answer.status, next);
    }
}

/// Emits the event of a finished request.
fn announce(session: &HttpSession, reply: &HttpReply) {
    let elapsed_ms = u64::try_from(reply.elapsed.as_millis()).unwrap_or(u64::MAX);
    session.emit(&ScraperEvent::Request {
        url: reply.redacted_url(),
        status: reply.status,
        mode: Mode::Http,
        elapsed_ms,
    });
}

/// Sends one hop and stores the cookies of its answer.
async fn exchange(
    session: &HttpSession,
    hop: &Hop,
    shape: RequestShape,
    deadline: Instant,
) -> Result<Answer, HttpError> {
    let inner = session.inner();
    if let Some(pacer) = &inner.pacer {
        pacer.acquire(&site_name(&hop.url)).await;
    }
    let context = super::cookies::context_for(shape, &hop.method, hop.crossed_site);
    let cookie = inner.jar.header_for(&hop.url, context, session.now());
    let wire = wire_for(session, &hop.url);
    let request = build_request(session, hop, shape, cookie.as_deref(), wire)?;
    let response = timeout_at(deadline, request.send())
        .await
        .map_err(|_elapsed| failure(&hop.url, TIMED_OUT))?
        .map_err(|error| failure(&hop.url, classify(&error)))?;
    let protocol = if response.version() == wreq::Version::HTTP_2 {
        ReplyProtocol::Http2
    } else {
        ReplyProtocol::Http1
    };
    remember(session, &hop.url, protocol);
    let headers = headers_of(response.headers());
    store_cookies(session, &hop.url, &headers);
    Ok(Answer {
        status: response.status().as_u16(),
        headers,
        protocol,
        response,
    })
}

/// The headers of a response with lower-case names; a value that is not UTF-8 is read with the
/// broken bytes replaced.
fn headers_of(map: &wreq::header::HeaderMap) -> Headers {
    Headers::new(
        map.iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
    )
}

/// Stores the `Set-Cookie` headers of an answer. A header that is not UTF-8 is ignored, as a
/// browser ignores it.
fn store_cookies(session: &HttpSession, url: &Url, headers: &Headers) {
    let lines: Vec<&str> = headers.get_all(SET_COOKIE.as_str()).collect();
    if lines.is_empty() {
        return;
    }
    let absorbed = session.inner().jar.absorb(url, &lines, session.now());
    tracing::debug!(
        stored = absorbed.stored,
        deleted = absorbed.deleted,
        ignored = absorbed.ignored,
        "set-cookie headers read"
    );
}

/// The protocol to write headers for: the one the origin settled on, HTTP/2 for an origin not met
/// yet (nearly every site speaks it), and HTTP/1.1 for a plain `http` URL.
fn wire_for(session: &HttpSession, url: &Url) -> WireProtocol {
    if url.scheme() == "http" {
        return WireProtocol::Http1;
    }
    let table = crate::sb_cdp::sync::locked(&session.inner().protocols);
    table
        .get(&origin_key(url))
        .copied()
        .unwrap_or(WireProtocol::Http2)
}

/// Remembers the protocol an origin answered on.
fn remember(session: &HttpSession, url: &Url, protocol: ReplyProtocol) {
    let wire = match protocol {
        ReplyProtocol::Http2 => WireProtocol::Http2,
        ReplyProtocol::Http1 => WireProtocol::Http1,
    };
    let mut table = crate::sb_cdp::sync::locked(&session.inner().protocols);
    if table.len() >= MAX_REMEMBERED_ORIGINS {
        table.clear();
    }
    table.insert(origin_key(url), wire);
}

fn origin_key(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// The wreq request for one hop: the profile's headers for `wire` with the cookie and the
/// caller's headers in, in the profile's order.
fn build_request(
    session: &HttpSession,
    hop: &Hop,
    shape: RequestShape,
    cookie: Option<&str>,
    wire: WireProtocol,
) -> Result<wreq::RequestBuilder, HttpError> {
    let inner = session.inner();
    let mut list = inner.profile.headers_for(shape, cookie, wire)?;
    merge_headers(&mut list, &hop.headers);
    let leading: &[&'static str] = match wire {
        WireProtocol::Http1 => &[HOST_HEADER],
        WireProtocol::Http2 => &[],
    };
    let (headers, order) = convert::wreq_headers(&list, leading).map_err(ImpersonateError::from)?;
    let mut request = inner
        .client
        .request(hop.method.clone(), hop.url.as_str())
        .default_headers(false)
        .headers(headers)
        .orig_headers(order);
    if let Some(body) = &hop.body {
        request = request.body(body.clone());
    }
    Ok(request)
}

/// Reads the short body of a redirect, so that its connection goes back to the pool. A long or
/// slow body is dropped, which closes the connection.
async fn drain(response: wreq::Response, deadline: Instant) {
    let mut stream = std::pin::pin!(response.bytes_stream());
    let mut seen = 0_usize;
    while let Ok(Some(Ok(chunk))) = timeout_at(deadline, stream.next()).await {
        seen = seen.saturating_add(chunk.len());
        if seen > MAX_REDIRECT_DRAIN_BYTES {
            break;
        }
    }
}

/// Reads the body of the last hop within `limit` bytes, counted after decompression.
async fn read_body(
    hop: &Hop,
    response: wreq::Response,
    limit: usize,
    deadline: Instant,
) -> Result<Bytes, HttpError> {
    let promised = response.content_length();
    if hop.method != Method::HEAD && promised.is_some_and(|bytes| bytes > limit as u64) {
        return Err(HttpError::BodyTooLarge(limit));
    }
    let presize = promised
        .and_then(|bytes| usize::try_from(bytes).ok())
        .unwrap_or(0)
        .min(limit)
        .min(MAX_PRESIZE_BYTES);
    let mut body = BytesMut::with_capacity(presize);
    let mut stream = std::pin::pin!(response.bytes_stream());
    while let Some(chunk) = timeout_at(deadline, stream.next())
        .await
        .map_err(|_elapsed| failure(&hop.url, TIMED_OUT))?
    {
        let chunk = chunk.map_err(|error| failure(&hop.url, classify(&error)))?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(HttpError::BodyTooLarge(limit));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

/// A failed request, in the words of [`HttpError::Request`].
fn failure(url: &Url, kind: &'static str) -> HttpError {
    HttpError::Request {
        url: RedactedUrl::new(url.as_str()),
        kind,
    }
}

/// What kind of failure an HTTP client error is, in a word or two. The error's own text can name
/// the URL and the proxy, so it is never kept.
fn classify(error: &wreq::Error) -> &'static str {
    if error.is_timeout() {
        TIMED_OUT
    } else if error.is_proxy_connect() {
        "the proxy refused or failed the connection"
    } else if error.is_dns() {
        "the host name could not be resolved"
    } else if error.is_tls() {
        "the TLS handshake or the certificate failed"
    } else if error.is_connect() {
        "the connection could not be made"
    } else if error.is_connection_reset() {
        "the connection was reset"
    } else if error.is_decode() {
        "the body could not be decoded"
    } else if error.is_body() {
        "the body could not be read"
    } else if error.is_builder() {
        "the request could not be built"
    } else {
        "the request failed"
    }
}
