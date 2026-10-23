//! On/off launch settings, and the browser preferences some of them set.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// A setting that is either on or off for a browser session.
///
/// Each one is a Python SeleniumBase option that takes no value, such as
/// `--block-images`. Turn them on with the matching `BrowserConfig::with_*`
/// method; [`LaunchSettings`](super::LaunchSettings) keeps the set that is on.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::LaunchSwitch;
///
/// // The Python option names, with dashes or underscores, name the switches.
/// assert_eq!(
///     LaunchSwitch::from_python_name("--block-images"),
///     Some(LaunchSwitch::BlockImages)
/// );
/// assert_eq!(LaunchSwitch::from_python_name("no_csp"), Some(LaunchSwitch::DisableCsp));
/// assert_eq!(LaunchSwitch::from_python_name("headless"), None);
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LaunchSwitch {
    /// Do not load images (`--block-images`).
    BlockImages,
    /// Turn JavaScript off (`--disable-js`). Most pages break without it.
    DisableJavaScript,
    /// Refuse every cookie (`--disable-cookies`). Many sites break without them.
    DisableCookies,
    /// Send the `DNT: 1` header (`--do-not-track`).
    DoNotTrack,
    /// Download PDF files instead of showing them in the browser
    /// (`--external-pdf`).
    ExternalPdf,
    /// Leave Chrome Sync on (`--enable-sync`). Sessions switch it off otherwise.
    EnableSync,
    /// Strip the Content Security Policy from every response (`--disable-csp`),
    /// so a page that forbids outside scripts can still be given a test
    /// library. Loads a bundled extension; Chromium only.
    DisableCsp,
    /// Keep Chrome's automation extension (`--use-auto-ext`). Upstream marks
    /// this deprecated; Chromium only.
    AutomationExtension,
    /// Load the SeleniumBase Recorder extension (`recorder_ext`). Chromium
    /// only.
    RecorderExtension,
    /// Fetch the driver, and the browser when a build has to be downloaded,
    /// through the session's proxy (`--proxy-driver`).
    ProxyForDriverDownload,
    /// End the current test when the person presses ESC in the browser
    /// (`--esc-end`, `--ee`). Nothing about the launch changes; `BaseCase`
    /// reads the switch to install the key listener.
    EscEnd,
    /// Write the recording as pure CDP code (`--rec-sb-cdp`, `rec_sb_cdp`).
    /// Nothing about the launch changes; `BaseCase::export_recording_as_rust`
    /// reads the switch to choose the generator.
    RecSbCdp,
    /// Force Chrome's dark theme on web pages (`--dark`, `dark_mode`), through
    /// `--enable-features=WebContentsForceDark`. Chromium only.
    DarkMode,
    /// Open the browser in an incognito window (`--incognito`). Left out when
    /// an extension is loaded, which Chrome does not allow in that mode.
    /// Chromium and Firefox (`-private`).
    Incognito,
    /// Open the browser as a guest (`--guest`). Left out when an extension is
    /// loaded. Chromium only.
    Guest,
    /// Open the developer tools with each tab (`--devtools`). A headed
    /// session only; Chromium and Firefox.
    Devtools,
    /// Switch web security off (`--disable-ws`): cross-origin requests and
    /// certificate errors are allowed. Chromium only.
    DisableWebSecurity,
    /// Render with SwiftShader, a software WebGL renderer (`--swiftshader`).
    /// Chromium only.
    Swiftshader,
    /// Keep the GPU in a headless session, for pages that need WebGL
    /// (`--enable-3d-apis`). Chromium only.
    #[serde(rename = "enable-3d-apis")]
    Enable3dApis,
    /// Open the window maximized (`--maximize`). A headed session only.
    Maximize,
    /// Start Chromium without its sandbox (`--no-sandbox`). Linux sessions
    /// already do.
    NoSandbox,
    /// Switch the GPU off, headed sessions too (`--disable-gpu`). Chromium
    /// only.
    DisableGpu,
    /// Run Chrome headless with `chrome-headless-shell` (`--headless1`), the
    /// old headless implementation.
    Headless1,
}

impl LaunchSwitch {
    /// Every switch.
    pub const ALL: [Self; 23] = [
        Self::BlockImages,
        Self::DisableJavaScript,
        Self::DisableCookies,
        Self::DoNotTrack,
        Self::ExternalPdf,
        Self::EnableSync,
        Self::DisableCsp,
        Self::AutomationExtension,
        Self::RecorderExtension,
        Self::ProxyForDriverDownload,
        Self::EscEnd,
        Self::RecSbCdp,
        Self::DarkMode,
        Self::Incognito,
        Self::Guest,
        Self::Devtools,
        Self::DisableWebSecurity,
        Self::Swiftshader,
        Self::Enable3dApis,
        Self::Maximize,
        Self::NoSandbox,
        Self::DisableGpu,
        Self::Headless1,
    ];

