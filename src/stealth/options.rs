//! Chrome / Edge option helpers for undetected-chrome (UC) stealth profiles.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::browser::config::BrowserConfig;
use crate::browser::options::{chrome_preferences, Bundled, LaunchSettings, LaunchSwitch};
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::Fingerprint;
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
use thirtyfour::LoggingPrefsLogLevel;

/// The user agent a phone-emulating session reports when the caller names none:
/// Chrome on Android in its frozen format (release 10, model `K`), claiming
/// [`PRESET_CHROMIUM_MAJOR`](crate::stealth::randomize::PRESET_CHROMIUM_MAJOR),
/// which a test holds it to.
const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Mobile Safari/537.36";

/// Chrome's port for its remote debugger when none is named.
pub const DEFAULT_REMOTE_DEBUG_PORT: u16 = 9222;

/// Collection of browser launch options that reduce automation fingerprints.
#[derive(Clone, Default)]
pub struct StealthOptions {
    pub headless: bool,
    pub window_size: Option<String>,
    /// `X,Y` of the window's top-left corner, for `--window-position`.
    pub window_position: Option<String>,
    pub user_agent: Option<String>,
    pub locale: Option<String>,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub mobile: bool,
    /// Whether the session blocks ad hosts. It adds no launch argument; see
    /// `BrowserSession::apply_ad_block`.
    pub ad_block: bool,
    pub uc: bool,
    /// Optional anti-detection fingerprint profile.
    pub fingerprint: Option<Fingerprint>,
    /// Headers supplied to the CDP network reactor when intercepting requests.
    pub extra_headers: HashMap<String, String>,
    /// Extra Chromium/Edge command-line arguments supplied by integrations.
    pub extra_args: Vec<String>,
    /// Optional explicit path to a patched or custom browser binary.
    pub binary_path: Option<PathBuf>,
    /// The launch settings of the configuration these options came from:
    /// preferences, page load strategy, mobile emulation, extensions and so on.
    pub launch: LaunchSettings,
}

// Written by hand: the proxy URL may carry a password and the extra headers
// may carry credentials, and neither should reach a log.
impl std::fmt::Debug for StealthOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut header_names: Vec<&String> = self.extra_headers.keys().collect();
        header_names.sort();
        f.debug_struct("StealthOptions")
            .field("headless", &self.headless)
            .field("window_size", &self.window_size)
            .field("window_position", &self.window_position)
            .field("user_agent", &self.user_agent)
            .field("locale", &self.locale)
            .field("proxy", &self.proxy.as_deref().map(without_credentials))
            .field(
                "proxy_pac_url",
                &self.proxy_pac_url.as_deref().map(without_credentials),
            )
            .field("user_data_dir", &self.user_data_dir)
            .field("extension_dir", &self.extension_dir)
            .field("mobile", &self.mobile)
            .field("ad_block", &self.ad_block)
            .field("uc", &self.uc)
            .field("fingerprint", &self.fingerprint)
            .field("extra_header_names", &header_names)
            .field("extra_args", &self.extra_args)
            .field("binary_path", &self.binary_path)
            .field("launch", &self.launch)
            .finish()
    }
}

/// `url` with any `user:password@` removed, for showing in a log.
fn without_credentials(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").map_or(("", url), |(s, r)| (s, r));
    let host_start = rest.rfind('@').map_or(0, |at| at + 1);
    let rest = &rest[host_start..];
    if scheme.is_empty() {
        rest.to_owned()
    } else {
        format!("{scheme}://{rest}")
    }
}

