//! A loopback HTTPS shop that plays a site behind a script challenge, for the examples that need
//! a site to talk to.
//!
//! The shop listens on `127.0.0.1` only and takes its certificates from an in-memory
//! [`EphemeralCa`]. Its clients trust that authority through the options this module builds, so
//! certificate verification stays on, and they resolve the shop's made-up name to the loopback
//! address, so nothing leaves the machine.
//!
//! The pages:
//!
//! - `/open` is a page nobody guards.
//! - `/` is guarded: it answers with a script challenge until the visitor holds the clearance
//!   cookie, and with the real page once it does.
//! - `/__solve` hands out the clearance cookie. The challenge page's script calls it, as the
//!   script of a real challenge earns its cookie.
//!
//! This is a stand-in written for the examples. It is not any vendor's challenge, and nothing
//! about it says how a real one behaves.
//!
//! Every request is recorded together with the JA4 of the `ClientHello` of its connection, so an
//! example can show what a site sees of the client's handshake.

use std::convert::Infallible;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::impersonate::{HttpOptions, MitmOptions};
use seleniumbase_rs::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use seleniumbase_rs::wire::ja4::Ja4;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Host;
use wreq::tls::trust::CertStore;

/// The made-up name of the shop. Clients resolve it to the loopback address of the shop.
pub const HOST: &str = "origin.test";
/// The cookie that proves the challenge was passed.
pub const CLEARANCE: &str = "cf_clearance";
/// What the real page says.
pub const REAL_MARKER: &str = "the real page";
/// The value of the clearance cookie. It is made up for the example and opens nothing.
const CLEARANCE_VALUE: &str = "example-clearance-value";
/// How long the clearance lasts, in seconds.
const CLEARANCE_TTL_SECS: u64 = 600;
/// The real page.
const REAL_HTML: &str = "<!doctype html><title>shop</title><p id=real>the real page</p>";
/// The challenge page: Cloudflare's title and options object, and a script that earns the cookie
/// and reloads, the way the browser's own challenge does. The script waits half a second first, as
/// the computation of a real challenge takes a moment. A scraper learns the clearance value it
/// had before the solve, and waits for a new one, so a cookie set before it looked would not
/// count as new.
const CHALLENGE_HTML: &str = "<!doctype html><html><head><title>Just a moment...</title></head>\
    <body><script>window._cf_chl_opt = { cType: 'managed' };\
    setTimeout(function () {\
        fetch('/__solve').then(function () { location.reload(); });\
    }, 500);</script></body></html>";
/// How long the shop waits for a `ClientHello`.
const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;

/// One request the shop saw.
#[derive(Clone, Debug)]
pub struct Seen {
    /// The path, without the query.
    pub path: String,
    /// The `Cookie` header, when the request had one.
    cookie: Option<String>,
    /// The `User-Agent` header, when the request had one.
    pub user_agent: Option<String>,
    /// The JA4 of the `ClientHello` of the connection the request came on.
    pub ja4: String,
}

impl Seen {
    /// The names of the cookies the request carried. The values are credentials, so an example
    /// prints the names only.
    pub fn cookie_names(&self) -> Vec<&str> {
        self.cookie
            .as_deref()
            .into_iter()
            .flat_map(|header| header.split("; "))
            .filter_map(|pair| pair.split_once('=').map(|(name, _value)| name))
            .collect()
    }

    /// Whether the request carried the clearance.
    fn has_clearance(&self) -> bool {
        self.cookie
            .as_deref()
            .is_some_and(|header| header.contains(&format!("{CLEARANCE}={CLEARANCE_VALUE}")))
    }
}

/// The record of the requests, shared with the tasks that serve them.
type Log = Arc<Mutex<Vec<Seen>>>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The shop: a listener on the loopback address and the task that serves it.
pub struct Shop {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    log: Log,
    task: JoinHandle<()>,
}

impl Drop for Shop {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Shop {
    /// Binds the loopback address on a port the system picks and starts serving.
    pub async fn start() -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let ca = Arc::new(EphemeralCa::generate()?);
        let tls = Arc::new(acceptor(&[ALPN_HTTP1])?);
        let log = Log::default();
        let task = tokio::spawn(serve(listener, Arc::clone(&ca), tls, Arc::clone(&log)));
        Ok(Self {
            addr,
            ca,
            log,
            task,
        })
    }

