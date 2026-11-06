//! Typed readers for the text of an option.
//!
//! An `SB_*` variable, a flag of `sbrs` and a flat key of the settings file all give the
//! value of an option as text. This module turns that text into the typed field of
//! [`Settings`](crate::Settings) the option table names (`browser.window_size`,
//! `session.grid.port`, ...), so that a value is refused in one place, when it is read, and
//! says which variable it came from.
//!
//! [`OptionValue`] reads the options whose text is the value of one field. The functions
//! below it read the options that are derived: several of them write one field
//! (`xvfb` and `xvfb_metrics` make `browser.virtual_display`), or one of them writes two
//! (`chromium_arg` fills `browser.chromium.args` and `browser.chromium.disable_features`).
//! Each takes the text of one variable and writes the field; within one source the rows of
//! the option table run in table order, and a higher source runs after a lower one, so the
//! higher source wins.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::browser::config::{split_extension_dirs, Browser, DriverMode};
use crate::browser::options::{
    BrowserSettings, CapabilityOverrides, CdpEventCapture, DeviceMetrics, DriverVersion,
    ExtensionArchive, FirefoxOptions, GridProtocol, LocaleCode, PageLoadStrategy, ProxyBypassList,
    SessionSettings, VirtualDisplay, WindowPosition, WindowSize,
};
use crate::browser::reuse::ReuseClass;
use crate::config::settings::parse_bool;
use crate::error::SeleniumBaseError;

/// How an option's text is written into a [`Settings`](crate::Settings) field.
pub(crate) trait OptionValue {
    /// Reads `text`, the value of the variable `name`.
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError>;
}

impl OptionValue for bool {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = parse_bool(text)?;
        Ok(())
    }
}

impl OptionValue for String {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        text.clone_into(self);
        Ok(())
    }
}

impl OptionValue for Option<String> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(text.to_owned());
        Ok(())
    }
}

impl OptionValue for Option<PathBuf> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(PathBuf::from(text));
        Ok(())
    }
}

/// The folders of `SB_EXTENSION_DIR`, separated by commas.
impl OptionValue for Vec<PathBuf> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = split_extension_dirs(text);
        Ok(())
    }
}

/// The archives of `SB_EXTENSION_ZIP`, separated by commas.
impl OptionValue for Vec<ExtensionArchive> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = ExtensionArchive::parse_list(text)?;
        Ok(())
    }
}

fn parse_number<T>(name: &str, text: &str) -> Result<T, SeleniumBaseError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    text.parse()
        .map_err(|error| SeleniumBaseError::InvalidConfig(format!("{name}: {error}")))
}

fn parse_seconds(name: &str, text: &str) -> Result<f64, SeleniumBaseError> {
    match text.trim().parse::<f64>() {
        Ok(seconds) if seconds.is_finite() && seconds >= 0.0 => Ok(seconds),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "{name}: expected a number of seconds, 0 or more, not '{text}'"
        ))),
    }
}

macro_rules! number_values {
    ($($t:ty),+ $(,)?) => {$(
        impl OptionValue for Option<$t> {
            fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
                *self = Some(parse_number(name, text.trim())?);
                Ok(())
            }
        }
    )+};
}

number_values!(u16, u32, u64, usize);

impl OptionValue for u32 {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = parse_number(name, text.trim())?;
        Ok(())
    }
}

impl OptionValue for f64 {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = parse_seconds(name, text)?;
        Ok(())
    }
}

impl OptionValue for Option<f64> {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(parse_seconds(name, text)?);
        Ok(())
    }
}

impl OptionValue for Option<Duration> {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let seconds = parse_seconds(name, text)?;
        *self = Some(Duration::try_from_secs_f64(seconds).map_err(|_| {
            SeleniumBaseError::InvalidConfig(format!(
                "{name}: expected a number of seconds, 0 or more, not '{text}'"
            ))
        })?);
        Ok(())
    }
}

/// Fields whose text is the name or the notation of a typed value: the type reads it with
/// [`FromStr`](std::str::FromStr), as the Python command line writes it (`"1280,720"`,
/// `"eager"`, `"firefox"`).
macro_rules! from_text {
    ($($t:ty),+ $(,)?) => {$(
        impl OptionValue for $t {
            fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
                *self = text.parse::<$t>()?;
                Ok(())
            }
        }

        impl OptionValue for Option<$t> {
            fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
                *self = Some(text.parse::<$t>()?);
                Ok(())
            }
        }
    )+};
}

from_text!(
    Browser,
    DriverMode,
    DeviceMetrics,
    DriverVersion,
    GridProtocol,
    PageLoadStrategy,
    ProxyBypassList,
    WindowPosition,
    WindowSize,
);

/// The items of a comma-separated list, trimmed, without the empty ones.
pub(crate) fn list(text: &str) -> impl Iterator<Item = &str> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

/// `SB_LOCALE_CODE`: a language code such as `fr` or `ar_eg`, written the way Chrome reads it
/// (`ar-EG`). It is the browser's language, as `SB_LOCALE` is.
pub(crate) fn locale_code(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    browser.lang = Some(text.parse::<LocaleCode>()?.as_str().to_owned());
    Ok(())
}

