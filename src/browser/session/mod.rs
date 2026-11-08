//! `BrowserSession`: a WebDriver session, its optional CDP client and its
//! optional auto-started driver process.
//!
//! The struct and its accessors live here; the methods are grouped by concern
//! in the submodules, each an `impl BrowserSession` block:
//!
//! * `connect`: starting a session, a leased one, or a direct CDP attachment;
//! * `lifecycle`: `reconnect`, `quit` and the cleanup on drop;
//! * `launch`: what is done before a WebDriver session exists (the driver, the
//!   browser program, the virtual screen);
//! * `capabilities`: the capabilities each browser is asked for;
//! * `navigation`, `elements`, `waits`, `page`: what a test does in the page;
//! * `cdp`, `fingerprint`, `emulation`: DevTools, the fingerprint, the ad block
//!   and the phone emulation;
//! * `plugins`: browser plugins (feature `plugins`).

mod capabilities;
mod cdp;
mod connect;
mod elements;
mod emulation;
mod fingerprint;
mod launch;
mod lifecycle;
mod navigation;
mod page;
#[cfg(feature = "plugins")]
mod plugins;
mod waits;

#[cfg(feature = "plugins")]
pub(crate) use emulation::blocked_patterns;
pub(crate) use launch::server_url_for_joins;
pub(crate) use waits::shown_text;

use serde_json::Value;
use thirtyfour::prelude::WebDriver;

use crate::browser::launcher::DriverProcess;
#[cfg(feature = "plugins")]
use crate::browser::plugin_hooks::SessionPlugins;
use crate::browser::proxy_auth::ProxyAuthSidecar;
use crate::browser::reuse::Lease;
use crate::browser::virtual_display::XvfbProcess;
use crate::error::SeleniumBaseError;
use crate::evasion::cdp::CdpClient;

/// Owns a WebDriver session, optional CDP client, and optional auto-started driver process.
pub struct BrowserSession {
    driver: Option<WebDriver>,
    cdp: Option<CdpClient>,
    /// A tab of an external Chromium app, reached without WebDriver.
    external_page: Option<crate::sb_cdp::Page>,
    driver_process: Option<DriverProcess>,
    /// The virtual screen of a headed Linux session; dropped, and so stopped,
    /// after the driver.
    virtual_display: Option<XvfbProcess>,
    /// The DevTools commands that make a window refuse the ad hosts of the
    /// configuration, or none if it blocks nothing. They are sent to the first
    /// window and to each window the session switches to.
    ad_block: Vec<(&'static str, Value)>,
    /// The DevTools commands that make a window of an undetected-mode session a
    /// phone, or none: the other modes ask for it with the `mobileEmulation`
    /// capability. Sent like the ad-block commands, because the emulation
    /// belongs to a window.
    mobile: Vec<(&'static str, Value)>,
    /// The hold on a browser kept for reuse (`--reuse-session`). With one,
    /// quitting or dropping the session gives the browser back instead of
    /// closing it, and the reuse registry owns the driver process and the
    /// virtual display.
    reuse: Option<Lease>,
    /// The port of Chrome's remote debugger, when the configuration asked for
    /// one: the port that was asked for, or the free one taken in its place.
    remote_debug: Option<u16>,
    /// The DevTools client that answers the proxy's password prompts, when the
    /// proxy has a login (see `browser::proxy_auth`); closed with the session.
    proxy_login: Option<ProxyAuthSidecar>,
    /// The browser plugins of the session (feature `plugins`), and the scripts
    /// they added; none without plugins.
    #[cfg(feature = "plugins")]
    plugins: Option<SessionPlugins>,
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
    /// WebDriver interaction: `connect`.
    ///
    /// # Panics
    ///
    /// On a direct CDP attachment (use `cdp_*` or [`pure_cdp_page`](Self::pure_cdp_page)),
    /// after [`quit`](Self::quit), or after a [`reconnect`](Self::reconnect)
    /// that could not start the new browser: the session then has no driver
    /// and a new one must be started.
    pub fn driver(&self) -> &WebDriver {
        self.driver.as_ref().expect(
            "the browser session has no WebDriver: it was quit, it is a direct CDP \
                     attachment, or a reconnect could not start a new browser; \
                     `webdriver()` returns an error instead of panicking",
        )
    }

    /// The session's WebDriver, or an error where [`driver`](Self::driver)
    /// panics.
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
    /// [`with_remote_debug`](crate::BrowserConfig::with_remote_debug) the port is
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
            external_page: None,
            driver_process: None,
            virtual_display: None,
            ad_block: Vec::new(),
            mobile: Vec::new(),
            remote_debug: None,
            proxy_login: None,
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        }
    }
}
