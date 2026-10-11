//! A loopback listener and the readers for the first bytes a client sends.
//!
//! The readers work on any [`AsyncRead`], so the tests feed them from memory. Each one reads until
//! the first message is complete, within a size limit and a time limit, and returns the exact bytes
//! it consumed beside what it parsed from them.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use super::CaptureError;
use crate::wire::client_hello::{ClientHello, MAX_CLIENT_HELLO};
use crate::wire::h2_frames::{
    encode_frame, frame_type, read_preface, H2Preface, MAX_PREFACE_BYTES,
};

/// Bytes asked of the socket per read.
const READ_CHUNK: usize = 4096;
/// How long a reader waits for the first message of a connection.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// The most bytes an HTTP/1 request line and headers may take.
pub const MAX_HTTP_HEAD: usize = 16 * 1024;
/// The most bytes of an HTTP/1 request body that are read.
pub const MAX_HTTP_BODY: usize = 64 * 1024;
/// The most header lines an HTTP/1 request may have.
pub const MAX_HTTP_HEADERS: usize = 100;
/// Extra bytes read beyond the largest hello, for the record headers between its fragments.
const HELLO_SLACK: usize = 1024;
/// Extra bytes read beyond the largest preface, for the frame that completes it.
const H2C_SLACK: usize = 16 * 1024;
/// SETTINGS and HEADERS flag: acknowledgement, or end of stream.
const FLAG_ACK_OR_END_STREAM: u8 = 0x1;
/// HEADERS flag: the header block is complete.
const FLAG_END_HEADERS: u8 = 0x4;
/// The HPACK byte for the static table entry `:status 200`.
const HPACK_STATUS_200: u8 = 0x88;
/// The end of an HTTP/1 head.
const HEAD_END: &[u8] = b"\r\n\r\n";

/// A `ClientHello` read from a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHello {
    /// The address the client connected from.
    pub peer: SocketAddr,
    /// The bytes of the TLS records that carry the hello, and nothing after them.
    pub raw: Vec<u8>,
    /// The parsed hello.
    pub hello: ClientHello,
}

/// The start of an HTTP/2 connection read from a plain TCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedH2c {
    /// The address the client connected from.
    pub peer: SocketAddr,
    /// The bytes from the connection preface to the end of the first request's header block.
    pub raw: Vec<u8>,
    /// What the frames say.
    pub preface: H2Preface,
}

/// An HTTP/1 request read from a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHttp1 {
    /// The address the client connected from.
    pub peer: SocketAddr,
    /// The request.
    pub request: Http1Request,
}

/// An HTTP/1 request with its header names, values and order exactly as sent.
#[derive(Clone, PartialEq, Eq)]
pub struct Http1Request {
    /// The method, such as `GET`.
    pub method: String,
    /// The request target, such as `/` or `/report?x=1`.
    pub target: String,
    /// The version, such as `HTTP/1.1`.
    pub version: String,
    /// The headers in wire order, names in the case the client wrote them.
    pub headers: Vec<(String, String)>,
    /// The body, as long as `Content-Length` says (at most [`MAX_HTTP_BODY`]).
    pub body: Vec<u8>,
}

impl Http1Request {
    /// The value of the first header named `name`, compared without regard to ASCII case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The path of the target, without a query string.
    #[must_use]
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }
}

/// Prints the header names but not their values, which can be cookies or credentials.
impl fmt::Debug for Http1Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Http1Request")
            .field("method", &self.method)
            .field("target", &self.target)
            .field("version", &self.version)
            .field("header_names", &names)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// A request line and headers, parsed from the bytes before the blank line.
#[derive(Debug)]
struct Head {
    method: String,
    target: String,
    version: String,
    headers: Vec<(String, String)>,
    content_length: usize,
}

/// Reads from `reader` until `ready` accepts the bytes so far.
///
/// Fails with [`CaptureError::Closed`] when the stream ends first, [`CaptureError::TooLarge`] when
/// more than `limit` bytes arrive without `ready` accepting them, and [`CaptureError::Timeout`]
/// when `within` passes.
async fn read_until<R, T>(
    reader: &mut R,
    limit: usize,
    within: Duration,
    what: &'static str,
    mut ready: impl FnMut(&[u8]) -> Result<Option<T>, CaptureError>,
) -> Result<(Vec<u8>, T), CaptureError>
where
    R: AsyncRead + Unpin,
{
    let work = async {
        let mut buf = Vec::new();
        let mut chunk = [0_u8; READ_CHUNK];
        loop {
            let read = reader.read(&mut chunk).await?;
            if read == 0 {
                return Err(CaptureError::Closed { have: buf.len() });
            }
            buf.extend_from_slice(chunk.get(..read).unwrap_or_default());
            if let Some(found) = ready(&buf)? {
                return Ok((buf, found));
            }
            if buf.len() > limit {
                return Err(CaptureError::TooLarge { limit });
            }
        }
    };
    tokio::time::timeout(within, work)
        .await
        .map_err(|_elapsed| CaptureError::Timeout { what })?
}

