//! Loopback sites, an egress proxy and raw clients for the MITM proxy tests.
//!
//! Every server here binds `127.0.0.1` and owns its tasks in a `JoinSet`, so dropping it stops
//! it. The sites use their own ephemeral CA; the proxy trusts one only when a test says so.
#![allow(
    dead_code,
    reason = "shared by the mock and the real-Chrome test binaries, which each use a part"
)]

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use btls::ssl::{SslConnector, SslMethod, SslVerifyMode};
use btls::x509::X509;
use bytes::Bytes;
use http_body_util::StreamBody;
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use seleniumbase_rs::impersonate::capture_h2::site::h2_reply;
use seleniumbase_rs::impersonate::client::ImpersonationProfile;
use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::scrape::egress::route::EgressRef;
use seleniumbase_rs::scrape::ledger::events::CollectingEventSink;
use seleniumbase_rs::scrape::runtime::capture::stream::{
    http1_response, read_h2c_preface, read_http1_request, Http1Request,
};
use seleniumbase_rs::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use seleniumbase_rs::wire::h2_frames::H2Preface;
use seleniumbase_rs::wire::BrowserKind;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinSet;
use tokio_btls::SslStream;
use url::Host;

/// The ALPN id of HTTP/2.
pub const ALPN_H2: &[u8] = b"h2";
/// How long a test waits for anything on loopback.
pub const WITHIN: Duration = Duration::from_secs(10);
/// How long a test waits to be sure something does not happen.
pub const QUIET: Duration = Duration::from_millis(400);
/// The bytes of one chunk of a streamed body.
pub const CHUNK: usize = 64 * 1024;
/// The page the recording origin answers with.
pub const PAGE: &[u8] =
    b"<!doctype html><title>origin</title><p id=marker>served by the origin</p>";
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;
/// The longest response or request head the raw client reads.
const MAX_HEAD: usize = 64 * 1024;

/// The measured Chrome profile.
pub fn profile() -> ImpersonationProfile {
    ImpersonationProfile::measured(BrowserKind::Chrome).expect("Chrome is measured")
}

/// A CA of its own, shared.
pub fn new_ca() -> Arc<EphemeralCa> {
    Arc::new(EphemeralCa::generate().expect("a CA"))
}

/// The DER of a CA's certificate.
pub fn der(ca: &EphemeralCa) -> Vec<u8> {
    ca.certificate().to_der().expect("the CA as DER")
}

/// A started proxy and the sink it reports to.
pub async fn start_mitm(
    egress: Option<EgressRef>,
    options: MitmOptions,
) -> (MitmProxy, CollectingEventSink) {
    let sink = CollectingEventSink::new();
    let mitm = MitmProxy::start(profile(), egress, options, Arc::new(sink.clone()))
        .await
        .expect("the proxy starts");
    (mitm, sink)
}

/// The token in a proxy's spec.
pub fn token(mitm: &MitmProxy) -> String {
    let spec = mitm.proxy_spec();
    let (userinfo, _) = spec.expose().rsplit_once('@').expect("userinfo");
    let (_, token) = userinfo.rsplit_once(':').expect("user:token");
    token.to_owned()
}

/// The `Proxy-Authorization` value the proxy expects.
pub fn proxy_auth(mitm: &MitmProxy) -> String {
    format!("Basic {}", STANDARD.encode(format!("sb:{}", token(mitm))))
}

// ===== A site that records the first request of each connection =====

/// The first request of one connection to a [`RecordingOrigin`].
#[derive(Debug)]
pub struct Seen {
    /// The parsed `ClientHello`.
    pub hello: ClientHello,
    /// The ALPN protocol the handshake settled on.
    pub alpn: Option<Vec<u8>>,
    /// The request.
    pub request: Recorded,
}

/// A request as the origin read it.
#[derive(Debug)]
pub enum Recorded {
    /// An HTTP/2 connection start.
    H2(H2Preface),
    /// An HTTP/1 request.
    Http1(Http1Request),
}

