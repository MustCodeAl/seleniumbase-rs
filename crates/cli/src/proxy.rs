//! `sbrs proxy`: a minimal forward proxy for this machine.
//!
//! It speaks HTTP/1.x to its client and does two things. A `CONNECT host:port` request is
//! answered with `200` and the connection becomes a tunnel to that address, which is how a
//! browser reaches HTTPS sites. An absolute-form request such as `GET http://host/path` is
//! rewritten to origin form and sent to the host with `Connection: close`, so one client
//! connection carries one forwarded request. Nothing is decrypted, cached or authenticated.
//!
//! The limits are fixed (see [`Limits`]): a request head of at most 64 KiB, 256 connections
//! at once and 30 seconds without a byte in either direction. A request the proxy refuses
//! gets a short plain-text answer and the connection is closed.

use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use seleniumbase_rs::common::shutdown::Shutdown;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::args::tools::ProxyArgs;

/// What the proxy puts up with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// The most bytes a request head (the request line and the headers) may take.
    pub head: usize,
    /// The most connections served at the same time; further clients wait to be accepted.
    pub connections: usize,
    /// How long a head may take to arrive, and how long a connection may carry no byte in
    /// either direction.
    pub idle: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            head: 64 * 1024,
            connections: 256,
            idle: Duration::from_secs(30),
        }
    }
}

/// The headers a proxy must not pass on, besides `Host`, which it writes itself.
const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "proxy-connection",
    "keep-alive",
    "proxy-authorization",
    "proxy-authenticate",
    "te",
    "trailer",
    "upgrade",
];

/// A listening proxy.
pub(crate) struct Proxy {
    listener: TcpListener,
    local: SocketAddr,
    limits: Limits,
}

