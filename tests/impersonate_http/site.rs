//! A loopback HTTPS site for the tests of `HttpSession`: one certificate authority, one leaf per
//! host name the client asks for (the server name picks it), HTTP/1.1 with keep-alive, and a
//! record of every request it saw.
//!
//! The authority is the ephemeral one of `impersonate::tls_server`; the client trusts it through
//! `tls_cert_store`, so certificate verification is on in every test. A handler answers each
//! request from what it saw.

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use seleniumbase_rs::impersonate::capture_h2::site::h2_reply;
use seleniumbase_rs::impersonate::http_session::{HttpOptions, HttpSession};
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::scrape::egress::route::{EgressKey, EgressRef};
use seleniumbase_rs::scrape::passport::identity::{EmulationRef, StealthIdentity};
use seleniumbase_rs::scrape::runtime::capture::stream::read_h2c_preface;
use seleniumbase_rs::stealth::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use seleniumbase_rs::wire::h2_frames::H2Preface;
use seleniumbase_rs::wire::{emulation, BrowserKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Host;
use wreq::tls::trust::CertStore;

/// How long the site waits for a hello.
const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;
/// The seed of the sample fingerprint.
const SEED: u64 = 7;
/// The ALPN id of HTTP/2.
const ALPN_H2: &[u8] = b"h2";

/// A request the site saw.
#[derive(Clone, Debug)]
pub struct Seen {
    /// The host name asked for, without the port.
    pub host: String,
    /// The port of the site, for a handler that names another host of the same site.
    pub port: u16,
    /// The method.
    pub method: String,
    /// The path and query.
    pub target: String,
    /// The headers in the order hyper lists them, names in lower case.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Seen {
    /// The first value of a header.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The `Cookie` header.
    pub fn cookie(&self) -> Option<&str> {
        self.header("cookie")
    }

    /// The path without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }
}

/// What the site answers.
#[derive(Clone, Debug)]
pub struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Duration,
}

impl Answer {
    /// A 200 with a text body.
    pub fn ok(body: &str) -> Self {
        Self::status(200).body(body.as_bytes())
    }

    /// An answer with this status and no body.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            delay: Duration::ZERO,
        }
    }

    /// A redirect.
    pub fn redirect(status: u16, location: &str) -> Self {
        Self::status(status).header("location", location)
    }

    /// Adds a header; `Set-Cookie` may be added more than once.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Sets a `Set-Cookie` header.
    pub fn cookie(self, line: &str) -> Self {
        self.header("set-cookie", line)
    }

    /// Sets the body.
    pub fn body(mut self, body: &[u8]) -> Self {
        self.body = body.to_vec();
        self
    }

    /// Waits before answering.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

type Handler = Arc<dyn Fn(&Seen) -> Answer + Send + Sync>;
type Log = Arc<Mutex<Vec<Seen>>>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The site: a listener on the loopback address and the task that serves it.
pub struct Site {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    seen: Log,
    task: JoinHandle<()>,
}

impl Drop for Site {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Site {
    /// Binds the loopback address on a port the system picks and starts serving.
    pub async fn start(handler: impl Fn(&Seen) -> Answer + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds the loopback address");
        let addr = listener.local_addr().expect("a bound address");
        let ca = Arc::new(EphemeralCa::generate().expect("a CA"));
        let seen: Log = Arc::default();
        let task = tokio::spawn(serve(
            listener,
            addr.port(),
            Arc::clone(&ca),
            Arc::new(acceptor(&[ALPN_HTTP1]).expect("an acceptor")),
            Arc::new(handler),
            Arc::clone(&seen),
        ));
        Self {
            addr,
            ca,
            seen,
            task,
        }
    }

    /// The port the site listens on.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The URL of `path` on `host`, which the client resolves to this site.
    pub fn url(&self, host: &str, path: &str) -> String {
        format!("https://{host}:{}{path}", self.port())
    }

    /// Options that trust this site's authority and resolve each host name to it.
    pub fn options(&self, hosts: &[&str]) -> HttpOptions {
        options_for(&self.ca, self.addr, hosts)
    }

