//! Browser lifecycle: configuration, driver download/launch, session management,
//! and connection handling for WebDriver and CDP/UC modes.

pub mod binaries;
pub mod chrome_for_testing;
pub mod config;
pub mod downloader;
pub mod driver_access;
pub mod driver_install;
pub mod driver_version;
pub mod launcher;
pub mod options;
pub mod reuse;
pub mod session;
pub mod virtual_display;

#[cfg(feature = "playwright")]
pub mod playwright;