impl Proxy {
    /// Listens on `host:port` (port 0 picks a free one).
    ///
    /// # Errors
    ///
    /// Returns the error of the lookup or the bind.
    pub(crate) async fn bind(host: &str, port: u16, limits: Limits) -> io::Result<Self> {
        let address = tokio::net::lookup_host((host, port))
            .await?
            .next()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{host} names no address"),
                )
            })?;
        let listener = TcpListener::bind(address).await?;
        let local = listener.local_addr()?;
        Ok(Self {
            listener,
            local,
            limits,
        })
    }

    /// The address the proxy listens on, with the port that was picked for port 0.
    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// Serves clients until the future is dropped.
    pub(crate) async fn serve(self) {
        let slots = Arc::new(Semaphore::new(self.limits.connections));
        loop {
            // A slot is taken before a client is accepted, so a full proxy leaves the
            // waiting clients in the operating system's queue.
            let Ok(slot) = Arc::clone(&slots).acquire_owned().await else {
                return;
            };
            match self.listener.accept().await {
                Ok((client, _)) => {
                    let (own, limits) = (self.local, self.limits);
                    tokio::spawn(async move {
                        serve_connection(client, own, limits).await;
                        drop(slot);
                    });
                }
                Err(error) => {
                    tracing::warn!(%error, "the proxy could not accept a connection");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

/// `sbrs proxy`: listens, says how to use it, and serves until Ctrl-C or SIGTERM, which
/// ends it with status 0.
pub(crate) async fn run(args: &ProxyArgs) -> Result<(), Box<dyn Error>> {
    let proxy = Proxy::bind(&args.hostname, args.port, Limits::default())
        .await
        .map_err(|error| format!("cannot listen on {}:{}: {error}", args.hostname, args.port))?;
    let local = proxy.local_addr();
    let mut shutdown = Shutdown::install()?;
    println!("Proxy listening on http://{local}");
    let usable = if local.ip().is_unspecified() {
        SocketAddr::new(std::net::Ipv4Addr::LOCALHOST.into(), local.port())
    } else {
        local
    };
    println!("Use it with: sbrs --proxy {usable} open https://example.com");
    if !local.ip().is_loopback() {
        eprintln!(
            "warning: {local} accepts connections from other machines; this is an open proxy"
        );
    }
    tokio::select! {
        () = proxy.serve() => {}
        _ = shutdown.recv() => {}
    }
    Ok(())
}

/// Serves one client connection and logs what became of it.
async fn serve_connection(mut client: TcpStream, own: SocketAddr, limits: Limits) {
    let peer = client
        .peer_addr()
        .map_or_else(|_| "?".to_owned(), |a| a.to_string());
    if let Err(error) = handle(&mut client, &peer, own, limits).await {
        tracing::debug!(%peer, %error, "a proxied connection ended with an error");
    }
}

async fn handle(
    client: &mut TcpStream,
    peer: &str,
    own: SocketAddr,
    limits: Limits,
) -> io::Result<()> {
    let (buffer, end) = match timeout(limits.idle, read_head(client, limits.head)).await {
        Err(_) => {
            tracing::info!(%peer, "rejected: no complete request head in time");
            return reply(
                client,
                408,
                "Request Timeout",
                "no complete request head arrived",
            )
            .await;
        }
        Ok(Err(HeadError::Closed)) => return Ok(()),
        Ok(Err(HeadError::Io(error))) => return Err(error),
        Ok(Err(HeadError::TooLarge)) => {
            tracing::info!(%peer, "rejected: request head too large");
            return reply(
                client,
                431,
                "Request Header Fields Too Large",
                "the request head is larger than the proxy accepts",
            )
            .await;
        }
        Ok(Ok(found)) => found,
    };
    let routed = parse_head(&buffer[..end]).and_then(|head| {
        route(&head).map(|route| (head.method.to_owned(), head.target.to_owned(), route))
    });
    let (method, target, route) = match routed {
        Ok(found) => found,
        Err(why) => {
            tracing::info!(%peer, "rejected: {why}");
            return reply(client, 400, "Bad Request", why).await;
        }
    };
    tracing::info!(%peer, "{method} {target}");
    let (address, first) = match &route {
        Route::Connect(target) => (target.address(), None),
        Route::Forward { target, request } => (target.address(), Some(request.as_slice())),
    };
    let mut upstream = match timeout(limits.idle, TcpStream::connect(&address)).await {
        Err(_) => {
            return reply(
                client,
                504,
                "Gateway Timeout",
                "the host did not answer in time",
            )
            .await
        }
        Ok(Err(error)) => {
            tracing::info!(%peer, "{address}: {error}");
            return reply(
                client,
                502,
                "Bad Gateway",
                &format!("cannot reach {address}"),
            )
            .await;
        }
        Ok(Ok(stream)) => stream,
    };
    if upstream
        .peer_addr()
        .is_ok_and(|reached| reaches_itself(reached, own))
    {
        return reply(
            client,
            508,
            "Loop Detected",
            "the proxy was asked to reach itself",
        )
        .await;
    }
    match first {
        None => {
            client
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;
        }
        Some(request) => upstream.write_all(request).await?,
    }
    // Whatever the client sent after its head (a body, or the first bytes of a tunnel).
    upstream.write_all(&buffer[end..]).await?;
    tunnel(client, &mut upstream, limits.idle).await
}

/// Whether `reached` is the proxy's own listening address.
fn reaches_itself(reached: SocketAddr, own: SocketAddr) -> bool {
    reached.port() == own.port()
        && (reached.ip() == own.ip() || (own.ip().is_unspecified() && reached.ip().is_loopback()))
}

async fn reply(client: &mut TcpStream, status: u16, reason: &str, message: &str) -> io::Result<()> {
    let body = format!("{message}\n");
    let text = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    client.write_all(text.as_bytes()).await?;
    client.shutdown().await
}

/// Why a request head was not read.
enum HeadError {
    TooLarge,
    Closed,
    Io(io::Error),
}

/// Reads up to the empty line that ends a request head. Returns what was read, which may
/// hold more than the head, and where the head ends.
async fn read_head(stream: &mut TcpStream, max: usize) -> Result<(Vec<u8>, usize), HeadError> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 4096];
    let mut scanned = 0;
    loop {
        if let Some(end) = find_head_end(&buffer, scanned) {
            return if end > max {
                Err(HeadError::TooLarge)
            } else {
                Ok((buffer, end))
            };
        }
        if buffer.len() >= max {
            return Err(HeadError::TooLarge);
        }
        scanned = buffer.len().saturating_sub(3);
        let count = stream.read(&mut chunk).await.map_err(HeadError::Io)?;
        if count == 0 {
            return Err(HeadError::Closed);
        }
        buffer.extend_from_slice(&chunk[..count]);
    }
}

/// Where the first empty line ends, looking from `from`.
fn find_head_end(buffer: &[u8], from: usize) -> Option<usize> {
    buffer
        .get(from..)?
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| from + at + 4)
}

/// A parsed request head.
#[derive(Debug, Eq, PartialEq)]
struct Head<'a> {
    method: &'a str,
    target: &'a str,
    headers: Vec<(&'a str, &'a str)>,
}

