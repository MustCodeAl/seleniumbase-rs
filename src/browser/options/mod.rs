//! Typed launch settings: how a browser, or its driver, is started.
//!
//! These are the Python SeleniumBase options that shape the browser a session
//! opens: preferences such as `--block-images`, the page load strategy, the
//! window's size and position, a proxy's bypass list, mobile emulation,
//! extensions, Firefox arguments and preferences, extra capabilities, the driver
//! version, a remote debugging port, and a virtual display. A
//! [`BrowserSettings`] holds what both engines start and a [`SessionSettings`]
//! what only a WebDriver session reads; [`BrowserConfig`](crate::BrowserConfig)
//! carries one of each, and its `with_*` methods are the usual way to set them.
//!
//! Every setting is a value with a name: an enum, or a newtype that checks
//! what it is given when it is built, so a bad window size or driver version is
//! an error at configuration time rather than a failed launch. Each type reads
//! the text the Python command line takes (`"1280,720"`, `"eager"`,
//! `"*.example.com;localhost"`) through [`FromStr`](std::str::FromStr).
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::browser::options::{DeviceMetrics, PageLoadStrategy, WindowSize};
//! use seleniumbase_rs::BrowserConfig;
//!
//! let config = BrowserConfig::default()
//!     .with_block_images(true)
//!     .with_page_load_strategy(PageLoadStrategy::Eager)
//!     .with_window_size("1280,720".parse::<WindowSize>()?)
//!     .with_device_metrics(DeviceMetrics::new(412, 732, 3.0));
//! assert!(config.browser.has(seleniumbase_rs::browser::options::LaunchSwitch::BlockImages));
//! assert_eq!(config.session.page_load_strategy, Some(PageLoadStrategy::Eager));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::browser::config::Browser;
use crate::config::ad_block_list::AdBlockList;

mod capabilities;
mod cdp_events;
mod chromium_args;
mod device;
mod driver;
mod extensions;
mod firefox;
mod locale;
mod page_load;
#[cfg(feature = "plugins")]
mod plugins;
mod proxy;
mod remote;
mod session;
mod switches;
mod virtual_display;
mod window;

pub use capabilities::CapabilityOverrides;
pub use cdp_events::{CdpEvent, CdpEventCapture};
pub use chromium_args::parse_chromium_args;
pub(crate) use chromium_args::switch_args;
pub(crate) use device::emulated_phone;
pub use device::{DeviceMetrics, MOBILE_USER_AGENT};
pub use driver::{DriverVersion, ReleaseChannel};
pub(crate) use extensions::Bundled;
pub use extensions::ExtensionArchive;
pub use firefox::{FirefoxOptions, FirefoxPreference};
pub use locale::LocaleCode;
pub use page_load::PageLoadStrategy;
#[cfg(feature = "plugins")]
pub use plugins::BrowserPlugins;
pub use proxy::{PacUrl, ProxyAddress, ProxyBypassList, ProxyScheme};
pub use remote::{grid_url, GridProtocol};
pub use session::{GridSettings, SessionSettings, DEFAULT_WEBDRIVER_URL};
pub use switches::{chrome_preferences, firefox_preferences, nested_preferences, LaunchSwitch};
pub use virtual_display::VirtualDisplay;
pub use window::{WindowPosition, WindowSize};

/// The proxy of a browser: the server, a proxy auto-config script, and the hosts that skip
/// the proxy.
///
/// Both engines read it, so it belongs to the [`BrowserSettings`] they share.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct ProxySettings {
    /// The proxy server, written `[scheme://][user:password@]host:port` (`--proxy`,
    /// `SB_PROXY`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// A proxy auto-config script the browser fetches to choose its proxy
    /// (`--proxy-pac-url`, `SB_PROXY_PAC_URL`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pac_url: Option<String>,
    /// Hosts that skip the proxy (`--proxy-bypass-list`, `SB_PROXY_BYPASS_LIST`).
    #[serde(skip_serializing_if = "ProxyBypassList::is_empty")]
    pub bypass_list: ProxyBypassList,
}

// Written by hand: the server and the script's address may carry a password.
impl std::fmt::Debug for ProxySettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::evasion::options::without_credentials;
        f.debug_struct("ProxySettings")
            .field("server", &self.server.as_deref().map(without_credentials))
            .field("pac_url", &self.pac_url.as_deref().map(without_credentials))
            .field("bypass_list", &self.bypass_list)
            .finish()
    }
}

/// The Chromium command-line settings that have no switch of their own.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct ChromiumSettings {
    /// Extra command-line arguments (`--chromium-arg`, `SB_CHROMIUM_ARG`), each written with
    /// its leading `--`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Chromium features to switch off (`--disable-features`), joined with the session's own
    /// into the one switch Chrome reads.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disable_features: Vec<String>,
    /// Rules that map host names to addresses (`--host-resolver-rules`), sent as one
    /// Chromium switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_resolver_rules: Option<String>,
}