    /// A session for the sample identity against this site.
    pub fn session(&self, hosts: &[&str]) -> HttpSession {
        HttpSession::from_identity(&identity(), None, self.options(hosts)).expect("a session")
    }

    /// Every request seen so far, oldest first.
    pub fn seen(&self) -> Vec<Seen> {
        locked(&self.seen).clone()
    }
}

/// The measured Chrome major version on macOS.
pub fn measured_major() -> u32 {
    emulation::get(BrowserKind::Chrome, OsType::Macos)
        .expect("macOS is measured")
        .major
}

/// The seconds since the Unix epoch, from the system clock.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
}

/// A fingerprint claiming Chrome `major` on `os`.
pub fn fingerprint(os: OsType, major: u32) -> Fingerprint {
    Fingerprint::randomized_with_major(os, SEED, major)
}

/// An identity behind `egress` whose emulation is `emulation` and whose fingerprint is `fp`.
pub fn identity_of(
    fp: Fingerprint,
    emulation: EmulationRef,
    egress: Option<EgressKey>,
) -> StealthIdentity {
    StealthIdentity::new(fp, emulation, egress, now()).expect("a coherent fingerprint")
}

/// The sample identity: the measured Chrome on macOS, going direct, holding no cookie.
pub fn identity() -> StealthIdentity {
    let major = measured_major();
    identity_of(
        fingerprint(OsType::Macos, major),
        EmulationRef {
            kind: BrowserKind::Chrome,
            major,
            os: OsType::Macos,
        },
        None,
    )
}

/// An egress for `spec` that is never connected to.
pub fn egress(spec: &str) -> EgressRef {
    EgressRef::parse(spec, None).expect("a valid egress")
}

async fn serve(
    listener: TcpListener,
    port: u16,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    handler: Handler,
    seen: Log,
) {
    while let Ok((tcp, _peer)) = listener.accept().await {
        tokio::spawn(connection(
            tcp,
            port,
            Arc::clone(&ca),
            Arc::clone(&tls),
            Arc::clone(&handler),
            Arc::clone(&seen),
        ));
    }
}

async fn connection(
    tcp: TcpStream,
    port: u16,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    handler: Handler,
    seen: Log,
) {
    let Some(stream) = tls_accept(tcp, &ca, &tls).await else {
        return;
    };
    let service =
        service_fn(move |request| respond(request, port, Arc::clone(&handler), Arc::clone(&seen)));
    // A client that goes away mid-request ends the connection; there is nobody to tell.
    let _ended = http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

async fn respond(
    request: Request<hyper::body::Incoming>,
    port: u16,
    handler: Handler,
    seen: Log,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, body) = request.into_parts();
    let body = body
        .collect()
        .await
        .map(http_body_util::Collected::to_bytes)
        .unwrap_or_default();
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let host = headers
        .iter()
        .find(|(name, _)| name == "host")
        .map(|(_, value)| value.split(':').next().unwrap_or_default().to_owned())
        .unwrap_or_default();
    let record = Seen {
        host,
        port,
        method: parts.method.to_string(),
        target: parts
            .uri
            .path_and_query()
            .map_or_else(String::new, ToString::to_string),
        headers,
        body: body.to_vec(),
    };
    let answer = handler(&record);
    locked(&seen).push(record);
    if !answer.delay.is_zero() {
        tokio::time::sleep(answer.delay).await;
    }
    let mut response = Response::builder().status(answer.status);
    for (name, value) in &answer.headers {
        response = response.header(name.as_str(), value.as_str());
    }
    Ok(response
        .body(Full::new(Bytes::from(answer.body)))
        .expect("a valid response"))
}

/// Options that trust `ca` and resolve each host name to `addr`.
fn options_for(ca: &EphemeralCa, addr: SocketAddr, hosts: &[&str]) -> HttpOptions {
    let der = ca.certificate().to_der().expect("the CA as DER");
    let store = CertStore::from_der_certs([der.as_slice()]).expect("a store");
    hosts
        .iter()
        .fold(HttpOptions::new().trust(store), |options, host| {
            options.resolve(*host, addr)
        })
}

