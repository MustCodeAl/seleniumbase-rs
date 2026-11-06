//! The one rule for the settings of a launch, through the public API and the arguments Chrome
//! would start with, with no Chrome:
//!
//! > A value set on the builder wins; else the value of the `BrowserConfig`; else the
//! > fingerprint's; else the default.
//!
//! Each setting that more than one source can give is checked with every source alone, every
//! pair and all three, and every order of the calls. Run them with
//! `cargo test --features test-util --test offline sb_cdp_launch_layers`.

#![cfg(feature = "test-util")]

use std::path::Path;

use seleniumbase_rs::browser::options::{DeviceMetrics, WindowSize};
use seleniumbase_rs::evasion::fingerprint::{FingerprintBuilder, ProxyConfig};
use seleniumbase_rs::sb_cdp::{LaunchOptions, LaunchOptionsBuilder};
use seleniumbase_rs::{BrowserConfig, Fingerprint, OsType};

/// A place a setting can come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Builder,
    Config,
    Fingerprint,
}

use Source::{Builder, Config, Fingerprint as Print};

/// One setting, how each source gives it, and the argument Chrome gets for it.
struct Setting {
    /// The beginning of the argument (`--lang=`).
    flag: &'static str,
    on_builder: fn(LaunchOptionsBuilder) -> LaunchOptionsBuilder,
    on_config: fn(&mut BrowserConfig),
    on_fingerprint: fn(FingerprintBuilder) -> FingerprintBuilder,
    /// What each source gives, in the order builder, config, fingerprint.
    values: [&'static str; 3],
}

fn setting_user_agent() -> Setting {
    Setting {
        flag: "--user-agent=",
        on_builder: |builder| builder.user_agent("Builder/1"),
        on_config: |config| config.user_agent = Some("Config/1".to_owned()),
        on_fingerprint: |print| print.user_agent("Fingerprint/1".to_owned()),
        values: ["Builder/1", "Config/1", "Fingerprint/1"],
    }
}

fn setting_lang() -> Setting {
    Setting {
        flag: "--lang=",
        on_builder: |builder| builder.lang("de-DE"),
        on_config: |config| config.locale = Some("fr_FR".to_owned()),
        on_fingerprint: |print| print.locale("es-ES".to_owned()),
        // The config's locale is written the way Chrome reads it.
        values: ["de-DE", "fr-FR", "es-ES"],
    }
}

fn setting_window_size() -> Setting {
    Setting {
        flag: "--window-size=",
        on_builder: |builder| builder.window_size(801, 601),
        on_config: |config| config.launch.window_size = Some(WindowSize::new(802, 602)),
        on_fingerprint: |print| print.screen(803, 603),
        values: ["801,601", "802,602", "803,603"],
    }
}

fn setting_proxy() -> Setting {
    Setting {
        flag: "--proxy-server=",
        on_builder: |builder| builder.proxy("builder.example:8001"),
        on_config: |config| config.proxy = Some("config.example:8002".to_owned()),
        on_fingerprint: |print| {
            print.proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "print.example".to_owned(),
                port: 8003,
                ..ProxyConfig::default()
            })
        },
        values: [
            "builder.example:8001",
            "config.example:8002",
            "http://print.example:8003",
        ],
    }
}

/// The arguments of a launch whose `setting` comes from `sources`, called in that order.
fn arguments(setting: &Setting, sources: &[Source]) -> Vec<String> {
    let mut config = BrowserConfig::default();
    (setting.on_config)(&mut config);
    let fingerprint =
        (setting.on_fingerprint)(Fingerprint::randomized(OsType::Linux, 11).to_builder())
            .build()
            .unwrap();
    let mut builder = LaunchOptions::builder().no_sandbox(false);
    for source in sources {
        builder = match source {
            Builder => (setting.on_builder)(builder),
            Config => builder.browser_config(&config),
            Print => builder.fingerprint(&fingerprint),
        };
    }
    builder
        .build()
        .unwrap()
        .browser_args(Path::new("/tmp/profile"))
}

/// The value of the argument that starts with `flag`, if there is one.
fn value_of(arguments: &[String], flag: &str) -> Option<String> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix(flag))
        .map(str::to_owned)
}

/// Every arrangement of the sources: each non-empty subset, in each order.
fn arrangements() -> Vec<Vec<Source>> {
    let all = [Builder, Config, Print];
    let mut found = Vec::new();
    for first in all {
        found.push(vec![first]);
        for second in all.into_iter().filter(|source| *source != first) {
            found.push(vec![first, second]);
            for third in all
                .into_iter()
                .filter(|source| *source != first && *source != second)
            {
                found.push(vec![first, second, third]);
            }
        }
    }
    found
}

