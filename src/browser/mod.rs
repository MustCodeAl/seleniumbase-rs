//! Browser lifecycle: configuration, driver download/launch, session management,
//! and connection handling for WebDriver and CDP/UC modes.

pub mod config;
pub mod downloader;
pub mod driver_access;
pub mod launcher;
pub mod session;

#[cfg(feature = "playwright")]
pub mod playwright;
