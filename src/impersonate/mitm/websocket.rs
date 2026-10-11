//! WebSocket through the proxy: the RFC 6455 opening handshake inside a `CONNECT` tunnel, sent
//! upstream over HTTP/1.1 with the measured WebSocket `ClientHello`, then the bytes of both
//! directions copied as they are.
//!
//! The proxy reads no frame. It checks the handshake ([`ask`]), writes the `Connection: Upgrade`
//! and `Upgrade: websocket` pair itself ([`handshake_headers`]), relays the upstream's
//! `101 Switching Protocols` ([`switching_protocols`]; checking `Sec-WebSocket-Accept` stays with
//! the browser, as it would without a proxy), and then copies bytes ([`pump`]) until either side
//! closes, the connection has carried nothing for [`MitmOptions::websocket_idle_timeout`], one
//! direction has carried more than [`MitmOptions::max_websocket_bytes`], or the proxy stops. Both
//! sides are then shut down.
//!
//! The copy runs in the task of the browser connection, after hyper has written the `101` and
//! handed the connection over, so it belongs to the accept loop's `JoinSet` like everything else.

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use http::header::{CONNECTION, CONTENT_LENGTH, TRANSFER_ENCODING, UPGRADE};
use http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode, Version};
use hyper::body::Incoming;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt as _, ReadBuf};
use tokio::sync::watch;
use tokio::time::Instant;

use super::connect::{refuse, stopped};
use super::forward::{
    answered, dropped_fields, empty, forwarded_response_headers, keeps_request_header, ProxyBody,
};
use super::upstream::Handshake;
use super::{Counters, MitmError, MitmOptions, Shared};
use crate::common::shutdown::cleanup_within;
use crate::sb_cdp::sync::locked;
use crate::scrape::egress::redact::RedactedUrl;

/// The one protocol an upgrade may name.
const WEBSOCKET: &str = "websocket";
/// The `Connection` token that marks an upgrade.
const UPGRADE_TOKEN: &str = "upgrade";
/// The `Connection` value the proxy writes on both legs of a handshake.
const CONNECTION_UPGRADE: &str = "Upgrade";
/// How long the two sides get to close once the relay has ended.
const CLOSE_GRACE: Duration = Duration::from_secs(5);

/// What a request asks of its connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    /// An ordinary request: no upgrade.
    Plain,
    /// The opening handshake of a WebSocket (RFC 6455, section 4.1).
    WebSocket,
}

/// A refusal: the empty status the browser gets, and the error the proxy logs.
pub(super) type Refusal = (StatusCode, MitmError);

/// Reads what a request asks for.
///
/// No upgrade asked: [`Ask::Plain`]. `Upgrade: websocket` with `Connection: upgrade` on a bodiless
/// HTTP/1.1 `GET`: [`Ask::WebSocket`]. An upgrade to anything else (another protocol, a list, two
/// `Upgrade` fields) is not intercepted (`502`); half of an upgrade, or a handshake that is not a
/// bodiless HTTP/1.1 `GET`, is malformed (`400`).
pub(super) fn ask(method: &Method, version: Version, headers: &HeaderMap) -> Result<Ask, Refusal> {
    let asks_upgrade = connection_lists_upgrade(headers);
    match upgrade_protocol(headers) {
        Protocol::Absent if asks_upgrade => {
            return Err(malformed("Connection: upgrade names no protocol"));
        }
        Protocol::Absent => return Ok(Ask::Plain),
        Protocol::Other => {
            return Err((
                StatusCode::BAD_GATEWAY,
                MitmError::NotIntercepted("only WebSocket upgrades are relayed"),
            ));
        }
        Protocol::WebSocket => {}
    }
    if !asks_upgrade {
        return Err(malformed("Upgrade without Connection: upgrade"));
    }
    if *method != Method::GET {
        return Err(malformed("a WebSocket handshake is a GET"));
    }
    if version != Version::HTTP_11 {
        return Err(malformed("a WebSocket handshake is HTTP/1.1"));
    }
    if announces_body(headers) {
        return Err(malformed("a WebSocket handshake has no body"));
    }
    Ok(Ask::WebSocket)
}

