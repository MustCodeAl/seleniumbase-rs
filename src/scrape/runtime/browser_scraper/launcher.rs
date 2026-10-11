//! The browser launcher port, and what the scraper reads from the browser it launched.

use async_trait::async_trait;
use serde_json::json;

use crate::sb_cdp::{Browser, LaunchOptions};
use crate::scrape::error::ScrapeError;
use crate::stealth::fingerprint::{Fingerprint, OsType};

/// The protocol command that names the browser product and version.
const VERSION_COMMAND: &str = "Browser.getVersion";

/// The prefix of the `Chrome/155.0.0.0` token of a user agent or product string.
const CHROME_TOKEN: &str = "Chrome/";

/// Starts a browser for a [`BrowserScraper`](super::BrowserScraper).
///
/// The scraper starts a browser when it is built and again when it rotates its profile. The shipped
/// launcher, [`ChromeLauncher`], starts Chrome; another implementation can start a different build,
/// reuse a browser, or hand out a scripted one in a test.
#[async_trait]
pub trait BrowserLauncher: Send + Sync {
    /// Starts a browser with `options`.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when no browser can be started or controlled.
    async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError>;
}

/// Starts Chrome (or the browser the launch options name) with [`Browser::launch`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ChromeLauncher;

#[async_trait]
impl BrowserLauncher for ChromeLauncher {
    async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError> {
        Ok(Browser::launch(options).await?)
    }
}

/// The operating system this machine runs, as the persona of a real browser on it.
pub(super) fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// The major version of the browser, from the product it reports, when it reports one.
pub(super) async fn browser_major(browser: &Browser) -> Option<u32> {
    let version = browser.execute(VERSION_COMMAND, json!({})).await.ok()?;
    major_of(version["product"].as_str()?)
}

/// The major version in a `Chrome/155.0.8059.40`, `HeadlessChrome/155.0.8059.40` or `Chromium/...`
/// token.
pub(super) fn major_of(text: &str) -> Option<u32> {
    let (_, after) = text.split_once('/')?;
    after.split('.').next()?.parse().ok()
}

/// The major version a fingerprint claims in its user agent.
pub(super) fn claimed_major(fingerprint: &Fingerprint) -> Option<u32> {
    let agent = fingerprint.user_agent.as_deref()?;
    let (_, after) = agent.split_once(CHROME_TOKEN)?;
    after.split('.').next()?.parse().ok()
}

/// A fresh persona for `os` and `seed`: claiming the running browser's major when it is known.
pub(super) fn persona_for(os: OsType, seed: u64, major: Option<u32>) -> Fingerprint {
    match major {
        Some(major) => Fingerprint::randomized_with_major(os, seed, major),
        None => Fingerprint::randomized(os, seed),
    }
}
