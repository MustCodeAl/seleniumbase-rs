//! Reading a [`BrowserConfig`] into pure CDP launch options.
//!
//! Python's `sb_cdp.Chrome(**kwargs)` takes the same options as the WebDriver
//! engine; [`LaunchOptionsBuilder::browser_config`](super::LaunchOptionsBuilder::browser_config) gives a pure CDP test the
//! settings a `BaseCase` test gets from the settings file and `SB_*`.

use std::net::{Ipv4Addr, TcpListener};

use serde_json::{json, Map};

use super::layers::{Layer, ProxySpec, SharedFingerprint};
use super::{Proxy, Redacted};
use crate::browser::config::{Browser as BrowserKind, BrowserConfig};
use crate::browser::options::{
    BrowserSettings, ChromiumSettings, LaunchSwitch, LocaleCode, ProxyAddress, ProxySettings,
    SessionSettings,
};
use crate::error::SeleniumBaseError;

/// A proxy address as Chrome reads it, with the login in front when there is
/// one (the form [`Proxy::parse`] takes): a `socks5h` proxy is written
/// `socks5`.
pub(super) fn proxy_spec(address: &ProxyAddress) -> String {
    let server = address.chrome_server();
    match address.credentials() {
        None => server,
        Some((user, password)) => match server.split_once("://") {
            Some((scheme, rest)) => format!("{scheme}://{user}:{password}@{rest}"),
            None => format!("{user}:{password}@{server}"),
        },
    }
}

/// Reads a proxy string: the pure CDP engine's own lenient reading, or, for a
/// proxy that came from a [`BrowserConfig`], the check Python makes.
pub(super) fn parse_proxy(text: &str, checked: bool) -> Result<Proxy, SeleniumBaseError> {
    if checked {
        Proxy::parse(&proxy_spec(&text.parse::<ProxyAddress>()?))
    } else {
        Proxy::parse(text)
    }
}

/// The settings of `config` that the pure CDP engine cannot use, by name.
pub(super) fn unused_settings(config: &BrowserConfig) -> Vec<&'static str> {
    // Destructured without `..`: a field added to the session settings does not compile until
    // it is listed here as used or unused by the pure CDP engine.
    let SessionSettings {
        mode: _,
        webdriver_url: _,
        grid: _,
        remote_webdriver_timeout_seconds: _,
        driver_version,
        page_load_strategy,
        capabilities,
        extension_archives,
        firefox,
        cdp_events,
        reuse: _,
        reuse_class: _,
        crumbs: _,
        auto_start_driver: _,
    } = &config.session;
    let mut unused = Vec::new();
    let mut note = |used_by_webdriver: bool, name: &'static str| {
        if used_by_webdriver {
            unused.push(name);
        }
    };
    note(page_load_strategy.is_some(), "page_load_strategy");
    note(!capabilities.is_empty(), "capabilities");
    note(driver_version.is_some(), "driver_version");
    note(!firefox.is_empty(), "firefox");
    note(!cdp_events.is_off(), "cdp_events");
    note(!extension_archives.is_empty(), "extension_archives");
    note(config.browser.ad_block_list.is_some(), "ad_block_list");
    note(
        !config.cdp_discovery && !config.is_default_webdriver_url(),
        "webdriver_url",
    );
    for switch in config.browser.switches_on() {
        let name = match switch {
            LaunchSwitch::EnableSync => "enable_sync",
            LaunchSwitch::AutomationExtension => "use_auto_ext",
            LaunchSwitch::RecorderExtension => "recorder_ext",
            LaunchSwitch::ProxyForDriverDownload => "proxy_driver",
            LaunchSwitch::EscEnd => "esc_end",
            LaunchSwitch::RecSbCdp => "rec_sb_cdp",
            LaunchSwitch::Headless1 => "headless1",
            _ => continue,
        };
        unused.push(name);
    }
    unused
}

/// The remote debugging port to ask for: the one named, or, when it was only
/// the default and something listens there, none (the browser picks a free
/// port).
fn debugging_port(browser: &BrowserSettings) -> Option<u16> {
    let port = browser.debugging_port?;
    if browser.debugging_any_port && TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err() {
        tracing::debug!(
            port,
            "the default remote debugging port is in use; a free one is used"
        );
        return None;
    }
    Some(port)
}

impl Layer {
    /// Everything of `config` that the pure CDP engine can use, as Python's
    /// `sb_cdp.Chrome(**kwargs)` reads the same options, for the config layer of
    /// [`LaunchOptionsBuilder::browser_config`](super::LaunchOptionsBuilder::browser_config).
    ///
    /// A setting the config only turns on (`mobile`, `ad_block`, a switch) is set when it is on
    /// and left unset when it is off, so that it cannot undo what a layer above it asks for.
    pub(super) fn from_config(config: &BrowserConfig) -> Self {
        let unused = unused_settings(config);
        if !unused.is_empty() {
            tracing::debug!("not used by the pure CDP engine: {}", unused.join(", "));
        }
        if !config.browser.kind.is_chromium_based() {
            tracing::warn!(
                "the pure CDP engine drives Chromium browsers only; {} is not used",
                config.browser.kind.name()
            );
        }
        Self {
            cdp_url: config
                .cdp_url
                .clone()
                .or_else(|| config.cdp_discovery.then(|| config.session.webdriver_url()))
                .map(Redacted),
            cdp_target: config.cdp_target.clone(),
            fingerprint: config.fingerprint.as_ref().map(SharedFingerprint::new),
            #[cfg(feature = "plugins")]
            plugins: Some(config.plugins().clone()),
            ..Self::of_browser(&config.browser)
        }
    }

