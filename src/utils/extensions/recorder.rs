//! The browser extension that records a person's actions.

/// The "SeleniumBase Recorder" extension, as a zip archive.
///
/// A manifest v3 extension whose content script runs on every page and saves
/// what the person does (clicks, typing, selections, drags) to
/// `sessionStorage` under `recorded_actions`, as `[kind, selector, value,
/// time]` lists with CSS selectors. Unpack it with
/// [`extract_extension`](super::extract_extension) to load it into Chrome.
///
/// For a recorder that needs no extension, see
/// [`RECORDER_JS`](crate::evasion::js::recorder_js::RECORDER_JS).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::RECORDER_BYTES;
///
/// assert!(RECORDER_BYTES.starts_with(b"PK"));
/// ```
pub const RECORDER_BYTES: &[u8] = include_bytes!("assets/recorder.zip");
