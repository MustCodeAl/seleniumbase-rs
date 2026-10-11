//! The browser side: the loopback accept loop, the proxy token check, `CONNECT`, and the TLS
//! leg the proxy ends.
//!
//! Every connection task belongs to one [`JoinSet`] owned by the accept loop, so aborting the
//! loop aborts them all; nothing is spawned anywhere else. A WebSocket relay runs in the task of
//! its browser connection, after hyper has handed the connection over.

use std::convert::Infallible;
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use http::header::{CONNECTION, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION};
use http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::{TokioIo, TokioTimer};
use ring::digest::{digest, SHA256};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;

use super::forward::{self, empty, Origin, ProxyBody};
use super::websocket::{self, RelaySlot};
use super::{Counters, MitmError, MitmOptions, Shared};
use crate::impersonate::tls_server::{self, Leaf, TlsServerError};
use crate::sb_cdp::sync::locked;
use crate::scrape::egress::redact::Secret;

/// The `Proxy-Authenticate` challenge of a `407`.
const AUTH_CHALLENGE: &str = "Basic realm=\"seleniumbase-rs\"";
/// The `Connection` value that ends a connection after a refusal.
const CLOSE: &str = "close";
/// How long the accept loop waits after a failed `accept` (out of file descriptors, say).
const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

/// The expected `Proxy-Authorization` value, kept only as its SHA-256.
pub(super) struct ProxyCredentials {
    /// SHA-256 of `Basic base64(user:token)`.
    expected: [u8; SHA256_LEN],
}

/// Bytes in a SHA-256 digest.
const SHA256_LEN: usize = 32;

impl ProxyCredentials {
    /// The credentials `user` with `token` as password.
    pub(super) fn new(user: &str, token: &Secret) -> Self {
        let pair = format!("{user}:{}", token.expose());
        let header = format!("Basic {}", STANDARD.encode(pair));
        Self {
            expected: sha256(header.as_bytes()),
        }
    }

    /// Whether `headers` carry exactly the expected `Proxy-Authorization`. The digests are
    /// compared in constant time, so the time taken says nothing about the token.
    pub(super) fn accepts(&self, headers: &HeaderMap) -> bool {
        headers
            .get(PROXY_AUTHORIZATION)
            .is_some_and(|value| same_digest(&sha256(value.as_bytes()), &self.expected))
    }
}

/// SHA-256 of `bytes`.
fn sha256(bytes: &[u8]) -> [u8; SHA256_LEN] {
    let mut out = [0_u8; SHA256_LEN];
    out.copy_from_slice(digest(&SHA256, bytes).as_ref());
    out
}

/// Compares two digests without an early exit.
fn same_digest(left: &[u8; SHA256_LEN], right: &[u8; SHA256_LEN]) -> bool {
    let difference = left
        .iter()
        .zip(right)
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b));
    std::hint::black_box(difference) == 0
}

/// A `CONNECT` answered `200`, waiting for hyper to hand over the connection.
struct Tunnel {
    /// Where the requests inside go.
    origin: Origin,
    /// The leaf for the `CONNECT` host.
    leaf: Arc<Leaf>,
    /// Resolves to the raw connection once the `200` is written.
    upgrade: OnUpgrade,
}

/// The tunnel a connection's `CONNECT` opened, if any.
type PendingTunnel = Arc<Mutex<Option<Tunnel>>>;

/// Accepts browser connections until told to stop, then waits for the connections to end.
pub(super) async fn run(
    listener: TcpListener,
    shared: Arc<Shared>,
    mut stop: watch::Receiver<bool>,
) {
    let permits = Arc::new(Semaphore::new(shared.options.max_connections));
    let mut clients = JoinSet::new();
    while let Some((tcp, permit)) = next_client(&listener, &permits, &mut stop).await {
        while clients.try_join_next().is_some() {}
        Counters::bump(&shared.counters.connections);
        let shared = Arc::clone(&shared);
        let stop = stop.clone();
        clients.spawn(async move {
            let _permit: OwnedSemaphorePermit = permit;
            serve_client(tcp, shared, stop).await;
        });
    }
    drop(listener);
    while clients.join_next().await.is_some() {}
}

/// Resolves once the proxy is told to stop, or once nobody can tell it anything any more.
pub(super) async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _seen = stop.wait_for(|stopped| *stopped).await;
}

