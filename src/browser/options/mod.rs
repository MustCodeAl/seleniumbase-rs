//! Typed launch settings: how a browser, or its driver, is started.
//!
//! These are the Python SeleniumBase options that shape the browser a session
//! opens: preferences such as `--block-images`, the page load strategy, the
//! window's size and position, a proxy's bypass list, mobile emulation,
//! extensions, Firefox arguments and preferences, extra capabilities, the driver
//! version, a remote debugging port, and a virtual display. A
//! [`BrowserSettings`] holds what both engines start and a [`SessionSettings`]
//! what only a WebDriver session reads; [`BrowserConfig`](crate::BrowserConfig)
//! carries one of each, and the setters of [`BrowserConfigBuilder`](crate::BrowserConfigBuilder) are the usual way to set them.
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
//! let config = BrowserConfig::builder()
//!     .block_images(true)
//!     .page_load_strategy(PageLoadStrategy::Eager)
//!     .window_size("1280,720".parse::<WindowSize>()?)
//!     .device_metrics(DeviceMetrics::new(412, 732, 3.0))
//!     .build()?;
//! assert!(config.browser.has(seleniumbase_rs::browser::options::LaunchSwitch::BlockImages));
//! assert_eq!(config.session.page_load_strategy, Some(PageLoadStrategy::Eager));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

mod browser_settings;

mod capabilities;
mod cdp_events;
mod chromium_args;
mod chromium_settings;

mod device;
mod driver;
mod extensions;
mod firefox;
mod locale;
mod network;
mod page_load;
#[cfg(feature = "plugins")]
mod plugins;
mod proxy;
mod proxy_settings;

mod remote;
mod session;
mod switches;
mod virtual_display;
mod window;

pub use browser_settings::BrowserSettings;

pub use capabilities::CapabilityOverrides;
pub use cdp_events::{CdpEvent, CdpEventCapture};
pub use chromium_args::parse_chromium_args;
pub(crate) use chromium_args::switch_args;
pub use chromium_settings::ChromiumSettings;

pub(crate) use device::emulated_phone;
pub use device::{DeviceMetrics, MOBILE_USER_AGENT};
pub use driver::{DriverVersion, ReleaseChannel};
pub(crate) use extensions::Bundled;
pub use extensions::ExtensionArchive;
pub use firefox::{FirefoxOptions, FirefoxPreference};
pub use locale::LocaleCode;
pub use network::NetworkThrottle;
pub use page_load::PageLoadStrategy;
#[cfg(feature = "plugins")]
pub use plugins::BrowserPlugins;
pub use proxy::{PacUrl, ProxyAddress, ProxyBypassList, ProxyScheme};
pub use proxy_settings::ProxySettings;

pub use remote::{grid_url, GridProtocol};
pub use session::{GridSettings, SessionSettings, DEFAULT_WEBDRIVER_URL};
pub use switches::{chrome_preferences, firefox_preferences, nested_preferences, LaunchSwitch};
pub use virtual_display::VirtualDisplay;
pub use window::{WindowPosition, WindowSize};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::browser::config::Browser;
    use crate::config::ad_block_list::AdBlockList;

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