/// Reads the first `ClientHello` from `reader`.
///
/// Returns the bytes of the records that carry the hello (anything after them is dropped) and the
/// parsed hello.
///
/// # Errors
///
/// Returns a [`CaptureError`] when the bytes are not a `ClientHello`, the stream ends first, or
/// `within` passes.
pub async fn read_client_hello<R>(
    reader: &mut R,
    within: Duration,
) -> Result<(Vec<u8>, ClientHello), CaptureError>
where
    R: AsyncRead + Unpin,
{
    let (buf, end) = read_until(
        reader,
        MAX_CLIENT_HELLO + HELLO_SLACK,
        within,
        "a ClientHello",
        |buf| Ok(ClientHello::needed_len(buf)?.filter(|&end| end <= buf.len())),
    )
    .await?;
    let raw = buf.get(..end).unwrap_or_default().to_vec();
    let hello = ClientHello::parse(&raw)?;
    Ok((raw, hello))
}

/// Reads an HTTP/2 connection start (prior knowledge, no TLS) from `reader`.
///
/// Returns the bytes up to the end of the first request's header block and what they say.
///
/// # Errors
///
/// Returns a [`CaptureError`] when the bytes are not an HTTP/2 client start, the stream ends
/// first, or `within` passes.
pub async fn read_h2c_preface<R>(
    reader: &mut R,
    within: Duration,
) -> Result<(Vec<u8>, H2Preface), CaptureError>
where
    R: AsyncRead + Unpin,
{
    let (buf, preface) = read_until(
        reader,
        MAX_PREFACE_BYTES + H2C_SLACK,
        within,
        "an HTTP/2 connection start",
        |buf| Ok(read_preface(buf)?),
    )
    .await?;
    let raw = buf.get(..preface.consumed).unwrap_or_default().to_vec();
    Ok((raw, preface))
}

/// Reads one HTTP/1 request, with the body `Content-Length` announces.
///
/// # Errors
///
/// Returns a [`CaptureError`] when the bytes are not an HTTP/1 request, it is larger than
/// [`MAX_HTTP_HEAD`] or [`MAX_HTTP_BODY`], the stream ends first, or `within` passes.
pub async fn read_http1_request<R>(
    reader: &mut R,
    within: Duration,
) -> Result<Http1Request, CaptureError>
where
    R: AsyncRead + Unpin,
{
    let (mut buf, head_len) = read_until(reader, MAX_HTTP_HEAD, within, "an HTTP request", |buf| {
        Ok(find_head_end(buf))
    })
    .await?;
    let head = parse_head(buf.get(..head_len).unwrap_or_default())?;
    if head.content_length > MAX_HTTP_BODY {
        return Err(CaptureError::TooLarge {
            limit: MAX_HTTP_BODY,
        });
    }
    let mut body = buf.split_off(head_len);
    body.truncate(head.content_length);
    read_body(reader, &mut body, head.content_length, within).await?;
    Ok(Http1Request {
        method: head.method,
        target: head.target,
        version: head.version,
        headers: head.headers,
        body,
    })
}

/// Where the head ends: the length of the bytes through the blank line.
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(HEAD_END.len())
        .position(|window| window == HEAD_END)
        .map(|start| start + HEAD_END.len())
}

/// Reads until `body` holds `len` bytes.
async fn read_body<R>(
    reader: &mut R,
    body: &mut Vec<u8>,
    len: usize,
    within: Duration,
) -> Result<(), CaptureError>
where
    R: AsyncRead + Unpin,
{
    let mut chunk = [0_u8; READ_CHUNK];
    while body.len() < len {
        let want = (len - body.len()).min(READ_CHUNK);
        let window = chunk.get_mut(..want).unwrap_or_default();
        let read = tokio::time::timeout(within, reader.read(window))
            .await
            .map_err(|_elapsed| CaptureError::Timeout {
                what: "an HTTP request body",
            })??;
        if read == 0 {
            return Err(CaptureError::Closed { have: body.len() });
        }
        body.extend_from_slice(window.get(..read).unwrap_or_default());
    }
    Ok(())
}