    /// The switch a Python SeleniumBase option name stands for, if any.
    ///
    /// Leading dashes are ignored, dashes and underscores are the same, and the
    /// aliases the Python command line accepts (`--dcsp`, `--no-csp`,
    /// `--auto-ext`, `--proxy_driver`, ...) are known.
    #[must_use]
    pub fn from_python_name(name: &str) -> Option<Self> {
        let name = name.trim_start_matches('-').replace('-', "_");
        Some(match name.as_str() {
            "block_images" => Self::BlockImages,
            "disable_js" => Self::DisableJavaScript,
            "disable_cookies" => Self::DisableCookies,
            "do_not_track" => Self::DoNotTrack,
            "external_pdf" => Self::ExternalPdf,
            "enable_sync" => Self::EnableSync,
            "disable_csp" | "dcsp" | "no_csp" => Self::DisableCsp,
            "use_auto_ext" | "auto_ext" => Self::AutomationExtension,
            "recorder_ext" => Self::RecorderExtension,
            "proxy_driver" => Self::ProxyForDriverDownload,
            "esc_end" | "ee" => Self::EscEnd,
            "rec_sb_cdp" => Self::RecSbCdp,
            "dark" | "dark_mode" => Self::DarkMode,
            "incognito" | "incognito_mode" => Self::Incognito,
            "guest" | "guest_mode" => Self::Guest,
            "devtools" | "open_devtools" => Self::Devtools,
            "disable_ws" | "dws" | "disable_web_security" => Self::DisableWebSecurity,
            "swiftshader" => Self::Swiftshader,
            "enable_3d_apis" => Self::Enable3dApis,
            "maximize" | "maximize_window" | "fullscreen" => Self::Maximize,
            "no_sandbox" => Self::NoSandbox,
            "disable_gpu" => Self::DisableGpu,
            "headless1" => Self::Headless1,
            _ => return None,
        })
    }
}

/// The Chrome profile preferences that `switches` ask for, by dotted name.
///
/// Chromedriver accepts these dotted names in the `prefs` of
/// `goog:chromeOptions` and writes them into the profile's `Preferences` file.
/// Use [`nested_preferences`] for the form that file holds.
#[must_use]
pub fn chrome_preferences<'a>(
    switches: impl IntoIterator<Item = &'a LaunchSwitch>,
) -> Map<String, Value> {
    let mut preferences = Map::new();
    for switch in switches {
        // 2 is Chrome's "block" in a content-setting preference.
        match switch {
            LaunchSwitch::BlockImages => {
                preferences.insert(
                    "profile.managed_default_content_settings.images".into(),
                    json!(2),
                );
            }
            LaunchSwitch::DisableJavaScript => {
                preferences.insert(
                    "profile.managed_default_content_settings.javascript".into(),
                    json!(2),
                );
            }
            LaunchSwitch::DisableCookies => {
                preferences.insert(
                    "profile.default_content_setting_values.cookies".into(),
                    json!(2),
                );
            }
            LaunchSwitch::DoNotTrack => {
                preferences.insert("enable_do_not_track".into(), json!(true));
            }
            LaunchSwitch::ExternalPdf => {
                preferences.insert("plugins.always_open_pdf_externally".into(), json!(true));
            }
            _ => {}
        }
    }
    preferences
}

/// The Firefox preferences that `switches` ask for.
#[must_use]
pub fn firefox_preferences<'a>(
    switches: impl IntoIterator<Item = &'a LaunchSwitch>,
) -> Map<String, Value> {
    let mut preferences = Map::new();
    for switch in switches {
        match switch {
            LaunchSwitch::BlockImages => {
                // 2 is Firefox's "block" in a permission preference.
                preferences.insert("permissions.default.image".into(), json!(2));
            }
            LaunchSwitch::DisableJavaScript => {
                preferences.insert("javascript.enabled".into(), json!(false));
            }
            LaunchSwitch::DisableCookies => {
                // 2 rejects all cookies.
                preferences.insert("network.cookie.cookieBehavior".into(), json!(2));
            }
            LaunchSwitch::DoNotTrack => {
                preferences.insert("privacy.donottrackheader.enabled".into(), json!(true));
            }
            LaunchSwitch::ExternalPdf => {
                preferences.insert("pdfjs.disabled".into(), json!(true));
            }
            LaunchSwitch::DisableCsp => {
                // Python's `security.csp.enable` for Firefox.
                preferences.insert("security.csp.enable".into(), json!(false));
            }
            _ => {}
        }
    }
    preferences
}