/// Completes the TLS handshake of `tcp` with the leaf of the host the client asked for.
async fn tls_accept(
    tcp: TcpStream,
    ca: &EphemeralCa,
    tls: &btls::ssl::SslAcceptor,
) -> Option<tokio_btls::SslStream<TcpStream>> {
    let hello = peek_hello(&tcp).await.ok()?;
    let host = Host::parse(hello.sni.as_deref()?).ok()?;
    let leaf = ca.leaf_for(&host, SystemTime::now()).ok()?;
    accept(tls, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT)
        .await
        .ok()
}

/// A site that speaks HTTP/2 only and records the start of each connection it serves.
pub struct H2Site {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    prefaces: Arc<Mutex<Vec<H2Preface>>>,
    task: JoinHandle<()>,
}

impl Drop for H2Site {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl H2Site {
    /// Starts serving `body` to every request.
    pub async fn start(body: &'static str) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds the loopback address");
        let addr = listener.local_addr().expect("a bound address");
        let ca = Arc::new(EphemeralCa::generate().expect("a CA"));
        let prefaces: Arc<Mutex<Vec<H2Preface>>> = Arc::default();
        let tls = Arc::new(acceptor(&[ALPN_H2]).expect("an acceptor"));
        let task = tokio::spawn(serve_h2(
            listener,
            Arc::clone(&ca),
            tls,
            body,
            Arc::clone(&prefaces),
        ));
        Self {
            addr,
            ca,
            prefaces,
            task,
        }
    }

    /// The URL of `path` on `host`.
    pub fn url(&self, host: &str, path: &str) -> String {
        format!("https://{host}:{}{path}", self.addr.port())
    }

    /// Options that trust this site's authority and resolve each host name to it.
    pub fn options(&self, hosts: &[&str]) -> HttpOptions {
        options_for(&self.ca, self.addr, hosts)
    }

    /// The start of every connection served, oldest first.
    pub fn prefaces(&self) -> Vec<H2Preface> {
        locked(&self.prefaces).clone()
    }
}

async fn serve_h2(
    listener: TcpListener,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    body: &'static str,
    prefaces: Arc<Mutex<Vec<H2Preface>>>,
) {
    while let Ok((tcp, _peer)) = listener.accept().await {
        tokio::spawn(connection_h2(
            tcp,
            Arc::clone(&ca),
            Arc::clone(&tls),
            body,
            Arc::clone(&prefaces),
        ));
    }
}

async fn connection_h2(
    tcp: TcpStream,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    body: &'static str,
    prefaces: Arc<Mutex<Vec<H2Preface>>>,
) {
    let Some(mut stream) = tls_accept(tcp, &ca, &tls).await else {
        return;
    };
    let Ok((_raw, preface)) = read_h2c_preface(&mut stream, HELLO_WITHIN).await else {
        return;
    };
    let Ok(reply) = h2_reply(preface.request_stream_id, Some(body.as_bytes())) else {
        return;
    };
    locked(&prefaces).push(preface);
    if stream.write_all(&reply).await.is_err() || stream.flush().await.is_err() {
        return;
    }
    // Hold the connection open until the client closes it.
    let mut sink = [0_u8; 1024];
    while matches!(stream.read(&mut sink).await, Ok(read) if read > 0) {}
}

/// Reads and parses the `ClientHello` waiting on `tcp`, leaving the bytes for the handshake.
async fn peek_hello(tcp: &TcpStream) -> Result<ClientHello, String> {
    let work = async {
        let mut buf = vec![0_u8; MAX_CLIENT_HELLO + PEEK_SLACK];
        loop {
            let read = tcp.peek(&mut buf).await.map_err(|e| e.to_string())?;
            if read == 0 {
                return Err("the client closed before a hello".to_owned());
            }
            let prefix = buf.get(..read).unwrap_or_default();
            let needed = ClientHello::needed_len(prefix).map_err(|e| e.to_string())?;
            if let Some(end) = needed.filter(|&end| end <= read) {
                let records = prefix.get(..end).unwrap_or_default();
                return ClientHello::parse(records).map_err(|e| e.to_string());
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    };
    tokio::time::timeout(HELLO_WITHIN, work)
        .await
        .map_err(|_elapsed| "no ClientHello in time".to_owned())?
}
