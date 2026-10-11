//! [`PrefaceReader`], which reads a client's connection start from a buffer that keeps growing.

use std::fmt;

use super::hpack::HpackDecoder;
use super::{
    frame_type, read_frame, Frame, H2CaptureError, H2Preface, HeadersPriority, CLIENT_PREFACE,
    DEFAULT_MAX_FRAME_PAYLOAD, MAX_HEADER_BLOCK_BYTES, MAX_PREFACE_BYTES, MAX_PREFACE_FRAMES,
};
use crate::wire::akamai::PriorityFrame;

/// A header block being collected from a HEADERS frame and its CONTINUATION frames.
struct PendingBlock {
    stream_id: u32,
    priority: Option<HeadersPriority>,
    block: Vec<u8>,
}

impl PendingBlock {
    fn append(&mut self, fragment: &[u8]) -> Result<(), H2CaptureError> {
        if self.block.len() + fragment.len() > MAX_HEADER_BLOCK_BYTES {
            return Err(H2CaptureError::HeaderBlockTooLarge);
        }
        self.block.extend_from_slice(fragment);
        Ok(())
    }
}

/// What has been seen of the connection so far.
#[derive(Default)]
struct PrefaceState {
    settings: Option<Vec<(u16, u32)>>,
    window_update: Option<u32>,
    priority_frames: Vec<PriorityFrame>,
    pending: Option<PendingBlock>,
}

impl PrefaceState {
    /// Takes one frame; returns the finished header block when the frame completes it.
    fn accept(&mut self, frame: Frame) -> Result<Option<PendingBlock>, H2CaptureError> {
        if let Some(pending) = self.pending.take() {
            return self.continue_block(pending, frame);
        }
        match frame {
            Frame::Settings {
                ack: false,
                settings,
            } => {
                self.settings.get_or_insert(settings);
            }
            Frame::WindowUpdate {
                stream_id: 0,
                increment,
            } => {
                self.window_update.get_or_insert(increment);
            }
            Frame::Priority(priority) => self.priority_frames.push(priority),
            Frame::Headers {
                stream_id,
                end_headers,
                priority,
                fragment,
                ..
            } => {
                let mut pending = PendingBlock {
                    stream_id,
                    priority,
                    block: Vec::new(),
                };
                pending.append(&fragment)?;
                return Ok(self.finish_or_wait(pending, end_headers));
            }
            Frame::Continuation { .. }
            | Frame::Other {
                frame_type: frame_type::DATA | frame_type::PUSH_PROMISE,
                ..
            } => {
                return Err(H2CaptureError::UnexpectedFrame {
                    frame_type: frame.frame_type(),
                });
            }
            _ => {}
        }
        Ok(None)
    }

    fn continue_block(
        &mut self,
        mut pending: PendingBlock,
        frame: Frame,
    ) -> Result<Option<PendingBlock>, H2CaptureError> {
        match frame {
            Frame::Continuation {
                stream_id,
                end_headers,
                fragment,
            } if stream_id == pending.stream_id => {
                pending.append(&fragment)?;
                Ok(self.finish_or_wait(pending, end_headers))
            }
            other => Err(H2CaptureError::ExpectedContinuation {
                frame_type: other.frame_type(),
            }),
        }
    }

    /// Hands back a complete block, or keeps an incomplete one for its next CONTINUATION.
    fn finish_or_wait(&mut self, pending: PendingBlock, complete: bool) -> Option<PendingBlock> {
        if complete {
            Some(pending)
        } else {
            self.pending = Some(pending);
            None
        }
    }

    fn finish(self, block: &PendingBlock, consumed: usize) -> Result<H2Preface, H2CaptureError> {
        let settings = self.settings.ok_or(H2CaptureError::MissingSettings)?;
        let headers = HpackDecoder::default().decode(&block.block)?;
        Ok(H2Preface {
            settings,
            window_update: self.window_update.unwrap_or(0),
            priority_frames: self.priority_frames,
            headers_priority: block.priority,
            request_stream_id: block.stream_id,
            headers,
            consumed,
        })
    }
}