fn check_the_rule(setting: &Setting) {
    // Nothing sets it: Chrome gets no argument for it.
    assert_eq!(
        value_of(&arguments(setting, &[]), setting.flag),
        None,
        "{}",
        setting.flag
    );
    let [builder, config, print] = setting.values;
    for sources in arrangements() {
        let expected = if sources.contains(&Builder) {
            builder
        } else if sources.contains(&Config) {
            config
        } else {
            print
        };
        let found = value_of(&arguments(setting, &sources), setting.flag);
        assert_eq!(
            found.as_deref(),
            Some(expected),
            "{} from {sources:?}",
            setting.flag
        );
    }
}

#[test]
fn the_user_agent_follows_the_rule_in_every_order() {
    check_the_rule(&setting_user_agent());
}

#[test]
fn the_language_follows_the_rule_in_every_order() {
    check_the_rule(&setting_lang());
}

#[test]
fn the_window_size_follows_the_rule_in_every_order() {
    check_the_rule(&setting_window_size());
}

#[test]
fn the_proxy_follows_the_rule_in_every_order() {
    check_the_rule(&setting_proxy());
}

fn headless(builder: LaunchOptionsBuilder) -> bool {
    builder.build().unwrap().is_headless()
}

#[test]
fn headless_is_the_builders_then_the_configs_in_either_order() {
    let headed_config = BrowserConfig::default().with_headless(false);
    let headless_config = BrowserConfig::default().with_headless(true);
    for (config, own, expected) in [
        (&headed_config, Some(true), true),
        (&headless_config, Some(false), false),
        (&headed_config, None, false),
        (&headless_config, None, true),
    ] {
        let set = |builder: LaunchOptionsBuilder| match own {
            Some(value) => builder.headless(value),
            None => builder,
        };
        assert_eq!(
            headless(set(LaunchOptions::builder()).browser_config(config)),
            expected,
            "the builder first, own {own:?}"
        );
        assert_eq!(
            headless(set(LaunchOptions::builder().browser_config(config))),
            expected,
            "the config first, own {own:?}"
        );
    }
}

#[test]
fn what_the_builder_sets_before_a_config_is_no_longer_replaced_by_it() {
    // Before the one rule, the config replaced the user agent set before it.
    let config = BrowserConfig {
        user_agent: Some("Config/1".to_owned()),
        ..BrowserConfig::default()
    };
    let options = LaunchOptions::builder()
        .user_agent("Builder/1")
        .browser_config(&config)
        .build()
        .unwrap();
    assert_eq!(options.user_agent(), Some("Builder/1"));
}

#[test]
fn the_lists_of_the_config_come_before_the_builders() {
    let config = BrowserConfig {
        extra_args: vec!["first-flag".to_owned()],
        ..BrowserConfig::default()
    };
    for options in [
        LaunchOptions::builder()
            .arg("--second-flag")
            .browser_config(&config),
        LaunchOptions::builder()
            .browser_config(&config)
            .arg("--second-flag"),
    ] {
        let arguments = options
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"));
        let first = arguments.iter().position(|a| a == "--first-flag").unwrap();
        let second = arguments.iter().position(|a| a == "--second-flag").unwrap();
        assert!(first < second, "{arguments:?}");
    }
}

#[test]
fn a_switch_turned_off_on_the_builder_beats_the_config_in_either_order() {
    let mut config = BrowserConfig::default();
    config.launch.set(
        seleniumbase_rs::browser::options::LaunchSwitch::DarkMode,
        true,
    );
    let dark = "--enable-features=WebContentsForceDark".to_owned();
    for builder in [
        LaunchOptions::builder()
            .dark_mode(false)
            .browser_config(&config),
        LaunchOptions::builder()
            .browser_config(&config)
            .dark_mode(false),
    ] {
        let arguments = builder
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"));
        assert!(!arguments.contains(&dark), "{arguments:?}");
    }
    let from_config = LaunchOptions::builder()
        .browser_config(&config)
        .build()
        .unwrap()
        .browser_args(Path::new("/tmp/profile"));
    assert!(from_config.contains(&dark), "{from_config:?}");
}

#[test]
fn mobile_false_turns_the_phone_off_whatever_else_asks_for_one() {
    let phone = DeviceMetrics::new(360, 640, 2.0);
    let config = BrowserConfig::default().with_mobile(true);
    let sends_a_phone_agent = |builder: LaunchOptionsBuilder| {
        builder
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"))
            .iter()
            .any(|argument| argument.starts_with("--user-agent="))
    };
    // Each of these would make a phone, which sends its own user agent.
    assert!(sends_a_phone_agent(LaunchOptions::builder().mobile(true)));
    assert!(sends_a_phone_agent(
        LaunchOptions::builder().browser_config(&config)
    ));
    // And with `mobile(false)` before or after, none does.
    for builder in [
        LaunchOptions::builder().mobile(false).device_metrics(phone),
        LaunchOptions::builder().device_metrics(phone).mobile(false),
        LaunchOptions::builder()
            .mobile(false)
            .browser_config(&config),
        LaunchOptions::builder()
            .browser_config(&config)
            .mobile(false),
    ] {
        assert!(!sends_a_phone_agent(builder));
    }
}