/// Turns dotted preference names into the nested object a Chrome profile's
/// `Preferences` file holds: `{"a.b": 1}` becomes `{"a": {"b": 1}}`.
///
/// Preferences that share a prefix end up under one object.
#[must_use]
pub fn nested_preferences(dotted: &Map<String, Value>) -> Value {
    let mut root = Map::new();
    for (name, value) in dotted {
        let mut segments: Vec<&str> = name.split('.').collect();
        let leaf = segments.pop().unwrap_or(name);
        let mut node = &mut root;
        for segment in segments {
            let child = node
                .entry(segment)
                .or_insert_with(|| Value::Object(Map::new()));
            if !child.is_object() {
                *child = Value::Object(Map::new());
            }
            let Value::Object(next) = child else {
                unreachable!("the entry was just made an object");
            };
            node = next;
        }
        node.insert(leaf.to_owned(), value.clone());
    }
    Value::Object(root)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn one(switch: LaunchSwitch) -> BTreeSet<LaunchSwitch> {
        BTreeSet::from([switch])
    }

    #[test]
    fn the_chrome_preferences_are_the_ones_python_sets() {
        let cases = [
            (
                LaunchSwitch::BlockImages,
                "profile.managed_default_content_settings.images",
                json!(2),
            ),
            (
                LaunchSwitch::DisableJavaScript,
                "profile.managed_default_content_settings.javascript",
                json!(2),
            ),
            (
                LaunchSwitch::DisableCookies,
                "profile.default_content_setting_values.cookies",
                json!(2),
            ),
            (LaunchSwitch::DoNotTrack, "enable_do_not_track", json!(true)),
            (
                LaunchSwitch::ExternalPdf,
                "plugins.always_open_pdf_externally",
                json!(true),
            ),
        ];
        for (switch, name, value) in cases {
            let preferences = chrome_preferences(&one(switch));
            assert_eq!(preferences.len(), 1, "{switch:?}");
            assert_eq!(preferences[name], value, "{switch:?}");
        }
    }

    #[test]
    fn the_firefox_preferences_are_the_ones_python_sets() {
        let cases = [
            (
                LaunchSwitch::BlockImages,
                "permissions.default.image",
                json!(2),
            ),
            (
                LaunchSwitch::DisableJavaScript,
                "javascript.enabled",
                json!(false),
            ),
            (
                LaunchSwitch::DisableCookies,
                "network.cookie.cookieBehavior",
                json!(2),
            ),
            (
                LaunchSwitch::DoNotTrack,
                "privacy.donottrackheader.enabled",
                json!(true),
            ),
            (LaunchSwitch::ExternalPdf, "pdfjs.disabled", json!(true)),
            (
                LaunchSwitch::DisableCsp,
                "security.csp.enable",
                json!(false),
            ),
        ];
        for (switch, name, value) in cases {
            let preferences = firefox_preferences(&one(switch));
            assert_eq!(preferences.len(), 1, "{switch:?}");
            assert_eq!(preferences[name], value, "{switch:?}");
        }
    }

    #[test]
    fn switches_that_are_not_preferences_set_none() {
        let others: BTreeSet<_> = [
            LaunchSwitch::EnableSync,
            LaunchSwitch::DarkMode,
            LaunchSwitch::Incognito,
            LaunchSwitch::Guest,
            LaunchSwitch::Devtools,
            LaunchSwitch::DisableWebSecurity,
            LaunchSwitch::Swiftshader,
            LaunchSwitch::Enable3dApis,
            LaunchSwitch::Maximize,
            LaunchSwitch::NoSandbox,
            LaunchSwitch::DisableGpu,
            LaunchSwitch::Headless1,
            LaunchSwitch::AutomationExtension,
            LaunchSwitch::RecorderExtension,
            LaunchSwitch::ProxyForDriverDownload,
            LaunchSwitch::EscEnd,
            LaunchSwitch::RecSbCdp,
        ]
        .into();
        assert!(chrome_preferences(&others).is_empty());
        assert!(firefox_preferences(&others).is_empty());
        assert!(chrome_preferences(&BTreeSet::new()).is_empty());
    }

    #[test]
    fn several_switches_set_several_preferences() {
        let set: BTreeSet<_> = LaunchSwitch::ALL.into();
        assert_eq!(chrome_preferences(&set).len(), 5);
        assert_eq!(firefox_preferences(&set).len(), 6);
    }

    #[test]
    fn dotted_names_nest_and_share_their_prefix() {
        let dotted = chrome_preferences(&BTreeSet::from([
            LaunchSwitch::BlockImages,
            LaunchSwitch::DisableJavaScript,
            LaunchSwitch::DoNotTrack,
        ]));
        assert_eq!(
            nested_preferences(&dotted),
            json!({
                "enable_do_not_track": true,
                "profile": {
                    "managed_default_content_settings": { "images": 2, "javascript": 2 }
                }
            })
        );
    }

    #[test]
    fn a_name_that_collides_with_a_value_replaces_it() {
        let mut dotted = Map::new();
        dotted.insert("a".into(), json!(1));
        dotted.insert("a.b".into(), json!(2));
        // `serde_json::Map` is sorted, so "a" comes first and "a.b" then turns
        // the number into an object rather than failing.
        assert_eq!(nested_preferences(&dotted), json!({ "a": { "b": 2 } }));
    }

    #[test]
    fn python_option_names_and_aliases_name_the_switches() {
        let cases = [
            ("--block-images", LaunchSwitch::BlockImages),
            ("block_images", LaunchSwitch::BlockImages),
            ("--disable-js", LaunchSwitch::DisableJavaScript),
            ("--disable_cookies", LaunchSwitch::DisableCookies),
            ("--do-not-track", LaunchSwitch::DoNotTrack),
            ("--external-pdf", LaunchSwitch::ExternalPdf),
            ("--enable-sync", LaunchSwitch::EnableSync),
            ("--disable-csp", LaunchSwitch::DisableCsp),
            ("--dcsp", LaunchSwitch::DisableCsp),
            ("--no-csp", LaunchSwitch::DisableCsp),
            ("--use-auto-ext", LaunchSwitch::AutomationExtension),
            ("--auto-ext", LaunchSwitch::AutomationExtension),
            ("recorder_ext", LaunchSwitch::RecorderExtension),
            ("--proxy-driver", LaunchSwitch::ProxyForDriverDownload),
            ("--esc-end", LaunchSwitch::EscEnd),
            ("--esc_end", LaunchSwitch::EscEnd),
            ("--ee", LaunchSwitch::EscEnd),
            ("--rec-sb-cdp", LaunchSwitch::RecSbCdp),
            ("rec_sb_cdp", LaunchSwitch::RecSbCdp),
            ("--dark", LaunchSwitch::DarkMode),
            ("dark_mode", LaunchSwitch::DarkMode),
            ("--incognito", LaunchSwitch::Incognito),
            ("incognito_mode", LaunchSwitch::Incognito),
            ("--guest", LaunchSwitch::Guest),
            ("guest_mode", LaunchSwitch::Guest),
            ("--devtools", LaunchSwitch::Devtools),
            ("open_devtools", LaunchSwitch::Devtools),
            ("--disable-ws", LaunchSwitch::DisableWebSecurity),
            ("--dws", LaunchSwitch::DisableWebSecurity),
            ("disable_web_security", LaunchSwitch::DisableWebSecurity),
            ("--swiftshader", LaunchSwitch::Swiftshader),
            ("--enable-3d-apis", LaunchSwitch::Enable3dApis),
            ("enable_3d_apis", LaunchSwitch::Enable3dApis),
            ("--maximize", LaunchSwitch::Maximize),
            ("maximize_window", LaunchSwitch::Maximize),
            ("fullscreen", LaunchSwitch::Maximize),
            ("--no-sandbox", LaunchSwitch::NoSandbox),
            ("--disable-gpu", LaunchSwitch::DisableGpu),
            ("--headless1", LaunchSwitch::Headless1),
        ];
        for (name, switch) in cases {
            assert_eq!(LaunchSwitch::from_python_name(name), Some(switch), "{name}");
        }
        assert_eq!(LaunchSwitch::from_python_name("--headless"), None);
        // Every switch is reachable from some Python name.
        let reached: BTreeSet<_> = cases.iter().map(|(_, switch)| *switch).collect();
        assert_eq!(reached, BTreeSet::from(LaunchSwitch::ALL));
    }

    #[test]
    fn switches_are_written_in_kebab_case() {
        let set = BTreeSet::from([LaunchSwitch::DisableJavaScript, LaunchSwitch::BlockImages]);
        let text = serde_json::to_string(&set).unwrap();
        assert_eq!(text, r#"["block-images","disable-java-script"]"#);
        assert_eq!(
            serde_json::from_str::<BTreeSet<LaunchSwitch>>(&text).unwrap(),
            set
        );
    }
}
