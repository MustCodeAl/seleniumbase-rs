//! How the options of [`Settings`] become a browser start-up: the launch options
//! that [`Settings::to_browser_config`] adds for the WebDriver engine, and
//! [`Settings::to_launch_options`] for the pure CDP engine.
//!
//! The browser settings are read by the typed builders of
//! [`BrowserConfig`](crate::BrowserConfig) (`with_dark_mode`, `with_window_size`,
//! `with_chromium_arg`, ...), so a value that does not parse is refused in one
//! place, [`Settings::validate`], and not at launch.

use std::path::PathBuf;
use std::time::Duration;

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::options::{
    switch_args, CapabilityOverrides, CdpEventCapture, DeviceMetrics, DriverVersion,
    ExtensionArchive, LaunchSwitch, LocaleCode, PageLoadStrategy, ProxyBypassList, VirtualDisplay,
    WindowPosition, WindowSize,
};
use crate::config::settings::Settings;
use crate::config::test_settings::TestData;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::LaunchOptions;

/// The port Selenium Grid and chromedriver listen on in Python SeleniumBase.
const DEFAULT_PORT: u16 = 4444;

impl Settings {
    /// Checks the values that [`with_launch_options`](Self::with_launch_options) reads: the
    /// window size and position, the phone screen, the page load strategy, the driver
    /// version, the bypass list, the capabilities, the extensions, the Firefox options, the
    /// virtual display, the language code and the seconds.
    pub(super) fn check_launch_values(&self) -> Result<(), SeleniumBaseError> {
        self.with_launch_options(BrowserConfig::default())
            .map(|_| ())
    }