impl Seen {
    /// The HTTP/2 start, panicking when the connection was HTTP/1.
    pub fn h2(&self) -> &H2Preface {
        match &self.request {
            Recorded::H2(preface) => preface,
            Recorded::Http1(_) => panic!("the upstream spoke HTTP/1.1, not HTTP/2"),
        }
    }
}

/// An HTTPS site with its own CA that answers every connection's first request with [`PAGE`].
pub struct RecordingOrigin {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    seen: mpsc::Receiver<Seen>,
    _tasks: JoinSet<()>,
}

impl RecordingOrigin {
    /// Starts a site offering the ALPN ids in `alpn`.
    pub async fn start(alpn: &[&[u8]]) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let addr = listener.local_addr().expect("an address");
        let ca = new_ca();
        let server = Arc::new(acceptor(alpn).expect("an acceptor"));
        let (sender, seen) = mpsc::channel(64);
        let mut tasks = JoinSet::new();
        let site_ca = Arc::clone(&ca);
        tasks.spawn(async move {
            let mut connections = JoinSet::new();
            while let Ok((tcp, _peer)) = listener.accept().await {
                let (ca, server, sender) =
                    (Arc::clone(&site_ca), Arc::clone(&server), sender.clone());
                connections.spawn(async move {
                    if let Ok(seen) = record_one(tcp, &ca, &server).await {
                        let _sent = sender.send(seen).await;
                    }
                });
                while connections.try_join_next().is_some() {}
            }
        });
        Self {
            addr,
            ca,
            seen,
            _tasks: tasks,
        }
    }

    /// The site's address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The DER of the site's CA, for the proxy to trust.
    pub fn root(&self) -> Vec<u8> {
        der(&self.ca)
    }

    /// The next connection's first request, or `None` when none comes within `within`.
    pub async fn next(&mut self, within: Duration) -> Option<Seen> {
        tokio::time::timeout(within, self.seen.recv())
            .await
            .ok()
            .flatten()
    }
}

/// Serves one connection: hello, handshake, first request, [`PAGE`].
async fn record_one(
    tcp: TcpStream,
    ca: &EphemeralCa,
    server: &btls::ssl::SslAcceptor,
) -> Result<Seen, String> {
    let hello = peek_hello(&tcp).await?;
    let host = match hello.sni.as_deref() {
        Some(name) => Host::parse(name).map_err(|e| e.to_string())?,
        None => Host::Ipv4(Ipv4Addr::LOCALHOST),
    };
    let leaf = ca
        .leaf_for(&host, SystemTime::now())
        .map_err(|e| e.to_string())?;
    let mut tls = accept(server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT)
        .await
        .map_err(|e| e.to_string())?;
    let alpn = tls.ssl().selected_alpn_protocol().map(<[u8]>::to_vec);
    let request = if alpn.as_deref() == Some(ALPN_H2) {
        let (_raw, preface) = read_h2c_preface(&mut tls, WITHIN)
            .await
            .map_err(|e| e.to_string())?;
        let reply = h2_reply(preface.request_stream_id, Some(PAGE)).map_err(|e| e.to_string())?;
        tls.write_all(&reply).await.map_err(|e| e.to_string())?;
        Recorded::H2(preface)
    } else {
        let request = read_http1_request(&mut tls, WITHIN)
            .await
            .map_err(|e| e.to_string())?;
        let reply = http1_response(200, "OK", &[("Content-Type", "text/html")], PAGE);
        tls.write_all(&reply).await.map_err(|e| e.to_string())?;
        Recorded::Http1(request)
    };
    tls.flush().await.map_err(|e| e.to_string())?;
    let seen = Seen {
        hello,
        alpn,
        request,
    };
    drain(&mut tls).await;
    Ok(seen)
}

