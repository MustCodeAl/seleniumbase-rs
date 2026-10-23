//! Fingerprint / anti-detection profile configuration.
//!
//! A [`Fingerprint`] says what a browser should claim to be: its navigator,
//! screen, time zone, position, graphics, media devices, fonts, proxy and the
//! rest of what a page can probe.
//!
//! # One setting, four states
//!
//! Every setting is one [`Spoof`]: leave the browser's own value
//! ([`Natural`](Spoof::Natural), the default), draw a valid one
//! ([`Random`](Spoof::Random)), use exactly this one
//! ([`Value`](Spoof::Value), which a plain value converts to) or turn the API
//! off ([`Off`](Spoof::Off), where that means something). How the claims are
//! delivered, and which protections run beside them, is the
//! [`StealthPolicy`], not a setting.
//!
//! A fingerprint is built once, with [`Fingerprint::builder()`], and not
//! changed afterwards. To change one, take [`to_builder`](Fingerprint::to_builder)
//! and build again. Hand the result to
//! [`StealthOptions`](crate::stealth::options::StealthOptions), to a
//! [`BrowserConfig`](crate::BrowserConfig) or to the pure CDP launch options.
//!
//! # Example
//!
//! ```rust
//! use seleniumbase_rs::stealth::fingerprint::{
//!     CanvasNoise, Fingerprint, Spoof, StealthPolicy,
//! };
//!
//! let fp = Fingerprint::builder()
//!     .timezone("Europe/Berlin")
//!     .screen(1920, 1080)
//!     .geolocation(Spoof::Random)
//!     .webrtc(Spoof::Off)
//!     .policy(StealthPolicy::balanced())
//!     .canvas_noise(CanvasNoise::Random)
//!     .build()?;
//!
//! assert!(fp.timezone().is_value());
//! assert!(fp.webrtc().is_off());
//! // The random position is a spot in Berlin, because the time zone is given.
//! let place = fp.geolocation().value().expect("a drawn position");
//! assert!((place.latitude - 52.5).abs() < 0.5);
//! # Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize};

mod compare;
mod file;
#[cfg(test)]
mod schema;

pub use compare::{FingerprintComparison, SettingDiff};

use super::humanize::Rng;
pub use super::policy::{AudioNoise, CanvasNoise, StealthPolicy};
use super::randomize;
pub use super::settings::{Geolocation, MediaDevices, ScreenSize, WebGl, WebRtcAddresses};
pub use super::spoof::Spoof;

/// Browser persona requested by a profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum BrowserType {
    /// Chromium-based persona (default).
    #[default]
    #[serde(rename = "chromium")]
    Chromium,
    /// Firefox-oriented persona (conceptual; WebDriver mode uses Chromium).
    #[serde(rename = "firefox")]
    Firefox,
    /// Mobile Safari / WebKit persona (iOS).
    #[serde(rename = "mobile_safari", alias = "safari")]
    MobileSafari,
}

/// Operating-system persona.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum OsType {
    #[default]
    Windows,
    Macos,
    Linux,
    Android,
    /// iOS / iPadOS persona.
    #[serde(rename = "ios", alias = "ipados")]
    Ios,
}

impl OsType {
    /// Returns the `navigator.platform` value that matches the OS persona.
    pub fn platform(&self) -> &'static str {
        match self {
            OsType::Windows => "Win32",
            OsType::Macos => "MacIntel",
            OsType::Linux => "Linux x86_64",
            OsType::Android => "Linux armv8l",
            OsType::Ios => "iPhone",
        }
    }

    /// Common default screen size for the OS persona.
    pub fn default_screen(&self) -> (u32, u32) {
        match self {
            OsType::Windows => (1920, 1080),
            OsType::Macos => (1920, 1080),
            OsType::Linux => (1920, 1080),
            OsType::Android => (412, 732),
            OsType::Ios => (390, 844),
        }
    }

    /// Returns true for mobile OS personas that typically use touch input.
    pub fn is_mobile(&self) -> bool {
        matches!(self, OsType::Android | OsType::Ios)
    }
}

/// QUIC mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum QuicMode {
    #[default]
    Natural,
    Disabled,
    Enabled,
    ForceHttp2,
    Auto,
}

/// WebRTC IP-handling policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum WebRtcPolicy {
    #[default]
    DisableNonProxiedUdp,
    PublicAndPrivateInterfaces,
    PublicInterfaceOnly,
}

/// Proxy configuration for a profile.
#[derive(Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ProxyConfig {
    pub r#type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_traffic: bool,
}

// Written by hand so that logging a fingerprint never prints the password.
impl fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyConfig")
            .field("type", &self.r#type)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("save_traffic", &self.save_traffic)
            .finish()
    }
}

impl ProxyConfig {
    /// Returns the proxy URL used for Chrome command-line flags.
    pub fn to_url(&self) -> String {
        let auth = match (&self.username, &self.password) {
            (Some(u), Some(p)) => format!("{u}:{p}@"),
            _ => String::new(),
        };
        format!("{}://{}{}:{}", self.r#type, auth, self.host, self.port)
    }
}

/// JSON has no infinity, so the two battery times that can be infinite are
/// written as `null` and read back as infinity.
mod infinite_as_null {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            serializer.serialize_f64(*value)
        } else {
            serializer.serialize_none()
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(f64::INFINITY))
    }

    pub(super) fn infinity() -> f64 {
        f64::INFINITY
    }
}

/// Battery Status API profile (`navigator.getBattery()`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct BatteryProfile {
    pub charging: bool,
    /// Battery level in the range `0.0..=1.0`.
    pub level: f64,
    /// Seconds until fully charged (`f64::INFINITY` when unknown; `null` in JSON).
    #[serde(with = "infinite_as_null", default = "infinite_as_null::infinity")]
    #[cfg_attr(test, schemars(with = "Option<f64>"))]
    pub charging_time: f64,
    /// Seconds until empty (`f64::INFINITY` when charging; `null` in JSON).
    #[serde(with = "infinite_as_null", default = "infinite_as_null::infinity")]
    #[cfg_attr(test, schemars(with = "Option<f64>"))]
    pub discharging_time: f64,
}

impl Default for BatteryProfile {
    fn default() -> Self {
        Self {
            charging: true,
            level: 0.94,
            charging_time: 0.0,
            discharging_time: f64::INFINITY,
        }
    }
}

/// Network Information API profile (`navigator.connection`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ConnectionProfile {
    pub effective_type: String,
    pub rtt: u32,
    pub downlink: f64,
    pub save_data: bool,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            effective_type: "4g".to_owned(),
            rtt: 50,
            downlink: 10.0,
            save_data: false,
        }
    }
}

/// A single synthesized speech voice for `speechSynthesis.getVoices()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SpeechVoice {
    pub name: String,
    pub lang: String,
    pub default: bool,
    pub local_service: bool,
    pub voice_uri: String,
}

impl SpeechVoice {
    /// Returns a plausible default set of English voices (those of Windows).
    pub fn default_set() -> Vec<Self> {
        vec![
            Self {
                name: "Microsoft David - English (United States)".to_owned(),
                lang: "en-US".to_owned(),
                default: true,
                local_service: true,
                voice_uri: "Microsoft David - English (United States)".to_owned(),
            },
            Self {
                name: "Microsoft Zira - English (United States)".to_owned(),
                lang: "en-US".to_owned(),
                default: false,
                local_service: true,
                voice_uri: "Microsoft Zira - English (United States)".to_owned(),
            },
            Self {
                name: "Google US English".to_owned(),
                lang: "en-US".to_owned(),
                default: false,
                local_service: false,
                voice_uri: "Google US English".to_owned(),
            },
        ]
    }
}

/// A `(brand, version)` pair used by the Client Hints `brands` array.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct BrandVersion {
    pub brand: String,
    pub version: String,
}

impl BrandVersion {
    pub fn new(brand: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            brand: brand.into(),
            version: version.into(),
        }
    }
}

/// A single labeled media device used by `MediaDevicesProvider`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct MediaDeviceSpec {
    pub kind: String,
    pub device_id: String,
    pub group_id: String,
    pub label: String,
}

/// User-Agent Client Hints (`navigator.userAgentData`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ClientHints {
    pub brands: Vec<BrandVersion>,
    pub full_version_list: Vec<BrandVersion>,
    pub platform: String,
    pub platform_version: String,
    pub architecture: String,
    pub bitness: String,
    pub model: String,
    pub ua_full_version: String,
    pub mobile: bool,
    /// Whether the browser runs as a 32-bit process on a 64-bit system (`Sec-CH-UA-WoW64`).
    #[serde(default)]
    pub wow64: bool,
    /// Client Hints form factors, such as `Desktop` or `Mobile` (`Sec-CH-UA-Form-Factors`).
    #[serde(default)]
    pub form_factors: Vec<String>,
}

/// Humanized input-timing configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct HumanizeConfig {
    pub enabled: bool,
    pub min_keystroke_delay_ms: u64,
    pub max_keystroke_delay_ms: u64,
    /// Number of intermediate points sampled along a mouse path.
    pub mouse_steps: u32,
}

impl Default for HumanizeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_keystroke_delay_ms: 40,
            max_keystroke_delay_ms: 180,
            mouse_steps: 24,
        }
    }
}

/// Default list of well-known tracking / fingerprinting script hosts that a
/// network interceptor may block. This is a generic, brand-neutral starter set.
pub fn default_tracker_hosts() -> Vec<String> {
    [
        "doubleclick.net",
        "googlesyndication.com",
        "google-analytics.com",
        "googletagmanager.com",
        "adservice.google.com",
        "facebook.net",
        "connect.facebook.net",
        "fpjs.io",
        "fingerprintjs.com",
        "cdn.fingerprint.com",
        "scorecardresearch.com",
        "hotjar.com",
        "segment.io",
        "amplitude.com",
        "mixpanel.com",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// Coherence report produced by [`Fingerprint::validate`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoherenceReport {
    /// Hard inconsistencies likely to trip detection (e.g. OS/UA mismatch).
    pub errors: Vec<String>,
    /// Softer issues worth reviewing.
    pub warnings: Vec<String>,
}

impl CoherenceReport {
    /// Returns `true` when there are no hard errors.
    pub fn is_coherent(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Why a [`FingerprintBuilder`] could not build a fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FingerprintError {
    /// A setting was given [`Off`](Spoof::Off), which means nothing for it.
    ///
    /// Only geolocation, WebRTC, media devices, battery, Bluetooth, speech
    /// voices, network information and Client Hints have an off state.
    #[error("{setting} cannot be Off: only geolocation, webrtc, media_devices, battery, bluetooth, speech_voices, connection and client_hints have an off state")]
    OffNotAvailable {
        /// The name of the setting, as its builder method is named.
        setting: &'static str,
    },
    /// A setting was given [`Random`](Spoof::Random), but there is no value to
    /// draw for it.
    #[error("{setting} cannot be Random: there is no value to draw for it")]
    NoRandomValue {
        /// The name of the setting, as its builder method is named.
        setting: &'static str,
    },
    /// A setting was given [`Random`](Spoof::Random), but nothing coherent with
    /// the settings that were given can be drawn.
    #[error("no coherent random {setting}: {reason}")]
    Incoherent {
        /// The name of the setting, as its builder method is named.
        setting: &'static str,
        /// What the draw could not agree with.
        reason: String,
    },
}

impl From<FingerprintError> for crate::error::SeleniumBaseError {
    fn from(error: FingerprintError) -> Self {
        Self::Stealth {
            reason: error.to_string(),
        }
    }
}

/// A part of a browser's identity that a fingerprint can mask.
///
/// [`Fingerprint::is_masked`] answers, for one of these, whether the
/// fingerprint spoofs it: the settings that belong to it are not `Natural`, or,
/// for the ones without a value of their own, the [`StealthPolicy`] switches it
/// on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dimension {
    /// User agent, platform, cores, memory, touch points, vendor and the like.
    Navigator,
    /// `navigator.language`, `navigator.languages` and `Accept-Language`.
    Localization,
    /// The time zone.
    Timezone,
    /// `screen.*`, the window size and the device pixel ratio.
    Screen,
    /// The WebGL vendor and renderer.
    Graphics,
    /// The installed fonts.
    Fonts,
    /// Camera, microphone and speaker lists.
    MediaDevices,
    /// The position `navigator.geolocation` reports.
    Geolocation,
    /// WebRTC address leaks.
    WebRtc,
    /// Canvas read-back noise.
    Canvas,
    /// Audio read-back noise.
    Audio,
    /// The Battery Status API.
    Battery,
    /// The Network Information API.
    Connection,
    /// `speechSynthesis.getVoices()`.
    Speech,
    /// User-Agent Client Hints.
    ClientHints,
    /// The proxy the browser is launched with.
    Proxy,
    /// Ports blocked on the loopback names.
    Ports,
    /// The tracker host block list.
    Trackers,
    /// The Web Bluetooth API.
    Bluetooth,
    /// The `window.chrome` stubs.
    ChromeRuntime,
    /// The headless fixes (permissions, matchMedia, hairline).
    Headless,
    /// `Function.prototype.toString` masking.
    NativeToString,
}

/// What a stable noise seed is derived from when nothing gives one.
const DEFAULT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// The audio seed is the canvas seed with this mask applied, so one `seed` gives
/// both noises and they differ from each other.
pub(crate) const AUDIO_SEED_MASK: u64 = 0x5DEE_CE66;

/// The data of a [`Fingerprint`]: one field per setting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
struct Fields {
    /// The browser the identity claims to be. `chromium` by default.
    browser_type: BrowserType,
    /// The operating system the identity claims. Taken from the user agent when
    /// the file does not say, and Windows when that names none.
    #[serde(skip_serializing_if = "Option::is_none")]
    os_type: Option<OsType>,
    /// The Chromium major version the identity claims, such as 155.
    #[serde(skip_serializing_if = "Option::is_none")]
    core_version: Option<u32>,
    /// The seed of the random draws and of the canvas and audio noise.
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,

    // Navigator
    /// `navigator.userAgent` and the `User-Agent` header.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    user_agent: Spoof<String>,
    /// `navigator.platform`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    platform: Spoof<String>,
    /// `navigator.hardwareConcurrency`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    hardware_concurrency: Spoof<u32>,
    /// `navigator.deviceMemory`, in gigabytes.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    device_memory: Spoof<f64>,
    /// `navigator.maxTouchPoints`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    max_touch_points: Spoof<u32>,
    /// `navigator.vendor`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    vendor: Spoof<String>,
    /// `navigator.oscpu` (Firefox persona).
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    oscpu: Spoof<String>,
    /// `navigator.productSub`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    product_sub: Spoof<String>,
    /// `navigator.buildID` (Firefox persona).
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    build_id: Spoof<String>,
    /// `navigator.appVersion`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    app_version: Spoof<String>,

    // Localization and time zone
    /// `navigator.language`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    locale: Spoof<String>,
    /// `navigator.languages`, comma-separated; quality values are ignored.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    languages: Spoof<String>,
    /// The `Accept-Language` header.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    accept_languages: Spoof<String>,
    /// The IANA time zone, such as `Europe/Berlin`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    timezone: Spoof<String>,

    // Screen
    /// `screen.width` and `screen.height`, in CSS pixels.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    screen: Spoof<ScreenSize>,
    /// `screen.availWidth` and `screen.availHeight`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    avail_screen: Spoof<ScreenSize>,
    /// `window.devicePixelRatio`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    pixel_ratio: Spoof<f64>,
    /// `screen.colorDepth`.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    color_depth: Spoof<u32>,

    // Everything else a page can probe
    /// The unmasked WebGL vendor and renderer.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    webgl: Spoof<WebGl>,
    /// What `enumerateDevices()` lists. `off` lists none.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    media_devices: Spoof<MediaDevices>,
    /// The position `navigator.geolocation` reports. `off` denies the request.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    geolocation: Spoof<Geolocation>,
    /// The installed fonts.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    fonts: Spoof<Vec<String>>,
    /// The proxy the browser is launched with. May hold a login.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    proxy: Spoof<ProxyConfig>,
    /// The addresses WebRTC reports. `off` makes WebRTC unavailable.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    webrtc: Spoof<WebRtcAddresses>,
    /// The Battery Status API. `off` makes it unavailable.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    battery: Spoof<BatteryProfile>,
    /// The Network Information API. `off` makes it unavailable.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    connection: Spoof<ConnectionProfile>,
    /// `speechSynthesis.getVoices()`. `off` lists none.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    speech_voices: Spoof<Vec<SpeechVoice>>,
    /// Web Bluetooth: `true` has an adapter, `false` has none, `off` makes the API
    /// unavailable.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    bluetooth: Spoof<bool>,
    /// User-Agent Client Hints (`navigator.userAgentData`). `off` makes them
    /// unavailable.
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    client_hints: Spoof<ClientHints>,

    // How the claims are delivered, and the profile's housekeeping
    /// How the claims are delivered and which protections run beside them.
    policy: StealthPolicy,
    /// Humanized input timing.
    humanize: HumanizeConfig,
    /// Whether `localStorage` is kept between sessions of the profile.
    local_storage: bool,
    /// Whether service workers are kept between sessions of the profile.
    save_service_worker: bool,
    /// The URLs the profile opens at start.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    custom_start_urls: Vec<String>,
    /// Extra browser command-line parameters.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    cmd_params: HashMap<String, String>,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            browser_type: BrowserType::default(),
            os_type: None,
            core_version: None,
            seed: None,
            user_agent: Spoof::Natural,
            platform: Spoof::Natural,
            hardware_concurrency: Spoof::Natural,
            device_memory: Spoof::Natural,
            max_touch_points: Spoof::Natural,
            vendor: Spoof::Natural,
            oscpu: Spoof::Natural,
            product_sub: Spoof::Natural,
            build_id: Spoof::Natural,
            app_version: Spoof::Natural,
            locale: Spoof::Natural,
            languages: Spoof::Natural,
            accept_languages: Spoof::Natural,
            timezone: Spoof::Natural,
            screen: Spoof::Natural,
            avail_screen: Spoof::Natural,
            pixel_ratio: Spoof::Natural,
            color_depth: Spoof::Natural,
            webgl: Spoof::Natural,
            media_devices: Spoof::Natural,
            geolocation: Spoof::Natural,
            fonts: Spoof::Natural,
            proxy: Spoof::Natural,
            webrtc: Spoof::Natural,
            battery: Spoof::Natural,
            connection: Spoof::Natural,
            speech_voices: Spoof::Natural,
            bluetooth: Spoof::Natural,
            client_hints: Spoof::Natural,
            policy: StealthPolicy::default(),
            humanize: HumanizeConfig::default(),
            local_storage: false,
            save_service_worker: false,
            custom_start_urls: Vec::new(),
            cmd_params: HashMap::new(),
        }
    }
}