    /// `config` with the launch options of these settings on top, in the order the builders
    /// of `BrowserConfig` take them.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] for a value that does not parse (the window size, the
    /// phone screen, ...) and for a capabilities or extension file that cannot be read.
    pub(super) fn with_launch_options(
        &self,
        mut config: BrowserConfig,
    ) -> Result<BrowserConfig, SeleniumBaseError> {
        for (on, switch) in [
            (self.block_images, LaunchSwitch::BlockImages),
            (self.disable_js, LaunchSwitch::DisableJavaScript),
            (self.disable_cookies, LaunchSwitch::DisableCookies),
            (self.do_not_track, LaunchSwitch::DoNotTrack),
            (self.external_pdf, LaunchSwitch::ExternalPdf),
            (self.enable_sync, LaunchSwitch::EnableSync),
            (self.disable_csp, LaunchSwitch::DisableCsp),
            (self.use_auto_ext, LaunchSwitch::AutomationExtension),
            (self.proxy_driver, LaunchSwitch::ProxyForDriverDownload),
            (self.dark_mode, LaunchSwitch::DarkMode),
            (self.incognito, LaunchSwitch::Incognito),
            (self.guest_mode, LaunchSwitch::Guest),
            (self.devtools, LaunchSwitch::Devtools),
            (self.disable_ws, LaunchSwitch::DisableWebSecurity),
            (self.swiftshader, LaunchSwitch::Swiftshader),
            (self.enable_3d_apis, LaunchSwitch::Enable3dApis),
            (self.maximize, LaunchSwitch::Maximize),
            (self.no_sandbox, LaunchSwitch::NoSandbox),
            (self.disable_gpu, LaunchSwitch::DisableGpu),
        ] {
            if on {
                config = config.with_switch(switch, true);
            }
        }
        if self.headless1 {
            config = config.with_headless1(true);
        }
        if let Some(list) = &self.chromium_arg {
            config = with_chromium_items(config, list.items());
        }
        if let Some(text) = &self.disable_features {
            config = config.with_disable_features(list(text));
        }
        if let Some(rules) = &self.host_resolver_rules {
            config = config.with_host_resolver_rules(rules.as_str());
        }
        if self.webdriver_url.is_none()
            && (self.server.is_some() || self.port.is_some() || self.protocol.is_some())
        {
            config = config.with_webdriver_url(remote_address(
                self.server.as_deref(),
                self.port,
                self.protocol.as_deref(),
            ));
        }
        if let Some(text) = &self.locale_code {
            config = config.with_locale(text.parse::<LocaleCode>()?.as_str());
        }
        if let Some(text) = &self.window_size {
            config = config.with_window_size(text.parse::<WindowSize>()?);
        }
        if let Some(text) = &self.window_position {
            config = config.with_window_position(text.parse::<WindowPosition>()?);
        }
        if let Some(text) = &self.device_metrics {
            config = config.with_device_metrics(text.parse::<DeviceMetrics>()?);
        }
        if let Some(text) = &self.page_load_strategy {
            config = config.with_page_load_strategy(text.parse::<PageLoadStrategy>()?);
        }
        if let Some(text) = &self.driver_version {
            config = config.with_driver_version(text.parse::<DriverVersion>()?);
        }
        if let Some(text) = &self.proxy_bypass_list {
            config = config.with_proxy_bypass_list(text.parse::<ProxyBypassList>()?);
        }
        if self.cap_file.is_some() || self.cap_string.is_some() {
            let mut capabilities = match &self.cap_file {
                Some(path) => CapabilityOverrides::from_file(path)?,
                None => CapabilityOverrides::default(),
            };
            if let Some(text) = &self.cap_string {
                capabilities.merge(&CapabilityOverrides::from_text(text)?);
            }
            config = config.with_capabilities(capabilities);
        }
        if let Some(text) = &self.extension_zip {
            for archive in ExtensionArchive::parse_list(text)? {
                config = config.with_extension_archive(archive);
            }
        }
        if self.firefox_arg.is_some() || self.firefox_pref.is_some() {
            let mut firefox = config.launch.firefox.clone();
            if let Some(text) = &self.firefox_arg {
                firefox.add_args(text)?;
            }
            if let Some(text) = &self.firefox_pref {
                firefox.add_preferences(text)?;
            }
            config = config.with_firefox_options(firefox);
        }
        let display = self
            .xvfb_metrics
            .as_deref()
            .map(str::parse::<VirtualDisplay>)
            .transpose()?;
        if self.xvfb {
            config = match display {
                Some(display) => config.with_virtual_display(display),
                None => config.with_xvfb(true),
            };
        }
        if self.remote_debug {
            config = config.with_remote_debug(true);
        }
        if self.log_cdp_events {
            config = config.with_cdp_events(CdpEventCapture::Always);
        } else if self.uc_cdp_events {
            config = config.with_cdp_events(CdpEventCapture::UndetectedOnly);
        }

        let mut run = config.run_settings().clone();
        if self.demo {
            run = run.with_demo_mode(true);
        }
        if self.slow {
            run = run.with_slow_mode(true);
        }
        if let Some(seconds) = self.demo_sleep {
            run = run.with_demo_sleep(duration("demo_sleep", seconds)?);
        }
        if let Some(seconds) = self.message_duration {
            run = run.with_message_duration(duration("message_duration", seconds)?);
        }
        if self.skip_js_waits {
            run = run.with_skip_js_waits(true);
        }
        if let Some(loops) = self.highlights {
            run = run.with_highlights(loops);
        }
        if let Some(seconds) = self.interval {
            run = run.with_interval(duration("interval", seconds)?);
        }
        if let Some(seconds) = self.verify_delay {
            run = run.with_verify_delay(duration("verify_delay", seconds)?);
        }
        if self.check_js {
            run = run.with_check_js(true);
        }
        if self.wait_for_angularjs {
            run = run.with_wait_for_angularjs(true);
        }
        if self.disable_beforeunload {
            run = run.with_disable_beforeunload(true);
        }
        if let Some(multiplier) = self.timeout_multiplier {
            run = run.with_timeout_multiplier(multiplier);
        }

        if self.test.rec_sleep {
            run = run.with_record_sleep(true);
        }
        if self.visual_baseline {
            run = run.with_visual_baseline_reset(true);
        }
        config = config.with_run_settings(run);
        // A file can hold any string for `env`; the variable is checked as it is read.
        let mut data = self.test.data.clone();
        if let Some(env) = &data.env {
            data.env = Some(TestData::parse_env(env)?);
        }
        config.launch.test_data = data;
        Ok(config)
    }

