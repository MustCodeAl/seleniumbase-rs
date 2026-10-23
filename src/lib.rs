//! # SeleniumBase for Rust
//!
//! A Rust port of the Python SeleniumBase testing framework. It provides a
//! `BaseCase` API for browser automation, stealth/undetected modes via CDP,
//! a command-line helper (`sbrs`), and supporting modules for configuration,
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
//!
//! ## Stealth scraping
//!
//! The experimental stealth scraping layer (wire fingerprints, `BrowserScraper`,
//! `HttpSession`, `MitmProxy`, `StealthSession`) is not part of this crate. It
//! lives in the crates `seleniumbase-rs-codec`, `seleniumbase-rs-scrape` and
//! `seleniumbase-rs-mimic` of
//! <https://github.com/MustCodeAl/seleniumbase-rs-stealth>, which build on the
//! public API of this one.

/// The version of Python SeleniumBase whose API this crate tracks.
///
/// Kept in step with `parity/upstream.toml`; see `docs/UPSTREAM_SYNC.md`.
pub const UPSTREAM_SELENIUMBASE_VERSION: &str = "4.55.2";

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
// The Microsoft Pragmatic Rust Guidelines lint set, held to for this module.
#[cfg(feature = "mcp-server")]
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph,
    missing_debug_implementations,
    redundant_imports,
    redundant_lifetimes,
    trivial_numeric_casts,
    unused_lifetimes
)]
pub mod mcp;
// The Microsoft Pragmatic Rust Guidelines lint set, held to for this module.
#[cfg(feature = "ocr")]
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph,
    missing_debug_implementations,
    redundant_imports,
    redundant_lifetimes,
    trivial_numeric_casts,
    unused_lifetimes
)]
pub mod ocr;
/// The plugin API of ADR 004, the crate `seleniumbase-rs-plugin` (feature
/// `plugins`): the `Plugin` trait, `PluginSet`, `LaunchSettings` and the
/// `BrowserHandle` and `PageHandle` traits.
///
/// Give a set of plugins to
/// [`LaunchOptionsBuilder::plugins`](sb_cdp::LaunchOptionsBuilder::plugins)
/// for the pure CDP engine, or to
/// [`BrowserConfig::with_plugins`](crate::BrowserConfig::with_plugins) for the
/// WebDriver engine (`BaseCase`). A plugin crate depends on `seleniumbase-rs-plugin` itself, never on this
/// crate. Not to be confused with [`plugins`], the test-reporting plugins.
#[cfg(feature = "plugins")]
pub use seleniumbase_rs_plugin as plugin_api;
/// The built-in plugins, one cargo feature each (`plugin-shield`,
/// `plugin-block-resources`, `plugin-crx`; `plugins-builtin` for all): the
/// crate `seleniumbase-rs-plugins-builtin`, with each plugin's own types and
/// its `registry()`. Most programs enable them by name instead; see
/// [`plugin_host`].
#[cfg(any(
    feature = "plugin-shield",
    feature = "plugin-block-resources",
    feature = "plugin-crx"
))]
pub use seleniumbase_rs_plugins_builtin as builtin_plugins;
/// Plugin names and options turned into a `PluginSet` (feature `plugins`): the
/// registry of the built-in plugins, and the functions the launch builders
/// call for `plugins_named` and `plugin_selection`.
#[cfg(feature = "plugins")]
pub mod plugin_host;
pub mod plugins;
pub mod profile_payloads;
pub mod resources;
#[cfg(feature = "turso")]
pub mod storage;
// The Microsoft Pragmatic Rust Guidelines lint set, held to for this module.
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph,
    missing_debug_implementations,
    redundant_imports,
    redundant_lifetimes,
    trivial_numeric_casts,
    unused_lifetimes
)]
pub mod sb_cdp;
pub mod stealth;
pub mod tracing_util;
pub mod utilities;
pub mod utils;
// The Microsoft Pragmatic Rust Guidelines lint set, held to for this module.
#[cfg(feature = "visual")]
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph,
    missing_debug_implementations,
    redundant_imports,
    redundant_lifetimes,
    trivial_numeric_casts,
    unused_lifetimes
)]
pub mod visual;

pub use api::base_case::{BaseCase, CookieExpiry, DriverId};
pub use api::chart::{Chart, ChartType};
#[cfg(feature = "gui")]
pub use api::gui::Gui;
pub use api::run_settings::{PauseKind, RunSettings};
pub use api::runner::{run_browser_test, run_cdp_test};
pub use api::tour::TourTheme;
pub use api::traits::{AssertionApi, BrowserApi, ElementApi, ScreenshotApi};
pub use browser::config::{Browser, BrowserConfig, DriverMode};
pub use browser::reuse::shutdown_reused_sessions;
pub use browser::session::BrowserSession;
pub use common::tls::install_tls_provider;
pub use config::settings::{Settings, SettingsOverrides};
pub use config::{LogFormat, RuntimeConfig};
pub use error::{Result, ResultExt, SeleniumBaseError};
pub use stealth::capture::CapturedFingerprint;
pub use stealth::fingerprint::{
    AudioNoise, BatteryProfile, BrandVersion, BrowserType, CanvasNoise, ClientHints,
    CoherenceReport, ConnectionProfile, Dimension, Fingerprint, FingerprintBuilder,
    FingerprintComparison, FingerprintError, Geolocation, HumanizeConfig, MediaDevices, OsType,
    ProxyConfig, QuicMode, ScreenSize, SettingDiff, SpeechVoice, Spoof, StealthPolicy, WebGl,
    WebRtcAddresses, WebRtcPolicy,
};
pub use stealth::providers::{
    default_registry, session_registry, EvasionConfig, EvasionContext, EvasionProvider,
    EvasionRegistry, SESSION_OMITTED_PROVIDERS,
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