impl From<&BrowserConfig> for StealthOptions {
    fn from(config: &BrowserConfig) -> Self {
        Self {
            headless: config.headless,
            window_size: config.launch.window_size.map(|size| size.to_string()),
            window_position: config
                .launch
                .window_position
                .map(|position| position.to_string()),
            user_agent: config.user_agent.clone(),
            locale: config.locale.clone(),
            proxy: config.proxy.clone(),
            proxy_pac_url: config.proxy_pac_url.clone(),
            user_data_dir: config.user_data_dir.clone(),
            extension_dir: config.extension_dir.clone(),
            mobile: config.mobile,
            ad_block: config.ad_block,
            uc: config.is_uc_enabled(),
            fingerprint: config.fingerprint.clone(),
            extra_headers: HashMap::new(),
            extra_args: config.extra_args.clone(),
            binary_path: config.browser_binary_path.clone(),
            launch: config.launch.clone(),
        }
    }
}

impl StealthOptions {
    /// Applies the options to a Chromium-like capabilities object.
    pub fn apply_to<C: ChromiumLikeCapabilities>(
        &self,
        caps: &mut C,
    ) -> Result<(), SeleniumBaseError> {
        // Baseline stability flags. The sandbox is only turned off where it
        // commonly cannot start (containers and root shells on Linux), and the
        // GPU only where there is no screen to draw on, because a browser with
        // no GPU reports a software renderer that pages can tell apart.
        caps.add_arg("--disable-dev-shm-usage")?;
        if cfg!(target_os = "linux") {
            caps.add_arg("--no-sandbox")?;
        }
        if self.headless {
            caps.add_arg("--disable-gpu")?;
            caps.add_arg("--headless=new")?;
        }

        // Chrome takes the last of two arguments for one switch, and the
        // fingerprint's come after these, so a switch the fingerprint also sets
        // is left out here instead of being sent twice.
        let fingerprint_args = self
            .fingerprint
            .as_ref()
            .map(crate::stealth::evasions::launch_args)
            .unwrap_or_default();
        let set_by_fingerprint =
            |switch: &str| fingerprint_args.iter().any(|arg| arg.starts_with(switch));

        let size =
            self.window_size
                .as_deref()
                .unwrap_or(if self.mobile { "390,844" } else { "1280,720" });
        if !set_by_fingerprint("--window-size=") {
            caps.add_arg(&format!("--window-size={size}"))?;
        }
        if let Some(position) = self.window_position.as_deref() {
            caps.add_arg(&format!("--window-position={position}"))?;
        }

        // `ad_block` adds no argument: the hosts are refused over the DevTools
        // channel once the session is connected (`BrowserSession::apply_ad_block`).
        // It used to turn images off here, which blocked no ad.

        if let Some(locale) = self
            .locale
            .as_deref()
            .filter(|_| !set_by_fingerprint("--lang="))
        {
            caps.add_arg(&format!("--lang={locale}"))?;
        }

        let fingerprint_agent = set_by_fingerprint("--user-agent=");
        if let Some(user_agent) = self.user_agent.as_deref().filter(|_| !fingerprint_agent) {
            caps.add_arg(&format!("--user-agent={user_agent}"))?;
        }

        if let Some(proxy) = self
            .proxy
            .as_deref()
            .filter(|_| !set_by_fingerprint("--proxy-server="))
        {
            caps.add_arg(&format!("--proxy-server={proxy}"))?;
        }

        if let Some(hosts) = self.launch.proxy_bypass_list.to_chrome_value() {
            caps.add_arg(&format!("--proxy-bypass-list={hosts}"))?;
        }

        if let Some(pac_url) = self.proxy_pac_url.as_deref() {
            caps.add_arg(&format!("--proxy-pac-url={pac_url}"))?;
        }

        if let Some(user_data_dir) = self.user_data_dir.as_deref() {
            caps.add_arg(&format!("--user-data-dir={user_data_dir}"))?;
        }

        if let Some(extension_dir) = self.extension_dir.as_deref() {
            caps.add_arg(&format!("--load-extension={extension_dir}"))?;
        }

        // A phone user agent only where the caller named none.
        if self.mobile && self.user_agent.is_none() && !fingerprint_agent {
            caps.add_arg(&format!("--user-agent={MOBILE_USER_AGENT}"))?;
        }

        if let Some(binary) = self.binary_path.as_deref() {
            let binary_str = binary.display().to_string();
            caps.set_binary(&binary_str).map_err(|e| {
                SeleniumBaseError::invalid_config(format!(
                    "failed to set chrome binary to {binary_str}: {e}"
                ))
            })?;
        }

        if self.uc {
            apply_undetected_args(caps)?;
        }

        for arg in &fingerprint_args {
            caps.add_arg(arg)?;
        }

        self.apply_launch_settings(caps)?;

        for arg in &self.extra_args {
            if arg.starts_with('-') {
                caps.add_arg(arg)?;
            } else {
                caps.add_arg(&format!("--{arg}"))?;
            }
        }

        // Chrome reads only the last `--disable-features=`, and several of the
        // arguments above carry one.
        merge_feature_switch(caps, "disable-features")?;
        merge_feature_switch(caps, "enable-features")?;

        Ok(())
    }

