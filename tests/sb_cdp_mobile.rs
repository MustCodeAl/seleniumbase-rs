//! Mobile emulation, the launch switches and the `BrowserConfig` bridge of the
//! pure CDP engine, against a scripted browser, with no Chrome.
//!
//! The tests read back the protocol commands the mock received and the
//! arguments the options would start Chrome with. Run them with
//! `cargo test --features test-util --test sb_cdp_mobile`.

#![cfg(feature = "test-util")]

use std::path::Path;

use seleniumbase_rs::browser::options::{
    DeviceMetrics, LaunchSwitch, ProxyBypassList, WindowPosition, WindowSize, MOBILE_USER_AGENT,
};
use seleniumbase_rs::sb_cdp::{
    Browser, ColorScheme, LaunchOptions, LaunchOptionsBuilder, MockCtrl,
};
use seleniumbase_rs::{BrowserConfig, Fingerprint, OsType, SeleniumBaseError};
use serde_json::json;

async fn launched(options: LaunchOptions) -> (Browser, MockCtrl) {
    let mock = MockCtrl::new();
    let browser = Browser::launch_mocked(options, &mock).await.unwrap();
    (browser, mock)
}

fn args(builder: LaunchOptionsBuilder) -> Vec<String> {
    builder
        .no_sandbox(false)
        .build()
        .unwrap()
        .browser_args(Path::new("/tmp/profile"))
}

#[tokio::test]
async fn a_mobile_launch_sets_every_tab_up_as_a_phone() {
    let options = LaunchOptions::builder().mobile(true).build().unwrap();
    let (browser, mock) = launched(options).await;

    let metrics = mock.calls_to("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics.len(), 1, "{metrics:?}");
    assert_eq!(
        metrics[0].params,
        json!({ "width": 390, "height": 715, "deviceScaleFactor": 3.0, "mobile": true })
    );
    assert_eq!(metrics[0].session_id.as_deref(), Some("S1"));

    let touch = mock.calls_to("Emulation.setTouchEmulationEnabled");
    assert_eq!(touch.len(), 1);
    assert_eq!(
        touch[0].params,
        json!({ "enabled": true, "maxTouchPoints": 5 })
    );

    let agent = mock.calls_to("Emulation.setUserAgentOverride");
    assert_eq!(agent.len(), 1);
    assert_eq!(agent[0].params["userAgent"], json!(MOBILE_USER_AGENT));
    assert_eq!(agent[0].params["userAgentMetadata"]["mobile"], json!(true));
    assert_eq!(
        agent[0].params["userAgentMetadata"]["platform"],
        json!("Android")
    );

    // A tab opened later is a phone too.
    browser.new_page().await.unwrap();
    assert_eq!(mock.calls_to("Emulation.setDeviceMetricsOverride").len(), 2);
    assert_eq!(mock.calls_to("Emulation.setTouchEmulationEnabled").len(), 2);
}

#[tokio::test]
async fn chosen_metrics_and_a_chosen_agent_replace_the_defaults() {
    let options = LaunchOptions::builder()
        .device_metrics(DeviceMetrics::new(412, 732, 2.625))
        .user_agent("Agent/1")
        .build()
        .unwrap();
    let (_browser, mock) = launched(options).await;

    let metrics = mock.calls_to("Emulation.setDeviceMetricsOverride");
    assert_eq!(
        metrics[0].params,
        json!({ "width": 412, "height": 732, "deviceScaleFactor": 2.625, "mobile": true })
    );
    let agent = mock.calls_to("Emulation.setUserAgentOverride");
    assert_eq!(agent[0].params["userAgent"], json!("Agent/1"));
}

#[tokio::test]
async fn a_plain_launch_emulates_no_device() {
    let (_browser, mock) = launched(LaunchOptions::default()).await;

    assert!(mock
        .calls_to("Emulation.setDeviceMetricsOverride")
        .is_empty());
    assert!(mock
        .calls_to("Emulation.setTouchEmulationEnabled")
        .is_empty());
    assert!(mock.calls_to("Page.setBypassCSP").is_empty());
}

#[test]
fn the_phone_agent_is_also_the_browsers_own_agent() {
    let arguments = args(LaunchOptions::builder().mobile(true));
    assert!(
        arguments.contains(&format!("--user-agent={MOBILE_USER_AGENT}")),
        "{arguments:?}"
    );
}

#[test]
fn mobile_emulation_and_a_fingerprint_are_refused_together() {
    let fingerprint = Fingerprint::randomized(OsType::Windows, 7);
    for builder in [
        LaunchOptions::builder().mobile(true),
        LaunchOptions::builder().device_metrics(DeviceMetrics::default()),
    ] {
        let error = builder.fingerprint(&fingerprint).build().unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(message)
                if message.contains("mobile emulation and a fingerprint")),
            "{error:?}"
        );
    }
    // Either alone is fine.
    LaunchOptions::builder()
        .fingerprint(&fingerprint)
        .build()
        .unwrap();
}

