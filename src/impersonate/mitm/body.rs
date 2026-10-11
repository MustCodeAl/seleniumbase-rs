//! A body cap that leaves the body's framing alone.
//!
//! `http_body_util::Limited` reports an exact size equal to its limit when the body it wraps
//! is larger, which contradicts a `Content-Length` the proxy forwards. [`Capped`] reports the
//! wrapped body's own size and only fails, mid-stream, once more data than the limit has passed.
//! A body whose announced length is over the limit is refused before it starts, so the cap only
//! ever cuts a body of unknown length or one that lied about its length.

use std::pin::Pin;
use std::task::{ready, Context, Poll};

use bytes::Buf;
use hyper::body::{Body, Frame, SizeHint};

use super::forward::BoxError;
use super::MitmError;

/// A body that fails once more than `limit` bytes of data have passed.
#[derive(Debug)]
pub(super) struct Capped<B> {
    /// The body being passed on.
    inner: B,
    /// The limit, for the error.
    limit: usize,
    /// How many more data bytes may pass.
    remaining: usize,
}

impl<B> Capped<B> {
    /// Caps `inner` at `limit` data bytes.
    pub(super) fn new(inner: B, limit: usize) -> Self {
        Self {
            inner,
            limit,
            remaining: limit,
        }
    }
}

impl<B> Body for Capped<B>
where
    B: Body + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = &mut *self;
        let polled = ready!(Pin::new(&mut this.inner).poll_frame(cx));
        let Some(frame) = polled else {
            return Poll::Ready(None);
        };
        let frame = match frame {
            Ok(frame) => frame,
            Err(error) => return Poll::Ready(Some(Err(error.into()))),
        };
        if let Some(data) = frame.data_ref() {
            let length = data.remaining();
            if length > this.remaining {
                let error = MitmError::BodyTooLarge { limit: this.limit };
                return Poll::Ready(Some(Err(Box::new(error))));
            }
            this.remaining -= length;
        }
        Poll::Ready(Some(Ok(frame)))
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use bytes::Bytes;
    use http_body_util::{BodyExt as _, Full, StreamBody};
    use hyper::body::{Body as _, Frame};

    use super::Capped;

    #[tokio::test]
    async fn a_body_within_the_limit_passes_whole_with_its_own_size() {
        let body = Capped::new(Full::new(Bytes::from_static(b"twelve bytes")), 12);
        assert_eq!(body.size_hint().exact(), Some(12));
        let collected = body.collect().await.expect("within the limit").to_bytes();
        assert_eq!(collected, Bytes::from_static(b"twelve bytes"));
    }

    #[tokio::test]
    async fn a_larger_body_keeps_its_own_size_hint_and_fails_past_the_limit() {
        let body = Capped::new(Full::new(Bytes::from(vec![0_u8; 64])), 16);
        assert_eq!(
            body.size_hint().exact(),
            Some(64),
            "the framing is untouched"
        );
        assert!(body.collect().await.is_err());
    }

    #[tokio::test]
    async fn a_stream_is_cut_at_the_first_chunk_past_the_limit() {
        let chunks = (0..4).map(|_| Ok::<_, Infallible>(Frame::data(Bytes::from(vec![1_u8; 10]))));
        let mut body = Capped::new(StreamBody::new(futures_util::stream::iter(chunks)), 25);
        let mut passed = 0;
        while let Some(frame) = body.frame().await {
            match frame {
                Ok(frame) => passed += frame.into_data().map_or(0, |data| data.len()),
                Err(error) => {
                    assert!(error.to_string().contains("25"), "{error}");
                    break;
                }
            }
        }
        assert_eq!(passed, 20);
    }
}
