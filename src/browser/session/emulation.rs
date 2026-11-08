//! What a window of a session is told over DevTools after it opens: the ad
//! hosts it refuses and the phone it pretends to be.

use serde_json::{json, Value};

use crate::browser::config::BrowserConfig;
use crate::browser::devtools::CdpClient;
use crate::browser::options::emulated_phone;
use crate::config::ad_block_list::{AdBlockList, MAX_URL_PATTERNS};
use crate::error::SeleniumBaseError;

use super::fingerprint::fingerprint_overrides;
use super::BrowserSession;

impl BrowserSession {
    /// Makes the window the session is on refuse the ad and tracker hosts of
    /// the configuration, when [`BrowserSettings::ad_block`](crate::browser::options::BrowserSettings::ad_block) was on.
    ///
    /// The session does this itself: for the window it opens with, again when
    /// it reconnects, and for each window it switches to with
    /// [`switch_to_window`](Self::switch_to_window) or
    /// [`switch_to_new_window`](Self::switch_to_new_window), because the block
    /// belongs to a window. Call it after switching windows some other way.
    /// It does nothing when no ads are blocked.
    ///
    /// The hosts are the built-in [`AdBlockList`], or the list set with
    /// [`BrowserConfig::with_ad_block_list`]. They go to the browser as
    /// `Network.enable` and `Network.setBlockedURLs`, through chromedriver's
    /// DevTools channel, which exists in every Chromium session whatever its
    /// mode. Firefox, Safari and Internet Explorer have none: nothing is sent
    /// and a warning is logged when the session connects.
    ///
    /// # Errors
    ///
    /// Returns the browser's refusal if it rejects a command, and
    /// [`SeleniumBaseError::Unsupported`] if the session is not connected.
    pub async fn apply_ad_block(&self) -> Result<(), SeleniumBaseError> {
        if self.ad_block.is_empty() {
            return Ok(());
        }
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session is not connected".to_owned())
        })?;
        // A WebDriver session of any mode can send DevTools commands; only the
        // CDP and UC modes keep a client for it.
        let own;
        let cdp = match self.cdp.as_ref() {
            Some(cdp) => cdp,
            None => {
                own = CdpClient::new(driver);
                &own
            }
        };
        // The window may hold more than the library's list: what its plugins
        // blocked is put back with it, because the list replaces the window's.
        #[cfg(feature = "plugins")]
        let union = match &self.plugins {
            Some(plugins) => plugins.window_blocklist(driver).await,
            None => None,
        };
        #[cfg(not(feature = "plugins"))]
        let union: Option<Vec<String>> = None;
        for (method, params) in &self.ad_block {
            let params = match &union {
                Some(urls) if *method == "Network.setBlockedURLs" => json!({ "urls": urls }),
                _ => params.clone(),
            };
            cdp.execute(method, params).await?;
        }
        Ok(())
    }

    /// Makes the window the session is on a phone, for an undetected-mode
    /// session that was asked to be mobile ([`BrowserSettings::mobile`](crate::browser::options::BrowserSettings::mobile) or
    /// [`BrowserConfig::with_device_metrics`]).
    ///
    /// The other modes ask for the phone with the `mobileEmulation`
    /// capability when the browser starts. The undetected mode drops that
    /// capability, which a page can see, and sends the DevTools commands
    /// instead (see [`DeviceMetrics::emulation_commands`](crate::browser::options::DeviceMetrics::emulation_commands)):
    /// device metrics with `mobile: true`, touch, and the user agent. The
    /// session does this itself for the window it opens with, again when it
    /// reconnects, and for each window it switches to with
    /// [`switch_to_window`](Self::switch_to_window) or
    /// [`switch_to_new_window`](Self::switch_to_new_window), because the
    /// emulation belongs to a window. Call it after switching windows some
    /// other way. It does nothing when the session sends no such commands.
    ///
    /// # Errors
    ///
    /// Returns the browser's refusal if it rejects a command, and
    /// [`SeleniumBaseError::Unsupported`] if the session is not connected.
    pub async fn apply_mobile_emulation(&self) -> Result<(), SeleniumBaseError> {
        if self.mobile.is_empty() {
            return Ok(());
        }
        let driver = self.driver.as_ref().ok_or_else(|| {
            SeleniumBaseError::Unsupported("the session is not connected".to_owned())
        })?;
        let own;
        let cdp = match self.cdp.as_ref() {
            Some(cdp) => cdp,
            None => {
                own = CdpClient::new(driver);
                &own
            }
        };
        for (method, params) in &self.mobile {
            cdp.execute(method, params.clone()).await?;
        }
        Ok(())
    }
}

