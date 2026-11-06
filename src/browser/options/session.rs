//! What only a `BaseCase` session reads: the driver mode, the WebDriver server, the Selenium
//! Grid, the reuse of browsers, the capabilities and the other settings of a WebDriver start-up.
//!
//! [`SessionSettings`] sits beside [`BrowserSettings`](super::BrowserSettings) in
//! [`Settings`](crate::Settings) and [`BrowserConfig`](crate::BrowserConfig). The pure CDP
//! engine starts a browser from the browser settings alone; it ignores these (see
//! `LaunchOptionsBuilder::browser_config`).

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    grid_url, CapabilityOverrides, CdpEventCapture, DriverVersion, ExtensionArchive,
    FirefoxOptions, GridProtocol, PageLoadStrategy,
};
use crate::browser::config::DriverMode;
use crate::browser::reuse::ReuseClass;
use crate::error::SeleniumBaseError;
use crate::evasion::options::without_credentials;

/// The WebDriver server a session connects to when nothing names another.
pub const DEFAULT_WEBDRIVER_URL: &str = "http://localhost:4444";

/// The port Selenium Grid and chromedriver listen on in Python SeleniumBase.
const DEFAULT_GRID_PORT: u16 = 4444;

/// The address of a Selenium Grid in three parts (`--server`, `--port`, `--protocol`).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::{GridProtocol, GridSettings};
///
/// let grid = GridSettings {
///     server: Some("hub.example.com".to_owned()),
///     port: Some(4445),
///     protocol: Some(GridProtocol::Https),
/// };
/// assert_eq!(
///     grid.url()?.as_deref(),
///     Some("https://hub.example.com:4445/wd/hub")
/// );
/// assert_eq!(GridSettings::default().url()?, None);
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct GridSettings {
    /// The Grid's host name, which may carry a scheme, a login, a port and a path
    /// (`--server`, `SB_SERVER`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// The Grid's port (`--port`, `SB_PORT`); 4444 unless given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// `http` or `https` (`--protocol`, `SB_PROTOCOL`); `http` unless given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<GridProtocol>,
}

impl GridSettings {
    /// Whether no part of the address is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.server.is_none() && self.port.is_none() && self.protocol.is_none()
    }

    /// The address of the Grid, built the way Python SeleniumBase builds it, or `None` when no
    /// part is set.
    ///
    /// With a `server`, the address is `PROTOCOL://SERVER:PORT/wd/hub`: a scheme in `server`
    /// wins over `protocol` (`https` only), a port in `server` wins over `port`, and `/wd/hub`
    /// is added unless it is there. With a port or a protocol only, the address is the local
    /// `PROTOCOL://localhost:PORT`. See [`grid_url`].
    ///
    /// # Errors
    ///
    /// Returns an error when `server` is empty.
    pub fn url(&self) -> Result<Option<String>, SeleniumBaseError> {
        if self.is_empty() {
            return Ok(None);
        }
        let protocol = self.protocol.unwrap_or_default();
        let port = self.port.unwrap_or(DEFAULT_GRID_PORT);
        match &self.server {
            Some(server) => grid_url(protocol, server, port).map(Some),
            None => Ok(Some(format!("{protocol}://localhost:{port}"))),
        }
    }
}

// Written by hand: the server may carry a cloud provider's access key.
impl fmt::Debug for GridSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GridSettings")
            .field("server", &self.server.as_deref().map(without_credentials))
            .field("port", &self.port)
            .field("protocol", &self.protocol)
            .finish()
    }
}

