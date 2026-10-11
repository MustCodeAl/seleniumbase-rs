//! Reads the HTTP/2 connection preface, SETTINGS, PRIORITY and the first HEADERS frame from captured bytes.
//!
//! A capture tool listens for a client, collects the bytes it sends, and calls
//! [`read_preface`] after each read. When the first request's header block is complete the
//! result holds everything an Akamai fingerprint needs: the SETTINGS in order, the
//! connection `WINDOW_UPDATE`, the PRIORITY frames, the HEADERS priority fields, and, from a
//! full HPACK decode of the block, the header order and the pseudo-header order.
//!
//! The reader works on bytes already in memory and never waits: an incomplete frame means
//! `Ok(None)` ("read more"), and hostile input is bounded (frame size, frame count, preface size,
//! header block size) so that no input makes it allocate without limit or panic. [`read_preface`]
//! reads from the start on each call; a loop that calls it after every socket read should keep a
//! [`PrefaceReader`], which parses each frame once.
//!
//! The HPACK decoder is in [`hpack`] and its Huffman decoder in [`huffman`].
//!
//! ```
//! use seleniumbase_rs::wire::h2_frames::{encode_frame, frame_type, read_preface, CLIENT_PREFACE};
//!
//! let mut bytes = CLIENT_PREFACE.to_vec();
//! bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &[0, 4, 0, 0, 0x10, 0])?);
//! assert_eq!(read_preface(&bytes)?, None, "no HEADERS yet: read more");
//! # Ok::<(), seleniumbase_rs::wire::h2_frames::H2CaptureError>(())
//! ```

pub mod hpack;
pub mod huffman;
mod preface;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use hpack::{HeaderField, HpackError};

pub use preface::PrefaceReader;

use super::akamai::{Akamai, PriorityFrame, Pseudo, MAX_STREAM_ID};

/// The 24 bytes every HTTP/2 client connection starts with (RFC 9113, section 3.4).
pub const CLIENT_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
/// Bytes in a frame header: length (3), type (1), flags (1), stream id (4).
pub const FRAME_HEADER_LEN: usize = 9;
/// The largest frame payload a peer may send before it learns a larger `MAX_FRAME_SIZE`.
pub const DEFAULT_MAX_FRAME_PAYLOAD: usize = 16_384;
/// The largest payload any frame can carry: the length field has 24 bits.
pub const MAX_FRAME_PAYLOAD: usize = 0x00ff_ffff;
/// The most bytes [`read_preface`] reads before it gives up on finding the first request.
pub const MAX_PREFACE_BYTES: usize = 256 * 1024;
/// The most bytes of HPACK data one header block may span across its frames.
pub const MAX_HEADER_BLOCK_BYTES: usize = 64 * 1024;
/// The most frames, of any type, that may come before the end of the first request's header
/// block. Browsers send three to eight.
pub const MAX_PREFACE_FRAMES: usize = 64;

/// Bytes of the priority fields in PRIORITY frames and in HEADERS with the PRIORITY flag.
const PRIORITY_FIELDS_LEN: usize = 5;
/// Bytes of one SETTINGS entry: a 16-bit id and a 32-bit value.
const SETTING_LEN: usize = 6;

/// Frame type codes (RFC 9113, section 6).
pub mod frame_type {
    /// DATA.
    pub const DATA: u8 = 0x0;
    /// HEADERS.
    pub const HEADERS: u8 = 0x1;
    /// PRIORITY.
    pub const PRIORITY: u8 = 0x2;
    /// `RST_STREAM`.
    pub const RST_STREAM: u8 = 0x3;
    /// SETTINGS.
    pub const SETTINGS: u8 = 0x4;
    /// `PUSH_PROMISE`.
    pub const PUSH_PROMISE: u8 = 0x5;
    /// PING.
    pub const PING: u8 = 0x6;
    /// GOAWAY.
    pub const GOAWAY: u8 = 0x7;
    /// `WINDOW_UPDATE`.
    pub const WINDOW_UPDATE: u8 = 0x8;
    /// CONTINUATION.
    pub const CONTINUATION: u8 = 0x9;
}

