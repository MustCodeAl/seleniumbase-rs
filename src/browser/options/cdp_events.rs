//! Capturing Chrome DevTools Protocol events during a WebDriver session.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// When a Chromium session keeps a log of the DevTools Protocol events its page
/// produces (network requests, page lifecycle, console).
///
/// Python SeleniumBase calls this `--log-cdp-events` (also `--log-cdp`) for any
/// Chromium session and `--uc-cdp-events` (also `--uc-cdp`) for undetected
/// (UC) sessions only. Read the log back with
/// [`BrowserSession::cdp_events`](crate::BrowserSession::cdp_events).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::CdpEventCapture;
///
/// assert!(!CdpEventCapture::Off.captures(true));
/// assert!(CdpEventCapture::UndetectedOnly.captures(true));
/// assert!(!CdpEventCapture::UndetectedOnly.captures(false));
/// assert!(CdpEventCapture::Always.captures(false));
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CdpEventCapture {
    /// Capture nothing.
    #[default]
    Off,
    /// Capture in undetected (UC) mode only.
    UndetectedOnly,
    /// Capture in every Chromium session.
    Always,
}

impl CdpEventCapture {
    /// Whether a session captures events, given whether it runs in UC mode.
    #[must_use]
    pub const fn captures(self, undetected: bool) -> bool {
        match self {
            Self::Off => false,
            Self::UndetectedOnly => undetected,
            Self::Always => true,
        }
    }

    /// Whether nothing is captured; serde skips the field when it holds.
    #[must_use]
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

/// One DevTools Protocol event from the `performance` log.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::CdpEvent;
///
/// let entry = r#"{"message":{"method":"Page.loadEventFired","params":{"timestamp":1.5}},"webview":"W1"}"#;
/// let event = CdpEvent::from_log_message(entry, 1_700_000_000_000).unwrap();
/// assert_eq!(event.method, "Page.loadEventFired");
/// assert_eq!(event.webview.as_deref(), Some("W1"));
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CdpEvent {
    /// The event's name, such as `Network.requestWillBeSent`.
    pub method: String,
    /// The event's parameters.
    pub params: Value,
    /// The id of the page (target) that produced it, when the driver says.
    pub webview: Option<String>,
    /// When the driver logged it, in milliseconds since the Unix epoch.
    pub timestamp_ms: i64,
}

impl CdpEvent {
    /// Reads the `message` of a `performance` log entry, which chromedriver
    /// writes as JSON text. Returns `None` for a message that is not an event.
    #[must_use]
    pub fn from_log_message(message: &str, timestamp_ms: i64) -> Option<Self> {
        let outer: Value = serde_json::from_str(message).ok()?;
        let inner = outer.get("message")?;
        Some(Self {
            method: inner.get("method")?.as_str()?.to_owned(),
            params: inner.get("params").cloned().unwrap_or(Value::Null),
            webview: outer
                .get("webview")
                .and_then(Value::as_str)
                .map(str::to_owned),
            timestamp_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn capture_depends_on_the_mode_only_for_the_uc_variant() {
        for undetected in [false, true] {
            assert!(!CdpEventCapture::Off.captures(undetected));
            assert!(CdpEventCapture::Always.captures(undetected));
        }
        assert!(CdpEventCapture::UndetectedOnly.captures(true));
        assert!(!CdpEventCapture::UndetectedOnly.captures(false));
        assert_eq!(CdpEventCapture::default(), CdpEventCapture::Off);
    }

    #[test]
    fn an_event_is_read_from_a_chromedriver_log_message() {
        let message = r#"{"message":{"method":"Network.requestWillBeSent","params":{"requestId":"1.2","request":{"url":"https://example.com/"}}},"webview":"ABCDEF"}"#;
        let event = CdpEvent::from_log_message(message, 42).unwrap();
        assert_eq!(event.method, "Network.requestWillBeSent");
        assert_eq!(
            event.params["request"]["url"],
            json!("https://example.com/")
        );
        assert_eq!(event.webview.as_deref(), Some("ABCDEF"));
        assert_eq!(event.timestamp_ms, 42);
    }

    #[test]
    fn an_event_without_params_or_webview_still_reads() {
        let event = CdpEvent::from_log_message(r#"{"message":{"method":"Inspector.detached"}}"#, 0)
            .unwrap();
        assert_eq!(event.params, Value::Null);
        assert_eq!(event.webview, None);
    }

    #[test]
    fn a_log_message_that_is_not_an_event_is_skipped() {
        for message in [
            "",
            "not json",
            "[]",
            r#"{"message": "text"}"#,
            r#"{"message": {"params": {}}}"#,
            r#"{"message": {"method": 5}}"#,
        ] {
            assert_eq!(CdpEvent::from_log_message(message, 0), None, "{message:?}");
        }
    }
}
