//! Stealthy Playwright Mode for `seleniumbase-rs`.
//!
//! Python SeleniumBase has a *Stealthy Playwright Mode*: `playwright-python` attaches to
//! SeleniumBase's stealthy Chrome through the remote-debugging URL
//! (`connect_over_cdp`), so Playwright inherits the stealth of that browser and the APIs
//! of both frameworks work on the same tabs. This crate is the Rust side of it.
//!
//! The stealthy browser is driven with this project's own API, the one every other page of
//! the documentation teaches: the capability traits `BrowserApi`, `ElementApi`,
//! `AssertionApi` and `ScreenshotApi` that [`sb_cdp::Page`](seleniumbase_rs::sb_cdp::Page)
//! and [`BaseCase`](seleniumbase_rs::BaseCase) implement, plus their SeleniumBase extras
//! such as `solve_captcha`. Playwright joins for what that API does not do, and the bridge
//! wraps no Playwright call:
//!
//! * The browser is the pure-CDP [`sb_cdp::Browser`](seleniumbase_rs::sb_cdp::Browser) of
//!   `seleniumbase-rs` (or the Chrome of a [`BaseCase`](seleniumbase_rs::BaseCase) in UC
//!   mode), with its fingerprint, proxy and launch-switch handling.
//! * The Playwright client is [`rustwright`], a Rust port of Playwright with a native CDP
//!   engine. It is re-exported as [`rustwright`], so a project lists one dependency.
//! * [`PlaywrightSidecar`] attaches the client beside a handle of this project, on the tab
//!   that handle is on ([`PlaywrightTarget`]: an `sb_cdp::Page` or a `BaseCase`), or to any
//!   Chromium's [`DevToolsEndpoint`](seleniumbase_rs::sb_cdp::DevToolsEndpoint).
//!   [`run`](PlaywrightSidecar::run) hands a closure the rustwright page.
//!
//! The library has no Playwright engine of its own: no `playwright` feature and no
//! `rustwright` dependency. A project that wants Playwright lists this crate:
//!
//! ```toml
//! [dependencies]
//! seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
//! seleniumbase-rs-playwright = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
//! ```
//!
//! # Examples
//!
//! The pure CDP format, Python's `sb_cdp.Chrome()` then `connect_over_cdp`: our page on the
//! stealthy browser, and Playwright beside it for one call.
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs::{AssertionApi, BrowserApi};
//! use seleniumbase_rs_playwright::PlaywrightSidecar;
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
//! let mut page = browser.default_page().await?;
//! page.open("https://example.com").await?;
//! page.assert_text("Example Domain", "h1").await?;
//!
//! let playwright = PlaywrightSidecar::attach(&page).await?;
//! // Placeholder: any rustwright call goes in the closure. Our own title is `page.title()`.
//! let title = playwright.run(|pw| pw.title(Default::default())).await?;
//! println!("{title}");
//!
//! playwright.close().await?; // detaches; the browser and the tab stay
//! browser.close().await
//! # }
//! ```
//!
//! The `SB(uc=True)` format: a [`BaseCase`](seleniumbase_rs::BaseCase) in UC mode, and the
//! sidecar attached to the window it is on.
//!
//! ```no_run
//! use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
//! use seleniumbase_rs_playwright::PlaywrightSidecar;
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let mut sb = BaseCase::new(BrowserConfig::default().with_mode(DriverMode::Uc)).await?;
//! sb.open("https://example.com").await?;
//!
//! let playwright = PlaywrightSidecar::attach(&sb).await?; // the window `sb` is on
//! // Placeholder: any rustwright call goes in the closure. Our own title is `sb.get_title()`.
//! playwright.run(|pw| pw.title(Default::default())).await?;
//! sb.assert_element("h1").await?;
//!
//! playwright.close().await?;
//! sb.quit().await
//! # }
//! ```

mod sidecar;
mod target;

pub use rustwright;
pub use sidecar::PlaywrightSidecar;
pub use target::PlaywrightTarget;
