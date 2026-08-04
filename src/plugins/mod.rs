pub mod base_plugin;
pub mod db_reporting_plugin;
pub mod driver_manager;
pub mod page_source;
pub mod sb_manager;
pub mod screen_shots;

#[cfg(feature = "s3")]
pub mod s3_logging_plugin;

#[cfg(feature = "azure")]
pub mod azure_logging_plugin;

#[cfg(feature = "gcp")]
pub mod gcp_logging_plugin;
