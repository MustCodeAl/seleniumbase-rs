//! How the options of [`Settings`] become a browser start-up: the launch options
//! that [`Settings::to_browser_config`] adds for the WebDriver engine, and
//! [`Settings::to_launch_options`] for the pure CDP engine.
//!
//! The browser settings are typed fields of [`Settings`] (the groups `browser` and `session`),
//! read by the readers of `config::text` when a variable or a file gives their text, so
//! a value that does not parse is refused where it is read, naming its source, and not at
//! launch. `BrowserConfig` and `Settings` hold the same two types, which `to_browser_config`
//! clones.

use crate::browser::config::Browser;

use crate::config::settings::Settings;

use crate::error::SeleniumBaseError;
use crate::sb_cdp::LaunchOptions;

impl Settings {
    /// Turns on what only the whole process can switch: `crumbs` keeps no cookies of a reused
    /// browser, as `browser::reuse::set_crumbs(true)` does.
    pub(super) fn apply_process_wide(&self) {
        if self.session.crumbs {
            crate::browser::reuse::set_crumbs(true);
        }
    }

    /// These settings as the launch options of the pure CDP engine
    /// ([`Browser`](crate::sb_cdp::Browser) in `sb_cdp`).
    ///
    /// The browser is Chrome, Chromium (`browser = "chromium"`) or the binary `chrome_bin`
    /// names; another browser needs `chrome_bin`. The options are the ones a `BaseCase` gets from
    /// [`to_browser_config`](Self::to_browser_config), read by
    /// `LaunchOptionsBuilder::browser_config`: the headless setting (`headless` off starts a
    /// window), the phone (`mobile`, `device_metrics`), the proxy PAC file and bypass list,
    /// `block_images` and the other preferences, and the Chromium switches (`dark_mode`, ...).
    /// Settings that only a WebDriver server reads (`server`, `driver_version`, `firefox_arg`,
    /// ...) are ignored.
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
    /// use seleniumbase_rs::browser::options::LaunchSwitch;
    /// use seleniumbase_rs::Settings;
    ///
    /// let mut settings = Settings::default();
    /// settings.browser.headless = true;
    /// settings.browser.set(LaunchSwitch::DarkMode, true);
    /// settings.browser.user_agent = Some("tests/1.0".to_owned());
    /// let options = settings.to_launch_options()?;
    /// assert!(options.is_headless());
    /// assert_eq!(options.user_agent(), Some("tests/1.0"));
    /// let args = options.browser_args(std::path::Path::new("/tmp/profile"));
    /// assert!(args.contains(&"--enable-features=WebContentsForceDark".to_owned()));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn to_launch_options(&self) -> Result<LaunchOptions, SeleniumBaseError> {
        let browser = &self.browser.kind;
        // The pure CDP engine starts Chrome, Chromium or the program `chrome_bin` names.
        if self.browser.executable_path.is_none()
            && !matches!(browser, Browser::Chrome | Browser::Chromium)
        {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the pure CDP engine starts Chrome or Chromium; set chrome_bin to run {browser}"
            )));
        }
        let mut config = self.to_browser_config();
        config.fingerprint = self.load_fingerprint()?;
        #[cfg(feature = "plugins")]
        {
            config = config.with_plugin_selection(&self.plugins)?;
        }
        LaunchOptions::builder().browser_config(&config).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::run_settings::RunSettings;
    use crate::browser::options::{
        DeviceMetrics, LaunchSwitch, PageLoadStrategy, VirtualDisplay, WindowPosition, WindowSize,
    };
    use std::path::PathBuf;
    use std::time::Duration;

    /// The settings the variables `pairs` make, as the environment gives them.
    fn settings_of(pairs: &[(&str, &str)]) -> Settings {
        let pairs: Vec<(&str, String)> = pairs
            .iter()
            .map(|(name, value)| (*name, (*value).to_owned()))
            .collect();
        let mut settings = Settings::default();
        settings.apply_options(&pairs).unwrap();
        settings
    }

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
        assert!(config.browser.switches.is_empty());
        assert!(config.browser.chromium.args.is_empty());
        assert_eq!(config.run_settings(), &RunSettings::default());
        assert_eq!(config.test_data, settings.test.data);
        assert_eq!(config.session.webdriver_url(), "http://localhost:4444");
    }

    #[test]
    fn each_switch_of_the_settings_turns_its_launch_switch_on() {
        let cases: [(&str, LaunchSwitch); 19] = [
            ("SB_BLOCK_IMAGES", LaunchSwitch::BlockImages),
            ("SB_DISABLE_JS", LaunchSwitch::DisableJavaScript),
            ("SB_DISABLE_COOKIES", LaunchSwitch::DisableCookies),
            ("SB_DO_NOT_TRACK", LaunchSwitch::DoNotTrack),
            ("SB_EXTERNAL_PDF", LaunchSwitch::ExternalPdf),
            ("SB_ENABLE_SYNC", LaunchSwitch::EnableSync),
            ("SB_DISABLE_CSP", LaunchSwitch::DisableCsp),
            ("SB_USE_AUTO_EXT", LaunchSwitch::AutomationExtension),
            ("SB_PROXY_DRIVER", LaunchSwitch::ProxyForDriverDownload),
            ("SB_DARK_MODE", LaunchSwitch::DarkMode),
            ("SB_INCOGNITO", LaunchSwitch::Incognito),
            ("SB_GUEST_MODE", LaunchSwitch::Guest),
            ("SB_DEVTOOLS", LaunchSwitch::Devtools),
            ("SB_DISABLE_WS", LaunchSwitch::DisableWebSecurity),
            ("SB_SWIFTSHADER", LaunchSwitch::Swiftshader),
            ("SB_ENABLE_3D_APIS", LaunchSwitch::Enable3dApis),
            ("SB_MAXIMIZE", LaunchSwitch::Maximize),
            ("SB_NO_SANDBOX", LaunchSwitch::NoSandbox),
            ("SB_DISABLE_GPU", LaunchSwitch::DisableGpu),
        ];
        for (variable, switch) in cases {
            let config = settings_of(&[(variable, "true")]).to_browser_config();
            assert!(config.browser.has(switch), "{switch:?}");
            assert_eq!(config.browser.switches.len(), 1, "{switch:?}");
        }
    }

    #[test]
    fn headless1_makes_the_session_headless_too() {
        let config = settings_of(&[("SB_HEADLESS1", "true")]).to_browser_config();
        assert!(config.browser.headless);
        assert!(config.browser.has(LaunchSwitch::Headless1));
    }

    #[test]
    fn chromium_text_options_reach_the_arguments() {
        let settings = settings_of(&[
            (
                "SB_CHROMIUM_ARG",
                "--mute-audio, lang=fr, disable-features=Foo",
            ),
            ("SB_DISABLE_FEATURES", "Bar, Baz,"),
            ("SB_HOST_RESOLVER_RULES", "MAP example.test 127.0.0.1"),
        ]);
        let config = settings.to_browser_config();
        assert_eq!(config.browser.chromium.args, ["--mute-audio", "--lang=fr"]);
        assert_eq!(
            config.browser.chromium.disable_features,
            ["Foo", "Bar", "Baz"]
        );
        assert_eq!(
            config.browser.chromium.host_resolver_rules.as_deref(),
            Some("MAP example.test 127.0.0.1")
        );
    }

    #[test]
    fn server_port_and_protocol_fill_the_webdriver_url_unless_it_is_given() {
        let mut settings = settings_of(&[("SB_SERVER", "grid.test"), ("SB_PORT", "4445")]);
        assert_eq!(
            settings.to_browser_config().session.webdriver_url(),
            "http://grid.test:4445/wd/hub"
        );
        settings.session.webdriver_url = Some("http://other.test:1".to_owned());
        assert_eq!(
            settings.to_browser_config().session.webdriver_url(),
            "http://other.test:1"
        );
    }

    #[test]
    fn typed_text_options_become_typed_launch_settings() {
        let settings = settings_of(&[
            ("SB_WINDOW_WIDTH", "800"),
            ("SB_WINDOW_HEIGHT", "600"),
            ("SB_WINDOW_SIZE", "1280,720"),
            ("SB_WINDOW_POSITION", "10,20"),
            ("SB_DEVICE_METRICS", "412,732,3"),
            ("SB_PAGE_LOAD_STRATEGY", "eager"),
            ("SB_DRIVER_VERSION", "126"),
            ("SB_PROXY_BYPASS_LIST", "*.example.test;localhost"),
            ("SB_LOCALE", "de"),
            ("SB_LOCALE_CODE", "ar_eg"),
            ("SB_XVFB", "true"),
            ("SB_XVFB_METRICS", "1920,1080"),
            ("SB_REMOTE_DEBUG", "true"),
        ]);
        let config = settings.to_browser_config();
        let launch = &config.browser;
        assert_eq!(launch.window_size, Some(WindowSize::new(1280, 720)));
        assert_eq!(launch.window_position, Some(WindowPosition::new(10, 20)));
        assert_eq!(
            launch.device_metrics,
            Some(DeviceMetrics::new(412, 732, 3.0))
        );
        assert!(config.browser.mobile);
        assert_eq!(
            config.session.page_load_strategy,
            Some(PageLoadStrategy::Eager)
        );
        assert!(config.session.driver_version.is_some());
        assert!(!launch.proxy.bypass_list.is_empty());
        assert_eq!(config.browser.lang.as_deref(), Some("ar-EG"));
        assert_eq!(
            launch.virtual_display,
            Some(VirtualDisplay::new(1920, 1080))
        );
        assert!(launch.debugging_port.is_some());
    }

    #[test]
    fn a_value_that_does_not_parse_is_refused_when_it_is_read() {
        for pair in [
            ("SB_WINDOW_SIZE", "wide"),
            ("SB_WINDOW_POSITION", "here"),
            ("SB_DEVICE_METRICS", "412"),
            ("SB_PAGE_LOAD_STRATEGY", "slow"),
            ("SB_LOCALE_CODE", "english"),
            ("SB_XVFB_METRICS", "big"),
            ("SB_CAP_STRING", "[1]"),
            ("SB_CAP_FILE", "/no/such/caps.json"),
        ] {
            let error = Settings::default()
                .apply_options(&[(pair.0, pair.1.to_owned())])
                .unwrap_err()
                .to_string();
            assert!(error.contains(pair.0), "{error}");
        }
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn capabilities_come_from_a_string_over_a_file() {
        let dir = std::env::temp_dir().join(format!("sb-caps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("caps.json");
        std::fs::write(&file, r#"{"name": "from-file", "extra": 1}"#).unwrap();
        let settings = settings_of(&[
            ("SB_CAP_FILE", &file.display().to_string()),
            ("SB_CAP_STRING", r#"{"name": "from-text"}"#),
        ]);
        let config = settings.to_browser_config();
        let capabilities = &config.session.capabilities;
        assert_eq!(
            capabilities.get("name"),
            Some(&serde_json::json!("from-text"))
        );
        assert_eq!(capabilities.get("extra"), Some(&serde_json::json!(1)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_settings_come_from_the_demo_slow_and_timeout_options() {
        let mut settings = Settings::default();
        settings
            .apply_options(&[
                ("SB_DEMO", "true".to_owned()),
                ("SB_SLOW", "true".to_owned()),
                ("SB_SKIP_JS_WAITS", "true".to_owned()),
                ("SB_VISUAL_BASELINE", "true".to_owned()),
                ("SB_DEMO_SLEEP", "0.5".to_owned()),
                ("SB_MESSAGE_DURATION", "3".to_owned()),
                ("SB_TIMEOUT", "4".to_owned()),
                ("SB_REC_SLEEP", "true".to_owned()),
            ])
            .unwrap();
        let expected = RunSettings::default()
            .with_default_timeout(Duration::from_secs(4))
            .with_demo_mode(true)
            .with_slow_mode(true)
            .with_demo_sleep(Duration::from_millis(500))
            .with_message_duration(Duration::from_secs(3))
            .with_skip_js_waits(true)
            .with_record_sleep(true)
            .with_visual_baseline_reset(true);
        assert_eq!(settings.run, expected);
        assert_eq!(settings.to_browser_config().run_settings(), &expected);
    }

    #[test]
    fn the_run_settings_of_the_fixture_come_from_the_settings() {
        let mut settings = Settings::default();
        settings
            .apply_options(&[
                ("SB_HIGHLIGHTS", "3".to_owned()),
                ("SB_INTERVAL", "1.5".to_owned()),
                ("SB_VERIFY_DELAY", "0".to_owned()),
                ("SB_CHECK_JS", "true".to_owned()),
                ("SB_WAIT_FOR_ANGULARJS", "true".to_owned()),
                ("SB_DISABLE_BEFOREUNLOAD", "true".to_owned()),
                ("SB_TIMEOUT_MULTIPLIER", "2".to_owned()),
                ("SB_TIMEOUT", "10".to_owned()),
            ])
            .unwrap();
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
        let mut slow = Settings::default();
        slow.apply_options(&[("SB_TIMEOUT_MULTIPLIER", "0.1".to_owned())])
            .unwrap();
        let config = slow.to_browser_config();
        assert_eq!(config.run_settings().timeout_multiplier(), Some(0.5));
        let wrong = Settings::default().apply_options(&[("SB_INTERVAL", "-1".to_owned())]);
        assert!(wrong.is_err());
    }

    #[test]
    fn an_array_item_of_chromium_arg_keeps_its_commas() {
        let rules = "--host-resolver-rules=MAP a 1.2.3.4,EXCLUDE b";
        let mut settings = Settings::default();
        crate::config::text::add_chromium_items(
            &mut settings.browser,
            &[rules.to_owned(), "disable-features=Foo".to_owned()],
        );
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
        assert_eq!(config.test_data.env(), "qa");
        assert_eq!(config.test_data.var1.as_deref(), Some("x"));
    }

    #[test]
    fn the_cdp_options_carry_the_chromium_settings() {
        let settings = settings_of(&[
            ("SB_HEADLESS", "true"),
            ("SB_DARK_MODE", "true"),
            ("SB_SWIFTSHADER", "true"),
            ("SB_DISABLE_WS", "true"),
            ("SB_HOST_RESOLVER_RULES", "MAP a.test 127.0.0.1"),
            ("SB_DISABLE_FEATURES", "Foo"),
            ("SB_CHROMIUM_ARG", "--mute-audio"),
            ("SB_START_PAGE", "https://example.test/"),
            ("SB_PROXY", "proxy.test:8080"),
            ("SB_USER_AGENT", "tests/1.0"),
            ("SB_LOCALE_CODE", "fr_FR"),
            ("SB_WINDOW_SIZE", "1000,700"),
            ("SB_WINDOW_POSITION", "5,6"),
            ("SB_NO_SANDBOX", "true"),
        ]);
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

    #[test]
    fn the_cdp_options_carry_the_profile_preferences_of_the_switches() {
        let settings = settings_of(&[("SB_BLOCK_IMAGES", "true"), ("SB_DO_NOT_TRACK", "true")]);
        let options = settings.to_launch_options().unwrap();
        assert_eq!(
            options.prefs["profile.managed_default_content_settings.images"],
            serde_json::json!(2)
        );
        assert_eq!(
            options.prefs["enable_do_not_track"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn a_phone_a_pac_file_a_bypass_list_and_block_images_in_the_settings_reach_the_launch_options()
    {
        let mut settings = Settings::default();
        settings.browser.mobile = true;
        settings.browser.device_metrics = Some(DeviceMetrics::new(412, 732, 3.0));
        settings.browser.proxy.pac_url = Some("http://pac.example/proxy.pac".to_owned());
        settings.browser.proxy.bypass_list = "*.example.test;localhost".parse().unwrap();
        settings.browser.set(LaunchSwitch::BlockImages, true);

        let options = settings.to_launch_options().unwrap();
        // The phone: a mobile user agent.
        assert!(options.user_agent().is_some());
        // The proxy's PAC file and bypass list reach the command line.
        let args = options.browser_args(std::path::Path::new("/tmp/profile"));
        assert!(
            args.iter().any(|arg| arg.starts_with("--proxy-pac-url=")),
            "{args:?}"
        );
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("--proxy-bypass-list=")),
            "{args:?}"
        );
        // The preference of `block_images` is a profile preference, not an argument.
        assert_eq!(
            options.prefs["profile.managed_default_content_settings.images"],
            serde_json::json!(2)
        );
    }

    #[test]
    fn a_browser_the_engine_cannot_start_is_refused_unless_a_program_is_named() {
        let mut firefox = settings_of(&[("SB_BROWSER", "firefox")]);
        let error = firefox.to_launch_options().unwrap_err().to_string();
        assert!(error.contains("set chrome_bin to run firefox"), "{error}");
        firefox.browser.executable_path = Some(PathBuf::from("/opt/other/chrome"));
        assert!(firefox.to_launch_options().is_ok());
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
        assert!(settings.browser.has(LaunchSwitch::DarkMode) && settings.test.pdb);
        assert_eq!(settings.test.data.env(), "qa");
        assert_eq!(settings.test.reruns, 2);
        assert_eq!(
            settings.to_browser_config().browser.window_size,
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
        assert!(!check.settings.browser.has(LaunchSwitch::DarkMode));
        assert_eq!(check.settings.test.data.env(), "staging");
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();

        let blank = Settings::global_file_with(&|name| {
            (name == "SB_SETTINGS_FILE").then(|| "  ".to_owned())
        });
        assert_ne!(blank, Some(PathBuf::from("  ")));
    }

    #[test]
    fn a_bad_value_in_the_file_is_reported_by_the_check_with_its_key() {
        let file = settings_file("bad", "window_size = \"wide\"\n[test]\nenv = \"nosuch\"\n");
        let named = file.display().to_string();
        let check = Settings::check_with(None::<&std::path::Path>, |name| {
            (name == "SB_SETTINGS_FILE").then(|| named.clone())
        });
        // The value is refused where it is read, naming the key; the file is dropped whole.
        assert!(
            check.problems.iter().any(|problem| {
                problem.source == named && problem.error.to_string().contains("window_size")
            }),
            "{:?}",
            check.problems
        );
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_cdp_engine_refuses_the_browsers_it_cannot_start() {
        let firefox = settings_of(&[("SB_BROWSER", "firefox")]);
        let error = firefox.to_launch_options().unwrap_err().to_string();
        assert!(error.contains("set chrome_bin to run firefox"), "{error}");
        let mut with_binary = settings_of(&[("SB_BROWSER", "brave")]);
        with_binary.browser.executable_path = Some(PathBuf::from("/opt/brave/brave"));
        assert!(with_binary.to_launch_options().is_ok());
        let both = settings_of(&[("SB_INCOGNITO", "true"), ("SB_GUEST_MODE", "true")]);
        assert!(both.to_launch_options().is_err());
    }
}