fn print(seed: u64, user_agent: &str, lang: &str, screen: (u32, u32), proxy: &str) -> Fingerprint {
    Fingerprint::randomized(OsType::Linux, seed)
        .to_builder()
        .user_agent(user_agent.to_owned())
        .locale(lang.to_owned())
        .screen(screen.0, screen.1)
        .proxy(ProxyConfig {
            r#type: "http".to_owned(),
            host: proxy.to_owned(),
            port: 8080,
            ..ProxyConfig::default()
        })
        .build()
        .unwrap()
}

fn browser_args(options: &LaunchOptions) -> Vec<String> {
    options.browser_args(Path::new("/tmp/profile"))
}

#[test]
fn options_made_again_with_a_fingerprint_are_what_the_builder_gives() {
    let fingerprint = print(1, "First/1", "fr-FR", (801, 601), "first.example");
    let bases = [
        LaunchOptions::builder().no_sandbox(false),
        LaunchOptions::builder()
            .no_sandbox(false)
            .user_agent("Mine/1")
            .arg("--mute-audio")
            .url("https://example.com/"),
        LaunchOptions::builder()
            .no_sandbox(false)
            .browser_config(&BrowserConfig::default().with_headless(false)),
    ];
    for base in bases {
        let direct = base.clone().fingerprint(&fingerprint).build().unwrap();
        let again = base
            .build()
            .unwrap()
            .with_fingerprint(&fingerprint)
            .unwrap();
        assert_eq!(browser_args(&again), browser_args(&direct));
        assert_eq!(again.fingerprint(), direct.fingerprint());
        assert_eq!(again.url(), direct.url());
        assert_eq!(again.user_agent(), direct.user_agent());
        assert_eq!(again.lang(), direct.lang());
        assert_eq!(again.is_headless(), direct.is_headless());
    }
}

#[test]
fn a_second_fingerprint_replaces_the_first_and_nothing_else() {
    let first = print(1, "First/1", "fr-FR", (801, 601), "first.example");
    let second = print(2, "Second/2", "de-DE", (802, 602), "second.example");
    let options = LaunchOptions::builder()
        .no_sandbox(false)
        .window_size(640, 480)
        .build()
        .unwrap();

    let rotated = options
        .clone()
        .with_fingerprint(&first)
        .unwrap()
        .with_fingerprint(&second)
        .unwrap();
    let fresh = options.with_fingerprint(&second).unwrap();
    assert_eq!(browser_args(&rotated), browser_args(&fresh));
    assert_eq!(rotated.fingerprint(), Some(&second));
    assert_eq!(rotated.user_agent(), Some("Second/2"));
    assert_eq!(rotated.lang(), Some("de-DE"));
    assert!(
        browser_args(&rotated)
            .iter()
            .all(|argument| !argument.contains("first.example") && !argument.contains("First/1")),
        "nothing of the first fingerprint is left"
    );
    // The window size the options were built with beats the fingerprint's, again and again.
    assert!(browser_args(&rotated).contains(&"--window-size=640,480".to_owned()));
}

#[test]
fn the_other_setters_of_built_options_make_them_again() {
    let options = LaunchOptions::builder()
        .no_sandbox(false)
        .headless(false)
        .arg("--first")
        .build()
        .unwrap();

    let changed = options
        .clone()
        .with_headless(true)
        .unwrap()
        .with_proxy("alice:s3cret@proxy.example:8080")
        .unwrap()
        .with_args(["--second", "--third"])
        .unwrap()
        .with_url("https://example.com/")
        .unwrap();
    let direct = LaunchOptions::builder()
        .no_sandbox(false)
        .headless(true)
        .proxy("alice:s3cret@proxy.example:8080")
        .arg("--first")
        .arg("--second")
        .arg("--third")
        .url("https://example.com/")
        .build()
        .unwrap();
    assert_eq!(browser_args(&changed), browser_args(&direct));
    assert!(changed.is_headless());
    assert_eq!(changed.url(), Some("https://example.com/"));
    assert!(changed.proxy().is_some_and(|proxy| proxy.has_credentials()));
    assert!(!format!("{changed:?}").contains("s3cret"));
    // The options they came from are not changed.
    assert!(!options.is_headless());
    assert_eq!(options.url(), None);
}

#[test]
fn made_again_the_combination_is_checked_again() {
    let attached = LaunchOptions::builder()
        .cdp_url("http://127.0.0.1:9222")
        .build()
        .unwrap();
    // A browser that is only attached to cannot be given a proxy.
    assert!(attached.clone().with_proxy("proxy.example:8080").is_err());
    assert!(attached.clone().with_url("https://example.com/").is_ok());

    let phone = LaunchOptions::builder().mobile(true).build().unwrap();
    let fingerprint = Fingerprint::randomized(OsType::Linux, 5);
    assert!(phone.with_fingerprint(&fingerprint).is_err());
}
