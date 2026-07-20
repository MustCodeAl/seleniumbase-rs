//! # SeleniumBase for Rust
//!
//! A Rust port of the Python SeleniumBase testing framework. It provides a
//! `BaseCase` API for browser automation, stealth/undetected modes via CDP,
//! a command-line helper (`sbase`), and supporting modules for configuration,
//! reporting, BDD, and more.
//!
//! ## Quick start
//!
//! ```no_run
//! use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = BrowserConfig::default().with_mode(DriverMode::Uc);
//!     let mut sb = BaseCase::new(config).await?;
//!     sb.open("https://example.com").await?;
//!     sb.assert_title("Example Domain").await?;
//!     sb.quit().await?;
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod artifacts;
pub mod behave;
pub mod browser;
pub mod cli;
pub mod common;
pub mod config;
pub mod core;
pub mod error;
pub mod js_code;
pub mod plugins;
pub mod resources;
pub mod stealth;
pub mod utilities;
pub mod utils;

pub use api::base_case::BaseCase;
pub use api::chart::{Chart, ChartType};
pub use api::gui::Gui;
pub use api::tour::TourTheme;
pub use browser::config::{Browser, BrowserConfig, DriverMode};
pub use browser::session::BrowserSession;
pub use error::SeleniumBaseError;
