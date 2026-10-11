//! The browser leg: one Chrome behind the MITM proxy, and the tab the session loads pages in.
//!
//! The leg is started with the fingerprint of the identity and the proxy and switches the MITM
//! proxy needs, so everything the browser sends leaves with the measured TLS and HTTP/2. It only
//! does what a single browser does: load a page, wait for a clearance, read the cookies. Deciding
//! what to do about what it found is the session's work.

use std::fmt;
use std::time::Duration;

use serde_json::json;

use super::{StealthError, Wiring};
use crate::impersonate::client::ImpersonateError;
use crate::impersonate::mitm::MitmProxy;
use crate::sb_cdp::{Browser, Page};
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::error::ScrapeError;
use crate::scrape::jar::store::CookieJar;
use crate::scrape::passport::identity::StealthIdentity;
use crate::scrape::runtime::cdp::{self, NavResponse, PageSnapshot};

/// The protocol command that names the browser product and version.
const VERSION_COMMAND: &str = "Browser.getVersion";

/// A Chrome behind the MITM proxy.
pub(super) struct BrowserLeg {
    browser: Browser,
    page: Page,
    /// The answer to the navigation that led to the page now open.
    last: Option<NavResponse>,
}

impl fmt::Debug for BrowserLeg {
    /// The tab only: the browser holds the proxy token and the cookies.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserLeg")
            .field("page", &self.page.id())
            .finish_non_exhaustive()
    }
}

impl BrowserLeg {
    /// Starts a Chrome with the fingerprint of `identity` behind `mitm`, checks that it is the
    /// Chrome `measured` describes, and sets the cookies of the identity in it.
    ///
    /// On an error the browser that was started is closed.
    pub(super) async fn launch(
        wiring: &Wiring,
        mitm: &MitmProxy,
        identity: &StealthIdentity,
        measured: u32,
    ) -> Result<Self, StealthError> {
        let options = mitm
            .configure_launch(wiring.launch.clone().fingerprint(&identity.fingerprint))
            .build()?;
        let browser = wiring.launcher.launch(options).await?;
        match Self::settle(&browser, identity, measured, (wiring.clock)()).await {
            Ok(page) => Ok(Self {
                browser,
                page,
                last: None,
            }),
            Err(error) => {
                close_quietly(&browser).await;
                Err(error)
            }
        }
    }

    /// Checks the version of a browser that just started, opens its tab and sets the cookies.
    async fn settle(
        browser: &Browser,
        identity: &StealthIdentity,
        measured: u32,
        now: u64,
    ) -> Result<Page, StealthError> {
        if let Some(found) = browser_major(browser).await {
            if found != measured {
                return Err(ImpersonateError::VersionMismatch {
                    browser: found,
                    measured,
                }
                .into());
            }
        }
        let page = browser.default_page().await?;
        cdp::import_cookies(&page, &identity.cookies, now).await?;
        Ok(page)
    }

    /// The tab the leg loads pages in.
    pub(super) fn page(&self) -> &Page {
        &self.page
    }

    /// Loads `url`, reads the page and judges it with the answer that led to it.
    pub(super) async fn load(
        &mut self,
        url: &str,
    ) -> Result<(NavResponse, PageSnapshot), ScrapeError> {
        let response = cdp::goto_with_response(&self.page, url).await?;
        let snapshot = cdp::detect_page(&self.page, &response).await?;
        self.last = Some(response.clone());
        Ok((response, snapshot))
    }

    /// Solves the challenge `detection` describes and waits for the browser to show the
    /// clearance, for at most `timeout`. Returns whether it showed in time.
    ///
    /// A challenge with a widget is attempted with a click, after the pointer has arrived on the
    /// page like a person's. A script challenge needs none: it is waited for. The wait ends when
    /// the vendor's clearance cookie holds a new value or, for a vendor without one, when the
    /// page stops being a challenge.
    pub(super) async fn solve(
        &self,
        detection: &Detection,
        seed: u64,
        timeout: Duration,
    ) -> Result<bool, ScrapeError> {
        let vendor = detection.evidence.vendor;
        let stale = cdp::clearance_value(&self.page, vendor).await?;
        let has_widget = detection.evidence.widget.is_some();
        if has_widget || detection.kind == ChallengeKind::Interactive {
            cdp::human_warm_up(&self.page, seed).await;
            self.page.solve_captcha().await?;
        }
        cdp::wait_for_clearance(&self.page, vendor, stale.as_deref(), timeout).await
    }

    /// The cookies of the browser, as a jar.
    pub(super) async fn cookies(&self, now: u64) -> Result<CookieJar, ScrapeError> {
        cdp::export_cookies(&self.page, now).await
    }

    /// Closes the browser. If it will not close, that is only logged.
    pub(super) async fn close(self) {
        close_quietly(&self.browser).await;
    }

    /// Closes the browser and says how it went.
    pub(super) async fn close_checked(self) -> Result<(), ScrapeError> {
        self.browser.close().await?;
        Ok(())
    }
}

/// Closes a browser that is no longer wanted. If it will not close, that is only logged.
async fn close_quietly(browser: &Browser) {
    if let Err(error) = browser.close().await {
        tracing::debug!(%error, "a browser did not close cleanly");
    }
}

/// The major version of the browser, from the product it reports, when it reports one.
async fn browser_major(browser: &Browser) -> Option<u32> {
    let version = browser.execute(VERSION_COMMAND, json!({})).await.ok()?;
    major_of(version["product"].as_str()?)
}

/// The major version in a `Chrome/155.0.8059.40`, `HeadlessChrome/155.0.8059.40` or
/// `Chromium/...` token.
fn major_of(text: &str) -> Option<u32> {
    let (_, after) = text.split_once('/')?;
    after.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_major_is_read_from_the_product_a_browser_reports() {
        assert_eq!(major_of("Chrome/155.0.8059.40"), Some(155));
        assert_eq!(major_of("HeadlessChrome/155.0.8059.40"), Some(155));
        assert_eq!(major_of("Chromium/9.1"), Some(9));
        assert_eq!(major_of("Chrome"), None);
        assert_eq!(major_of("Chrome/x.1"), None);
        assert_eq!(major_of(""), None);
    }
}