impl Fields {
    /// The operating-system persona; Windows while none is set.
    const fn os(&self) -> OsType {
        match self.os_type {
            Some(os) => os,
            None => OsType::Windows,
        }
    }
}

/// Defines the read accessors of [`Fingerprint`], one per setting.
macro_rules! accessors {
    ($($(#[$doc:meta])* $name:ident: $ty:ty;)*) => {
        $(
            $(#[$doc])*
            #[must_use]
            pub const fn $name(&self) -> &$ty {
                &self.0.$name
            }
        )*
    };
}

/// Complete browser fingerprint / anti-detection profile.
///
/// Build one with [`Fingerprint::builder`] or take a preset such as
/// [`windows_desktop`](Fingerprint::windows_desktop). It cannot be changed
/// afterwards: the fields are private, and [`to_builder`](Fingerprint::to_builder)
/// starts a builder from it. A built fingerprint holds no
/// [`Random`](Spoof::Random): the builder drew those values.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::stealth::fingerprint::{Fingerprint, Spoof};
///
/// let fp = Fingerprint::windows_desktop();
/// assert_eq!(fp.screen().value().map(|s| s.width), Some(1920));
///
/// // Change one thing: take the builder back, change it, build again.
/// let wider = fp.to_builder().screen(2560, 1440).build()?;
/// assert_eq!(wider.screen().value().map(|s| s.width), Some(2560));
/// assert!(wider.hardware_concurrency().is_value());
/// # Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[cfg_attr(
    test,
    schemars(
        title = "Fingerprint",
        description = "What a browser claims to be, as seleniumbase-rs reads it: one entry per \
                       setting, each \"natural\" (left out), \"random\", \"off\" or a value. A \
                       setting that is left out is the browser's own."
    )
)]
#[serde(transparent)]
pub struct Fingerprint(Fields);

/// The fingerprint of a builder with nothing set: every setting `Natural`, with
/// the seed the builder gives it, so it equals `Fingerprint::builder().build()`.
impl Default for Fingerprint {
    fn default() -> Self {
        Self::builder()
            .build()
            .unwrap_or_else(|_| Self(Fields::default()))
    }
}

/// A fingerprint is read as the builder would build it: a `"random"` setting is
/// drawn, an `"off"` where it means nothing is an error, and a field this
/// version does not know is an error (the flat shape of the old versions is
/// one).
impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = Fields::deserialize(deserializer)?;
        FingerprintBuilder::from_fields(fields)
            .build()
            .map_err(serde::de::Error::custom)
    }
}

impl Fingerprint {
    /// Returns a builder for fluent construction.
    pub fn builder() -> FingerprintBuilder {
        FingerprintBuilder::default()
    }

    /// A builder that starts from this fingerprint, to change a setting and
    /// build another.
    #[must_use]
    pub fn to_builder(&self) -> FingerprintBuilder {
        FingerprintBuilder::from_fields(self.0.clone())
    }

    /// The browser persona.
    #[must_use]
    pub const fn browser_type(&self) -> BrowserType {
        self.0.browser_type
    }

    /// The operating-system persona.
    #[must_use]
    pub const fn os_type(&self) -> OsType {
        self.0.os()
    }

    /// The Chromium major version the identity claims, if it says.
    #[must_use]
    pub const fn core_version(&self) -> Option<u32> {
        self.0.core_version
    }

