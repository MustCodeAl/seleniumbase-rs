//! What the ports share: a headless browser configuration for each engine.
//!
//! The Python examples run headed by default and `--headless` turns that on.
//! The ports always run headless, with a window as large as the Python
//! headless default (1440 x 1880), so a page lays out the way it does there.

use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::BrowserConfig;

/// The WebDriver engine (`BaseCase`), headless.
pub fn headless() -> BrowserConfig {
    BrowserConfig::default().with_headless(true)
}

/// The pure CDP engine (`sb_cdp`), headless.
pub fn cdp_options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1440, 1880)
        .build()
        .expect("the launch options are valid")
}