/// The next loopback client and its connection permit, or `None` once the proxy stops.
async fn next_client(
    listener: &TcpListener,
    permits: &Arc<Semaphore>,
    stop: &mut watch::Receiver<bool>,
) -> Option<(TcpStream, OwnedSemaphorePermit)> {
    loop {
        let permit = tokio::select! {
            biased;
            () = stopped(stop) => return None,
            permit = Arc::clone(permits).acquire_owned() => permit.ok()?,
        };
        let accepted = tokio::select! {
            biased;
            () = stopped(stop) => return None,
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok((tcp, peer)) if peer.ip().is_loopback() => return Some((tcp, permit)),
            Ok(_) => tracing::warn!("dropped a connection from outside loopback"),
            Err(error) => {
                tracing::debug!(%error, "accept failed");
                tokio::time::sleep(ACCEPT_BACKOFF).await;
            }
        }
    }
}

/// The HTTP/1 server settings of both legs: the head and idle limits, no automatic `Date`.
fn server(options: &MitmOptions) -> http1::Builder {
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(options.idle_timeout)
        .max_headers(options.max_headers)
        .max_header_size(options.max_header_bytes)
        .max_buf_size(options.max_header_bytes)
        .keep_alive(true)
        .auto_date_header(false);
    builder
}

/// Serves one browser connection: proxy requests until a `CONNECT` is answered, then the
/// requests inside its TLS.
async fn serve_client(tcp: TcpStream, shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    let pending = PendingTunnel::default();
    let for_tunnel = Arc::clone(&shared);
    let connection = server(&shared.options)
        .serve_connection(
            TokioIo::new(tcp),
            proxy_service(shared, Arc::clone(&pending)),
        )
        .with_upgrades();
    let mut connection = pin!(connection);
    let served = tokio::select! {
        served = connection.as_mut() => served,
        () = stopped(&mut stop) => {
            connection.as_mut().graceful_shutdown();
            connection.await
        }
    };
    if let Err(error) = served {
        tracing::debug!(%error, "a proxy connection ended with an error");
        return;
    }
    let tunnel = locked(&pending).take();
    if let Some(tunnel) = tunnel {
        intercept(tunnel, for_tunnel, stop).await;
    }
}

/// The service of the proxy leg: every request goes to [`answer_proxy_request`].
fn proxy_service(
    shared: Arc<Shared>,
    pending: PendingTunnel,
) -> impl hyper::service::Service<
    Request<Incoming>,
    Response = Response<ProxyBody>,
    Error = Infallible,
    Future = impl Future<Output = Result<Response<ProxyBody>, Infallible>> + Send,
> + Send {
    service_fn(move |request| {
        let shared = Arc::clone(&shared);
        let pending = Arc::clone(&pending);
        async move { Ok(answer_proxy_request(&shared, &pending, request).await) }
    })
}

/// Checks the token, then opens a tunnel, forwards an absolute-form `http://` request, or
/// refuses.
async fn answer_proxy_request(
    shared: &Shared,
    pending: &Mutex<Option<Tunnel>>,
    request: Request<Incoming>,
) -> Response<ProxyBody> {
    if !shared.credentials.accepts(request.headers()) {
        Counters::bump(&shared.counters.unauthorized);
        tracing::debug!(error = %MitmError::Unauthorized, "proxy request refused");
        return proxy_auth_required();
    }
    if request.method() == Method::CONNECT {
        return open_tunnel(shared, pending, request);
    }
    let origin = Origin::from_absolute(request.uri()).and_then(|origin| {
        origin.check_destination(&shared.options)?;
        Ok(origin)
    });
    match origin {
        Ok(origin) => forward::forward(shared, &origin, request, None).await,
        Err(error) => refuse(
            shared,
            refusal_status(&error, StatusCode::BAD_REQUEST),
            &error,
            true,
        ),
    }
}