/// Reads the `ClientHello` waiting on `tcp` without consuming it.
async fn peek_hello(tcp: &TcpStream) -> Result<ClientHello, String> {
    let work = async {
        let mut buf = vec![0_u8; MAX_CLIENT_HELLO + PEEK_SLACK];
        loop {
            let seen = tcp.peek(&mut buf).await.map_err(|e| e.to_string())?;
            if seen == 0 {
                return Err("closed before a hello".to_owned());
            }
            let prefix = buf.get(..seen).unwrap_or_default();
            let needed = ClientHello::needed_len(prefix).map_err(|e| e.to_string())?;
            if let Some(end) = needed.filter(|&end| end <= seen) {
                return ClientHello::parse(prefix.get(..end).unwrap_or_default())
                    .map_err(|e| e.to_string());
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    };
    tokio::time::timeout(WITHIN, work)
        .await
        .map_err(|_elapsed| "no hello in time".to_owned())?
}

/// Reads and drops what the peer still sends for a moment, then closes.
async fn drain<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S) {
    let mut sink = [0_u8; 1024];
    let reading = async {
        while let Ok(read) = stream.read(&mut sink).await {
            if read == 0 {
                break;
            }
        }
    };
    let _timed_out = tokio::time::timeout(QUIET, reading).await;
    let _closed = stream.shutdown().await;
}

// ===== A site that streams a large body =====

/// An HTTP/1.1 site (TLS or plain) whose `/` answers `total` bytes: one chunk, then the rest
/// only after [`StreamOrigin::release`]. A proxy that buffered the body whole would never hand
/// the first chunk on, and the test would time out. With `announce`, the answer carries a
/// `Content-Length`; without, it is chunked.
pub struct StreamOrigin {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    gate: Arc<Notify>,
    requests: Arc<AtomicUsize>,
    _tasks: JoinSet<()>,
}

impl StreamOrigin {
    /// Starts the site; `tls` picks HTTPS (ALPN `http/1.1` only) or plain HTTP.
    pub async fn start(tls: bool, total: usize, announce: bool) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let addr = listener.local_addr().expect("an address");
        let (ca, gate, requests) = (new_ca(), Arc::new(Notify::new()), Arc::default());
        let shared = (Arc::clone(&ca), Arc::clone(&gate), Arc::clone(&requests));
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let server = Arc::new(acceptor(&[ALPN_HTTP1]).expect("an acceptor"));
            let mut connections = JoinSet::new();
            while let Ok((tcp, _peer)) = listener.accept().await {
                let (ca, gate, requests) = (
                    Arc::clone(&shared.0),
                    Arc::clone(&shared.1),
                    Arc::clone(&shared.2),
                );
                let server = Arc::clone(&server);
                connections.spawn(async move {
                    let site = Body {
                        total,
                        announce,
                        gate,
                        requests,
                    };
                    serve_stream(tcp, tls.then_some((&*ca, &*server)), site).await;
                });
                while connections.try_join_next().is_some() {}
            }
        });
        Self {
            addr,
            ca,
            gate,
            requests,
            _tasks: tasks,
        }
    }

    /// The site's address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The DER of the site's CA.
    pub fn root(&self) -> Vec<u8> {
        der(&self.ca)
    }

    /// Lets the rest of the body go.
    pub fn release(&self) {
        self.gate.notify_one();
    }

    /// How many requests reached the site.
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

/// What a [`StreamOrigin`] answers.
struct Body {
    total: usize,
    announce: bool,
    gate: Arc<Notify>,
    requests: Arc<AtomicUsize>,
}

