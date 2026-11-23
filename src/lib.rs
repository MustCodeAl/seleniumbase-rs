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
//!     let config = BrowserConfig::builder().mode(DriverMode::Uc).build()?;
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
//! `seleniumbase-rs-wire` of
//! <https://github.com/MustCodeAl/seleniumbase-rs-stealth>, which build on the
//! public API of this one.
//!
//! ## Process-wide state
//!
//! A few functions change state for the whole process, so two tests in one
//! process can disturb each other. [`install_tls_provider`] installs the TLS
//! backend. [`init_tracing`], [`init_tracing_json`], [`init_tracing_from_runtime`],
//! [`init_tracing_with_filter`] and [`init_tracing_json_with_filter`] install
//! the log subscriber. [`shutdown_reused_sessions`] closes browsers another
//! test may still be using. The failure-log registry, the shutdown registry
//! and the reused-session table are process-wide as well.

// Every public type has `Debug` (C-DEBUG, M-PUBLIC-DEBUG); the gate builds with `-D warnings`.
#![warn(missing_debug_implementations)]
#![allow(clippy::map_unwrap_or)]

/// The version of Python SeleniumBase whose API this crate tracks.
///
/// Kept in step with `parity/upstream.toml`; see `docs/UPSTREAM_SYNC.md`.
pub const UPSTREAM_SELENIUMBASE_VERSION: &str = "4.56.5";

pub mod api;
pub mod artifacts;

pub mod browser;
pub mod cli;
pub mod common;
pub mod config;
pub mod crawler;
pub mod extract;
// The Microsoft Pragmatic Rust Guidelines lint set, held to for this module (it held for
// the cookie type while it lived in `sb_cdp`).
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
mod cookie;
pub mod core;
pub mod drivers;
pub mod error;
pub mod js_code;
pub mod json;
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
/// [`BrowserConfigBuilder::plugins`](crate::BrowserConfigBuilder::plugins) for the
/// WebDriver engine (`BaseCase`). A plugin crate depends on `seleniumbase-rs-plugin` itself, never on this
/// crate. Not to be confused with [`plugins`], the test-reporting plugins.
#[cfg(feature = "plugins")]
pub use seleniumbase_rs_plugin as plugin_api;
/// The built-in plugins, one cargo feature each (`plugin-shield`,
/// `plugin-block-resources`, `plugin-crx`, `plugin-fetcher`, `plugin-extractor`,
/// `plugin-batcher`, `plugin-crawler`, `plugin-browser`, `plugin-imap`, `plugin-tor`; `plugins-builtin` for all): the
/// crate `seleniumbase-rs-plugins-builtin`, with each plugin's own types and
/// its `registry()`. Most programs enable them by name instead; see
/// [`plugin_host`].
#[cfg(any(
    feature = "plugin-shield",
    feature = "plugin-block-resources",
    feature = "plugin-fetcher",
    feature = "plugin-extractor",
    feature = "plugin-batcher",
    feature = "plugin-crawler",
    feature = "plugin-browser",
    feature = "plugin-imap",
    feature = "plugin-tor",
    feature = "plugin-crx"
))]
pub use seleniumbase_rs_plugins_builtin as builtin_plugins;
pub mod evasion;
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
/// The `sb` and `sb_cdp` fixtures for `cargo test` with the per-test lifecycle:
/// the run configuration, reruns, a time limit, evidence of a failure and the debug
/// prompts (feature `testing`).
#[cfg(feature = "testing")]
pub mod testing;

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

/// The Tokio runtime this crate runs on, under its own name, so a program or a test needs
/// no `tokio` line in its manifest: `use seleniumbase_rs::tokio;` and then
/// `#[tokio::main]` or `#[tokio::test]`. The attribute macros expand to paths that start
/// with `tokio::`, which resolve to this import. It has the features this crate
/// enables (`macros`, `rt-multi-thread`, `time`, `signal`, `sync`, `process`, `net`,
/// `io-util` and `fs`); a program that needs another one lists `tokio` itself.
pub use tokio;