/// Whether `bytes` holds the whole client preface; an error as soon as it cannot be one.
fn check_magic(bytes: &[u8]) -> Result<bool, H2CaptureError> {
    let seen = bytes.len().min(CLIENT_PREFACE.len());
    if bytes.get(..seen) != CLIENT_PREFACE.get(..seen) {
        return Err(H2CaptureError::BadPreface);
    }
    Ok(bytes.len() >= CLIENT_PREFACE.len())
}

/// Reads a client's connection start from a buffer that grows between calls, parsing every
/// frame once.
///
/// [`read_preface`](super::read_preface) reads from the start of the buffer on every call, so a
/// peer that sends one byte at a time makes a loop that calls it after each read do work
/// quadratic in the bytes received. A capture loop keeps one `PrefaceReader` instead and passes
/// it the whole buffer after each read: the reader remembers the frames it has already parsed
/// and the offset it stopped at, so each call costs the new bytes only, and the work for the
/// whole connection is linear. The buffer of a later call must start with the bytes of the
/// earlier ones; a shorter buffer only makes the call report that it needs more.
///
/// After it returns a request or an error, the reader is spent: a later call returns
/// [`H2CaptureError::ReaderFinished`]. What it reads is bounded by [`MAX_PREFACE_BYTES`] bytes
/// and [`MAX_PREFACE_FRAMES`] frames.
///
/// `Debug` shows the counters only; the bytes read may hold cookies.
#[derive(Default)]
pub struct PrefaceReader {
    state: PrefaceState,
    /// Bytes of the input consumed: the magic and the frames parsed so far; `0` before the magic.
    offset: usize,
    /// Frames parsed so far.
    frames: usize,
    magic_seen: bool,
    finished: bool,
}

impl PrefaceReader {
    /// A reader that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many frames the reader has parsed. A frame counts once, however many calls it took
    /// to arrive.
    #[must_use]
    pub fn frames_read(&self) -> usize {
        self.frames
    }

    /// How many bytes of the input the reader has consumed: the magic and the frames parsed.
    #[must_use]
    pub fn bytes_read(&self) -> usize {
        self.offset
    }

    /// Continues reading `bytes`, the whole buffer received so far.
    ///
    /// Returns `Ok(None)` when `bytes` ends before the first request is complete; call it again
    /// with the longer buffer.
    ///
    /// # Errors
    ///
    /// Everything [`read_preface`](super::read_preface) returns, and
    /// [`H2CaptureError::ReaderFinished`] once the reader has returned a result or an error.
    pub fn read(&mut self, bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError> {
        if self.finished {
            return Err(H2CaptureError::ReaderFinished);
        }
        let outcome = self.advance(bytes);
        self.finished = !matches!(outcome, Ok(None));
        outcome
    }

    fn advance(&mut self, bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError> {
        if !self.magic_seen {
            if !check_magic(bytes)? {
                return Ok(None);
            }
            self.magic_seen = true;
            self.offset = CLIENT_PREFACE.len();
        }
        loop {
            if self.offset > MAX_PREFACE_BYTES {
                return Err(H2CaptureError::PrefaceTooLarge);
            }
            let rest = bytes.get(self.offset..).unwrap_or_default();
            let Some((frame, used)) = read_frame(rest, DEFAULT_MAX_FRAME_PAYLOAD)? else {
                return Ok(None);
            };
            if self.frames >= MAX_PREFACE_FRAMES {
                return Err(H2CaptureError::TooManyFrames {
                    max: MAX_PREFACE_FRAMES,
                });
            }
            self.frames += 1;
            self.offset += used;
            if let Some(block) = self.state.accept(frame)? {
                let state = std::mem::take(&mut self.state);
                return state.finish(&block, self.offset).map(Some);
            }
        }
    }
}

impl fmt::Debug for PrefaceReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrefaceReader")
            .field("bytes_read", &self.offset)
            .field("frames_read", &self.frames)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}