/// Reads a request head that ends with its empty line.
fn parse_head(raw: &[u8]) -> Result<Head<'_>, &'static str> {
    let text = std::str::from_utf8(raw).map_err(|_| "the request head is not text")?;
    let text = text
        .strip_suffix("\r\n\r\n")
        .ok_or("the request head does not end with an empty line")?;
    if text.bytes().any(|byte| byte == 0) || text.replace("\r\n", "").contains(['\r', '\n']) {
        return Err("the request head has a stray control character");
    }
    let mut lines = text.split("\r\n");
    let mut words = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err("the request line is not `METHOD TARGET VERSION`");
    };
    if method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err("the request method is not valid");
    }
    if target.is_empty() {
        return Err("the request has no target");
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err("only HTTP/1.0 and HTTP/1.1 are accepted");
    }
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or("a header line has no colon")?;
        if name.is_empty() || !name.bytes().all(is_token_byte) {
            return Err("a header name is not valid");
        }
        headers.push((name, value.trim_matches([' ', '\t'])));
    }
    Ok(Head {
        method,
        target,
        headers,
    })
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// A host and port to connect to.
#[derive(Debug, Eq, PartialEq)]
struct Target {
    /// A name, an IPv4 address, or an IPv6 address in brackets.
    host: String,
    port: u16,
}

impl Target {
    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// What a request asks the proxy to do.
#[derive(Debug, Eq, PartialEq)]
enum Route {
    /// Tunnel to the target.
    Connect(Target),
    /// Send `request` (the rewritten head) to the target.
    Forward { target: Target, request: Vec<u8> },
}

/// Decides what to do with a request, or why it is refused.
fn route(head: &Head<'_>) -> Result<Route, &'static str> {
    if head.method == "CONNECT" {
        return parse_authority(head.target, None).map(Route::Connect);
    }
    let Some(rest) = strip_prefix_ignore_case(head.target, "http://") else {
        return Err(
            if strip_prefix_ignore_case(head.target, "https://").is_some() {
                "an https:// URL cannot be forwarded; HTTPS goes through CONNECT"
            } else {
                "this is a proxy: send an absolute http:// URL, or CONNECT host:port"
            },
        );
    };
    if rest.contains('#') {
        return Err("a request target has no fragment");
    }
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let target = parse_authority(authority, Some(80))?;
    let path = match tail {
        "" => "/".to_owned(),
        tail if tail.starts_with('/') => tail.to_owned(),
        tail => format!("/{tail}"),
    };
    let host = if target.port == 80 {
        target.host.clone()
    } else {
        target.address()
    };
    let mut request = format!("{} {path} HTTP/1.1\r\nHost: {host}\r\n", head.method);
    for (name, value) in &head.headers {
        let lower = name.to_ascii_lowercase();
        if lower == "host" || HOP_BY_HOP.contains(&lower.as_str()) {
            continue;
        }
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("Connection: close\r\n\r\n");
    Ok(Route::Forward {
        target,
        request: request.into_bytes(),
    })
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Reads `host`, `host:port` or `[v6]:port`; `default_port` is used when none is given.
fn parse_authority(text: &str, default_port: Option<u16>) -> Result<Target, &'static str> {
    const BAD: &str = "the host and port are not valid";
    if text.is_empty() || text.contains(['@', ' ', '/', '\\', '?', '#']) {
        return Err(BAD);
    }
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (inside, after) = rest.split_once(']').ok_or(BAD)?;
        inside.parse::<std::net::Ipv6Addr>().map_err(|_| BAD)?;
        let port = match after {
            "" => None,
            after => Some(after.strip_prefix(':').ok_or(BAD)?),
        };
        (format!("[{inside}]"), port)
    } else {
        let (host, port) = match text.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (text, None),
        };
        let name = host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
        if host.is_empty() || !name {
            return Err(BAD);
        }
        (host.to_owned(), port)
    };
    let port = match port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(BAD)?,
        None => default_port.ok_or("CONNECT needs host:port")?,
    };
    Ok(Target { host, port })
}