/// Serves one connection of a [`StreamOrigin`].
async fn serve_stream(
    tcp: TcpStream,
    tls: Option<(&EphemeralCa, &btls::ssl::SslAcceptor)>,
    site: Body,
) {
    let service = service_fn(move |_request: Request<Incoming>| {
        site.requests.fetch_add(1, Ordering::SeqCst);
        let gate = Arc::clone(&site.gate);
        let (total, announce) = (site.total, site.announce);
        async move { Ok::<_, Infallible>(gated_body(total, announce, gate)) }
    });
    let builder = http1::Builder::new();
    match tls {
        Some((ca, server)) => {
            let host = Host::parse("stream.test").expect("a host");
            let Ok(leaf) = ca.leaf_for(&host, SystemTime::now()) else {
                return;
            };
            let Ok(stream) = accept(server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await else {
                return;
            };
            let _served = builder
                .serve_connection(TokioIo::new(stream), service)
                .await;
        }
        None => {
            let _served = builder.serve_connection(TokioIo::new(tcp), service).await;
        }
    }
}

/// A response of `total` bytes: one chunk at once, the rest after `gate` opens.
fn gated_body(
    total: usize,
    announce: bool,
    gate: Arc<Notify>,
) -> Response<StreamBody<impl futures_util::Stream<Item = Result<Frame<Bytes>, Infallible>> + Send>>
{
    let chunks = futures_util::stream::unfold((0_usize, gate), move |(sent, gate)| async move {
        if sent >= total {
            return None;
        }
        if sent == CHUNK {
            gate.notified().await;
        }
        let size = CHUNK.min(total - sent);
        Some((
            Ok(Frame::data(Bytes::from(vec![b'x'; size]))),
            (sent + size, gate),
        ))
    });
    let mut response = Response::builder().header("content-type", "application/octet-stream");
    if announce {
        response = response.header("content-length", total);
    }
    response.body(StreamBody::new(chunks)).expect("a response")
}

// ===== An egress proxy =====

/// An HTTP proxy that tunnels every `CONNECT` to one fixed address and records what it saw.
pub struct EgressProxy {
    addr: SocketAddr,
    tunnels: Arc<AtomicUsize>,
    auth: Arc<Mutex<Vec<Option<String>>>>,
    _tasks: JoinSet<()>,
}

impl EgressProxy {
    /// Starts a proxy that sends every tunnel to `target`.
    pub async fn start(target: SocketAddr) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let addr = listener.local_addr().expect("an address");
        let (tunnels, auth) = (
            Arc::new(AtomicUsize::new(0)),
            Arc::new(Mutex::new(Vec::new())),
        );
        let (count, record) = (Arc::clone(&tunnels), Arc::clone(&auth));
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let mut connections = JoinSet::new();
            while let Ok((tcp, _peer)) = listener.accept().await {
                let (count, record) = (Arc::clone(&count), Arc::clone(&record));
                connections.spawn(tunnel_one(tcp, target, count, record));
                while connections.try_join_next().is_some() {}
            }
        });
        Self {
            addr,
            tunnels,
            auth,
            _tasks: tasks,
        }
    }

    /// The spec of this proxy, with credentials when given.
    pub fn spec(&self, credentials: Option<(&str, &str)>) -> String {
        match credentials {
            Some((user, password)) => format!("http://{user}:{password}@{}", self.addr),
            None => format!("http://{}", self.addr),
        }
    }

    /// How many tunnels it opened.
    pub fn tunnels(&self) -> usize {
        self.tunnels.load(Ordering::SeqCst)
    }

    /// The `Proxy-Authorization` of each `CONNECT`, in order.
    pub fn auth(&self) -> Vec<Option<String>> {
        self.auth
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Serves one `CONNECT` of an [`EgressProxy`].
async fn tunnel_one(
    mut tcp: TcpStream,
    target: SocketAddr,
    count: Arc<AtomicUsize>,
    record: Arc<Mutex<Vec<Option<String>>>>,
) {
    let Some(head) = read_head(&mut tcp).await else {
        return;
    };
    if !head.first_line.starts_with("CONNECT ") {
        return;
    }
    record
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(head.header("proxy-authorization").map(str::to_owned));
    let Ok(mut upstream) = TcpStream::connect(target).await else {
        return;
    };
    count.fetch_add(1, Ordering::SeqCst);
    if tcp
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await
        .is_err()
    {
        return;
    }
    let _copied = tokio::io::copy_bidirectional(&mut tcp, &mut upstream).await;
}

// ===== Raw clients =====

/// A response or request head read off a stream.
#[derive(Debug, Clone)]
pub struct Head {
    /// The first line.
    pub first_line: String,
    /// The headers, names lower case, in order.
    pub headers: Vec<(String, String)>,
    /// Bytes read past the head.
    pub rest: Vec<u8>,
}

impl Head {
    /// The status code of a response head.
    pub fn status(&self) -> u16 {
        self.first_line
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("a status line")
    }

    /// The first value of a header.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The `Content-Length`, when present.
    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length")
            .and_then(|value| value.parse().ok())
    }
}

