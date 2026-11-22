//! What both engines start.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::chromium_settings::ChromiumSettings;
use super::proxy_settings::ProxySettings;
use super::{DeviceMetrics, LaunchSwitch, VirtualDisplay, WindowPosition, WindowSize};
use crate::browser::config::Browser;
use crate::config::ad_block_list::AdBlockList;

/// What both engines start: the browser, its window, its proxy, its profile and its
/// switches. See the [module documentation](super).
///
/// [`Settings`](crate::Settings) and [`BrowserConfig`](crate::BrowserConfig) both hold one,
/// and the pure CDP engine reads it whole (`LaunchOptionsBuilder::browser_config`). Every
/// field has a default that changes nothing, so a default value is the browser as it would
/// start without these options: the default [`Browser`] and headed (Python's default). A
/// default [`BrowserConfig`](crate::BrowserConfig) is headless.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserSettings {
    /// Which browser to start (`--browser`, `SB_BROWSER`).
    pub kind: Browser,
    /// The program of the browser (`--binary-location`, `SB_CHROME_BIN`). Unset, the browser
    /// is looked for in the usual places. When `native_spoofing` is enabled and this is unset,
    /// the crate locates the system Chrome binary, patches a cached copy, and sets this field
    /// before launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<PathBuf>,
    /// Runs the browser without a window (`--headless`, `SB_HEADLESS`).
    pub headless: bool,
    /// A page the browser opens as soon as it starts (`--start-page`, `SB_START_PAGE`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The user-agent string the browser sends (`--agent`, `SB_USER_AGENT`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// The browser's language, such as `fr-FR` (`--locale`, `SB_LOCALE`, `SB_LOCALE_CODE`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The timezone for emulation, such as `"America/New_York"` (`--timezone`, `SB_TIMEZONE`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// The geolocation coordinates for emulation (`latitude`, `longitude`, `accuracy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<(f64, f64, Option<f64>)>,
    /// The emulation platform, such as `"Win32"` or `"MacIntel"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Mobile emulation (`--mobile`, `SB_MOBILE`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mobile: bool,
    /// The phone screen to emulate (`--metrics`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_metrics: Option<DeviceMetrics>,
    /// The window's size (`--window-size`); the browser's own default if unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size: Option<WindowSize>,
    /// The window's position (`--window-position`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_position: Option<WindowPosition>,
    /// The proxy (`[browser.proxy]`).
    pub proxy: ProxySettings,
    /// A browser profile folder that keeps cookies and settings between runs
    /// (`--user-data-dir`, `SB_USER_DATA_DIR`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data_dir: Option<PathBuf>,
    /// Unpacked browser extensions to load (`--extension-dir`, `SB_EXTENSION_DIR`, separated
    /// by commas there).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<PathBuf>,
    /// The `--ad-block` setting: refuse the requests of advertising and tracking hosts.
    ///
    /// A Chromium WebDriver session (Chrome, Chromium, Edge, Brave, Comet, Opera, Chrome for
    /// Testing, `chrome-headless-shell`) sends the hosts of an [`AdBlockList`] to the browser
    /// with `Network.setBlockedURLs` once it is connected: the built-in list, or the one set
    /// with [`ad_block_list`](Self::ad_block_list). The block belongs to a window, so the
    /// session repeats it for each window it switches to with `switch_to_window` or
    /// `switch_to_new_window`; a window reached some other way needs
    /// [`BrowserSession::apply_ad_block`](crate::browser::session::BrowserSession::apply_ad_block).
    ///
    /// What it does not reach, as seen in Chrome 155 (`tests/ad_block_chrome.rs`): a URL that
    /// names a port is not matched, and a frame of another site is not stopped from loading,
    /// nor are its own requests blocked. A frame of the page's own site is blocked like the
    /// page.
    ///
    /// Firefox, Safari and Internet Explorer have no DevTools channel through WebDriver, so
    /// the setting blocks nothing there (a warning says so). The pure CDP engine blocks hosts
    /// with `LaunchOptionsBuilder::ad_block`, and `BaseCase::ad_block` removes ad elements
    /// from a page that loaded.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ad_block: bool,
    /// The hosts to refuse when `ad_block` is on. Unset, the built-in list is used; see
    /// [`AdBlockList`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ad_block_list: Option<AdBlockList>,
    /// The on/off settings that were asked for (`[browser.switches]`). A switch is on when the
    /// map says `true`; one that is missing or `false` is off. Use [`has`](Self::has) and
    /// [`set`](Self::set).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub switches: BTreeMap<LaunchSwitch, bool>,
    /// The Chromium command-line settings with no switch of their own
    /// (`[browser.chromium]`).
    pub chromium: ChromiumSettings,
    /// The virtual screen for a headed browser on Linux (`--xvfb`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_display: Option<VirtualDisplay>,
    /// The port of Chrome's remote debugger (`--remote-debug`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debugging_port: Option<u16>,
    /// Whether the remote debugger may take another port when `debugging_port` is in use.
    /// Set by [`remote_debug`](crate::BrowserConfigBuilder::remote_debug), which asks for
    /// the default port only; a port named with
    /// [`remote_debug_port`](crate::BrowserConfigBuilder::remote_debug_port) is an error
    /// when it is busy.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub debugging_any_port: bool,
}

impl BrowserSettings {
    /// Whether `switch` is on.
    #[must_use]
    pub fn has(&self, switch: LaunchSwitch) -> bool {
        self.switches.get(&switch).copied().unwrap_or(false)
    }

    /// Turns `switch` on or off. Off removes the entry, so a value that sets nothing is a
    /// value that writes nothing.
    pub fn set(&mut self, switch: LaunchSwitch, on: bool) {
        if on {
            self.switches.insert(switch, true);
        } else {
            self.switches.remove(&switch);
        }
    }

    /// The switches that are on, in the order of [`LaunchSwitch`].
    pub fn switches_on(&self) -> impl Iterator<Item = LaunchSwitch> + '_ {
        self.switches
            .iter()
            .filter(|(_, on)| **on)
            .map(|(switch, _)| *switch)
    }
}
