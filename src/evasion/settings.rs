//! The values the settings of a [`Fingerprint`](super::fingerprint::Fingerprint)
//! can hold.
//!
//! Most settings are a plain `String`, `u32` or `f64`. A few belong together
//! (a screen is a width and a height, a position is a latitude and a longitude)
//! and are small types of their own. Every one of them converts from the
//! obvious tuple, so a builder call reads like the thing it sets:
//!
//! ```
//! use seleniumbase_rs::evasion::fingerprint::{Fingerprint, Spoof};
//!
//! let fp = Fingerprint::builder()
//!     .screen(1920, 1080)
//!     .geolocation((52.52, 13.405))
//!     .webgl(("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, GeForce RTX 4070)"))
//!     .webrtc(Spoof::Off)
//!     .build()?;
//! # Ok::<(), seleniumbase_rs::evasion::fingerprint::FingerprintError>(())
//! ```

use serde::{Deserialize, Serialize};

use super::fingerprint::MediaDeviceSpec;
use super::spoof::Spoof;

/// What a desktop's taskbar or menu bar takes from the height of the screen
/// when a fingerprint gives no available screen, in CSS pixels.
pub(crate) const TASKBAR_HEIGHT: u32 = 40;

/// A screen size in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ScreenSize {
    /// The width, in CSS pixels.
    pub width: u32,
    /// The height, in CSS pixels.
    pub height: u32,
}

impl ScreenSize {
    /// A screen of `width` by `height` CSS pixels.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for ScreenSize {
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for Spoof<ScreenSize> {
    fn from(size: (u32, u32)) -> Self {
        Self::Value(size.into())
    }
}

/// What `matchMedia` and `@media` rules report about the display and the
/// user's preferences. The pointer and hover features belong to the device:
/// a phone persona's touch emulation gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MediaFeatures {
    /// `color-gamut`: the widest gamut the display covers.
    pub color_gamut: ColorGamut,
    /// `prefers-color-scheme`.
    pub color_scheme: ColorScheme,
    /// `prefers-reduced-motion: reduce`.
    pub reduced_motion: bool,
    /// `prefers-reduced-transparency: reduce`.
    pub reduced_transparency: bool,
    /// `prefers-contrast`.
    pub contrast: Contrast,
    /// `forced-colors: active`, Windows high contrast.
    pub forced_colors: bool,
}

impl Default for MediaFeatures {
    fn default() -> Self {
        Self::new(ColorGamut::Srgb)
    }
}

impl MediaFeatures {
    /// The features of a display that covers `color_gamut`, with no preference set.
    #[must_use]
    pub const fn new(color_gamut: ColorGamut) -> Self {
        Self {
            color_gamut,
            color_scheme: ColorScheme::Light,
            reduced_motion: false,
            reduced_transparency: false,
            contrast: Contrast::NoPreference,
            forced_colors: false,
        }
    }

    /// The `features` array of `Emulation.setEmulatedMedia` for these values,
    /// one `{ "name", "value" }` per feature, in the order of the fields.
    pub(crate) fn emulated(&self) -> serde_json::Value {
        let reduce = |on: bool| if on { "reduce" } else { "no-preference" };
        serde_json::json!([
            { "name": "color-gamut", "value": self.color_gamut.media_value() },
            { "name": "prefers-color-scheme", "value": self.color_scheme.media_value() },
            { "name": "prefers-reduced-motion", "value": reduce(self.reduced_motion) },
            {
                "name": "prefers-reduced-transparency",
                "value": reduce(self.reduced_transparency)
            },
            { "name": "prefers-contrast", "value": self.contrast.media_value() },
            {
                "name": "forced-colors",
                "value": if self.forced_colors { "active" } else { "none" }
            },
        ])
    }
}

/// The `color-gamut` a display covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ColorGamut {
    /// `srgb`: the gamut of an ordinary desktop display.
    #[default]
    Srgb,
    /// `p3`: a Retina Mac or a recent phone.
    P3,
    /// `rec2020`.
    Rec2020,
}

