//! Comparing two fingerprints, and a fingerprint with what a page saw of a
//! browser.

use std::fmt;

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::Fingerprint;

/// The names of the settings, in the order of the file. A test keeps this list
/// equal to the `Spoof` fields of the fingerprint.
pub(super) const SETTINGS: [&str; 29] = [
    "user_agent",
    "platform",
    "hardware_concurrency",
    "device_memory",
    "max_touch_points",
    "vendor",
    "oscpu",
    "product_sub",
    "build_id",
    "app_version",
    "locale",
    "languages",
    "accept_languages",
    "timezone",
    "screen",
    "avail_screen",
    "pixel_ratio",
    "color_depth",
    "webgl",
    "media_devices",
    "geolocation",
    "fonts",
    "proxy",
    "webrtc",
    "battery",
    "connection",
    "speech_voices",
    "bluetooth",
    "client_hints",
];

/// The settings that no page can read: the header is not visible to a script,
/// a position needs a permission prompt, the fonts and the addresses of WebRTC
/// need sweeps that would change what is measured, and the proxy is outside the
/// page.
const UNOBSERVABLE: [&str; 5] = ["accept_languages", "geolocation", "fonts", "proxy", "webrtc"];

/// How a setting that is `Natural` reads in the JSON of a setting.
const NATURAL: &str = "natural";

/// One setting that differs between two fingerprints.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SettingDiff {
    /// The name of the setting, as the file and its builder method spell it.
    pub setting: &'static str,
    /// What the expected fingerprint says, in the JSON of a setting: a value,
    /// `"off"` or `"natural"`.
    pub expected: Value,
    /// What the other side says, in the same form.
    pub actual: Value,
}

impl fmt::Display for SettingDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: expected {}, found {}",
            self.setting, self.expected, self.actual
        )
    }
}

/// The result of asking whether a live page presents a fingerprint: see
/// [`Fingerprint::compare_to`].
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FingerprintComparison {
    /// Settings the fingerprint claims and the capture contradicts.
    pub differences: Vec<SettingDiff>,
    /// Settings the fingerprint claims that the capture could not read: the ones
    /// no page can observe (`accept_languages`, `geolocation`, `fonts`, `proxy`,
    /// `webrtc`) and any reading that failed. They are not matches.
    pub unverifiable: Vec<&'static str>,
    /// Settings the fingerprint claims and the capture agrees with.
    pub matched: Vec<&'static str>,
}

impl FingerprintComparison {
    /// Whether nothing the page could be read for contradicts the fingerprint.
    ///
    /// This says nothing about the [`unverifiable`](Self::unverifiable) settings.
    #[must_use]
    pub fn presents(&self) -> bool {
        self.differences.is_empty()
    }

    /// Whether every setting the fingerprint claims was read and agrees.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.differences.is_empty() && self.unverifiable.is_empty()
    }
}

/// How strictly two readings are compared.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Two fingerprints: every difference counts.
    Exact,
    /// A fingerprint and a capture: the battery and the connection move, so
    /// they agree when both are there or both are off.
    Live,
}

impl Fingerprint {
    /// The settings that differ between this fingerprint, which is what is
    /// expected, and `other`, which is what is found.
    ///
    /// A setting differs when this fingerprint gives it (anything but
    /// `Natural`) and `other` holds another. A setting this fingerprint leaves
    /// `Natural` is never compared: the browser's own value agrees with any
    /// value, so `Natural` against a value is a difference only when the
    /// expected side gives the value. Language lists are compared without their
    /// quality values (`en-US,en;q=0.9` is `en-US,en`), a WebGL string this side
    /// leaves out is not compared, media devices are compared by count (`Off`
    /// lists none), and the brands of Client Hints in any order.
    ///
    /// The result is in the order of the settings in the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Fingerprint;
    ///
    /// let windows = Fingerprint::windows_desktop();
    /// let tokyo = windows.to_builder().timezone("Asia/Tokyo").build()?;
    ///
    /// let diff = tokyo.diff(&windows);
    /// assert_eq!(diff.len(), 1);
    /// assert_eq!(diff[0].setting, "timezone");
    /// assert_eq!(diff[0].expected, "Asia/Tokyo");
    /// assert_eq!(diff[0].actual, "America/New_York");
    ///
    /// // What `windows` leaves natural is not compared, so this side agrees.
    /// let bare = Fingerprint::builder().timezone("America/New_York").build()?;
    /// assert!(bare.diff(&windows).is_empty());
    /// # Ok::<(), seleniumbase_rs::FingerprintError>(())
    /// ```
    #[must_use]
    pub fn diff(&self, other: &Self) -> Vec<SettingDiff> {
        self.compare_with(other, Mode::Exact).differences
    }