#[tokio::test]
async fn disable_csp_bypasses_the_policy_in_every_tab() {
    let options = LaunchOptions::builder()
        .switch(LaunchSwitch::DisableCsp, true)
        .build()
        .unwrap();
    let (browser, mock) = launched(options).await;

    let bypass = mock.calls_to("Page.setBypassCSP");
    assert_eq!(bypass.len(), 1);
    assert_eq!(bypass[0].params, json!({ "enabled": true }));
    assert_eq!(bypass[0].session_id.as_deref(), Some("S1"));

    browser.new_page().await.unwrap();
    assert_eq!(mock.calls_to("Page.setBypassCSP").len(), 2);
}

#[tokio::test]
async fn the_emulation_of_a_tab_can_be_changed_at_run_time() {
    let (browser, mock) = Browser::new_mocked();
    let emulation = browser.default_page().await.unwrap().emulation();

    emulation
        .device_metrics(DeviceMetrics::new(360, 640, 2.0), true)
        .await
        .unwrap();
    emulation.touch(true).await.unwrap();
    emulation.touch(false).await.unwrap();
    emulation.clear_device_metrics().await.unwrap();
    emulation.color_scheme(ColorScheme::Dark).await.unwrap();
    emulation
        .color_scheme(ColorScheme::NoPreference)
        .await
        .unwrap();

    assert_eq!(
        mock.calls_to("Emulation.setDeviceMetricsOverride")[0].params,
        json!({ "width": 360, "height": 640, "deviceScaleFactor": 2.0, "mobile": true })
    );
    let touch: Vec<_> = mock
        .calls_to("Emulation.setTouchEmulationEnabled")
        .into_iter()
        .map(|call| call.params)
        .collect();
    assert_eq!(
        touch,
        [
            json!({ "enabled": true, "maxTouchPoints": 5 }),
            json!({ "enabled": false })
        ]
    );
    assert_eq!(
        mock.calls_to("Emulation.clearDeviceMetricsOverride").len(),
        1
    );
    let media: Vec<_> = mock
        .calls_to("Emulation.setEmulatedMedia")
        .into_iter()
        .map(|call| call.params["features"][0].clone())
        .collect();
    assert_eq!(
        media,
        [
            json!({ "name": "prefers-color-scheme", "value": "dark" }),
            json!({ "name": "prefers-color-scheme", "value": "" })
        ]
    );
}

#[test]
fn each_switch_gives_the_arguments_the_webdriver_engine_gives() {
    let contains = |builder: LaunchOptionsBuilder, wanted: &[&str]| {
        let arguments = args(builder);
        for want in wanted {
            assert!(
                arguments.iter().any(|arg| arg == want),
                "{want} is missing from {arguments:?}"
            );
        }
    };
    contains(
        LaunchOptions::builder().dark_mode(true),
        &["--enable-features=WebContentsForceDark"],
    );
    contains(
        LaunchOptions::builder().headless(false).devtools(true),
        &["--auto-open-devtools-for-tabs"],
    );
    contains(
        LaunchOptions::builder().disable_web_security(true),
        &[
            "--disable-web-security",
            "--ignore-certificate-errors",
            "--ignore-ssl-errors=yes",
        ],
    );
    contains(
        LaunchOptions::builder().swiftshader(true),
        &["--use-gl=angle", "--use-angle=swiftshader-webgl"],
    );
    contains(
        LaunchOptions::builder().headless(false).maximize(true),
        &["--start-maximized"],
    );
    contains(
        LaunchOptions::builder().switch(LaunchSwitch::DisableGpu, true),
        &["--disable-gpu"],
    );
    contains(
        LaunchOptions::builder().switch(LaunchSwitch::Incognito, true),
        &["--incognito"],
    );
}

