//! The capabilities a session asks the WebDriver server for, one builder per
//! browser family.

use serde_json::{json, Value};
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
use thirtyfour::prelude::DesiredCapabilities;

use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::options::{firefox_preferences, BrowserSettings, LaunchSwitch};
use crate::error::SeleniumBaseError;

/// The capabilities a session for `config` asks the WebDriver server for: what
/// the settings produce, with [`BrowserSettings::capabilities`] merged over it.
pub(super) fn session_capabilities(
    config: &BrowserConfig,
) -> Result<thirtyfour::Capabilities, SeleniumBaseError> {
    let mut caps = browser_capabilities(config)?;
    // Downloads go to ./downloaded_files, as in Python; what the capabilities
    // the caller gave name for themselves (merged next) wins.
    crate::browser::downloads::ask_for_downloads_in(
        &mut caps,
        config.browser.kind,
        &crate::browser::downloads::downloads_dir(),
    );
    // Chrome's password prompts take the keyboard from the page, so they stay
    // off, as in Python; what the capabilities the caller gave name wins.
    crate::browser::password_prompts::ask_for_quiet_passwords_in(&mut caps, config.browser.kind);
    keep_recorder_dialogs_open(&mut caps, config);

    config.session.capabilities.apply_to(&mut caps);
    Ok(caps)
}