/// What the `Upgrade` fields of a message name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    /// No `Upgrade` field.
    Absent,
    /// Exactly one field, naming `websocket`.
    WebSocket,
    /// Anything else: another protocol, a list, an empty value, two fields.
    Other,
}

/// Reads the `Upgrade` fields of a message.
fn upgrade_protocol(headers: &HeaderMap) -> Protocol {
    let mut values = headers.get_all(UPGRADE).iter();
    let Some(first) = values.next() else {
        return Protocol::Absent;
    };
    let websocket = values.next().is_none()
        && first
            .as_bytes()
            .trim_ascii()
            .eq_ignore_ascii_case(WEBSOCKET.as_bytes());
    if websocket {
        Protocol::WebSocket
    } else {
        Protocol::Other
    }
}

/// Whether a `Connection` field of a message lists `upgrade`.
fn connection_lists_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case(UPGRADE_TOKEN))
}

/// The refusal of a malformed upgrade: an empty `400`.
fn malformed(reason: &'static str) -> Refusal {
    (StatusCode::BAD_REQUEST, MitmError::MalformedUpgrade(reason))
}

/// The handshake headers that go upstream, in the browser's order: what an ordinary request
/// keeps, with the `Connection: Upgrade` and `Upgrade: websocket` pair written by the proxy where
/// the browser had its `Connection` and `Upgrade` fields.
pub(super) fn handshake_headers(headers: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let listed = dropped_fields(headers);
    let mut kept = Vec::with_capacity(headers.len());
    let (mut connection, mut upgrade) = (false, false);
    for (name, value) in headers {
        if *name == CONNECTION {
            if !connection {
                connection = true;
                kept.push((CONNECTION, HeaderValue::from_static(CONNECTION_UPGRADE)));
            }
        } else if *name == UPGRADE {
            if !upgrade {
                upgrade = true;
                kept.push((UPGRADE, HeaderValue::from_static(WEBSOCKET)));
            }
        } else if keeps_request_header(name, value, &listed) {
            kept.push((name.clone(), value.clone()));
        }
    }
    kept
}

/// Whether the headers of an upstream `101` switch to WebSocket: exactly one `Upgrade` naming
/// `websocket`, and `Connection: upgrade`.
pub(super) fn switched_to_websocket(headers: &HeaderMap) -> bool {
    upgrade_protocol(headers) == Protocol::WebSocket && connection_lists_upgrade(headers)
}

/// The `101` the browser gets: the upstream's headers without the hop-by-hop fields and any
/// length, plus the `Connection: Upgrade` and `Upgrade: websocket` pair.
pub(super) fn switching_protocols(upstream: &HeaderMap) -> Response<ProxyBody> {
    let mut headers = forwarded_response_headers(upstream);
    headers.remove(CONTENT_LENGTH);
    headers.insert(CONNECTION, HeaderValue::from_static(CONNECTION_UPGRADE));
    headers.insert(UPGRADE, HeaderValue::from_static(WEBSOCKET));
    let mut response = empty(StatusCode::SWITCHING_PROTOCOLS);
    *response.headers_mut() = headers;
    response
}

/// A switched connection the relay copies bytes over.
pub(super) trait RelayIo: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> RelayIo for T {}

/// A handshake the upstream accepted, waiting for hyper to hand over the browser's connection.
pub(super) struct PendingRelay {
    /// Resolves to the browser's side once the `101` is written.
    browser: OnUpgrade,
    /// The upstream side, already switched.
    upstream: Box<dyn RelayIo>,
    /// The URL, redacted, for the log.
    url: RedactedUrl,
}