/// `SB_XVFB`: on opens a virtual display of the size already asked for, else of the default
/// size; off closes it. The size comes from `SB_XVFB_METRICS` ([`xvfb_metrics`]), which the
/// table reads after this variable.
pub(crate) fn xvfb(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    if !parse_bool(text)? {
        browser.virtual_display = None;
    } else if browser.virtual_display.is_none() {
        browser.virtual_display = Some(VirtualDisplay::default());
    }
    Ok(())
}

/// `SB_XVFB_METRICS`: the size of the virtual display. It is checked always and used when the
/// display is on; a size with no display asked for opens none.
pub(crate) fn xvfb_metrics(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let display = text.parse::<VirtualDisplay>()?;
    if browser.virtual_display.is_some() {
        browser.virtual_display = Some(display);
    }
    Ok(())
}

/// `SB_REMOTE_DEBUG`: on opens Chrome's remote debugger on the default port, or on another
/// one when that one is busy; off closes it.
pub(crate) fn remote_debug(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let on = parse_bool(text)?;
    browser.debugging_port = on.then_some(crate::evasion::options::DEFAULT_REMOTE_DEBUG_PORT);
    browser.debugging_any_port = on;
    Ok(())
}

/// Adds the items of a `chromium_arg` list to `browser`: each item is one switch, written with
/// its leading `--`, and a `disable-features=` item joins the features to disable. The
/// switches replace the earlier ones; the features are added to the earlier ones.
///
/// An item is not split at commas here, which is what the array form of the settings file is
/// for; [`chromium_arg`] splits the text of a variable.
pub(crate) fn add_chromium_items(browser: &mut BrowserSettings, items: &[String]) {
    browser.chromium.args.clear();
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
            browser.chromium.args.push(format!("--{switch}"));
        }
    }
    add_disable_features(browser, features);
}

/// `SB_CHROMIUM_ARG`: extra Chromium switches, separated by commas.
pub(crate) fn chromium_arg(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let items = crate::config::browser_options::ArgList::from_text(text);
    add_chromium_items(browser, items.items());
    Ok(())
}

/// Adds `features` to the Chromium features to switch off, once each.
fn add_disable_features(browser: &mut BrowserSettings, features: Vec<String>) {
    for feature in features {
        if !browser.chromium.disable_features.contains(&feature) {
            browser.chromium.disable_features.push(feature);
        }
    }
}

/// `SB_DISABLE_FEATURES`: Chromium features to switch off, separated by commas.
pub(crate) fn disable_features(
    browser: &mut BrowserSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    add_disable_features(browser, list(text).map(str::to_owned).collect());
    Ok(())
}

/// `SB_REUSE_CLASS_SESSION`: on shares a browser among the tests of one Rust module.
pub(crate) fn reuse_class(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    session.reuse_class = parse_bool(text)?.then_some(ReuseClass::TestModule);
    Ok(())
}

/// `SB_CAP_FILE`: the capabilities in a JSON file, laid over the ones already asked for.
pub(crate) fn cap_file(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let capabilities = CapabilityOverrides::from_file(Path::new(text))?;
    session.capabilities.merge(&capabilities);
    Ok(())
}

/// `SB_CAP_STRING`: the capabilities in a JSON object, laid over the ones already asked for
/// (the table reads it after the file, so it wins over it within one source).
pub(crate) fn cap_string(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    session
        .capabilities
        .merge(&CapabilityOverrides::from_text(text)?);
    Ok(())
}

/// `SB_FIREFOX_ARG`: Firefox arguments, separated by commas. They replace the earlier
/// arguments and keep the preferences.
pub(crate) fn firefox_arg(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let mut firefox = FirefoxOptions::default();
    firefox.add_args(text)?;
    for (name, value) in session.firefox.preferences() {
        firefox.set_preference(name.clone(), value.clone());
    }
    session.firefox = firefox;
    Ok(())
}

/// `SB_FIREFOX_PREF`: Firefox preferences as `NAME:VALUE`, separated by commas. They replace
/// the earlier preferences and keep the arguments.
pub(crate) fn firefox_pref(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let mut firefox = FirefoxOptions::default();
    for arg in session.firefox.args() {
        firefox.add_arg(arg)?;
    }
    firefox.add_preferences(text)?;
    session.firefox = firefox;
    Ok(())
}

/// `SB_LOG_CDP_EVENTS`: log the DevTools Protocol events in every Chromium session. Off turns
/// that off and leaves the undetected-only capture as it was.
pub(crate) fn log_cdp_events(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    if parse_bool(text)? {
        session.cdp_events = CdpEventCapture::Always;
    } else if matches!(session.cdp_events, CdpEventCapture::Always) {
        session.cdp_events = CdpEventCapture::Off;
    }
    Ok(())
}