    /// The seed of the deterministic noise and of the random draws.
    ///
    /// It is the one given with [`seed`](FingerprintBuilder::seed). Without
    /// one, it is fresh entropy when a setting was drawn at random, and a value
    /// derived from the user agent otherwise, so the same persona gets the same
    /// noise on every launch.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.0.seed.unwrap_or(DEFAULT_SEED)
    }

    accessors! {
        /// `navigator.userAgent` and the `User-Agent` header.
        user_agent: Spoof<String>;
        /// `navigator.platform`.
        platform: Spoof<String>;
        /// `navigator.hardwareConcurrency`.
        hardware_concurrency: Spoof<u32>;
        /// `navigator.deviceMemory`.
        device_memory: Spoof<f64>;
        /// `navigator.maxTouchPoints`.
        max_touch_points: Spoof<u32>;
        /// `navigator.vendor`.
        vendor: Spoof<String>;
        /// `navigator.oscpu` (Firefox persona).
        oscpu: Spoof<String>;
        /// `navigator.productSub`.
        product_sub: Spoof<String>;
        /// `navigator.buildID` (Firefox persona).
        build_id: Spoof<String>;
        /// `navigator.appVersion`.
        app_version: Spoof<String>;
        /// `navigator.language`.
        locale: Spoof<String>;
        /// `navigator.languages`, comma-separated.
        languages: Spoof<String>;
        /// The `Accept-Language` header.
        accept_languages: Spoof<String>;
        /// The IANA time zone.
        timezone: Spoof<String>;
        /// `screen.width` and `screen.height`.
        screen: Spoof<ScreenSize>;
        /// `screen.availWidth` and `screen.availHeight`.
        avail_screen: Spoof<ScreenSize>;
        /// `window.devicePixelRatio`.
        pixel_ratio: Spoof<f64>;
        /// `screen.colorDepth`.
        color_depth: Spoof<u32>;
        /// The unmasked WebGL vendor and renderer.
        webgl: Spoof<WebGl>;
        /// What `enumerateDevices()` lists. `Off` lists none.
        media_devices: Spoof<MediaDevices>;
        /// The position `navigator.geolocation` reports. `Off` denies the request.
        geolocation: Spoof<Geolocation>;
        /// The installed fonts.
        fonts: Spoof<Vec<String>>;
        /// The proxy the browser is launched with.
        proxy: Spoof<ProxyConfig>;
        /// The addresses WebRTC reports. `Off` makes WebRTC unavailable.
        webrtc: Spoof<WebRtcAddresses>;
        /// The Battery Status API. `Off` makes it unavailable.
        battery: Spoof<BatteryProfile>;
        /// The Network Information API. `Off` makes it unavailable.
        connection: Spoof<ConnectionProfile>;
        /// `speechSynthesis.getVoices()`. `Off` lists none.
        speech_voices: Spoof<Vec<SpeechVoice>>;
        /// Web Bluetooth: `Value(true)` has an adapter, `Value(false)` has none,
        /// `Off` makes the API unavailable.
        bluetooth: Spoof<bool>;
        /// User-Agent Client Hints. `Off` makes `navigator.userAgentData` unavailable.
        client_hints: Spoof<ClientHints>;
        /// How the claims are delivered and which protections run beside them.
        policy: StealthPolicy;
        /// Humanized input timing.
        humanize: HumanizeConfig;
        /// The URLs the profile opens at start.
        custom_start_urls: Vec<String>;
        /// Extra browser command-line parameters.
        cmd_params: HashMap<String, String>;
    }

    /// Whether `localStorage` is kept between sessions of the profile.
    #[must_use]
    pub const fn local_storage(&self) -> bool {
        self.0.local_storage
    }

    /// Whether service workers are kept between sessions of the profile.
    #[must_use]
    pub const fn save_service_worker(&self) -> bool {
        self.0.save_service_worker
    }

    /// The tracker host block list, defaulting to [`default_tracker_hosts`]
    /// when the policy gives none.
    pub fn tracker_hosts(&self) -> Vec<String> {
        if self.0.policy.blocked_trackers.is_empty() {
            default_tracker_hosts()
        } else {
            self.0.policy.blocked_trackers.clone()
        }
    }

    /// Whether this fingerprint spoofs `dimension`.
    ///
    /// This is the one predicate the page-script providers, the launch
    /// arguments, the protocol overrides and the proxy all ask. A dimension is
    /// on when a setting that belongs to it is not
    /// [`Natural`](Spoof::Natural); the few dimensions without a setting of
    /// their own follow the [`StealthPolicy`] (`NativeToString` is on unless
    /// the policy switches it off).
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::stealth::fingerprint::{Dimension, Fingerprint};
    ///
    /// let fp = Fingerprint::builder().timezone("Europe/Berlin").build()?;
    /// assert!(fp.is_masked(Dimension::Timezone));
    /// assert!(!fp.is_masked(Dimension::Screen));
    /// # Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
    /// ```
    #[must_use]
    pub fn is_masked(&self, dimension: Dimension) -> bool {
        let f = &self.0;
        match dimension {
            Dimension::Navigator => [
                f.user_agent.is_set(),
                f.platform.is_set(),
                f.hardware_concurrency.is_set(),
                f.device_memory.is_set(),
                f.max_touch_points.is_set(),
                f.vendor.is_set(),
                f.oscpu.is_set(),
                f.product_sub.is_set(),
                f.build_id.is_set(),
                f.app_version.is_set(),
            ]
            .contains(&true),
            Dimension::Localization => {
                f.locale.is_set() || f.languages.is_set() || f.accept_languages.is_set()
            }
            Dimension::Timezone => f.timezone.is_set(),
            Dimension::Screen => {
                f.screen.is_set()
                    || f.avail_screen.is_set()
                    || f.pixel_ratio.is_set()
                    || f.color_depth.is_set()
            }
            Dimension::Graphics => f.webgl.is_set(),
            Dimension::Fonts => f.fonts.is_set(),
            Dimension::MediaDevices => f.media_devices.is_set(),
            Dimension::Geolocation => f.geolocation.is_set(),
            Dimension::WebRtc => f.webrtc.is_set() || f.policy.webrtc_policy.is_some(),
            Dimension::Canvas => f.policy.canvas_noise.adds_noise(),
            Dimension::Audio => f.policy.audio_noise.adds_noise(),
            Dimension::Battery => f.battery.is_set(),
            Dimension::Connection => f.connection.is_set(),
            Dimension::Speech => f.speech_voices.is_set(),
            Dimension::ClientHints => f.client_hints.is_set(),
            Dimension::Proxy => f.proxy.is_value(),
            Dimension::Ports => !f.policy.blocked_ports.is_empty(),
            Dimension::Trackers => f.policy.block_trackers || !f.policy.blocked_trackers.is_empty(),
            Dimension::Bluetooth => f.bluetooth.is_set(),
            Dimension::ChromeRuntime => f.policy.chrome_runtime,
            Dimension::Headless => f.policy.headless_fixes,
            Dimension::NativeToString => f.policy.native_tostring,
        }
    }

    /// Validates cross-field coherence and returns a [`CoherenceReport`].
    ///
    /// This catches common mismatches such as a macOS user-agent paired with a
    /// `Win32` platform, or a mobile user-agent without touch points, that make
    /// an otherwise convincing profile easy to detect. Only values that were
    /// given are checked; a `Natural` setting is the browser's own.
    pub fn validate(&self) -> CoherenceReport {
        let mut report = CoherenceReport::default();
        let f = &self.0;
        let ua = f.user_agent.as_str().unwrap_or("");
        let ua_lower = ua.to_lowercase();

        let os = f.os();
        let platform = f.platform.as_str().unwrap_or_else(|| os.platform());

        // OS persona vs navigator.platform.
        let platform_ok = match os {
            OsType::Windows => platform.starts_with("Win"),
            OsType::Macos => platform.contains("Mac"),
            OsType::Linux => platform.contains("Linux") && !platform.contains("arm"),
            OsType::Android => platform.contains("arm") || platform.contains("Linux"),
            OsType::Ios => {
                platform == "iPhone" || platform == "iPad" || platform.contains("iPhone")
            }
        };
        if !platform_ok {
            report.errors.push(format!(
                "navigator.platform '{platform}' does not match OS persona {os:?}"
            ));
        }

        // OS persona vs user-agent token.
        if !ua.is_empty() {
            let ua_os_ok = match os {
                OsType::Windows => ua_lower.contains("windows"),
                OsType::Macos => ua_lower.contains("mac os"),
                OsType::Linux => ua_lower.contains("linux") || ua_lower.contains("x11"),
                OsType::Android => ua_lower.contains("android"),
                OsType::Ios => ua_lower.contains("iphone") || ua_lower.contains("ipad"),
            };
            if !ua_os_ok {
                report.errors.push(format!(
                    "user-agent does not contain a token for OS persona {os:?}"
                ));
            }
        }

        // Mobile user-agents need touch points.
        let touch_points = f.max_touch_points.value().copied().unwrap_or(0);
        let is_mobile_ua = ua_lower.contains("mobile")
            || ua_lower.contains("android")
            || ua_lower.contains("iphone")
            || ua_lower.contains("ipad");
        if is_mobile_ua && touch_points == 0 {
            report
                .warnings
                .push("mobile user-agent but maxTouchPoints is 0".to_owned());
        }
        if !is_mobile_ua && touch_points > 0 {
            report
                .warnings
                .push("desktop user-agent but maxTouchPoints is greater than 0".to_owned());
        }

        // Locale vs primary language.
        if let (Some(locale), Some(langs)) = (f.locale.as_str(), f.languages.as_str()) {
            let primary = langs.split(',').next().unwrap_or("").trim();
            if !primary.is_empty() && !primary.eq_ignore_ascii_case(locale) {
                report.warnings.push(format!(
                    "locale '{locale}' differs from primary language '{primary}'"
                ));
            }
        }

        // A time zone but no position, while the policy grants the geolocation
        // permission: a page is then handed the real position next to a made-up
        // time zone. Without the grant a request is only asked, as a real
        // browser asks it, so there is nothing to warn about (the presets leave
        // the position natural for that reason).
        if f.timezone.is_value() && f.geolocation.is_natural() && f.policy.grant_permissions {
            report.warnings.push(
                "timezone set without geolocation while permissions are granted; \
                 pages get the real position, consider matching coordinates"
                    .to_owned(),
            );
        }

        // Screen sanity.
        if let Some(size) = f.screen.value() {
            if size.width == 0 || size.height == 0 {
                report
                    .errors
                    .push("screen dimensions must be non-zero".to_owned());
            }
        }

        self.check_graphics(&mut report);
        self.check_browser_persona(ua, &mut report);
        self.check_place(&mut report);

        report
    }

    /// The WebGL strings against the operating system: each graphics API is
    /// the one a single family of systems reports, so a renderer from another
    /// family gives the persona away.
    fn check_graphics(&self, report: &mut CoherenceReport) {
        let Some(webgl) = self.0.webgl.value() else {
            return;
        };
        let named = format!(
            "{} {}",
            webgl.vendor.as_deref().unwrap_or(""),
            webgl.renderer.as_deref().unwrap_or("")
        );
        // (what the strings name, the API's name, the systems that report it)
        let families: [(&[&str], &str, &[OsType]); 4] = [
            (&["Direct3D", "D3D11"], "Direct3D", &[OsType::Windows]),
            (
                &["Metal", "Apple"],
                "Apple GPU or Metal",
                &[OsType::Macos, OsType::Ios],
            ),
            (&["Mesa"], "Mesa", &[OsType::Linux]),
            (
                &["Adreno", "Mali", "PowerVR"],
                "mobile GPU",
                &[OsType::Android],
            ),
        ];
        for (needles, api, systems) in families {
            if !needles.iter().any(|n| named.contains(n)) {
                continue;
            }
            if !systems.contains(&self.0.os()) {
                report.errors.push(format!(
                    "WebGL renderer '{}' is a {api} renderer, which {:?} does not report",
                    webgl.renderer.as_deref().unwrap_or(""),
                    self.0.os()
                ));
            }
        }
    }

    /// The browser persona against its user agent.
    fn check_browser_persona(&self, ua: &str, report: &mut CoherenceReport) {
        if self.0.browser_type == BrowserType::MobileSafari && ua.contains("Chrome/") {
            report.errors.push(
                "browser persona MobileSafari but the user agent has a Chrome/ token".to_owned(),
            );
        }
    }

    /// The locale against the time zone: a locale is spoken in its own country,
    /// so a time zone the table of places puts in another one is a tell.
    fn check_place(&self, report: &mut CoherenceReport) {
        let (Some(locale), Some(zone)) = (self.0.locale.as_str(), self.0.timezone.as_str()) else {
            return;
        };
        let region = locale
            .rsplit_once('-')
            .map(|(_, region)| region)
            .filter(|region| region.len() == 2);
        let country = randomize::locale_of_timezone(zone).map(|l| l.country);
        if let (Some(region), Some(country)) = (region, country) {
            if !region.eq_ignore_ascii_case(country) {
                report.warnings.push(format!(
                    "locale '{locale}' is not spoken in the country of time zone '{zone}' ({country})"
                ));
            }
        }
    }

    /// Generates a random but coherent identity for `os`, fully determined by
    /// `seed`.
    ///
    /// Randomising fields one at a time yields the mismatches detectors look
    /// for (a Mac with a Direct3D renderer, a Tokyo time zone under an `en-US`
    /// locale), so this draws a whole machine and a whole place and derives
    /// every field from them. It is the builder with these settings
    /// [`Random`](Spoof::Random): user agent, platform, cores, memory, touch
    /// points, vendor, locale, languages, time zone, position, screen, pixel
    /// ratio, colour depth, WebGL, Client Hints, battery, network information,
    /// speech voices and Bluetooth, with [`StealthPolicy::balanced`].
    ///
    /// - **Graphics**: WebGL vendor and renderer strings (and PCI ids) that the
    ///   OS really reports: ANGLE over Direct3D 11 on Windows, ANGLE over Metal
    ///   with an Apple chip on macOS, ANGLE over Mesa or the NVIDIA driver on
    ///   Linux, a Mali or Adreno chip on Android, and `Apple GPU` on iOS.
    /// - **Hardware**: core count, device memory, screen size, colour depth and
    ///   device pixel ratio that machine ships with. A Mac gets its chip's core
    ///   count and its model's screen; a PC's class (integrated, mainstream,
    ///   enthusiast) decides its cores, memory and screens.
    /// - **Place**: a locale, its language list, a time zone it is spoken in, and
    ///   coordinates inside that time zone's main city, so locale, time zone and
    ///   geolocation agree with each other.
    /// - **Software**: a user agent, `navigator.platform`, `navigator.vendor` and
    ///   Client Hints (built the way Chromium builds them) that agree with the OS
    ///   and with the claimed Chromium major version.
    ///
    /// The result passes [`validate`](Fingerprint::validate) without errors or
    /// warnings for every seed the tests try: 1,500 per OS, and edge cases such
    /// as `0` and `u64::MAX`. The noise [`seed`](Fingerprint::seed) is `seed`.
    ///
    /// The same `seed` gives the same identity within one version of this crate.
    /// Adding or changing a table row changes what a seed maps to, so persist
    /// the generated [`Fingerprint`] (it implements `Serialize`), not the seed,
    /// if an identity must outlive an upgrade.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::{Fingerprint, OsType};
    ///
    /// let fp = Fingerprint::randomized(OsType::Macos, 7);
    ///
    /// // The same seed always gives the same identity.
    /// assert_eq!(fp, Fingerprint::randomized(OsType::Macos, 7));
    ///
    /// // It is coherent, with nothing even to warn about.
    /// let report = fp.validate();
    /// assert!(report.is_coherent() && report.warnings.is_empty());
    ///
    /// // A Mac reports an Apple GPU through Metal, never Direct3D.
    /// let renderer = fp
    ///     .webgl()
    ///     .value()
    ///     .and_then(|gl| gl.renderer.as_deref())
    ///     .unwrap_or_default();
    /// assert!(renderer.contains("Metal") && !renderer.contains("Direct3D"));
    ///
    /// // Another seed is another machine.
    /// assert_ne!(fp, Fingerprint::randomized(OsType::Macos, 8));
    /// ```
    ///
    /// # Limitations
    ///
    /// - The claimed Chromium major version comes from a short, fixed window of
    ///   recent releases. Pair the identity with a browser of that era, or
    ///   change `user_agent`, `client_hints` and `core_version` together.
    /// - Client Hints report full versions as `major.0.0.0`, the form the
    ///   user agent itself uses.
    /// - Fonts, media devices and WebRTC addresses are left alone, and the proxy
    ///   is left off. Ask for them with [`Spoof::Random`] on a builder.
    /// - It describes an identity, not a browser. Give it to a browser with
    ///   [`BrowserConfig::with_fingerprint`](crate::BrowserConfig::with_fingerprint)
    ///   (WebDriver and UC sessions), with
    ///   [`LaunchOptionsBuilder::fingerprint`](crate::sb_cdp::LaunchOptionsBuilder::fingerprint)
    ///   (the pure CDP engine, for every tab), or with the evasion registry. The
    ///   browser itself stays what it is: a fingerprint of another operating
    ///   system does not change the binary's real behaviour.
    #[must_use]
    pub fn randomized(os: OsType, seed: u64) -> Self {
        Self::randomized_builder(os, seed, None)
    }

    /// Like [`Fingerprint::randomized`], but with the Chromium major fixed to `major`, so the
    /// user agent and Client Hints claim the release that the measured TLS and HTTP/2 of the
    /// identity come from. Safari and iOS have no Chromium major and ignore `major`.
    #[must_use]
    pub fn randomized_with_major(os: OsType, seed: u64, major: u32) -> Self {
        Self::randomized_builder(os, seed, Some(major))
    }

    fn randomized_builder(os: OsType, seed: u64, major: Option<u32>) -> Self {
        let mut builder = Self::builder().os_type(os).seed(seed);
        if let Some(major) = major.filter(|_| os != OsType::Ios) {
            builder = builder.core_version(major);
        }
        builder
            .user_agent(Spoof::Random)
            .platform(Spoof::Random)
            .hardware_concurrency(Spoof::Random)
            .device_memory(Spoof::Random)
            .max_touch_points(Spoof::Random)
            .vendor(Spoof::Random)
            .locale(Spoof::Random)
            .languages(Spoof::Random)
            .accept_languages(Spoof::Random)
            .timezone(Spoof::Random)
            .geolocation(Spoof::Random)
            .screen_spoof(Spoof::Random)
            .pixel_ratio(Spoof::Random)
            .color_depth(Spoof::Random)
            .webgl(Spoof::Random)
            .client_hints(Spoof::Random)
            .battery(Spoof::Random)
            .connection(Spoof::Random)
            .speech_voices(Spoof::Random)
            .bluetooth(Spoof::Random)
            .policy(StealthPolicy::balanced())
            .build()
            .expect("every setting of a drawn persona has a value to draw, and none is Off")
    }

    /// The settings that go with a machine of `os` in every preset: a battery,
    /// network information, speech voices and Web Bluetooth where the browser
    /// has them, and none where it has not.
    fn with_machine_extras(builder: FingerprintBuilder, os: OsType) -> FingerprintBuilder {
        if os == OsType::Ios {
            return builder
                .battery(Spoof::Off)
                .connection(Spoof::Off)
                .bluetooth(Spoof::Off)
                .speech_voices(randomize::speech_voices_for(os));
        }
        builder
            .battery(BatteryProfile::default())
            .connection(ConnectionProfile::default())
            .speech_voices(randomize::speech_voices_for(os))
            .bluetooth(matches!(os, OsType::Android | OsType::Macos))
    }

    /// Quick preset for a Windows desktop Chrome profile.
    ///
    /// The Chrome presets claim
    /// [`PRESET_CHROMIUM_MAJOR`](super::randomize::PRESET_CHROMIUM_MAJOR), the
    /// newest major a randomized identity may claim, in Chrome's frozen user-agent
    /// format. They set no Client Hints; the evasions derive those from the user
    /// agent. Every preset is built with [`StealthPolicy::balanced`] and passes
    /// [`validate`](Fingerprint::validate) without a warning.
    pub fn windows_desktop() -> Self {
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .os_type(OsType::Windows)
                .user_agent(preset_user_agent("Windows NT 10.0; Win64; x64", false))
                .platform("Win32")
                .screen(1920, 1080)
                .hardware_concurrency(8)
                .device_memory(8.0)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/New_York")
                .webgl((
                    "Google Inc. (NVIDIA)",
                    "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)",
                )),
            OsType::Windows,
        ))
    }

    /// Quick preset for a Windows desktop Microsoft Edge profile.
    ///
    /// The Windows Chrome preset with Edge's `Edg/` token in the user agent. The
    /// Client Hints name `Microsoft Edge` instead of `Google Chrome`: the
    /// evasions derive them from the user agent, as they do for the Chrome
    /// presets. [`fingerprint!(edge)`](crate::fingerprint) is this preset.
    pub fn edge_desktop() -> Self {
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .os_type(OsType::Windows)
                .user_agent(randomize::edge_user_agent(
                    "Windows NT 10.0; Win64; x64",
                    randomize::PRESET_CHROMIUM_MAJOR,
                ))
                .platform("Win32")
                .screen(1920, 1080)
                .hardware_concurrency(8)
                .device_memory(8.0)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/New_York")
                .webgl((
                    "Google Inc. (NVIDIA)",
                    "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)",
                )),
            OsType::Windows,
        ))
    }

    /// Quick preset for a macOS desktop Chrome profile.
    pub fn macos_desktop() -> Self {
        let webgl = randomize::macos_webgl("Apple M3");
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .os_type(OsType::Macos)
                .user_agent(preset_user_agent(
                    "Macintosh; Intel Mac OS X 10_15_7",
                    false,
                ))
                .platform("MacIntel")
                .screen(1920, 1080)
                .hardware_concurrency(8)
                .device_memory(8.0)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/Los_Angeles")
                .webgl(webgl),
            OsType::Macos,
        ))
    }

    /// Quick preset for a Linux desktop Chrome profile.
    ///
    /// Its WebGL strings are worded the way Chrome words them on Linux, as a
    /// generated Linux identity words them: ANGLE over desktop OpenGL.
    pub fn linux_desktop() -> Self {
        let webgl = randomize::linux_preset_webgl();
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .os_type(OsType::Linux)
                .user_agent(preset_user_agent("X11; Linux x86_64", false))
                .platform("Linux x86_64")
                .screen(1920, 1080)
                .hardware_concurrency(8)
                .device_memory(8.0)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/Chicago")
                .webgl(webgl),
            OsType::Linux,
        ))
    }

    /// Quick preset for an Android mobile Chrome profile.
    pub fn android_mobile() -> Self {
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .os_type(OsType::Android)
                // Chrome on Android freezes the version and hides the model in the
                // user agent; the model is only in the Client Hints.
                .user_agent(preset_user_agent("Linux; Android 10; K", true))
                .platform("Linux armv8l")
                .screen(412, 915)
                .pixel_ratio(3.0)
                .hardware_concurrency(8)
                .max_touch_points(5)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/New_York")
                .webgl(("Qualcomm", "Adreno (TM) 740")),
            OsType::Android,
        ))
    }

    /// Quick preset for an iPhone / Mobile Safari profile.
    ///
    /// Safari 26.0 on iOS 26, whose user agent names the frozen `iPhone OS 18_6`
    /// (WebKit bug 298717); see the Safari table in [`super::randomize`]. Safari
    /// has no battery, network information, Web Bluetooth, Client Hints or
    /// `navigator.deviceMemory`, so the first four are `Off` and the last is
    /// left out.
    pub fn ios_mobile_safari() -> Self {
        Self::preset(Self::with_machine_extras(
            Self::builder()
                .browser_type(BrowserType::MobileSafari)
                .os_type(OsType::Ios)
                .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1")
                .platform("iPhone")
                .screen(390, 844)
                .pixel_ratio(3.0)
                .hardware_concurrency(4)
                .max_touch_points(5)
                .locale("en-US")
                .languages("en-US,en;q=0.9")
                .timezone("America/New_York")
                .webgl(("Apple Inc.", "Apple GPU"))
                .vendor("Apple Computer, Inc.")
                // Safari has neither Client Hints nor `navigator.deviceMemory`.
                .client_hints(Spoof::Off),
            OsType::Ios,
        ))
    }

    /// Builds a preset: the balanced policy, and nothing in it can fail.
    fn preset(builder: FingerprintBuilder) -> Self {
        builder
            .policy(StealthPolicy::balanced())
            .build()
            .expect("a preset gives values only, so it always builds")
    }
}