    /// Turns on what only the whole process can switch: `crumbs` keeps no cookies of a reused
    /// browser, as `browser::reuse::set_crumbs(true)` does.
    pub(super) fn apply_process_wide(&self) {
        if self.crumbs {
            crate::browser::reuse::set_crumbs(true);
        }
    }

    /// These settings as the launch options of the pure CDP engine
    /// ([`Browser`](crate::sb_cdp::Browser) in `sb_cdp`).
    ///
    /// The browser is Chrome, Chromium (`browser = "chromium"`) or the binary `chrome_bin`
    /// names; another browser needs `chrome_bin`. `headless` is set when `headless` or
    /// `headless1` is on, and left to the engine otherwise. The Chromium switches are the
    /// ones [`to_browser_config`](Self::to_browser_config) gives a WebDriver session
    /// (`dark_mode`, `disable_ws`, `swiftshader`, `host_resolver_rules`, ...). Settings that
    /// only a WebDriver server reads (`server`, `driver_version`, `firefox_arg`, ...) are
    /// ignored.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a browser the engine cannot start, for
    /// `incognito` together with `guest_mode`, and for a value that does not parse; and the
    /// error of a fingerprint file that cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Settings;
    ///
    /// let settings = Settings {
    ///     headless: true,
    ///     dark_mode: true,
    ///     user_agent: Some("tests/1.0".to_owned()),
    ///     ..Settings::default()
    /// };
    /// let options = settings.to_launch_options()?;
    /// assert!(options.is_headless());
    /// assert_eq!(options.user_agent(), Some("tests/1.0"));
    /// let args = options.browser_args(std::path::Path::new("/tmp/profile"));
    /// assert!(args.contains(&"--enable-features=WebContentsForceDark".to_owned()));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn to_launch_options(&self) -> Result<LaunchOptions, SeleniumBaseError> {
        let browser = self.browser.parse::<Browser>()?;
        let config = self.with_launch_options(BrowserConfig::default())?;
        let headless = self.headless || self.headless1;
        let loads_extensions =
            self.extension_dir.is_some() || !config.launch.extension_archives.is_empty();

        let mut builder = LaunchOptions::builder();
        builder = match (&self.chrome_bin, browser) {
            (Some(path), _) => builder.executable_path(path.clone()),
            (None, Browser::Chrome) => builder,
            (None, Browser::Chromium) => builder.use_chromium(true),
            (None, other) => {
                return Err(SeleniumBaseError::InvalidConfig(format!(
                    "the pure CDP engine starts Chrome or Chromium; set chrome_bin to run {other}"
                )))
            }
        };
        if headless {
            builder = builder.headless(true);
        }
        if let Some(page) = &self.start_page {
            builder = builder.url(page.as_str());
        }
        builder = builder
            .incognito(self.incognito)
            .guest(self.guest_mode)
            .ad_block(self.ad_block);
        if self.no_sandbox {
            builder = builder.no_sandbox(true);
        }
        if let Some(proxy) = &self.proxy {
            builder = builder.proxy(proxy.as_str());
        }
        if let Some(dir) = &self.user_data_dir {
            builder = builder.user_data_dir(dir.as_str());
        }
        if let Some(user_agent) = &self.user_agent {
            builder = builder.user_agent(user_agent.as_str());
        }
        if let Some(lang) = config.locale.as_deref().or(self.locale.as_deref()) {
            builder = builder.lang(lang);
        }
        let size = config.launch.window_size.or_else(|| {
            self.window_width
                .zip(self.window_height)
                .map(|(width, height)| WindowSize::new(width, height))
        });
        if let Some(size) = size {
            builder = builder.window_size(size.width, size.height);
        }
        if let Some(position) = config.launch.window_position {
            builder = builder.window_position(position.x, position.y);
        }
        if let Some(dirs) = &self.extension_dir {
            for dir in list(dirs) {
                builder = builder.extension(PathBuf::from(dir));
            }
        }
        if let Some(port) = config.launch.remote_debug_port {
            builder = builder.debugging_port(port);
        }
        for arg in config.extra_args.iter().cloned().chain(
            switch_args(&config.launch, headless, loads_extensions)
                .into_iter()
                // The builder has its own setters for these.
                .filter(|arg| !matches!(arg.as_str(), "--incognito" | "--guest" | "--no-sandbox")),
        ) {
            builder = builder.arg(arg);
        }
        if let Some(fingerprint) = self.load_fingerprint()? {
            builder = builder.fingerprint(&fingerprint);
        }
        #[cfg(feature = "plugins")]
        {
            builder = builder.plugin_selection(&self.plugins);
        }
        builder.build()
    }
}