    /// The part of a `BrowserConfig` both engines start: the [`BrowserSettings`] they share.
    ///
    /// `BrowserSettings` is destructured without `..`, so a field added to it does not compile
    /// until the pure CDP engine reads it here or names it unused.
    fn of_browser(browser: &BrowserSettings) -> Self {
        let BrowserSettings {
            kind,
            executable_path,
            headless,
            url,
            user_agent,
            lang,
            mobile,
            device_metrics,
            window_size,
            window_position,
            proxy:
                ProxySettings {
                    server: proxy_server,
                    pac_url,
                    bypass_list,
                },
            user_data_dir,
            extensions,
            ad_block,
            // Not a launch setting: the WebDriver session sends the list over DevTools.
            ad_block_list: _,
            switches: _,
            chromium:
                ChromiumSettings {
                    args,
                    disable_features,
                    host_resolver_rules,
                },
            virtual_display,
            debugging_port: _,
            debugging_any_port: _,
        } = browser;

        // A binary path and Chromium are one choice of browser; the path comes first.
        let (executable_path, use_chromium) = match executable_path {
            Some(path) => (Some(path.clone()), None),
            None => (None, matches!(kind, BrowserKind::Chromium).then_some(true)),
        };
        let locale = lang.as_deref().map(LocaleCode::normalise);
        let mut prefs = Map::new();
        if let Some(code) = &locale {
            prefs.insert("intl.accept_languages".to_owned(), json!(code));
        }
        let mut layer = Self {
            headless: Some(*headless),
            use_chromium,
            executable_path,
            ad_block: ad_block.then_some(true),
            user_agent: user_agent.clone(),
            lang: locale,
            proxy: proxy_server.clone().map(|text| ProxySpec {
                text: Redacted(text),
                checked: true,
            }),
            proxy_pac_url: pac_url.clone().map(Redacted),
            proxy_bypass_list: Some(bypass_list.clone()),
            user_data_dir: user_data_dir.clone(),
            url: url.clone(),
            window_size: window_size.map(|size| (size.width, size.height)),
            window_position: window_position.map(|position| (position.x, position.y)),
            mobile: mobile.then_some(true),
            device_metrics: *device_metrics,
            virtual_display: *virtual_display,
            debugging_port: debugging_port(browser),
            extensions: extensions.clone(),
            extra_args: args
                .iter()
                .map(|arg| {
                    if arg.starts_with('-') {
                        arg.clone()
                    } else {
                        format!("--{arg}")
                    }
                })
                .collect(),
            prefs,
            disable_features: disable_features.clone(),
            host_resolver_rules: host_resolver_rules.iter().cloned().collect(),
            ..Self::default()
        };
        for switch in browser.switches_on() {
            layer.set_switch(switch, true);
        }
        layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::options::{CapabilityOverrides, CdpEventCapture, PageLoadStrategy};
    use crate::sb_cdp::LaunchOptionsBuilder;

    #[test]
    fn a_plain_config_leaves_nothing_unused() {
        assert_eq!(
            unused_settings(&BrowserConfig::default()),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn what_only_the_webdriver_engine_uses_is_listed() {
        let mut config = BrowserConfig::default();
        config.session.page_load_strategy = Some(PageLoadStrategy::Eager);
        config.session.driver_version = Some("126".parse().unwrap());
        config.session.cdp_events = CdpEventCapture::Always;
        config.session.capabilities = CapabilityOverrides::from_text(r#"{"name":"t"}"#).unwrap();
        config.browser.set(LaunchSwitch::EscEnd, true);
        config.browser.set(LaunchSwitch::BlockImages, true);
        assert_eq!(
            unused_settings(&config),
            [
                "page_load_strategy",
                "capabilities",
                "driver_version",
                "cdp_events",
                "esc_end"
            ]
        );
    }

    #[test]
    fn a_login_stays_in_the_proxy_the_browser_is_given_without_it() {
        let address: ProxyAddress = "socks5h://10.0.0.1:1080".parse().unwrap();
        assert_eq!(proxy_spec(&address), "socks5://10.0.0.1:1080");
        let address: ProxyAddress = "https://alice:s3:cret@proxy.example:8443".parse().unwrap();
        let proxy = Proxy::parse(&proxy_spec(&address)).unwrap();
        assert_eq!(proxy.server(), "https://proxy.example:8443");
        assert_eq!(proxy.credentials(), Some(("alice", "s3:cret")));
    }

    #[test]
    fn a_proxy_from_a_config_is_checked_and_an_explicit_one_is_not() {
        assert!(parse_proxy("ftp://proxy.example:21", true).is_err());
        assert!(parse_proxy("host:notaport", true).is_err());
        assert!(parse_proxy("ftp://proxy.example:21", false).is_ok());
    }

    #[test]
    fn a_bad_proxy_in_a_config_fails_the_build_without_echoing_the_password() {
        let config = BrowserConfig::default().with_proxy("user:hunter2@host:notaport");
        let error = LaunchOptionsBuilder::default()
            .browser_config(&config)
            .build()
            .unwrap_err();
        assert!(!error.to_string().contains("hunter2"), "{error}");
        assert!(!format!("{error:?}").contains("hunter2"), "{error:?}");
    }
}