/// Whether `text` is a non-empty HTTP token (RFC 9110, section 5.6.2).
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Splits `bytes` at each CRLF.
fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut rest = bytes;
    while let Some(end) = rest.windows(2).position(|pair| pair == b"\r\n") {
        lines.push(rest.get(..end).unwrap_or_default());
        rest = rest.get(end + 2..).unwrap_or_default();
    }
    lines.push(rest);
    lines
}

fn http_error(reason: &'static str) -> CaptureError {
    CaptureError::Http { reason }
}

/// Parses the request line: three parts separated by single spaces.
fn parse_request_line(line: &[u8]) -> Result<(String, String, String), CaptureError> {
    let text = std::str::from_utf8(line).map_err(|_invalid| http_error("request line"))?;
    let mut parts = text.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(http_error("request line"));
    };
    let sane_target = !target.is_empty() && !target.chars().any(char::is_control);
    if !is_token(method) || !sane_target || !version.starts_with("HTTP/1.") {
        return Err(http_error("request line"));
    }
    Ok((method.to_owned(), target.to_owned(), version.to_owned()))
}

/// Parses one header line into its name and trimmed value.
fn parse_header_line(line: &[u8]) -> Result<(String, String), CaptureError> {
    if line
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        return Err(http_error("header name"));
    }
    let colon = line
        .iter()
        .position(|&byte| byte == b':')
        .ok_or_else(|| http_error("header line"))?;
    let name = std::str::from_utf8(line.get(..colon).unwrap_or_default())
        .ok()
        .filter(|name| is_token(name))
        .ok_or_else(|| http_error("header name"))?;
    let value = std::str::from_utf8(line.get(colon + 1..).unwrap_or_default())
        .map_err(|_invalid| http_error("header is not UTF-8"))?;
    Ok((
        name.to_owned(),
        value.trim_matches(|c| c == ' ' || c == '\t').to_owned(),
    ))
}

/// Reads the one `Content-Length` of a request, or zero when there is none.
fn content_length(headers: &[(String, String)]) -> Result<usize, CaptureError> {
    let mut lengths = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.as_str());
    let Some(first) = lengths.next() else {
        return Ok(0);
    };
    if lengths.next().is_some() || first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) {
        return Err(http_error("content-length"));
    }
    first
        .parse()
        .map_err(|_overflow| http_error("content-length"))
}

/// Parses the request line and headers of `head`, which ends with the blank line.
fn parse_head(head: &[u8]) -> Result<Head, CaptureError> {
    let lines_end = head.strip_suffix(HEAD_END).unwrap_or(head);
    let mut lines = split_lines(lines_end).into_iter();
    let (method, target, version) = parse_request_line(lines.next().unwrap_or_default())?;
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() >= MAX_HTTP_HEADERS {
            return Err(http_error("too many headers"));
        }
        headers.push(parse_header_line(line)?);
    }
    if headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("transfer-encoding"))
    {
        return Err(http_error("chunked bodies"));
    }
    let content_length = content_length(&headers)?;
    Ok(Head {
        method,
        target,
        version,
        headers,
        content_length,
    })
}

/// The bytes a server sends to let an HTTP/2 client finish its first request: an empty SETTINGS
/// frame, the acknowledgement of the client's SETTINGS, and a `200` response without a body.
///
/// # Errors
///
/// Returns [`CaptureError::H2`] when `request_stream_id` does not fit 31 bits.
pub fn h2c_reply(request_stream_id: u32) -> Result<Vec<u8>, CaptureError> {
    let mut reply = encode_frame(frame_type::SETTINGS, 0, 0, &[])?;
    reply.extend(encode_frame(
        frame_type::SETTINGS,
        FLAG_ACK_OR_END_STREAM,
        0,
        &[],
    )?);
    reply.extend(encode_frame(
        frame_type::HEADERS,
        FLAG_ACK_OR_END_STREAM | FLAG_END_HEADERS,
        request_stream_id,
        &[HPACK_STATUS_200],
    )?);
    Ok(reply)
}

/// An HTTP/1 response with a `Content-Length` and `Connection: close`.
#[must_use]
pub fn http1_response(status: u16, reason: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut lines = vec![format!("HTTP/1.1 {status} {reason}")];
    lines.extend(
        headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}")),
    );
    lines.push(format!("Content-Length: {}", body.len()));
    lines.push("Connection: close".to_owned());
    let mut bytes = lines.join("\r\n").into_bytes();
    bytes.extend_from_slice(b"\r\n\r\n");
    bytes.extend_from_slice(body);
    bytes
}