/// Adds the items of a `chromium_arg` list to `config`, each item one switch, written as
/// [`BrowserConfig::with_chromium_arg`] reads one item (a `disable-features=` item joins the
/// features to disable). Unlike that method it does not split an item at commas, which is
/// what the array form of the settings file is for.
fn with_chromium_items(mut config: BrowserConfig, items: &[String]) -> BrowserConfig {
    let mut features = Vec::new();
    for item in items.iter().map(|item| item.trim()) {
        if item.len() < 3 {
            continue;
        }
        let switch = item.trim_start_matches('-');
        if let Some(feature) = switch.strip_prefix("disable-features=") {
            let feature = feature.trim();
            if !feature.is_empty() {
                features.push(feature.to_owned());
            }
        } else if !switch.is_empty() {
            config.extra_args.push(format!("--{switch}"));
        }
    }
    config.with_disable_features(features)
}

/// The items of a comma-separated list, trimmed, without the empty ones.
fn list(text: &str) -> impl Iterator<Item = &str> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

/// `seconds` as a `Duration`, or an error naming the setting.
fn duration(name: &str, seconds: f64) -> Result<Duration, SeleniumBaseError> {
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        SeleniumBaseError::InvalidConfig(format!(
            "{name}: expected a number of seconds, 0 or more, not {seconds}"
        ))
    })
}

