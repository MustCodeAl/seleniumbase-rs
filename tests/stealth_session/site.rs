//! A loopback HTTPS site that plays a bot-protected shop, for the tests of `StealthSession`.
//!
//! The shop answers `/` with a script challenge until the visitor holds the clearance cookie, and
//! gives the cookie to whoever runs the challenge page's script (`/__solve`). `/open` is a page
//! nobody guards, `/limited` asks the visitor to slow down and `/blocked` refuses outright. The
//! authority is the ephemeral one of `impersonate::tls_server`; clients trust it through the
//! options this file builds, so certificate verification is on in every test.
//!
//! Every request is recorded with the JA4 of the `ClientHello` of its connection, so a test can
//! check that both legs of a session look alike to the origin.
#![allow(
    dead_code,
    reason = "shared by the scripted and the real-Chrome test binaries, which each use a part"
)]

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use seleniumbase_rs::impersonate::http_session::HttpOptions;
use seleniumbase_rs::impersonate::mitm::MitmOptions;
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use seleniumbase_rs::wire::ja4::Ja4;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Host;
use wreq::tls::trust::CertStore;

/// The name of the shop; the clients resolve it to the loopback address of the site.
pub const HOST: &str = "origin.test";
/// The cookie that proves the challenge was passed.
pub const CLEARANCE: &str = "cf_clearance";
/// What the real page says.
pub const REAL_MARKER: &str = "the real page";
/// The real page.
pub const REAL_HTML: &str = "<!doctype html><title>shop</title><p id=real>the real page</p>";
/// The challenge page: Cloudflare's title and options object, and a script that earns the cookie
/// and reloads, as the browser's own challenge does.
pub const CHALLENGE_HTML: &str =
    "<!doctype html><html><head><title>Just a moment...</title></head>\
    <body><script>window._cf_chl_opt = { cType: 'managed' };\
    fetch('/__solve').then(function () { location.reload(); });</script></body></html>";
/// The block page.
const BLOCK_HTML: &str =
    "<!doctype html><html><head><title>Attention Required! | Cloudflare</title></head></html>";
/// How long the clearance the shop gives lasts, in seconds.
pub const DEFAULT_TTL_SECS: u64 = 600;
/// How long the site waits for a hello.
const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;
/// The wait the rate-limit page asks for, in seconds.
const RETRY_AFTER_SECS: u64 = 120;

/// A request the site saw.
#[derive(Clone, Debug)]
pub struct Seen {
    /// The path without the query.
    pub path: String,
    /// The `Cookie` header.
    pub cookie: Option<String>,
    /// The `User-Agent` header.
    pub user_agent: Option<String>,
    /// The JA4 of the `ClientHello` of the connection the request came on.
    pub ja4: Ja4,
}

impl Seen {
    /// Whether the request carried the clearance `token`.
    pub fn carried(&self, token: &str) -> bool {
        self.cookie
            .as_deref()
            .is_some_and(|cookie| cookie.contains(&format!("{CLEARANCE}={token}")))
    }
}

/// What the site shares with the tasks that serve it.
struct Shared {
    token: Mutex<String>,
    issued: Mutex<u64>,
    log: Mutex<Vec<Seen>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The site: a listener on the loopback address and the task that serves it.
pub struct ChallengeSite {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for ChallengeSite {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ChallengeSite {
    /// Binds the loopback address on a port the system picks and starts serving.
    pub async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds the loopback address");
        let addr = listener.local_addr().expect("a bound address");
        let ca = Arc::new(EphemeralCa::generate().expect("a CA"));
        let shared = Arc::new(Shared {
            token: Mutex::new(token_number(1)),
            issued: Mutex::new(1),
            log: Mutex::new(Vec::new()),
        });
        let tls = Arc::new(acceptor(&[ALPN_HTTP1]).expect("an acceptor"));
        let task = tokio::spawn(serve(listener, Arc::clone(&ca), tls, Arc::clone(&shared)));
        Self {
            addr,
            ca,
            shared,
            task,
        }
    }

    /// The URL of `path` on the shop.
    pub fn url(&self, path: &str) -> String {
        format!("https://{HOST}:{}{path}", self.addr.port())
    }

    /// The options of an HTTP session that trusts this site's authority and resolves the shop's
    /// name to it.
    pub fn http_options(&self) -> HttpOptions {
        let der = self.ca.certificate().to_der().expect("the CA as DER");
        let store = CertStore::from_der_certs([der.as_slice()]).expect("a store");
        HttpOptions::new().trust(store).resolve(HOST, self.addr)
    }