/// The capabilities the settings alone produce for the browser `config` names.
fn browser_capabilities(
    config: &BrowserConfig,
) -> Result<thirtyfour::Capabilities, SeleniumBaseError> {
    let caps: thirtyfour::Capabilities = match config.browser.kind {
        // Brave, Comet, Opera and the two Chrome builds are driven as Chrome is,
        // with their own program set as the binary.
        Browser::Chrome
        | Browser::Chromium
        | Browser::Brave
        | Browser::Comet
        | Browser::Opera
        | Browser::ChromeForTesting
        | Browser::ChromeHeadlessShell => {
            let mut caps = DesiredCapabilities::chrome();
            apply_chromium_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::Edge => {
            let mut caps = DesiredCapabilities::edge();
            apply_chromium_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::Firefox => {
            let mut caps = DesiredCapabilities::firefox();
            apply_firefox_capabilities(&mut caps, config)?;
            caps.into()
        }
        // The server's own browser: only the capabilities the caller gave.
        Browser::Remote => thirtyfour::Capabilities::new(),
        Browser::Safari => {
            let mut caps = DesiredCapabilities::safari();
            apply_generic_capabilities(&mut caps, config)?;
            caps.into()
        }
        Browser::InternetExplorer => {
            let mut caps = DesiredCapabilities::internet_explorer();
            apply_generic_capabilities(&mut caps, config)?;
            caps.into()
        }
    };
    Ok(caps)
}

/// Recorder Mode polls the page while a person works in it: a dialog the
/// person has not answered must stay open, not be dismissed by the next
/// command (chromedriver's default is to dismiss it and fail the command).
fn keep_recorder_dialogs_open(caps: &mut thirtyfour::Capabilities, config: &BrowserConfig) {
    if config.recorder_mode()
        && config.browser.kind.is_chromium_based()
        && caps.get_mut("unhandledPromptBehavior").is_none()
    {
        // `Capabilities::set` only fails when serialising, which text cannot.
        let _ = caps.set("unhandledPromptBehavior", "ignore");
    }
}

/// What every browser's capabilities take from the settings: the page load
/// strategy. Safari and Internet Explorer have nothing else to set.
fn apply_generic_capabilities(
    caps: &mut impl thirtyfour::CapabilitiesHelper,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    if let Some(strategy) = config.session.page_load_strategy {
        caps.set_page_load_strategy(strategy.into())?;
    }
    Ok(())
}

/// Firefox's `moz:firefoxOptions` and page load strategy from the settings: the
/// headless flag, the browser program, the profile folder (`-profile`, from
/// `user_data_dir`), the arguments, and the preferences, among them the user
/// agent and the language as Python SeleniumBase sets them.
fn apply_firefox_capabilities(
    caps: &mut thirtyfour::FirefoxCapabilities,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    use thirtyfour::CapabilitiesHelper;
    add_firefox_arguments(caps, config)?;
    if let Some(binary) = &config.browser.executable_path {
        caps.set_firefox_binary(&binary.to_string_lossy())?;
    }
    if let Some(strategy) = config.session.page_load_strategy {
        caps.set_page_load_strategy(strategy.into())?;
    }
    let preferences = firefox_preferences_for(config)?;
    if !preferences.is_empty() {
        let preferences: thirtyfour::common::capabilities::firefox::FirefoxPreferences =
            serde_json::from_value(Value::Object(preferences))?;
        caps.set_preferences(preferences)?;
    }
    Ok(())
}

/// The command-line arguments Firefox is started with: headless, private
/// window, developer tools, the arguments of the Firefox settings, and the
/// profile folder.
fn add_firefox_arguments(
    caps: &mut thirtyfour::FirefoxCapabilities,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    let launch = &config.browser;
    if config.browser.headless {
        caps.add_arg("-headless")?;
    }
    if launch.has(LaunchSwitch::Incognito) {
        caps.add_arg("-private")?;
    }
    if launch.has(LaunchSwitch::Devtools) && !config.browser.headless {
        caps.add_arg("-devtools")?;
    }
    warn_about_chromium_only_switches(launch);
    for arg in config.session.firefox.args() {
        caps.add_arg(arg)?;
    }
    if let Some(profile) = &config.browser.user_data_dir {
        caps.add_arg("-profile")?;
        caps.add_arg(&profile.to_string_lossy())?;
    }
    Ok(())
}

/// Says, for each switch that only a Chromium browser understands, that
/// Firefox ignores it.
fn warn_about_chromium_only_switches(launch: &BrowserSettings) {
    for switch in [
        LaunchSwitch::DarkMode,
        LaunchSwitch::Guest,
        LaunchSwitch::DisableWebSecurity,
        LaunchSwitch::Swiftshader,
        LaunchSwitch::DisableGpu,
    ] {
        if launch.has(switch) {
            tracing::warn!("{switch:?} needs a Chromium browser; Firefox ignores it");
        }
    }
}

/// The preferences Firefox is asked for: the ones the switches set, the proxy,
/// the user agent and the language, with the preferences named outright in the
/// Firefox settings over them.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a proxy or PAC address that
/// does not parse, or that carries a login, which Firefox cannot be given.
fn firefox_preferences_for(
    config: &BrowserConfig,
) -> Result<serde_json::Map<String, Value>, SeleniumBaseError> {
    let launch = &config.browser;
    let mut from_settings = firefox_preferences(launch.switches_on());
    if let Some(proxy) = &config.browser.proxy.server {
        let proxy: crate::browser::options::ProxyAddress = proxy.parse()?;
        if proxy.credentials().is_some() {
            return Err(SeleniumBaseError::invalid_config(
                crate::browser::config::FIREFOX_PROXY_LOGIN,
            ));
        }
        from_settings.extend(proxy.firefox_preferences(&launch.proxy.bypass_list));
    }
    if let Some(pac) = &config.browser.proxy.pac_url {
        let pac: crate::browser::options::PacUrl = pac.parse()?;
        if pac.credentials().is_some() {
            return Err(SeleniumBaseError::invalid_config(
                crate::browser::config::FIREFOX_PROXY_LOGIN,
            ));
        }
        from_settings.extend(pac.firefox_preferences());
    }
    if let Some(agent) = &config.browser.user_agent {
        from_settings.insert("general.useragent.override".into(), json!(agent));
    }
    if let Some(locale) = &config.browser.lang {
        from_settings.insert("intl.accept_languages".into(), json!(locale));
    }
    Ok(config.session.firefox.preferences_over(from_settings))
}

fn apply_chromium_capabilities<C: ChromiumLikeCapabilities>(
    caps: &mut C,
    config: &BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    crate::evasion::options::StealthOptions::from(config).apply_to(caps)
}

#[cfg(test)]
mod capability_tests {
    use serde_json::json;

    use super::*;
    use crate::browser::options::{
        CapabilityOverrides, FirefoxOptions, PageLoadStrategy, WindowSize,
    };

    /// `prefs` without the preferences that name the downloads folder.
    fn without_downloads(prefs: &Value) -> Value {
        let mut rest = prefs.clone();
        if let Some(map) = rest.as_object_mut() {
            map.retain(|name, _| !name.starts_with("browser.download."));
        }
        rest
    }

    #[test]
    fn chrome_is_asked_to_save_downloads_in_downloaded_files() {
        let caps = caps_json(&BrowserConfig::default());
        let prefs = &caps["goog:chromeOptions"]["prefs"];
        let folder = crate::browser::downloads::downloads_dir();
        assert_eq!(
            prefs["download.default_directory"],
            folder.to_string_lossy().as_ref()
        );
        assert_eq!(prefs["download.prompt_for_download"], false);
    }

    #[test]
    fn a_preference_the_caller_names_wins_over_the_downloads_folder() {
        let mut config = BrowserConfig::default();
        config.session.capabilities = CapabilityOverrides::from_text(
            r#"{"goog:chromeOptions": {"prefs": {"download.default_directory": "/elsewhere"}}}"#,
        )
        .unwrap();
        let prefs = &caps_json(&config)["goog:chromeOptions"]["prefs"];
        assert_eq!(prefs["download.default_directory"], "/elsewhere");
        assert_eq!(prefs["download.prompt_for_download"], false);
    }

    fn caps_json(config: &BrowserConfig) -> Value {
        serde_json::to_value(session_capabilities(config).unwrap()).unwrap()
    }

    #[test]
    fn firefox_gets_its_arguments_preferences_and_page_load_strategy() {
        let mut firefox = FirefoxOptions::default();
        firefox.add_args("private-window").unwrap();
        firefox
            .add_preferences("javascript.enabled:true,intl.accept_languages:fr")
            .unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(true)
            .with_block_images(true)
            .with_disable_js(true)
            .with_do_not_track(true)
            .with_page_load_strategy(PageLoadStrategy::Eager)
            .with_firefox_options(firefox);
        let caps = caps_json(&config);

        assert_eq!(caps["browserName"], "firefox");
        assert_eq!(caps["pageLoadStrategy"], "eager");
        let options = &caps["moz:firefoxOptions"];
        assert_eq!(options["args"], json!(["-headless", "--private-window"]));
        // An explicit preference wins over the one a switch sets.
        assert_eq!(
            without_downloads(&options["prefs"]),
            json!({
                "permissions.default.image": 2,
                "javascript.enabled": true,
                "privacy.donottrackheader.enabled": true,
                "intl.accept_languages": "fr",
            })
        );
    }

    #[test]
    fn a_plain_firefox_session_asks_for_no_preferences_but_the_downloads_folder() {
        let caps = caps_json(&BrowserConfig::default().with_browser(Browser::Firefox));
        let options = &caps["moz:firefoxOptions"];
        assert_eq!(without_downloads(&options["prefs"]), json!({}), "{options}");
        assert!(options.get("binary").is_none(), "{options}");
        assert!(caps.get("pageLoadStrategy").is_none());
    }

    #[test]
    fn firefox_gets_its_program_profile_user_agent_and_language() {
        let mut firefox = FirefoxOptions::default();
        firefox.add_preferences("intl.accept_languages:de").unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(true)
            .with_browser_binary_path("/opt/firefox/firefox")
            .with_user_data_dir("/tmp/ff-profile")
            .with_user_agent("Agent/1.0")
            .with_locale("fr")
            .with_firefox_options(firefox);
        let options = &caps_json(&config)["moz:firefoxOptions"];
        assert_eq!(options["binary"], "/opt/firefox/firefox");
        assert_eq!(
            options["args"],
            json!(["-headless", "-profile", "/tmp/ff-profile"])
        );
        assert_eq!(options["prefs"]["general.useragent.override"], "Agent/1.0");
        // A preference named outright wins over the one the locale sets.
        assert_eq!(options["prefs"]["intl.accept_languages"], "de");
    }

    #[test]
    fn edge_is_asked_for_as_microsoft_edge_with_its_own_options() {
        let config = BrowserConfig::default()
            .with_browser(Browser::Edge)
            .with_headless(true)
            .with_browser_binary_path("/opt/edge/msedge");
        let caps = caps_json(&config);
        assert_eq!(caps["browserName"], "MicrosoftEdge");
        assert!(caps.get("goog:chromeOptions").is_none(), "{caps}");
        let options = &caps["ms:edgeOptions"];
        assert_eq!(options["binary"], "/opt/edge/msedge");
        let args = options["args"].as_array().unwrap();
        assert!(
            args.iter().any(|arg| arg
                .as_str()
                .is_some_and(|arg| arg.starts_with("--headless"))),
            "{args:?}"
        );
    }

    #[test]
    fn capability_overrides_are_merged_into_every_browsers_capabilities() {
        let overrides = CapabilityOverrides::from_text(
            r#"{"se:name": "smoke", "goog:chromeOptions": {"args": ["--mute-audio"]}}"#,
        )
        .unwrap();
        for browser in [
            Browser::Chrome,
            Browser::Chromium,
            Browser::Edge,
            Browser::Firefox,
        ] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_capabilities(overrides.clone());
            assert_eq!(caps_json(&config)["se:name"], "smoke", "{browser:?}");
        }

        // Chrome's own arguments keep what the settings added.
        let config = BrowserConfig::default()
            .with_window_size(WindowSize::new(800, 600))
            .with_capabilities(overrides);
        let args = caps_json(&config)["goog:chromeOptions"]["args"].clone();
        let args = args.as_array().unwrap();
        assert!(args.contains(&json!("--mute-audio")), "{args:?}");
        assert!(args.contains(&json!("--window-size=800,600")), "{args:?}");
    }

    #[test]
    fn an_override_can_replace_what_the_settings_chose() {
        let config = BrowserConfig::default()
            .with_page_load_strategy(PageLoadStrategy::Eager)
            .with_capabilities(
                CapabilityOverrides::from_text(r#"{"pageLoadStrategy": "none"}"#).unwrap(),
            );
        assert_eq!(caps_json(&config)["pageLoadStrategy"], "none");
    }

    #[test]
    fn firefox_gets_its_proxy_as_preferences() {
        let firefox = BrowserConfig::default().with_browser(Browser::Firefox);
        let socks = firefox
            .clone()
            .with_proxy("socks5://proxy.test:1080")
            .with_proxy_bypass_list("*.example.com;localhost".parse().unwrap());
        let caps = caps_json(&socks);
        let prefs = &caps["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.type"], 1);
        assert_eq!(prefs["network.proxy.socks"], "proxy.test");
        assert_eq!(prefs["network.proxy.socks_port"], 1080);
        assert_eq!(prefs["network.proxy.socks_version"], 5);
        assert_eq!(
            prefs["network.proxy.no_proxies_on"],
            ".example.com, localhost"
        );

        let http = caps_json(&firefox.clone().with_proxy("h.test:8080"));
        let prefs = &http["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.http"], "h.test");
        assert_eq!(prefs["network.proxy.http_port"], 8080);
        assert_eq!(prefs["network.proxy.ssl"], "h.test");
        assert_eq!(prefs["network.proxy.ssl_port"], 8080);

        let pac = caps_json(&firefox.with_proxy_pac_url("http://pac.test/p.pac"));
        let prefs = &pac["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["network.proxy.type"], 2);
        assert_eq!(
            prefs["network.proxy.autoconfig_url"],
            "http://pac.test/p.pac"
        );
    }

    #[test]
    fn firefox_with_a_proxy_login_is_refused_with_pythons_message() {
        for config in [
            BrowserConfig::default()
                .with_browser(Browser::Firefox)
                .with_proxy("user:hunter2@h.test:8080"),
            BrowserConfig::default()
                .with_browser(Browser::Firefox)
                .with_proxy_pac_url("user:hunter2@http://pac.test/p.pac"),
        ] {
            let error = session_capabilities(&config).unwrap_err().to_string();
            assert!(
                error.contains("Chrome or Edge is required when using a proxy server"),
                "{error}"
            );
            assert!(!error.contains("hunter2"), "{error}");
        }
    }

    #[test]
    fn remote_asks_for_nothing_but_the_capabilities_it_was_given() {
        let caps = CapabilityOverrides::from_text(
            r#"{"browserName":"chrome","bstack:options":{"os":"Windows"}}"#,
        )
        .unwrap();
        let config = BrowserConfig::default()
            .with_browser(Browser::Remote)
            .with_headless(true)
            .with_block_images(true)
            .with_window_size(WindowSize::new(800, 600))
            .with_capabilities(caps);
        assert_eq!(
            caps_json(&config),
            json!({"browserName": "chrome", "bstack:options": {"os": "Windows"}})
        );
    }

    #[test]
    fn firefox_is_asked_for_a_private_window_and_developer_tools() {
        let config = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_headless(false)
            .with_incognito(true)
            .with_devtools(true);
        let caps = serde_json::to_value(session_capabilities(&config).unwrap()).unwrap();
        assert_eq!(
            caps["moz:firefoxOptions"]["args"],
            json!(["-private", "-devtools"])
        );

        let headless = config.with_headless(true);
        let caps = serde_json::to_value(session_capabilities(&headless).unwrap()).unwrap();
        assert_eq!(
            caps["moz:firefoxOptions"]["args"],
            json!(["-headless", "-private"])
        );
    }
}
