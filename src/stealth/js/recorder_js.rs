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
/// use seleniumbase_rs::stealth::js::recorder_js::get_recorder_js;
///
/// assert!(get_recorder_js().contains("recorded_actions"));
/// ```
#[must_use]
pub fn get_recorder_js() -> &'static str {
    crate::api::recorder::RECORDER_JS
}