    /// The options of a MITM proxy whose upstream trusts this site's authority and resolves the
    /// shop's name to it.
    pub fn mitm_options(&self) -> MitmOptions {
        let der = self.ca.certificate().to_der().expect("the CA as DER");
        MitmOptions::default()
            .trust_upstream_root(der)
            .resolve(HOST, self.addr)
    }

    /// The value of the clearance the site accepts now.
    pub fn token(&self) -> String {
        locked(&self.shared.token).clone()
    }

    /// A function that tells the clearance the site accepts at the moment it is called.
    pub fn token_source(&self) -> Arc<dyn Fn() -> String + Send + Sync> {
        let shared = Arc::clone(&self.shared);
        Arc::new(move || locked(&shared.token).clone())
    }

    /// Makes every clearance given so far worthless: the site accepts a new value from now on.
    pub fn revoke(&self) {
        let mut issued = locked(&self.shared.issued);
        *issued += 1;
        *locked(&self.shared.token) = token_number(*issued);
    }

    /// Every request seen so far, oldest first.
    pub fn seen(&self) -> Vec<Seen> {
        locked(&self.shared.log).clone()
    }

    /// The requests seen so far for `path`, oldest first.
    pub fn seen_at(&self, path: &str) -> Vec<Seen> {
        self.seen()
            .into_iter()
            .filter(|seen| seen.path == path)
            .collect()
    }
}

/// The `n`th clearance value.
fn token_number(n: u64) -> String {
    format!("clearance-token-{n}")
}

async fn serve(
    listener: TcpListener,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    shared: Arc<Shared>,
) {
    while let Ok((tcp, _peer)) = listener.accept().await {
        tokio::spawn(connection(
            tcp,
            Arc::clone(&ca),
            Arc::clone(&tls),
            Arc::clone(&shared),
        ));
    }
}

async fn connection(
    tcp: TcpStream,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    shared: Arc<Shared>,
) {
    let Some((stream, ja4)) = tls_accept(tcp, &ca, &tls).await else {
        return;
    };
    let service = service_fn(move |request| respond(request, ja4.clone(), Arc::clone(&shared)));
    // A client that goes away mid-request ends the connection; there is nobody to tell.
    let _ended = http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

async fn respond(
    request: Request<hyper::body::Incoming>,
    ja4: Ja4,
    shared: Arc<Shared>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
    };
    let seen = Seen {
        path: request.uri().path().to_owned(),
        cookie: header("cookie"),
        user_agent: header("user-agent"),
        ja4,
    };
    let answer = answer(&seen, &shared);
    locked(&shared.log).push(seen);
    Ok(answer)
}

/// What the shop says to `seen`.
fn answer(seen: &Seen, shared: &Shared) -> Response<Full<Bytes>> {
    let token = locked(&shared.token).clone();
    let builder = Response::builder();
    let (builder, body) = match seen.path.as_str() {
        "/open" => (builder.status(200), REAL_HTML),
        "/__solve" => (
            builder.status(200).header(
                "set-cookie",
                format!(
                    "{CLEARANCE}={token}; Path=/; Max-Age={DEFAULT_TTL_SECS}; Secure; HttpOnly"
                ),
            ),
            "ok",
        ),
        "/limited" => (
            builder
                .status(429)
                .header("retry-after", RETRY_AFTER_SECS.to_string()),
            "slow down",
        ),
        "/blocked" => (
            builder.status(403).header("cf-ray", "8a1b2c3d4e5f6071-AMS"),
            BLOCK_HTML,
        ),
        _ if seen.carried(&token) => (builder.status(200), REAL_HTML),
        _ => (
            builder.status(403).header("cf-mitigated", "challenge"),
            CHALLENGE_HTML,
        ),
    };
    builder
        .header("content-type", "text/html")
        .body(Full::new(Bytes::from(body)))
        .expect("a valid response")
}

/// Completes the TLS handshake of `tcp` with the leaf of the host the client asked for, and
/// returns the stream with the JA4 of the hello.
async fn tls_accept(
    tcp: TcpStream,
    ca: &EphemeralCa,
    tls: &btls::ssl::SslAcceptor,
) -> Option<(tokio_btls::SslStream<TcpStream>, Ja4)> {
    let hello = peek_hello(&tcp).await.ok()?;
    let host = Host::parse(hello.sni.as_deref()?).ok()?;
    let leaf = ca.leaf_for(&host, SystemTime::now()).ok()?;
    let stream = accept(tls, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT)
        .await
        .ok()?;
    Some((stream, Ja4::of(&hello)))
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
    tokio::time::timeout(HELLO_WITHIN, work)
        .await
        .map_err(|_elapsed| "no hello in time".to_owned())?
}