#[test]
fn the_lists_of_values_become_one_switch_each() {
    let arguments = args(
        LaunchOptions::builder()
            .disable_features(["A", "B"])
            .disable_features(["B", "C"])
            .enable_features(["X"])
            .host_resolver_rules("MAP a.test 127.0.0.1")
            .host_resolver_rules("MAP b.test 127.0.0.1")
            .arg("--disable-features=D"),
    );
    let one = |name: &str| {
        let found: Vec<_> = arguments
            .iter()
            .filter(|arg| arg.starts_with(&format!("--{name}=")))
            .collect();
        assert_eq!(found.len(), 1, "{name}: {arguments:?}");
        found[0].clone()
    };
    assert_eq!(
        one("disable-features"),
        "--disable-features=Translate,A,B,C,D"
    );
    assert_eq!(one("enable-features"), "--enable-features=X");
    assert_eq!(
        one("host-resolver-rules"),
        "--host-resolver-rules=MAP a.test 127.0.0.1,MAP b.test 127.0.0.1"
    );
}

#[test]
fn a_bypass_list_and_a_pac_file_reach_chrome_without_a_login() {
    let list = ProxyBypassList::new(["*.example.com", "localhost"]).unwrap();
    let arguments = args(
        LaunchOptions::builder()
            .proxy_bypass_list(list.clone())
            .proxy_pac_url("user:secret@http://pac.example/proxy.pac"),
    );
    assert!(arguments.contains(&format!(
        "--proxy-bypass-list={}",
        list.to_chrome_value().unwrap()
    )));
    assert!(arguments.contains(&"--proxy-pac-url=http://pac.example/proxy.pac".to_owned()));
    assert!(
        !arguments.iter().any(|arg| arg.contains("secret")),
        "{arguments:?}"
    );
}

#[tokio::test]
async fn a_pac_file_that_asks_for_a_login_is_answered_over_the_protocol() {
    let options = LaunchOptions::builder()
        .proxy_pac_url("user:secret@http://pac.example/proxy.pac")
        .build()
        .unwrap();
    assert!(!format!("{options:?}").contains("secret"));
    let (_browser, mock) = launched(options).await;

    let enabled = mock.calls_to("Fetch.enable");
    assert!(!enabled.is_empty(), "no Fetch.enable");
    assert_eq!(enabled[0].params["handleAuthRequests"], json!(true));
}

/// A config that sets every option the pure CDP engine can use.
fn full_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: false,
        mobile: true,
        user_agent: Some("Agent/1".to_owned()),
        locale: Some("ar_eg".to_owned()),
        proxy: Some("user:secret@127.0.0.1:8080".to_owned()),
        start_page: Some("https://example.com/".to_owned()),
        extra_args: vec!["mute-audio".to_owned(), "--no-pings".to_owned()],
        ad_block: true,
        ..BrowserConfig::default()
    };
    let launch = &mut config.launch;
    for switch in [
        LaunchSwitch::DarkMode,
        LaunchSwitch::Incognito,
        LaunchSwitch::Devtools,
        LaunchSwitch::DisableWebSecurity,
        LaunchSwitch::Swiftshader,
        LaunchSwitch::Maximize,
        LaunchSwitch::DisableGpu,
        LaunchSwitch::BlockImages,
        LaunchSwitch::EscEnd,
    ] {
        launch.set(switch, true);
    }
    launch.window_size = Some(WindowSize::new(1000, 700));
    launch.window_position = Some(WindowPosition::new(5, 6));
    launch.proxy_bypass_list = ProxyBypassList::new(["localhost"]).unwrap();
    launch.disable_features = vec!["Bar".to_owned()];
    launch.host_resolver_rules = Some("MAP a.test 127.0.0.1".to_owned());
    config
}

