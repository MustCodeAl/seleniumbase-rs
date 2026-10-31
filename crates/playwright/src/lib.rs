//! Stealthy Playwright Mode for `seleniumbase-rs`.
//!
//! Python SeleniumBase has a *Stealthy Playwright Mode*: `playwright-python` attaches to
//! SeleniumBase's stealthy Chrome through the remote-debugging URL
//! (`connect_over_cdp`), so Playwright inherits the stealth of that browser and the APIs
//! of both frameworks work on the same tabs. This crate is the Rust side of it.
//!
//! * The browser is the pure-CDP [`sb_cdp::Browser`](seleniumbase_rs::sb_cdp::Browser) of
//!   `seleniumbase-rs`, launched with its fingerprint, proxy and launch-switch handling.
//! * The Playwright client is [`rustwright`], a Rust port of Playwright with a native CDP
//!   engine. It is re-exported as [`rustwright`], so a project lists one dependency.
//! * [`PlaywrightSession`] attaches the client to a running browser and offers the usual page
//!   actions as `async` methods. [`StealthyPlaywright`] launches the browser and attaches in
//!   one call.
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
//! Launch a stealthy browser and drive it with both APIs:
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::LaunchOptions;
//! use seleniumbase_rs_playwright::StealthyPlaywright;
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let stealthy = StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?)
//!     .await?;
//! stealthy.playwright().goto("https://example.com").await?;
//! println!("{}", stealthy.playwright().get_text("h1").await?);
//! stealthy.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! Attach to a browser you launched yourself, and use the rustwright page directly:
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs_playwright::PlaywrightSession;
//!
//! # async fn demo() -> seleniumbase_rs::Result<()> {
//! let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
//! let session = PlaywrightSession::attach(&browser).await?;
//! let title = session
//!     .with_page(|page| {
//!         page.goto("https://example.com", Default::default())?;
//!         page.title(Default::default())
//!     })
//!     .await?;
//! println!("{title}");
//! session.close().await?;
//! browser.close().await?;
//! # Ok(())
//! # }
//! ```

mod session;
mod stealthy;

pub use rustwright;
pub use session::PlaywrightSession;
pub use stealthy::StealthyPlaywright;