/// The URL patterns of the `Network.setBlockedURLs` commands in `commands`,
/// each once.
pub(crate) fn blocked_patterns<M: AsRef<str>>(commands: &[(M, Value)]) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for (_, params) in commands
        .iter()
        .filter(|(method, _)| method.as_ref() == "Network.setBlockedURLs")
    {
        let listed = params.get("urls").and_then(Value::as_array);
        for url in listed.into_iter().flatten().filter_map(Value::as_str) {
            if !urls.iter().any(|seen| seen == url) {
                urls.push(url.to_owned());
            }
        }
    }
    urls
}

/// The URL patterns the fingerprint of `config` makes the first window block
/// (`block_trackers`); none when no fingerprint is applied in this mode.
pub(super) fn fingerprint_blocklist(config: &BrowserConfig) -> Vec<String> {
    match config.fingerprint.as_ref() {
        Some(fp) if config.is_uc_enabled() || config.is_cdp_enabled() => {
            blocked_patterns(&fingerprint_overrides(fp))
        }
        _ => Vec::new(),
    }
}

/// The DevTools commands that make a session refuse the hosts `config` blocks,
/// in the order they are sent; none if `config` blocks nothing or the browser
/// has no DevTools channel.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a list that needs more
/// patterns than the browser is given.
pub(super) fn ad_block_commands(
    config: &BrowserConfig,
) -> Result<Vec<(&'static str, Value)>, SeleniumBaseError> {
    let Some(list) = AdBlockList::for_config(config) else {
        return Ok(Vec::new());
    };
    if !config.browser.kind.is_chromium_based() {
        tracing::warn!(
            browser = %config.browser.kind,
            "ad_block needs the DevTools Protocol, which WebDriver does not give this browser; \
             nothing is blocked"
        );
        return Ok(Vec::new());
    }
    let mut patterns = list.bounded_url_patterns()?;
    if patterns.is_empty() {
        return Ok(Vec::new());
    }
    // In the CDP and UC modes the fingerprint's own block (its tracker hosts and
    // blocked ports) goes out first through the same command, and a second
    // `Network.setBlockedURLs` replaces the first, so the list carries them.
    // Plain WebDriver applies no fingerprint, so it has nothing to join.
    for tracker in fingerprint_blocklist(config) {
        if !patterns.contains(&tracker) {
            patterns.push(tracker);
        }
    }
    if patterns.len() > MAX_URL_PATTERNS {
        return Err(SeleniumBaseError::invalid_config(format!(
            "the ad-block list and the fingerprint's trackers need {} URL patterns, more than the \
             {MAX_URL_PATTERNS} the browser is given; remove hosts",
            patterns.len()
        )));
    }
    Ok(vec![
        ("Network.enable", json!({})),
        ("Network.setBlockedURLs", json!({ "urls": patterns })),
    ])
}

/// The DevTools commands that make each window of `config`'s session a phone,
/// or none.
///
/// Only the undetected mode needs them: the other Chromium modes are given the
/// `mobileEmulation` capability. A browser that is not Chromium-based cannot
/// emulate a phone, so it starts as a desktop browser, and a warning says so.
pub(super) fn mobile_commands(config: &BrowserConfig) -> Vec<(&'static str, Value)> {
    let Some((metrics, user_agent)) = emulated_phone(
        config.browser.mobile,
        config.browser.device_metrics,
        config.browser.user_agent.as_deref(),
    ) else {
        return Vec::new();
    };
    if !config.browser.kind.is_chromium_based() {
        tracing::warn!(
            "mobile emulation needs a Chromium browser; {} starts as a desktop browser",
            config.browser.kind
        );
        return Vec::new();
    }
    if !config.is_uc_enabled() {
        return Vec::new();
    }
    metrics.emulation_commands(user_agent)
}

/// Ad blocking in a WebDriver session, against the mock WebDriver in
/// `webdriver_support` (no browser): which DevTools commands a session sends.
#[cfg(test)]
mod ad_block_tests {
    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::{Browser, DriverMode};
    use crate::config::ad_block_list::MAX_URL_PATTERNS;

    const CHROMIUM: [Browser; 8] = [
        Browser::Chrome,
        Browser::Chromium,
        Browser::Edge,
        Browser::Brave,
        Browser::Comet,
        Browser::Opera,
        Browser::ChromeForTesting,
        Browser::ChromeHeadlessShell,
    ];

    fn own_list() -> AdBlockList {
        AdBlockList::from_hosts(["tracker.example.net", "ads.example.com"]).unwrap()
    }

