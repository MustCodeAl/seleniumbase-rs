//! Browser profile payloads, in the JSON shape anti-detect browser tools use.
//!
//! [`ProfileParams`] models the body such a tool posts to its
//! `POST /profile/create` endpoint. Its methods turn the payload into what
//! this crate runs on: a [`BrowserConfig`] to launch a browser with, a
//! [`Fingerprint`] to present, and the screen size and location to apply to a
//! running [`BaseCase`] with
//! [`apply_runtime_overrides`](ProfileParams::apply_runtime_overrides).
//!
//! A payload can come from outside the program, so the conversion drops what
//! would be unsafe to honour: Chrome switches that run programs, load code or
//! reroute traffic, start pages that are not web pages, and a `folder_id` that
//! would leave `./profile-data`.
//!
//! [`BrowserConfig`]: crate::browser::config::BrowserConfig
//! [`Fingerprint`]: crate::stealth::fingerprint::Fingerprint
//! [`BaseCase`]: crate::BaseCase
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::profile_payloads::ProfileParams;
//! use serde_json::json;
//!
//! let params: ProfileParams = serde_json::from_value(json!({
//!     "name": "Windows Chromium",
//!     "browser_type": "chromium",
//!     "os_type": "windows",
//!     "folder_id": "work",
//!     "parameters": {
//!         "fingerprint": {
//!             "navigator": {
//!                 "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
//!                 "platform": "Win32",
//!                 "hardware_concurrency": 8
//!             },
//!             "screen": { "width": 1920, "height": 1200, "pixel_ratio": 1 },
//!             "geolocation": { "latitude": 52.02, "longitude": -52.1, "accuracy": 100 }
//!         }
//!     }
//! }))?;
//!
//! let config = params.to_browser_config("http://localhost:4444");
//! assert_eq!(config.webdriver_url, "http://localhost:4444");
//! assert_eq!(
//!     config.user_agent.as_deref(),
//!     Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
//! );
//! assert_eq!(config.user_data_dir.as_deref(), Some("./profile-data/work"));
//! # Ok::<(), serde_json::Error>(())
//! ```

mod profile;

pub use profile::*;

/// The old name of [`ProfileFlags`], which was easy to mistake for
/// [`StealthFlags`](crate::stealth::fingerprint::StealthFlags).
#[deprecated(note = "renamed ProfileFlags, so it is not mistaken for StealthFlags")]
pub type Flags = ProfileFlags;

/// The old name of [`ProfileFingerprint`], which was easy to mistake for the
/// [`Fingerprint`](crate::stealth::fingerprint::Fingerprint) it becomes.
#[deprecated(note = "renamed ProfileFingerprint, so it is not mistaken for Fingerprint")]
pub type Fingerprint = ProfileFingerprint;

/// The old name of [`ProfileProxy`], which was easy to mistake for the
/// [`ProxyConfig`](crate::stealth::fingerprint::ProxyConfig) it becomes.
#[deprecated(note = "renamed ProfileProxy, so it is not mistaken for ProxyConfig")]
pub type ProxyConfig = ProfileProxy;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated, reason = "the test is that the old names still work")]
    fn the_old_names_are_the_new_types() {
        let flags: Flags = ProfileFlags::default();
        let fingerprint: Fingerprint = ProfileFingerprint::default();
        let proxy: ProxyConfig = ProfileProxy::default();
        assert_eq!(flags.webrtc_masking, "mask");
        assert!(fingerprint.navigator.is_none());
        assert_eq!(proxy.port, 0);
    }
}
