//! A loopback WebSocket echo site for the MITM proxy tests, on raw tokio: the RFC 6455 opening
//! handshake answered by hand, and frames read and written by hand (masked from a client,
//! unmasked from the site), so a test sees every byte the proxy passes on.
//!
//! An ordinary `GET` gets [`ECHO_PAGE`]; a handshake for [`REFUSED_PATH`] gets a `403`; any other
//! handshake gets `101` and then every data frame back, a ping a pong, and a close a close.
#![allow(
    dead_code,
    reason = "shared by the mock and the real-Chrome test binaries, which each use a part"
)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use btls::ssl::SslAcceptor;
use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use seleniumbase_rs::scrape::runtime::capture::stream::{
    http1_response, read_http1_request, Http1Request,
};
use seleniumbase_rs::wire::client_hello::ClientHello;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use url::Host;

use crate::support::{der, new_ca, peek_hello, WITHIN};

/// The GUID RFC 6455 appends to the key before hashing it.
const ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// What the echo site answers an ordinary `GET` with.
pub const ECHO_PAGE: &[u8] = b"<!doctype html><title>echo</title><p id=marker>echo site</p>";
/// The path whose handshake the site refuses with a `403`.
pub const REFUSED_PATH: &str = "/refuse";
/// The body of that `403`.
pub const REFUSED_BODY: &[u8] = b"no";
/// The largest frame either side reads.
const MAX_FRAME: usize = 16 * 1024 * 1024;
/// The opcode of a text frame.
pub const TEXT: u8 = 0x1;
/// The opcode of a binary frame.
pub const BINARY: u8 = 0x2;
/// The opcode of a close frame.
pub const CLOSE: u8 = 0x8;
/// The opcode of a ping.
pub const PING: u8 = 0x9;
/// The opcode of a pong.
pub const PONG: u8 = 0xA;
/// The mask a test client puts on its frames.
const CLIENT_MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
/// The key of the test handshakes: the sample of RFC 6455, section 1.3.
pub const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

/// What the echo site saw of one handshake it accepted.
#[derive(Debug)]
pub struct WsSeen {
    /// The parsed `ClientHello`.
    pub hello: ClientHello,
    /// The ALPN protocol the handshake settled on.
    pub alpn: Option<Vec<u8>>,
    /// The handshake request, names in the case they were sent.
    pub request: Http1Request,
}

/// What happened at the echo site so far.
#[derive(Debug, Default)]
struct Counts {
    /// TCP connections accepted.
    connections: AtomicUsize,
    /// Handshakes read, accepted or refused.
    handshakes: AtomicUsize,
    /// Bytes received after an accepted handshake, partial frames included.
    received: AtomicUsize,
    /// Accepted WebSocket connections that ended.
    closed: AtomicUsize,
}

/// An HTTPS WebSocket echo site with its own CA, offering ALPN `http/1.1` only.
pub struct EchoOrigin {
    addr: SocketAddr,
    ca: Arc<EphemeralCa>,
    seen: mpsc::Receiver<WsSeen>,
    counts: Arc<Counts>,
    _tasks: JoinSet<()>,
}

