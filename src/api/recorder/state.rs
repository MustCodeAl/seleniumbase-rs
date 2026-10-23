//! What [`BaseCase`](crate::BaseCase) keeps while Recorder Mode is on.

use std::collections::{BTreeMap, HashSet};

use super::output::RecordingTarget;
use super::RawAction;

/// Why [`BaseCase::wait_for_recorder_end`](crate::BaseCase::wait_for_recorder_end)
/// returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecorderEnd {
    /// The person pressed SHIFT+ESC in the browser (`mkrec --ee`).
    ShiftEsc,
    /// Every browser window was closed.
    NoWindowLeft,
    /// The time limit passed.
    TimeLimit,
}

/// The state of a recording: what is needed to harvest it when the case ends.
#[derive(Debug, Default)]
pub(crate) struct RecorderState {
    /// Recorder Mode is on: by the configuration, or by `activate_recorder`.
    pub on: bool,
    /// The recording was harvested and written (it happens once).
    pub finished: bool,
    /// The browser's `Date.now()` when recording began; older actions are
    /// dropped.
    pub start_ms: i64,
    /// The browser's clock minus the local one, in milliseconds, so the log
    /// of `BaseCase` is stamped in browser time (a remote Grid's clock can
    /// differ).
    pub clock_offset_ms: i64,
    /// Raw actions of tabs saved by `save_recorded_actions`.
    pub stash: Vec<RawAction>,
    /// The origins of the stashed tabs.
    pub stash_origins: Vec<String>,
    /// The last actions seen on each tab (by window handle), so closing the
    /// browser does not lose them.
    pub cache: BTreeMap<String, Vec<RawAction>>,
    /// The windows that got the page script for their next documents.
    pub registered: HashSet<String>,
    /// How many windows the last look at the browser found.
    pub window_count: usize,
    /// The names of the output, when the test gave them.
    pub target: Option<RecordingTarget>,
}