    /// The URL of `path` on the shop.
    pub fn url(&self, path: &str) -> String {
        format!("https://{HOST}:{}{path}", self.addr.port())
    }

    /// The options of an HTTP session that trusts the shop's authority and resolves the shop's
    /// name to it. Certificate verification stays on.
    pub fn http_options(&self) -> Result<HttpOptions, Box<dyn Error>> {
        let der = self.ca.certificate().to_der()?;
        let store = CertStore::from_der_certs([der.as_slice()])?;
        Ok(HttpOptions::new().trust(store).resolve(HOST, self.addr))
    }

    /// The options of a MITM proxy whose upstream trusts the shop's authority and resolves the
    /// shop's name to it. The proxy refuses loopback destinations unless told otherwise, so this
    /// allows them: the shop is on the loopback address.
    pub fn mitm_options(&self) -> Result<MitmOptions, Box<dyn Error>> {
        let der = self.ca.certificate().to_der()?;
        Ok(MitmOptions::default()
            .trust_upstream_root(der)
            .resolve(HOST, self.addr)
            .allow_private_destinations(true))
    }

    /// Every request seen so far, oldest first.
    pub fn seen(&self) -> Vec<Seen> {
        locked(&self.log).clone()
    }
}

async fn serve(
    listener: TcpListener,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    log: Log,
) {
    while let Ok((tcp, _peer)) = listener.accept().await {
        tokio::spawn(connection(
            tcp,
            Arc::clone(&ca),
            Arc::clone(&tls),
            Arc::clone(&log),
        ));
    }
}

async fn connection(
    tcp: TcpStream,
    ca: Arc<EphemeralCa>,
    tls: Arc<btls::ssl::SslAcceptor>,
    log: Log,
) {
    let Some((stream, ja4)) = tls_accept(tcp, &ca, &tls).await else {
        return;
    };
    let service = service_fn(move |request| respond(request, ja4.clone(), Arc::clone(&log)));
    // A client that goes away mid-request ends the connection; there is nobody to tell.
    let _ended = http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

async fn respond(
    request: Request<hyper::body::Incoming>,
    ja4: String,
    log: Log,
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
    let answer = answer(&seen);
    locked(&log).push(seen);
    Ok(answer)
}

/// What the shop says to `seen`.
fn answer(seen: &Seen) -> Response<Full<Bytes>> {
    let builder = Response::builder();
    let (builder, body) = match seen.path.as_str() {
        "/open" => (builder.status(200), REAL_HTML),
        "/__solve" => (
            builder.status(200).header(
                "set-cookie",
                format!(
                    "{CLEARANCE}={CLEARANCE_VALUE}; Path=/; Max-Age={CLEARANCE_TTL_SECS}; \
                     Secure; HttpOnly"
                ),
            ),
            "ok",
        ),
        _ if seen.has_clearance() => (builder.status(200), REAL_HTML),
        _ => (
            builder.status(403).header("cf-mitigated", "challenge"),
            CHALLENGE_HTML,
        ),
    };
    builder
        .header("content-type", "text/html")
        .body(Full::new(Bytes::from(body)))
        .unwrap_or_default()
}

/// Completes the TLS handshake of `tcp` with the leaf of the host the client asked for, and
/// returns the stream with the JA4 of the hello. A connection that does not finish a handshake
/// is dropped.
async fn tls_accept(
    tcp: TcpStream,
    ca: &EphemeralCa,
    tls: &btls::ssl::SslAcceptor,
) -> Option<(tokio_btls::SslStream<TcpStream>, String)> {
    let hello = peek_hello(&tcp).await.ok()?;
    let host = Host::parse(hello.sni.as_deref()?).ok()?;
    let leaf = ca.leaf_for(&host, SystemTime::now()).ok()?;
    let stream = accept(tls, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT)
        .await
        .ok()?;
    Some((stream, Ja4::of(&hello).to_string()))
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