    /// Whether the browser a capture was taken in presents this fingerprint:
    /// the settings it claims against `observed`, the fingerprint of a
    /// [`CapturedFingerprint`](crate::stealth::capture::CapturedFingerprint).
    ///
    /// It is [`diff`](Self::diff) with two differences, so that the answer
    /// cannot overstate. A setting this fingerprint claims that the capture could
    /// not read, because no page can (`accept_languages`, `geolocation`, `fonts`,
    /// `proxy`, `webrtc`) or because the reading failed, is listed under
    /// [`unverifiable`](FingerprintComparison::unverifiable) and never as a match.
    /// And the battery and the connection are compared by whether they are there,
    /// not by their numbers, which move.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Fingerprint;
    ///
    /// let claimed = Fingerprint::windows_desktop();
    ///
    /// // A capture that read the same browser, and only that: nothing else read.
    /// let seen = Fingerprint::builder()
    ///     .timezone("America/New_York")
    ///     .locale("en-US")
    ///     .build()?;
    /// let comparison = claimed.compare_to(&seen);
    ///
    /// assert!(comparison.presents(), "nothing read contradicts the claim");
    /// assert!(!comparison.is_verified(), "most of the claim was not read");
    /// assert!(comparison.matched.contains(&"timezone"));
    /// assert!(comparison.unverifiable.contains(&"user_agent"));
    /// # Ok::<(), seleniumbase_rs::FingerprintError>(())
    /// ```
    #[must_use]
    pub fn compare_to(&self, observed: &Self) -> FingerprintComparison {
        self.compare_with(observed, Mode::Live)
    }

    /// The names of the settings this fingerprint gives, which is every one that is
    /// not `Natural`, in the order of the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Fingerprint;
    ///
    /// let fp = Fingerprint::builder().timezone("Europe/Berlin").screen(800, 600).build()?;
    ///
    /// assert_eq!(fp.claimed_settings(), ["timezone", "screen"]);
    /// # Ok::<(), seleniumbase_rs::FingerprintError>(())
    /// ```
    #[must_use]
    pub fn claimed_settings(&self) -> Vec<&'static str> {
        self.setting_values()
            .into_iter()
            .filter(|(_, value)| !is_natural(value))
            .map(|(name, _)| name)
            .collect()
    }

    /// The settings as the JSON of a setting each: `"natural"` for one that is
    /// left to the browser.
    fn setting_values(&self) -> Vec<(&'static str, Value)> {
        let mut map = match serde_json::to_value(&self.0) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        SETTINGS
            .iter()
            .map(|name| (*name, map.remove(*name).unwrap_or_else(|| json!(NATURAL))))
            .collect()
    }

    fn compare_with(&self, other: &Self, mode: Mode) -> FingerprintComparison {
        let mut comparison = FingerprintComparison::default();
        for ((setting, expected), (_, actual)) in
            self.setting_values().into_iter().zip(other.setting_values())
        {
            if is_natural(&expected) {
                continue;
            }
            if mode == Mode::Live && (UNOBSERVABLE.contains(&setting) || is_natural(&actual)) {
                comparison.unverifiable.push(setting);
            } else if agree(setting, &expected, &actual, mode) {
                comparison.matched.push(setting);
            } else {
                comparison.differences.push(SettingDiff {
                    setting,
                    expected,
                    actual,
                });
            }
        }
        comparison
    }
}

fn is_natural(setting: &Value) -> bool {
    setting.as_str() == Some(NATURAL)
}