impl EchoOrigin {
    /// Starts the site on loopback.
    pub async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let addr = listener.local_addr().expect("an address");
        let ca = new_ca();
        let server = Arc::new(acceptor(&[ALPN_HTTP1]).expect("an acceptor"));
        let counts = Arc::new(Counts::default());
        let (sender, seen) = mpsc::channel(64);
        let mut tasks = JoinSet::new();
        let (site_ca, site_counts) = (Arc::clone(&ca), Arc::clone(&counts));
        tasks.spawn(async move {
            let mut connections = JoinSet::new();
            while let Ok((tcp, _peer)) = listener.accept().await {
                site_counts.connections.fetch_add(1, Ordering::SeqCst);
                let (ca, server, sender, counts) = (
                    Arc::clone(&site_ca),
                    Arc::clone(&server),
                    sender.clone(),
                    Arc::clone(&site_counts),
                );
                connections.spawn(async move {
                    let _served = serve(tcp, &ca, &server, &sender, &counts).await;
                });
                while connections.try_join_next().is_some() {}
            }
        });
        Self {
            addr,
            ca,
            seen,
            counts,
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

    /// The next accepted handshake, or `None` when none comes within `within`.
    pub async fn next(&mut self, within: Duration) -> Option<WsSeen> {
        tokio::time::timeout(within, self.seen.recv())
            .await
            .ok()
            .flatten()
    }

    /// TCP connections the site accepted.
    pub fn connections(&self) -> usize {
        self.counts.connections.load(Ordering::SeqCst)
    }

    /// Handshakes the site read, accepted or refused.
    pub fn handshakes(&self) -> usize {
        self.counts.handshakes.load(Ordering::SeqCst)
    }

    /// Bytes received after accepted handshakes, partial frames included.
    pub fn received(&self) -> usize {
        self.counts.received.load(Ordering::SeqCst)
    }

    /// Accepted WebSocket connections that ended.
    pub fn closed(&self) -> usize {
        self.counts.closed.load(Ordering::SeqCst)
    }
}

/// Serves one connection: TLS, then one request; a handshake it accepts is echoed until it ends.
async fn serve(
    tcp: TcpStream,
    ca: &EphemeralCa,
    server: &SslAcceptor,
    sender: &mpsc::Sender<WsSeen>,
    counts: &Counts,
) -> Result<(), String> {
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
    let request = read_http1_request(&mut tls, WITHIN)
        .await
        .map_err(|e| e.to_string())?;
    let upgrade = request
        .header("upgrade")
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    if !upgrade {
        let reply = http1_response(200, "OK", &[("Content-Type", "text/html")], ECHO_PAGE);
        tls.write_all(&reply).await.map_err(|e| e.to_string())?;
        let _closed = tls.shutdown().await;
        return Ok(());
    }
    counts.handshakes.fetch_add(1, Ordering::SeqCst);
    if request.path() == REFUSED_PATH {
        let reply = http1_response(
            403,
            "Forbidden",
            &[("Content-Type", "text/plain")],
            REFUSED_BODY,
        );
        tls.write_all(&reply).await.map_err(|e| e.to_string())?;
        let _closed = tls.shutdown().await;
        return Ok(());
    }
    let key = request
        .header("sec-websocket-key")
        .ok_or("the handshake carries no key")?;
    let answer = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\r\n",
        accept_for(key)
    );
    tls.write_all(answer.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    tls.flush().await.map_err(|e| e.to_string())?;
    let _sent = sender
        .send(WsSeen {
            hello,
            alpn,
            request,
        })
        .await;
    echo(&mut tls, &counts.received).await;
    counts.closed.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Sends every frame back until the peer closes, fails or sends a close.
async fn echo<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S, received: &AtomicUsize) {
    while let Some((opcode, payload)) = read_frame_counted(stream, Some(received)).await {
        let reply = match opcode {
            PING => frame(PONG, &payload, None),
            other => frame(other, &payload, None),
        };
        if stream.write_all(&reply).await.is_err() || stream.flush().await.is_err() {
            break;
        }
        if opcode == CLOSE {
            break;
        }
    }
    let _closed = stream.shutdown().await;
}

/// The `Sec-WebSocket-Accept` of a key (RFC 6455, section 4.2.2).
pub fn accept_for(key: &str) -> String {
    let hash = digest(
        &SHA1_FOR_LEGACY_USE_ONLY,
        format!("{key}{ACCEPT_GUID}").as_bytes(),
    );
    STANDARD.encode(hash.as_ref())
}

/// A frame as a client writes it: final, masked.
pub fn client_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    frame(opcode, payload, Some(CLIENT_MASK))
}