impl ColorGamut {
    /// The value `Emulation.setEmulatedMedia` takes for `color-gamut`.
    pub(crate) const fn media_value(self) -> &'static str {
        match self {
            Self::Srgb => "srgb",
            Self::P3 => "p3",
            Self::Rec2020 => "rec2020",
        }
    }
}

/// `prefers-contrast`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Contrast {
    /// No preference.
    #[default]
    NoPreference,
    /// `more`.
    More,
    /// `less`.
    Less,
    /// `custom`.
    Custom,
}

impl Contrast {
    /// The value `Emulation.setEmulatedMedia` takes for `prefers-contrast`.
    pub(crate) const fn media_value(self) -> &'static str {
        match self {
            Self::NoPreference => "no-preference",
            Self::More => "more",
            Self::Less => "less",
            Self::Custom => "custom",
        }
    }
}

/// The colour scheme a page's `prefers-color-scheme` media query reports.
///
/// Chrome's own setting decides it unless
/// `Emulation::color_scheme` of the `sb_cdp` engine or a
/// fingerprint's [`MediaFeatures`] overrides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ColorScheme {
    /// `(prefers-color-scheme: light)` matches.
    Light,
    /// `(prefers-color-scheme: dark)` matches.
    Dark,
    /// No override: the browser's own setting decides again.
    NoPreference,
}

impl ColorScheme {
    /// The value `Emulation.setEmulatedMedia` takes for the feature. An empty
    /// value clears the override.
    pub(crate) const fn media_value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::NoPreference => "",
        }
    }
}

/// The layout `navigator.keyboard.getLayoutMap()` reports: the character each
/// writing-system key types, unshifted. The maps are Windows's own layouts
/// (KBDUS, KBDGR, KBDFR); a Mac's punctuation keys differ a little.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum KeyboardLayout {
    /// The US layout, used for English and most of the world.
    Qwerty,
    /// The German layout, used in central Europe.
    Qwertz,
    /// The French layout.
    Azerty,
}

/// The 48 key codes of a map, in the order the three layouts list them.
const KEY_CODES: [&str; 48] = [
    "Backquote",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Digit0",
    "Minus",
    "Equal",
    "KeyQ",
    "KeyW",
    "KeyE",
    "KeyR",
    "KeyT",
    "KeyY",
    "KeyU",
    "KeyI",
    "KeyO",
    "KeyP",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "KeyA",
    "KeyS",
    "KeyD",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyJ",
    "KeyK",
    "KeyL",
    "Semicolon",
    "Quote",
    "IntlBackslash",
    "KeyZ",
    "KeyX",
    "KeyC",
    "KeyV",
    "KeyB",
    "KeyN",
    "KeyM",
    "Comma",
    "Period",
    "Slash",
];

