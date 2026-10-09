//! Hooks that run around a test and the helpers that go with them.
//!
//! See [`observer`] for how plugins are attached to a test run.

pub mod driver_manager;
pub mod observer;
pub mod page_source;
pub mod reports;
pub mod screen_shots;

#[cfg(feature = "s3")]
pub mod s3_logging_plugin;

#[cfg(feature = "azure")]
pub mod azure_logging_plugin;

#[cfg(feature = "gcp")]
pub mod gcp_logging_plugin;