/// A listener on a loopback port that reads the first message of each connection it accepts.
#[derive(Debug)]
pub struct Capture {
    listener: TcpListener,
    addr: SocketAddr,
    timeout: Duration,
}

impl Capture {
    /// Binds `127.0.0.1` on a port the system picks.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Io`] when the socket cannot be bound.
    pub async fn bind() -> Result<Self, CaptureError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        Ok(Self {
            listener,
            addr,
            timeout: DEFAULT_READ_TIMEOUT,
        })
    }

    /// Replaces how long a connection may take to send its first message.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The address the listener is bound to.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The port the listener is bound to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Accepts one connection, reads its `ClientHello` and drops the connection.
    ///
    /// Waits for a client to connect for as long as the caller lets the future run; wrap it in
    /// [`tokio::time::timeout`] to bound that.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureError`] when the connection does not send a `ClientHello` in time.
    pub async fn accept_client_hello(&self) -> Result<CapturedHello, CaptureError> {
        let (mut stream, peer) = self.listener.accept().await?;
        read_hello_from(&mut stream, peer, self.timeout).await
    }

    /// Accepts connections until `count` hellos have arrived or `within` has passed, and returns
    /// the hellos that arrived.
    ///
    /// Connections that send nothing (a browser opens spare sockets) or something else are skipped.
    pub async fn collect_client_hellos(
        &self,
        count: usize,
        within: Duration,
    ) -> Vec<CapturedHello> {
        self.collect_client_hellos_until(within, |hellos| hellos.len() >= count)
            .await
    }

    /// Accepts connections until `done` accepts the hellos collected so far or `within` has passed,
    /// and returns them in arrival order.
    ///
    /// `done` is asked after each hello. Every connection is read in its own task, so one that
    /// never sends anything does not hold up the others.
    pub async fn collect_client_hellos_until(
        &self,
        within: Duration,
        mut done: impl FnMut(&[CapturedHello]) -> bool,
    ) -> Vec<CapturedHello> {
        let mut hellos = Vec::new();
        if done(&hellos) {
            return hellos;
        }
        let mut readers: JoinSet<Result<CapturedHello, CaptureError>> = JoinSet::new();
        let deadline = tokio::time::sleep(within);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                accepted = self.listener.accept() => match accepted {
                    Ok((stream, peer)) => {
                        let timeout = self.timeout;
                        readers.spawn(async move {
                            let mut stream = stream;
                            read_hello_from(&mut stream, peer, timeout).await
                        });
                    }
                    Err(error) => {
                        tracing::debug!(%error, "the capture listener could not accept");
                        break;
                    }
                },
                Some(joined) = readers.join_next() => match joined {
                    Ok(Ok(hello)) => {
                        hellos.push(hello);
                        if done(&hellos) {
                            break;
                        }
                    }
                    Ok(Err(error)) => tracing::debug!(%error, "a connection sent no ClientHello"),
                    Err(error) => tracing::debug!(%error, "a reader task failed"),
                },
                () = &mut deadline => break,
            }
        }
        hellos
    }

    /// Accepts one connection, reads an HTTP/2 connection start, answers it with
    /// [`h2c_reply`] and closes.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureError`] when the connection does not send one in time.
    pub async fn accept_h2c(&self) -> Result<CapturedH2c, CaptureError> {
        let (mut stream, peer) = self.listener.accept().await?;
        let (mut raw, preface) = read_h2c_preface(&mut stream, self.timeout).await?;
        stream
            .write_all(&h2c_reply(preface.request_stream_id)?)
            .await?;
        stream.shutdown().await?;
        raw.truncate(preface.consumed);
        Ok(CapturedH2c { peer, raw, preface })
    }

    /// Accepts one connection, reads an HTTP/1 request, sends `response` and closes.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureError`] when the connection does not send a request in time.
    pub async fn accept_http1(&self, response: &[u8]) -> Result<CapturedHttp1, CaptureError> {
        let (mut stream, peer) = self.listener.accept().await?;
        let request = read_http1_request(&mut stream, self.timeout).await?;
        stream.write_all(response).await?;
        stream.shutdown().await?;
        Ok(CapturedHttp1 { peer, request })
    }
}

/// Reads a hello from an accepted connection and drops it.
async fn read_hello_from(
    stream: &mut TcpStream,
    peer: SocketAddr,
    within: Duration,
) -> Result<CapturedHello, CaptureError> {
    let (raw, hello) = read_client_hello(stream, within).await?;
    Ok(CapturedHello { peer, raw, hello })
}

#[cfg(test)]
mod tests;