/// Pairs each code of [`KEY_CODES`] with the key a layout types on it.
const fn pair_keys(keys: [&'static str; 48]) -> [(&'static str, &'static str); 48] {
    let mut pairs = [("", ""); 48];
    let mut i = 0;
    while i < 48 {
        pairs[i] = (KEY_CODES[i], keys[i]);
        i += 1;
    }
    pairs
}

/// KBDUS, unshifted.
const QWERTY_KEYS: [(&str, &str); 48] = pair_keys([
    "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "q", "w", "e", "r", "t", "y",
    "u", "i", "o", "p", "[", "]", "\\", "a", "s", "d", "f", "g", "h", "j", "k", "l", ";", "'",
    "\\", "z", "x", "c", "v", "b", "n", "m", ",", ".", "/",
]);

/// KBDGR, unshifted.
const QWERTZ_KEYS: [(&str, &str); 48] = pair_keys([
    "^", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "ß", "´", "q", "w", "e", "r", "t", "z",
    "u", "i", "o", "p", "ü", "+", "#", "a", "s", "d", "f", "g", "h", "j", "k", "l", "ö", "ä", "<",
    "y", "x", "c", "v", "b", "n", "m", ",", ".", "-",
]);

/// KBDFR, unshifted.
const AZERTY_KEYS: [(&str, &str); 48] = pair_keys([
    "²", "&", "é", "\"", "'", "(", "-", "è", "_", "ç", "à", ")", "=", "a", "z", "e", "r", "t", "y",
    "u", "i", "o", "p", "^", "$", "*", "q", "s", "d", "f", "g", "h", "j", "k", "l", "m", "ù", "<",
    "w", "x", "c", "v", "b", "n", ",", ";", ":", "!",
]);

impl KeyboardLayout {
    /// The layout a machine set up for `locale` has, when its language has one
    /// that everybody uses: German and the languages of central Europe
    /// (`de`, `cs`, `sk`, `hu`, `sl`, `hr`) QWERTZ, French AZERTY except in
    /// Canada and Switzerland, and `en`, `es`, `it`, `pt`, `nl`, `pl`, `sv`,
    /// `da`, `nb`, `fi`, `ja`, `ko` and `zh` QWERTY; `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::evasion::fingerprint::KeyboardLayout;
    ///
    /// assert_eq!(KeyboardLayout::for_locale("de-DE"), Some(KeyboardLayout::Qwertz));
    /// assert_eq!(KeyboardLayout::for_locale("fr-CA"), None);
    /// ```
    #[must_use]
    pub fn for_locale(locale: &str) -> Option<Self> {
        let mut parts = locale.split(['-', '_']);
        let language = parts.next().unwrap_or_default().to_ascii_lowercase();
        let region = parts.next_back().unwrap_or_default().to_ascii_uppercase();
        match language.as_str() {
            "de" | "cs" | "sk" | "hu" | "sl" | "hr" => Some(Self::Qwertz),
            "fr" if region != "CA" && region != "CH" => Some(Self::Azerty),
            "en" | "es" | "it" | "pt" | "nl" | "pl" | "sv" | "da" | "nb" | "fi" | "ja" | "ko"
            | "zh" => Some(Self::Qwerty),
            _ => None,
        }
    }

    /// The layout's name in lower case, as the capture and the reports write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qwerty => "qwerty",
            Self::Qwertz => "qwertz",
            Self::Azerty => "azerty",
        }
    }

    /// The 48 keys of the map, `(code, key)`.
    #[must_use]
    pub const fn keys(self) -> &'static [(&'static str, &'static str); 48] {
        match self {
            Self::Qwerty => &QWERTY_KEYS,
            Self::Qwertz => &QWERTZ_KEYS,
            Self::Azerty => &AZERTY_KEYS,
        }
    }

    /// The layout a sample of a map is, read from `KeyQ`, `KeyY`, `KeyZ` and `KeyA`.
    pub(crate) fn of_sample(sample: &serde_json::Map<String, serde_json::Value>) -> Option<Self> {
        let key = |code: &str| sample.get(code).and_then(serde_json::Value::as_str);
        match (key("KeyQ"), key("KeyY"), key("KeyZ"), key("KeyA")) {
            (Some("a"), _, _, Some("q")) => Some(Self::Azerty),
            (Some("q"), Some("z"), Some("y"), _) => Some(Self::Qwertz),
            (Some("q"), Some("y"), Some("z"), _) => Some(Self::Qwerty),
            _ => None,
        }
    }
}

/// The position `navigator.geolocation` reports.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Geolocation {
    /// Degrees north.
    pub latitude: f64,
    /// Degrees east.
    pub longitude: f64,
    /// Metres above sea level; `0` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    /// How far off the position may be, in metres; `100` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

impl Geolocation {
    /// A position at `latitude`, `longitude`.
    #[must_use]
    pub const fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            altitude: None,
            accuracy: None,
        }
    }

    /// The same position with an altitude, in metres.
    #[must_use]
    pub const fn altitude(mut self, metres: f64) -> Self {
        self.altitude = Some(metres);
        self
    }

    /// The same position with an accuracy, in metres.
    #[must_use]
    pub const fn accuracy(mut self, metres: f64) -> Self {
        self.accuracy = Some(metres);
        self
    }
}

impl From<(f64, f64)> for Geolocation {
    fn from((latitude, longitude): (f64, f64)) -> Self {
        Self::new(latitude, longitude)
    }
}

