//! The script that records what happens in a page.

/// The Recorder Mode page script, [`RECORDER_JS`](crate::api::recorder::RECORDER_JS).
///
/// This was a smaller script of its own (`window.recordedActions`); since
/// Recorder Mode is the Python recorder it returns that one. It records what
/// a person does as `[kind, a, b, time]` actions in sessionStorage
/// `recorded_actions` (see [`RawAction`](crate::api::recorder::RawAction)).
/// It is written as the body of a function, the form WebDriver's Execute
/// Script takes.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::evasion::js::recorder_js::RECORDER_JS;
///
/// assert!(RECORDER_JS.contains("recorded_actions"));
/// ```
pub const RECORDER_JS: &str = crate::api::recorder::RECORDER_JS;
