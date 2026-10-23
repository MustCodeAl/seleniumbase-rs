//! Reading a [`BrowserConfig`] into pure CDP launch options.
//!
//! Python's `sb_cdp.Chrome(**kwargs)` takes the same options as the WebDriver
//! engine; [`LaunchOptionsBuilder::browser_config`] gives a pure CDP test the
//! settings a `BaseCase` test gets from the settings file and `SB_*`.

use std::net::{Ipv4Addr, TcpListener};

use serde_json::json;

use super::{LaunchOptionsBuilder, Proxy, Redacted};
use crate::browser::config::{Browser as BrowserKind, BrowserConfig};
use crate::browser::options::{LaunchSwitch, LocaleCode, ProxyAddress};
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
    let launch = &config.launch;
    let mut unused = Vec::new();
    let mut note = |used_by_webdriver: bool, name: &'static str| {
        if used_by_webdriver {
            unused.push(name);
        }
    };
    note(launch.page_load_strategy.is_some(), "page_load_strategy");
    note(!launch.capabilities.is_empty(), "capabilities");
    note(launch.driver_version.is_some(), "driver_version");
    note(!launch.firefox.is_empty(), "firefox");
    note(!launch.cdp_events.is_off(), "cdp_events");
    note(!launch.extension_archives.is_empty(), "extension_archives");
    note(launch.ad_block_list.is_some(), "ad_block_list");
    note(
        !config.webdriver_url.is_empty() && !config.is_default_webdriver_url(),
        "webdriver_url",
    );
    for switch in &launch.switches {
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
fn debugging_port(config: &BrowserConfig) -> Option<u16> {
    let launch = &config.launch;
    let port = launch.remote_debug_port?;
    if launch.remote_debug_any_port && TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err() {
        tracing::debug!(
            port,
            "the default remote debugging port is in use; a free one is used"
        );
        return None;
    }
    Some(port)
}

impl LaunchOptionsBuilder {
    /// Everything of `config` that the pure CDP engine can use, as Python's
    /// `sb_cdp.Chrome(**kwargs)` reads the same options.
    ///
    /// Headless or headed, the browser (a binary path, or Chromium), the user
    /// agent, the locale (`--lang` and `intl.accept_languages`), the proxy and
    /// its bypass list, a PAC file, the profile folder, unpacked extension
    /// folders, the start page, extra arguments, mobile emulation and its
    /// screen, the window size and position, the virtual display, the
    /// remote debugging port, `ad_block`, a fingerprint, the on/off switches
    /// (see [`switch`](Self::switch)), the disabled features and the host
    /// resolver rules.
    ///
    /// What only the WebDriver engine can use (the page load strategy,
    /// capabilities, the driver version, Firefox options, the DevTools event
    /// log, extension archives, a custom ad block list, a WebDriver URL) is
    /// listed at debug level and left out. A proxy or PAC address that is not
    /// well formed makes [`build`](Self::build) fail, as it makes the WebDriver
    /// engine fail. Settings made on the builder after this call replace the
    /// ones read here; the fingerprint is applied last, so settings made
    /// before it win over it.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let config = BrowserConfig::default().with_headless(true).with_mobile(true);
    /// let options = LaunchOptions::builder().browser_config(&config).build()?;
    /// assert!(options.is_headless());
    /// assert!(options
    ///     .browser_args(Path::new("/tmp/profile"))
    ///     .iter()
    ///     .any(|arg| arg.starts_with("--user-agent=") && arg.contains("Android")));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    #[must_use]
    pub fn browser_config(mut self, config: &BrowserConfig) -> Self {
        let unused = unused_settings(config);
        if !unused.is_empty() {
            tracing::debug!("not used by the pure CDP engine: {}", unused.join(", "));
        }
        let launch = &config.launch;

        self = self.headless(config.headless).ad_block(config.ad_block);
        if let Some(path) = &config.browser_binary_path {
            self = self.executable_path(path);
        } else if matches!(config.browser, BrowserKind::Chromium) {
            self = self.use_chromium(true);
        }
        if !config.browser.is_chromium_based() {
            tracing::warn!(
                "the pure CDP engine drives Chromium browsers only; {} is not used",
                config.browser.name()
            );
        }
        if let Some(user_agent) = &config.user_agent {
            self = self.user_agent(user_agent);
        }
        if let Some(locale) = &config.locale {
            let code = LocaleCode::normalise(locale);
            self = self
                .preference("intl.accept_languages", json!(code))
                .lang(code);
        }
        if let Some(proxy) = &config.proxy {
            self.proxy = Some(Redacted(proxy.clone()));
            self.checked_proxy = true;
        }
        if let Some(pac) = &config.proxy_pac_url {
            self = self.proxy_pac_url(pac);
        }
        self = self.proxy_bypass_list(launch.proxy_bypass_list.clone());
        if let Some(dir) = &config.user_data_dir {
            self = self.user_data_dir(dir);
        }
        for dir in config.extension_dir.iter().flat_map(|dirs| dirs.split(',')) {
            let dir = dir.trim();
            if !dir.is_empty() {
                self = self.extension(dir);
            }
        }
        if let Some(url) = &config.start_page {
            self = self.url(url);
        }
        if let Some(size) = launch.window_size {
            self = self.window_size(size.width, size.height);
        }
        if let Some(position) = launch.window_position {
            self = self.window_position(position.x, position.y);
        }
        self.mobile = config.mobile;
        self.device_metrics = launch.device_metrics;
        if let Some(display) = launch.virtual_display {
            self = self.virtual_display(display);
        }
        if let Some(port) = debugging_port(config) {
            self = self.debugging_port(port);
        }
        for switch in &launch.switches {
            self = self.switch(*switch, true);
        }
        self = self.disable_features(launch.disable_features.iter().cloned());
        if let Some(rules) = &launch.host_resolver_rules {
            self = self.host_resolver_rules(rules);
        }
        for arg in &config.extra_args {
            if arg.starts_with('-') {
                self = self.arg(arg);
            } else {
                self = self.arg(format!("--{arg}"));
            }
        }
        if let Some(fingerprint) = &config.fingerprint {
            self = self.fingerprint(fingerprint);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::options::{CapabilityOverrides, CdpEventCapture, PageLoadStrategy};

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
        config.launch.page_load_strategy = Some(PageLoadStrategy::Eager);
        config.launch.driver_version = Some("126".parse().unwrap());
        config.launch.cdp_events = CdpEventCapture::Always;
        config.launch.capabilities = CapabilityOverrides::from_text(r#"{"name":"t"}"#).unwrap();
        config.launch.set(LaunchSwitch::EscEnd, true);
        config.launch.set(LaunchSwitch::BlockImages, true);
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
        let config = BrowserConfig {
            proxy: Some("user:hunter2@host:notaport".to_owned()),
            ..BrowserConfig::default()
        };
        let error = LaunchOptionsBuilder::default()
            .browser_config(&config)
            .build()
            .unwrap_err();
        assert!(!error.to_string().contains("hunter2"), "{error}");
        assert!(!format!("{error:?}").contains("hunter2"), "{error:?}");
    }
}