impl From<(f64, f64)> for Spoof<Geolocation> {
    fn from(position: (f64, f64)) -> Self {
        Self::Value(position.into())
    }
}

/// The WebGL strings `getParameter` reports for the unmasked vendor and
/// renderer.
///
/// A half that is `None` is the browser's own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct WebGl {
    /// `UNMASKED_VENDOR_WEBGL`, such as `Google Inc. (NVIDIA)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// `UNMASKED_RENDERER_WEBGL`, such as `ANGLE (NVIDIA, ...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    /// The PCI vendor id of the GPU (`0x10de`). It is kept for tools that read
    /// it and never shown to a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    /// The PCI device id of the GPU (`0x1f91`). Kept, never shown to a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer_id: Option<String>,
}

impl WebGl {
    /// A vendor and a renderer.
    #[must_use]
    pub fn new(vendor: impl Into<String>, renderer: impl Into<String>) -> Self {
        Self {
            vendor: Some(vendor.into()),
            renderer: Some(renderer.into()),
            ..Self::default()
        }
    }

    /// A renderer alone: the vendor stays the browser's own.
    #[must_use]
    pub fn renderer_only(renderer: impl Into<String>) -> Self {
        Self {
            renderer: Some(renderer.into()),
            ..Self::default()
        }
    }

    /// A vendor alone: the renderer stays the browser's own.
    #[must_use]
    pub fn vendor_only(vendor: impl Into<String>) -> Self {
        Self {
            vendor: Some(vendor.into()),
            ..Self::default()
        }
    }

    /// The same strings with the PCI ids of the GPU.
    #[must_use]
    pub fn ids(mut self, vendor_id: impl Into<String>, renderer_id: impl Into<String>) -> Self {
        self.vendor_id = Some(vendor_id.into());
        self.renderer_id = Some(renderer_id.into());
        self
    }

    /// Whether neither string is given, so the setting would change nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vendor.is_none() && self.renderer.is_none()
    }
}

impl<V: Into<String>, R: Into<String>> From<(V, R)> for WebGl {
    fn from((vendor, renderer): (V, R)) -> Self {
        Self::new(vendor, renderer)
    }
}

impl<V: Into<String>, R: Into<String>> From<(V, R)> for Spoof<WebGl> {
    fn from(strings: (V, R)) -> Self {
        Self::Value(strings.into())
    }
}

/// The addresses WebRTC reports, and with them the request that its STUN and
/// TURN servers are filtered.
///
/// A fingerprint whose WebRTC is a `Value` filters the servers a page names, so
/// no peer connection can learn the real address. The two addresses are
/// optional on top of that: one that is given replaces the address the
/// browser would write into the SDP.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct WebRtcAddresses {
    /// The public address of the candidates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_ip: Option<String>,
    /// The local address of the connection line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
}

impl WebRtcAddresses {
    /// A public and a local address.
    #[must_use]
    pub fn new(public_ip: impl Into<String>, local_ip: impl Into<String>) -> Self {
        Self {
            public_ip: Some(public_ip.into()),
            local_ip: Some(local_ip.into()),
        }
    }
}

impl<P: Into<String>, L: Into<String>> From<(P, L)> for WebRtcAddresses {
    fn from((public_ip, local_ip): (P, L)) -> Self {
        Self::new(public_ip, local_ip)
    }
}

impl<P: Into<String>, L: Into<String>> From<(P, L)> for Spoof<WebRtcAddresses> {
    fn from(addresses: (P, L)) -> Self {
        Self::Value(addresses.into())
    }
}

/// What `navigator.mediaDevices.enumerateDevices()` lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct MediaDevices {
    /// How many microphones are listed.
    #[serde(default)]
    pub audio_inputs: u32,
    /// How many speakers are listed.
    #[serde(default)]
    pub audio_outputs: u32,
    /// How many cameras are listed.
    #[serde(default)]
    pub video_inputs: u32,
    /// Devices with their own ids and labels. When this is not empty, it is the
    /// whole list and the counts are not used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labelled: Vec<MediaDeviceSpec>,
}