/// The user agent of a Chrome preset: Chrome's frozen format on `platform`,
/// claiming [`PRESET_CHROMIUM_MAJOR`](super::randomize::PRESET_CHROMIUM_MAJOR).
fn preset_user_agent(platform: &str, mobile: bool) -> String {
    randomize::chromium_user_agent(platform, randomize::PRESET_CHROMIUM_MAJOR, mobile)
}

/// A change to the policy, applied last by [`FingerprintBuilder::build`].
#[derive(Clone)]
struct PolicyChange(Arc<dyn Fn(StealthPolicy) -> StealthPolicy + Send + Sync>);

impl fmt::Debug for PolicyChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PolicyChange")
    }
}

/// Defines the builder methods of the settings that take any
/// `impl Into<Spoof<T>>`.
macro_rules! setters {
    ($($(#[$doc:meta])* $name:ident: $ty:ty;)*) => {
        $(
            $(#[$doc])*
            #[must_use]
            pub fn $name(mut self, value: impl Into<Spoof<$ty>>) -> Self {
                self.fields.$name = value.into();
                self
            }
        )*
    };
}

/// Fluent builder for [`Fingerprint`].
///
/// Each setting takes anything that converts to its [`Spoof`]: a plain value is
/// [`Value`](Spoof::Value), and [`Random`](Spoof::Random),
/// [`Natural`](Spoof::Natural) and [`Off`](Spoof::Off) are named. The calls can
/// come in any order, and a later call for the same setting replaces an earlier
/// one. [`build`](Self::build) draws what was asked for at random, checks that
/// every `Off` means something, and returns the fingerprint.
///
/// # Random
///
/// `build` works out the operating system (the explicit one, else the one the
/// user agent names, else Windows) and the place (the region of a given time
/// zone, else of a given locale, else a drawn one), then takes every `Random`
/// setting from one seeded draw of that machine and that place. A random locale
/// agrees with a given time zone, a random position is in that time zone's main
/// city, a random graphics adapter is one the operating system reports. When
/// nothing coherent can be drawn, `build` fails and names the setting.
///
/// [`seed`](Self::seed) makes the draw repeatable. Without it the draw uses
/// fresh entropy, and the seed it used is kept in the fingerprint.
///
/// # Policy
///
/// How the claims are delivered is the [`StealthPolicy`]. [`policy`](Self::policy)
/// gives the base, and [`with_policy`](Self::with_policy) and the shortcuts
/// ([`canvas_noise`](Self::canvas_noise), [`grant_permissions`](Self::grant_permissions)
/// and the others) change it. They are applied after the base, in the order they
/// were called, whatever their order against `policy`. A policy never changes a
/// setting.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::stealth::fingerprint::{
///     Dimension, Fingerprint, Spoof, StealthPolicy,
/// };
///
/// let fp = Fingerprint::builder()
///     .timezone("Europe/Berlin")
///     .hardware_concurrency(8)
///     .webrtc(Spoof::Off)
///     .policy(StealthPolicy::native_only())
///     .grant_permissions(true)
///     .build()?;
/// assert!(fp.is_masked(Dimension::Timezone) && fp.is_masked(Dimension::Navigator));
/// assert!(fp.policy().native_spoofing && fp.policy().grant_permissions);
///
/// // An `Off` where it means nothing is an error that names the setting.
/// let refused = Fingerprint::builder().timezone(Spoof::Off).build();
/// assert!(refused.is_err());
/// # Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct FingerprintBuilder {
    fields: Fields,
    /// Whether [`browser_type`](Self::browser_type) was called.
    browser_type_set: bool,
    seed: Option<u64>,
    /// The base policy given with [`policy`](Self::policy).
    policy: StealthPolicy,
    /// The changes given with [`with_policy`](Self::with_policy) and the
    /// shortcuts, in order.
    changes: Vec<PolicyChange>,
}

impl FingerprintBuilder {
    /// A builder that starts from `fields`, which are kept as they are.
    fn from_fields(fields: Fields) -> Self {
        Self {
            seed: fields.seed,
            policy: fields.policy.clone(),
            browser_type_set: true,
            fields,
            changes: Vec::new(),
        }
    }

    /// Leaves every setting that asked to be drawn at random to the browser:
    /// a `Random` becomes `Natural`. The payload conversion falls back to it
    /// when a random value cannot be drawn.
    pub(crate) fn without_random(mut self) -> Self {
        fn demote<T>(setting: &mut Spoof<T>) {
            if setting.is_random() {
                *setting = Spoof::Natural;
            }
        }
        let f = &mut self.fields;
        demote(&mut f.user_agent);
        demote(&mut f.platform);
        demote(&mut f.hardware_concurrency);
        demote(&mut f.device_memory);
        demote(&mut f.max_touch_points);
        demote(&mut f.vendor);
        demote(&mut f.oscpu);
        demote(&mut f.product_sub);
        demote(&mut f.build_id);
        demote(&mut f.app_version);
        demote(&mut f.locale);
        demote(&mut f.languages);
        demote(&mut f.accept_languages);
        demote(&mut f.timezone);
        demote(&mut f.screen);
        demote(&mut f.avail_screen);
        demote(&mut f.pixel_ratio);
        demote(&mut f.color_depth);
        demote(&mut f.webgl);
        demote(&mut f.media_devices);
        demote(&mut f.geolocation);
        demote(&mut f.fonts);
        demote(&mut f.proxy);
        demote(&mut f.webrtc);
        demote(&mut f.battery);
        demote(&mut f.connection);
        demote(&mut f.speech_voices);
        demote(&mut f.bluetooth);
        demote(&mut f.client_hints);
        self
    }

    /// Sets the browser persona. When it is not set, [`build`](Self::build)
    /// takes it from a random user agent, and it is Chromium otherwise.
    #[must_use]
    pub fn browser_type(mut self, v: BrowserType) -> Self {
        self.fields.browser_type = v;
        self.browser_type_set = true;
        self
    }

    /// Sets the operating-system persona. When it is not set, [`build`](Self::build)
    /// takes it from a recognised user agent, and it is Windows otherwise.
    #[must_use]
    pub fn os_type(mut self, v: OsType) -> Self {
        self.fields.os_type = Some(v);
        self
    }

    /// Sets the Chromium major version the identity claims. A random user agent
    /// and random Client Hints claim it too.
    #[must_use]
    pub fn core_version(mut self, v: u32) -> Self {
        self.fields.core_version = Some(v);
        self
    }

    /// Sets the seed of the random draws and of the deterministic noise, so the
    /// same calls give the same fingerprint.
    #[must_use]
    pub fn seed(mut self, v: u64) -> Self {
        self.seed = Some(v);
        self
    }

    setters! {
        /// Sets the user agent.
        user_agent: String;
        /// Sets `navigator.platform`.
        platform: String;
        /// Sets `navigator.hardwareConcurrency`.
        hardware_concurrency: u32;
        /// Sets `navigator.deviceMemory`.
        device_memory: f64;
        /// Sets `navigator.maxTouchPoints`.
        max_touch_points: u32;
        /// Sets `navigator.vendor`.
        vendor: String;
        /// Sets `navigator.oscpu` (Firefox persona). It has no random value.
        oscpu: String;
        /// Sets `navigator.productSub`.
        product_sub: String;
        /// Sets `navigator.buildID` (Firefox persona). It has no random value.
        build_id: String;
        /// Sets `navigator.appVersion`.
        app_version: String;
        /// Sets the locale (`navigator.language`). Without
        /// [`languages`](Self::languages), the language list is derived from it.
        locale: String;
        /// Sets `navigator.languages` as a comma-separated list.
        languages: String;
        /// Sets the `Accept-Language` header.
        accept_languages: String;
        /// Sets the IANA time zone.
        timezone: String;
        /// Sets the available screen size (`screen.availWidth` and
        /// `availHeight`). `(width, height)` works.
        avail_screen: ScreenSize;
        /// Sets the device pixel ratio.
        pixel_ratio: f64;
        /// Sets the colour depth.
        color_depth: u32;
        /// Sets the WebGL vendor and renderer. `(vendor, renderer)` works.
        webgl: WebGl;
        /// Sets what `enumerateDevices()` lists. `(audio_in, audio_out, video_in)`
        /// and a `Vec` of labelled devices work. `Off` lists none.
        media_devices: MediaDevices;
        /// Sets the position `navigator.geolocation` reports. `(latitude,
        /// longitude)` works. `Off` denies the request; a position answers it
        /// without a prompt. A `Random` position is in the city of the given
        /// time zone or locale; with neither it is in a drawn city, and the
        /// browser's own time zone does not follow it, so draw the
        /// [`timezone`](Self::timezone) with it (`Spoof::Random`).
        geolocation: Geolocation;
        /// Sets the installed fonts.
        fonts: Vec<String>;
        /// Sets the proxy the browser is launched with. It has no random value.
        proxy: ProxyConfig;
        /// Sets the addresses WebRTC reports and turns its leak filter on.
        /// `(public_ip, local_ip)` works. `Off` makes WebRTC unavailable.
        webrtc: WebRtcAddresses;
        /// Sets the Battery Status API profile. `Off` makes it unavailable.
        battery: BatteryProfile;
        /// Sets the Network Information API profile. `Off` makes it unavailable.
        connection: ConnectionProfile;
        /// Sets the synthesized speech voices. `Off` lists none.
        speech_voices: Vec<SpeechVoice>;
        /// Sets Web Bluetooth: `true` has an adapter, `false` has none, `Off`
        /// makes the API unavailable.
        bluetooth: bool;
        /// Sets the User-Agent Client Hints. `Off` makes `navigator.userAgentData`
        /// unavailable.
        client_hints: ClientHints;
    }

    /// Sets the screen size in CSS pixels: the common case of
    /// [`screen_spoof`](Self::screen_spoof).
    #[must_use]
    pub fn screen(mut self, width: u32, height: u32) -> Self {
        self.fields.screen = Spoof::Value(ScreenSize::new(width, height));
        self
    }

    /// Sets the screen size from any [`Spoof<ScreenSize>`]: `Random` draws the
    /// screen of the drawn machine.
    #[must_use]
    pub fn screen_spoof(mut self, value: impl Into<Spoof<ScreenSize>>) -> Self {
        self.fields.screen = value.into();
        self
    }

    /// Sets how many microphones, speakers and cameras `enumerateDevices()`
    /// lists: the common case of [`media_devices`](Self::media_devices).
    #[must_use]
    pub fn media_device_counts(self, audio_in: u32, audio_out: u32, video_in: u32) -> Self {
        self.media_devices(MediaDevices::counts(audio_in, audio_out, video_in))
    }

    /// Keeps `localStorage` between sessions of the profile.
    #[must_use]
    pub fn local_storage(mut self, v: bool) -> Self {
        self.fields.local_storage = v;
        self
    }

    /// Keeps service workers between sessions of the profile.
    #[must_use]
    pub fn save_service_worker(mut self, v: bool) -> Self {
        self.fields.save_service_worker = v;
        self
    }

    /// Sets the URLs the profile opens at start.
    #[must_use]
    pub fn custom_start_urls(mut self, v: Vec<String>) -> Self {
        self.fields.custom_start_urls = v;
        self
    }

    /// Sets extra browser command-line parameters.
    #[must_use]
    pub fn cmd_params(mut self, v: HashMap<String, String>) -> Self {
        self.fields.cmd_params = v;
        self
    }

    /// Sets the humanized input timing configuration.
    #[must_use]
    pub fn humanize(mut self, v: HumanizeConfig) -> Self {
        self.fields.humanize = v;
        self
    }

    /// Sets the base [`StealthPolicy`], such as [`StealthPolicy::balanced`].
    ///
    /// It is a base: the order of this call against the settings does not
    /// matter, and [`with_policy`](Self::with_policy) and the shortcuts apply on
    /// top of it whenever they were called.
    #[must_use]
    pub fn policy(mut self, base: StealthPolicy) -> Self {
        self.policy = base;
        self
    }

    /// Changes the policy after the base has been set. Several calls, and the
    /// shortcuts, run in the order they were made.
    #[must_use]
    pub fn with_policy(
        mut self,
        change: impl Fn(StealthPolicy) -> StealthPolicy + Send + Sync + 'static,
    ) -> Self {
        self.changes.push(PolicyChange(Arc::new(change)));
        self
    }

    /// Sets the WebRTC IP-handling policy the browser is launched with; `None`
    /// passes no flag.
    #[must_use]
    pub fn webrtc_policy(self, policy: impl Into<Option<WebRtcPolicy>>) -> Self {
        let policy = policy.into();
        self.with_policy(move |p| p.webrtc_policy(policy))
    }

    /// Sets the canvas read-back noise.
    #[must_use]
    pub fn canvas_noise(self, noise: CanvasNoise) -> Self {
        self.with_policy(move |p| p.canvas_noise(noise))
    }

    /// Sets the audio read-back noise.
    #[must_use]
    pub fn audio_noise(self, noise: AudioNoise) -> Self {
        self.with_policy(move |p| p.audio_noise(noise))
    }

    /// Delivers the claims through CDP and launch arguments instead of
    /// JavaScript where the browser can, so a page finds no override to inspect.
    #[must_use]
    pub fn native_spoofing(self, on: bool) -> Self {
        self.with_policy(move |p| p.native_spoofing(on))
    }

    /// Grants common permissions at start, so no permission prompt fires.
    #[must_use]
    pub fn grant_permissions(self, on: bool) -> Self {
        self.with_policy(move |p| p.grant_permissions(on))
    }

    /// Blocks the default tracker hosts, or stops blocking.
    #[must_use]
    pub fn block_trackers(self, on: bool) -> Self {
        self.with_policy(move |p| p.block_trackers(on))
    }

    /// Blocks these hosts instead of the default ones.
    #[must_use]
    pub fn blocked_trackers(self, hosts: Vec<String>) -> Self {
        self.with_policy(move |p| p.blocked_trackers(hosts.clone()))
    }

    /// Sets the ports the browser may not reach on the loopback names.
    #[must_use]
    pub fn blocked_ports(self, ports: Vec<u16>) -> Self {
        self.with_policy(move |p| p.blocked_ports(ports.clone()))
    }

    /// Bypasses Content-Security-Policy for the page context, or not.
    #[must_use]
    pub fn disable_csp(self, on: bool) -> Self {
        self.with_policy(move |p| p.disable_csp(on))
    }

    /// Sets how QUIC is handled.
    #[must_use]
    pub fn quic(self, mode: QuicMode) -> Self {
        self.with_policy(move |p| p.quic(mode))
    }

    /// Builds the fingerprint.
    ///
    /// The steps are: refuse an `Off` where it means nothing; take the
    /// operating system from a recognised user agent when none was set; draw
    /// every `Random` setting from one seeded draw (see the
    /// [type documentation](Self#random)); derive the language list from a given
    /// locale and `navigator.platform` from a recognised user agent when they
    /// were not given; apply the policy.
    ///
    /// # Errors
    ///
    /// Returns a [`FingerprintError`] naming the setting when an `Off` means
    /// nothing for it, when a `Random` has nothing to draw, or when nothing
    /// coherent with the other settings can be drawn.
    pub fn build(self) -> Result<Fingerprint, FingerprintError> {
        let Self {
            mut fields,
            browser_type_set,
            seed,
            policy,
            changes,
        } = self;

        refuse_useless_off(&fields)?;
        if fields
            .proxy
            .value()
            .is_some_and(|proxy| proxy.host.is_empty())
        {
            return Err(FingerprintError::Incoherent {
                setting: "proxy",
                reason: "the proxy has no host".to_owned(),
            });
        }
        if fields.os_type.is_none() {
            fields.os_type = Some(
                fields
                    .user_agent
                    .as_str()
                    .and_then(os_of_user_agent)
                    .unwrap_or_default(),
            );
        }

        let random = has_random(&fields);
        let seed = seed.unwrap_or_else(|| {
            if random {
                Rng::from_entropy().next_u64()
            } else {
                stable_seed(&fields)
            }
        });
        fields.seed = Some(seed);
        if random {
            draw_random(&mut fields, browser_type_set)?;
        }

        if fields.languages.is_natural() {
            if let Some(locale) = fields.locale.as_str() {
                fields.languages = Spoof::Value(languages_for_locale(locale));
            }
        }
        if fields.platform.is_natural() && fields.user_agent.is_value() {
            fields.platform = Spoof::Value(fields.os().platform().to_owned());
        }
        fields.policy = changes.iter().fold(policy, |p, change| (change.0)(p));
        Ok(Fingerprint(fields))
    }
}

/// Refuses an `Off` for a setting that has no off state.
fn refuse_useless_off(f: &Fields) -> Result<(), FingerprintError> {
    let off = [
        ("user_agent", f.user_agent.is_off()),
        ("platform", f.platform.is_off()),
        ("hardware_concurrency", f.hardware_concurrency.is_off()),
        ("device_memory", f.device_memory.is_off()),
        ("max_touch_points", f.max_touch_points.is_off()),
        ("vendor", f.vendor.is_off()),
        ("oscpu", f.oscpu.is_off()),
        ("product_sub", f.product_sub.is_off()),
        ("build_id", f.build_id.is_off()),
        ("app_version", f.app_version.is_off()),
        ("locale", f.locale.is_off()),
        ("languages", f.languages.is_off()),
        ("accept_languages", f.accept_languages.is_off()),
        ("timezone", f.timezone.is_off()),
        ("screen", f.screen.is_off()),
        ("avail_screen", f.avail_screen.is_off()),
        ("pixel_ratio", f.pixel_ratio.is_off()),
        ("color_depth", f.color_depth.is_off()),
        ("webgl", f.webgl.is_off()),
        ("fonts", f.fonts.is_off()),
        ("proxy", f.proxy.is_off()),
    ];
    match off.iter().find(|(_, is_off)| *is_off) {
        Some((setting, _)) => Err(FingerprintError::OffNotAvailable { setting }),
        None => Ok(()),
    }
}

/// Whether any setting asks to be drawn at random.
fn has_random(f: &Fields) -> bool {
    [
        f.user_agent.is_random(),
        f.platform.is_random(),
        f.hardware_concurrency.is_random(),
        f.device_memory.is_random(),
        f.max_touch_points.is_random(),
        f.vendor.is_random(),
        f.oscpu.is_random(),
        f.product_sub.is_random(),
        f.build_id.is_random(),
        f.app_version.is_random(),
        f.locale.is_random(),
        f.languages.is_random(),
        f.accept_languages.is_random(),
        f.timezone.is_random(),
        f.screen.is_random(),
        f.avail_screen.is_random(),
        f.pixel_ratio.is_random(),
        f.color_depth.is_random(),
        f.webgl.is_random(),
        f.media_devices.is_random(),
        f.geolocation.is_random(),
        f.fonts.is_random(),
        f.proxy.is_random(),
        f.webrtc.is_random(),
        f.battery.is_random(),
        f.connection.is_random(),
        f.speech_voices.is_random(),
        f.bluetooth.is_random(),
        f.client_hints.is_random(),
    ]
    .contains(&true)
}

/// A noise seed derived from the user agent, so a persona that draws nothing at
/// random gets the same noise on every launch.
fn stable_seed(f: &Fields) -> u64 {
    f.user_agent
        .as_str()
        .map_or(DEFAULT_SEED, |ua| fnv1a(ua.as_bytes()))
}

/// The Chrome major a user agent names.
fn chrome_major_of(ua: &str) -> Option<u32> {
    let rest = ua.split("Chrome/").nth(1)?;
    rest.split('.').next()?.parse().ok()
}

/// Replaces every `Random` setting with the value of one seeded draw.
fn draw_random(f: &mut Fields, browser_type_set: bool) -> Result<(), FingerprintError> {
    for (setting, random) in [
        ("proxy", f.proxy.is_random()),
        ("oscpu", f.oscpu.is_random()),
        ("build_id", f.build_id.is_random()),
    ] {
        if random {
            return Err(FingerprintError::NoRandomValue { setting });
        }
    }

    let seed = f.seed.unwrap_or(DEFAULT_SEED);
    let os = f.os();

    // The place the draw has to agree with: a given time zone first, then a
    // given locale. `Some(None)` is a value the table of places does not know.
    let anchor = match (f.timezone.as_str(), f.locale.as_str()) {
        (Some(zone), _) => Some((
            randomize::locale_of_timezone(zone),
            format!("time zone '{zone}'"),
        )),
        (None, Some(locale)) => Some((
            randomize::locale_by_tag(locale),
            format!("locale '{locale}'"),
        )),
        (None, None) => None,
    };
    let place_known = anchor.as_ref().is_none_or(|(place, _)| place.is_some());
    let need_place = |setting: &'static str| -> Result<(), FingerprintError> {
        match &anchor {
            Some((None, what)) if !place_known => Err(FingerprintError::Incoherent {
                setting,
                reason: format!(
                    "the {what} is not in the table of places, so nothing can be drawn to agree with it"
                ),
            }),
            _ => Ok(()),
        }
    };

    // A given user agent decides the Chromium major the rest claims.
    let ua = f.user_agent.as_str().map(str::to_owned);
    let forced_major = f
        .core_version
        .or_else(|| ua.as_deref().and_then(chrome_major_of));
    let draw = randomize::draw(os, seed, forced_major, anchor.as_ref().and_then(|a| a.0));
    let profile = &draw.profile;

    if f.user_agent.is_random() {
        f.user_agent = Spoof::Value(profile.user_agent.clone());
        if !browser_type_set {
            f.browser_type = profile.browser_type;
        }
        if f.core_version.is_none() {
            f.core_version = profile.core_version;
        }
    }
    if f.platform.is_random() {
        f.platform = Spoof::Value(os.platform().to_owned());
    }
    if f.hardware_concurrency.is_random() {
        f.hardware_concurrency = Spoof::Value(profile.cores);
    }
    if f.device_memory.is_random() {
        // Safari has no `navigator.deviceMemory`, so there is nothing to claim.
        f.device_memory = profile.memory_gb.map_or(Spoof::Natural, Spoof::Value);
    }
    if f.max_touch_points.is_random() {
        f.max_touch_points = Spoof::Value(profile.touch_points);
    }
    if f.vendor.is_random() {
        f.vendor = Spoof::Value(profile.vendor.to_owned());
    }
    if f.product_sub.is_random() {
        f.product_sub = Spoof::Value("20030107".to_owned());
    }
    if f.app_version.is_random() {
        let source = f.user_agent.as_str().map(str::to_owned).ok_or_else(|| {
            FingerprintError::Incoherent {
                setting: "app_version",
                reason: "it is the user agent without its Mozilla/ prefix, and the user agent is not set"
                    .to_owned(),
            }
        })?;
        f.app_version = Spoof::Value(source.replacen("Mozilla/", "", 1));
    }

    if f.locale.is_random() {
        need_place("locale")?;
        f.locale = Spoof::Value(draw.region.tag.to_owned());
    }
    // The language list follows a given locale, else the place.
    let languages = match f.locale.as_str() {
        Some(locale) => randomize::locale_by_tag(locale).map_or_else(
            || languages_for_locale(locale),
            randomize::Locale::accept_language,
        ),
        None => {
            if f.languages.is_random() || f.accept_languages.is_random() {
                need_place("languages")?;
            }
            draw.region.accept_language()
        }
    };
    if f.languages.is_random() {
        f.languages = Spoof::Value(languages.clone());
    }
    if f.accept_languages.is_random() {
        f.accept_languages = Spoof::Value(languages);
    }
    if f.timezone.is_random() {
        need_place("timezone")?;
        f.timezone = Spoof::Value(draw.region.timezone.to_owned());
    }
    if f.geolocation.is_random() {
        need_place("geolocation")?;
        f.geolocation = Spoof::Value(
            Geolocation::new(draw.position.latitude, draw.position.longitude)
                .accuracy(draw.position.accuracy),
        );
    }

    if f.screen.is_random() {
        f.screen = Spoof::Value(ScreenSize::new(profile.screen.width, profile.screen.height));
    }
    if f.avail_screen.is_random() {
        // The page loses a taskbar's height on a desktop, nothing on a phone.
        let (width, height) = f
            .screen
            .value()
            .map_or((profile.screen.width, profile.screen.height), |size| {
                (size.width, size.height)
            });
        let lost = if os.is_mobile() { 0 } else { 40 };
        f.avail_screen = Spoof::Value(ScreenSize::new(width, height.saturating_sub(lost)));
    }
    if f.pixel_ratio.is_random() {
        f.pixel_ratio = Spoof::Value(profile.screen.pixel_ratio);
    }
    if f.color_depth.is_random() {
        f.color_depth = Spoof::Value(profile.color_depth);
    }
    if f.webgl.is_random() {
        f.webgl = Spoof::Value(profile.graphics.clone());
    }
    if f.client_hints.is_random() {
        // A browser without `navigator.userAgentData` (Safari) has none to claim.
        let hints = match (&profile.client_hints, forced_major, ua.as_deref()) {
            (Some(_), Some(major), Some(ua)) if ua.contains("Edg/") => {
                Some(randomize::edge_hints(major))
            }
            (hints, ..) => hints.clone(),
        };
        f.client_hints = hints.map_or(Spoof::Off, Spoof::Value);
    }

    if f.fonts.is_random() {
        f.fonts = Spoof::Value(draw.fonts());
    }
    if f.media_devices.is_random() {
        f.media_devices = Spoof::Value(draw.media_devices());
    }
    if f.webrtc.is_random() {
        f.webrtc = Spoof::Value(draw.webrtc());
    }
    if f.battery.is_random() {
        f.battery = draw.battery().map_or(Spoof::Off, Spoof::Value);
    }
    if f.connection.is_random() {
        f.connection = draw.connection().map_or(Spoof::Off, Spoof::Value);
    }
    if f.speech_voices.is_random() {
        f.speech_voices = Spoof::Value(draw.speech_voices());
    }
    if f.bluetooth.is_random() {
        f.bluetooth = draw.bluetooth().map_or(Spoof::Off, Spoof::Value);
    }
    Ok(())
}

/// The `navigator.languages` list for `locale`, as a comma-separated string:
/// the table's list when the locale is in [`LOCALES`](super::randomize::LOCALES),
/// otherwise the locale and its bare language.
pub(crate) fn languages_for_locale(locale: &str) -> String {
    if let Some(known) = randomize::LOCALES
        .iter()
        .find(|l| l.tag.eq_ignore_ascii_case(locale))
    {
        return known.languages.join(",");
    }
    match locale.split_once('-') {
        Some((language, _)) if !language.is_empty() => format!("{locale},{language}"),
        _ => locale.to_owned(),
    }
}

/// The operating system a user agent names, if it names one this crate models.
fn os_of_user_agent(ua: &str) -> Option<OsType> {
    let ua = ua.to_ascii_lowercase();
    if ua.contains("iphone") || ua.contains("ipad") {
        Some(OsType::Ios)
    } else if ua.contains("android") {
        Some(OsType::Android)
    } else if ua.contains("windows") {
        Some(OsType::Windows)
    } else if ua.contains("mac os") {
        Some(OsType::Macos)
    } else if ua.contains("linux") || ua.contains("x11") {
        Some(OsType::Linux)
    } else {
        None
    }
}

/// FNV-1a 64-bit hash used to derive a stable noise seed from the user-agent.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Fingerprint {
        /// Whether any setting still asks to be drawn: never, for a built one.
        fn has_random_settings(&self) -> bool {
            has_random(&self.0)
        }
    }

    /// Builds, or fails the test with the error.
    fn built(builder: FingerprintBuilder) -> Fingerprint {
        builder.build().expect("the builder builds")
    }

    fn user_agent(fp: &Fingerprint) -> &str {
        fp.user_agent().as_str().unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // The builder and the presets
    // ------------------------------------------------------------------

    #[test]
    fn builder_builds_profile() {
        let fp = built(
            Fingerprint::builder()
                .os_type(OsType::Windows)
                .user_agent("UA")
                .screen(1920, 1080)
                .hardware_concurrency(8),
        );

        assert_eq!(fp.os_type(), OsType::Windows);
        assert_eq!(user_agent(&fp), "UA");
        assert_eq!(fp.screen().value().map(|s| s.width), Some(1920));
        assert_eq!(fp.hardware_concurrency().value(), Some(&8));
    }

    #[test]
    fn a_fingerprint_without_settings_leaves_every_setting_natural() {
        let fp = Fingerprint::default();

        for dimension in EVERY_DIMENSION {
            // Only the `toString` mask is on by default, because the others rely on it.
            assert_eq!(
                fp.is_masked(dimension),
                dimension == Dimension::NativeToString,
                "{dimension:?}"
            );
        }
        assert!(fp.user_agent().is_natural() && fp.proxy().is_natural());
    }

    /// Every dimension.
    const EVERY_DIMENSION: [Dimension; 22] = [
        Dimension::Navigator,
        Dimension::Localization,
        Dimension::Timezone,
        Dimension::Screen,
        Dimension::Graphics,
        Dimension::Fonts,
        Dimension::MediaDevices,
        Dimension::Geolocation,
        Dimension::WebRtc,
        Dimension::Canvas,
        Dimension::Audio,
        Dimension::Battery,
        Dimension::Connection,
        Dimension::Speech,
        Dimension::ClientHints,
        Dimension::Proxy,
        Dimension::Ports,
        Dimension::Trackers,
        Dimension::Bluetooth,
        Dimension::ChromeRuntime,
        Dimension::Headless,
        Dimension::NativeToString,
    ];

    #[test]
    fn a_value_turns_on_the_dimension_it_belongs_to_and_no_other() {
        let on = |builder: FingerprintBuilder| -> Vec<Dimension> {
            let fp = built(builder);
            EVERY_DIMENSION
                .into_iter()
                .filter(|d| *d != Dimension::NativeToString && fp.is_masked(*d))
                .collect()
        };
        let proxy = ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example".to_owned(),
            port: 8080,
            ..ProxyConfig::default()
        };

        assert_eq!(
            on(Fingerprint::builder().user_agent("UA")),
            [Dimension::Navigator]
        );
        assert_eq!(
            on(Fingerprint::builder().locale("de-DE")),
            [Dimension::Localization]
        );
        assert_eq!(
            on(Fingerprint::builder().timezone("Europe/Berlin")),
            [Dimension::Timezone]
        );
        assert_eq!(
            on(Fingerprint::builder().pixel_ratio(2.0)),
            [Dimension::Screen]
        );
        assert_eq!(
            on(Fingerprint::builder().webgl(("V", "R"))),
            [Dimension::Graphics]
        );
        assert_eq!(
            on(Fingerprint::builder().fonts(["Arial"])),
            [Dimension::Fonts]
        );
        assert_eq!(
            on(Fingerprint::builder().media_device_counts(1, 1, 1)),
            [Dimension::MediaDevices]
        );
        assert_eq!(
            on(Fingerprint::builder().geolocation((1.0, 2.0))),
            [Dimension::Geolocation]
        );
        assert_eq!(
            on(Fingerprint::builder().webrtc(Spoof::Off)),
            [Dimension::WebRtc]
        );
        assert_eq!(
            on(Fingerprint::builder().battery(Spoof::Off)),
            [Dimension::Battery]
        );
        assert_eq!(
            on(Fingerprint::builder().connection(Spoof::Off)),
            [Dimension::Connection]
        );
        assert_eq!(
            on(Fingerprint::builder().speech_voices(Spoof::Off)),
            [Dimension::Speech]
        );
        assert_eq!(
            on(Fingerprint::builder().client_hints(Spoof::Off)),
            [Dimension::ClientHints]
        );
        assert_eq!(
            on(Fingerprint::builder().bluetooth(false)),
            [Dimension::Bluetooth]
        );
        assert_eq!(on(Fingerprint::builder().proxy(proxy)), [Dimension::Proxy]);
        // The policy turns on the ones that have no setting of their own.
        assert_eq!(
            on(Fingerprint::builder().canvas_noise(CanvasNoise::Low)),
            [Dimension::Canvas]
        );
        assert_eq!(
            on(Fingerprint::builder().audio_noise(AudioNoise::Persistent)),
            [Dimension::Audio]
        );
        assert_eq!(
            on(Fingerprint::builder().blocked_ports(vec![9222])),
            [Dimension::Ports]
        );
        assert_eq!(
            on(Fingerprint::builder().block_trackers(true)),
            [Dimension::Trackers]
        );
        assert_eq!(
            on(Fingerprint::builder().webrtc_policy(WebRtcPolicy::PublicInterfaceOnly)),
            [Dimension::WebRtc]
        );
    }

    #[test]
    fn presets_are_valid() {
        assert_eq!(Fingerprint::windows_desktop().os_type(), OsType::Windows);
        assert_eq!(Fingerprint::macos_desktop().os_type(), OsType::Macos);
        assert_eq!(Fingerprint::linux_desktop().os_type(), OsType::Linux);
        assert_eq!(Fingerprint::android_mobile().os_type(), OsType::Android);
        let ios = Fingerprint::ios_mobile_safari();
        assert_eq!(ios.os_type(), OsType::Ios);
        assert_eq!(ios.browser_type(), BrowserType::MobileSafari);
    }

    #[test]
    fn proxy_url_includes_auth() {
        let proxy = ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example.com".to_owned(),
            port: 8080,
            username: Some("u".to_owned()),
            password: Some("p".to_owned()),
            save_traffic: false,
        };
        assert_eq!(proxy.to_url(), "http://u:p@proxy.example.com:8080");
    }

    fn every_preset() -> [(&'static str, Fingerprint); 6] {
        [
            ("windows", Fingerprint::windows_desktop()),
            ("edge", Fingerprint::edge_desktop()),
            ("macos", Fingerprint::macos_desktop()),
            ("linux", Fingerprint::linux_desktop()),
            ("android", Fingerprint::android_mobile()),
            ("ios", Fingerprint::ios_mobile_safari()),
        ]
    }

    #[test]
    fn every_preset_validates_without_errors_or_warnings() {
        for (name, fp) in every_preset() {
            let report = fp.validate();
            assert!(report.errors.is_empty(), "{name}: {:?}", report.errors);
            assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
        }
    }

    #[test]
    fn a_preset_is_the_same_every_time_so_its_noise_is_stable() {
        for (name, fp) in every_preset() {
            assert!(!fp.has_random_settings(), "{name}");
        }
        assert_eq!(
            Fingerprint::windows_desktop(),
            Fingerprint::windows_desktop()
        );
        assert_eq!(
            Fingerprint::windows_desktop().seed(),
            Fingerprint::windows_desktop().seed()
        );
        assert_ne!(
            Fingerprint::windows_desktop().seed(),
            Fingerprint::macos_desktop().seed()
        );
    }

    #[test]
    fn every_preset_time_zone_is_one_the_table_of_places_knows_and_its_locale_agrees() {
        for (name, fp) in every_preset() {
            let zone = fp.timezone().as_str().expect("a time zone");
            assert!(
                randomize::locale_of_timezone(zone).is_some(),
                "{name}: {zone} is not in the table"
            );
            // The position stays the browser's own: see the preset documentation.
            assert!(!fp.is_masked(Dimension::Geolocation), "{name}");
        }
    }

    #[test]
    fn every_preset_masks_the_dimensions_its_values_and_its_policy_need() {
        for (name, fp) in every_preset() {
            for dimension in [
                Dimension::Navigator,
                Dimension::Localization,
                Dimension::Timezone,
                Dimension::Screen,
                Dimension::Graphics,
                Dimension::WebRtc,
                Dimension::Canvas,
                Dimension::Speech,
                Dimension::ChromeRuntime,
                Dimension::Headless,
                Dimension::NativeToString,
            ] {
                assert!(fp.is_masked(dimension), "{name}: {dimension:?} is off");
            }
        }
    }

    #[test]
    fn safari_has_no_battery_connection_or_bluetooth_and_the_others_do() {
        let ios = Fingerprint::ios_mobile_safari();
        assert!(ios.battery().is_off() && ios.connection().is_off() && ios.bluetooth().is_off());
        let windows = Fingerprint::windows_desktop();
        assert!(windows.battery().is_value() && windows.connection().is_value());
        assert_eq!(windows.bluetooth().value(), Some(&false));
        assert_eq!(
            Fingerprint::android_mobile().bluetooth().value(),
            Some(&true)
        );
    }

    #[test]
    fn the_chrome_presets_claim_one_current_major_in_the_frozen_format() {
        let version = format!("Chrome/{}.0.0.0", randomize::PRESET_CHROMIUM_MAJOR);
        for (fp, platform) in [
            (
                Fingerprint::windows_desktop(),
                "(Windows NT 10.0; Win64; x64)",
            ),
            (
                Fingerprint::macos_desktop(),
                "(Macintosh; Intel Mac OS X 10_15_7)",
            ),
            (Fingerprint::linux_desktop(), "(X11; Linux x86_64)"),
            (Fingerprint::android_mobile(), "(Linux; Android 10; K)"),
        ] {
            let ua = user_agent(&fp);
            assert!(ua.contains(&version), "{ua}");
            assert!(ua.contains(platform), "{ua}");
        }
        assert_eq!(
            user_agent(&Fingerprint::ios_mobile_safari()),
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1"
        );
    }

    #[test]
    fn the_edge_preset_is_edge_and_not_chrome() {
        let fp = Fingerprint::edge_desktop();
        let major = randomize::PRESET_CHROMIUM_MAJOR;
        let ua = user_agent(&fp);
        assert!(ua.ends_with(&format!("Edg/{major}.0.0.0")), "{ua}");
        assert!(ua.contains(&format!("Chrome/{major}.0.0.0")), "{ua}");
        assert_eq!(fp.os_type(), OsType::Windows);
        assert_eq!(fp.platform().as_str(), Some("Win32"));
        assert_eq!(fp.webgl(), Fingerprint::windows_desktop().webgl());
    }

    #[test]
    fn the_macro_names_each_preset() {
        assert_eq!(crate::fingerprint!(windows), Fingerprint::windows_desktop());
        assert_eq!(crate::fingerprint!(edge), Fingerprint::edge_desktop());
        assert_ne!(crate::fingerprint!(edge), crate::fingerprint!(windows));
        assert_eq!(crate::fingerprint!(macos), Fingerprint::macos_desktop());
        assert_eq!(crate::fingerprint!(linux), Fingerprint::linux_desktop());
        assert_eq!(crate::fingerprint!(android), Fingerprint::android_mobile());
        assert_eq!(crate::fingerprint!(ios), Fingerprint::ios_mobile_safari());
        assert_eq!(crate::fingerprint!(safari), crate::fingerprint!(ios));
    }

    #[test]
    fn the_linux_preset_reports_a_linux_graphics_stack_and_the_mac_one_metal() {
        let linux = Fingerprint::linux_desktop();
        let gl = linux.webgl().value().expect("webgl");
        let renderer = gl.renderer.as_deref().unwrap_or_default();
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("D3D11"),
            "{renderer}"
        );
        assert!(renderer.contains("OpenGL"), "{renderer}");
        assert_eq!(
            gl.vendor.as_deref(),
            Some("Google Inc. (NVIDIA Corporation)")
        );

        let mac = Fingerprint::macos_desktop();
        let renderer = mac
            .webgl()
            .value()
            .and_then(|gl| gl.renderer.as_deref())
            .unwrap_or_default();
        assert!(renderer.contains("Metal"), "{renderer}");
    }

    #[test]
    fn the_available_screen_is_what_was_given() {
        let fp = built(
            Fingerprint::builder()
                .screen(1920, 1080)
                .avail_screen((1920, 1040)),
        );
        assert_eq!(
            fp.avail_screen().value(),
            Some(&ScreenSize::new(1920, 1040))
        );
    }

    // ------------------------------------------------------------------
    // What build derives, and what it refuses
    // ------------------------------------------------------------------

    #[test]
    fn the_locale_gives_the_language_list_and_the_user_agent_gives_the_os() {
        let from_locale = built(Fingerprint::builder().locale("de-DE"));
        assert_eq!(from_locale.languages().as_str(), Some("de-DE,de,en-US,en"));
        let given = built(Fingerprint::builder().locale("de-DE").languages("de-DE"));
        assert_eq!(given.languages().as_str(), Some("de-DE"));

        let mac = built(Fingerprint::builder().user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/155.0.0.0 Safari/537.36",
        ));
        assert_eq!(mac.os_type(), OsType::Macos);
        assert_eq!(mac.platform().as_str(), Some("MacIntel"));
        // An explicit operating system is not overridden by the user agent.
        let explicit = built(
            Fingerprint::builder()
                .os_type(OsType::Linux)
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"),
        );
        assert_eq!(explicit.os_type(), OsType::Linux);
    }

    #[test]
    fn mismatched_platform_and_ua_is_incoherent() {
        // macOS user-agent with a Windows platform string is a classic tell.
        let fp = built(
            Fingerprint::builder()
                .os_type(OsType::Macos)
                .user_agent(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                     (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
                )
                .platform("Win32"),
        );
        let report = fp.validate();
        assert!(!report.is_coherent());
        assert!(report.errors.iter().any(|e| e.contains("platform")));
    }

    #[test]
    fn off_where_it_means_nothing_names_the_setting() {
        let refused = |builder: FingerprintBuilder| builder.build().unwrap_err();
        let off: [(&str, FingerprintBuilder); 12] = [
            ("user_agent", Fingerprint::builder().user_agent(Spoof::Off)),
            (
                "hardware_concurrency",
                Fingerprint::builder().hardware_concurrency(Spoof::Off),
            ),
            ("locale", Fingerprint::builder().locale(Spoof::Off)),
            ("timezone", Fingerprint::builder().timezone(Spoof::Off)),
            ("screen", Fingerprint::builder().screen_spoof(Spoof::Off)),
            (
                "pixel_ratio",
                Fingerprint::builder().pixel_ratio(Spoof::Off),
            ),
            ("webgl", Fingerprint::builder().webgl(Spoof::Off)),
            ("fonts", Fingerprint::builder().fonts(Spoof::Off)),
            ("proxy", Fingerprint::builder().proxy(Spoof::Off)),
            (
                "app_version",
                Fingerprint::builder().app_version(Spoof::Off),
            ),
            (
                "avail_screen",
                Fingerprint::builder().avail_screen(Spoof::Off),
            ),
            (
                "color_depth",
                Fingerprint::builder().color_depth(Spoof::Off),
            ),
        ];
        for (setting, builder) in off {
            assert_eq!(
                refused(builder),
                FingerprintError::OffNotAvailable { setting },
                "{setting}"
            );
        }
        // The message says which setting.
        let message = refused(Fingerprint::builder().timezone(Spoof::Off)).to_string();
        assert!(message.starts_with("timezone cannot be Off"), "{message}");
    }

    #[test]
    fn off_is_a_state_of_the_eight_settings_that_can_be_turned_off() {
        let fp = built(
            Fingerprint::builder()
                .geolocation(Spoof::Off)
                .webrtc(Spoof::Off)
                .media_devices(Spoof::Off)
                .battery(Spoof::Off)
                .bluetooth(Spoof::Off)
                .speech_voices(Spoof::Off)
                .connection(Spoof::Off)
                .client_hints(Spoof::Off),
        );
        assert!(fp.geolocation().is_off() && fp.client_hints().is_off() && fp.battery().is_off());
        assert!(fp.webrtc().is_off() && fp.media_devices().is_off() && fp.bluetooth().is_off());
        assert!(fp.speech_voices().is_off() && fp.connection().is_off());
    }

    #[test]
    fn a_proxy_without_a_host_is_refused_because_it_would_send_nothing_anywhere() {
        let error = Fingerprint::builder()
            .proxy(ProxyConfig::default())
            .build()
            .unwrap_err();
        assert!(
            matches!(
                error,
                FingerprintError::Incoherent {
                    setting: "proxy",
                    ..
                }
            ),
            "{error:?}"
        );
        let fine = built(Fingerprint::builder().proxy(ProxyConfig {
            host: "proxy.example".to_owned(),
            ..ProxyConfig::default()
        }));
        assert!(fine.is_masked(Dimension::Proxy));
    }

    #[test]
    fn random_where_there_is_nothing_to_draw_names_the_setting() {
        for (setting, builder) in [
            ("proxy", Fingerprint::builder().proxy(Spoof::Random)),
            ("oscpu", Fingerprint::builder().oscpu(Spoof::Random)),
            ("build_id", Fingerprint::builder().build_id(Spoof::Random)),
        ] {
            assert_eq!(
                builder.build().unwrap_err(),
                FingerprintError::NoRandomValue { setting }
            );
        }
    }

    // ------------------------------------------------------------------
    // Random
    // ------------------------------------------------------------------

    #[test]
    fn a_built_fingerprint_holds_no_random() {
        let fp = built(
            Fingerprint::builder()
                .user_agent(Spoof::Random)
                .locale(Spoof::Random)
                .timezone(Spoof::Random)
                .geolocation(Spoof::Random)
                .screen_spoof(Spoof::Random)
                .webgl(Spoof::Random)
                .fonts(Spoof::Random)
                .media_devices(Spoof::Random)
                .webrtc(Spoof::Random)
                .battery(Spoof::Random)
                .client_hints(Spoof::Random),
        );
        assert!(!fp.has_random_settings());
        assert!(fp.user_agent().is_value() && fp.fonts().is_value() && fp.webrtc().is_value());
    }

    #[test]
    fn the_same_seed_gives_the_same_fingerprint_and_another_gives_another() {
        let draw = |seed: u64| {
            built(
                Fingerprint::builder()
                    .seed(seed)
                    .user_agent(Spoof::Random)
                    .timezone(Spoof::Random)
                    .geolocation(Spoof::Random)
                    .webgl(Spoof::Random),
            )
        };
        assert_eq!(draw(7), draw(7));
        assert_ne!(draw(7), draw(8));
        assert_eq!(draw(7).seed(), 7);
    }

    #[test]
    fn without_a_seed_a_draw_uses_fresh_entropy_and_keeps_the_seed_it_used() {
        let draw = || {
            built(
                Fingerprint::builder()
                    .user_agent(Spoof::Random)
                    .geolocation(Spoof::Random),
            )
        };
        let (first, second) = (draw(), draw());
        assert_ne!(first.seed(), second.seed());
        // Rebuilding from the kept seed gives the same fingerprint.
        let again = built(
            Fingerprint::builder()
                .seed(first.seed())
                .user_agent(Spoof::Random)
                .geolocation(Spoof::Random),
        );
        assert_eq!(again, first);
    }

    #[test]
    fn a_random_place_agrees_with_a_given_time_zone() {
        for seed in 0..40 {
            let fp = built(
                Fingerprint::builder()
                    .seed(seed)
                    .timezone("Europe/Berlin")
                    .locale(Spoof::Random)
                    .languages(Spoof::Random)
                    .geolocation(Spoof::Random),
            );
            assert_eq!(fp.locale().as_str(), Some("de-DE"), "seed {seed}");
            assert!(
                fp.languages()
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("de-DE"),
                "seed {seed}"
            );
            let place = fp.geolocation().value().expect("a position");
            assert!((place.latitude - 52.52).abs() < 0.1, "seed {seed}");
            assert!((place.longitude - 13.405).abs() < 0.1, "seed {seed}");
            let report = fp.validate();
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        }
    }

    #[test]
    fn a_random_time_zone_agrees_with_a_given_locale() {
        let fp = built(
            Fingerprint::builder()
                .locale("ja-JP")
                .timezone(Spoof::Random)
                .geolocation(Spoof::Random),
        );
        assert_eq!(fp.timezone().as_str(), Some("Asia/Tokyo"));
        let place = fp.geolocation().value().expect("a position");
        assert!((place.latitude - 35.68).abs() < 0.2, "{place:?}");
    }

    #[test]
    fn a_random_place_next_to_a_zone_the_table_lacks_is_an_error_that_says_why() {
        let error = Fingerprint::builder()
            .timezone("Pacific/Chatham")
            .locale(Spoof::Random)
            .build()
            .unwrap_err();
        match error {
            FingerprintError::Incoherent { setting, reason } => {
                assert_eq!(setting, "locale");
                assert!(reason.contains("Pacific/Chatham"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
        // What does not depend on the place can still be drawn.
        let fine = built(
            Fingerprint::builder()
                .timezone("Pacific/Chatham")
                .screen_spoof(Spoof::Random)
                .webgl(Spoof::Random),
        );
        assert!(fine.screen().is_value() && fine.webgl().is_value());
    }

    #[test]
    fn a_random_machine_is_one_the_operating_system_reports() {
        for (os, needle) in [
            (OsType::Windows, "Direct3D"),
            (OsType::Macos, "Metal"),
            (OsType::Linux, "OpenGL"),
        ] {
            for seed in 0..25 {
                let fp = built(
                    Fingerprint::builder()
                        .os_type(os)
                        .seed(seed)
                        .webgl(Spoof::Random)
                        .platform(Spoof::Random),
                );
                let renderer = fp
                    .webgl()
                    .value()
                    .and_then(|gl| gl.renderer.as_deref())
                    .unwrap_or_default();
                assert!(renderer.contains(needle), "{os:?} {renderer}");
                assert_eq!(fp.platform().as_str(), Some(os.platform()));
            }
        }
    }

    #[test]
    fn the_operating_system_of_a_given_user_agent_decides_what_is_drawn() {
        let fp = built(
            Fingerprint::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/150.0.0.0 Safari/537.36")
                .webgl(Spoof::Random)
                .client_hints(Spoof::Random),
        );
        assert_eq!(fp.os_type(), OsType::Macos);
        let renderer = fp
            .webgl()
            .value()
            .and_then(|g| g.renderer.as_deref())
            .unwrap_or_default();
        assert!(renderer.contains("Metal"), "{renderer}");
        // The Client Hints claim the major the user agent names.
        let hints = fp.client_hints().value().expect("hints");
        assert!(hints.brands.iter().any(|b| b.version == "150"), "{hints:?}");
    }

    #[test]
    fn a_pinned_major_reaches_the_random_user_agent_and_client_hints() {
        let fp = built(
            Fingerprint::builder()
                .os_type(OsType::Windows)
                .core_version(150)
                .user_agent(Spoof::Random)
                .client_hints(Spoof::Random),
        );
        assert!(
            user_agent(&fp).contains("Chrome/150.0.0.0"),
            "{}",
            user_agent(&fp)
        );
        let hints = fp.client_hints().value().expect("hints");
        assert!(hints.brands.iter().any(|b| b.version == "150"), "{hints:?}");
        assert_eq!(fp.core_version(), Some(150));
    }

    #[test]
    fn a_safari_draw_has_no_client_hints_battery_connection_or_bluetooth() {
        let fp = Fingerprint::randomized(OsType::Ios, 3);
        assert_eq!(fp.browser_type(), BrowserType::MobileSafari);
        assert!(fp.client_hints().is_off());
        assert!(fp.battery().is_off() && fp.connection().is_off() && fp.bluetooth().is_off());
        // Safari has no `deviceMemory` either.
        assert!(fp.device_memory().is_natural());
    }

    #[test]
    fn randomized_with_major_claims_that_major_and_keeps_the_rest_of_the_seed() {
        let free = Fingerprint::randomized(OsType::Macos, 12);
        let pinned = Fingerprint::randomized_with_major(OsType::Macos, 12, 155);
        assert_eq!(pinned.core_version(), Some(155));
        assert!(user_agent(&pinned).contains("Chrome/155.0.0.0"));
        assert_eq!(pinned.timezone(), free.timezone());
        assert_eq!(pinned.webgl(), free.webgl());
        assert_eq!(pinned.seed(), free.seed());
        assert!(pinned.validate().is_coherent());
    }

    #[test]
    fn every_randomized_persona_validates_without_a_warning() {
        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
            OsType::Ios,
        ] {
            for seed in (0..40).chain([u64::MAX]) {
                let report = Fingerprint::randomized(os, seed).validate();
                assert!(
                    report.errors.is_empty(),
                    "{os:?} {seed}: {:?}",
                    report.errors
                );
                assert!(
                    report.warnings.is_empty(),
                    "{os:?} {seed}: {:?}",
                    report.warnings
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // The policy
    // ------------------------------------------------------------------

    #[test]
    fn the_policy_and_its_shortcuts_come_in_any_order() {
        let a = built(
            Fingerprint::builder()
                .policy(StealthPolicy::balanced())
                .canvas_noise(CanvasNoise::Random)
                .grant_permissions(true),
        );
        let b = built(
            Fingerprint::builder()
                .canvas_noise(CanvasNoise::Random)
                .grant_permissions(true)
                .policy(StealthPolicy::balanced()),
        );
        assert_eq!(a, b);
        assert_eq!(a.policy().canvas_noise, CanvasNoise::Random);
        assert!(a.policy().grant_permissions && a.policy().chrome_runtime);
    }

    #[test]
    fn with_policy_changes_run_in_order_after_the_base() {
        let fp = built(
            Fingerprint::builder()
                .with_policy(|p| p.grant_permissions(true))
                .with_policy(|p| p.grant_permissions(false).disable_csp(true))
                .policy(StealthPolicy::max()),
        );
        assert!(!fp.policy().grant_permissions);
        assert!(fp.policy().disable_csp && fp.policy().block_trackers);
    }

    #[test]
    fn a_policy_never_changes_a_setting() {
        let plain = built(Fingerprint::builder().timezone("Europe/Berlin"));
        for policy in [
            StealthPolicy::default(),
            StealthPolicy::balanced(),
            StealthPolicy::native_only(),
            StealthPolicy::max(),
        ] {
            let with = built(
                Fingerprint::builder()
                    .timezone("Europe/Berlin")
                    .policy(policy),
            );
            assert_eq!(with.timezone(), plain.timezone());
            assert!(with.screen().is_natural() && with.webrtc().is_natural());
            assert!(with.user_agent().is_natural() && with.battery().is_natural());
        }
    }

    #[test]
    fn a_tracker_list_is_a_request_to_block_and_can_be_refused_by_a_later_change() {
        let fp = built(Fingerprint::builder().blocked_trackers(vec!["metrics.test".to_owned()]));
        assert!(fp.is_masked(Dimension::Trackers));
        assert_eq!(fp.tracker_hosts(), vec!["metrics.test"]);

        let refused = built(
            Fingerprint::builder()
                .blocked_trackers(vec!["metrics.test".to_owned()])
                .block_trackers(false),
        );
        assert!(!refused.is_masked(Dimension::Trackers));
        assert!(!refused.tracker_hosts().is_empty(), "the default list");
    }

    #[test]
    fn tracker_hosts_fall_back_to_defaults() {
        assert!(!Fingerprint::windows_desktop().tracker_hosts().is_empty());
    }

    // ------------------------------------------------------------------
    // Editing a built fingerprint
    // ------------------------------------------------------------------

    #[test]
    fn to_builder_changes_one_setting_and_keeps_the_rest() {
        let fp = Fingerprint::windows_desktop();
        let changed = built(fp.to_builder().timezone("Asia/Tokyo"));

        assert_eq!(changed.timezone().as_str(), Some("Asia/Tokyo"));
        assert_eq!(changed.screen(), fp.screen());
        assert_eq!(changed.webgl(), fp.webgl());
        assert_eq!(changed.policy(), fp.policy());
        assert_eq!(changed.seed(), fp.seed());
        // Rebuilding without a change gives the same fingerprint.
        assert_eq!(built(fp.to_builder()), fp);
    }

    #[test]
    fn to_builder_keeps_the_os_even_when_the_user_agent_is_changed() {
        let fp = Fingerprint::linux_desktop();
        let changed = built(fp.to_builder().user_agent("Mozilla/5.0 (Windows NT 10.0)"));
        assert_eq!(changed.os_type(), OsType::Linux);
    }

    // ------------------------------------------------------------------
    // JSON
    // ------------------------------------------------------------------

    #[test]
    fn json_leaves_natural_settings_out() {
        let fp = built(
            Fingerprint::builder()
                .timezone("Europe/Berlin")
                .webrtc(Spoof::Off),
        );
        let json = serde_json::to_value(&fp).unwrap();
        let object = json.as_object().unwrap();

        assert_eq!(object["timezone"], "Europe/Berlin");
        assert_eq!(object["webrtc"], "off");
        assert!(!object.contains_key("screen") && !object.contains_key("user_agent"));
    }

    #[test]
    fn json_round_trips_every_preset_and_a_drawn_persona() {
        for (name, fp) in every_preset() {
            let text = serde_json::to_string(&fp).unwrap();
            let again: Fingerprint = serde_json::from_str(&text).unwrap();
            assert_eq!(again, fp, "{name}");
        }
        let drawn = Fingerprint::randomized(OsType::Windows, 9);
        let again: Fingerprint =
            serde_json::from_str(&serde_json::to_string(&drawn).unwrap()).unwrap();
        assert_eq!(again, drawn);
    }

    #[test]
    fn json_round_trips_a_fingerprint_with_every_setting_given() {
        let everything = built(
            Fingerprint::builder()
                .os_type(OsType::Linux)
                .seed(5)
                .user_agent("Mozilla/5.0 (X11; Linux x86_64)")
                .platform("Linux x86_64")
                .hardware_concurrency(4)
                .device_memory(8.0)
                .max_touch_points(0)
                .vendor("Google Inc.")
                .oscpu("Linux x86_64")
                .product_sub("20030107")
                .build_id("20100101")
                .app_version("5.0 (X11)")
                .locale("de-DE")
                .languages("de-DE,de")
                .accept_languages("de-DE,de;q=0.9")
                .timezone("Europe/Berlin")
                .screen(1280, 720)
                .avail_screen((1280, 680))
                .pixel_ratio(1.5)
                .color_depth(24)
                .webgl(WebGl::new("V", "R").ids("0x1", "0x2"))
                .media_devices(MediaDevices::counts(1, 1, 1))
                .geolocation(Geolocation::new(1.0, 2.0).altitude(3.0).accuracy(4.0))
                .fonts(["Arial"])
                .proxy(ProxyConfig {
                    r#type: "http".to_owned(),
                    host: "proxy.example".to_owned(),
                    port: 8080,
                    ..ProxyConfig::default()
                })
                .webrtc(("203.0.113.9", "10.0.0.2"))
                .battery(BatteryProfile::default())
                .connection(ConnectionProfile::default())
                .speech_voices(SpeechVoice::default_set())
                .bluetooth(true)
                .client_hints(ClientHints::default())
                .local_storage(true)
                .save_service_worker(true)
                .custom_start_urls(vec!["https://example.com".to_owned()])
                .policy(StealthPolicy::max()),
        );
        let text = serde_json::to_string(&everything).unwrap();
        let again: Fingerprint = serde_json::from_str(&text).unwrap();
        assert_eq!(again, everything);
    }

    #[test]
    fn a_battery_that_never_empties_survives_json() {
        // JSON has no infinity: it is written as null and read back as infinity.
        let fp = built(Fingerprint::builder().battery(BatteryProfile::default()));
        let text = serde_json::to_string(&fp).unwrap();
        assert!(text.contains(r#""discharging_time":null"#), "{text}");
        let again: Fingerprint = serde_json::from_str(&text).unwrap();
        assert_eq!(
            again.battery().value().map(|b| b.discharging_time),
            Some(f64::INFINITY)
        );
    }

    #[test]
    fn json_reads_a_keyword_in_place_of_a_value_and_resolves_random() {
        let text = r#"{
            "timezone": "Europe/Berlin",
            "geolocation": "random",
            "webrtc": "off",
            "screen": {"width": 800, "height": 600},
            "seed": 11
        }"#;
        let fp: Fingerprint = serde_json::from_str(text).unwrap();

        assert!(
            fp.geolocation().is_value(),
            "random is drawn when it is read"
        );
        assert!(fp.webrtc().is_off());
        assert_eq!(fp.screen().value().map(|s| s.height), Some(600));
        assert_eq!(fp.seed(), 11);
        let place = fp.geolocation().value().unwrap();
        assert!((place.latitude - 52.52).abs() < 0.1);
        // The same text gives the same fingerprint, because the seed is in it.
        assert_eq!(serde_json::from_str::<Fingerprint>(text).unwrap(), fp);
    }

    #[test]
    fn json_in_the_old_flat_shape_is_an_error_and_not_a_silent_loss() {
        let old = r#"{ "os_type": "Windows", "hardware_concurrency": 8,
            "screen_width": 1920, "flags": { "navigator_masking": "Custom" } }"#;
        let error = serde_json::from_str::<Fingerprint>(old)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field"), "{error}");
    }

    #[test]
    fn json_with_an_off_where_it_means_nothing_is_an_error() {
        let error = serde_json::from_str::<Fingerprint>(r#"{ "timezone": "off" }"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("timezone cannot be Off"), "{error}");
    }

    #[test]
    fn client_hints_saved_before_wow64_and_form_factors_still_load() {
        let raw = r#"{
            "brands": [], "full_version_list": [], "platform": "macOS",
            "platform_version": "15.0.0", "architecture": "arm", "bitness": "64",
            "model": "", "ua_full_version": "155.0.0.0", "mobile": false
        }"#;
        let hints: ClientHints = serde_json::from_str(raw).unwrap();
        assert!(!hints.wow64);
        assert!(hints.form_factors.is_empty());
        assert_eq!(hints.platform, "macOS");
    }

    #[test]
    fn client_hints_carry_wow64_and_form_factors_through_serde() {
        let hints = ClientHints {
            wow64: true,
            form_factors: vec!["Desktop".to_owned()],
            ..ClientHints::default()
        };
        let again: ClientHints =
            serde_json::from_str(&serde_json::to_string(&hints).unwrap()).unwrap();
        assert_eq!(again, hints);
    }

    // ------------------------------------------------------------------
    // validate()
    // ------------------------------------------------------------------

    /// A report for a persona with `os` and the WebGL strings given, and nothing else.
    fn graphics_report(os: OsType, vendor: &str, renderer: &str) -> CoherenceReport {
        built(Fingerprint::builder().os_type(os).webgl((vendor, renderer))).validate()
    }

    #[test]
    fn a_renderer_from_another_operating_systems_graphics_api_is_an_error() {
        let direct3d = "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Direct3D11 vs_5_0 ps_5_0, D3D11)";
        let metal = "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)";
        let mesa = "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)";
        for (os, vendor, renderer, ok) in [
            (OsType::Windows, "Google Inc. (NVIDIA)", direct3d, true),
            (OsType::Linux, "Google Inc. (NVIDIA)", direct3d, false),
            (OsType::Macos, "Google Inc. (NVIDIA)", direct3d, false),
            (OsType::Android, "Google Inc. (NVIDIA)", direct3d, false),
            (OsType::Ios, "Google Inc. (NVIDIA)", direct3d, false),
            (OsType::Macos, "Google Inc. (Apple)", metal, true),
            (OsType::Ios, "Apple Inc.", "Apple GPU", true),
            (OsType::Windows, "Google Inc. (Apple)", metal, false),
            (OsType::Linux, "Google Inc. (Apple)", metal, false),
            (OsType::Android, "Apple Inc.", "Apple GPU", false),
            (OsType::Linux, "Google Inc. (Intel)", mesa, true),
            (OsType::Windows, "Google Inc. (Intel)", mesa, false),
            (OsType::Android, "Qualcomm", "Adreno (TM) 740", true),
            (OsType::Android, "ARM", "Mali-G715", true),
            (OsType::Windows, "Qualcomm", "Adreno (TM) 740", false),
            (OsType::Ios, "ARM", "Mali-G715", false),
            (
                OsType::Windows,
                "Imagination Technologies",
                "PowerVR Rogue GE8320",
                false,
            ),
        ] {
            let report = graphics_report(os, vendor, renderer);
            let flagged = report.errors.iter().any(|e| e.contains("WebGL"));
            assert_eq!(!flagged, ok, "{os:?} {renderer}: {:?}", report.errors);
        }
    }

    #[test]
    fn a_safari_persona_must_not_claim_chrome() {
        let chrome_ua =
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Chrome/155.0.0.0 Mobile/15E148 Safari/604.1";
        let fp = built(
            Fingerprint::builder()
                .browser_type(BrowserType::MobileSafari)
                .os_type(OsType::Ios)
                .user_agent(chrome_ua)
                .max_touch_points(5),
        );
        let report = fp.validate();
        assert!(
            report.errors.iter().any(|e| e.contains("Chrome/")),
            "{:?}",
            report.errors
        );

        // The real Safari persona, and a Firefox persona, are not flagged.
        assert!(Fingerprint::ios_mobile_safari()
            .validate()
            .errors
            .is_empty());
        let firefox = built(
            Fingerprint::builder()
                .browser_type(BrowserType::Firefox)
                .os_type(OsType::Linux)
                .user_agent(
                    "Mozilla/5.0 (X11; Linux x86_64; rv:153.0) Gecko/20100101 Firefox/153.0",
                ),
        );
        assert!(firefox.validate().errors.is_empty());
    }

    #[test]
    fn a_locale_from_another_country_than_the_time_zone_is_a_warning() {
        let warns = |locale: &str, zone: &str| {
            built(
                Fingerprint::builder()
                    .locale(locale)
                    .timezone(zone)
                    .geolocation((0.0, 0.0)),
            )
            .validate()
            .warnings
            .iter()
            .any(|w| w.contains("time zone"))
        };
        assert!(warns("de-DE", "Asia/Tokyo"));
        assert!(warns("ja-JP", "America/New_York"));
        assert!(!warns("de-DE", "Europe/Berlin"));
        assert!(!warns("en-US", "America/Chicago"));
        // A zone the table does not know, or a locale with no country, is not judged.
        assert!(!warns("en-US", "Pacific/Honolulu"));
        assert!(!warns("en", "Asia/Tokyo"));
    }

    #[test]
    fn a_time_zone_without_a_position_is_a_warning_only_when_permissions_are_granted() {
        let warns = |builder: FingerprintBuilder| {
            built(builder)
                .validate()
                .warnings
                .iter()
                .any(|w| w.contains("without geolocation"))
        };
        let granting = || {
            Fingerprint::builder()
                .timezone("Europe/Berlin")
                .grant_permissions(true)
        };
        // Granted, the page gets the real position next to the made-up zone.
        assert!(warns(granting()));
        assert!(!warns(granting().geolocation(Spoof::Off)));
        assert!(!warns(granting().geolocation((52.5, 13.4))));
        // Not granted, a request is asked, as a real browser asks it.
        assert!(!warns(Fingerprint::builder().timezone("Europe/Berlin")));
    }

    #[test]
    fn the_presets_leave_the_position_natural_and_ios_claims_no_memory_or_hints() {
        for preset in [
            Fingerprint::windows_desktop(),
            Fingerprint::edge_desktop(),
            Fingerprint::macos_desktop(),
            Fingerprint::linux_desktop(),
            Fingerprint::android_mobile(),
            Fingerprint::ios_mobile_safari(),
        ] {
            // A position is answered without a prompt, which a fresh browser
            // does not do.
            assert!(preset.geolocation().is_natural());
            assert!(preset.timezone().is_value());
        }
        let ios = Fingerprint::ios_mobile_safari();
        assert!(ios.device_memory().is_natural());
        assert!(ios.client_hints().is_off());
    }

    #[test]
    fn the_default_fingerprint_is_the_one_a_builder_with_nothing_set_builds() {
        assert_eq!(Fingerprint::default(), built(Fingerprint::builder()));
    }

    #[test]
    fn json_without_an_os_type_takes_it_from_the_user_agent() {
        let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";
        let fp: Fingerprint = serde_json::from_value(serde_json::json!({ "user_agent": mac }))
            .expect("a fingerprint without an os_type");
        assert_eq!(fp.os_type(), OsType::Macos);
        assert_eq!(fp.platform().as_str(), Some("MacIntel"));
        // An os_type that the JSON gives is kept, whatever the user agent says.
        let kept: Fingerprint =
            serde_json::from_value(serde_json::json!({ "os_type": "Windows", "user_agent": mac }))
                .expect("a fingerprint with an os_type");
        assert_eq!(kept.os_type(), OsType::Windows);
        // And one that is built round-trips with its os_type.
        let round: Fingerprint =
            serde_json::from_str(&serde_json::to_string(&fp).expect("serialises"))
                .expect("reads back");
        assert_eq!(round, fp);
    }

    #[test]
    fn a_screen_of_no_pixels_is_an_error() {
        let report = built(Fingerprint::builder().screen(0, 1080)).validate();
        assert!(report.errors.iter().any(|e| e.contains("screen")));
    }
}