/// Frame flags (RFC 9113, section 6).
mod flag {
    /// SETTINGS acknowledgement.
    pub const ACK: u8 = 0x1;
    /// HEADERS: no further frames on the stream from the sender.
    pub const END_STREAM: u8 = 0x1;
    /// HEADERS and CONTINUATION: the header block is complete.
    pub const END_HEADERS: u8 = 0x4;
    /// HEADERS: the payload starts with a pad length.
    pub const PADDED: u8 = 0x8;
    /// HEADERS: the payload carries priority fields.
    pub const PRIORITY: u8 = 0x20;
}

/// The priority fields of a HEADERS frame, as Akamai notation reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadersPriority {
    /// The stream the new stream depends on.
    pub depends_on: u32,
    /// The protocol weight, 1 to 256 (the wire byte plus one).
    pub weight: u16,
    /// Whether the dependency is exclusive.
    pub exclusive: bool,
}

/// One frame, reduced to what a fingerprint needs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Frame {
    /// SETTINGS, with the entries in wire order (an acknowledgement has none).
    Settings {
        /// Whether this is an acknowledgement.
        ack: bool,
        /// `(id, value)` entries in the order they were sent.
        settings: Vec<(u16, u32)>,
    },
    /// `WINDOW_UPDATE`.
    WindowUpdate {
        /// The stream, or `0` for the connection.
        stream_id: u32,
        /// The window increment, at least 1.
        increment: u32,
    },
    /// PRIORITY.
    Priority(PriorityFrame),
    /// HEADERS, with the padding and priority fields removed.
    Headers {
        /// The stream the headers open.
        stream_id: u32,
        /// Whether the sender ends the stream with this frame.
        end_stream: bool,
        /// Whether the header block is complete in this frame.
        end_headers: bool,
        /// The priority fields, when the frame carried them.
        priority: Option<HeadersPriority>,
        /// The HPACK fragment.
        fragment: Vec<u8>,
    },
    /// CONTINUATION, a further fragment of a header block.
    Continuation {
        /// The stream of the header block.
        stream_id: u32,
        /// Whether the header block ends with this fragment.
        end_headers: bool,
        /// The HPACK fragment.
        fragment: Vec<u8>,
    },
    /// Any other frame type: only its framing is read.
    Other {
        /// The frame type code.
        frame_type: u8,
        /// The flags byte.
        flags: u8,
        /// The stream id.
        stream_id: u32,
        /// The payload length.
        len: usize,
    },
}

impl Frame {
    /// The frame type code of this frame.
    #[must_use]
    pub fn frame_type(&self) -> u8 {
        match self {
            Frame::Settings { .. } => frame_type::SETTINGS,
            Frame::WindowUpdate { .. } => frame_type::WINDOW_UPDATE,
            Frame::Priority(_) => frame_type::PRIORITY,
            Frame::Headers { .. } => frame_type::HEADERS,
            Frame::Continuation { .. } => frame_type::CONTINUATION,
            Frame::Other { frame_type, .. } => *frame_type,
        }
    }
}

