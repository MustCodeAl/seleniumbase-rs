//! The HTTP/2 specification a client should reproduce: SETTINGS, connection window, priority and HEADERS priority.
//!
//! [`H2Spec`] describes the start of an HTTP/2 connection: the SETTINGS entries and their
//! order, the connection `WINDOW_UPDATE`, the PRIORITY frames, the priority fields of the
//! first HEADERS frame, and the order of the pseudo-headers. It is built from bytes captured
//! from a real client ([`H2Spec::from_capture`], [`H2Spec::from_preface`]), from an Akamai
//! fingerprint string ([`H2Spec::from_akamai`]), or by overlaying such a string on a measured
//! base ([`H2Spec::with_akamai`]).
//!
//! Recording a capture and judging it are separate steps. `from_preface` records whatever
//! the client sent, GREASE settings included. [`H2Spec::check_reproducible`] then says
//! whether the `wreq` HTTP/2 stack can send it, and names what it cannot in a [`SpecError`]:
//! the check runs before any client exists, so a spec that cannot be sent is never half
//! applied. What the stack cannot emit, as of `wreq-proto` 0.2.6:
//!
//! - SETTINGS other than ids 1 to 6, 8 and 9 (experimental and GREASE settings);
//! - a SETTINGS id twice (the stack's order builder drops the second silently);
//! - leaving out `SETTINGS_INITIAL_WINDOW_SIZE`, which the stack always sends;
//! - two PRIORITY frames for one stream, or one for stream 0 (both are dropped silently).
//!
//! ```
//! use seleniumbase_rs::wire::akamai::Akamai;
//! use seleniumbase_rs::wire::h2_spec::{H2Spec, SpecError};
//!
//! let akamai: Akamai = "1:4096;2:0;4:1048576|983041|0|m,a,s,p".parse()?;
//! let spec = H2Spec::from_akamai(&akamai)?;
//! assert_eq!(spec.akamai(), akamai);
//!
//! let grease: Akamai = "2570:1;4:1048576|0|0|m,a,s,p".parse()?;
//! assert_eq!(H2Spec::from_akamai(&grease), Err(SpecError::UnsupportedSetting(2570)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::akamai::{Akamai, PriorityFrame, Pseudo, MAX_STREAM_ID, MAX_WEIGHT, MIN_WEIGHT};
use super::h2_frames::{read_preface, H2CaptureError, H2Preface, HeadersPriority};

/// SETTINGS identifiers (RFC 9113 section 6.5.2, RFC 8441 and RFC 9218).
pub mod setting_id {
    /// `SETTINGS_HEADER_TABLE_SIZE`.
    pub const HEADER_TABLE_SIZE: u16 = 1;
    /// `SETTINGS_ENABLE_PUSH`.
    pub const ENABLE_PUSH: u16 = 2;
    /// `SETTINGS_MAX_CONCURRENT_STREAMS`.
    pub const MAX_CONCURRENT_STREAMS: u16 = 3;
    /// `SETTINGS_INITIAL_WINDOW_SIZE`.
    pub const INITIAL_WINDOW_SIZE: u16 = 4;
    /// `SETTINGS_MAX_FRAME_SIZE`.
    pub const MAX_FRAME_SIZE: u16 = 5;
    /// `SETTINGS_MAX_HEADER_LIST_SIZE`.
    pub const MAX_HEADER_LIST_SIZE: u16 = 6;
    /// `SETTINGS_ENABLE_CONNECT_PROTOCOL`.
    pub const ENABLE_CONNECT_PROTOCOL: u16 = 8;
    /// `SETTINGS_NO_RFC7540_PRIORITIES`.
    pub const NO_RFC7540_PRIORITIES: u16 = 9;
}

/// The SETTINGS ids the `wreq` HTTP/2 stack can send.
pub const EMITTABLE_SETTINGS: [u16; 8] = [
    setting_id::HEADER_TABLE_SIZE,
    setting_id::ENABLE_PUSH,
    setting_id::MAX_CONCURRENT_STREAMS,
    setting_id::INITIAL_WINDOW_SIZE,
    setting_id::MAX_FRAME_SIZE,
    setting_id::MAX_HEADER_LIST_SIZE,
    setting_id::ENABLE_CONNECT_PROTOCOL,
    setting_id::NO_RFC7540_PRIORITIES,
];