    /// The part of [`apply_to`](Self::apply_to) that comes from
    /// [`LaunchSettings`]: preferences, page load strategy, mobile emulation,
    /// extensions, sync, the remote debugger and the DevTools event log.
    fn apply_launch_settings<C: ChromiumLikeCapabilities>(
        &self,
        caps: &mut C,
    ) -> Result<(), SeleniumBaseError> {
        let launch = &self.launch;

        if let Some(strategy) = launch.page_load_strategy {
            caps.set_page_load_strategy(strategy.into())?;
        }

        let preferences = chrome_preferences(&launch.switches);
        if !preferences.is_empty() {
            caps.add_experimental_option("prefs", preferences)?;
        }

        if let Some(metrics) = launch.device_metrics {
            let user_agent = self
                .user_agent
                .as_deref()
                .or(self.mobile.then_some(MOBILE_USER_AGENT));
            caps.add_experimental_option("mobileEmulation", metrics.mobile_emulation(user_agent))?;
        }

        // Chrome Sync is off unless it was asked for. The undetected-mode
        // arguments already carry the switch, so it is taken out for the one
        // case that wants sync.
        if launch.has(LaunchSwitch::EnableSync) {
            caps.remove_arg("--disable-sync")?;
        } else {
            caps.add_arg("--disable-sync")?;
        }

        if launch.has(LaunchSwitch::AutomationExtension) {
            caps.add_experimental_option("useAutomationExtension", true)?;
        }

        let mut loads_extensions = self.extension_dir.is_some();
        for archive in &launch.extension_archives {
            caps.add_encoded_extension(&archive.encoded()?)?;
            loads_extensions = true;
        }
        for (switch, bundled) in [
            (LaunchSwitch::DisableCsp, Bundled::DisableCsp),
            (LaunchSwitch::RecorderExtension, Bundled::Recorder),
        ] {
            if launch.has(switch) {
                caps.add_encoded_extension(&bundled.encoded())?;
                loads_extensions = true;
            }
        }
        if loads_extensions {
            // Chrome 137 and later ignore `--load-extension` unless this
            // feature is off.
            caps.add_arg("--disable-features=DisableLoadExtensionCommandLineSwitch")?;
        }

        if let Some(port) = launch.remote_debug_port {
            caps.add_arg(&format!("--remote-debugging-port={port}"))?;
        }

        if launch.cdp_events.captures(self.uc) {
            caps.set_logging_prefs("performance", LoggingPrefsLogLevel::All)?;
            caps.set_logging_prefs("browser", LoggingPrefsLogLevel::All)?;
        }

        Ok(())
    }

    /// Returns the full list of Chrome/Edge launch arguments that these options
    /// would produce.
    pub fn args(&self) -> Result<Vec<String>, SeleniumBaseError> {
        use thirtyfour::BrowserCapabilitiesHelper;
        use thirtyfour::DesiredCapabilities;
        let mut caps = DesiredCapabilities::chrome();
        self.apply_to(&mut caps)?;
        Ok(caps.args())
    }
}