/// Why captured HTTP/2 bytes were refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum H2CaptureError {
    /// The bytes do not start with the HTTP/2 client connection preface.
    #[error("the bytes do not start with the HTTP/2 client connection preface")]
    BadPreface,
    /// A frame declares a payload larger than the limit.
    #[error("a frame declares a {len}-byte payload, above the limit of {max}")]
    FrameTooLarge {
        /// The declared payload length.
        len: usize,
        /// The limit in force.
        max: usize,
    },
    /// A frame's payload length is impossible for its type.
    #[error("a frame of type {frame_type:#x} has an impossible payload length of {len}")]
    FrameSize {
        /// The frame type code.
        frame_type: u8,
        /// The payload length.
        len: usize,
    },
    /// A frame is on a stream its type forbids, or off the stream it requires.
    #[error("a frame of type {frame_type:#x} is not allowed on stream {stream_id}")]
    StreamId {
        /// The frame type code.
        frame_type: u8,
        /// The stream id.
        stream_id: u32,
    },
    /// The pad length of a padded frame is larger than the frame's data.
    #[error("a padded frame declares more padding than it has room for")]
    BadPadding,
    /// A `WINDOW_UPDATE` carries an increment of zero.
    #[error("a WINDOW_UPDATE frame carries an increment of zero")]
    ZeroWindowIncrement,
    /// A frame that cannot come before the first request arrived.
    #[error("a frame of type {frame_type:#x} is not allowed before the first request")]
    UnexpectedFrame {
        /// The frame type code.
        frame_type: u8,
    },
    /// A header block was interrupted by something other than its CONTINUATION frames.
    #[error("a frame of type {frame_type:#x} interrupts a header block")]
    ExpectedContinuation {
        /// The frame type code.
        frame_type: u8,
    },
    /// A header block spans more than [`MAX_HEADER_BLOCK_BYTES`].
    #[error("a header block is larger than {} bytes", MAX_HEADER_BLOCK_BYTES)]
    HeaderBlockTooLarge,
    /// The first request did not complete within [`MAX_PREFACE_BYTES`].
    #[error("no complete request within {} bytes", MAX_PREFACE_BYTES)]
    PrefaceTooLarge,
    /// The first request did not complete within [`MAX_PREFACE_FRAMES`] frames.
    #[error("no complete request within {max} frames")]
    TooManyFrames {
        /// The limit in force.
        max: usize,
    },
    /// A [`PrefaceReader`] was asked to read again after it returned a result or an error.
    #[error("the reader has already returned its result; start a new one")]
    ReaderFinished,
    /// The first request arrived before any SETTINGS frame.
    #[error("the first request arrived before any SETTINGS frame")]
    MissingSettings,
    /// The pseudo-headers are not each of `:method`, `:authority`, `:scheme` and `:path` once, first.
    #[error("the pseudo-headers are not exactly :method, :authority, :scheme and :path before the other headers")]
    PseudoOrder,
    /// A frame cannot be built: the stream id is above 2^31 - 1.
    #[error("stream id {0} is above 2147483647")]
    StreamIdRange(u32),
    /// The header block does not decode.
    #[error("the header block does not decode: {0}")]
    Hpack(#[from] HpackError),
}

/// The decoded start of a client connection, up to and including its first request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H2Preface {
    /// The entries of the first SETTINGS frame, in wire order.
    pub settings: Vec<(u16, u32)>,
    /// The first connection-level `WINDOW_UPDATE` increment, or `0` when there was none.
    pub window_update: u32,
    /// The PRIORITY frames before the request, in order.
    pub priority_frames: Vec<PriorityFrame>,
    /// The priority fields of the request's HEADERS frame, when it had them.
    pub headers_priority: Option<HeadersPriority>,
    /// The stream id of the first request.
    pub request_stream_id: u32,
    /// The request's headers in wire order, pseudo-headers included.
    pub headers: Vec<HeaderField>,
    /// How many bytes of the input the preface used, the magic included.
    pub consumed: usize,
}

impl H2Preface {
    /// The order of the four pseudo-headers in the request.
    ///
    /// # Errors
    ///
    /// Returns [`H2CaptureError::PseudoOrder`] unless the request has each of `:method`,
    /// `:authority`, `:scheme` and `:path` exactly once, all before the other headers.
    pub fn pseudo_order(&self) -> Result<[Pseudo; 4], H2CaptureError> {
        let mut order: Vec<Pseudo> = Vec::with_capacity(Pseudo::ALL.len());
        let mut regular_seen = false;
        for field in &self.headers {
            if !field.is_pseudo() {
                regular_seen = true;
                continue;
            }
            let pseudo =
                Pseudo::from_header_name(&field.name).ok_or(H2CaptureError::PseudoOrder)?;
            if regular_seen || order.contains(&pseudo) {
                return Err(H2CaptureError::PseudoOrder);
            }
            order.push(pseudo);
        }
        <[Pseudo; 4]>::try_from(order).map_err(|_short| H2CaptureError::PseudoOrder)
    }

    /// The Akamai fingerprint of this connection start.
    ///
    /// # Errors
    ///
    /// Returns [`H2CaptureError::PseudoOrder`] when the pseudo-header order is not a
    /// permutation of the four request pseudo-headers.
    pub fn akamai(&self) -> Result<Akamai, H2CaptureError> {
        Ok(Akamai {
            settings: self.settings.clone(),
            window_update: self.window_update,
            priority_frames: self.priority_frames.clone(),
            pseudo_order: self.pseudo_order()?,
        })
    }

    /// The request's headers without the pseudo-headers, in wire order.
    pub fn regular_headers(&self) -> impl Iterator<Item = &HeaderField> {
        self.headers.iter().filter(|field| !field.is_pseudo())
    }

    /// The value of the first header named `name`, compared without regard to ASCII case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
            .map(|field| field.value.as_str())
    }
}