/// The SETTINGS ids the `wreq` HTTP/2 stack sends whatever it is configured with.
///
/// Read from `wreq-proto` 0.2.6: `Http2Options::initial_window_size` is a plain number that is
/// always passed on to the connection, so `SETTINGS_INITIAL_WINDOW_SIZE` is always in the
/// frame. The capability probe of the client package is the final authority on this list.
pub const ALWAYS_SENT_SETTINGS: [u16; 1] = [setting_id::INITIAL_WINDOW_SIZE];

/// The connection flow-control window before any `WINDOW_UPDATE`.
pub const INITIAL_CONNECTION_WINDOW: u32 = 65_535;
/// The largest connection `WINDOW_UPDATE` increment: the window may not pass 2^31 - 1.
pub const MAX_WINDOW_UPDATE: u32 = MAX_STREAM_ID - INITIAL_CONNECTION_WINDOW;
/// The smallest legal `SETTINGS_MAX_FRAME_SIZE`.
pub const MIN_MAX_FRAME_SIZE: u32 = 16_384;
/// The largest legal `SETTINGS_MAX_FRAME_SIZE`.
pub const MAX_MAX_FRAME_SIZE: u32 = 16_777_215;
/// The most PRIORITY frames a spec may carry.
pub const MAX_SPEC_PRIORITY_FRAMES: usize = 64;
/// The stream id of a client's first request unless it says otherwise.
const DEFAULT_INITIAL_STREAM: u32 = 1;

/// Why an HTTP/2 spec cannot be built or cannot be sent.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SpecError {
    /// A SETTINGS id the HTTP/2 stack cannot send, such as a GREASE or experimental id.
    #[error("SETTINGS id {0} cannot be sent by the HTTP/2 stack (experimental and GREASE ids are unsupported)")]
    UnsupportedSetting(u16),
    /// A SETTINGS id appears twice.
    #[error("SETTINGS id {0} appears more than once")]
    DuplicateSetting(u16),
    /// A SETTINGS value outside the range RFC 9113 allows for its id.
    #[error("SETTINGS id {id} cannot have the value {value}")]
    SettingValue {
        /// The SETTINGS id.
        id: u16,
        /// The refused value.
        value: u32,
    },
    /// A SETTINGS id the stack always sends is missing, so the spec could not be matched.
    #[error("SETTINGS id {0} is always sent by the HTTP/2 stack and cannot be left out")]
    SettingAlwaysSent(u16),
    /// A `WINDOW_UPDATE` increment that would push the window past 2^31 - 1.
    #[error("WINDOW_UPDATE increment {0} would take the connection window past 2147483647")]
    WindowUpdate(u32),
    /// A PRIORITY frame that is invalid or that the stack would drop.
    #[error("PRIORITY frame for stream {stream}: {reason}")]
    Priority {
        /// The stream of the frame.
        stream: u32,
        /// What is wrong.
        reason: &'static str,
    },
    /// Priority fields on HEADERS that are out of range.
    #[error("HEADERS priority: {reason}")]
    HeadersPriority {
        /// What is wrong.
        reason: &'static str,
    },
    /// The pseudo-header order is not a permutation of the four request pseudo-headers.
    #[error(
        "the pseudo-header order is not a permutation of :method, :authority, :scheme and :path"
    )]
    PseudoOrder,
    /// An initial stream id that is not a valid client stream id.
    #[error("initial stream id {0} is not an odd id below 2147483648")]
    InitialStreamId(u32),
    /// The captured bytes stop before the first request is complete.
    #[error("the captured bytes end before the first request is complete")]
    Incomplete,
    /// The captured bytes are not a valid HTTP/2 client start.
    #[error("the captured bytes are not a valid HTTP/2 client start: {0}")]
    Capture(#[from] H2CaptureError),
}

/// A whole HTTP/2 profile: what a client sends before and in its first request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct H2Spec {
    /// SETTINGS entries `(id, value)` in wire order.
    pub settings: Vec<(u16, u32)>,
    /// The connection-level `WINDOW_UPDATE` increment; `0` means no frame.
    pub window_update: u32,
    /// PRIORITY frames sent before the first request, in order.
    #[serde(default)]
    pub priority_frames: Vec<PriorityFrame>,
    /// The priority fields of the first HEADERS frame, when the client sends them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers_priority: Option<HeadersPriority>,
    /// The order of the request pseudo-headers.
    pub pseudo_order: [Pseudo; 4],
    /// The stream id of the first request, when it is not the usual `1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_stream_id: Option<u32>,
}

