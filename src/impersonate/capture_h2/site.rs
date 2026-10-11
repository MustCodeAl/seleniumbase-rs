//! The local HTTPS site the browser connects to, and the record of the first request on each connection.
//!
//! The site listens on a loopback port. For each connection it reads the `ClientHello` without
//! consuming it (`peek`), takes the server name from it, has the ephemeral CA issue a leaf for
//! exactly that name, and runs the handshake with ALPN fixed to the protocol being measured. Then
//! it reads the first request (HTTP/2 frames through the HPACK decoder, or an HTTP/1 request head)
//! and answers it with a small response, so the page the browser loads is complete.
//!
//! Only `*.capture.test` names get a certificate. A name outside the probe table is dropped
//! before any certificate exists, so the CA never signs for a host the tool did not choose.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use btls::ssl::SslAcceptor;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use url::Host;

use super::probes::{Probe, DOCUMENT, DOCUMENT_BODY};
use super::CaptureH2Error;
use crate::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};
use crate::scrape::runtime::capture::stream::{
    http1_response, read_h2c_preface, read_http1_request, Http1Request,
};
use crate::scrape::runtime::capture::CaptureError;
use crate::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use crate::wire::h2_frames::{encode_frame, frame_type, H2Preface};

/// The ALPN id of HTTP/2.
pub const ALPN_H2: &[u8] = b"h2";
/// Extra bytes `peek` may hold beyond the largest hello.
const PEEK_SLACK: usize = 1024;
/// How long to wait before looking again when a hello has not grown.
const PEEK_RETRY: Duration = Duration::from_millis(5);
/// How long a connection is read after its response, so the browser is not reset mid-response.
const DRAIN_FOR: Duration = Duration::from_millis(250);
/// Bytes read per call while draining.
const DRAIN_CHUNK: usize = 1024;
/// HEADERS and DATA flag: no more frames follow on the stream.
const FLAG_END_STREAM: u8 = 0x1;
/// SETTINGS flag: an acknowledgement.
const FLAG_ACK: u8 = 0x1;
/// HEADERS flag: the header block is complete.
const FLAG_END_HEADERS: u8 = 0x4;
/// The HPACK byte for the static table entry `:status 200`.
const HPACK_STATUS_200: u8 = 0x88;
/// The HPACK prefix of a literal `content-type` field without indexing: static name index 31.
const HPACK_CONTENT_TYPE: [u8; 2] = [0x0f, 0x10];
/// The `Content-Type` of the document the navigation is answered with.
const HTML_TYPE: &str = "text/html; charset=utf-8";

/// Which protocol a site speaks; the ALPN list is fixed to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// HTTP/2 over TLS (ALPN `h2`).
    H2,
    /// HTTP/1.1 over TLS (ALPN `http/1.1`).
    Http1,
}

impl Protocol {
    /// The ALPN id the site accepts.
    #[must_use]
    pub fn alpn(self) -> &'static [u8] {
        match self {
            Protocol::H2 => ALPN_H2,
            Protocol::Http1 => ALPN_HTTP1,
        }
    }
}

/// What the first request on a connection looked like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// An HTTP/2 connection start: the bytes read and what the frames say.
    H2 {
        /// The bytes from the connection preface to the end of the first request's header block.
        raw: Vec<u8>,
        /// The decoded frames.
        preface: H2Preface,
    },
    /// An HTTP/1 request with its headers in wire order.
    Http1 {
        /// The request.
        request: Http1Request,
    },
}

/// A connection read to the end of its first request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// The probe whose host name the browser asked for.
    pub probe: Probe,
    /// The first request.
    pub seen: Seen,
}

/// What every connection task needs.
struct Shared {
    ca: Arc<EphemeralCa>,
    acceptor: SslAcceptor,
    protocol: Protocol,
    timeout: Duration,
}

/// A loopback HTTPS site that records the first request on each connection.
pub struct Site {
    listener: TcpListener,
    addr: SocketAddr,
    shared: Arc<Shared>,
}

impl fmt::Debug for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Site")
            .field("addr", &self.addr)
            .field("protocol", &self.shared.protocol)
            .finish_non_exhaustive()
    }
}