/// Reads one head, or `None` when the stream ends (or errs, or stalls) first.
pub async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> Option<Head> {
    let work = async {
        let mut buf = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            if let Some(end) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                return Some(parse_head(&buf, end));
            }
            if buf.len() > MAX_HEAD {
                return None;
            }
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            buf.extend_from_slice(chunk.get(..read)?);
        }
    };
    tokio::time::timeout(WITHIN, work).await.ok().flatten()
}

/// Splits a head at `end` (the index of its blank line).
fn parse_head(buf: &[u8], end: usize) -> Head {
    let text = String::from_utf8_lossy(&buf[..end]).into_owned();
    let mut lines = text.split("\r\n");
    let first_line = lines.next().unwrap_or_default().to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Head {
        first_line,
        headers,
        rest: buf[end + 4..].to_vec(),
    }
}

/// Reads until the peer closes or `QUIET` passes; `true` when the stream ended.
pub async fn closes<S: AsyncRead + Unpin>(stream: &mut S) -> bool {
    let mut chunk = [0_u8; 1024];
    let reading = async {
        loop {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return true,
                Ok(_) => {}
            }
        }
    };
    tokio::time::timeout(WITHIN, reading).await.unwrap_or(false)
}

/// Opens a TCP connection to the proxy and sends `CONNECT target`, with `auth` as
/// `Proxy-Authorization` when given. Returns the stream and the answer's head.
pub async fn raw_connect(
    proxy: SocketAddr,
    target: &str,
    auth: Option<&str>,
) -> (TcpStream, Option<Head>) {
    let mut tcp = TcpStream::connect(proxy).await.expect("reaches the proxy");
    let mut request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n");
    if let Some(auth) = auth {
        request.push_str(&format!("Proxy-Authorization: {auth}\r\n"));
    }
    request.push_str("\r\n");
    tcp.write_all(request.as_bytes()).await.expect("writes");
    let head = read_head(&mut tcp).await;
    (tcp, head)
}

/// Runs a TLS client handshake over `tcp` offering ALPN `http/1.1`.
///
/// With `root`, the server must present a chain to it for `sni`; without, nothing is verified
/// (the attacker's view, or a test of the server's own checks). `sni` of `None` sends no SNI.
pub async fn tls_client(
    tcp: TcpStream,
    sni: Option<&str>,
    root: Option<&[u8]>,
) -> Result<SslStream<TcpStream>, String> {
    let mut builder = SslConnector::builder(SslMethod::tls()).map_err(|e| e.to_string())?;
    builder
        .set_alpn_protos(b"\x08http/1.1")
        .map_err(|e| e.to_string())?;
    match root {
        Some(der) => {
            let cert = X509::from_der(der).map_err(|e| e.to_string())?;
            builder
                .cert_store_mut()
                .add_cert(cert)
                .map_err(|e| e.to_string())?;
            builder.set_verify(SslVerifyMode::PEER);
        }
        None => builder.set_verify(SslVerifyMode::NONE),
    }
    let mut config = builder.build().configure().map_err(|e| e.to_string())?;
    if root.is_none() {
        config.set_verify_hostname(false);
    }
    if sni.is_none() {
        config.set_use_server_name_indication(false);
    }
    let ssl = config
        .into_ssl(sni.unwrap_or("127.0.0.1"))
        .map_err(|e| e.to_string())?;
    let mut stream = SslStream::new(ssl, tcp).map_err(|e| e.to_string())?;
    tokio::time::timeout(WITHIN, Pin::new(&mut stream).connect())
        .await
        .map_err(|_elapsed| "the handshake stalled".to_owned())?
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

/// A tunnel through the proxy to `host:port` with the TLS the proxy ends, verified against the
/// proxy's CA `ca_der`.
pub async fn tunnel(
    mitm: &MitmProxy,
    host: &str,
    port: u16,
    ca_der: &[u8],
) -> SslStream<TcpStream> {
    let target = format!("{host}:{port}");
    let (tcp, head) = raw_connect(mitm.local_addr(), &target, Some(&proxy_auth(mitm))).await;
    assert_eq!(head.expect("an answer to CONNECT").status(), 200);
    tls_client(tcp, Some(host), Some(ca_der))
        .await
        .expect("the proxy's leaf verifies against its CA")
}

/// Writes a raw HTTP/1.1 request and reads the answer's head and body (`Content-Length` or
/// chunked).
pub async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    request: &str,
) -> Option<(Head, Vec<u8>)> {
    stream.write_all(request.as_bytes()).await.ok()?;
    stream.flush().await.ok()?;
    let head = read_head(stream).await?;
    let chunked = head
        .header("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"));
    let mut raw = head.rest.clone();
    let body = if chunked {
        loop {
            if let Some(body) = dechunk(&raw) {
                break body;
            }
            read_more(stream, &mut raw).await?;
        }
    } else {
        let length = head.content_length().unwrap_or(0);
        while raw.len() < length {
            read_more(stream, &mut raw).await?;
        }
        raw
    };
    Some((head, body))
}