fn is_off(setting: &Value) -> bool {
    setting.as_str() == Some("off")
}

/// Whether two readings of one setting say the same.
fn agree(setting: &str, expected: &Value, actual: &Value, mode: Mode) -> bool {
    match setting {
        "languages" | "accept_languages" => match (language_tags(expected), language_tags(actual)) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => same_json(expected, actual),
        },
        "webgl" => webgl_agrees(expected, actual),
        "media_devices" => device_counts(expected) == device_counts(actual),
        "speech_voices" if is_off(expected) || is_off(actual) => {
            is_empty_list(expected) && is_empty_list(actual)
        }
        "client_hints" => same_json(&sorted_hints(expected), &sorted_hints(actual)),
        "battery" | "connection" if mode == Mode::Live => is_off(expected) == is_off(actual),
        _ => same_json(expected, actual),
    }
}

/// The tags of a language list, without quality values, lower-cased.
fn language_tags(list: &Value) -> Option<Vec<String>> {
    Some(
        list.as_str()?
            .split(',')
            .filter_map(|tag| {
                let tag = tag.split(';').next().unwrap_or(tag).trim();
                (!tag.is_empty()).then(|| tag.to_ascii_lowercase())
            })
            .collect(),
    )
}

/// Whether the WebGL strings the expected side gives are the actual ones.
fn webgl_agrees(expected: &Value, actual: &Value) -> bool {
    let (Some(expected), Some(actual)) = (expected.as_object(), actual.as_object()) else {
        return same_json(expected, actual);
    };
    ["vendor", "renderer"]
        .iter()
        .all(|half| expected.get(*half).is_none_or(|given| actual.get(*half) == Some(given)))
}

/// How many microphones, speakers and cameras a media-devices setting lists.
fn device_counts(devices: &Value) -> (u64, u64, u64) {
    let Some(devices) = devices.as_object() else {
        // `Off` lists none.
        return (0, 0, 0);
    };
    let labelled = devices.get("labelled").and_then(Value::as_array);
    if let Some(list) = labelled.filter(|list| !list.is_empty()) {
        let count = |kind: &str| {
            list.iter()
                .filter(|device| device.get("kind").and_then(Value::as_str) == Some(kind))
                .count() as u64
        };
        return (count("audioinput"), count("audiooutput"), count("videoinput"));
    }
    let number = |key: &str| devices.get(key).and_then(Value::as_u64).unwrap_or(0);
    (number("audio_inputs"), number("audio_outputs"), number("video_inputs"))
}

/// Whether a voices setting lists nothing: `Off`, or an empty list.
fn is_empty_list(voices: &Value) -> bool {
    is_off(voices) || voices.as_array().is_some_and(Vec::is_empty)
}

/// Client Hints with their lists in a fixed order, because Chrome's brand list
/// is shuffled by version.
fn sorted_hints(hints: &Value) -> Value {
    let Some(object) = hints.as_object() else {
        return hints.clone();
    };
    let mut object = object.clone();
    for key in ["brands", "full_version_list"] {
        if let Some(Value::Array(list)) = object.get_mut(key) {
            list.sort_by_key(|brand| brand.to_string());
        }
    }
    if let Some(Value::Array(list)) = object.get_mut("form_factors") {
        list.sort_by_key(ToString::to_string);
    }
    Value::Object(object)
}