/// Joins every `--<switch>=a,b` argument into the first one, keeping each
/// feature once.
///
/// Chrome keeps only the last occurrence of a switch, so two
/// `--disable-features=` arguments would silently drop the earlier list.
fn merge_feature_switch<C: ChromiumLikeCapabilities>(
    caps: &mut C,
    switch: &str,
) -> Result<(), SeleniumBaseError> {
    let prefix = format!("--{switch}=");
    let args = caps.args();
    if args.iter().filter(|arg| arg.starts_with(&prefix)).count() < 2 {
        return Ok(());
    }
    let mut features: Vec<&str> = Vec::new();
    for list in args.iter().filter_map(|arg| arg.strip_prefix(&prefix)) {
        for feature in list.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            if !features.contains(&feature) {
                features.push(feature);
            }
        }
    }
    let merged_arg = format!("{prefix}{}", features.join(","));
    let mut merged = Vec::with_capacity(args.len());
    let mut placed = false;
    for arg in &args {
        if arg.starts_with(&prefix) {
            if !placed {
                merged.push(merged_arg.clone());
                placed = true;
            }
        } else {
            merged.push(arg.clone());
        }
    }
    caps.set_browser_option("args", merged)?;
    Ok(())
}

/// The standard undetected-chrome launch arguments.
const UC_ARGS: [&str; 17] = [
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--disable-popup-blocking",
    "--no-first-run",
    "--disable-notifications",
    "--disable-background-networking",
    "--disable-client-side-phishing-detection",
    "--disable-default-apps",
    "--disable-prompt-on-repost",
    "--disable-sync",
    "--disable-translate",
    "--metrics-recording-only",
    "--no-default-browser-check",
    "--password-store=basic",
    "--use-mock-keychain",
    "--disable-search-engine-choice-screen",
    "--safebrowsing-disable-download-protection",
];

/// Adds the standard undetected-chrome launch arguments.
pub fn apply_undetected_args<C: ChromiumLikeCapabilities>(
    caps: &mut C,
) -> Result<(), SeleniumBaseError> {
    for arg in UC_ARGS {
        caps.add_arg(arg)?;
    }
    caps.add_exclude_switch("enable-automation")?;
    caps.add_experimental_option("useAutomationExtension", false)?;
    Ok(())
}

