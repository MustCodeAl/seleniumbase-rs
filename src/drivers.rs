//! The WebDriver-side types of the engine behind [`BaseCase`](crate::BaseCase), under this
//! crate's name.
//!
//! The WebDriver engine is built on the `thirtyfour` crate, and a few of its types show up in
//! this crate's signatures: [`BaseCase::find_element`](crate::BaseCase::find_element) returns
//! a [`WebElement`], [`BrowserSession::find`](crate::BrowserSession::find) takes a [`By`],
//! and the capabilities of a custom launch are [`DesiredCapabilities`]. Import them from here
//! and a program needs no `thirtyfour` line in its manifest:
//!
//! ```no_run
//! use seleniumbase_rs::drivers::By;
//! use seleniumbase_rs::{BaseCase, BrowserConfig};
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let mut sb = BaseCase::new(BrowserConfig::default()).await?;
//! sb.open("https://example.com").await?;
//!
//! // `get_shadow_root` hands back the shadow root as a `WebElement`; search inside it
//! // with a `By`.
//! let root = sb.get_shadow_root("my-app").await?;
//! let button = root.find(By::Css("button")).await?;
//! button.click().await?;
//! sb.quit().await
//! # }
//! ```
//!
//! These are the items this crate itself uses or returns; the version is the one this crate
//! builds against (`thirtyfour` 0.37), so a type from here is always the one the API expects.
//! A program that needs a part of `thirtyfour` that is not listed lists the crate itself.
//! The pure CDP engine ([`sb_cdp`](crate::sb_cdp)) does not use any of these.

pub use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
pub use thirtyfour::common::keys::Key;
pub use thirtyfour::cookie::Cookie;
pub use thirtyfour::error::{WebDriverError, WebDriverErrorInner};
// `BaseCase::set_network_conditions` takes the chromedriver `NetworkConditions` of
// thirtyfour, which thirtyfour has deprecated in favour of its typed CDP domain; the
// re-export carries the type the API takes, not an opinion about it.
#[expect(
    deprecated,
    reason = "the API takes thirtyfour's chromedriver NetworkConditions"
)]
pub use thirtyfour::extensions::cdp::{ConnectionType, NetworkConditions};
pub use thirtyfour::prelude::WebDriverResult;
pub use thirtyfour::{
    BrowserCapabilitiesHelper, By, Capabilities, CapabilitiesHelper, DesiredCapabilities,
    LoggingPrefsLogLevel, WebDriver, WebElement,
};