/// JSON equality that reads `8` and `8.0` as one number.
fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same_json(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, x)| y.get(key).is_some_and(|y| same_json(x, y)))
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::{
        FingerprintBuilder, MediaDeviceSpec, OsType, ProxyConfig, SpeechVoice, Spoof, WebGl,
    };

    fn base() -> Fingerprint {
        Fingerprint::windows_desktop()
    }

    fn changed(change: impl FnOnce(FingerprintBuilder) -> FingerprintBuilder) -> Fingerprint {
        change(base().to_builder()).build().expect("builds")
    }

    fn names(diff: &[SettingDiff]) -> Vec<&'static str> {
        diff.iter().map(|d| d.setting).collect()
    }

    #[test]
    fn the_list_of_settings_is_the_list_of_the_fields_that_are_a_spoof() {
        // Every setting set, so that every key is written.
        let full = Fingerprint::randomized(OsType::Linux, 3)
            .to_builder()
            .oscpu("Linux x86_64")
            .product_sub("20030107")
            .build_id("20181001000000")
            .app_version("5.0 (X11)")
            .avail_screen((1920, 1040))
            .fonts(["Arial"])
            .media_devices((1, 1, 1))
            .webrtc(("203.0.113.9", "10.0.0.2"))
            .proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "proxy.test".to_owned(),
                port: 8080,
                ..ProxyConfig::default()
            })
            .build()
            .unwrap();
        let Value::Object(written) = serde_json::to_value(&full).unwrap() else {
            panic!("a fingerprint is a JSON object");
        };
        let not_settings = [
            "browser_type",
            "os_type",
            "core_version",
            "seed",
            "policy",
            "humanize",
            "local_storage",
            "save_service_worker",
            "custom_start_urls",
            "cmd_params",
        ];

        let mut settings: Vec<&str> = written
            .keys()
            .map(String::as_str)
            .filter(|key| !not_settings.contains(key))
            .collect();
        let mut listed = SETTINGS.to_vec();
        settings.sort_unstable();
        listed.sort_unstable();

        assert_eq!(settings, listed);
    }

    #[test]
    fn the_claimed_settings_are_the_ones_that_are_not_natural_in_the_order_of_the_file() {
        assert!(Fingerprint::builder().build().unwrap().claimed_settings().is_empty());
        let fp = Fingerprint::builder()
            .geolocation(Spoof::Off)
            .screen(800, 600)
            .timezone("Europe/Berlin")
            .build()
            .unwrap();

        assert_eq!(fp.claimed_settings(), ["timezone", "screen", "geolocation"]);
        assert!(Fingerprint::windows_desktop().claimed_settings().len() > 10);
    }

    #[test]
    fn a_fingerprint_does_not_differ_from_itself() {
        for fp in [
            Fingerprint::windows_desktop(),
            Fingerprint::ios_mobile_safari(),
            Fingerprint::randomized(OsType::Macos, 5),
        ] {
            assert!(fp.diff(&fp).is_empty());
        }
    }

    #[test]
    fn a_difference_names_the_setting_and_both_sides() {
        let tokyo = changed(|b| b.timezone("Asia/Tokyo"));

        let diff = tokyo.diff(&base());

        assert_eq!(names(&diff), ["timezone"]);
        assert_eq!(diff[0].expected, json!("Asia/Tokyo"));
        assert_eq!(diff[0].actual, json!("America/New_York"));
        assert_eq!(
            diff[0].to_string(),
            r#"timezone: expected "Asia/Tokyo", found "America/New_York""#
        );
    }

    #[test]
    fn the_differences_come_in_the_order_of_the_file() {
        let other = Fingerprint::macos_desktop();

        let diff = base().diff(&other);

        let positions: Vec<usize> = diff
            .iter()
            .map(|d| SETTINGS.iter().position(|s| *s == d.setting).unwrap())
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted);
        assert!(names(&diff).contains(&"user_agent") && names(&diff).contains(&"webgl"));
    }

    #[test]
    fn natural_against_a_value_differs_only_when_the_expected_side_gives_a_value() {
        let natural = Fingerprint::builder().build().unwrap();
        let tokyo = Fingerprint::builder().timezone("Asia/Tokyo").build().unwrap();

        assert!(natural.diff(&tokyo).is_empty(), "nothing was expected");
        let reverse = tokyo.diff(&natural);
        assert_eq!(names(&reverse), ["timezone"]);
        assert_eq!(reverse[0].actual, json!("natural"));
    }

    #[test]
    fn off_is_a_claim_and_differs_from_a_value_and_from_natural() {
        let off = Fingerprint::builder().battery(Spoof::Off).build().unwrap();
        let on = Fingerprint::builder().battery(Fingerprint::windows_desktop().battery().value().cloned().unwrap()).build().unwrap();
        let natural = Fingerprint::builder().build().unwrap();

        assert_eq!(names(&off.diff(&on)), ["battery"]);
        assert_eq!(names(&off.diff(&natural)), ["battery"]);
        assert!(off.diff(&off).is_empty());
    }

    #[test]
    fn language_lists_are_compared_without_their_quality_values() {
        let with_q = changed(|b| b.languages("en-US,en;q=0.9"));
        let without = changed(|b| b.languages("EN-us , en"));
        let other = changed(|b| b.languages("de-DE,de"));

        assert!(with_q.diff(&without).is_empty());
        assert_eq!(names(&with_q.diff(&other)), ["languages"]);
    }

    #[test]
    fn a_webgl_string_the_expected_side_leaves_out_is_not_compared() {
        let full = changed(|b| b.webgl(("Vendor", "Renderer")));
        let renderer_only = changed(|b| b.webgl(WebGl::renderer_only("Renderer")));
        let other_renderer = changed(|b| b.webgl(("Vendor", "Other")));

        assert!(renderer_only.diff(&full).is_empty());
        assert_eq!(names(&full.diff(&renderer_only)), ["webgl"]);
        assert_eq!(names(&renderer_only.diff(&other_renderer)), ["webgl"]);
    }

    #[test]
    fn media_devices_are_compared_by_count_and_off_lists_none() {
        let counts = changed(|b| b.media_devices((1, 2, 3)));
        let none = changed(|b| b.media_devices((0, 0, 0)));
        let off = changed(|b| b.media_devices(Spoof::Off));
        let labelled = changed(|b| {
            b.media_devices(vec![
                spec("audioinput", "a"),
                spec("audiooutput", "b"),
                spec("audiooutput", "c"),
                spec("videoinput", "d"),
                spec("videoinput", "e"),
                spec("videoinput", "f"),
            ])
        });

        assert!(counts.diff(&labelled).is_empty(), "labelled devices count by kind");
        assert!(none.diff(&off).is_empty() && off.diff(&none).is_empty());
        assert_eq!(names(&counts.diff(&none)), ["media_devices"]);
    }

    fn spec(kind: &str, id: &str) -> MediaDeviceSpec {
        MediaDeviceSpec {
            kind: kind.to_owned(),
            device_id: id.to_owned(),
            group_id: id.to_owned(),
            label: id.to_owned(),
        }
    }

    #[test]
    fn no_voice_and_off_are_the_same_claim() {
        let off = changed(|b| b.speech_voices(Spoof::Off));
        let empty = changed(|b| b.speech_voices(Vec::<SpeechVoice>::new()));
        let some = base();

        assert!(off.diff(&empty).is_empty() && empty.diff(&off).is_empty());
        assert_eq!(names(&off.diff(&some)), ["speech_voices"]);
    }

    #[test]
    fn the_brands_of_client_hints_are_compared_in_any_order() {
        let randomized = Fingerprint::randomized(OsType::Windows, 11);
        let mut hints = randomized.client_hints().value().cloned().unwrap();
        hints.brands.reverse();
        hints.full_version_list.reverse();
        let reordered = randomized.to_builder().client_hints(hints.clone()).build().unwrap();
        hints.platform_version = "99.0.0".to_owned();
        let other = randomized.to_builder().client_hints(hints).build().unwrap();

        assert!(randomized.diff(&reordered).is_empty());
        assert_eq!(names(&randomized.diff(&other)), ["client_hints"]);
    }

    #[test]
    fn a_whole_number_and_the_same_float_are_one_number() {
        assert!(same_json(&json!(8), &json!(8.0)));
        assert!(same_json(&json!([1, {"a": 2.0}]), &json!([1.0, {"a": 2}])));
        assert!(!same_json(&json!(8), &json!(8.5)));
        assert!(!same_json(&json!("8"), &json!(8)));
    }

    // -----------------------------------------------------------------------
    // compare_to
    // -----------------------------------------------------------------------

    /// A capture that read everything a page can read and agrees.
    fn as_captured(claimed: &Fingerprint) -> Fingerprint {
        // What a page cannot read is natural in a capture, except the header
        // that a Chromium capture derives.
        claimed
            .to_builder()
            .geolocation(Spoof::Natural)
            .fonts(Spoof::Natural)
            .webrtc(Spoof::Natural)
            .proxy(Spoof::Natural)
            .build()
            .unwrap()
    }

    #[test]
    fn what_a_page_cannot_read_is_unverifiable_never_a_match() {
        let claimed =
            changed(|b| b.geolocation((52.52, 13.405)).fonts(["Arial"]).webrtc(Spoof::Off));
        let seen = as_captured(&claimed);

        let comparison = claimed.compare_to(&seen);

        for setting in ["geolocation", "fonts", "webrtc"] {
            assert!(comparison.unverifiable.contains(&setting), "{setting}: {comparison:?}");
            assert!(!comparison.matched.contains(&setting), "{setting}");
        }
        assert!(comparison.presents());
    }

    #[test]
    fn a_header_the_capture_derived_is_still_unverifiable() {
        let claimed = changed(|b| b.accept_languages("en-US,en;q=0.9"));
        let seen = changed(|b| b.accept_languages("en-US,en;q=0.9"));

        let comparison = claimed.compare_to(&seen);

        assert!(comparison.unverifiable.contains(&"accept_languages"));
        assert!(!comparison.matched.contains(&"accept_languages"));
    }

    #[test]
    fn a_setting_the_capture_failed_to_read_is_unverifiable() {
        let claimed = base();
        let seen = changed(|b| b.timezone(Spoof::Natural));

        let comparison = claimed.compare_to(&seen);

        assert!(comparison.unverifiable.contains(&"timezone"));
        assert!(comparison.differences.is_empty());
    }

    #[test]
    fn a_capture_that_agrees_with_what_it_could_read_presents_the_fingerprint() {
        let claimed = changed(|b| b.accept_languages("en-US,en;q=0.9"));
        let seen = as_captured(&claimed);

        let comparison = claimed.compare_to(&seen);

        assert!(comparison.presents(), "{:?}", comparison.differences);
        assert!(comparison.matched.contains(&"user_agent"));
        assert!(comparison.matched.contains(&"webgl"));
        assert!(!comparison.is_verified(), "the header was not read");
    }

    #[test]
    fn a_capture_that_contradicts_the_claim_is_listed_with_both_sides() {
        let claimed = base();
        let seen = changed(|b| b.timezone("Asia/Tokyo").hardware_concurrency(2));

        let comparison = claimed.compare_to(&seen);

        assert!(!comparison.presents());
        assert_eq!(names(&comparison.differences), ["hardware_concurrency", "timezone"]);
        let timezone = &comparison.differences[1];
        assert_eq!(timezone.expected, json!("America/New_York"));
        assert_eq!(timezone.actual, json!("Asia/Tokyo"));
    }

    #[test]
    fn the_battery_and_the_connection_are_compared_by_presence_in_a_live_page() {
        let claimed = base();
        let mut battery = claimed.battery().value().cloned().unwrap();
        battery.level = 0.31;
        battery.charging = false;
        let mut link = claimed.connection().value().cloned().unwrap();
        link.rtt = 250;
        link.downlink = 1.1;
        let seen = claimed.to_builder().battery(battery).connection(link).build().unwrap();

        let live = claimed.compare_to(&seen);
        let exact = claimed.diff(&seen);

        assert!(live.presents(), "{:?}", live.differences);
        assert!(live.matched.contains(&"battery") && live.matched.contains(&"connection"));
        assert_eq!(names(&exact), ["battery", "connection"], "two fingerprints compare exactly");
    }

    #[test]
    fn a_page_that_has_a_battery_contradicts_a_claim_of_none() {
        let claimed = changed(|b| b.battery(Spoof::Off));
        let seen = base();

        let comparison = claimed.compare_to(&seen);

        assert_eq!(names(&comparison.differences), ["battery"]);
    }

    #[test]
    fn a_fingerprint_that_claims_nothing_is_presented_by_any_page_and_verifies_nothing() {
        let claimed = Fingerprint::builder().build().unwrap();

        let comparison = claimed.compare_to(&base());

        assert!(comparison.is_verified());
        assert!(comparison.matched.is_empty() && comparison.unverifiable.is_empty());
    }
}