/// The same type under the name the design plan uses.
pub type Http2Spec = H2Spec;

impl H2Spec {
    /// Records a captured connection start. Nothing is judged here.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError::PseudoOrder`] when the request does not carry the four
    /// pseudo-headers exactly once and before its other headers.
    pub fn from_preface(preface: &H2Preface) -> Result<Self, SpecError> {
        let pseudo_order = preface
            .pseudo_order()
            .map_err(|_malformed| SpecError::PseudoOrder)?;
        Ok(Self {
            settings: preface.settings.clone(),
            window_update: preface.window_update,
            priority_frames: preface.priority_frames.clone(),
            headers_priority: preface.headers_priority,
            pseudo_order,
            initial_stream_id: (preface.request_stream_id != DEFAULT_INITIAL_STREAM)
                .then_some(preface.request_stream_id),
        })
    }

    /// Reads a captured connection start from raw client bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError::Incomplete`] when the bytes end before the first request is
    /// complete, [`SpecError::Capture`] when they are not a valid client start, and
    /// [`SpecError::PseudoOrder`] as [`H2Spec::from_preface`] does.
    pub fn from_capture(bytes: &[u8]) -> Result<Self, SpecError> {
        let preface = read_preface(bytes)?.ok_or(SpecError::Incomplete)?;
        Self::from_preface(&preface)
    }

    /// A spec from an Akamai fingerprint alone, checked to be sendable.
    ///
    /// The string says nothing about the priority fields of HEADERS, so none are set.
    ///
    /// # Errors
    ///
    /// Returns the [`SpecError`] that [`H2Spec::check_reproducible`] reports.
    pub fn from_akamai(akamai: &Akamai) -> Result<Self, SpecError> {
        let spec = Self::from_akamai_parts(akamai, None);
        spec.check_reproducible()?;
        Ok(spec)
    }

    /// Overlays an Akamai fingerprint on this spec.
    ///
    /// The string replaces the SETTINGS, the connection window, the PRIORITY frames and the
    /// pseudo-header order, and the first request's stream id is recomputed to come after the
    /// streams the PRIORITY frames name. What the string does not carry, the HEADERS priority
    /// fields, is kept from this spec.
    ///
    /// # Errors
    ///
    /// Returns the [`SpecError`] that [`H2Spec::check_reproducible`] reports for the result.
    pub fn with_akamai(&self, akamai: &Akamai) -> Result<Self, SpecError> {
        let spec = Self::from_akamai_parts(akamai, self.headers_priority);
        spec.check_reproducible()?;
        Ok(spec)
    }

    /// Checks that the `wreq` HTTP/2 stack can send exactly this spec.
    ///
    /// # Errors
    ///
    /// Returns the first problem found, naming the setting, frame or field.
    pub fn check_reproducible(&self) -> Result<(), SpecError> {
        check_settings(&self.settings)?;
        if self.window_update > MAX_WINDOW_UPDATE {
            return Err(SpecError::WindowUpdate(self.window_update));
        }
        check_priority_frames(&self.priority_frames)?;
        if let Some(priority) = &self.headers_priority {
            check_headers_priority(priority)?;
        }
        if !Pseudo::ALL.iter().all(|p| self.pseudo_order.contains(p)) {
            return Err(SpecError::PseudoOrder);
        }
        match self.initial_stream_id {
            Some(id) if id % 2 == 0 || id > MAX_STREAM_ID => Err(SpecError::InitialStreamId(id)),
            _ => Ok(()),
        }
    }

    /// The Akamai fingerprint of this spec.
    #[must_use]
    pub fn akamai(&self) -> Akamai {
        Akamai {
            settings: self.settings.clone(),
            window_update: self.window_update,
            priority_frames: self.priority_frames.clone(),
            pseudo_order: self.pseudo_order,
        }
    }

    /// The value of the SETTINGS entry with this id, when the spec has one.
    #[must_use]
    pub fn setting(&self, id: u16) -> Option<u32> {
        self.settings
            .iter()
            .find(|(entry, _)| *entry == id)
            .map(|(_, value)| *value)
    }

