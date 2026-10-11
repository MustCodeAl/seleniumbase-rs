//! Browser lifecycle: configuration, driver download/launch, session management,
//! and connection handling for WebDriver and CDP/UC modes.

pub mod binaries;
pub mod config;
pub mod downloader;
pub mod driver_access;
pub mod driver_version;
pub mod launcher;
pub mod options;
pub mod session;

#[cfg(feature = "playwright")]
pub mod playwright;