/// The time of the last byte either way through a tunnel, to cut an idle one.
struct Clock {
    start: Instant,
    last: AtomicU64,
}

impl Clock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            last: AtomicU64::new(0),
        }
    }

    fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn touch(&self) {
        self.last.store(self.now(), Ordering::Relaxed);
    }

    /// Completes once `idle` has passed with no `touch`.
    async fn expired(&self, idle: Duration) {
        let idle = u64::try_from(idle.as_millis()).unwrap_or(u64::MAX);
        loop {
            let quiet = self.now().saturating_sub(self.last.load(Ordering::Relaxed));
            if quiet >= idle {
                return;
            }
            tokio::time::sleep(Duration::from_millis(idle - quiet)).await;
        }
    }
}

/// A stream that tells a [`Clock`] about every byte that passes.
struct Active<S> {
    inner: S,
    clock: Arc<Clock>,
}

impl<S: AsyncRead + Unpin> AsyncRead for Active<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let poll = Pin::new(&mut self.inner).poll_read(cx, buf);
        if matches!(poll, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            self.clock.touch();
        }
        poll
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Active<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let poll = Pin::new(&mut self.inner).poll_write(cx, data);
        if matches!(poll, Poll::Ready(Ok(count)) if count > 0) {
            self.clock.touch();
        }
        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Copies both ways until both sides are done, or until `idle` passes with no byte.