impl Site {
    /// Binds `127.0.0.1` on a port the system picks and creates a CA for this site alone.
    ///
    /// `timeout` bounds how long each connection may take to send its hello and its first request.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureH2Error`] when the socket cannot be bound or `BoringSSL` refuses to set
    /// up the CA or the acceptor.
    pub async fn bind(protocol: Protocol, timeout: Duration) -> Result<Self, CaptureH2Error> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(CaptureError::from)?;
        let addr = listener.local_addr().map_err(CaptureError::from)?;
        let shared = Shared {
            ca: Arc::new(EphemeralCa::generate()?),
            acceptor: acceptor(&[protocol.alpn()])?,
            protocol,
            timeout,
        };
        Ok(Self {
            listener,
            addr,
            shared: Arc::new(shared),
        })
    }

    /// The port the site listens on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The Chrome switch that trusts this site's CA and its leaves, and nothing else.
    #[must_use]
    pub fn chrome_arg(&self) -> String {
        self.shared.ca.chrome_spki_arg()
    }

    /// The site's certificate authority, for a test client that must trust it.
    #[must_use]
    pub fn ca(&self) -> &EphemeralCa {
        &self.shared.ca
    }

    /// Accepts connections until `done` accepts the observations so far or `within` has passed,
    /// and returns them in arrival order.
    ///
    /// Each connection is served in its own task, so one that never sends anything does not hold
    /// up the others. Connections that fail (a wrong server name, a handshake the browser
    /// abandons, a request that is not the expected protocol) are logged and skipped.
    pub async fn collect_until(
        &self,
        within: Duration,
        mut done: impl FnMut(&[Observed]) -> bool,
    ) -> Vec<Observed> {
        let mut observed = Vec::new();
        if done(&observed) {
            return observed;
        }
        let mut serving: JoinSet<Result<Observed, CaptureH2Error>> = JoinSet::new();
        let deadline = tokio::time::sleep(within);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                accepted = self.listener.accept() => match accepted {
                    Ok((tcp, _peer)) => {
                        let shared = Arc::clone(&self.shared);
                        serving.spawn(serve(tcp, shared));
                    }
                    Err(error) => {
                        tracing::debug!(%error, "the capture site could not accept");
                        break;
                    }
                },
                Some(joined) = serving.join_next() => match joined {
                    Ok(Ok(one)) => {
                        observed.push(one);
                        if done(&observed) {
                            break;
                        }
                    }
                    Ok(Err(error)) => tracing::debug!(%error, "a connection was skipped"),
                    Err(error) => tracing::debug!(%error, "a connection task failed"),
                },
                () = &mut deadline => break,
            }
        }
        observed
    }
}

/// Serves one connection: server name, certificate, handshake, first request, response.
async fn serve(tcp: TcpStream, shared: Arc<Shared>) -> Result<Observed, CaptureH2Error> {
    let name = peek_server_name(&tcp, shared.timeout).await?;
    let probe = Probe::for_host(&name).ok_or(CaptureH2Error::UnknownHost)?;
    let host = Host::parse(&name).map_err(|_invalid| CaptureH2Error::UnknownHost)?;
    let leaf = shared.ca.leaf_for(&host, SystemTime::now())?;
    let mut tls = accept(&shared.acceptor, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await?;
    if tls.ssl().selected_alpn_protocol() != Some(shared.protocol.alpn()) {
        return Err(CaptureH2Error::NotNegotiated {
            expected: shared.protocol,
        });
    }
    let seen = match shared.protocol {
        Protocol::H2 => serve_h2(&mut tls, probe, shared.timeout).await?,
        Protocol::Http1 => serve_http1(&mut tls, probe, shared.timeout).await?,
    };
    Ok(Observed { probe, seen })
}

/// Reads the server name from the `ClientHello` waiting on `tcp`, leaving the bytes in place for
/// the handshake.
///
/// # Errors
///
/// Returns a [`CaptureH2Error`] when the bytes are not a `ClientHello`, it names no server, the
/// connection closes first, or `within` passes.
pub async fn peek_server_name(tcp: &TcpStream, within: Duration) -> Result<String, CaptureH2Error> {
    let work = async {
        let mut buf = vec![0_u8; MAX_CLIENT_HELLO + PEEK_SLACK];
        let mut last = 0;
        loop {
            let seen = tcp.peek(&mut buf).await?;
            let prefix = buf.get(..seen).unwrap_or_default();
            if seen == 0 {
                return Err(CaptureError::Closed { have: 0 });
            }
            if let Some(end) = ClientHello::needed_len(prefix)?.filter(|&end| end <= seen) {
                let hello = ClientHello::parse(prefix.get(..end).unwrap_or_default())?;
                return Ok(hello.sni);
            }
            if seen == buf.len() {
                return Err(CaptureError::TooLarge { limit: buf.len() });
            }
            if seen == last {
                tokio::time::sleep(PEEK_RETRY).await;
            }
            last = seen;
        }
    };
    let name = tokio::time::timeout(within, work)
        .await
        .map_err(|_elapsed| CaptureError::Timeout {
            what: "a ClientHello",
        })??;
    name.ok_or(CaptureH2Error::NoServerName)
}

/// Reads the first HTTP/2 request, answers it and closes.
async fn serve_h2<S>(tls: &mut S, probe: Probe, within: Duration) -> Result<Seen, CaptureH2Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (raw, preface) = read_h2c_preface(tls, within).await?;
    let body = (probe == DOCUMENT).then_some(DOCUMENT_BODY);
    tls.write_all(&h2_reply(preface.request_stream_id, body)?)
        .await
        .map_err(CaptureError::from)?;
    tls.flush().await.map_err(CaptureError::from)?;
    finish(tls).await;
    Ok(Seen::H2 { raw, preface })
}