/// What both engines start: the browser, its window, its proxy, its profile and its
/// switches. See the [module documentation](self).
///
/// [`Settings`](crate::Settings) and [`BrowserConfig`](crate::BrowserConfig) both hold one,
/// and the pure CDP engine reads it whole (`LaunchOptionsBuilder::browser_config`). Every
/// field has a default that changes nothing, so a default value is the browser as it would
/// start without these options: the default [`Browser`] and headed (Python's default). A
/// default [`BrowserConfig`](crate::BrowserConfig) is headless.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
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
    /// Set by [`with_remote_debug`](crate::BrowserConfig::with_remote_debug), which asks for
    /// the default port only; a port named with
    /// [`with_remote_debug_port`](crate::BrowserConfig::with_remote_debug_port) is an error
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing_and_writes_only_the_browser() {
        let settings = BrowserSettings::default();
        assert_eq!(
            serde_json::to_string(&settings).unwrap(),
            r#"{"kind":"Chrome","headless":false,"proxy":{},"chromium":{}}"#
        );
        assert!(settings.switches.is_empty());
        assert_eq!(
            serde_json::from_str::<BrowserSettings>("{}").unwrap(),
            settings
        );
    }

    #[test]
    fn a_switch_is_turned_on_and_off() {
        let mut settings = BrowserSettings::default();
        settings.set(LaunchSwitch::DoNotTrack, true);
        settings.set(LaunchSwitch::DoNotTrack, true);
        assert!(settings.has(LaunchSwitch::DoNotTrack));
        assert!(!settings.has(LaunchSwitch::BlockImages));
        assert_eq!(settings.switches.len(), 1);
        settings.set(LaunchSwitch::DoNotTrack, false);
        assert!(!settings.has(LaunchSwitch::DoNotTrack));
        assert!(settings.switches.is_empty());
    }

    #[test]
    fn a_switch_the_map_says_false_is_off() {
        let settings: BrowserSettings =
            serde_json::from_str(r#"{"switches":{"dark-mode":false,"block-images":true}}"#)
                .unwrap();
        assert!(!settings.has(LaunchSwitch::DarkMode));
        assert!(settings.has(LaunchSwitch::BlockImages));
        assert_eq!(
            settings.switches_on().collect::<Vec<_>>(),
            [LaunchSwitch::BlockImages]
        );
    }

    #[test]
    fn settings_survive_a_round_trip_through_json() {
        let mut settings = BrowserSettings::default();
        settings.set(LaunchSwitch::BlockImages, true);
        settings.kind = Browser::Firefox;
        settings.headless = true;
        settings.url = Some("https://example.com".to_owned());
        settings.window_size = Some(WindowSize::new(800, 600));
        settings.window_position = Some(WindowPosition::new(-10, 5));
        settings.device_metrics = Some(DeviceMetrics::new(360, 640, 2.0));
        settings.proxy.server = Some("proxy.example.com:3128".to_owned());
        settings.proxy.bypass_list = ProxyBypassList::new(["*.a.com"]).unwrap();
        settings.extensions = vec![PathBuf::from("/opt/ext")];
        settings.chromium.disable_features = vec!["Translate".to_owned()];
        settings.chromium.host_resolver_rules = Some("MAP a.test 127.0.0.1".to_owned());
        settings.virtual_display = Some(VirtualDisplay::new(1024, 768));
        settings.debugging_port = Some(9222);
        settings.ad_block_list = Some(AdBlockList::from_hosts(["ads.example.com"]).unwrap());

        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<BrowserSettings>(&text).unwrap(),
            settings
        );
    }

    #[test]
    fn unset_fields_are_filled_with_defaults_when_reading() {
        let settings: BrowserSettings =
            serde_json::from_str(r#"{"headless":true,"switches":{"block-images":true}}"#).unwrap();
        assert!(settings.headless);
        assert!(settings.has(LaunchSwitch::BlockImages));
        assert_eq!(settings.kind, Browser::Chrome);
    }

    #[test]
    fn the_debug_text_hides_the_proxy_password() {
        let mut settings = BrowserSettings::default();
        settings.proxy.server = Some("http://user:SECRET@proxy.example.com:3128".to_owned());
        settings.proxy.pac_url = Some("https://user:SECRET@proxy.example.com/p.pac".to_owned());
        let text = format!("{settings:?}");
        assert!(!text.contains("SECRET"), "{text}");
        assert!(text.contains("proxy.example.com"), "{text}");
    }
}