/// Returns the default list of undetected-chrome launch arguments.
pub fn default_uc_args() -> Vec<String> {
    UC_ARGS.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use thirtyfour::BrowserCapabilitiesHelper;
    use thirtyfour::DesiredCapabilities;

    #[test]
    fn the_mobile_user_agent_claims_the_preset_major_in_chromes_frozen_format() {
        let major = crate::stealth::randomize::PRESET_CHROMIUM_MAJOR;
        assert_eq!(
            MOBILE_USER_AGENT,
            format!(
                "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/{major}.0.0.0 Mobile Safari/537.36"
            )
        );
    }

    #[test]
    fn uc_args_contain_automation_switch() {
        let args = default_uc_args();
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
    }

    #[test]
    fn apply_to_adds_stealth_args() {
        let mut caps = DesiredCapabilities::chrome();
        let opts = StealthOptions {
            uc: true,
            window_size: Some("1920,1080".to_owned()),
            ..Default::default()
        };
        opts.apply_to(&mut caps).unwrap();
        let args = caps.args();
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
        assert!(args.contains(&"--window-size=1920,1080".to_owned()));
        assert!(!args.iter().any(|a| a.contains("enable-automation")));
    }

    #[test]
    fn ad_blocking_does_not_turn_images_off() {
        // The hosts are refused over the DevTools channel once the session is
        // connected (`BrowserSession::apply_ad_block`); a launch argument that
        // turned images off would also stop the images of every other site.
        // `--block-images` is the setting for that.
        let blocking = StealthOptions {
            ad_block: true,
            ..Default::default()
        };
        let args = blocking.args().unwrap();
        assert!(
            !args.iter().any(|a| a.contains("imagesEnabled")),
            "{args:?}"
        );
    }

    #[test]
    fn mobile_defaults_do_not_override_what_the_caller_set() {
        let own = StealthOptions {
            mobile: true,
            user_agent: Some("MyAgent/1.0".to_owned()),
            window_size: Some("500,900".to_owned()),
            ..Default::default()
        };
        let args = own.args().unwrap();
        assert_eq!(
            args.iter()
                .filter(|a| a.starts_with("--user-agent="))
                .count(),
            1,
            "{args:?}"
        );
        assert!(args.contains(&"--user-agent=MyAgent/1.0".to_owned()));
        assert_eq!(
            args.iter()
                .filter(|a| a.starts_with("--window-size="))
                .count(),
            1
        );
        assert!(args.contains(&"--window-size=500,900".to_owned()));

        let bare = StealthOptions {
            mobile: true,
            ..Default::default()
        };
        let args = bare.args().unwrap();
        assert!(args.contains(&"--window-size=390,844".to_owned()));
        assert!(args.iter().any(|a| a.contains("Android")));
    }

    #[test]
    fn a_switch_the_fingerprint_sets_is_sent_once_and_with_the_fingerprints_value() {
        let fingerprint = Fingerprint::randomized(crate::stealth::fingerprint::OsType::Windows, 4);
        let opts = StealthOptions {
            user_agent: Some("MyAgent/1.0".to_owned()),
            locale: Some("fr-FR".to_owned()),
            window_size: Some("800,600".to_owned()),
            fingerprint: Some(fingerprint.clone()),
            ..Default::default()
        };

        let args = opts.args().unwrap();

        for switch in ["--user-agent=", "--lang=", "--window-size="] {
            assert_eq!(
                args.iter().filter(|a| a.starts_with(switch)).count(),
                1,
                "{switch} is sent twice: {args:?}"
            );
        }
        // Chrome takes the last of two, and the fingerprint's came last.
        let user_agent = fingerprint.user_agent.as_deref().unwrap();
        assert!(args.contains(&format!("--user-agent={user_agent}")));
        let locale = fingerprint.locale.as_deref().unwrap();
        assert!(args.contains(&format!("--lang={locale}")));
    }

    #[test]
    fn what_the_fingerprint_leaves_alone_still_comes_from_the_options() {
        // An explicit `Natural` leaves the fingerprint's user agent out of the
        // arguments, so the option's is the one sent.
        let fingerprint = Fingerprint::builder()
            .user_agent("FromFingerprint/1.0")
            .with_flags(|f| {
                f.navigator_masking = crate::stealth::fingerprint::MaskingMode::Natural;
            })
            .build();
        let opts = StealthOptions {
            user_agent: Some("MyAgent/1.0".to_owned()),
            fingerprint: Some(fingerprint),
            ..Default::default()
        };

        let args = opts.args().unwrap();

        assert!(
            args.contains(&"--user-agent=MyAgent/1.0".to_owned()),
            "{args:?}"
        );
        assert!(
            !args.iter().any(|a| a.contains("FromFingerprint")),
            "{args:?}"
        );
    }

    #[test]
    fn gpu_stays_on_unless_headless() {
        let shown = StealthOptions::default().args().unwrap();
        assert!(!shown.contains(&"--disable-gpu".to_owned()));
        let headless = StealthOptions {
            headless: true,
            ..Default::default()
        };
        assert!(headless
            .args()
            .unwrap()
            .contains(&"--disable-gpu".to_owned()));
    }

    #[test]
    fn debug_output_hides_proxy_passwords_and_header_values() {
        let opts = StealthOptions {
            proxy: Some("http://user:hunter2@proxy.example:8080".to_owned()),
            extra_headers: [("Authorization".to_owned(), "Bearer s3cret".to_owned())].into(),
            ..Default::default()
        };
        let shown = format!("{opts:?}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("user:"),
            "{shown}"
        );
        assert!(!shown.contains("s3cret"), "{shown}");
        assert!(shown.contains("http://proxy.example:8080"), "{shown}");
        assert!(shown.contains("Authorization"), "{shown}");
    }

    #[test]
    fn credentials_are_cut_from_urls_for_logs() {
        assert_eq!(without_credentials("socks5://a:b@h:1"), "socks5://h:1");
        assert_eq!(without_credentials("h:1"), "h:1");
        assert_eq!(without_credentials("a:b@h:1"), "h:1");
        // A password that itself contains an `@`.
        assert_eq!(without_credentials("http://a:p@ss@h:1"), "http://h:1");
    }

    // -- Launch settings ---------------------------------------------------

    use crate::browser::config::DriverMode;
    use crate::browser::options::{
        CapabilityOverrides, CdpEventCapture, DeviceMetrics, ExtensionArchive, PageLoadStrategy,
        ProxyBypassList, WindowPosition, WindowSize,
    };
    use serde_json::{json, Value};

    fn options_for(config: &BrowserConfig) -> StealthOptions {
        StealthOptions::from(config)
    }

    /// The capabilities Chrome would be asked for, as JSON.
    fn chrome_caps(options: &StealthOptions) -> Value {
        let mut caps = DesiredCapabilities::chrome();
        options.apply_to(&mut caps).unwrap();
        serde_json::to_value(&caps).unwrap()
    }

    fn chrome_options(options: &StealthOptions) -> Value {
        chrome_caps(options)["goog:chromeOptions"].clone()
    }

    #[test]
    fn the_window_geometry_comes_from_the_launch_settings() {
        let config = BrowserConfig::default()
            .with_window_size(WindowSize::new(1920, 1080))
            .with_window_position(WindowPosition::new(-10, 20));
        let args = options_for(&config).args().unwrap();
        assert!(
            args.contains(&"--window-size=1920,1080".to_owned()),
            "{args:?}"
        );
        assert!(
            args.contains(&"--window-position=-10,20".to_owned()),
            "{args:?}"
        );
        assert_eq!(
            args.iter()
                .filter(|a| a.starts_with("--window-size="))
                .count(),
            1
        );

        let plain = options_for(&BrowserConfig::default()).args().unwrap();
        assert!(!plain.iter().any(|a| a.starts_with("--window-position=")));
        assert!(plain.contains(&"--window-size=1280,720".to_owned()));
    }

    #[test]
    fn the_proxy_bypass_list_is_one_semicolon_separated_flag() {
        let config = BrowserConfig::default()
            .with_proxy("proxy.example:8080")
            .with_proxy_bypass_list(ProxyBypassList::new(["*.example.com", "localhost"]).unwrap());
        let args = options_for(&config).args().unwrap();
        assert!(args.contains(&"--proxy-server=proxy.example:8080".to_owned()));
        assert!(
            args.contains(&"--proxy-bypass-list=*.example.com;localhost".to_owned()),
            "{args:?}"
        );

        let none = options_for(&BrowserConfig::default()).args().unwrap();
        assert!(!none.iter().any(|a| a.starts_with("--proxy-bypass-list")));
    }

    #[test]
    fn the_content_switches_become_chrome_preferences() {
        let config = BrowserConfig::default()
            .with_block_images(true)
            .with_disable_js(true)
            .with_disable_cookies(true)
            .with_do_not_track(true)
            .with_external_pdf(true);
        assert_eq!(
            chrome_options(&options_for(&config))["prefs"],
            json!({
                "profile.managed_default_content_settings.images": 2,
                "profile.managed_default_content_settings.javascript": 2,
                "profile.default_content_setting_values.cookies": 2,
                "enable_do_not_track": true,
                "plugins.always_open_pdf_externally": true,
            })
        );

        let off = BrowserConfig::default()
            .with_block_images(true)
            .with_block_images(false);
        assert!(chrome_options(&options_for(&off)).get("prefs").is_none());
    }

    #[test]
    fn the_page_load_strategy_is_a_capability() {
        for (strategy, wanted) in [
            (PageLoadStrategy::Normal, "normal"),
            (PageLoadStrategy::Eager, "eager"),
            (PageLoadStrategy::None, "none"),
        ] {
            let config = BrowserConfig::default().with_page_load_strategy(strategy);
            assert_eq!(
                chrome_caps(&options_for(&config))["pageLoadStrategy"],
                wanted
            );
        }
        let unset = chrome_caps(&options_for(&BrowserConfig::default()));
        assert!(unset.get("pageLoadStrategy").is_none());
    }

    #[test]
    fn device_metrics_turn_on_chromes_mobile_emulation() {
        let config =
            BrowserConfig::default().with_device_metrics(DeviceMetrics::new(412, 732, 3.0));
        let emulation = chrome_options(&options_for(&config))["mobileEmulation"].clone();
        assert_eq!(
            emulation["deviceMetrics"],
            json!({"width": 412, "height": 732, "pixelRatio": 3.0})
        );
        assert!(emulation["userAgent"].as_str().unwrap().contains("Android"));

        let own = config.clone().with_user_agent("MyAgent/1.0");
        let emulation = chrome_options(&options_for(&own))["mobileEmulation"].clone();
        assert_eq!(emulation["userAgent"], "MyAgent/1.0");

        // Without metrics there is no emulation, only the mobile defaults.
        let mobile = BrowserConfig::default().with_mobile(true);
        assert!(chrome_options(&options_for(&mobile))
            .get("mobileEmulation")
            .is_none());
    }

    #[test]
    fn chrome_sync_is_off_unless_it_was_asked_for() {
        let default = options_for(&BrowserConfig::default()).args().unwrap();
        assert!(
            default.contains(&"--disable-sync".to_owned()),
            "{default:?}"
        );

        let enabled = BrowserConfig::default().with_enable_sync(true);
        assert!(!options_for(&enabled)
            .args()
            .unwrap()
            .contains(&"--disable-sync".to_owned()));

        // The undetected-mode arguments carry the switch too.
        let uc = BrowserConfig::default().with_mode(DriverMode::Uc);
        assert!(options_for(&uc)
            .args()
            .unwrap()
            .contains(&"--disable-sync".to_owned()));
        let uc_synced = uc.with_enable_sync(true);
        assert!(!options_for(&uc_synced)
            .args()
            .unwrap()
            .contains(&"--disable-sync".to_owned()));
    }

    #[test]
    fn the_automation_extension_is_kept_only_when_asked_for() {
        let uc = BrowserConfig::default().with_mode(DriverMode::Uc);
        assert_eq!(
            chrome_options(&options_for(&uc))["useAutomationExtension"],
            false
        );
        let kept = uc.with_automation_extension(true);
        assert_eq!(
            chrome_options(&options_for(&kept))["useAutomationExtension"],
            true
        );
        let plain = chrome_options(&options_for(&BrowserConfig::default()));
        assert!(plain.get("useAutomationExtension").is_none());
    }

    #[test]
    fn extension_files_and_bundled_extensions_reach_the_extensions_capability() {
        use base64::Engine;
        let dir = tempfile::tempdir().unwrap();
        let crx = dir.path().join("mine.crx");
        std::fs::write(&crx, b"crx bytes").unwrap();

        let config = BrowserConfig::default()
            .with_extension_archive(ExtensionArchive::new(&crx).unwrap())
            .with_extension_archive(ExtensionArchive::new(&crx).unwrap());
        let options = chrome_options(&options_for(&config));
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"crx bytes");
        assert_eq!(
            options["extensions"],
            json!([encoded]),
            "added twice, loaded once"
        );
        assert!(options["args"].as_array().unwrap().contains(&json!(
            "--disable-features=DisableLoadExtensionCommandLineSwitch"
        )));

        let bundled = BrowserConfig::default()
            .with_disable_csp(true)
            .with_recorder_extension(true);
        let extensions = chrome_options(&options_for(&bundled))["extensions"].clone();
        assert_eq!(extensions.as_array().unwrap().len(), 2);

        let none = chrome_options(&options_for(&BrowserConfig::default()));
        assert!(none.get("extensions").is_none());
        assert!(!none["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a.as_str().unwrap().contains("DisableLoadExtension")));
    }

    #[test]
    fn a_missing_extension_file_fails_the_session_before_it_starts() {
        let config = BrowserConfig::default()
            .with_extension_archive(ExtensionArchive::new("/no/such/extension.zip").unwrap());
        let error = options_for(&config).args().unwrap_err();
        assert!(
            error.to_string().contains("/no/such/extension.zip"),
            "{error}"
        );
    }

    #[test]
    fn disable_features_lists_are_merged_not_overridden() {
        let config = BrowserConfig::default()
            .with_disable_csp(true)
            .with_extra_args(["disable-features=Foo", "--disable-features=Bar,Foo"]);
        let args = options_for(&config).args().unwrap();
        let lists: Vec<_> = args
            .iter()
            .filter(|a| a.starts_with("--disable-features="))
            .collect();
        assert_eq!(
            lists,
            ["--disable-features=DisableLoadExtensionCommandLineSwitch,Foo,Bar"],
            "{args:?}"
        );
    }

    #[test]
    fn the_remote_debugger_port_is_a_chrome_flag() {
        let on = BrowserConfig::default().with_remote_debug(true);
        assert!(options_for(&on)
            .args()
            .unwrap()
            .contains(&"--remote-debugging-port=9222".to_owned()));
        let custom = BrowserConfig::default().with_remote_debug_port(9333);
        assert!(options_for(&custom)
            .args()
            .unwrap()
            .contains(&"--remote-debugging-port=9333".to_owned()));
        let off = on.with_remote_debug(false);
        assert!(!options_for(&off)
            .args()
            .unwrap()
            .iter()
            .any(|a| a.starts_with("--remote-debugging-port")));
    }

    #[test]
    fn the_cdp_event_log_is_the_performance_and_browser_logging_capability() {
        let logged = BrowserConfig::default().with_cdp_events(CdpEventCapture::Always);
        assert_eq!(
            chrome_caps(&options_for(&logged))["goog:loggingPrefs"],
            json!({"performance": "ALL", "browser": "ALL"})
        );

        // UC-only capture leaves a plain session alone, and logs a UC one.
        let uc_only = BrowserConfig::default().with_cdp_events(CdpEventCapture::UndetectedOnly);
        assert!(chrome_caps(&options_for(&uc_only))
            .get("goog:loggingPrefs")
            .is_none());
        let uc = uc_only.with_mode(DriverMode::Uc);
        assert!(chrome_caps(&options_for(&uc))
            .get("goog:loggingPrefs")
            .is_some());

        let off = chrome_caps(&options_for(&BrowserConfig::default()));
        assert!(off.get("goog:loggingPrefs").is_none());
    }

    #[test]
    fn capability_overrides_do_not_change_the_chrome_arguments() {
        // They are merged into the capabilities when the session is built.
        let caps = CapabilityOverrides::from_text(r#"{"name": "x"}"#).unwrap();
        let config = BrowserConfig::default().with_capabilities(caps);
        assert_eq!(
            options_for(&config).args().unwrap(),
            options_for(&BrowserConfig::default()).args().unwrap()
        );
    }
}
