pub mod artifacts;
pub mod cdp;
pub mod config;
pub mod core;
pub mod dashboard;
pub mod error;
pub mod fixtures;
pub mod packages;
pub mod patcher;
pub mod recorder;
pub mod scenario;
pub mod uc;

pub use config::{Browser, BrowserConfig, DriverMode};
pub use core::session::BrowserSession;
pub use error::SeleniumBaseError;
pub use fixtures::base_case::BaseCase;
pub use thirtyfour::extensions::cdp::NetworkConditions;