#[test]
fn a_config_with_every_option_gives_this_command_line() {
    let options = LaunchOptions::builder()
        .browser_config(&full_config())
        .no_sandbox(false)
        .build()
        .unwrap();
    let expected = [
        "--remote-debugging-port=0",
        "--user-data-dir=/tmp/profile",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-search-engine-choice-screen",
        "--disable-features=Translate,Bar",
        "--password-store=basic",
        "--use-mock-keychain",
        "--incognito",
        "--enable-features=WebContentsForceDark",
        "--host-resolver-rules=MAP a.test 127.0.0.1",
        "--proxy-server=127.0.0.1:8080",
        "--proxy-bypass-list=localhost",
        "--user-agent=Agent/1",
        "--lang=ar-EG",
        "--window-position=5,6",
        "--auto-open-devtools-for-tabs",
        "--disable-web-security",
        "--ignore-certificate-errors",
        "--ignore-ssl-errors=yes",
        "--use-gl=angle",
        "--use-angle=swiftshader-webgl",
        "--disable-gpu",
        "--start-maximized",
        "--mute-audio",
        "--no-pings",
        "about:blank",
    ];
    assert_eq!(options.browser_args(Path::new("/tmp/profile")), expected);
    assert!(!options.is_headless());
    assert_eq!(options.url(), Some("https://example.com/"));
    assert_eq!(options.lang(), Some("ar-EG"));
    assert_eq!(options.user_agent(), Some("Agent/1"));
    assert!(options.proxy().is_some_and(|proxy| proxy.has_credentials()));
    assert!(!format!("{options:?}").contains("secret"));
}

#[tokio::test]
async fn a_config_with_mobile_set_makes_every_tab_a_phone() {
    let mut config = BrowserConfig::default().with_mobile(true);
    config.launch.device_metrics = Some(DeviceMetrics::new(360, 780, 2.0));
    let options = LaunchOptions::builder()
        .browser_config(&config)
        .build()
        .unwrap();
    let (_browser, mock) = launched(options).await;

    assert_eq!(
        mock.calls_to("Emulation.setDeviceMetricsOverride")[0].params,
        json!({ "width": 360, "height": 780, "deviceScaleFactor": 2.0, "mobile": true })
    );
    assert_eq!(
        mock.calls_to("Emulation.setUserAgentOverride")[0].params["userAgent"],
        json!(MOBILE_USER_AGENT)
    );
}

#[test]
fn a_config_with_a_fingerprint_and_mobile_is_refused() {
    let config = BrowserConfig {
        mobile: true,
        fingerprint: Some(Fingerprint::randomized(OsType::Linux, 3)),
        ..BrowserConfig::default()
    };
    let error = LaunchOptions::builder()
        .browser_config(&config)
        .build()
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("mobile emulation and a fingerprint"),
        "{error}"
    );
}

#[test]
fn a_config_proxy_is_checked_as_python_checks_it() {
    for bad in [
        "ftp://proxy.example:21",
        "host:notaport",
        "user:hunter2@:80",
    ] {
        let config = BrowserConfig {
            proxy: Some(bad.to_owned()),
            ..BrowserConfig::default()
        };
        let error = LaunchOptions::builder()
            .browser_config(&config)
            .build()
            .unwrap_err();
        assert!(!error.to_string().contains("hunter2"), "{error}");
    }
    // A SOCKS proxy is written the way Chrome reads it.
    let config = BrowserConfig {
        proxy: Some("socks5h://10.0.0.1:1080".to_owned()),
        ..BrowserConfig::default()
    };
    let arguments = LaunchOptions::builder()
        .browser_config(&config)
        .build()
        .unwrap()
        .browser_args(Path::new("/tmp/profile"));
    assert!(arguments.contains(&"--proxy-server=socks5://10.0.0.1:1080".to_owned()));
}

#[test]
fn a_config_switch_that_is_a_profile_preference_reaches_the_profile() {
    // The preferences are written into the profile before Chrome starts; the
    // real-Chrome test reads them back.
    let mut config = BrowserConfig::default();
    config.launch.set(LaunchSwitch::BlockImages, true);
    let builder = LaunchOptions::builder().browser_config(&config);
    assert!(format!("{builder:?}").contains("BlockImages"));
}