#[doc(inline)]
pub use api::base_case::{BaseCase, CookieExpiry, DriverId};
#[doc(inline)]
pub use api::chart::{Chart, ChartType};
#[cfg(feature = "gui")]
#[doc(inline)]
pub use api::gui::Gui;
#[doc(inline)]
pub use api::run_settings::{PauseKind, RunSettings, RunSettingsBuilder};
#[doc(inline)]
pub use api::runner::{run_browser_test, run_cdp_test};
#[doc(inline)]
pub use api::switching::Alert;
#[doc(inline)]
pub use api::tour::TourTheme;
#[doc(inline)]
pub use api::traits::{AssertionApi, BrowserApi, ElementApi, ScreenshotApi};
#[doc(inline)]
pub use browser::config::{Browser, BrowserConfig, BrowserConfigBuilder, DriverMode};
#[doc(inline)]
pub use browser::element::Element;
#[doc(inline)]
pub use browser::options::NetworkThrottle;
#[doc(inline)]
pub use browser::reuse::shutdown_reused_sessions;
#[doc(inline)]
pub use browser::session::BrowserSession;
#[doc(inline)]
pub use browser::session::{BidiNetworkEvent, DEFAULT_BIDI_BUFFER_SIZE};
#[doc(inline)]
pub use common::tls::install_tls_provider;
#[doc(inline)]
pub use config::settings::{Settings, SettingsOverrides};
#[doc(inline)]
pub use config::{LogFormat, RuntimeConfig};
#[doc(inline)]
pub use cookie::{Cookie, SameSite};
#[doc(inline)]
pub use crawler::{
    CrawlItem, Crawler, CrawlerBuilder, RobotsPolicy, Spider, SpiderConfig, SpiderConfigBuilder,
};
#[doc(inline)]
pub use error::{DriverError, DriverErrorKind, Result, ResultExt, SeleniumBaseError};
#[doc(inline)]
pub use evasion::capture::CapturedFingerprint;
#[doc(inline)]
pub use evasion::fingerprint::{
    AudioNoise, BatteryProfile, BrandVersion, BrowserType, CanvasNoise, ClientHints,
    CoherenceReport, ConnectionProfile, Contradiction, Dimension, Fingerprint, FingerprintBuilder,
    FingerprintComparison, FingerprintError, Geolocation, HumanizeConfig, MediaDevices, OsType,
    ProxyConfig, QuicMode, ScreenSize, SettingDiff, Severity, SpeechVoice, Spoof, StealthPolicy,
    WebGl, WebRtcAddresses, WebRtcPolicy,
};
#[doc(inline)]
pub use evasion::mimic::Specimen;
#[doc(inline)]
pub use evasion::providers::{
    default_registry, session_registry, EvasionConfig, EvasionContext, EvasionProvider,
    EvasionRegistry, SESSION_OMITTED_PROVIDERS,
};
#[doc(inline)]
pub use evasion::{
    engine_spoofing_args, find_system_chrome, find_system_chrome_with, ChromeBinaryPatcher,
    ChromedriverPatcher, EnginePatch,
};
#[doc(inline)]
pub use extract::{
    extract_json_ld, extract_json_ld_raw, PageArena, PageMetadata, TableData, TableDataArena,
};
#[doc(inline)]
pub use sb_cdp::ResilientLocator;
#[doc(inline)]
pub use sb_cdp::{Key, SelectBy};
#[doc(inline)]
pub use thirtyfour::bidi::BiDi;
#[doc(inline)]
pub use tracing_util::{
    init_tracing, init_tracing_from_runtime, init_tracing_json, init_tracing_json_with_filter,
    init_tracing_with_filter,
};
#[doc(inline)]
pub use utilities::python_importer::{
    import_python, ImportDiagnostic, ImportOptions, ImportResult, ImportSeverity, PythonSource,
};
#[doc(inline)]
pub use utilities::retry::RetryPolicy;
#[doc(inline)]
pub use utils::color::Color;
#[doc(inline)]
pub use utils::keys::Keys;
#[doc(inline)]
pub use utils::select::Select;
#[doc(inline)]
pub use utils::selectors::{By, Selector, SelectorRef};