/// The relay the handshake of a tunnel opened, if any.
pub(super) type RelaySlot = Mutex<Option<PendingRelay>>;

/// Serves a WebSocket handshake that passed [`ask`] and the ordinary checks: sends it upstream
/// and, when the upstream switches, leaves the relay in `slot` and answers the browser `101`.
/// Any other upstream answer goes to the browser as an ordinary answer; a failure is an empty
/// `502`.
pub(super) async fn open(
    shared: &Shared,
    url: String,
    redacted: &RedactedUrl,
    mut request: Request<Incoming>,
    slot: &RelaySlot,
) -> Response<ProxyBody> {
    let upstream = shared.upstream.current();
    if !upstream.speaks_websocket() {
        let error =
            MitmError::NotIntercepted("no WebSocket ClientHello is measured for this profile");
        return refuse(shared, StatusCode::BAD_GATEWAY, &error, true);
    }
    let browser = hyper::upgrade::on(&mut request);
    let headers = handshake_headers(request.headers());
    drop(request);
    let sent = upstream
        .open_websocket(url, headers, shared.options.max_body_bytes)
        .await;
    match sent {
        Ok(Handshake::Switched { headers, stream }) if switched_to_websocket(&headers) => {
            Counters::bump(&shared.counters.forwarded);
            Counters::bump(&shared.counters.websockets);
            *locked(slot) = Some(PendingRelay {
                browser,
                upstream: stream,
                url: redacted.clone(),
            });
            switching_protocols(&headers)
        }
        Ok(Handshake::Switched { .. }) => answered(
            shared,
            redacted,
            Err("the upstream switched to another protocol than WebSocket"),
        ),
        Ok(Handshake::Answered(response)) => answered(shared, redacted, Ok(response)),
        Err(kind) => answered(shared, redacted, Err(kind)),
    }
}

/// Copies the bytes of a switched handshake once hyper hands over the browser's side, within
/// the bounds of `options`, until it ends or `stop` says so.
pub(super) async fn run(
    pending: PendingRelay,
    options: &MitmOptions,
    stop: &mut watch::Receiver<bool>,
) {
    let PendingRelay {
        browser,
        upstream,
        url,
    } = pending;
    let browser = match browser.await {
        Ok(upgraded) => TokioIo::new(upgraded),
        Err(error) => {
            tracing::debug!(%error, %url, "the browser side of a WebSocket was not handed over");
            return;
        }
    };
    let bounds = Bounds {
        idle: options.websocket_idle_timeout,
        max_bytes: options.max_websocket_bytes,
    };
    let relayed = pump(browser, upstream, bounds, stop).await;
    tracing::debug!(
        %url,
        ended = ?relayed.ended,
        to_upstream = relayed.to_upstream,
        to_browser = relayed.to_browser,
        "WebSocket relay ended"
    );
}

/// The limits of one relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Bounds {
    /// How long the connection may carry no byte in either direction.
    pub(super) idle: Duration,
    /// The most bytes each direction carries.
    pub(super) max_bytes: u64,
}

/// Why a relay ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ended {
    /// Both sides closed.
    Closed,
    /// No byte passed for the idle limit.
    Idle,
    /// One direction carried more than the byte limit.
    TooLarge,
    /// The proxy stopped.
    Stopped,
    /// A side failed.
    Failed,
}

/// What a relay did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Relayed {
    /// Why it ended.
    pub(super) ended: Ended,
    /// Bytes read from the browser.
    pub(super) to_upstream: u64,
    /// Bytes read from the upstream.
    pub(super) to_browser: u64,
}