/// Reads the first HTTP/1 request, answers it and closes.
async fn serve_http1<S>(tls: &mut S, probe: Probe, within: Duration) -> Result<Seen, CaptureH2Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request = read_http1_request(tls, within).await?;
    let response = if probe == DOCUMENT {
        http1_response(200, "OK", &[("Content-Type", HTML_TYPE)], DOCUMENT_BODY)
    } else {
        http1_response(200, "OK", &[], &[])
    };
    tls.write_all(&response).await.map_err(CaptureError::from)?;
    tls.flush().await.map_err(CaptureError::from)?;
    finish(tls).await;
    Ok(Seen::Http1 { request })
}

/// Reads and drops what the client still sends for a moment, then closes the connection.
///
/// Closing at once, with the browser's last frames unread, would reset the connection and could
/// discard the response the browser has not yet read.
async fn finish<S>(tls: &mut S)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut sink = [0_u8; DRAIN_CHUNK];
    let drain = async {
        loop {
            match tls.read(&mut sink).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    };
    if tokio::time::timeout(DRAIN_FOR, drain).await.is_err() {
        tracing::trace!("the client was still sending when the capture closed");
    }
    if let Err(error) = tls.shutdown().await {
        tracing::debug!(%error, "closing a capture connection failed");
    }
}

/// The bytes a server sends to let an HTTP/2 client finish its first request: an empty SETTINGS
/// frame, the acknowledgement of the client's SETTINGS, and a `200` response.
///
/// With a `body` the response is an HTML document (`content-type` and a DATA frame that ends the
/// stream); without one it is a bare `200` that ends the stream in its HEADERS frame.
///
/// # Errors
///
/// Returns [`CaptureH2Error::H2`] when `stream_id` does not fit 31 bits.
pub fn h2_reply(stream_id: u32, body: Option<&[u8]>) -> Result<Vec<u8>, CaptureH2Error> {
    let mut reply = encode_frame(frame_type::SETTINGS, 0, 0, &[])?;
    reply.extend(encode_frame(frame_type::SETTINGS, FLAG_ACK, 0, &[])?);
    let mut block = vec![HPACK_STATUS_200];
    let mut flags = FLAG_END_HEADERS;
    if body.is_none() {
        flags |= FLAG_END_STREAM;
    } else {
        block.extend_from_slice(&HPACK_CONTENT_TYPE);
        block.push(u8::try_from(HTML_TYPE.len()).unwrap_or(u8::MAX));
        block.extend_from_slice(HTML_TYPE.as_bytes());
    }
    reply.extend(encode_frame(frame_type::HEADERS, flags, stream_id, &block)?);
    if let Some(body) = body {
        reply.extend(encode_frame(
            frame_type::DATA,
            FLAG_END_STREAM,
            stream_id,
            body,
        )?);
    }
    Ok(reply)
}
