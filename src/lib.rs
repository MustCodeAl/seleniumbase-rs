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
//!
//! ## Helpers
//!
//! Convenience macros such as [`selector!`], [`sb_test!`], [`sb_open!`],
//! [`sb_click!`], [`sb_type!`], [`sb_wait_for!`], [`fingerprint!`], and
//! [`uc_config!`] live in the [`macros`] module. Fingerprint presets include
//! `windows_desktop`, `macos_desktop`, `linux_desktop`, `android_mobile`, and
//! `ios_mobile_safari`.

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
pub mod macros;
pub mod plugins;
pub mod profile_payloads;
pub mod resources;
pub mod stealth;
pub mod tracing_util;
pub mod utilities;
pub mod utils;

pub use api::base_case::BaseCase;
pub use api::chart::{Chart, ChartType};
pub use api::gui::Gui;
pub use api::runner::{run_browser_test, BrowserTestFuture};
pub use api::tour::TourTheme;
pub use api::traits::{AssertionApi, BrowserApi, ElementApi, ScreenshotApi};
pub use browser::config::{Browser, BrowserConfig, DriverMode};
pub use browser::session::BrowserSession;
pub use config::{LogFormat, RuntimeConfig};
pub use error::{Result, ResultExt, SeleniumBaseError};
pub use stealth::fingerprint::{
    BatteryProfile, BrandVersion, BrowserType, CanvasNoiseMode, ClientHints, CoherenceReport,
    ConnectionProfile, Fingerprint, HumanizeConfig, MaskingMode, NoiseMode, OsType, PopupMode,
    ProxyConfig, ProxyMaskingMode, QuicMode, SpeechVoice, StartupBehavior, StealthFlags,
    WebRtcPolicy,
};
pub use stealth::providers::{
    default_registry, EvasionConfig, EvasionContext, EvasionProvider, EvasionRegistry,
};
pub use stealth::{
    engine_spoofing_args, find_system_chrome, ChromeBinaryPatcher, ChromedriverPatcher, EnginePatch,
};
pub use tracing_util::{
    init_tracing, init_tracing_from_runtime, init_tracing_json, init_tracing_json_with_filter,
    init_tracing_with_filter,
};
pub use utilities::python_importer::{
    import_python, ImportDiagnostic, ImportOptions, ImportResult, ImportSeverity, PythonSource,
};
pub use utilities::retry::RetryPolicy;
pub use utils::selectors::Selector;
