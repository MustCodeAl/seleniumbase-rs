//! External browser profile profile parameters for anti-detect browser automation.
//!
//! This module models the JSON payload used by external anti-detect browser profile
//! `POST /profile/create` endpoint and provides helpers to translate those
//! parameters into [`BrowserConfig`] and runtime [`BaseCase`] actions supported
//! by `seleniumbase-rs`.
//!
//! # Example
//!
//! ```ignore
//! use seleniumbase_rs::profile_payloads::ProfileParams;
//! use serde_json::json;
//!
//! let params: ProfileParams = serde_json::from_value(json!({
//!     "name": "Windows Chromium",
//!     "browser_type": "chromium",
//!     "os_type": "windows",
//!     "parameters": {
//!         "fingerprint": {
//!             "navigator": {
//!                 "user_agent": "Mozilla/5.0 ...",
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
//! ```

mod profile;

pub use profile::*;
