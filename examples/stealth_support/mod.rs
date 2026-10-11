//! What the stealth examples share. This folder is a module of theirs, not an example: Cargo only
//! builds `examples/*.rs` and `examples/*/main.rs`, and this folder has neither a `main.rs` nor
//! an entry in `Cargo.toml`.
//!
//! Every example declares it with `mod stealth_support;` and uses the part it needs.
#![allow(
    dead_code,
    reason = "each example uses a part of this module, so the rest looks unused to it"
)]

use std::time::{SystemTime, UNIX_EPOCH};

use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation;
use seleniumbase_rs::wire::BrowserKind;

#[cfg(feature = "impersonate")]
pub mod shop;

/// The operating system this machine runs, as the persona of a real browser on it.
pub fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// The seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The system of this machine and the Chrome major version that was measured on it.
///
/// The measured tables hold one entry, Chrome 155 on macOS. On any other system this fails with a
/// message that names the missing measurement. There is no fallback to the values of another
/// system: a Chrome 155 handshake under a Windows user agent is a contradiction a detector can
/// see.
pub fn measured_chrome() -> Result<(OsType, u32), String> {
    let os = host_os();
    match emulation::get(BrowserKind::Chrome, os) {
        Ok(entry) => Ok((os, entry.major)),
        Err(error) => Err(format!(
            "{error}.\nThis example needs a measured system, and only Chrome 155 on macOS is \
             measured today. See \"Re-measuring when Chrome updates\" in \
             docs/tutorials/stealth_scraping.md."
        )),
    }
}