/// The address of a Selenium Grid, built the way Python SeleniumBase builds it.
///
/// With a `server`, the address is `PROTOCOL://SERVER:PORT/wd/hub`: a scheme in `server` wins
/// over `protocol` (`https` only), a port in `server` wins over `port`, and `/wd/hub` is added
/// unless it is there. Without one, the address is the local `PROTOCOL://localhost:PORT`.
/// The protocol is `http` and the port 4444 unless given.
pub(super) fn remote_address(
    server: Option<&str>,
    port: Option<u16>,
    protocol: Option<&str>,
) -> String {
    let mut protocol = protocol.unwrap_or("http");
    let port = port.unwrap_or(DEFAULT_PORT);
    let Some(mut server) = server else {
        return format!("{protocol}://localhost:{port}");
    };
    if let Some(rest) = server.strip_prefix("https://") {
        protocol = "https";
        server = rest;
    } else if let Some((_, rest)) = server.split_once("://") {
        server = rest;
    }
    let with_port = if server.contains(':') {
        server.to_owned()
    } else if let Some(slash) = server.find('/') {
        format!("{}:{port}{}", &server[..slash], &server[slash..])
    } else {
        format!("{server}:{port}")
    };
    let mut address = format!("{protocol}://{with_port}");
    if !address.ends_with("/wd/hub") {
        if !address.ends_with('/') {
            address.push('/');
        }
        address.push_str("wd/hub");
    }
    address
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::run_settings::RunSettings;

    /// Changes one setting of a `Settings`.
    type Setter = fn(&mut Settings);

    fn args_of(settings: &Settings) -> Vec<String> {
        settings
            .to_launch_options()
            .unwrap()
            .browser_args(std::path::Path::new("/tmp/profile"))
    }

    #[test]
    fn the_defaults_add_nothing_to_the_browser_config() {
        let settings = Settings::default();
        let config = settings.to_browser_config();
        assert!(config.launch.switches.is_empty());
        assert!(config.extra_args.is_empty());
        assert_eq!(config.run_settings(), &RunSettings::default());
        assert_eq!(config.launch.test_data, settings.test.data);
        assert_eq!(config.webdriver_url, "http://localhost:4444");
    }

    #[test]
    fn each_switch_of_the_settings_turns_its_launch_switch_on() {
        let cases: [(Setter, LaunchSwitch); 19] = [
            (|s| s.block_images = true, LaunchSwitch::BlockImages),
            (|s| s.disable_js = true, LaunchSwitch::DisableJavaScript),
            (|s| s.disable_cookies = true, LaunchSwitch::DisableCookies),
            (|s| s.do_not_track = true, LaunchSwitch::DoNotTrack),
            (|s| s.external_pdf = true, LaunchSwitch::ExternalPdf),
            (|s| s.enable_sync = true, LaunchSwitch::EnableSync),
            (|s| s.disable_csp = true, LaunchSwitch::DisableCsp),
            (|s| s.use_auto_ext = true, LaunchSwitch::AutomationExtension),
            (
                |s| s.proxy_driver = true,
                LaunchSwitch::ProxyForDriverDownload,
            ),
            (|s| s.dark_mode = true, LaunchSwitch::DarkMode),
            (|s| s.incognito = true, LaunchSwitch::Incognito),
            (|s| s.guest_mode = true, LaunchSwitch::Guest),
            (|s| s.devtools = true, LaunchSwitch::Devtools),
            (|s| s.disable_ws = true, LaunchSwitch::DisableWebSecurity),
            (|s| s.swiftshader = true, LaunchSwitch::Swiftshader),
            (|s| s.enable_3d_apis = true, LaunchSwitch::Enable3dApis),
            (|s| s.maximize = true, LaunchSwitch::Maximize),
            (|s| s.no_sandbox = true, LaunchSwitch::NoSandbox),
            (|s| s.disable_gpu = true, LaunchSwitch::DisableGpu),
        ];
        for (set, switch) in cases {
            let mut settings = Settings::default();
            set(&mut settings);
            let config = settings.to_browser_config();
            assert!(config.launch.has(switch), "{switch:?}");
            assert_eq!(config.launch.switches.len(), 1, "{switch:?}");
        }
    }

    #[test]
    fn headless1_makes_the_session_headless_too() {
        let settings = Settings {
            headless1: true,
            ..Settings::default()
        };
        let config = settings.to_browser_config();
        assert!(config.headless);
        assert!(config.launch.has(LaunchSwitch::Headless1));
    }

    #[test]
    fn chromium_text_options_reach_the_arguments() {
        let settings = Settings {
            chromium_arg: Some("--mute-audio, lang=fr, disable-features=Foo".into()),
            disable_features: Some("Bar, Baz,".to_owned()),
            host_resolver_rules: Some("MAP example.test 127.0.0.1".to_owned()),
            ..Settings::default()
        };
        let config = settings.to_browser_config();
        assert_eq!(config.extra_args, ["--mute-audio", "--lang=fr"]);
        assert_eq!(config.launch.disable_features, ["Foo", "Bar", "Baz"]);
        assert_eq!(
            config.launch.host_resolver_rules.as_deref(),
            Some("MAP example.test 127.0.0.1")
        );
    }

    #[test]
    fn the_grid_address_is_built_as_python_builds_it() {
        assert_eq!(remote_address(None, None, None), "http://localhost:4444");
        assert_eq!(
            remote_address(None, Some(9515), Some("https")),
            "https://localhost:9515"
        );
        assert_eq!(
            remote_address(Some("grid.test"), None, None),
            "http://grid.test:4444/wd/hub"
        );
        assert_eq!(
            remote_address(Some("grid.test"), Some(5555), Some("https")),
            "https://grid.test:5555/wd/hub"
        );
        assert_eq!(
            remote_address(Some("https://grid.test"), None, Some("http")),
            "https://grid.test:4444/wd/hub"
        );
        assert_eq!(
            remote_address(Some("http://grid.test:4445/wd/hub"), Some(1), None),
            "http://grid.test:4445/wd/hub"
        );
        assert_eq!(
            remote_address(Some("grid.test/base/"), Some(80), None),
            "http://grid.test:80/base/wd/hub"
        );
    }

    #[test]
    fn server_port_and_protocol_fill_the_webdriver_url_unless_it_is_given() {
        let settings = Settings {
            server: Some("grid.test".to_owned()),
            port: Some(4445),
            ..Settings::default()
        };
        assert_eq!(
            settings.to_browser_config().webdriver_url,
            "http://grid.test:4445/wd/hub"
        );
        let settings = Settings {
            webdriver_url: Some("http://other.test:1".to_owned()),
            ..settings
        };
        assert_eq!(
            settings.to_browser_config().webdriver_url,
            "http://other.test:1"
        );
    }

    #[test]
    fn typed_text_options_become_typed_launch_settings() {
        let settings = Settings {
            window_width: Some(800),
            window_height: Some(600),
            window_size: Some("1280,720".to_owned()),
            window_position: Some("10,20".to_owned()),
            device_metrics: Some("412,732,3".to_owned()),
            page_load_strategy: Some("eager".to_owned()),
            driver_version: Some("126".to_owned()),
            proxy_bypass_list: Some("*.example.test;localhost".to_owned()),
            locale: Some("de".to_owned()),
            locale_code: Some("ar_eg".to_owned()),
            xvfb: true,
            xvfb_metrics: Some("1920,1080".to_owned()),
            remote_debug: true,
            ..Settings::default()
        };
        let config = settings.to_browser_config();
        let launch = &config.launch;
        assert_eq!(launch.window_size, Some(WindowSize::new(1280, 720)));
        assert_eq!(launch.window_position, Some(WindowPosition::new(10, 20)));
        assert_eq!(
            launch.device_metrics,
            Some(DeviceMetrics::new(412, 732, 3.0))
        );
        assert!(config.mobile);
        assert_eq!(launch.page_load_strategy, Some(PageLoadStrategy::Eager));
        assert!(launch.driver_version.is_some());
        assert!(!launch.proxy_bypass_list.is_empty());
        assert_eq!(config.locale.as_deref(), Some("ar-EG"));
        assert_eq!(
            launch.virtual_display,
            Some(VirtualDisplay::new(1920, 1080))
        );
        assert!(launch.remote_debug_port.is_some());
    }

    #[test]
    fn a_value_that_does_not_parse_is_refused_when_the_settings_are_checked() {
        for set in [
            (|s: &mut Settings| s.window_size = Some("wide".to_owned())) as Setter,
            |s| s.window_position = Some("here".to_owned()),
            |s| s.device_metrics = Some("412".to_owned()),
            |s| s.page_load_strategy = Some("slow".to_owned()),
            |s| s.locale_code = Some("english".to_owned()),
            |s| s.xvfb_metrics = Some("big".to_owned()),
            |s| s.cap_string = Some("[1]".to_owned()),
            |s| s.cap_file = Some(PathBuf::from("/no/such/caps.json")),
            |s| s.demo_sleep = Some(-1.0),
        ] {
            let mut settings = Settings::default();
            set(&mut settings);
            assert!(settings.validate().is_err(), "{settings:?}");
        }
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn capabilities_come_from_a_string_over_a_file() {
        let dir = std::env::temp_dir().join(format!("sb-caps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("caps.json");
        std::fs::write(&file, r#"{"name": "from-file", "extra": 1}"#).unwrap();
        let settings = Settings {
            cap_file: Some(file),
            cap_string: Some(r#"{"name": "from-text"}"#.to_owned()),
            ..Settings::default()
        };
        let config = settings.to_browser_config();
        let capabilities = &config.launch.capabilities;
        assert_eq!(
            capabilities.get("name"),
            Some(&serde_json::json!("from-text"))
        );
        assert_eq!(capabilities.get("extra"), Some(&serde_json::json!(1)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_settings_come_from_the_demo_slow_and_timeout_options() {
        let settings = Settings {
            demo: true,
            slow: true,
            skip_js_waits: true,
            visual_baseline: true,
            demo_sleep: Some(0.5),
            message_duration: Some(3.0),
            timeout_seconds: Some(4),
            test: crate::config::test_settings::TestSettings {
                rec_sleep: true,
                ..Default::default()
            },
            ..Settings::default()
        };
        let expected = RunSettings::default()
            .with_default_timeout(Duration::from_secs(4))
            .with_demo_mode(true)
            .with_slow_mode(true)
            .with_demo_sleep(Duration::from_millis(500))
            .with_message_duration(Duration::from_secs(3))
            .with_skip_js_waits(true)
            .with_record_sleep(true)
            .with_visual_baseline_reset(true);
        assert_eq!(settings.to_browser_config().run_settings(), &expected);
    }

    #[test]
    fn the_run_settings_of_the_fixture_come_from_the_settings() {
        let settings = Settings {
            highlights: Some(3),
            interval: Some(1.5),
            verify_delay: Some(0.0),
            check_js: true,
            wait_for_angularjs: true,
            disable_beforeunload: true,
            timeout_multiplier: Some(2.0),
            timeout_seconds: Some(10),
            ..Settings::default()
        };
        let expected = RunSettings::default()
            .with_default_timeout(Duration::from_secs(10))
            .with_highlights(3)
            .with_interval(Duration::from_millis(1500))
            .with_verify_delay(Duration::ZERO)
            .with_check_js(true)
            .with_wait_for_angularjs(true)
            .with_disable_beforeunload(true)
            .with_timeout_multiplier(2.0);
        let config = settings.to_browser_config();
        assert_eq!(config.run_settings(), &expected);
        assert_eq!(config.run_settings().scale_timeout(10), 20);
    }

    #[test]
    fn a_multiplier_below_the_minimum_is_raised_and_a_negative_pause_is_refused() {
        let slow = Settings {
            timeout_multiplier: Some(0.1),
            ..Settings::default()
        };
        let config = slow.to_browser_config();
        assert_eq!(config.run_settings().timeout_multiplier(), Some(0.5));
        let wrong = Settings {
            interval: Some(-1.0),
            ..Settings::default()
        };
        assert!(wrong.validate().is_err());
    }

    #[test]
    fn an_array_item_of_chromium_arg_keeps_its_commas() {
        let rules = "--host-resolver-rules=MAP a 1.2.3.4,EXCLUDE b";
        let settings = Settings {
            chromium_arg: Some(vec![rules.to_owned(), "disable-features=Foo".to_owned()].into()),
            ..Settings::default()
        };
        let args = args_of(&settings);
        assert!(args.iter().any(|arg| arg == rules), "{args:?}");
        assert!(
            args.iter()
                .any(|arg| arg == "--disable-features=Translate,Foo"),
            "{args:?}"
        );
    }

    #[test]

    fn test_data_reaches_the_launch_settings() {
        let mut settings = Settings::default();
        settings.test.data.env = Some("qa".to_owned());
        settings.test.data.var1 = Some("x".to_owned());
        let config = settings.to_browser_config();
        assert_eq!(config.launch.test_data.env(), "qa");
        assert_eq!(config.launch.test_data.var1.as_deref(), Some("x"));
    }

    #[test]
    fn the_cdp_options_carry_the_chromium_settings() {
        let settings = Settings {
            headless: true,
            dark_mode: true,
            swiftshader: true,
            disable_ws: true,
            host_resolver_rules: Some("MAP a.test 127.0.0.1".to_owned()),
            disable_features: Some("Foo".to_owned()),
            chromium_arg: Some("--mute-audio".into()),
            start_page: Some("https://example.test/".to_owned()),
            proxy: Some("proxy.test:8080".to_owned()),
            user_agent: Some("tests/1.0".to_owned()),
            locale_code: Some("fr_FR".to_owned()),
            window_size: Some("1000,700".to_owned()),
            window_position: Some("5,6".to_owned()),
            no_sandbox: true,
            ..Settings::default()
        };
        let options = settings.to_launch_options().unwrap();
        assert!(options.is_headless());
        assert_eq!(options.url(), Some("https://example.test/"));
        assert_eq!(options.user_agent(), Some("tests/1.0"));
        assert_eq!(options.lang(), Some("fr-FR"));
        let args = args_of(&settings);
        for expected in [
            "--enable-features=WebContentsForceDark",
            "--disable-web-security",
            "--use-gl=angle",
            "--use-angle=swiftshader-webgl",
            "--host-resolver-rules=MAP a.test 127.0.0.1",
            "--mute-audio",
            "--window-size=1000,700",
            "--window-position=5,6",
            "--no-sandbox",
            "--proxy-server=proxy.test:8080",
        ] {
            assert!(
                args.iter().any(|arg| arg == expected),
                "{expected}: {args:?}"
            );
        }
        let features: Vec<_> = args
            .iter()
            .filter(|arg| arg.starts_with("--disable-features="))
            .collect();
        assert_eq!(features, ["--disable-features=Translate,Foo"]);
        assert_eq!(args.iter().filter(|arg| *arg == "--no-sandbox").count(), 1);
    }

    fn settings_file(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sb-settings-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("run.toml");
        std::fs::write(&file, text).unwrap();
        file
    }

    #[test]
    fn the_variable_names_the_settings_file_and_the_table_reads_the_new_keys() {
        let file = settings_file(
            "named",
            "dark_mode = true\nwindow_size = \"1000,700\"\n[test]\nenv = \"qa\"\npdb = true\nreruns = 2\n",
        );
        let named = file.display().to_string();
        let check = Settings::check_with(None::<&std::path::Path>, |name| {
            (name == "SB_SETTINGS_FILE").then(|| named.clone())
        });
        assert!(check.problems.is_empty(), "{:?}", check.problems);
        assert_eq!(check.file.as_deref(), Some(file.as_path()));
        let settings = check.settings;
        assert!(settings.dark_mode && settings.test.pdb);
        assert_eq!(settings.test.data.env(), "qa");
        assert_eq!(settings.test.reruns, 2);
        assert_eq!(
            settings.to_browser_config().launch.window_size,
            Some(WindowSize::new(1000, 700))
        );
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_variable_beats_the_file_and_a_blank_file_name_is_unset() {
        let file = settings_file("precedence", "dark_mode = true\n[test]\nenv = \"qa\"\n");
        let named = file.display().to_string();
        let check = Settings::check_with(None::<&std::path::Path>, |name| match name {
            "SB_SETTINGS_FILE" => Some(named.clone()),
            "SB_DARK_MODE" => Some("false".to_owned()),
            "SB_ENV" => Some("Staging".to_owned()),
            _ => None,
        });
        assert!(check.problems.is_empty(), "{:?}", check.problems);
        assert!(!check.settings.dark_mode);
        assert_eq!(check.settings.test.data.env(), "staging");
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();

        let blank = Settings::global_file_with(&|name| {
            (name == "SB_SETTINGS_FILE").then(|| "  ".to_owned())
        });
        assert_ne!(blank, Some(PathBuf::from("  ")));
    }

    #[test]
    fn a_bad_value_in_the_file_is_reported_by_the_check() {
        let file = settings_file("bad", "window_size = \"wide\"\n[test]\nenv = \"nosuch\"\n");
        let named = file.display().to_string();
        let check = Settings::check_with(None::<&std::path::Path>, |name| {
            (name == "SB_SETTINGS_FILE").then(|| named.clone())
        });
        assert!(
            check
                .problems
                .iter()
                .any(|problem| problem.source == "validation"),
            "{:?}",
            check.problems
        );
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_cdp_engine_refuses_the_browsers_it_cannot_start() {
        let firefox = Settings {
            browser: "firefox".to_owned(),
            ..Settings::default()
        };
        let error = firefox.to_launch_options().unwrap_err().to_string();
        assert!(error.contains("set chrome_bin to run firefox"), "{error}");
        let with_binary = Settings {
            chrome_bin: Some(PathBuf::from("/opt/brave/brave")),
            browser: "brave".to_owned(),
            ..Settings::default()
        };
        assert!(with_binary.to_launch_options().is_ok());
        let both = Settings {
            incognito: true,
            guest_mode: true,
            ..Settings::default()
        };
        assert!(both.to_launch_options().is_err());
    }
}
