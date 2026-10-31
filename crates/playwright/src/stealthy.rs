//! A stealthy CDP browser with Playwright attached: both APIs on one browser.

use seleniumbase_rs::sb_cdp::{Browser, DevToolsEndpoint, LaunchOptions};
use seleniumbase_rs::SeleniumBaseError;

use crate::PlaywrightSession;

/// A stealthy pure-CDP browser and a [`PlaywrightSession`] attached to it.
///
/// This is Python SeleniumBase's *Stealthy Playwright Mode* in one value: the browser is
/// launched by [`sb_cdp`](seleniumbase_rs::sb_cdp) with all of its launch switches, fingerprint
/// and proxy handling, and rustwright connects to it over the DevTools Protocol. The CDP API
/// ([`cdp`](Self::cdp)) and the Playwright API ([`playwright`](Self::playwright)) then work on
/// the same tabs at the same time.
///
/// Close it with [`close`](Self::close): that detaches Playwright first and then closes the
/// browser.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::LaunchOptions;
/// use seleniumbase_rs_playwright::StealthyPlaywright;
///
/// # async fn demo() -> seleniumbase_rs::Result<()> {
/// let stealthy = StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?)
///     .await?;
///
/// // Playwright drives the page ...
/// stealthy.playwright().goto("https://example.com").await?;
/// // ... and the CDP API reads it back from the same browser.
/// let title = stealthy.cdp().default_page().await?.title().await?;
/// println!("{title}");
///
/// stealthy.close().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StealthyPlaywright {
    cdp: Browser,
    playwright: PlaywrightSession,
}

impl StealthyPlaywright {
    /// Launches a stealthy browser as `options` say and attaches Playwright to it.
    ///
    /// To open a page with the CDP API before Playwright attaches (the order Python
    /// SeleniumBase asks for with an authenticated proxy), launch the browser yourself with
    /// [`Browser::launch`], open the page, and call [`PlaywrightSession::attach`].
    ///
    /// # Errors
    ///
    /// Returns the error of [`Browser::launch`] if the browser does not start, or
    /// [`SeleniumBaseError::Playwright`] if Playwright cannot attach. In the second case the
    /// browser that was started is closed again.
    pub async fn launch(options: LaunchOptions) -> Result<Self, SeleniumBaseError> {
        let cdp = Browser::launch(options).await?;
        match PlaywrightSession::attach(&cdp).await {
            Ok(playwright) => Ok(Self { cdp, playwright }),
            Err(error) => {
                // Do not leave a browser running that nobody holds.
                let _ = cdp.close().await;
                Err(error)
            }
        }
    }

    /// The pure-CDP browser: the CDP Mode API (pages, locators, interception, input).
    #[must_use]
    pub fn cdp(&self) -> &Browser {
        &self.cdp
    }

    /// The Playwright session attached to the same browser.
    #[must_use]
    pub fn playwright(&self) -> &PlaywrightSession {
        &self.playwright
    }

    /// The Playwright session, mutably, for [`PlaywrightSession::new_page`].
    #[must_use]
    pub fn playwright_mut(&mut self) -> &mut PlaywrightSession {
        &mut self.playwright
    }

    /// The DevTools endpoint of the browser, for other clients that attach to it
    /// (Playwright MCP, `chrome-devtools-mcp`, a Playwright in another language).
    #[must_use]
    pub fn endpoint(&self) -> DevToolsEndpoint {
        self.cdp.devtools_endpoint()
    }

    /// Detaches Playwright and closes the browser.
    ///
    /// # Errors
    ///
    /// Returns the first error of detaching and of closing the browser; the browser is
    /// closed even when detaching fails.
    pub async fn close(self) -> Result<(), SeleniumBaseError> {
        let detached = self.playwright.close().await;
        let closed = self.cdp.close().await;
        detached.and(closed)
    }
}
