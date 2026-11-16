//! The script that reads what a page can see of the browser.

/// The capture script, written as the body of an `async` function.
///
/// It reads the navigator, the Client Hints, the screen and window sizes, the
/// time zone, the WebGL strings, the media devices, the battery, the network
/// connection, the speech voices, Bluetooth, the permission states, a canvas
/// hash and an audio hash, and the markers a headless-browser check looks for,
/// and returns them as one object. Each reading is guarded on its own: a reading
/// that throws or never answers is reported under `errors` and costs nothing
/// else. It asks for no permission and writes nothing to the page.
///
/// The body awaits, so it must run inside an `async` function. The two engines
/// wrap it differently:
/// [`capture_expression`](crate::evasion::capture::capture_expression) for the
/// pure CDP engine and
/// [`capture_async_script`](crate::evasion::capture::capture_async_script) for
/// WebDriver. [`CapturedFingerprint::from_probe`](crate::evasion::capture::CapturedFingerprint::from_probe)
/// reads what it returns.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::evasion::js::capture_js::CAPTURE_JS;
///
/// assert!(CAPTURE_JS.contains("getHighEntropyValues"));
/// ```
pub const CAPTURE_JS: &str = include_str!("capture.js");
