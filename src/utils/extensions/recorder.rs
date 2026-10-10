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
/// [`get_recorder_js`](crate::stealth::js::recorder_js::get_recorder_js).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::get_recorder_bytes;
///
/// assert!(get_recorder_bytes().starts_with(b"PK"));
/// ```
#[must_use]
pub fn get_recorder_bytes() -> &'static [u8] {
    include_bytes!("assets/recorder.zip")
}
