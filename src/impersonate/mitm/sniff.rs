//! What a `CONNECT` tunnel carries, told from its first bytes, which are then put back.
//!
//! Chrome opens a tunnel for `https://` and `wss://` and starts it with a TLS `ClientHello`; it
//! opens one for `ws://` as well and writes the plain-text WebSocket handshake into it (measured on
//! Chrome 155: `CONNECT host:port`, then `GET /path HTTP/1.1` with `Upgrade: websocket`). The proxy
//! reads the first [`SNIFF_LEN`] bytes to tell the two apart, and treats anything else as a tunnel
//! it cannot intercept.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, ReadBuf};

/// How many bytes are read to tell what a tunnel carries.
pub(super) const SNIFF_LEN: usize = 4;
/// The record type of a TLS handshake, the first byte of every `ClientHello`.
const TLS_HANDSHAKE: u8 = 0x16;
/// How a WebSocket handshake starts: it is always a `GET`.
const PLAIN_GET: &[u8; SNIFF_LEN] = b"GET ";

/// What a tunnel carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Carried {
    /// A TLS handshake.
    Tls,
    /// A plain-text request that starts as a WebSocket handshake does (`GET `).
    PlainGet,
    /// Anything else, which is never relayed.
    Other,
}

impl Carried {
    /// What the first bytes of a tunnel say it carries.
    pub(super) fn of(first: [u8; SNIFF_LEN]) -> Self {
        if first[0] == TLS_HANDSHAKE {
            Self::Tls
        } else if &first == PLAIN_GET {
            Self::PlainGet
        } else {
            Self::Other
        }
    }
}

/// Reads the first bytes of a tunnel within `timeout`. Returns what they say it carries and the
/// stream with them put back, or `None` when the client sent fewer in time or closed.
pub(super) async fn sniff<S>(mut stream: S, timeout: Duration) -> Option<(Carried, Rewound<S>)>
where
    S: AsyncRead + Unpin,
{
    let mut first = [0_u8; SNIFF_LEN];
    tokio::time::timeout(timeout, stream.read_exact(&mut first))
        .await
        .ok()?
        .ok()?;
    Some((Carried::of(first), Rewound::new(first, stream)))
}

/// A stream with its first bytes, read already, put back in front of it.
#[derive(Debug)]
pub(super) struct Rewound<S> {
    /// The bytes read to sniff the stream.
    first: [u8; SNIFF_LEN],
    /// How many of them were handed on.
    given: usize,
    /// The stream after them.
    inner: S,
}

impl<S> Rewound<S> {
    /// `inner` with `first` in front.
    fn new(first: [u8; SNIFF_LEN], inner: S) -> Self {
        Self {
            first,
            given: 0,
            inner,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Rewound<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let left = this.first.get(this.given..).unwrap_or_default();
        if left.is_empty() {
            return Pin::new(&mut this.inner).poll_read(cx, buf);
        }
        let count = left.len().min(buf.remaining());
        buf.put_slice(left.get(..count).unwrap_or_default());
        this.given += count;
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Rewound<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt as _;

    use super::*;

    #[test]
    fn the_first_bytes_tell_tls_from_a_plain_get_from_anything_else() {
        assert_eq!(Carried::of([0x16, 0x03, 0x01, 0x02]), Carried::Tls);
        assert_eq!(Carried::of(*b"GET "), Carried::PlainGet);
        for other in [
            b"POST",
            b"get ",
            b"GET/",
            b"SSH-",
            b"\x00\x00\x00\x00",
            b"HEAD",
        ] {
            assert_eq!(Carried::of(*other), Carried::Other, "{other:?}");
        }
    }

    #[tokio::test]
    async fn the_sniffed_bytes_come_back_first_then_the_rest_and_writes_pass_through() {
        let (mut client, server) = tokio::io::duplex(64);
        client
            .write_all(b"GET /chat HTTP/1.1\r\n")
            .await
            .expect("writes");

        let (carried, mut stream) = sniff(server, Duration::from_secs(5))
            .await
            .expect("four bytes in time");
        let mut small = [0_u8; 2];
        stream.read_exact(&mut small).await.expect("reads");
        let mut rest = vec![0_u8; 18];
        stream.read_exact(&mut rest).await.expect("reads");
        stream.write_all(b"HTTP/1.1 101").await.expect("writes");
        let mut answer = [0_u8; 12];
        client.read_exact(&mut answer).await.expect("reads");

        assert_eq!(carried, Carried::PlainGet);
        assert_eq!(&small, b"GE");
        assert_eq!(rest, b"T /chat HTTP/1.1\r\n");
        assert_eq!(&answer, b"HTTP/1.1 101");
    }

    #[tokio::test]
    async fn too_few_bytes_in_time_or_a_close_sniff_nothing() {
        let (mut client, server) = tokio::io::duplex(64);
        client.write_all(b"GE").await.expect("writes");
        assert!(sniff(server, Duration::from_millis(100)).await.is_none());

        let (client, server) = tokio::io::duplex(64);
        drop(client);
        assert!(sniff(server, Duration::from_secs(5)).await.is_none());
    }
}