/// The settings only a WebDriver session reads, held by both
/// [`Settings`](crate::Settings) and [`BrowserConfig`](crate::BrowserConfig).
///
/// Every field has a default that changes nothing; the default session is the one a
/// `BrowserConfig::default()` starts.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SessionSettings {
    /// How the browser is driven: WebDriver, DevTools without a driver binary, or the
    /// undetected mode (`--cdp`, `--uc`, `SB_MODE`).
    pub mode: DriverMode,
    /// The WebDriver server to connect to (`SB_WEBDRIVER_URL`). Unset, the Grid address
    /// [`grid`](Self::grid) names is used, else `http://localhost:4444`; see
    /// [`webdriver_url`](Self::webdriver_url).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdriver_url: Option<String>,
    /// The Selenium Grid (`[session.grid]`: `--server`, `--port`, `--protocol`).
    #[serde(skip_serializing_if = "GridSettings::is_empty")]
    pub grid: GridSettings,
    /// Client-side timeout, in seconds, for each HTTP request sent to a Remote WebDriver
    /// server such as a Selenium Grid (`SB_REMOTE_WEBDRIVER_TIMEOUT`).
    ///
    /// `None` leaves the WebDriver client's own default in place. Set it higher than the
    /// page-load timeout and higher than the time a new-session request may sit in the
    /// grid's queue, otherwise healthy sessions will be cut off. Its purpose is to stop a
    /// command blocking forever when a grid node dies mid-session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_webdriver_timeout_seconds: Option<u64>,
    /// Which chromedriver to run (`--driver-version`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<DriverVersion>,
    /// How long a navigation waits (`--page-load-strategy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_load_strategy: Option<PageLoadStrategy>,
    /// Capabilities to add to the session's (`--cap-string`, `--cap-file`).
    #[serde(skip_serializing_if = "CapabilityOverrides::is_empty")]
    pub capabilities: CapabilityOverrides,
    /// Extension archives to load (`--extension-zip`, `--crx`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extension_archives: Vec<ExtensionArchive>,
    /// Firefox arguments and preferences (`--firefox-arg`, `--firefox-pref`).
    #[serde(skip_serializing_if = "FirefoxOptions::is_empty")]
    pub firefox: FirefoxOptions,
    /// Whether to log DevTools Protocol events (`--log-cdp-events`).
    #[serde(skip_serializing_if = "CdpEventCapture::is_off")]
    pub cdp_events: CdpEventCapture,
    /// Python's `--reuse-session` (`--rs`): the tests of a run share browsers.
    ///
    /// With it on, [`BrowserSession::connect`](crate::BrowserSession::connect) (and so
    /// `BaseCase::new`) attaches to a browser that an earlier test started with the same
    /// configuration and has given back, after cleaning it (other windows closed, the
    /// page's storage deleted, cookies deleted if
    /// [`crumbs`](crate::browser::reuse::crumbs) are on, `about:blank`), and
    /// `BaseCase::quit` gives the browser back instead of closing it. One test holds a
    /// browser at a time; a test that starts while every matching browser is held gets one
    /// of its own, kept for reuse too. Idle browsers close after
    /// [`reuse::idle_timeout`](crate::browser::reuse::idle_timeout) or at
    /// [`shutdown_reused_sessions`](crate::shutdown_reused_sessions). The `sbrs` flag,
    /// `Settings` and `SB_REUSE_SESSION` set it;
    /// [`with_reuse_class_session`](crate::BrowserConfig::with_reuse_class_session) and
    /// [`with_reuse_module_session`](crate::BrowserConfig::with_reuse_module_session) narrow
    /// who shares (`--rcs`). See [`crate::browser::reuse`] for how it works and what differs
    /// from Python.
    pub reuse: bool,
    /// Which tests share a browser when [`reuse`](Self::reuse) is on (Python's `--rcs`).
    /// Unset, every test of the run with the same configuration may get the same browser.
    /// See [`crate::browser::reuse`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reuse_class: Option<ReuseClass>,
    /// Keep no cookies of a reused browser (`--crumbs`), as
    /// [`reuse::set_crumbs(true)`](crate::browser::reuse::set_crumbs) does for the process.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub crumbs: bool,
    /// Whether the session starts the driver itself. Code only; `true` unless a caller
    /// attaches to a driver that already runs.
    pub auto_start_driver: bool,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            mode: DriverMode::default(),
            webdriver_url: None,
            grid: GridSettings::default(),
            remote_webdriver_timeout_seconds: None,
            driver_version: None,
            page_load_strategy: None,
            capabilities: CapabilityOverrides::default(),
            extension_archives: Vec::new(),
            firefox: FirefoxOptions::default(),
            cdp_events: CdpEventCapture::default(),
            reuse: false,
            reuse_class: None,
            crumbs: false,
            auto_start_driver: true,
        }
    }
}

