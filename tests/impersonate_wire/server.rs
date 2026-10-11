//! A loopback HTTPS server that records what the client sent: its `ClientHello` and the first
//! request, over HTTP/2 or HTTP/1.1, whichever the handshake settles on.
//!
//! It is the WP10 TLS stack (`EphemeralCa`, `acceptor`, `accept`) with the WP6/WP7 readers
//! (`ClientHello`, `read_h2c_preface`, `read_http1_request`), so the proof of the client is made
//! with our own parsers, never with the client's word.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime};

use seleniumbase_rs::impersonate::capture_h2::site::h2_reply;
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::scrape::runtime::capture::stream::{
    http1_response, read_h2c_preface, read_http1_request, Http1Request,
};
use seleniumbase_rs::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use seleniumbase_rs::wire::h2_frames::H2Preface;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Host;
use wreq::tls::trust::CertStore;

/// The ALPN id of HTTP/2.
const ALPN_H2: &[u8] = b"h2";
/// The host name every test connects to; the client resolves it to the loopback address.
pub const HOST: &str = "wire.test";
/// How long a connection may take to send its hello and its first request.
const WITHIN: Duration = Duration::from_secs(10);
/// How long the server keeps reading after its answer, so the client is not reset mid-response.
const DRAIN: Duration = Duration::from_millis(200);
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;

/// The first request on a connection.
#[derive(Debug)]
pub enum Request {
    /// An HTTP/2 connection start, with the bytes it was decoded from.
    H2 {
        /// The bytes from the connection preface to the end of the first header block.
        #[allow(dead_code, reason = "kept so a failing test can print the raw frames")]
        raw: Vec<u8>,
        /// What the frames say.
        preface: H2Preface,
    },
    /// An HTTP/1 request.
    Http1(Http1Request),
}

/// What a connection showed.
#[derive(Debug)]
pub struct Served {
    /// The parsed `ClientHello`.
    pub hello: ClientHello,
    /// The ALPN protocol the handshake settled on.
    pub alpn: Option<Vec<u8>>,
    /// The first request.
    pub request: Request,
}

impl Served {
    /// The HTTP/2 preface, panicking with a message when the connection was HTTP/1.
    pub fn h2(&self) -> &H2Preface {
        match &self.request {
            Request::H2 { preface, .. } => preface,
            Request::Http1(_) => panic!("the connection settled on HTTP/1.1, not HTTP/2"),
        }
    }

    /// The HTTP/1 request, panicking with a message when the connection was HTTP/2.
    pub fn http1(&self) -> &Http1Request {
        match &self.request {
            Request::Http1(request) => request,
            Request::H2 { .. } => panic!("the connection settled on HTTP/2, not HTTP/1.1"),
        }
    }
}

/// A server with its own certificate authority, bound to the loopback address.
pub struct WireServer {
    listener: TcpListener,
    addr: SocketAddr,
    ca: EphemeralCa,
    acceptor: btls::ssl::SslAcceptor,
}

impl WireServer {
    /// Binds `127.0.0.1` on a port the system picks and creates a CA for this server alone.
    pub async fn bind() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds the loopback address");
        let addr = listener.local_addr().expect("a bound address");
        Self {
            listener,
            addr,
            ca: EphemeralCa::generate().expect("a CA"),
            acceptor: acceptor(&[ALPN_H2, ALPN_HTTP1]).expect("an acceptor"),
        }
    }

    /// The address the server listens on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The URL of the server's root.
    pub fn url(&self) -> String {
        format!("https://{HOST}:{}/", self.addr.port())
    }

    /// A trust store holding only this server's CA, for `tls_cert_store`.
    pub fn trust(&self) -> CertStore {
        let der = self.ca.certificate().to_der().expect("the CA as DER");
        CertStore::from_der_certs([der.as_slice()]).expect("a store")
    }

    /// Accepts one connection, completes its handshake, reads its first request, answers it
    /// and returns what was seen.
    pub async fn serve_one(&self) -> Result<Served, String> {
        let (tcp, _peer) = self.listener.accept().await.map_err(|e| e.to_string())?;
        let hello = peek_hello(&tcp).await?;
        if hello.sni.as_deref() != Some(HOST) {
            return Err(format!("unexpected server name {:?}", hello.sni));
        }
        let host = Host::parse(HOST).map_err(|e| e.to_string())?;
        let leaf = self
            .ca
            .leaf_for(&host, SystemTime::now())
            .map_err(|e| e.to_string())?;
        let mut tls = accept(&self.acceptor, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT)
            .await
            .map_err(|e| e.to_string())?;
        let alpn = tls.ssl().selected_alpn_protocol().map(<[u8]>::to_vec);
        let request = if alpn.as_deref() == Some(ALPN_H2) {
            let (raw, preface) = read_h2c_preface(&mut tls, WITHIN)
                .await
                .map_err(|e| e.to_string())?;
            let reply = h2_reply(preface.request_stream_id, None).map_err(|e| e.to_string())?;
            tls.write_all(&reply).await.map_err(|e| e.to_string())?;
            Request::H2 { raw, preface }
        } else {
            let request = read_http1_request(&mut tls, WITHIN)
                .await
                .map_err(|e| e.to_string())?;
            let reply = http1_response(200, "OK", &[], &[]);
            tls.write_all(&reply).await.map_err(|e| e.to_string())?;
            Request::Http1(request)
        };
        tls.flush().await.map_err(|e| e.to_string())?;
        drain(&mut tls).await;
        Ok(Served {
            hello,
            alpn,
            request,
        })
    }
}

/// Reads and parses the `ClientHello` waiting on `tcp`, leaving the bytes for the handshake.
async fn peek_hello(tcp: &TcpStream) -> Result<ClientHello, String> {
    let work = async {
        let mut buf = vec![0_u8; MAX_CLIENT_HELLO + PEEK_SLACK];
        loop {
            let seen = tcp.peek(&mut buf).await.map_err(|e| e.to_string())?;
            if seen == 0 {
                return Err("the client closed before a hello".to_owned());
            }
            let prefix = buf.get(..seen).unwrap_or_default();
            let needed = ClientHello::needed_len(prefix).map_err(|e| e.to_string())?;
            if let Some(end) = needed.filter(|&end| end <= seen) {
                let records = prefix.get(..end).unwrap_or_default();
                return ClientHello::parse(records).map_err(|e| e.to_string());
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    };
    tokio::time::timeout(WITHIN, work)
        .await
        .map_err(|_elapsed| "no ClientHello in time".to_owned())?
}

/// Reads and drops what the client still sends for a moment, then closes.
async fn drain<S>(tls: &mut S)
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut sink = [0_u8; 1024];
    let reading = async {
        while let Ok(read) = tls.read(&mut sink).await {
            if read == 0 {
                break;
            }
        }
    };
    let _timed_out = tokio::time::timeout(DRAIN, reading).await;
    let _closed = tls.shutdown().await;
}
