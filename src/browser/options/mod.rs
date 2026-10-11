//! Typed launch settings: how a browser, or its driver, is started.
//!
//! These are the Python SeleniumBase options that shape the browser a session
//! opens: preferences such as `--block-images`, the page load strategy, the
//! window's size and position, a proxy's bypass list, mobile emulation,
//! extensions, Firefox arguments and preferences, extra capabilities, the driver
//! version, a remote debugging port, and a virtual display. A
//! [`LaunchSettings`] holds them, [`BrowserConfig`](crate::BrowserConfig) carries
//! one, and its `with_*` methods are the usual way to set them.
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
//! assert!(config.launch.has(seleniumbase_rs::browser::options::LaunchSwitch::BlockImages));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::config::ad_block_list::AdBlockList;

mod capabilities;
mod cdp_events;
mod device;
mod driver;
mod extensions;
mod firefox;
mod page_load;
mod proxy;
mod switches;
mod virtual_display;
mod window;

pub use capabilities::CapabilityOverrides;
pub use cdp_events::{CdpEvent, CdpEventCapture};
pub use device::DeviceMetrics;
pub use driver::{DriverVersion, ReleaseChannel};
pub(crate) use extensions::Bundled;
pub use extensions::ExtensionArchive;
pub use firefox::{FirefoxOptions, FirefoxPreference};
pub use page_load::PageLoadStrategy;
pub use proxy::ProxyBypassList;
pub use switches::{chrome_preferences, firefox_preferences, nested_preferences, LaunchSwitch};
pub use virtual_display::VirtualDisplay;
pub use window::{WindowPosition, WindowSize};

/// The Python SeleniumBase options that shape how a browser or its driver is
/// started. See the [module documentation](self).
///
/// Every field has a default that changes nothing, so a default value is the
/// browser as it would start without these options.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct LaunchSettings {
    /// The on/off settings that are on.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub switches: BTreeSet<LaunchSwitch>,
    /// How long a navigation waits (`--page-load-strategy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_load_strategy: Option<PageLoadStrategy>,
    /// The window's size (`--window-size`); the browser's own default if unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size: Option<WindowSize>,
    /// The window's position (`--window-position`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_position: Option<WindowPosition>,
    /// The phone screen to emulate (`--metrics`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_metrics: Option<DeviceMetrics>,
    /// Hosts that skip the proxy (`--proxy-bypass-list`).
    #[serde(skip_serializing_if = "ProxyBypassList::is_empty")]
    pub proxy_bypass_list: ProxyBypassList,
    /// Extension archives to load (`--extension-zip`, `--crx`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extension_archives: Vec<ExtensionArchive>,
    /// Firefox arguments and preferences (`--firefox-arg`, `--firefox-pref`).
    #[serde(skip_serializing_if = "FirefoxOptions::is_empty")]
    pub firefox: FirefoxOptions,
    /// Capabilities to add to the session's (`--cap-string`, `--cap-file`).
    #[serde(skip_serializing_if = "CapabilityOverrides::is_empty")]
    pub capabilities: CapabilityOverrides,
    /// Which chromedriver to run (`--driver-version`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<DriverVersion>,
    /// The port of Chrome's remote debugger (`--remote-debug`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_debug_port: Option<u16>,
    /// Whether to log DevTools Protocol events (`--log-cdp-events`).
    #[serde(skip_serializing_if = "CdpEventCapture::is_off")]
    pub cdp_events: CdpEventCapture,
    /// The virtual screen for a headed browser on Linux (`--xvfb`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_display: Option<VirtualDisplay>,
    /// The hosts to refuse when `ad_block` is on. Unset, the built-in list is
    /// used; see [`AdBlockList`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ad_block_list: Option<AdBlockList>,
}

impl LaunchSettings {
    /// Whether `switch` is on.
    #[must_use]
    pub fn has(&self, switch: LaunchSwitch) -> bool {
        self.switches.contains(&switch)
    }

    /// Turns `switch` on or off.
    pub fn set(&mut self, switch: LaunchSwitch, on: bool) {
        if on {
            self.switches.insert(switch);
        } else {
            self.switches.remove(&switch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing_and_writes_nothing() {
        let settings = LaunchSettings::default();
        assert_eq!(serde_json::to_string(&settings).unwrap(), "{}");
        assert!(settings.switches.is_empty());
        assert_eq!(
            serde_json::from_str::<LaunchSettings>("{}").unwrap(),
            settings
        );
    }

    #[test]
    fn a_switch_is_turned_on_and_off() {
        let mut settings = LaunchSettings::default();
        settings.set(LaunchSwitch::DoNotTrack, true);
        settings.set(LaunchSwitch::DoNotTrack, true);
        assert!(settings.has(LaunchSwitch::DoNotTrack));
        assert!(!settings.has(LaunchSwitch::BlockImages));
        assert_eq!(settings.switches.len(), 1);
        settings.set(LaunchSwitch::DoNotTrack, false);
        assert!(!settings.has(LaunchSwitch::DoNotTrack));
    }

    #[test]
    fn settings_survive_a_round_trip_through_json() {
        let mut settings = LaunchSettings::default();
        settings.set(LaunchSwitch::BlockImages, true);
        settings.page_load_strategy = Some(PageLoadStrategy::None);
        settings.window_size = Some(WindowSize::new(800, 600));
        settings.window_position = Some(WindowPosition::new(-10, 5));
        settings.device_metrics = Some(DeviceMetrics::new(360, 640, 2.0));
        settings.proxy_bypass_list = ProxyBypassList::new(["*.a.com"]).unwrap();
        settings.extension_archives = ExtensionArchive::parse_list("x.crx").unwrap();
        settings.firefox.add_args("private").unwrap();
        settings.capabilities = CapabilityOverrides::from_text(r#"{"name":"t"}"#).unwrap();
        settings.driver_version = Some("126".parse().unwrap());
        settings.virtual_display = Some(VirtualDisplay::new(1024, 768));
        settings.remote_debug_port = Some(9222);
        settings.cdp_events = CdpEventCapture::Always;
        settings.ad_block_list = Some(AdBlockList::from_hosts(["ads.example.com"]).unwrap());

        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<LaunchSettings>(&text).unwrap(),
            settings
        );
    }

    #[test]
    fn unset_fields_are_filled_with_defaults_when_reading() {
        let settings: LaunchSettings =
            serde_json::from_str(r#"{"page_load_strategy":"eager","switches":["block-images"]}"#)
                .unwrap();
        assert_eq!(settings.page_load_strategy, Some(PageLoadStrategy::Eager));
        assert!(settings.has(LaunchSwitch::BlockImages));
        assert_eq!(settings.cdp_events, CdpEventCapture::Off);
    }
}