/// Copies bytes between `browser` and `upstream` until one of the ends of [`Ended`], then shuts
/// both down (each within a short grace).
pub(super) async fn pump<A, B>(
    browser: A,
    upstream: B,
    bounds: Bounds,
    stop: &mut watch::Receiver<bool>,
) -> Relayed
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let activity = Activity::new();
    let mut browser = Metered::new(browser, bounds.max_bytes, &activity);
    let mut upstream = Metered::new(upstream, bounds.max_bytes, &activity);
    let ended = tokio::select! {
        copied = tokio::io::copy_bidirectional(&mut browser, &mut upstream) => {
            if copied.is_ok() { Ended::Closed } else { Ended::Failed }
        }
        () = activity.idle_for(bounds.idle) => Ended::Idle,
        () = stopped(stop) => Ended::Stopped,
    };
    let ended = if browser.over || upstream.over {
        Ended::TooLarge
    } else {
        ended
    };
    let _closed = cleanup_within(CLOSE_GRACE, async {
        let (_browser, _upstream) = tokio::join!(browser.shutdown(), upstream.shutdown());
    })
    .await;
    Relayed {
        ended,
        to_upstream: browser.passed,
        to_browser: upstream.passed,
    }
}

/// When the relay last moved a byte, as milliseconds since it started.
#[derive(Debug)]
struct Activity {
    /// When the relay started.
    started: Instant,
    /// Milliseconds from `started` to the last byte.
    last: AtomicU64,
}

impl Activity {
    /// A clock that starts now.
    fn new() -> Self {
        Self {
            started: Instant::now(),
            last: AtomicU64::new(0),
        }
    }

    /// Records that a byte moved now.
    fn touch(&self) {
        let elapsed = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last.store(elapsed, Ordering::Relaxed);
    }

    /// Resolves once no byte has moved for `idle`.
    async fn idle_for(&self, idle: Duration) {
        loop {
            let last = Duration::from_millis(self.last.load(Ordering::Relaxed));
            let Some(deadline) = self
                .started
                .checked_add(last)
                .and_then(|moved| moved.checked_add(idle))
            else {
                return std::future::pending().await;
            };
            if Instant::now() >= deadline {
                return;
            }
            tokio::time::sleep_until(deadline).await;
        }
    }
}

/// One side of a relay: counts the bytes read from it, fails a read that takes it past the
/// limit, and records activity.
struct Metered<'a, S> {
    /// The side.
    inner: S,
    /// The most bytes that may be read from it.
    limit: u64,
    /// The bytes read so far.
    passed: u64,
    /// Whether a read went past the limit.
    over: bool,
    /// The relay's clock.
    activity: &'a Activity,
}

impl<'a, S> Metered<'a, S> {
    /// Meters `inner`.
    fn new(inner: S, limit: u64, activity: &'a Activity) -> Self {
        Self {
            inner,
            limit,
            passed: 0,
            over: false,
            activity,
        }
    }
}

/// The error of a read past the byte limit.
fn too_large(limit: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("a WebSocket direction carried more than {limit} bytes"),
    )
}

impl<S: AsyncRead + Unpin> AsyncRead for Metered<'_, S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.over {
            return Poll::Ready(Err(too_large(this.limit)));
        }
        let before = buf.filled().len();
        ready!(Pin::new(&mut this.inner).poll_read(cx, buf))?;
        let read = u64::try_from(buf.filled().len().saturating_sub(before)).unwrap_or(u64::MAX);
        if read > 0 {
            this.activity.touch();
            this.passed = this.passed.saturating_add(read);
            if this.passed > this.limit {
                this.over = true;
                return Poll::Ready(Err(too_large(this.limit)));
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Metered<'_, S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let written = ready!(Pin::new(&mut this.inner).poll_write(cx, buf))?;
        if written > 0 {
            this.activity.touch();
        }
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// Whether a message announces a body: any `Transfer-Encoding`, or a `Content-Length` that is
/// not `0`.
fn announces_body(headers: &HeaderMap) -> bool {
    headers.contains_key(TRANSFER_ENCODING)
        || headers
            .get_all(CONTENT_LENGTH)
            .iter()
            .any(|value| value.as_bytes().trim_ascii() != b"0")
}

#[cfg(test)]
mod tests;
