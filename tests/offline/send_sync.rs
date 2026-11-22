//! Compile-time pins of the traits the public handle and value types promise.
//!
//! C-SEND-SYNC and M-TYPES-SEND: a test moves a browser handle into `tokio::spawn`, shares one
//! across tasks and threads, and returns an error from a spawned task, so those types must be
//! `Send` and `Sync` (and `'static`: nothing borrows from the case that made it). C-DEBUG and
//! M-PUBLIC-DEBUG: every public type has `Debug`, and the main types are pinned here as well,
//! next to the `missing_debug_implementations` lint of `src/lib.rs`, which covers the rest.
//!
//! Nothing runs: a function that names a type in a bound compiles only when the type has the
//! traits, so the test is the compilation of this file. A change that puts a `Rc`, a `RefCell`
//! or a raw pointer in one of these types stops this file from compiling.

use std::fmt::Debug;

/// A type that a task may own (`Send`), that several tasks may share (`Sync`), that owns what
/// it uses (`'static`) and that prints for a failure message (`Debug`).
fn handle<T: Send + Sync + Debug + 'static>() {}

/// The same for a value that is also cloned and compared in tests.
fn value<T: Send + Sync + Debug + Clone + PartialEq + 'static>() {}

/// The WebDriver engine: the case a test owns, its session and its configuration.
#[test]
fn the_webdriver_engine_handles_are_send_and_sync() {
    handle::<seleniumbase_rs::BaseCase>();
    handle::<seleniumbase_rs::BrowserSession>();
    handle::<seleniumbase_rs::Element>();
    handle::<seleniumbase_rs::RunSettings>();
    handle::<seleniumbase_rs::api::deferred::DeferredAsserts>();
}

/// The configuration and the settings are built on one thread and read on another.
#[test]
fn the_configuration_types_are_send_and_sync() {
    value::<seleniumbase_rs::BrowserConfig>();
    handle::<seleniumbase_rs::BrowserConfigBuilder>();
    handle::<seleniumbase_rs::RunSettingsBuilder>();
    value::<seleniumbase_rs::Settings>();
    handle::<seleniumbase_rs::SettingsOverrides>();
    value::<seleniumbase_rs::RuntimeConfig>();
    value::<seleniumbase_rs::RetryPolicy>();
    value::<seleniumbase_rs::Browser>();
    value::<seleniumbase_rs::DriverMode>();
}

/// The pure CDP engine: the handles are cheap to clone and every clone is usable on any task.
#[test]
fn the_pure_cdp_handles_are_send_and_sync() {
    use seleniumbase_rs::sb_cdp;

    handle::<sb_cdp::Browser>();
    handle::<sb_cdp::Page>();
    handle::<sb_cdp::Locator>();
    handle::<sb_cdp::BrowserContext>();
    handle::<sb_cdp::BrowserPool>();
    handle::<sb_cdp::Lease>();
    handle::<sb_cdp::SessionStore>();
    handle::<sb_cdp::Session>();
    handle::<sb_cdp::Cookies>();
    handle::<sb_cdp::Storage>();
    handle::<sb_cdp::Window>();
    handle::<sb_cdp::Emulation>();
    handle::<sb_cdp::Mouse>();
    handle::<sb_cdp::Keyboard>();
    handle::<sb_cdp::Human>();
    handle::<sb_cdp::LocatorExpect>();
    handle::<sb_cdp::PageExpect>();
    handle::<sb_cdp::HarRecorder>();
    handle::<sb_cdp::TraceRecorder>();
    handle::<sb_cdp::Interception>();
    handle::<sb_cdp::CdpInbox>();
    handle::<sb_cdp::StealthPlan>();
    handle::<sb_cdp::DevToolsEndpoint>();
}

/// The options of the pure CDP engine and the plain values its calls take and return.
#[test]
fn the_pure_cdp_values_are_send_and_sync() {
    use seleniumbase_rs::sb_cdp;

    handle::<sb_cdp::LaunchOptions>();
    handle::<sb_cdp::LaunchOptionsBuilder>();
    value::<sb_cdp::PoolOptions>();
    handle::<sb_cdp::PoolOptionsBuilder>();
    value::<sb_cdp::PoolStats>();
    value::<sb_cdp::ContextOptions>();
    value::<sb_cdp::Point>();
    value::<sb_cdp::Rect>();
    value::<sb_cdp::PageInfo>();
    value::<sb_cdp::ElementInfo>();
    value::<sb_cdp::CdpRequest>();
}

/// A cookie and a recorded action cross tasks; an error comes back from a spawned task.
#[test]
fn the_value_types_a_task_returns_are_send_and_sync() {
    value::<seleniumbase_rs::Cookie>();
    value::<seleniumbase_rs::SameSite>();
    value::<seleniumbase_rs::api::recorder::RecordedCommand>();
    value::<seleniumbase_rs::utils::selectors::Selector>();
    value::<seleniumbase_rs::utils::selectors::SelectorRef<'static>>();
    handle::<seleniumbase_rs::SeleniumBaseError>();
    handle::<seleniumbase_rs::DriverError>();
    handle::<seleniumbase_rs::sb_cdp::TransportError>();
}

/// The fingerprint is built once and handed to every browser of a pool.
#[test]
fn the_fingerprint_types_are_send_and_sync() {
    value::<seleniumbase_rs::Fingerprint>();
    handle::<seleniumbase_rs::FingerprintBuilder>();
    handle::<seleniumbase_rs::FingerprintError>();
    value::<seleniumbase_rs::StealthPolicy>();
    value::<seleniumbase_rs::ProxyConfig>();
    value::<seleniumbase_rs::CapturedFingerprint>();
}

/// The fixtures of the `testing` feature are moved into the test body that `run` drives.
#[cfg(feature = "testing")]
#[test]
fn the_test_fixtures_are_send_and_sync() {
    handle::<seleniumbase_rs::testing::Sb>();
    handle::<seleniumbase_rs::testing::SbCdp>();
}