/// `SB_UC_CDP_EVENTS`: log the DevTools Protocol events in undetected mode only. It never
/// lowers the capture of every session (`SB_LOG_CDP_EVENTS`).
pub(crate) fn uc_cdp_events(
    session: &mut SessionSettings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    let on = parse_bool(text)?;
    match session.cdp_events {
        CdpEventCapture::Always => {}
        _ if on => session.cdp_events = CdpEventCapture::UndetectedOnly,
        CdpEventCapture::UndetectedOnly => session.cdp_events = CdpEventCapture::Off,
        _ => {}
    }
    Ok(())
}

/// The text of a value of the settings file, for an option the file writes as a flat key: a
/// string as it is, a number or a boolean as it prints. `None` for any other value (a table,
/// an array, a null).
pub(crate) fn text_of(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_or_a_notation_is_read_into_its_type() {
        let mut browser = Browser::default();
        browser.assign("SB_BROWSER", "Firefox").unwrap();
        assert_eq!(browser, Browser::Firefox);
        let mut size: Option<WindowSize> = None;
        size.assign("SB_WINDOW_SIZE", "1280,720").unwrap();
        assert_eq!(size, Some(WindowSize::new(1280, 720)));
        assert!(size.assign("SB_WINDOW_SIZE", "0,720").is_err());
        let mut mode = DriverMode::default();
        assert!(mode.assign("SB_MODE", "stealth").is_err());
    }

    #[test]
    fn a_display_needs_its_switch_and_takes_the_metrics_after_it() {
        let mut browser = BrowserSettings::default();
        xvfb_metrics(&mut browser, "SB_XVFB_METRICS", "1920,1080").unwrap();
        assert_eq!(browser.virtual_display, None);
        xvfb(&mut browser, "SB_XVFB", "true").unwrap();
        assert_eq!(browser.virtual_display, Some(VirtualDisplay::default()));
        xvfb_metrics(&mut browser, "SB_XVFB_METRICS", "1920,1080").unwrap();
        assert_eq!(
            browser.virtual_display,
            Some(VirtualDisplay::new(1920, 1080))
        );
        xvfb(&mut browser, "SB_XVFB", "false").unwrap();
        assert_eq!(browser.virtual_display, None);
        assert!(xvfb_metrics(&mut browser, "SB_XVFB_METRICS", "wide").is_err());
    }

    #[test]
    fn the_chromium_items_keep_their_switches_and_send_the_features_apart() {
        let mut browser = BrowserSettings::default();
        chromium_arg(
            &mut browser,
            "SB_CHROMIUM_ARG",
            "--mute-audio,disable-features=Foo, lang=fr",
        )
        .unwrap();
        assert_eq!(browser.chromium.args, ["--mute-audio", "--lang=fr"]);
        assert_eq!(browser.chromium.disable_features, ["Foo"]);
        disable_features(&mut browser, "SB_DISABLE_FEATURES", "Foo,Bar").unwrap();
        assert_eq!(browser.chromium.disable_features, ["Foo", "Bar"]);
        // A later list of switches replaces the earlier one.
        chromium_arg(&mut browser, "SB_CHROMIUM_ARG", "--no-first-run").unwrap();
        assert_eq!(browser.chromium.args, ["--no-first-run"]);
    }

    #[test]
    fn the_cdp_events_keep_the_wider_capture() {
        let mut session = SessionSettings::default();
        uc_cdp_events(&mut session, "SB_UC_CDP_EVENTS", "true").unwrap();
        assert_eq!(session.cdp_events, CdpEventCapture::UndetectedOnly);
        log_cdp_events(&mut session, "SB_LOG_CDP_EVENTS", "true").unwrap();
        assert_eq!(session.cdp_events, CdpEventCapture::Always);
        uc_cdp_events(&mut session, "SB_UC_CDP_EVENTS", "true").unwrap();
        assert_eq!(session.cdp_events, CdpEventCapture::Always);
        log_cdp_events(&mut session, "SB_LOG_CDP_EVENTS", "false").unwrap();
        assert_eq!(session.cdp_events, CdpEventCapture::Off);
    }

    #[test]
    fn the_firefox_arguments_and_preferences_replace_their_own_kind_only() {
        let mut session = SessionSettings::default();
        firefox_arg(&mut session, "SB_FIREFOX_ARG", "-headless").unwrap();
        firefox_pref(&mut session, "SB_FIREFOX_PREF", "a.b:1").unwrap();
        firefox_arg(&mut session, "SB_FIREFOX_ARG", "width=800").unwrap();
        assert_eq!(session.firefox.args(), ["--width=800"]);
        assert!(session.firefox.preferences().contains_key("a.b"));
    }

    #[test]
    fn the_text_of_a_file_value_is_a_string_a_number_or_a_boolean() {
        use serde_json::json;
        assert_eq!(text_of(&json!("a")).as_deref(), Some("a"));
        assert_eq!(text_of(&json!(7)).as_deref(), Some("7"));
        assert_eq!(text_of(&json!(true)).as_deref(), Some("true"));
        assert_eq!(text_of(&json!(["a"])), None);
        assert_eq!(text_of(&json!({"a": 1})), None);
    }
}