/// Answers a `CONNECT`: `200` and a pending tunnel when the target can be intercepted, an empty
/// `502` that closes the connection otherwise.
fn open_tunnel(
    shared: &Shared,
    pending: &Mutex<Option<Tunnel>>,
    mut request: Request<Incoming>,
) -> Response<ProxyBody> {
    let target = Origin::from_connect(request.uri()).and_then(|(origin, host)| {
        origin.check_destination(&shared.options)?;
        let leaf = shared
            .ca
            .leaf_for(&host, SystemTime::now())
            .map_err(|error| match error {
                TlsServerError::InvalidHost(_) => {
                    MitmError::BadTarget("no certificate can name this host")
                }
                other => MitmError::Tls(other),
            })?;
        Ok((origin, leaf))
    });
    match target {
        Ok((origin, leaf)) => {
            let upgrade = hyper::upgrade::on(&mut request);
            *locked(pending) = Some(Tunnel {
                origin,
                leaf,
                upgrade,
            });
            empty(StatusCode::OK)
        }
        Err(error) => refuse(
            shared,
            refusal_status(&error, StatusCode::BAD_GATEWAY),
            &error,
            true,
        ),
    }
}

/// The status of a refused target: `403` when the destination is not allowed, `otherwise` for a
/// target that is malformed or cannot be intercepted.
fn refusal_status(error: &MitmError, otherwise: StatusCode) -> StatusCode {
    match error {
        MitmError::ForbiddenDestination(_) => StatusCode::FORBIDDEN,
        _ => otherwise,
    }
}

/// Ends the browser's TLS inside a tunnel and serves the requests in it, then the WebSocket a
/// handshake in it opened, if any.
async fn intercept(tunnel: Tunnel, shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    let upgraded = match tunnel.upgrade.await {
        Ok(upgraded) => upgraded,
        Err(error) => {
            tracing::debug!(%error, "the CONNECT upgrade failed");
            return;
        }
    };
    let timeout = shared.options.handshake_timeout;
    let tls = match tls_server::accept(
        &shared.acceptor,
        &tunnel.leaf,
        TokioIo::new(upgraded),
        timeout,
    )
    .await
    {
        Ok(tls) => tls,
        Err(error) => {
            Counters::bump(&shared.counters.refused);
            tracing::debug!(error = %MitmError::Tls(error), "TLS interception refused");
            return;
        }
    };
    Counters::bump(&shared.counters.tunnels);
    let builder = server(&shared.options);
    let slot = Arc::new(RelaySlot::default());
    let service = tunnel_service(
        Arc::clone(&shared),
        Arc::new(tunnel.origin),
        Arc::clone(&slot),
    );
    let connection = builder
        .serve_connection(TokioIo::new(tls), service)
        .with_upgrades();
    let mut connection = pin!(connection);
    let served = tokio::select! {
        served = connection.as_mut() => served,
        () = stopped(&mut stop) => {
            connection.as_mut().graceful_shutdown();
            connection.await
        }
    };
    if let Err(error) = served {
        tracing::debug!(%error, "an intercepted connection ended with an error");
        return;
    }
    let pending = locked(&slot).take();
    if let Some(pending) = pending {
        websocket::run(pending, &shared.options, &mut stop).await;
    }
}

/// The service of the intercepted leg: every request is forwarded to the tunnel's origin, and a
/// WebSocket handshake leaves its relay in `slot`.
fn tunnel_service(
    shared: Arc<Shared>,
    origin: Arc<Origin>,
    slot: Arc<RelaySlot>,
) -> impl hyper::service::Service<
    Request<Incoming>,
    Response = Response<ProxyBody>,
    Error = Infallible,
    Future = impl Future<Output = Result<Response<ProxyBody>, Infallible>> + Send,
> + Send {
    service_fn(move |request| {
        let shared = Arc::clone(&shared);
        let origin = Arc::clone(&origin);
        let slot = Arc::clone(&slot);
        async move { Ok(forward::forward(&shared, &origin, request, Some(slot.as_ref())).await) }
    })
}

/// An empty `407` with the Basic challenge.
fn proxy_auth_required() -> Response<ProxyBody> {
    let mut response = empty(StatusCode::PROXY_AUTHENTICATION_REQUIRED);
    response
        .headers_mut()
        .insert(PROXY_AUTHENTICATE, HeaderValue::from_static(AUTH_CHALLENGE));
    response
}

/// Logs a refusal with its named error, counts it, and answers an empty `status`, closing the
/// connection when `close` is set.
pub(super) fn refuse(
    shared: &Shared,
    status: StatusCode,
    error: &MitmError,
    close: bool,
) -> Response<ProxyBody> {
    Counters::bump(&shared.counters.refused);
    tracing::debug!(%error, status = status.as_u16(), "request refused");
    let mut response = empty(status);
    if close {
        response
            .headers_mut()
            .insert(CONNECTION, HeaderValue::from_static(CLOSE));
    }
    response
}