async fn tunnel(
    client: &mut TcpStream,
    upstream: &mut TcpStream,
    idle: Duration,
) -> io::Result<()> {
    let clock = Arc::new(Clock::new());
    let mut client = Active {
        inner: client,
        clock: Arc::clone(&clock),
    };
    let mut upstream = Active {
        inner: upstream,
        clock: Arc::clone(&clock),
    };
    tokio::select! {
        done = copy_bidirectional(&mut client, &mut upstream) => done.map(|_| ()),
        () = clock.expired(idle) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    fn head(raw: &str) -> Head<'_> {
        parse_head(raw.as_bytes()).expect("a valid head")
    }

    fn forwarded(raw: &str) -> String {
        match route(&head(raw)).expect("a route") {
            Route::Forward { request, .. } => String::from_utf8(request).expect("text"),
            Route::Connect(_) => panic!("expected a forwarded request"),
        }
    }

    #[test]
    fn an_absolute_request_becomes_an_origin_form_request_that_closes() {
        let request = forwarded(
            "GET http://example.com:8080/a/b?x=1 HTTP/1.1\r\nHost: wrong\r\n\
             Proxy-Connection: keep-alive\r\nConnection: keep-alive\r\n\
             Proxy-Authorization: Basic abc\r\nAccept: */*\r\n\r\n",
        );
        assert_eq!(
            request,
            "GET /a/b?x=1 HTTP/1.1\r\nHost: example.com:8080\r\nAccept: */*\r\n\
             Connection: close\r\n\r\n"
        );
    }

    #[test]
    fn the_default_port_is_left_out_of_host_and_a_missing_path_is_a_slash() {
        let request = forwarded("POST http://example.com HTTP/1.1\r\nContent-Length: 2\r\n\r\n");
        assert!(
            request.starts_with("POST / HTTP/1.1\r\nHost: example.com\r\n"),
            "{request}"
        );
        assert!(request.contains("Content-Length: 2\r\n"), "{request}");
        let request = forwarded("GET http://example.com?q=1 HTTP/1.1\r\n\r\n");
        assert!(request.starts_with("GET /?q=1 HTTP/1.1\r\n"), "{request}");
    }

    #[test]
    fn connect_names_a_host_and_a_port() {
        let route = route(&head("CONNECT example.com:443 HTTP/1.1\r\n\r\n")).expect("a route");
        assert_eq!(
            route,
            Route::Connect(Target {
                host: "example.com".to_owned(),
                port: 443
            })
        );
        let route = route_of("CONNECT [::1]:8443 HTTP/1.1\r\n\r\n");
        assert_eq!(
            route.expect("a route"),
            Route::Connect(Target {
                host: "[::1]".to_owned(),
                port: 8443
            })
        );
        assert!(route_of("CONNECT example.com HTTP/1.1\r\n\r\n").is_err());
        assert!(route_of("CONNECT example.com:0 HTTP/1.1\r\n\r\n").is_err());
        assert!(route_of("CONNECT user@example.com:443 HTTP/1.1\r\n\r\n").is_err());
    }

    fn route_of(raw: &str) -> Result<Route, &'static str> {
        parse_head(raw.as_bytes()).and_then(|head| route(&head))
    }

    #[test]
    fn what_cannot_be_forwarded_is_refused() {
        for raw in [
            "GET /path HTTP/1.1\r\n\r\n",
            "GET https://example.com/ HTTP/1.1\r\n\r\n",
            "GET ftp://example.com/ HTTP/1.1\r\n\r\n",
            "GET http://user:pw@example.com/ HTTP/1.1\r\n\r\n",
            "GET http://example.com:99999/ HTTP/1.1\r\n\r\n",
            "GET http://exa mple.com/ HTTP/1.1\r\n\r\n",
            "GET http://example.com/#frag HTTP/1.1\r\n\r\n",
        ] {
            assert!(route_of(raw).is_err(), "{raw:?} was accepted");
        }
    }

    #[test]
    fn a_malformed_head_is_refused() {
        for raw in [
            "GET http://a/ HTTP/2\r\n\r\n",
            "GET http://a/\r\n\r\n",
            "GET  http://a/ HTTP/1.1\r\n\r\n",
            "get http://a/ HTTP/1.1\r\n\r\n",
            "GET http://a/ HTTP/1.1\r\nNo colon here\r\n\r\n",
            "GET http://a/ HTTP/1.1\r\nBad Name: x\r\n\r\n",
            "GET http://a/ HTTP/1.1\r\nX: a\nInjected: b\r\n\r\n",
            "GET http://a/ HTTP/1.1\r\nX: a\0b\r\n\r\n",
            "\r\n\r\n",
        ] {
            assert!(parse_head(raw.as_bytes()).is_err(), "{raw:?} was accepted");
        }
        assert!(parse_head(&[0xff, 0xfe, b'\r', b'\n', b'\r', b'\n']).is_err());
    }

    #[test]
    fn the_end_of_the_head_is_found_across_reads() {
        assert_eq!(find_head_end(b"GET / HTTP/1.1\r\n\r\nbody", 0), Some(18));
        assert_eq!(find_head_end(b"GET / HTTP/1.1\r\n\r", 0), None);
        assert_eq!(find_head_end(b"GET / HTTP/1.1\r\n\r\n", 14), Some(18));
    }

    #[test]
    fn the_proxy_knows_its_own_address() {
        let own: SocketAddr = "127.0.0.1:8899".parse().unwrap();
        assert!(reaches_itself("127.0.0.1:8899".parse().unwrap(), own));
        assert!(!reaches_itself("127.0.0.1:8900".parse().unwrap(), own));
        let any: SocketAddr = "0.0.0.0:8899".parse().unwrap();
        assert!(reaches_itself("127.0.0.1:8899".parse().unwrap(), any));
        assert!(!reaches_itself("93.184.216.34:8899".parse().unwrap(), any));
    }

    async fn start(limits: Limits) -> SocketAddr {
        let proxy = Proxy::bind("127.0.0.1", 0, limits).await.expect("binds");
        let address = proxy.local_addr();
        tokio::spawn(proxy.serve());
        address
    }

    /// An origin that records the head it is sent and answers `ok`.
    async fn origin() -> (SocketAddr, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let address = listener.local_addr().expect("address");
        let (sent, received) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accepts");
            let mut seen = Vec::new();
            let mut chunk = [0_u8; 1024];
            while find_head_end(&seen, 0).is_none() {
                let count = stream.read(&mut chunk).await.expect("reads");
                assert!(count > 0, "the proxy closed before the head ended");
                seen.extend_from_slice(&chunk[..count]);
            }
            let _ = sent.send(String::from_utf8_lossy(&seen).into_owned());
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .expect("writes");
        });
        (address, received)
    }

    /// An origin that sends back what it is sent.
    async fn echo() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut read, mut write) = stream.split();
                    let _ = tokio::io::copy(&mut read, &mut write).await;
                });
            }
        });
        address
    }

    /// Sends `request` and reads everything the proxy answers.
    async fn exchange(proxy: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(proxy).await.expect("connects");
        stream.write_all(request.as_bytes()).await.expect("writes");
        let mut answer = Vec::new();
        timeout(Duration::from_secs(5), stream.read_to_end(&mut answer))
            .await
            .expect("the proxy answers in time")
            .expect("reads");
        String::from_utf8_lossy(&answer).into_owned()
    }

    #[tokio::test]
    async fn a_forwarded_request_reaches_the_origin_in_origin_form() {
        let proxy = start(Limits::default()).await;
        let (origin, seen) = origin().await;
        let answer = exchange(
            proxy,
            &format!("GET http://{origin}/page?a=1 HTTP/1.1\r\nHost: {origin}\r\n\r\n"),
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
        assert!(answer.ends_with("ok"), "{answer}");
        let head = seen.await.expect("the origin saw a head");
        assert!(head.starts_with("GET /page?a=1 HTTP/1.1\r\n"), "{head}");
        assert!(head.contains(&format!("Host: {origin}\r\n")), "{head}");
        assert!(head.contains("Connection: close\r\n"), "{head}");
    }

    #[tokio::test]
    async fn a_connect_request_becomes_a_tunnel() {
        let proxy = start(Limits::default()).await;
        let echo = echo().await;
        let mut stream = TcpStream::connect(proxy).await.expect("connects");
        // The first bytes of the tunnel may arrive together with the head.
        stream
            .write_all(format!("CONNECT {echo} HTTP/1.1\r\nHost: {echo}\r\n\r\nearly ").as_bytes())
            .await
            .expect("writes");
        let mut answer = [0_u8; 39];
        stream
            .read_exact(&mut answer)
            .await
            .expect("the proxy answers");
        assert_eq!(&answer, b"HTTP/1.1 200 Connection Established\r\n\r\n");
        stream.write_all(b"late").await.expect("writes");
        let mut echoed = [0_u8; 10];
        timeout(Duration::from_secs(5), stream.read_exact(&mut echoed))
            .await
            .expect("the echo comes back")
            .expect("reads");
        assert_eq!(&echoed, b"early late");
    }

    #[tokio::test]
    async fn an_unreachable_host_is_a_bad_gateway() {
        let proxy = start(Limits::default()).await;
        let closed = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let address = closed.local_addr().expect("address");
        drop(closed);
        let answer = exchange(proxy, &format!("GET http://{address}/ HTTP/1.1\r\n\r\n")).await;
        assert!(
            answer.starts_with("HTTP/1.1 502 Bad Gateway\r\n"),
            "{answer}"
        );
    }

    #[tokio::test]
    async fn a_request_to_the_proxy_itself_is_not_followed() {
        let proxy = start(Limits::default()).await;
        let answer = exchange(proxy, &format!("CONNECT {proxy} HTTP/1.1\r\n\r\n")).await;
        assert!(
            answer.starts_with("HTTP/1.1 508 Loop Detected\r\n"),
            "{answer}"
        );
    }

    #[tokio::test]
    async fn a_request_that_is_not_for_a_proxy_is_a_bad_request() {
        let proxy = start(Limits::default()).await;
        let answer = exchange(proxy, "GET /index.html HTTP/1.1\r\nHost: x\r\n\r\n").await;
        assert!(
            answer.starts_with("HTTP/1.1 400 Bad Request\r\n"),
            "{answer}"
        );
        assert!(answer.contains("this is a proxy"), "{answer}");
    }

    #[tokio::test]
    async fn a_head_over_the_limit_is_refused() {
        let limits = Limits {
            head: 256,
            ..Limits::default()
        };
        let proxy = start(limits).await;
        let long = "x".repeat(400);
        let answer = exchange(
            proxy,
            &format!("GET http://a/ HTTP/1.1\r\nX-Long: {long}\r\n\r\n"),
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 431 "), "{answer}");
        // No terminator at all: the limit still applies.
        let answer = exchange(proxy, &format!("GET http://a/{long}")).await;
        assert!(answer.starts_with("HTTP/1.1 431 "), "{answer}");
    }

    #[tokio::test]
    async fn a_tunnel_with_no_traffic_is_closed() {
        let limits = Limits {
            idle: Duration::from_millis(300),
            ..Limits::default()
        };
        let proxy = start(limits).await;
        let echo = echo().await;
        let answer = exchange(proxy, &format!("CONNECT {echo} HTTP/1.1\r\n\r\n")).await;
        assert_eq!(answer, "HTTP/1.1 200 Connection Established\r\n\r\n");
    }

    #[tokio::test]
    async fn a_head_that_never_ends_times_out() {
        let limits = Limits {
            idle: Duration::from_millis(300),
            ..Limits::default()
        };
        let proxy = start(limits).await;
        let answer = exchange(proxy, "GET http://a/ HTTP/1.1\r\nHost: a\r\n").await;
        assert!(answer.starts_with("HTTP/1.1 408 "), "{answer}");
    }

    #[tokio::test]
    async fn clients_over_the_connection_limit_wait_for_a_free_slot() {
        let limits = Limits {
            connections: 1,
            ..Limits::default()
        };
        let proxy = start(limits).await;
        let echo = echo().await;
        // The first client keeps the only slot.
        let mut first = TcpStream::connect(proxy).await.expect("connects");
        first
            .write_all(format!("CONNECT {echo} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .expect("writes");
        let mut established = [0_u8; 39];
        first.read_exact(&mut established).await.expect("tunnel");

        let mut second = TcpStream::connect(proxy).await.expect("connects");
        second
            .write_all(b"GET /x HTTP/1.1\r\n\r\n")
            .await
            .expect("writes");
        let mut answer = [0_u8; 16];
        let waited = timeout(Duration::from_millis(300), second.read_exact(&mut answer)).await;
        assert!(waited.is_err(), "the second client was served at once");

        drop(first);
        timeout(Duration::from_secs(5), second.read_exact(&mut answer))
            .await
            .expect("the second client is served once the slot is free")
            .expect("reads");
        assert_eq!(&answer, b"HTTP/1.1 400 Bad");
    }
}