/// Builds the bytes of one frame.
///
/// # Errors
///
/// Returns [`H2CaptureError::FrameTooLarge`] when the payload does not fit the 24-bit length,
/// and [`H2CaptureError::StreamIdRange`] when the stream id does not fit 31 bits.
pub fn encode_frame(
    frame_type: u8,
    flags: u8,
    stream_id: u32,
    payload: &[u8],
) -> Result<Vec<u8>, H2CaptureError> {
    if payload.len() > MAX_FRAME_PAYLOAD {
        return Err(H2CaptureError::FrameTooLarge {
            len: payload.len(),
            max: MAX_FRAME_PAYLOAD,
        });
    }
    if stream_id > MAX_STREAM_ID {
        return Err(H2CaptureError::StreamIdRange(stream_id));
    }
    let len = u32::try_from(payload.len()).map_err(|_wide| H2CaptureError::FrameTooLarge {
        len: payload.len(),
        max: MAX_FRAME_PAYLOAD,
    })?;
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
    out.extend_from_slice(&len.to_be_bytes()[1..]);
    out.push(frame_type);
    out.push(flags);
    out.extend_from_slice(&stream_id.to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// The nine-byte header in front of every frame.
struct Head {
    len: usize,
    kind: u8,
    flags: u8,
    stream_id: u32,
}

impl Head {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let [l0, l1, l2, kind, flags, s0, s1, s2, s3] = *bytes.first_chunk::<FRAME_HEADER_LEN>()?;
        Some(Self {
            len: usize::try_from(u32::from_be_bytes([0, l0, l1, l2])).ok()?,
            kind,
            flags,
            stream_id: u32::from_be_bytes([s0, s1, s2, s3]) & MAX_STREAM_ID,
        })
    }

    fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    fn require_stream_zero(&self) -> Result<(), H2CaptureError> {
        if self.stream_id == 0 {
            Ok(())
        } else {
            Err(H2CaptureError::StreamId {
                frame_type: self.kind,
                stream_id: self.stream_id,
            })
        }
    }

    fn require_stream(&self) -> Result<(), H2CaptureError> {
        if self.stream_id == 0 {
            Err(H2CaptureError::StreamId {
                frame_type: self.kind,
                stream_id: 0,
            })
        } else {
            Ok(())
        }
    }

    fn size_error(&self) -> H2CaptureError {
        H2CaptureError::FrameSize {
            frame_type: self.kind,
            len: self.len,
        }
    }
}

/// Reads one frame from the front of `bytes`.
///
/// Returns the frame and the number of bytes it used, or `Ok(None)` when `bytes` ends before
/// the frame does. A payload length above `max_payload` is refused as soon as the nine header
/// bytes are in, without waiting for the payload.
///
/// # Errors
///
/// Returns an [`H2CaptureError`] when the frame is larger than `max_payload` or its payload is
/// impossible for its type.
pub fn read_frame(
    bytes: &[u8],
    max_payload: usize,
) -> Result<Option<(Frame, usize)>, H2CaptureError> {
    let Some(head) = Head::parse(bytes) else {
        return Ok(None);
    };
    if head.len > max_payload {
        return Err(H2CaptureError::FrameTooLarge {
            len: head.len,
            max: max_payload,
        });
    }
    let total = FRAME_HEADER_LEN + head.len;
    let Some(payload) = bytes.get(FRAME_HEADER_LEN..total) else {
        return Ok(None);
    };
    Ok(Some((parse_payload(&head, payload)?, total)))
}

fn parse_payload(head: &Head, payload: &[u8]) -> Result<Frame, H2CaptureError> {
    match head.kind {
        frame_type::SETTINGS => parse_settings(head, payload),
        frame_type::WINDOW_UPDATE => parse_window_update(head, payload),
        frame_type::PRIORITY => parse_priority(head, payload),
        frame_type::HEADERS => parse_headers(head, payload),
        frame_type::CONTINUATION => {
            head.require_stream()?;
            Ok(Frame::Continuation {
                stream_id: head.stream_id,
                end_headers: head.has(flag::END_HEADERS),
                fragment: payload.to_vec(),
            })
        }
        kind => Ok(Frame::Other {
            frame_type: kind,
            flags: head.flags,
            stream_id: head.stream_id,
            len: head.len,
        }),
    }
}

fn parse_settings(head: &Head, payload: &[u8]) -> Result<Frame, H2CaptureError> {
    head.require_stream_zero()?;
    let ack = head.has(flag::ACK);
    if (ack && !payload.is_empty()) || !payload.len().is_multiple_of(SETTING_LEN) {
        return Err(head.size_error());
    }
    let (entries, _whole_entries_only) = payload.as_chunks::<SETTING_LEN>();
    let settings = entries
        .iter()
        .map(|&[i0, i1, v0, v1, v2, v3]| {
            (
                u16::from_be_bytes([i0, i1]),
                u32::from_be_bytes([v0, v1, v2, v3]),
            )
        })
        .collect();
    Ok(Frame::Settings { ack, settings })
}

fn parse_window_update(head: &Head, payload: &[u8]) -> Result<Frame, H2CaptureError> {
    let Ok(raw) = <[u8; 4]>::try_from(payload) else {
        return Err(head.size_error());
    };
    let increment = u32::from_be_bytes(raw) & MAX_STREAM_ID;
    if increment == 0 {
        return Err(H2CaptureError::ZeroWindowIncrement);
    }
    Ok(Frame::WindowUpdate {
        stream_id: head.stream_id,
        increment,
    })
}

fn parse_priority(head: &Head, payload: &[u8]) -> Result<Frame, H2CaptureError> {
    head.require_stream()?;
    let Ok(raw) = <[u8; PRIORITY_FIELDS_LEN]>::try_from(payload) else {
        return Err(head.size_error());
    };
    let fields = priority_fields(raw);
    Ok(Frame::Priority(PriorityFrame {
        stream_id: head.stream_id,
        exclusive: fields.exclusive,
        depends_on: fields.depends_on,
        weight: fields.weight,
    }))
}

/// Reads the exclusive bit, the dependency and the weight byte (stored plus one).
fn priority_fields(raw: [u8; PRIORITY_FIELDS_LEN]) -> HeadersPriority {
    let [d0, d1, d2, d3, weight] = raw;
    let dependency = u32::from_be_bytes([d0, d1, d2, d3]);
    HeadersPriority {
        depends_on: dependency & MAX_STREAM_ID,
        weight: u16::from(weight) + 1,
        exclusive: dependency > MAX_STREAM_ID,
    }
}

fn parse_headers(head: &Head, payload: &[u8]) -> Result<Frame, H2CaptureError> {
    head.require_stream()?;
    let mut data = payload;
    let mut padding = 0;
    if head.has(flag::PADDED) {
        let (pad, rest) = data.split_first().ok_or_else(|| head.size_error())?;
        padding = usize::from(*pad);
        data = rest;
    }
    let mut priority = None;
    if head.has(flag::PRIORITY) {
        let (fields, rest) = data
            .split_first_chunk::<PRIORITY_FIELDS_LEN>()
            .ok_or_else(|| head.size_error())?;
        priority = Some(priority_fields(*fields));
        data = rest;
    }
    let fragment_len = data
        .len()
        .checked_sub(padding)
        .ok_or(H2CaptureError::BadPadding)?;
    Ok(Frame::Headers {
        stream_id: head.stream_id,
        end_stream: head.has(flag::END_STREAM),
        end_headers: head.has(flag::END_HEADERS),
        priority,
        fragment: data.get(..fragment_len).unwrap_or_default().to_vec(),
    })
}

/// Reads the client preface, its SETTINGS, `WINDOW_UPDATE` and PRIORITY frames, and the first
/// request's header block.
///
/// Returns `Ok(None)` when `bytes` ends before the first request is complete; call it again
/// with the longer buffer. Each call reads from the start, which the [`MAX_PREFACE_BYTES`] and
/// [`MAX_PREFACE_FRAMES`] limits keep bounded; a loop that reads a socket and calls this after
/// every read should use a [`PrefaceReader`], which parses each frame once.
///
/// # Errors
///
/// Returns an [`H2CaptureError`] when the bytes are not an HTTP/2 client start, a frame is
/// malformed or oversized (above [`DEFAULT_MAX_FRAME_PAYLOAD`]), the request does not arrive
/// within [`MAX_PREFACE_BYTES`] or [`MAX_PREFACE_FRAMES`], or its header block does not decode.
pub fn read_preface(bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError> {
    PrefaceReader::new().read(bytes)
}

#[cfg(test)]
mod tests;
