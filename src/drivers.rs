//! The WebDriver-side types of the engine behind [`BaseCase`](crate::BaseCase), under this
//! crate's name.
//!
//! The WebDriver engine is built on the `thirtyfour` crate, and a few of its types show up in
//! this crate's escape hatches: an
//! [`Element`](crate::Element) hands out the raw [`WebElement`] behind it. The calls on those
//! raw types fail with a [`WebDriverError`], which `?` turns into a
//! [`SeleniumBaseError`](crate::SeleniumBaseError); this crate's own calls fail with a
//! [`DriverError`](crate::DriverError) in its `WebDriver` variant, which names no `thirtyfour`
//! type and tells the failure by its [`DriverErrorKind`](crate::DriverErrorKind). Import them from here
//! and a program needs no `thirtyfour` line in its manifest:
//!
//! ```no_run
//! use seleniumbase_rs::drivers::WebElement;
//! use seleniumbase_rs::{BaseCase, BrowserConfig};
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let mut sb = BaseCase::new(BrowserConfig::default()).await?;
//! sb.open("https://example.com").await?;
//!
//! // The raw `thirtyfour` element, for a call that `Element` has no method for.
//! let element = sb.find_element("my-app::shadow button").await?;
//! let raw: &WebElement = element.as_webdriver_element();
//! println!("{}", raw.element_id());
//! element.click().await?;
//! sb.quit().await
//! # }
//! ```
//!
//! These are the items this crate itself uses or returns; the version is the one this crate
//! builds against (`thirtyfour` 0.38), so a type from here is always the one the API expects.
//! A program that needs a part of `thirtyfour` that is not listed lists the crate itself.
//! The pure CDP engine ([`sb_cdp`](crate::sb_cdp)) does not use any of these.
//!
//! There is no `Key` here. The named key of this crate is [`Key`](crate::Key), at the crate root,
//! and both engines take it (`Element::press`, `Locator::press`); `thirtyfour`'s own `Key`, the
//! private-use code points a WebDriver session types, is not re-exported, so the name has one
//! meaning.

pub use thirtyfour::error::WebDriverError;

pub use thirtyfour::prelude::WebDriverResult;
pub use thirtyfour::{By, WebDriver, WebElement};