impl SessionSettings {
    /// The WebDriver server to connect to: [`webdriver_url`](Self::webdriver_url) when set,
    /// else the address [`grid`](Self::grid) names, else `http://localhost:4444`.
    ///
    /// A Grid whose address cannot be built (an empty server) gives the default; the
    /// settings file and `Settings::check` refuse it when it is read.
    #[must_use]
    pub fn webdriver_url(&self) -> String {
        self.webdriver_url
            .clone()
            .or_else(|| self.grid.url().ok().flatten())
            .unwrap_or_else(|| DEFAULT_WEBDRIVER_URL.to_owned())
    }

    /// Whether [`webdriver_url`](Self::webdriver_url) is the local default server (or empty),
    /// so that a driver is started on this machine.
    #[must_use]
    pub fn is_default_webdriver_url(&self) -> bool {
        let url = self.webdriver_url();
        url == DEFAULT_WEBDRIVER_URL || url.is_empty()
    }
}

// Written by hand: the WebDriver server URL may carry a cloud provider's access key.
impl fmt::Debug for SessionSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSettings")
            .field("mode", &self.mode)
            .field(
                "webdriver_url",
                &self.webdriver_url.as_deref().map(without_credentials),
            )
            .field("grid", &self.grid)
            .field(
                "remote_webdriver_timeout_seconds",
                &self.remote_webdriver_timeout_seconds,
            )
            .field("driver_version", &self.driver_version)
            .field("page_load_strategy", &self.page_load_strategy)
            .field("capabilities", &self.capabilities)
            .field("extension_archives", &self.extension_archives)
            .field("firefox", &self.firefox)
            .field("cdp_events", &self.cdp_events)
            .field("reuse", &self.reuse)
            .field("reuse_class", &self.reuse_class)
            .field("crumbs", &self.crumbs)
            .field("auto_start_driver", &self.auto_start_driver)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_session_starts_a_local_driver() {
        let session = SessionSettings::default();
        assert!(session.auto_start_driver);
        assert_eq!(session.webdriver_url(), "http://localhost:4444");
        assert!(session.is_default_webdriver_url());
        assert_eq!(session.mode, DriverMode::WebDriver);
    }

    #[test]
    fn the_server_is_the_url_then_the_grid_then_the_local_default() {
        let mut session = SessionSettings::default();
        session.grid.server = Some("hub.example.com".to_owned());
        assert_eq!(
            session.webdriver_url(),
            "http://hub.example.com:4444/wd/hub"
        );
        assert!(!session.is_default_webdriver_url());
        session.webdriver_url = Some("http://127.0.0.1:9515".to_owned());
        assert_eq!(session.webdriver_url(), "http://127.0.0.1:9515");
        session.webdriver_url = None;
        session.grid = GridSettings {
            port: Some(5555),
            ..GridSettings::default()
        };
        assert_eq!(session.webdriver_url(), "http://localhost:5555");
    }

    #[test]
    fn a_grid_with_an_empty_server_is_an_error_and_the_session_falls_back() {
        let grid = GridSettings {
            server: Some(" ".to_owned()),
            ..GridSettings::default()
        };
        assert!(grid.url().is_err());
        let session = SessionSettings {
            grid,
            ..SessionSettings::default()
        };
        assert_eq!(session.webdriver_url(), "http://localhost:4444");
    }

    #[test]
    fn the_debug_text_hides_a_login() {
        let session = SessionSettings {
            webdriver_url: Some("https://USER:SECRET@hub.example.com/wd/hub".to_owned()),
            grid: GridSettings {
                server: Some("USER:KEY@hub.example.com".to_owned()),
                ..GridSettings::default()
            },
            ..SessionSettings::default()
        };
        let text = format!("{session:?}");
        assert!(!text.contains("SECRET"), "{text}");
        assert!(!text.contains("KEY@"), "{text}");
    }

    #[test]
    fn the_default_writes_only_what_it_always_writes() {
        let text = serde_json::to_string(&SessionSettings::default()).unwrap();
        assert_eq!(
            text,
            r#"{"mode":"webdriver","reuse":false,"auto_start_driver":true}"#
        );
        let back: SessionSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(back, SessionSettings::default());
    }
}