    /// This spec without the SETTINGS the stack cannot send, and the entries removed.
    ///
    /// For measured specs that include a GREASE setting: the dropped entries are the known
    /// gap between the capture and what a client can reproduce.
    #[must_use]
    pub fn without_unsupported_settings(&self) -> (Self, Vec<(u16, u32)>) {
        let (kept, dropped) = self
            .settings
            .iter()
            .partition(|(id, _)| EMITTABLE_SETTINGS.contains(id));
        let spec = Self {
            settings: kept,
            ..self.clone()
        };
        (spec, dropped)
    }

    /// The spec an Akamai string describes, with the first request after its PRIORITY streams.
    fn from_akamai_parts(akamai: &Akamai, headers_priority: Option<HeadersPriority>) -> Self {
        let after_priority_streams = akamai
            .priority_frames
            .iter()
            .map(|frame| frame.stream_id)
            .max()
            .map(|highest| highest.saturating_add(2));
        Self {
            settings: akamai.settings.clone(),
            window_update: akamai.window_update,
            priority_frames: akamai.priority_frames.clone(),
            headers_priority,
            pseudo_order: akamai.pseudo_order,
            initial_stream_id: after_priority_streams,
        }
    }
}

fn check_settings(settings: &[(u16, u32)]) -> Result<(), SpecError> {
    let mut seen: Vec<u16> = Vec::with_capacity(EMITTABLE_SETTINGS.len());
    for &(id, value) in settings {
        if !EMITTABLE_SETTINGS.contains(&id) {
            return Err(SpecError::UnsupportedSetting(id));
        }
        if seen.contains(&id) {
            return Err(SpecError::DuplicateSetting(id));
        }
        seen.push(id);
        if !setting_value_is_legal(id, value) {
            return Err(SpecError::SettingValue { id, value });
        }
    }
    match ALWAYS_SENT_SETTINGS.iter().find(|id| !seen.contains(id)) {
        Some(missing) => Err(SpecError::SettingAlwaysSent(*missing)),
        None => Ok(()),
    }
}

/// The value ranges of RFC 9113 section 6.5.2, RFC 8441 and RFC 9218.
fn setting_value_is_legal(id: u16, value: u32) -> bool {
    match id {
        setting_id::ENABLE_PUSH
        | setting_id::ENABLE_CONNECT_PROTOCOL
        | setting_id::NO_RFC7540_PRIORITIES => value <= 1,
        setting_id::INITIAL_WINDOW_SIZE => value <= MAX_STREAM_ID,
        setting_id::MAX_FRAME_SIZE => (MIN_MAX_FRAME_SIZE..=MAX_MAX_FRAME_SIZE).contains(&value),
        _ => true,
    }
}

fn check_priority_frames(frames: &[PriorityFrame]) -> Result<(), SpecError> {
    let mut seen: Vec<u32> = Vec::with_capacity(frames.len().min(MAX_SPEC_PRIORITY_FRAMES));
    for frame in frames {
        let stream = frame.stream_id;
        let problem = if seen.len() >= MAX_SPEC_PRIORITY_FRAMES {
            Some("too many PRIORITY frames")
        } else if seen.contains(&stream) {
            Some("more than one PRIORITY frame for the stream")
        } else {
            priority_problem(frame)
        };
        if let Some(reason) = problem {
            return Err(SpecError::Priority { stream, reason });
        }
        seen.push(stream);
    }
    Ok(())
}

/// What is wrong with one PRIORITY frame on its own, if anything.
fn priority_problem(frame: &PriorityFrame) -> Option<&'static str> {
    if frame.stream_id == 0 {
        Some("stream id 0 is the connection")
    } else if frame.stream_id > MAX_STREAM_ID {
        Some("stream id above 2^31 - 1")
    } else if frame.stream_id % 2 == 0 {
        Some("client streams have odd ids")
    } else if frame.depends_on > MAX_STREAM_ID {
        Some("dependency above 2^31 - 1")
    } else if frame.depends_on == frame.stream_id {
        Some("a stream cannot depend on itself")
    } else if !(MIN_WEIGHT..=MAX_WEIGHT).contains(&frame.weight) {
        Some("weight outside 1 to 256")
    } else {
        None
    }
}

fn check_headers_priority(priority: &HeadersPriority) -> Result<(), SpecError> {
    let reason = if !(MIN_WEIGHT..=MAX_WEIGHT).contains(&priority.weight) {
        "weight outside 1 to 256"
    } else if priority.depends_on > MAX_STREAM_ID {
        "dependency above 2^31 - 1"
    } else {
        return Ok(());
    };
    Err(SpecError::HeadersPriority { reason })
}

#[cfg(test)]
mod tests;