/// One final frame, masked with `mask` when given.
fn frame(opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut out = vec![0x80 | opcode];
    let mask_bit = if mask.is_some() { 0x80 } else { 0 };
    let len = payload.len();
    if len < 126 {
        out.push(mask_bit | u8::try_from(len).expect("under 126"));
    } else if let Ok(medium) = u16::try_from(len) {
        out.push(mask_bit | 126);
        out.extend_from_slice(&medium.to_be_bytes());
    } else {
        out.push(mask_bit | 127);
        let long = u64::try_from(len).expect("a length fits 64 bits");
        out.extend_from_slice(&long.to_be_bytes());
    }
    match mask {
        Some(mask) => {
            out.extend_from_slice(&mask);
            out.extend(
                payload
                    .iter()
                    .zip(mask.iter().cycle())
                    .map(|(byte, key)| byte ^ key),
            );
        }
        None => out.extend_from_slice(payload),
    }
    out
}

/// Reads one frame: its opcode and its payload, unmasked. `None` when the stream ends or fails
/// first, or the frame is larger than 16 MiB.
pub async fn read_frame<S: AsyncRead + Unpin>(stream: &mut S) -> Option<(u8, Vec<u8>)> {
    tokio::time::timeout(WITHIN, read_frame_counted(stream, None))
        .await
        .ok()
        .flatten()
}

/// [`read_frame`] without a time limit, adding every byte read to `counter`.
async fn read_frame_counted<S: AsyncRead + Unpin>(
    stream: &mut S,
    counter: Option<&AtomicUsize>,
) -> Option<(u8, Vec<u8>)> {
    let head = read_counted(stream, 2, counter).await?;
    let opcode = head[0] & 0x0f;
    let masked = head[1] & 0x80 != 0;
    let len = match head[1] & 0x7f {
        126 => {
            let bytes: [u8; 2] = read_counted(stream, 2, counter).await?.try_into().ok()?;
            usize::from(u16::from_be_bytes(bytes))
        }
        127 => {
            let bytes: [u8; 8] = read_counted(stream, 8, counter).await?.try_into().ok()?;
            usize::try_from(u64::from_be_bytes(bytes)).ok()?
        }
        short => usize::from(short),
    };
    if len > MAX_FRAME {
        return None;
    }
    let mask = if masked {
        Some(read_counted(stream, 4, counter).await?)
    } else {
        None
    };
    let mut payload = read_counted(stream, len, counter).await?;
    if let Some(mask) = mask {
        for (byte, key) in payload.iter_mut().zip(mask.iter().cycle()) {
            *byte ^= key;
        }
    }
    Some((opcode, payload))
}

/// Reads exactly `len` bytes, adding each read to `counter`.
async fn read_counted<S: AsyncRead + Unpin>(
    stream: &mut S,
    len: usize,
    counter: Option<&AtomicUsize>,
) -> Option<Vec<u8>> {
    let mut buf = vec![0_u8; len];
    let mut filled = 0;
    while filled < len {
        let read = stream.read(&mut buf[filled..]).await.ok()?;
        if read == 0 {
            return None;
        }
        if let Some(counter) = counter {
            counter.fetch_add(read, Ordering::SeqCst);
        }
        filled += read;
    }
    Some(buf)
}

/// A handshake as Chrome writes one for `wss://host:port{path}`, with `extra` lines at the end.
pub fn websocket_handshake(
    host: &str,
    port: u16,
    path: &str,
    user_agent: &str,
    extra: &[&str],
) -> String {
    let mut lines = vec![
        format!("GET {path} HTTP/1.1"),
        format!("Host: {host}:{port}"),
        "Connection: Upgrade".to_owned(),
        "Pragma: no-cache".to_owned(),
        "Cache-Control: no-cache".to_owned(),
        format!("User-Agent: {user_agent}"),
        "Upgrade: websocket".to_owned(),
        format!("Origin: https://{host}:{port}"),
        "Sec-WebSocket-Version: 13".to_owned(),
        "Accept-Encoding: gzip, deflate, br, zstd".to_owned(),
        "Accept-Language: en-US,en;q=0.9".to_owned(),
        format!("Sec-WebSocket-Key: {KEY}"),
        "Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits".to_owned(),
    ];
    lines.extend(extra.iter().map(|line| (*line).to_owned()));
    format!("{}\r\n\r\n", lines.join("\r\n"))
}
