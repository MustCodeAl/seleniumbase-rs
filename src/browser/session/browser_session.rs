//! `BrowserSession`: a WebDriver session, its optional CDP client and its
//! optional auto-started driver process.

use std::sync::Mutex;

use serde_json::Value;
use thirtyfour::prelude::WebDriver;

use super::pure_cdp::PageSlot;
use crate::browser::devtools::CdpClient;
use crate::browser::launcher::DriverProcess;
#[cfg(feature = "plugins")]
use crate::browser::plugin_hooks::SessionPlugins;
use crate::browser::proxy_auth::ProxyAuthSidecar;
use crate::browser::reuse::Lease;
use crate::browser::virtual_display::XvfbProcess;
use crate::error::SeleniumBaseError;

/// Owns a WebDriver session, optional CDP client, and optional auto-started driver process.
pub struct BrowserSession {
    pub(super) driver: Option<WebDriver>,
    pub(super) cdp: Option<CdpClient>,
    /// A tab of an external Chromium app, reached without WebDriver.
    pub(super) external_page: PageSlot,
    /// The frames a direct CDP attachment has entered, outermost first, each the
    /// locator of its `<iframe>` or `<frame>`: WebDriver keeps the current frame
    /// in the browser, the DevTools engine in the session. Empty at the top
    /// document, and always empty for a WebDriver session.
    pub(super) frames: Mutex<Vec<crate::sb_cdp::Locator>>,

    pub(super) driver_process: Option<DriverProcess>,
    /// The virtual screen of a headed Linux session; dropped, and so stopped,
    /// after the driver.
    pub(super) virtual_display: Option<XvfbProcess>,
    /// The DevTools commands that make a window refuse the ad hosts of the
    /// configuration, or none if it blocks nothing. They are sent to the first
    /// window and to each window the session switches to.
    pub(super) ad_block: Vec<(&'static str, Value)>,
    /// The DevTools commands that make a window of an undetected-mode session a
    /// phone, or none: the other modes ask for it with the `mobileEmulation`
    /// capability. Sent like the ad-block commands, because the emulation
    /// belongs to a window.
    pub(super) mobile: Vec<(&'static str, Value)>,
    /// The DevTools commands that set up session emulation (timezone, geolocation, user agent override).
    /// Sent like the ad-block and mobile commands, because emulation belongs to a window.
    pub(super) emulation: Vec<(&'static str, Value)>,
    /// The hold on a browser kept for reuse (`--reuse-session`). With one,
    /// quitting or dropping the session gives the browser back instead of
    /// closing it, and the reuse registry owns the driver process and the
    /// virtual display.
    pub(super) reuse: Option<Lease>,
    /// The port of Chrome's remote debugger, when the configuration asked for
    /// one: the port that was asked for, or the free one taken in its place.
    pub(super) remote_debug: Option<u16>,
    /// The DevTools client that answers the proxy's password prompts, when the
    /// proxy has a login (see `browser::proxy_auth`); closed with the session.
    pub(super) proxy_login: Option<ProxyAuthSidecar>,
    /// Whether the current WebDriver session was re-attached to an already
    /// running Chromium browser via `debuggerAddress` (UC reconnect).
    pub(super) attached_debugger: bool,
    /// The browser plugins of the session (feature `plugins`), and the scripts
    /// they added; none without plugins.
    #[cfg(feature = "plugins")]
    pub(super) plugins: Option<SessionPlugins>,
}

impl std::fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSession")
            .field("has_driver", &self.driver.is_some())
            .field("has_cdp", &self.cdp.is_some())
            .field("external_page", &self.external_page.is_some())
            .field("remote_debug", &self.remote_debug)
            .field("reused", &self.reuse.is_some())
            .finish_non_exhaustive()
    }
}

impl BrowserSession {
    /// The session's [`WebDriver`], the raw type of the WebDriver
    /// engine for a call that the session has no method for (Python's `self.driver`), or an
    /// error when the session has none; it never panics.
    /// A failed call on it is a [`WebDriverError`](crate::drivers::WebDriverError), which `?`
    /// turns into a [`SeleniumBaseError`].
    ///
    /// Every WebDriver-backed method of the session, and of `BaseCase`, goes
    /// through this, so a session without a driver answers them with an error.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Unsupported`], naming the engine, on a direct CDP
    /// attachment (`cdp_url` or `cdp_discovery`): the driver does not exist
    /// there, so use the `cdp_*` methods or
    /// [`pure_cdp_page`](Self::pure_cdp_page).
    /// [`SeleniumBaseError::SessionNotStarted`] after [`quit`](Self::quit), or
    /// after a [`reconnect`](Self::reconnect) that could not start the new
    /// browser.
    pub fn webdriver(&self) -> Result<&WebDriver, SeleniumBaseError> {
        match self.driver.as_ref() {
            Some(driver) => Ok(driver),
            None if self.external_page.is_some() => Err(SeleniumBaseError::Unsupported(
                "this needs the WebDriver engine, but the session is a direct CDP attachment \
                     (cdp_url): use the cdp_* methods or pure_cdp_page()"
                    .to_owned(),
            )),
            None => Err(SeleniumBaseError::SessionNotStarted),
        }
    }

    /// Returns the WebDriver BiDi handle for bidirectional browser interaction.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if the WebDriver session is not active or BiDi connection fails.
    pub async fn bidi(&self) -> Result<thirtyfour::bidi::BiDi, SeleniumBaseError> {
        let driver = self.webdriver()?;
        driver.bidi().await.map_err(Into::into)
    }

    /// The WebDriver session id (Python's `driver.session_id`), or `None` for
    /// a session that is not connected.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.driver
            .as_ref()
            .map(|driver| driver.handle().session_id().to_string())
    }

    /// Where Chrome's remote debugger listens (`localhost:9222`), when the
    /// configuration opened one (`--remote-debug`); open `chrome://inspect` in
    /// another Chrome to see the session. With
    /// [`remote_debug`](crate::BrowserConfigBuilder::remote_debug) the port is
    /// the free one taken in place of a busy 9222.
    #[must_use]
    pub fn remote_debug_address(&self) -> Option<String> {
        self.remote_debug.map(|port| format!("localhost:{port}"))
    }

    /// Creates a disconnected session for test-only use. Any WebDriver call will
    /// panic, so this must only be used when the caller never touches the driver.
    pub fn disconnected() -> Self {
        Self {
            driver: None,
            cdp: None,
            external_page: PageSlot::default(),
            frames: Mutex::new(Vec::new()),

            driver_process: None,
            virtual_display: None,
            ad_block: Vec::new(),
            mobile: Vec::new(),
            emulation: Vec::new(),
            remote_debug: None,
            proxy_login: None,
            reuse: None,
            attached_debugger: false,
            #[cfg(feature = "plugins")]
            plugins: None,
        }
    }
}