/// Reads what is there into `buf`; `None` at the end of the stream.
async fn read_more<S: AsyncRead + Unpin>(stream: &mut S, buf: &mut Vec<u8>) -> Option<()> {
    let mut chunk = [0_u8; 16 * 1024];
    let read = tokio::time::timeout(WITHIN, stream.read(&mut chunk))
        .await
        .ok()?
        .ok()?;
    if read == 0 {
        return None;
    }
    buf.extend_from_slice(&chunk[..read]);
    Some(())
}

/// The body of a complete chunked encoding, or `None` while it is incomplete.
fn dechunk(raw: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut at = 0;
    loop {
        let line_end = at + raw.get(at..)?.windows(2).position(|pair| pair == b"\r\n")?;
        let size_text = std::str::from_utf8(&raw[at..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        let start = line_end + 2;
        if size == 0 {
            return raw
                .get(start..start + 2)
                .filter(|end| *end == b"\r\n")
                .map(|_| body);
        }
        body.extend_from_slice(raw.get(start..start + size)?);
        at = start + size + 2;
    }
}

/// A request head as Chrome writes a navigation on HTTP/1.1, with `extra` lines after `Host`
/// and `Connection`.
pub fn navigation(host: &str, port: u16, user_agent: &str, extra: &[&str]) -> String {
    let mut lines = vec![
        "GET /page?q=1 HTTP/1.1".to_owned(),
        format!("Host: {host}:{port}"),
        "Connection: keep-alive".to_owned(),
    ];
    lines.extend(extra.iter().map(|line| (*line).to_owned()));
    lines.extend([
        "sec-ch-ua-mobile: ?0".to_owned(),
        "Upgrade-Insecure-Requests: 1".to_owned(),
        format!("User-Agent: {user_agent}"),
        "Accept: text/html".to_owned(),
        "Sec-Fetch-Site: none".to_owned(),
        "Sec-Fetch-Dest: document".to_owned(),
        "Accept-Encoding: gzip, deflate, br, zstd".to_owned(),
        "Accept-Language: en-US,en;q=0.9".to_owned(),
    ]);
    format!("{}\r\n\r\n", lines.join("\r\n"))
}

/// The user agent the measured profile carries.
pub fn measured_user_agent() -> String {
    profile().user_agent().to_owned()
}

/// A wreq client playing the browser: through the proxy, trusting only the proxy's CA,
/// HTTP/1.1 as on Chrome's intercepted leg.
pub fn browser_client(mitm: &MitmProxy, ca_der: &[u8]) -> wreq::Client {
    let token = token(mitm);
    let proxy = wreq::Proxy::all(format!("http://{}", mitm.local_addr()))
        .expect("a proxy")
        .basic_auth("sb", &token);
    wreq::Client::builder()
        .proxy(proxy)
        .tls_cert_store(wreq::tls::trust::CertStore::from_der_certs([ca_der]).expect("a store"))
        .http1_only()
        .build()
        .expect("a client")
}

/// Waits until `check` holds, for up to [`WITHIN`].
pub async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + WITHIN;
    while tokio::time::Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    check()
}