impl MediaDevices {
    /// This many microphones, speakers and cameras, with generated labels.
    #[must_use]
    pub fn counts(audio_inputs: u32, audio_outputs: u32, video_inputs: u32) -> Self {
        Self {
            audio_inputs,
            audio_outputs,
            video_inputs,
            labelled: Vec::new(),
        }
    }

    /// Exactly these devices.
    #[must_use]
    pub fn labelled(devices: Vec<MediaDeviceSpec>) -> Self {
        Self {
            labelled: devices,
            ..Self::default()
        }
    }
}

impl From<(u32, u32, u32)> for MediaDevices {
    fn from((audio_inputs, audio_outputs, video_inputs): (u32, u32, u32)) -> Self {
        Self::counts(audio_inputs, audio_outputs, video_inputs)
    }
}

impl From<(u32, u32, u32)> for Spoof<MediaDevices> {
    fn from(counts: (u32, u32, u32)) -> Self {
        Self::Value(counts.into())
    }
}

impl From<Vec<MediaDeviceSpec>> for Spoof<MediaDevices> {
    fn from(devices: Vec<MediaDeviceSpec>) -> Self {
        Self::Value(MediaDevices::labelled(devices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_layout_has_48_keys_and_types_its_letters() {
        for layout in [
            KeyboardLayout::Qwerty,
            KeyboardLayout::Qwertz,
            KeyboardLayout::Azerty,
        ] {
            let keys = layout.keys();
            let mut codes: Vec<&str> = keys.iter().map(|(code, _)| *code).collect();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(keys.len(), 48);
            assert_eq!(codes.len(), 48, "{layout:?} repeats a code");
        }
        let key = |layout: KeyboardLayout, code: &str| {
            layout
                .keys()
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, key)| *key)
        };
        assert_eq!(key(KeyboardLayout::Qwerty, "KeyY"), Some("y"));
        assert_eq!(key(KeyboardLayout::Qwertz, "KeyY"), Some("z"));
        assert_eq!(key(KeyboardLayout::Qwertz, "KeyZ"), Some("y"));
        assert_eq!(key(KeyboardLayout::Azerty, "KeyQ"), Some("a"));
        assert_eq!(key(KeyboardLayout::Azerty, "KeyA"), Some("q"));
    }

    #[test]
    fn for_locale_picks_the_layout_of_the_language() {
        assert_eq!(
            KeyboardLayout::for_locale("de-DE"),
            Some(KeyboardLayout::Qwertz)
        );
        assert_eq!(
            KeyboardLayout::for_locale("fr-FR"),
            Some(KeyboardLayout::Azerty)
        );
        assert_eq!(KeyboardLayout::for_locale("fr-CA"), None);
        assert_eq!(
            KeyboardLayout::for_locale("en-GB"),
            Some(KeyboardLayout::Qwerty)
        );
        assert_eq!(KeyboardLayout::for_locale("ru-RU"), None);
    }

    #[test]
    fn a_sample_of_a_map_names_its_layout() {
        for layout in [
            KeyboardLayout::Qwerty,
            KeyboardLayout::Qwertz,
            KeyboardLayout::Azerty,
        ] {
            let sample: serde_json::Map<String, serde_json::Value> = layout
                .keys()
                .iter()
                .map(|(code, key)| ((*code).to_owned(), serde_json::json!(key)))
                .collect();
            assert_eq!(KeyboardLayout::of_sample(&sample), Some(layout));
        }
    }

    #[test]
    fn media_features_serialize_with_their_css_words() {
        let features = MediaFeatures {
            color_scheme: ColorScheme::Dark,
            contrast: Contrast::NoPreference,
            ..MediaFeatures::new(ColorGamut::P3)
        };

        let json = serde_json::to_value(features).unwrap();

        assert_eq!(json["color_gamut"], "p3");
        assert_eq!(json["color_scheme"], "dark");
        assert_eq!(json["contrast"], "no-preference");
        let again: MediaFeatures = serde_json::from_value(json).unwrap();
        assert_eq!(again, features);
    }

    #[test]
    fn media_features_become_one_emulated_media_array_in_field_order() {
        let features = MediaFeatures {
            reduced_motion: true,
            forced_colors: true,
            contrast: Contrast::More,
            ..MediaFeatures::new(ColorGamut::Rec2020)
        };

        let emulated = features.emulated();

        let pairs: Vec<(&str, &str)> = emulated
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["name"].as_str().unwrap(), f["value"].as_str().unwrap()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("color-gamut", "rec2020"),
                ("prefers-color-scheme", "light"),
                ("prefers-reduced-motion", "reduce"),
                ("prefers-reduced-transparency", "no-preference"),
                ("prefers-contrast", "more"),
                ("forced-colors", "active"),
            ]
        );
    }

    #[test]
    fn a_tuple_is_the_value_of_the_setting_it_names() {
        let screen: Spoof<ScreenSize> = (1920, 1080).into();
        let place: Spoof<Geolocation> = (52.52, 13.405).into();
        let gl: Spoof<WebGl> = ("Vendor", "Renderer").into();
        let rtc: Spoof<WebRtcAddresses> = ("203.0.113.9", "10.0.0.2").into();
        let devices: Spoof<MediaDevices> = (1, 2, 3).into();

        assert_eq!(screen, Spoof::Value(ScreenSize::new(1920, 1080)));
        assert_eq!(place.value().map(|p| p.latitude), Some(52.52));
        assert_eq!(
            gl.value().and_then(|g| g.renderer.as_deref()),
            Some("Renderer")
        );
        assert_eq!(
            rtc.value().and_then(|a| a.public_ip.as_deref()),
            Some("203.0.113.9")
        );
        assert_eq!(devices.value().map(|d| d.video_inputs), Some(3));
    }

    #[test]
    fn a_position_carries_its_optional_altitude_and_accuracy() {
        let place = Geolocation::new(1.0, 2.0).altitude(30.0).accuracy(15.0);

        assert_eq!((place.altitude, place.accuracy), (Some(30.0), Some(15.0)));
        assert_eq!(Geolocation::new(1.0, 2.0).accuracy, None);
    }

    #[test]
    fn a_webgl_half_is_the_browsers_own_until_it_is_given() {
        assert!(WebGl::default().is_empty());
        assert_eq!(WebGl::renderer_only("R").vendor, None);
        assert_eq!(WebGl::vendor_only("V").renderer, None);
        assert!(!WebGl::new("V", "R").ids("0x10de", "0x1f91").is_empty());
    }

    #[test]
    fn json_leaves_out_the_parts_that_are_absent() {
        let text = serde_json::to_string(&Spoof::Value(Geolocation::new(1.0, 2.0))).unwrap();
        let gl = serde_json::to_string(&Spoof::Value(WebGl::renderer_only("R"))).unwrap();

        assert_eq!(text, r#"{"latitude":1.0,"longitude":2.0}"#);
        assert_eq!(gl, r#"{"renderer":"R"}"#);
    }

    #[test]
    fn json_reads_a_keyword_or_the_value_for_a_compound_setting() {
        let off: Spoof<Geolocation> = serde_json::from_str(r#""off""#).unwrap();
        let place: Spoof<Geolocation> =
            serde_json::from_str(r#"{"latitude":1.0,"longitude":2.0}"#).unwrap();
        let size: Spoof<ScreenSize> =
            serde_json::from_str(r#"{"width":800,"height":600}"#).unwrap();

        assert_eq!(off, Spoof::Off);
        assert_eq!(place, Spoof::Value(Geolocation::new(1.0, 2.0)));
        assert_eq!(size, Spoof::Value(ScreenSize::new(800, 600)));
    }

    #[test]
    fn labelled_devices_replace_the_counts() {
        let spec = MediaDeviceSpec {
            kind: "audioinput".to_owned(),
            device_id: "a".to_owned(),
            group_id: "g".to_owned(),
            label: "Mic".to_owned(),
        };
        let devices = MediaDevices::labelled(vec![spec]);

        assert_eq!(devices.labelled.len(), 1);
        assert_eq!(devices.audio_inputs, 0);
    }
}