    fn own_patterns() -> Value {
        json!([
            "*://ads.example.com/*",
            "*://*.ads.example.com/*",
            "*://tracker.example.net/*",
            "*://*.tracker.example.net/*",
        ])
    }

    /// A session on `driver` that starts no driver of its own.
    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.session.auto_start_driver = false;
        config
    }

    /// A WebDriver that answers every command, DevTools commands included.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |_| Reply::Value(json!({})))
    }

    /// The bodies of the DevTools commands the driver was sent, oldest first.
    fn devtools_commands(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| request.body)
            .collect()
    }

    #[test]
    fn nothing_is_sent_while_ad_blocking_is_off() {
        assert!(ad_block_commands(&BrowserConfig::default())
            .unwrap()
            .is_empty());

        // A list that is set but not asked for blocks nothing.
        let mut config = BrowserConfig::default().with_ad_block_list(own_list());
        config.browser.ad_block = false;
        assert!(ad_block_commands(&config).unwrap().is_empty());
    }

    #[test]
    fn the_builtin_list_goes_to_the_browser_after_network_enable() {
        let commands = ad_block_commands(&BrowserConfig::default().with_ad_block(true)).unwrap();

        let patterns = AdBlockList::builtin().bounded_url_patterns().unwrap();
        assert_eq!(
            commands,
            [
                ("Network.enable", json!({})),
                ("Network.setBlockedURLs", json!({ "urls": patterns })),
            ]
        );
        let urls = commands[1].1["urls"].as_array().unwrap();
        assert!(urls.contains(&json!("*://doubleclick.net/*")));
        assert!(urls.contains(&json!("*://*.doubleclick.net/*")));
    }

    #[test]
    fn a_list_of_the_callers_own_is_sent_in_place_of_the_builtin_one() {
        let config = BrowserConfig::default().with_ad_block_list(own_list());
        let commands = ad_block_commands(&config).unwrap();

        assert_eq!(
            commands,
            [
                ("Network.enable", json!({})),
                ("Network.setBlockedURLs", json!({ "urls": own_patterns() })),
            ]
        );
    }

    #[test]
    fn the_trackers_of_a_fingerprint_go_with_the_ad_block_list() {
        use crate::evasion::fingerprint::Fingerprint;

        let fingerprint = Fingerprint::builder()
            .block_trackers(true)
            .build()
            .expect("the builder builds");
        let tracker = format!("*{}*", fingerprint.tracker_hosts()[0]);

        let mut config = BrowserConfig::default()
            .with_ad_block_list(own_list())
            .with_fingerprint(fingerprint);
        // A plain WebDriver session does not apply the fingerprint, so it has no trackers.
        assert!(fingerprint_blocklist(&config).is_empty());
        let plain = ad_block_commands(&config).unwrap();
        assert_eq!(plain[1].1, json!({ "urls": own_patterns() }));

        config.session.mode = Some(DriverMode::Cdp);
        let commands = ad_block_commands(&config).unwrap();
        let urls = blocked_patterns(&commands);
        let own: Vec<String> = serde_json::from_value(own_patterns()).unwrap();
        assert_eq!(urls[..own.len()], own[..]);
        assert!(urls.contains(&tracker), "{urls:?}");
        assert!(fingerprint_blocklist(&config).contains(&tracker));
    }

    #[test]
    fn the_block_keeps_what_the_fingerprint_blocks_where_the_fingerprint_is_applied() {
        let fingerprint = crate::evasion::fingerprint::Fingerprint::builder()
            .blocked_ports(vec![9222])
            .build()
            .expect("the builder builds");
        let urls = |mode: DriverMode| {
            let config = BrowserConfig::default()
                .with_mode(mode)
                .with_ad_block_list(own_list())
                .with_fingerprint(fingerprint.clone());
            ad_block_commands(&config).unwrap()[1].1["urls"]
                .as_array()
                .unwrap()
                .clone()
        };

        // A later `Network.setBlockedURLs` replaces the fingerprint's, so the
        // list that is sent holds both.
        for mode in [DriverMode::Cdp, DriverMode::Uc] {
            let urls = urls(mode);
            assert!(urls.contains(&json!("*://localhost:9222/*")), "{mode:?}");
            for own in own_patterns().as_array().unwrap() {
                assert!(urls.contains(own), "{mode:?}: {own}");
            }
        }
        // Plain WebDriver applies no fingerprint, so there is nothing to join.
        assert_eq!(json!(urls(DriverMode::WebDriver)), own_patterns());
    }

    #[test]
    fn an_empty_list_of_the_callers_own_sends_nothing() {
        let config = BrowserConfig::default().with_ad_block_list(AdBlockList::new());
        assert!(ad_block_commands(&config).unwrap().is_empty());
    }

    #[test]
    fn every_chromium_browser_is_sent_the_block_and_the_others_nothing() {
        for browser in CHROMIUM {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_ad_block(true);
            assert_eq!(ad_block_commands(&config).unwrap().len(), 2, "{browser}");
        }
        for browser in [Browser::Firefox, Browser::Safari, Browser::InternetExplorer] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_ad_block(true);
            assert!(ad_block_commands(&config).unwrap().is_empty(), "{browser}");
        }
    }

    #[test]
    fn a_list_too_long_for_the_browser_is_refused() {
        let hosts: Vec<String> = (0..=MAX_URL_PATTERNS / 2)
            .map(|i| format!("host{i}.com"))
            .collect();
        let config =
            BrowserConfig::default().with_ad_block_list(AdBlockList::from_hosts(hosts).unwrap());
        assert!(matches!(
            ad_block_commands(&config),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }

    #[tokio::test]
    async fn a_chromium_session_sends_the_two_commands_once_it_is_connected() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());

        let mut session = BrowserSession::connect(config).await.expect("connects");

        assert_eq!(
            devtools_commands(&driver),
            [
                json!({ "cmd": "Network.enable", "params": {} }),
                json!({ "cmd": "Network.setBlockedURLs", "params": { "urls": own_patterns() } }),
            ]
        );
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_session_that_does_not_block_ads_sends_no_devtools_command() {
        let driver = driver();

        let mut session = BrowserSession::connect(config_for(&driver))
            .await
            .expect("connects");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_browser_without_a_devtools_channel_is_sent_nothing() {
        let driver = driver();
        let config = config_for(&driver)
            .with_browser(Browser::Firefox)
            .with_ad_block(true);

        let mut session = BrowserSession::connect(config).await.expect("connects");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn the_builtin_list_is_sent_when_no_list_of_ones_own_is_set() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block(true);

        let mut session = BrowserSession::connect(config).await.expect("connects");

        let sent = devtools_commands(&driver);
        let patterns = AdBlockList::builtin().bounded_url_patterns().unwrap();
        assert_eq!(sent.len(), 2, "{sent:?}");
        assert_eq!(sent[1]["params"], json!({ "urls": patterns }));
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_reconnected_session_blocks_again() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config.clone())
            .await
            .expect("connects");

        session.reconnect(&config).await.expect("reconnects");

        assert_eq!(devtools_commands(&driver).len(), 4);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_window_reached_some_other_way_can_be_blocked_with_apply_ad_block() {
        let driver = driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config).await.expect("connects");

        session.apply_ad_block().await.expect("blocks again");

        assert_eq!(devtools_commands(&driver).len(), 4);
        session.quit().await.expect("quits");
    }

    /// A WebDriver that opens windows and switches to them.
    fn window_driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "w2", "type": "tab" })),
                ("POST", "/window") => Reply::Value(Value::Null),
                _ => Reply::Value(json!({})),
            }
        })
    }

    #[tokio::test]
    async fn each_window_the_session_switches_to_is_blocked_too() {
        let driver = window_driver();
        let config = config_for(&driver).with_ad_block_list(own_list());
        let mut session = BrowserSession::connect(config).await.expect("connects");
        assert_eq!(devtools_commands(&driver).len(), 2, "the first window");

        session.switch_to_new_window().await.expect("a new window");
        assert_eq!(devtools_commands(&driver).len(), 4, "the new window");

        session.switch_to_window("w2").await.expect("a switch");
        let sent = devtools_commands(&driver);
        assert_eq!(sent.len(), 6, "the window switched to");
        assert_eq!(sent[4], sent[0]);
        assert_eq!(sent[5], sent[1]);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn switching_windows_sends_nothing_when_no_ads_are_blocked() {
        let driver = window_driver();
        let mut session = BrowserSession::connect(config_for(&driver))
            .await
            .expect("connects");

        session.switch_to_new_window().await.expect("a new window");
        session.switch_to_window("w2").await.expect("a switch");

        assert!(devtools_commands(&driver).is_empty());
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_browser_that_refuses_the_block_fails_the_connection() {
        let driver = MockWebDriver::start(json!({}), |request| {
            if request.path == "/goog/cdp/execute" {
                Reply::Error(500, "unknown error")
            } else {
                Reply::Value(json!({}))
            }
        });
        let config = config_for(&driver).with_ad_block(true);

        let connected = BrowserSession::connect(config).await;

        assert!(
            matches!(connected, Err(SeleniumBaseError::CdpDriver(_))),
            "a block that is not in place must not look like one that is"
        );
    }

    #[tokio::test]
    async fn a_list_too_long_is_refused_before_a_browser_is_asked_for() {
        let driver = driver();
        let hosts: Vec<String> = (0..=MAX_URL_PATTERNS / 2)
            .map(|i| format!("host{i}.com"))
            .collect();
        let config =
            config_for(&driver).with_ad_block_list(AdBlockList::from_hosts(hosts).unwrap());

        let connected = BrowserSession::connect(config).await;

        assert!(matches!(
            connected,
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert!(
            driver.requests("/session").is_empty(),
            "no session was opened for a configuration that cannot work"
        );
    }
}

#[cfg(test)]
mod mobile_tests {
    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::{Browser, DriverMode};
    use crate::browser::options::{DeviceMetrics, MOBILE_USER_AGENT};

    fn config_for(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.session.auto_start_driver = false;
        config
    }

    /// A WebDriver that answers every command, opens windows and switches to
    /// them.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "w2", "type": "tab" })),
                ("POST", "/window") => Reply::Value(Value::Null),
                _ => Reply::Value(json!({})),
            }
        })
    }

    fn devtools_commands(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/goog/cdp/execute")
            .into_iter()
            .map(|request| request.body)
            .collect()
    }

    fn names(commands: &[Value]) -> Vec<&str> {
        commands
            .iter()
            .map(|command| command["cmd"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn only_the_undetected_mode_sends_the_phone_over_devtools() {
        let mobile = BrowserConfig::default().with_mobile(true);
        assert!(mobile_commands(&mobile).is_empty());
        assert!(mobile_commands(&BrowserConfig::default().with_mode(DriverMode::Uc)).is_empty());

        let uc = mobile.clone().with_mode(DriverMode::Uc);
        let commands = mobile_commands(&uc);
        assert_eq!(
            commands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            [
                "Emulation.setDeviceMetricsOverride",
                "Emulation.setTouchEmulationEnabled",
                "Emulation.setUserAgentOverride"
            ]
        );
        assert_eq!(
            commands[0].1,
            json!({ "width": 390, "height": 715, "deviceScaleFactor": 3.0, "mobile": true })
        );
        assert_eq!(commands[2].1["userAgent"], MOBILE_USER_AGENT);

        let own = uc
            .with_device_metrics(DeviceMetrics::new(360, 640, 2.0))
            .with_user_agent("MyAgent/1.0");
        let commands = mobile_commands(&own);
        assert_eq!(commands[0].1["width"], 360);
        assert_eq!(commands[2].1["userAgent"], "MyAgent/1.0");
    }

    #[test]
    fn a_browser_that_is_not_chromium_is_sent_no_phone() {
        let firefox = BrowserConfig::default()
            .with_browser(Browser::Firefox)
            .with_mobile(true);
        assert!(mobile_commands(&firefox).is_empty());
    }

    #[tokio::test]
    async fn an_undetected_mobile_session_is_made_a_phone_and_each_new_window_too() {
        let driver = driver();
        let config = config_for(&driver)
            .with_mode(DriverMode::Uc)
            .with_mobile(true);
        let mut session = match BrowserSession::connect(config).await {
            Ok(session) => session,
            Err(error) => panic!("connects: {error}"),
        };
        let first = devtools_commands(&driver);
        let phone: Vec<&str> = names(&first)
            .into_iter()
            .filter(|name| name.starts_with("Emulation."))
            .collect();
        assert_eq!(
            phone,
            [
                "Emulation.setDeviceMetricsOverride",
                "Emulation.setTouchEmulationEnabled",
                "Emulation.setUserAgentOverride"
            ],
            "{first:?}"
        );

        let before = first.len();
        session.switch_to_new_window().await.expect("a new window");
        let after = devtools_commands(&driver);
        assert_eq!(after.len(), before + 3, "{after:?}");
        assert_eq!(names(&after)[before], "Emulation.setDeviceMetricsOverride");

        session.switch_to_window("w2").await.expect("a switch");
        assert_eq!(devtools_commands(&driver).len(), before + 6);
        session.quit().await.expect("quits");
    }

    #[tokio::test]
    async fn a_session_that_is_not_mobile_sends_no_emulation_command() {
        let driver = driver();
        let config = config_for(&driver).with_mode(DriverMode::Uc);
        let mut session = BrowserSession::connect(config).await.expect("connects");
        let sent = devtools_commands(&driver);
        assert!(
            names(&sent)
                .iter()
                .all(|name| !name.starts_with("Emulation.setDeviceMetrics")),
            "{sent:?}"
        );
        session.quit().await.expect("quits");
    }
}
