//! Fingerprint / anti-detection profile configuration.
//!
//! A [`Fingerprint`] captures the browser personality dimensions exposed by
//! External anti-detect browser profile APIs: navigator, screen, timezone, geolocation,
//! WebGL, media devices, fonts, proxy, and masking flags.
//!
//! Use [`Fingerprint::builder()`] to construct a profile, then pass it to
//! [`StealthOptions`](crate::stealth::options::StealthOptions) or launch a
//! [`PlaywrightSession`](crate::browser::playwright::PlaywrightSession) with it.
//!
//! # Example
//!
//! ```rust
//! use seleniumbase_rs::stealth::fingerprint::{Fingerprint, OsType};
//!
//! let fp = Fingerprint::builder()
//!     .os_type(OsType::Windows)
//!     .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …")
//!     .screen(1920, 1080)
//!     .hardware_concurrency(8)
//!     .build();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Browser persona requested by a profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// Masking mode for a fingerprint dimension.
///
/// The default is [`Auto`](MaskingMode::Auto): the dimension is on when the
/// fingerprint has a value for it, so giving a value is enough to use it.
/// [`Fingerprint::is_masked`] is the one place that decides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MaskingMode {
    /// On when the fingerprint has a value for the dimension, off otherwise.
    #[default]
    Auto,
    /// Explicitly off: the browser's own value, whatever the fingerprint holds.
    Natural,
    /// On. A value the fingerprint lacks is filled with a generic one.
    Mask,
    /// The same as [`Mask`](MaskingMode::Mask). Deprecated: use `Mask`.
    Custom,
    /// The same as [`Natural`](MaskingMode::Natural). Deprecated: use `Natural`.
    Disabled,
}

impl MaskingMode {
    /// Whether a dimension in this mode is on, given whether the fingerprint
    /// `has_value` for it.
    #[must_use]
    pub fn resolve(self, has_value: bool) -> bool {
        match self {
            Self::Auto => has_value,
            Self::Mask | Self::Custom => true,
            Self::Natural | Self::Disabled => false,
        }
    }

    /// Whether a value the fingerprint lacks is filled with a generic one.
    #[must_use]
    pub fn fills_defaults(self) -> bool {
        matches!(self, Self::Mask | Self::Custom)
    }

    /// Whether this mode switches the dimension off explicitly.
    #[must_use]
    pub fn is_off(self) -> bool {
        matches!(self, Self::Natural | Self::Disabled)
    }
}

/// Noise/masking strategy used by `graphics_noise` and `ports_masking`.
///
/// For `ports_masking`, `Natural` and `Off` leave every port open and any other
/// mode blocks the listed ports. `graphics_noise` is ignored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NoiseMode {
    /// Add deterministic noise / active masking.
    #[default]
    Mask,
    /// Use natural rendering / allow all.
    Natural,
    /// Turn the feature off entirely.
    Off,
    /// Block / deny (used by `ports_masking`).
    Block,
    /// Randomized output.
    Random,
    /// Low intensity (used by `graphics_noise`).
    Low,
    /// Medium intensity (used by `graphics_noise`).
    Medium,
    /// High intensity (used by `graphics_noise`).
    High,
}

/// Canvas-specific noise mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CanvasNoiseMode {
    /// Noise when the fingerprint has a [`seed`](Fingerprint::seed), none
    /// otherwise.
    #[default]
    Auto,
    /// Deterministic noise from the profile seed, derived from the user agent
    /// when there is no seed.
    Mask,
    /// No noise: the browser's own canvas.
    Natural,
    /// No noise, the same as [`Natural`](CanvasNoiseMode::Natural).
    Disabled,
    /// Noise from a fresh seed each time the script is generated, so each
    /// browser context gets different noise.
    Random,
    /// Deterministic noise tied to the profile seed, the same as
    /// [`Mask`](CanvasNoiseMode::Mask).
    Persistent,
    /// Seeded noise on about one pixel in eight.
    Low,
}

impl CanvasNoiseMode {
    /// Whether this mode adds canvas noise whatever the fingerprint holds.
    /// [`Auto`](CanvasNoiseMode::Auto) does not: it depends on the seed, which
    /// [`Fingerprint::is_masked`] checks.
    #[must_use]
    pub fn adds_noise(self) -> bool {
        matches!(
            self,
            Self::Mask | Self::Persistent | Self::Random | Self::Low
        )
    }

    /// How sparse the noise is: it touches one pixel in this many.
    #[must_use]
    pub fn density(self) -> u32 {
        if self == Self::Low {
            8
        } else {
            1
        }
    }
}

/// Geolocation permission popup behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PopupMode {
    /// `Allow` when the fingerprint has a position, `Prompt` otherwise. See
    /// [`Fingerprint::geolocation_popup`].
    #[default]
    Auto,
    /// Leave the browser's own permission prompt.
    Prompt,
    /// Answer the request with the fingerprint's position, without asking.
    Allow,
    /// Answer the request as a user who pressed "Block" would.
    Block,
}

/// Proxy masking mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProxyMaskingMode {
    /// Use the proxy when the fingerprint has one with a host.
    #[default]
    Auto,
    /// Never use the fingerprint's proxy.
    Disabled,
    /// Use the fingerprint's proxy, whatever its host.
    Custom,
    /// The same as [`Custom`](ProxyMaskingMode::Custom). Deprecated: the proxy's
    /// own `type` says how to reach it.
    Socks5,
    /// The same as [`Custom`](ProxyMaskingMode::Custom). Deprecated.
    Http,
    /// The same as [`Custom`](ProxyMaskingMode::Custom). Deprecated.
    Https,
    /// The same as [`Disabled`](ProxyMaskingMode::Disabled). Deprecated.
    Direct,
}

/// QUIC mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum QuicMode {
    #[default]
    Natural,
    Disabled,
    Enabled,
    ForceHttp2,
    Auto,
}

/// Startup behaviour for a profile session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StartupBehavior {
    /// Restore previous session tabs.
    #[default]
    Recover,
    /// Open custom start URLs.
    Custom,
}

/// Masking flags controlling which dimensions are spoofed.
///
/// Every mode defaults to `Auto`: a dimension is on when the fingerprint has a
/// value for it ([`Fingerprint::is_masked`]), so a fingerprint built from values
/// needs no flags at all. Set a mode to [`Natural`](MaskingMode::Natural) to
/// switch a dimension off whatever the fingerprint holds, or to
/// [`Mask`](MaskingMode::Mask) to switch it on and fill the values it lacks.
///
/// The dimensions that carry no value of their own (bluetooth, chrome runtime,
/// headless fixes) are off under `Auto`; `native_tostring_masking` is on under
/// `Auto`, because the other evasions rely on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StealthFlags {
    #[serde(default)]
    pub webrtc_masking: MaskingMode,
    #[serde(default)]
    pub audio_masking: MaskingMode,
    /// Ignored. It never drove graphics noise; the Modernizr hairline fix it
    /// used to gate follows [`headless_masking`](StealthFlags::headless_masking).
    /// Deprecated; kept so saved fingerprints still load.
    #[serde(default)]
    pub graphics_noise: NoiseMode,
    /// How a geolocation request is answered. See [`PopupMode`].
    #[serde(default)]
    pub geolocation_popup: PopupMode,
    #[serde(default)]
    pub navigator_masking: MaskingMode,
    #[serde(default)]
    pub localization_masking: MaskingMode,
    #[serde(default)]
    pub timezone_masking: MaskingMode,
    #[serde(default)]
    pub graphics_masking: MaskingMode,
    #[serde(default)]
    pub fonts_masking: MaskingMode,
    #[serde(default)]
    pub media_devices_masking: MaskingMode,
    #[serde(default)]
    pub screen_masking: MaskingMode,
    #[serde(default)]
    pub geolocation_masking: MaskingMode,
    /// Whether the fingerprint's [`ports`](Fingerprint::ports) are blocked.
    /// `Natural` and `Off` leave them open.
    #[serde(default)]
    pub ports_masking: NoiseMode,
    #[serde(default)]
    pub proxy_masking: ProxyMaskingMode,
    #[serde(default)]
    pub quic_mode: QuicMode,
    #[serde(default)]
    pub canvas_noise: CanvasNoiseMode,
    /// Stored only; nothing reads it.
    #[serde(default)]
    pub startup_behavior: StartupBehavior,

    // --- Extended dimensions (appended for backward compatibility) ---
    /// Mask the Battery Status API (`navigator.getBattery`).
    #[serde(default)]
    pub battery_masking: MaskingMode,
    /// Mask the Network Information API (`navigator.connection`).
    #[serde(default)]
    pub connection_masking: MaskingMode,
    /// Spoof `speechSynthesis.getVoices()`.
    #[serde(default)]
    pub speech_masking: MaskingMode,
    /// Stub the Web Bluetooth API.
    #[serde(default)]
    pub bluetooth_masking: MaskingMode,
    /// Override `navigator.userAgentData` (Client Hints).
    #[serde(default)]
    pub client_hints_masking: MaskingMode,
    /// Mask `Function.prototype.toString` so patched natives look native.
    #[serde(default)]
    pub native_tostring_masking: MaskingMode,
    /// Install `window.chrome` runtime/app/csi/loadTimes stubs.
    #[serde(default)]
    pub chrome_runtime_masking: MaskingMode,
    /// Apply headless-mode fixes (matchMedia, permissions consistency).
    #[serde(default)]
    pub headless_masking: MaskingMode,
    /// Ignored: the switch is [`Fingerprint::humanize`]'s `enabled`.
    /// Deprecated; kept so saved fingerprints still load.
    #[serde(default)]
    pub humanize: bool,
    /// Emit a tracker/fingerprint-script domain block list.
    #[serde(default)]
    pub block_trackers: bool,
    /// Bypass Content-Security-Policy for the page context.
    #[serde(default)]
    pub disable_csp: bool,
    /// Grant common browser permissions at startup so permission prompts do
    /// not fire (a bot-detection signal).
    #[serde(default)]
    pub grant_permissions: bool,
    /// Apply spoofing through CDP / launch args instead of JavaScript where
    /// possible. This makes the spoof invisible to page-side introspection
    /// (`toString`, `getOwnPropertyNames`, etc.) because the browser's own
    /// implementation returns the fake value.
    #[serde(default)]
    pub native_spoofing: bool,
    /// Force every closed shadow root open (`Element.attachShadow` with
    /// `mode: 'open'`), so automation can reach into it. A page can detect this,
    /// so turn it off where that matters. On by default.
    #[serde(default = "default_open_shadow_roots")]
    pub open_shadow_roots: bool,
}

fn default_open_shadow_roots() -> bool {
    true
}

impl Default for StealthFlags {
    /// Every dimension on `Auto` (on when the fingerprint has a value for it),
    /// the choices that carry no value off, shadow roots opened.
    fn default() -> Self {
        Self {
            webrtc_masking: MaskingMode::Auto,
            audio_masking: MaskingMode::Auto,
            graphics_noise: NoiseMode::Mask,
            geolocation_popup: PopupMode::Auto,
            navigator_masking: MaskingMode::Auto,
            localization_masking: MaskingMode::Auto,
            timezone_masking: MaskingMode::Auto,
            graphics_masking: MaskingMode::Auto,
            fonts_masking: MaskingMode::Auto,
            media_devices_masking: MaskingMode::Auto,
            screen_masking: MaskingMode::Auto,
            geolocation_masking: MaskingMode::Auto,
            ports_masking: NoiseMode::Mask,
            proxy_masking: ProxyMaskingMode::Auto,
            quic_mode: QuicMode::Natural,
            canvas_noise: CanvasNoiseMode::Auto,
            startup_behavior: StartupBehavior::Recover,
            battery_masking: MaskingMode::Auto,
            connection_masking: MaskingMode::Auto,
            speech_masking: MaskingMode::Auto,
            bluetooth_masking: MaskingMode::Auto,
            client_hints_masking: MaskingMode::Auto,
            native_tostring_masking: MaskingMode::Auto,
            chrome_runtime_masking: MaskingMode::Auto,
            headless_masking: MaskingMode::Auto,
            humanize: false,
            block_trackers: false,
            disable_csp: false,
            grant_permissions: false,
            native_spoofing: false,
            open_shadow_roots: true,
        }
    }
}

impl StealthFlags {
    /// The flags of the built-in presets.
    ///
    /// The dimensions that carry the fingerprint's own values (navigator,
    /// localization, time zone, screen, graphics, fonts, media devices,
    /// geolocation, audio, proxy) stay on `Auto`, so each is on exactly when a
    /// value was given, in whatever order the flags and the values were set.
    /// The dimensions with no persona value of their own, or a generic fill,
    /// are `Mask`: WebRTC (leak protection and the launch policy), canvas
    /// noise, battery, connection, speech, client hints, bluetooth, chrome
    /// runtime, headless fixes and native `toString`.
    #[must_use]
    pub fn balanced() -> Self {
        Self {
            webrtc_masking: MaskingMode::Mask,
            canvas_noise: CanvasNoiseMode::Mask,
            battery_masking: MaskingMode::Mask,
            connection_masking: MaskingMode::Mask,
            speech_masking: MaskingMode::Mask,
            bluetooth_masking: MaskingMode::Mask,
            client_hints_masking: MaskingMode::Mask,
            native_tostring_masking: MaskingMode::Mask,
            chrome_runtime_masking: MaskingMode::Mask,
            headless_masking: MaskingMode::Mask,
            ..Self::default()
        }
    }

    /// Every maskable dimension on, with generic values filled in where the
    /// fingerprint has none.
    #[must_use]
    pub fn mask_all() -> Self {
        Self {
            webrtc_masking: MaskingMode::Mask,
            audio_masking: MaskingMode::Mask,
            geolocation_popup: PopupMode::Auto,
            navigator_masking: MaskingMode::Mask,
            localization_masking: MaskingMode::Mask,
            timezone_masking: MaskingMode::Mask,
            graphics_masking: MaskingMode::Mask,
            fonts_masking: MaskingMode::Mask,
            media_devices_masking: MaskingMode::Mask,
            screen_masking: MaskingMode::Mask,
            geolocation_masking: MaskingMode::Mask,
            proxy_masking: ProxyMaskingMode::Custom,
            canvas_noise: CanvasNoiseMode::Mask,
            battery_masking: MaskingMode::Mask,
            connection_masking: MaskingMode::Mask,
            speech_masking: MaskingMode::Mask,
            bluetooth_masking: MaskingMode::Mask,
            client_hints_masking: MaskingMode::Mask,
            native_tostring_masking: MaskingMode::Mask,
            chrome_runtime_masking: MaskingMode::Mask,
            headless_masking: MaskingMode::Mask,
            ..Self::default()
        }
    }

    /// Every maskable dimension on. Deprecated: use [`mask_all`](Self::mask_all);
    /// `Custom` was always the same as `Mask`.
    #[deprecated(note = "use StealthFlags::mask_all; Custom is the same as Mask")]
    #[must_use]
    pub fn all_custom() -> Self {
        Self::mask_all()
    }
}

/// WebRTC IP-handling policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WebRtcPolicy {
    #[default]
    DisableNonProxiedUdp,
    PublicAndPrivateInterfaces,
    PublicInterfaceOnly,
}

/// Proxy configuration for a profile.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub r#type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_traffic: bool,
}

// Written by hand so that logging a fingerprint never prints the password.
impl std::fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

/// Battery Status API profile (`navigator.getBattery()`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryProfile {
    pub charging: bool,
    /// Battery level in the range `0.0..=1.0`.
    pub level: f64,
    /// Seconds until fully charged (`f64::INFINITY` when unknown).
    pub charging_time: f64,
    /// Seconds until empty (`f64::INFINITY` when charging).
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechVoice {
    pub name: String,
    pub lang: String,
    pub default: bool,
    pub local_service: bool,
    pub voice_uri: String,
}

impl SpeechVoice {
    /// Returns a plausible default set of English voices.
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDeviceSpec {
    pub kind: String,
    pub device_id: String,
    pub group_id: String,
    pub label: String,
}

/// User-Agent Client Hints (`navigator.userAgentData`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// A part of a browser's identity that a fingerprint can mask.
///
/// [`Fingerprint::is_masked`] answers, for one of these, whether the
/// fingerprint spoofs it. Each dimension has a flag in [`StealthFlags`] and the
/// fingerprint values that switch it on under `Auto`.
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
    /// The Web Bluetooth stub.
    Bluetooth,
    /// The `window.chrome` stubs.
    ChromeRuntime,
    /// The headless fixes (permissions, matchMedia, hairline).
    Headless,
    /// `Function.prototype.toString` masking.
    NativeToString,
}

impl Dimension {
    /// The bit this dimension uses in a set of dimensions.
    const fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// Complete browser fingerprint / anti-detection profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    #[serde(default)]
    pub browser_type: BrowserType,
    #[serde(default)]
    pub os_type: OsType,
    pub core_version: Option<u32>,

    // Navigator
    pub user_agent: Option<String>,
    pub platform: Option<String>,
    pub hardware_concurrency: Option<u32>,
    pub device_memory: Option<f64>,
    pub max_touch_points: Option<u32>,
    /// `navigator.vendor` (e.g. `Google Inc.`).
    #[serde(default)]
    pub vendor: Option<String>,
    /// `navigator.oscpu` (Firefox persona).
    #[serde(default)]
    pub oscpu: Option<String>,
    /// `navigator.productSub` (usually `20030107` for Chromium).
    #[serde(default)]
    pub product_sub: Option<String>,
    /// `navigator.buildID` (Firefox persona).
    #[serde(default)]
    pub build_id: Option<String>,
    /// `navigator.appVersion` override.
    #[serde(default)]
    pub app_version: Option<String>,

    // Localization
    pub locale: Option<String>,
    pub languages: Option<String>,
    pub accept_languages: Option<String>,

    // Timezone
    pub timezone: Option<String>,

    // Screen
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
    /// `screen.availWidth`. When `None`, the screen's width.
    #[serde(default)]
    pub avail_width: Option<u32>,
    /// `screen.availHeight`. When `None`, the screen's height less 40 pixels.
    #[serde(default)]
    pub avail_height: Option<u32>,
    pub pixel_ratio: Option<f64>,
    pub color_depth: Option<u32>,

    // WebGL
    pub webgl_vendor: Option<String>,
    pub webgl_renderer: Option<String>,
    /// Optional GPU vendor id (e.g. `0x10de`). Not all browsers expose this.
    #[serde(default)]
    pub webgl_vendor_id: Option<String>,
    /// Optional GPU renderer id (e.g. `0x1f91`). Not all browsers expose this.
    #[serde(default)]
    pub webgl_renderer_id: Option<String>,

    // Media devices
    pub audio_inputs: Option<u32>,
    pub audio_outputs: Option<u32>,
    pub video_inputs: Option<u32>,
    /// Explicitly labeled media devices (overrides the default generated labels).
    #[serde(default)]
    pub media_devices: Vec<MediaDeviceSpec>,

    // Geolocation
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    pub accuracy: Option<f64>,

    // Fonts
    #[serde(default)]
    pub fonts: Vec<String>,

    // Proxy
    pub proxy: Option<ProxyConfig>,

    // WebRTC
    /// The WebRTC IP-handling policy the browser is launched with. `None`
    /// passes no policy flag, unless WebRTC is masked, which uses
    /// [`WebRtcPolicy::DisableNonProxiedUdp`].
    #[serde(default)]
    pub webrtc_policy: Option<WebRtcPolicy>,
    /// Public IP reported by WebRTC when the masking mode supplies one.
    #[serde(default)]
    pub webrtc_public_ip: Option<String>,
    /// Local IP reported by WebRTC when the masking mode supplies one.
    #[serde(default)]
    pub webrtc_local_ip: Option<String>,

    // Ports
    #[serde(default)]
    pub ports: Vec<u16>,

    // Storage
    #[serde(default)]
    pub local_storage: bool,
    #[serde(default)]
    pub save_service_worker: bool,

    // Start URLs
    #[serde(default)]
    pub custom_start_urls: Vec<String>,

    // Command-line params injected into the browser.
    #[serde(default)]
    pub cmd_params: HashMap<String, String>,

    // --- Extended dimensions (appended for backward compatibility) ---
    /// Deterministic canvas noise seed. Giving one turns canvas noise on. When
    /// `None` and the canvas mode is `Mask`, a seed is derived from the
    /// user-agent so repeated sessions with the same persona are stable.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Deterministic audio noise seed. Giving one turns audio noise on. When
    /// `None` and the audio mode is `Mask`, it is derived from
    /// [`seed_value`](Fingerprint::seed_value).
    #[serde(default)]
    pub audio_seed: Option<u64>,
    /// Battery Status API profile.
    #[serde(default)]
    pub battery: Option<BatteryProfile>,
    /// Network Information API profile.
    #[serde(default)]
    pub connection: Option<ConnectionProfile>,
    /// Synthesized speech voices.
    #[serde(default)]
    pub speech_voices: Vec<SpeechVoice>,
    /// User-Agent Client Hints.
    #[serde(default)]
    pub client_hints: Option<ClientHints>,
    /// Humanized input timing configuration.
    #[serde(default)]
    pub humanize: HumanizeConfig,
    /// Custom tracker host block list (empty falls back to
    /// [`default_tracker_hosts`]).
    #[serde(default)]
    pub blocked_trackers: Vec<String>,

    // Masking flags
    #[serde(default)]
    pub flags: StealthFlags,
}

impl Fingerprint {
    /// Returns a builder for fluent construction.
    pub fn builder() -> FingerprintBuilder {
        FingerprintBuilder::default()
    }

    /// Returns the deterministic noise seed for this profile.
    ///
    /// Uses the explicit [`seed`](Fingerprint::seed) when set, otherwise
    /// derives a stable value from the user-agent (falling back to a constant
    /// when no user-agent is present) so noise is reproducible per persona.
    pub fn seed_value(&self) -> u64 {
        self.seed.unwrap_or_else(|| {
            self.user_agent
                .as_deref()
                .map_or(0x9E37_79B9_7F4A_7C15, |ua| fnv1a(ua.as_bytes()))
        })
    }

    /// Whether this fingerprint spoofs `dimension`.
    ///
    /// This is the one predicate the page-script providers, the launch
    /// arguments, the protocol overrides and the proxy all ask. Under `Auto`
    /// (the default of every mode) a dimension is on when the fingerprint has a
    /// value for it; `Mask` is on and fills the values that are missing;
    /// `Natural` is off whatever the fingerprint holds. The few dimensions
    /// without a value of their own are off under `Auto`, except
    /// [`Dimension::NativeToString`], which is on unless switched off.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::stealth::fingerprint::{Dimension, Fingerprint};
    ///
    /// let fp = Fingerprint::builder().timezone("Europe/Berlin").build();
    /// assert!(fp.is_masked(Dimension::Timezone));
    /// assert!(!fp.is_masked(Dimension::Screen));
    /// ```
    #[must_use]
    pub fn is_masked(&self, dimension: Dimension) -> bool {
        let f = &self.flags;
        match dimension {
            Dimension::Navigator => f.navigator_masking.resolve(
                self.user_agent.is_some()
                    || self.platform.is_some()
                    || self.hardware_concurrency.is_some()
                    || self.device_memory.is_some()
                    || self.max_touch_points.is_some()
                    || self.vendor.is_some()
                    || self.oscpu.is_some()
                    || self.product_sub.is_some()
                    || self.build_id.is_some()
                    || self.app_version.is_some(),
            ),
            Dimension::Localization => f.localization_masking.resolve(
                self.locale.is_some()
                    || self.languages.is_some()
                    || self.accept_languages.is_some(),
            ),
            Dimension::Timezone => f.timezone_masking.resolve(self.timezone.is_some()),
            Dimension::Screen => f.screen_masking.resolve(
                self.screen_width.is_some()
                    || self.screen_height.is_some()
                    || self.avail_width.is_some()
                    || self.avail_height.is_some()
                    || self.pixel_ratio.is_some()
                    || self.color_depth.is_some(),
            ),
            Dimension::Graphics => f
                .graphics_masking
                .resolve(self.webgl_vendor.is_some() || self.webgl_renderer.is_some()),
            Dimension::Fonts => f.fonts_masking.resolve(!self.fonts.is_empty()),
            Dimension::MediaDevices => f.media_devices_masking.resolve(
                self.audio_inputs.is_some()
                    || self.audio_outputs.is_some()
                    || self.video_inputs.is_some()
                    || !self.media_devices.is_empty(),
            ),
            Dimension::Geolocation => f
                .geolocation_masking
                .resolve(self.latitude.is_some() && self.longitude.is_some()),
            Dimension::WebRtc => f.webrtc_masking.resolve(
                self.webrtc_public_ip.is_some()
                    || self.webrtc_local_ip.is_some()
                    || self.webrtc_policy.is_some(),
            ),
            Dimension::Canvas => match f.canvas_noise {
                CanvasNoiseMode::Auto => self.seed.is_some(),
                mode => mode.adds_noise(),
            },
            Dimension::Audio => f.audio_masking.resolve(self.audio_seed.is_some()),
            Dimension::Battery => f.battery_masking.resolve(self.battery.is_some()),
            Dimension::Connection => f.connection_masking.resolve(self.connection.is_some()),
            Dimension::Speech => f.speech_masking.resolve(!self.speech_voices.is_empty()),
            Dimension::ClientHints => f.client_hints_masking.resolve(self.client_hints.is_some()),
            Dimension::Proxy => match f.proxy_masking {
                ProxyMaskingMode::Auto => self.proxy.as_ref().is_some_and(|p| !p.host.is_empty()),
                ProxyMaskingMode::Disabled | ProxyMaskingMode::Direct => false,
                _ => self.proxy.is_some(),
            },
            Dimension::Ports => {
                !self.ports.is_empty()
                    && !matches!(f.ports_masking, NoiseMode::Natural | NoiseMode::Off)
            }
            Dimension::Trackers => f.block_trackers || !self.blocked_trackers.is_empty(),
            Dimension::Bluetooth => f.bluetooth_masking.resolve(false),
            Dimension::ChromeRuntime => f.chrome_runtime_masking.resolve(false),
            Dimension::Headless => f.headless_masking.resolve(false),
            Dimension::NativeToString => !f.native_tostring_masking.is_off(),
        }
    }

    /// Whether `dimension` fills the values the fingerprint lacks with generic
    /// ones: its mode is `Mask`. A value the fingerprint has is always used.
    #[must_use]
    pub fn fills_defaults(&self, dimension: Dimension) -> bool {
        let f = &self.flags;
        match dimension {
            Dimension::Navigator => f.navigator_masking.fills_defaults(),
            Dimension::Localization => f.localization_masking.fills_defaults(),
            Dimension::Timezone => f.timezone_masking.fills_defaults(),
            Dimension::Screen => f.screen_masking.fills_defaults(),
            Dimension::Graphics => f.graphics_masking.fills_defaults(),
            Dimension::Fonts => f.fonts_masking.fills_defaults(),
            Dimension::MediaDevices => f.media_devices_masking.fills_defaults(),
            Dimension::Geolocation => f.geolocation_masking.fills_defaults(),
            Dimension::WebRtc => f.webrtc_masking.fills_defaults(),
            Dimension::Canvas => f.canvas_noise.adds_noise(),
            Dimension::Audio => f.audio_masking.fills_defaults(),
            Dimension::Battery => f.battery_masking.fills_defaults(),
            Dimension::Connection => f.connection_masking.fills_defaults(),
            Dimension::Speech => f.speech_masking.fills_defaults(),
            Dimension::ClientHints => f.client_hints_masking.fills_defaults(),
            Dimension::Bluetooth => f.bluetooth_masking.fills_defaults(),
            Dimension::ChromeRuntime => f.chrome_runtime_masking.fills_defaults(),
            Dimension::Headless => f.headless_masking.fills_defaults(),
            Dimension::NativeToString => f.native_tostring_masking.fills_defaults(),
            Dimension::Proxy | Dimension::Ports | Dimension::Trackers => false,
        }
    }

    /// How a geolocation request is answered, with `Auto` resolved: `Allow`
    /// when the fingerprint has a position, `Prompt` when it has none.
    #[must_use]
    pub fn geolocation_popup(&self) -> PopupMode {
        match self.flags.geolocation_popup {
            PopupMode::Auto if self.latitude.is_some() && self.longitude.is_some() => {
                PopupMode::Allow
            }
            PopupMode::Auto => PopupMode::Prompt,
            mode => mode,
        }
    }

    /// Validates cross-field coherence and returns a [`CoherenceReport`].
    ///
    /// This catches common mismatches such as a macOS user-agent paired with a
    /// `Win32` platform, or a mobile user-agent without touch points, that make
    /// an otherwise convincing profile easy to detect.
    pub fn validate(&self) -> CoherenceReport {
        let mut report = CoherenceReport::default();
        let ua = self.user_agent.as_deref().unwrap_or("");
        let ua_lower = ua.to_lowercase();

        let platform = self
            .platform
            .as_deref()
            .unwrap_or_else(|| self.os_type.platform());

        // OS persona vs navigator.platform.
        let platform_ok = match self.os_type {
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
                "navigator.platform '{platform}' does not match OS persona {:?}",
                self.os_type
            ));
        }

        // OS persona vs user-agent token.
        if !ua.is_empty() {
            let ua_os_ok = match self.os_type {
                OsType::Windows => ua_lower.contains("windows"),
                OsType::Macos => ua_lower.contains("mac os"),
                OsType::Linux => ua_lower.contains("linux") || ua_lower.contains("x11"),
                OsType::Android => ua_lower.contains("android"),
                OsType::Ios => ua_lower.contains("iphone") || ua_lower.contains("ipad"),
            };
            if !ua_os_ok {
                report.errors.push(format!(
                    "user-agent does not contain a token for OS persona {:?}",
                    self.os_type
                ));
            }
        }

        // Mobile user-agents need touch points.
        let is_mobile_ua = ua_lower.contains("mobile")
            || ua_lower.contains("android")
            || ua_lower.contains("iphone")
            || ua_lower.contains("ipad");
        if is_mobile_ua && self.max_touch_points.unwrap_or(0) == 0 {
            report
                .warnings
                .push("mobile user-agent but maxTouchPoints is 0".to_owned());
        }
        if !is_mobile_ua && self.max_touch_points.unwrap_or(0) > 0 {
            report
                .warnings
                .push("desktop user-agent but maxTouchPoints is greater than 0".to_owned());
        }

        // Locale vs primary language.
        if let (Some(locale), Some(langs)) = (self.locale.as_deref(), self.languages.as_deref()) {
            let primary = langs.split(',').next().unwrap_or("").trim();
            if !primary.is_empty() && !primary.eq_ignore_ascii_case(locale) {
                report.warnings.push(format!(
                    "locale '{locale}' differs from primary language '{primary}'"
                ));
            }
        }

        // Timezone present but no geolocation (soft).
        if self.timezone.is_some() && self.latitude.is_none() {
            report
                .warnings
                .push("timezone set without geolocation; consider matching coordinates".to_owned());
        }

        // Screen sanity.
        if let (Some(w), Some(h)) = (self.screen_width, self.screen_height) {
            if w == 0 || h == 0 {
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
        let named = format!(
            "{} {}",
            self.webgl_vendor.as_deref().unwrap_or(""),
            self.webgl_renderer.as_deref().unwrap_or("")
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
            if !systems.contains(&self.os_type) {
                report.errors.push(format!(
                    "WebGL renderer '{}' is a {api} renderer, which {:?} does not report",
                    self.webgl_renderer.as_deref().unwrap_or(""),
                    self.os_type
                ));
            }
        }

        // The generic renderer `Mask` fills in is an Intel one worded for a Mac.
        if self.fills_defaults(Dimension::Graphics)
            && (self.webgl_vendor.is_none() || self.webgl_renderer.is_none())
            && !matches!(self.os_type, OsType::Macos | OsType::Ios)
        {
            report.warnings.push(format!(
                "WebGL is masked without a vendor and a renderer, so the generic Mac-style Intel \
                 renderer fills in, which does not match {:?}",
                self.os_type
            ));
        }
    }

    /// The browser persona against its user agent.
    fn check_browser_persona(&self, ua: &str, report: &mut CoherenceReport) {
        if self.browser_type == BrowserType::MobileSafari && ua.contains("Chrome/") {
            report.errors.push(
                "browser persona MobileSafari but the user agent has a Chrome/ token".to_owned(),
            );
        }
    }

    /// The locale against the time zone: a locale is spoken in its own country,
    /// so a time zone the table of places puts in another one is a tell.
    fn check_place(&self, report: &mut CoherenceReport) {
        let (Some(locale), Some(zone)) = (self.locale.as_deref(), self.timezone.as_deref()) else {
            return;
        };
        let region = locale
            .rsplit_once('-')
            .map(|(_, region)| region)
            .filter(|region| region.len() == 2);
        let country = super::randomize::locale_of_timezone(zone).map(|l| l.country);
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
    /// every field from them:
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
    /// - **Noise**: a canvas and audio noise [`seed`](Fingerprint::seed), so the
    ///   noise is stable for this identity and different for the next.
    ///
    /// The result passes [`validate`](Fingerprint::validate) without errors or
    /// warnings for every seed the tests try: 1,500 per OS, and edge cases such
    /// as `0` and `u64::MAX`. The masking flags are [`StealthFlags::balanced`], so the
    /// values are applied as they are.
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
    /// let renderer = fp.webgl_renderer.as_deref().unwrap_or_default();
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
    /// - Fonts and media devices are left unset, and the proxy is left off.
    /// - It describes an identity, not a browser. Give it to a browser with
    ///   [`BrowserConfig::with_fingerprint`](crate::BrowserConfig::with_fingerprint)
    ///   (WebDriver and UC sessions), with
    ///   [`LaunchOptionsBuilder::fingerprint`](crate::sb_cdp::LaunchOptionsBuilder::fingerprint)
    ///   (the pure CDP engine, for every tab), or with the evasion registry. The
    ///   browser itself stays what it is: a fingerprint of another operating
    ///   system does not change the binary's real behaviour.
    #[must_use]
    pub fn randomized(os: OsType, seed: u64) -> Self {
        super::randomize::generate(os, seed, None)
    }

    /// Like [`Fingerprint::randomized`], but with the Chromium major fixed to `major`, so the
    /// user agent and Client Hints claim the release that the measured TLS and HTTP/2 of the
    /// identity come from. Safari and iOS have no Chromium major and ignore `major`.
    #[must_use]
    pub fn randomized_with_major(os: OsType, seed: u64, major: u32) -> Self {
        super::randomize::generate(os, seed, Some(major))
    }

    /// Returns the tracker host block list, defaulting to
    /// [`default_tracker_hosts`] when none were supplied.
    pub fn tracker_hosts(&self) -> Vec<String> {
        if self.blocked_trackers.is_empty() {
            default_tracker_hosts()
        } else {
            self.blocked_trackers.clone()
        }
    }

    /// Quick preset for a Windows desktop Chrome profile.
    ///
    /// The Chrome presets claim
    /// [`PRESET_CHROMIUM_MAJOR`](super::randomize::PRESET_CHROMIUM_MAJOR), the
    /// newest major a randomized identity may claim, in Chrome's frozen user-agent
    /// format. They set no Client Hints; the evasions derive those from the user
    /// agent.
    pub fn windows_desktop() -> Self {
        Self::builder()
            .os_type(OsType::Windows)
            .user_agent(preset_user_agent("Windows NT 10.0; Win64; x64", false))
            .platform("Win32")
            .screen(1920, 1080)
            .hardware_concurrency(8)
            .device_memory(8.0)
            .locale("en-US")
            .languages("en-US,en;q=0.9")
            .place("America/New_York")
            .webgl(
                "Google Inc. (NVIDIA)",
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)",
            )
            .flags(StealthFlags::balanced())
            .build()
    }

    /// Quick preset for a Windows desktop Microsoft Edge profile.
    ///
    /// The Windows Chrome preset with Edge's `Edg/` token in the user agent. The
    /// Client Hints name `Microsoft Edge` instead of `Google Chrome`: the
    /// evasions derive them from the user agent, as they do for the Chrome
    /// presets. [`fingerprint!(edge)`](crate::fingerprint) is this preset.
    pub fn edge_desktop() -> Self {
        Self::builder()
            .os_type(OsType::Windows)
            .user_agent(super::randomize::edge_user_agent(
                "Windows NT 10.0; Win64; x64",
                super::randomize::PRESET_CHROMIUM_MAJOR,
            ))
            .platform("Win32")
            .screen(1920, 1080)
            .hardware_concurrency(8)
            .device_memory(8.0)
            .locale("en-US")
            .languages("en-US,en;q=0.9")
            .place("America/New_York")
            .webgl(
                "Google Inc. (NVIDIA)",
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)",
            )
            .flags(StealthFlags::balanced())
            .build()
    }

    /// Quick preset for a macOS desktop Chrome profile.
    pub fn macos_desktop() -> Self {
        let (vendor, renderer) = super::randomize::macos_webgl("Apple M3");
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
            .place("America/Los_Angeles")
            .webgl(vendor, renderer)
            .flags(StealthFlags::balanced())
            .build()
    }

    /// Quick preset for a Linux desktop Chrome profile.
    ///
    /// Its WebGL strings are worded the way Chrome words them on Linux, as a
    /// generated Linux identity words them: ANGLE over desktop OpenGL.
    pub fn linux_desktop() -> Self {
        let (vendor, renderer) = super::randomize::linux_preset_webgl();
        Self::builder()
            .os_type(OsType::Linux)
            .user_agent(preset_user_agent("X11; Linux x86_64", false))
            .platform("Linux x86_64")
            .screen(1920, 1080)
            .hardware_concurrency(8)
            .device_memory(8.0)
            .locale("en-US")
            .languages("en-US,en;q=0.9")
            .place("America/Chicago")
            .webgl(vendor, renderer)
            .flags(StealthFlags::balanced())
            .build()
    }

    /// Quick preset for an Android mobile Chrome profile.
    pub fn android_mobile() -> Self {
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
            .place("America/New_York")
            .webgl("Qualcomm", "Adreno (TM) 740")
            .flags(StealthFlags::balanced())
            .build()
    }

    /// Quick preset for an iPhone / Mobile Safari profile.
    ///
    /// Safari 26.0 on iOS 26, whose user agent names the frozen `iPhone OS 18_6`
    /// (WebKit bug 298717); see the Safari table in [`super::randomize`].
    pub fn ios_mobile_safari() -> Self {
        Self::builder()
            .browser_type(BrowserType::MobileSafari)
            .os_type(OsType::Ios)
            .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1")
            .platform("iPhone")
            .screen(390, 844)
            .pixel_ratio(3.0)
            .hardware_concurrency(4)
            .device_memory(8.0)
            .max_touch_points(5)
            .locale("en-US")
            .languages("en-US,en;q=0.9")
            .place("America/New_York")
            .webgl("Apple Inc.", "Apple GPU")
            .vendor("Apple Computer, Inc.")
            .client_hints(ClientHints {
                brands: vec![BrandVersion::new("Safari", "26")],
                full_version_list: vec![BrandVersion::new("Safari", "26.0")],
                platform: "iPhone".to_owned(),
                platform_version: "26.0".to_owned(),
                architecture: "arm".to_owned(),
                bitness: "64".to_owned(),
                model: "iPhone".to_owned(),
                ua_full_version: "26.0".to_owned(),
                mobile: true,
                ..ClientHints::default()
            })
            .flags(StealthFlags::balanced())
            .build()
    }
}

/// The user agent of a Chrome preset: Chrome's frozen format on `platform`,
/// claiming [`PRESET_CHROMIUM_MAJOR`](super::randomize::PRESET_CHROMIUM_MAJOR).
fn preset_user_agent(platform: &str, mobile: bool) -> String {
    super::randomize::chromium_user_agent(platform, super::randomize::PRESET_CHROMIUM_MAJOR, mobile)
}

/// A change to the masking flags, applied last by [`FingerprintBuilder::build`].
#[derive(Clone)]
struct FlagsChange(std::sync::Arc<dyn Fn(&mut StealthFlags) + Send + Sync>);

impl std::fmt::Debug for FlagsChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlagsChange")
    }
}

/// Fluent builder for [`Fingerprint`].
///
/// A value setter only writes its value, and giving a value is the intent to
/// use it: [`build`](Self::build) turns the dimension the value belongs to on
/// (see [`Dimension`]), whatever flags preset was given and in whatever order,
/// so nobody has to touch [`Fingerprint::flags`] afterwards. The one way to turn
/// a dimension off again is [`with_flags`](Self::with_flags), which runs last.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::stealth::fingerprint::{
///     Dimension, Fingerprint, MaskingMode, StealthFlags,
/// };
///
/// let fp = Fingerprint::builder()
///     .timezone("Europe/Berlin")
///     .hardware_concurrency(8)
///     .build();
/// assert!(fp.is_masked(Dimension::Timezone) && fp.is_masked(Dimension::Navigator));
///
/// // Switch one back off with an explicit override.
/// let fp = Fingerprint::builder()
///     .timezone("Europe/Berlin")
///     .with_flags(|f| f.timezone_masking = MaskingMode::Natural)
///     .build();
/// assert!(!fp.is_masked(Dimension::Timezone));
/// ```
#[derive(Clone, Debug, Default)]
pub struct FingerprintBuilder {
    inner: Fingerprint,
    /// The flags preset given with [`flags`](Self::flags).
    base: Option<StealthFlags>,
    /// The overrides given with [`with_flags`](Self::with_flags), in order.
    changes: Vec<FlagsChange>,
    /// The dimensions a value was given for (a set of [`Dimension::bit`]s).
    touched: u32,
    /// Whether [`os_type`](Self::os_type) was called.
    os_type_set: bool,
    native_spoofing: Option<bool>,
}

impl FingerprintBuilder {
    /// Notes that a value for `dimension` was given.
    fn touch(mut self, dimension: Dimension) -> Self {
        self.touched |= dimension.bit();
        self
    }

    /// Sets the time zone and, when the table of places knows the zone, the
    /// position of its main city, so the two agree. The presets use it.
    fn place(self, timezone: &str) -> Self {
        let city = super::randomize::locale_of_timezone(timezone);
        let builder = self.timezone(timezone);
        match city {
            Some(city) => builder.geolocation(city.latitude, city.longitude),
            None => builder,
        }
    }

    /// Sets the browser persona.
    pub fn browser_type(mut self, v: BrowserType) -> Self {
        self.inner.browser_type = v;
        self
    }

    /// Sets the operating-system persona. When it is not set, [`build`](Self::build)
    /// takes it from a recognised user agent.
    pub fn os_type(mut self, v: OsType) -> Self {
        self.inner.os_type = v;
        self.os_type_set = true;
        self
    }

    /// Sets the Chromium major version the identity claims.
    pub fn core_version(mut self, v: u32) -> Self {
        self.inner.core_version = Some(v);
        self
    }

    /// Sets the user agent. Turns on [`Dimension::Navigator`].
    pub fn user_agent(mut self, v: impl Into<String>) -> Self {
        self.inner.user_agent = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.platform`. Turns on [`Dimension::Navigator`].
    pub fn platform(mut self, v: impl Into<String>) -> Self {
        self.inner.platform = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.hardwareConcurrency`. Turns on [`Dimension::Navigator`].
    pub fn hardware_concurrency(mut self, v: u32) -> Self {
        self.inner.hardware_concurrency = Some(v);
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.deviceMemory`. Turns on [`Dimension::Navigator`].
    pub fn device_memory(mut self, v: f64) -> Self {
        self.inner.device_memory = Some(v);
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.maxTouchPoints`. Turns on [`Dimension::Navigator`].
    pub fn max_touch_points(mut self, v: u32) -> Self {
        self.inner.max_touch_points = Some(v);
        self.touch(Dimension::Navigator)
    }

    /// Sets the locale (`navigator.language`). Turns on [`Dimension::Localization`].
    /// Without [`languages`](Self::languages), the language list is derived from it.
    pub fn locale(mut self, v: impl Into<String>) -> Self {
        self.inner.locale = Some(v.into());
        self.touch(Dimension::Localization)
    }

    /// Sets `navigator.languages` as a comma-separated list. Turns on
    /// [`Dimension::Localization`].
    pub fn languages(mut self, v: impl Into<String>) -> Self {
        self.inner.languages = Some(v.into());
        self.touch(Dimension::Localization)
    }

    /// Sets the `Accept-Language` header. Turns on [`Dimension::Localization`].
    pub fn accept_languages(mut self, v: impl Into<String>) -> Self {
        self.inner.accept_languages = Some(v.into());
        self.touch(Dimension::Localization)
    }

    /// Sets the IANA time zone. Turns on [`Dimension::Timezone`].
    pub fn timezone(mut self, v: impl Into<String>) -> Self {
        self.inner.timezone = Some(v.into());
        self.touch(Dimension::Timezone)
    }

    /// Sets the screen size. Turns on [`Dimension::Screen`].
    pub fn screen(mut self, width: u32, height: u32) -> Self {
        self.inner.screen_width = Some(width);
        self.inner.screen_height = Some(height);
        self.touch(Dimension::Screen)
    }

    /// Sets the available screen size (`screen.availWidth` and `availHeight`).
    /// Turns on [`Dimension::Screen`].
    pub fn avail_screen(mut self, width: u32, height: u32) -> Self {
        self.inner.avail_width = Some(width);
        self.inner.avail_height = Some(height);
        self.touch(Dimension::Screen)
    }

    /// Sets the device pixel ratio. Turns on [`Dimension::Screen`].
    pub fn pixel_ratio(mut self, v: f64) -> Self {
        self.inner.pixel_ratio = Some(v);
        self.touch(Dimension::Screen)
    }

    /// Sets the colour depth. Turns on [`Dimension::Screen`].
    pub fn color_depth(mut self, v: u32) -> Self {
        self.inner.color_depth = Some(v);
        self.touch(Dimension::Screen)
    }

    /// Sets the WebGL vendor and renderer. Turns on [`Dimension::Graphics`].
    pub fn webgl(mut self, vendor: impl Into<String>, renderer: impl Into<String>) -> Self {
        self.inner.webgl_vendor = Some(vendor.into());
        self.inner.webgl_renderer = Some(renderer.into());
        self.touch(Dimension::Graphics)
    }

    /// Sets the PCI vendor and renderer ids of the GPU. They are kept with the
    /// fingerprint for tools that read them and are never shown to a page, so
    /// they turn nothing on: [`webgl`](Self::webgl) does.
    pub fn webgl_ids(
        mut self,
        vendor_id: impl Into<String>,
        renderer_id: impl Into<String>,
    ) -> Self {
        self.inner.webgl_vendor_id = Some(vendor_id.into());
        self.inner.webgl_renderer_id = Some(renderer_id.into());
        self
    }

    /// Sets `navigator.oscpu` (Firefox persona). Turns on [`Dimension::Navigator`].
    pub fn os_cpu(mut self, v: impl Into<String>) -> Self {
        self.inner.oscpu = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.productSub`. Turns on [`Dimension::Navigator`].
    pub fn product_sub(mut self, v: impl Into<String>) -> Self {
        self.inner.product_sub = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.buildID` (Firefox persona). Turns on [`Dimension::Navigator`].
    pub fn build_id(mut self, v: impl Into<String>) -> Self {
        self.inner.build_id = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets `navigator.appVersion`. Turns on [`Dimension::Navigator`].
    pub fn app_version(mut self, v: impl Into<String>) -> Self {
        self.inner.app_version = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets how many audio inputs, audio outputs and video inputs
    /// `enumerateDevices()` lists. Turns on [`Dimension::MediaDevices`].
    pub fn media_device_counts(mut self, audio_in: u32, audio_out: u32, video_in: u32) -> Self {
        self.inner.audio_inputs = Some(audio_in);
        self.inner.audio_outputs = Some(audio_out);
        self.inner.video_inputs = Some(video_in);
        self.touch(Dimension::MediaDevices)
    }

    /// The same as [`media_device_counts`](Self::media_device_counts).
    #[deprecated(note = "use media_device_counts; this sets counts, not the devices")]
    pub fn media_devices(self, audio_in: u32, audio_out: u32, video_in: u32) -> Self {
        self.media_device_counts(audio_in, audio_out, video_in)
    }

    /// Sets the labelled media devices. Turns on [`Dimension::MediaDevices`].
    pub fn media_device_specs(mut self, v: Vec<MediaDeviceSpec>) -> Self {
        self.inner.media_devices = v;
        self.touch(Dimension::MediaDevices)
    }

    /// Sets the public and local IP addresses WebRTC reports. Turns on
    /// [`Dimension::WebRtc`].
    pub fn webrtc_ips(mut self, public_ip: impl Into<String>, local_ip: impl Into<String>) -> Self {
        self.inner.webrtc_public_ip = Some(public_ip.into());
        self.inner.webrtc_local_ip = Some(local_ip.into());
        self.touch(Dimension::WebRtc)
    }

    /// Sets the ports the browser may not reach on the loopback names. Turns on
    /// [`Dimension::Ports`].
    pub fn blocked_ports(mut self, v: Vec<u16>) -> Self {
        self.inner.ports = v;
        self.touch(Dimension::Ports)
    }

    /// Sets the position `navigator.geolocation` reports. Turns on
    /// [`Dimension::Geolocation`] and lets the request through without a prompt
    /// (see [`Fingerprint::geolocation_popup`]).
    pub fn geolocation(mut self, latitude: f64, longitude: f64) -> Self {
        self.inner.latitude = Some(latitude);
        self.inner.longitude = Some(longitude);
        self.touch(Dimension::Geolocation)
    }

    /// Sets the altitude of the reported position. Turns on [`Dimension::Geolocation`].
    pub fn altitude(mut self, v: f64) -> Self {
        self.inner.altitude = Some(v);
        self.touch(Dimension::Geolocation)
    }

    /// Sets the accuracy of the reported position, in metres. Turns on
    /// [`Dimension::Geolocation`].
    pub fn accuracy(mut self, v: f64) -> Self {
        self.inner.accuracy = Some(v);
        self.touch(Dimension::Geolocation)
    }

    /// Sets the installed fonts. Turns on [`Dimension::Fonts`] (when the list
    /// is not empty).
    pub fn fonts(mut self, v: Vec<String>) -> Self {
        let any = !v.is_empty();
        self.inner.fonts = v;
        if any {
            self = self.touch(Dimension::Fonts);
        }
        self
    }

    /// Sets the proxy the browser is launched with. Turns on [`Dimension::Proxy`].
    pub fn proxy(mut self, v: ProxyConfig) -> Self {
        self.inner.proxy = Some(v);
        self.touch(Dimension::Proxy)
    }

    /// Sets the WebRTC IP-handling policy. Turns on [`Dimension::WebRtc`].
    pub fn webrtc_policy(mut self, v: WebRtcPolicy) -> Self {
        self.inner.webrtc_policy = Some(v);
        self.touch(Dimension::WebRtc)
    }

    /// Keeps `localStorage` between sessions of the profile.
    pub fn local_storage(mut self, v: bool) -> Self {
        self.inner.local_storage = v;
        self
    }

    /// Keeps service workers between sessions of the profile.
    pub fn save_service_worker(mut self, v: bool) -> Self {
        self.inner.save_service_worker = v;
        self
    }

    /// Sets the URLs the profile opens at start.
    pub fn custom_start_urls(mut self, v: Vec<String>) -> Self {
        self.inner.custom_start_urls = v;
        self
    }

    /// Sets extra browser command-line parameters.
    pub fn cmd_params(mut self, v: HashMap<String, String>) -> Self {
        self.inner.cmd_params = v;
        self
    }

    /// Sets the base masking flags, such as [`StealthFlags::balanced`].
    ///
    /// It is a base, not the last word: the order of this call against the
    /// value setters does not matter, a dimension a value was given for stays
    /// on, and [`with_flags`](Self::with_flags) changes run after it.
    pub fn flags(mut self, base: StealthFlags) -> Self {
        self.base = Some(base);
        self
    }

    /// Changes the masking flags after the base and after the value setters
    /// have had their say. This is the explicit way to turn a dimension off
    /// again; several calls run in order.
    pub fn with_flags(
        mut self,
        change: impl Fn(&mut StealthFlags) + Send + Sync + 'static,
    ) -> Self {
        self.changes.push(FlagsChange(std::sync::Arc::new(change)));
        self
    }

    /// Sets the canvas noise seed. Turns on [`Dimension::Canvas`].
    pub fn canvas_seed(mut self, v: u64) -> Self {
        self.inner.seed = Some(v);
        self.touch(Dimension::Canvas)
    }

    /// Sets the audio noise seed. Turns on [`Dimension::Audio`].
    pub fn audio_seed(mut self, v: u64) -> Self {
        self.inner.audio_seed = Some(v);
        self.touch(Dimension::Audio)
    }

    /// Sets the deterministic noise seed for both canvas and audio: the canvas
    /// seed is `v` and the audio seed `v ^ 0x5DEE_CE66`. Turns on
    /// [`Dimension::Canvas`] and [`Dimension::Audio`].
    pub fn seed(self, v: u64) -> Self {
        self.canvas_seed(v).audio_seed(v ^ AUDIO_SEED_MASK)
    }

    /// Sets `navigator.vendor`. Turns on [`Dimension::Navigator`].
    pub fn vendor(mut self, v: impl Into<String>) -> Self {
        self.inner.vendor = Some(v.into());
        self.touch(Dimension::Navigator)
    }

    /// Sets the Battery Status API profile. Turns on [`Dimension::Battery`].
    pub fn battery(mut self, v: BatteryProfile) -> Self {
        self.inner.battery = Some(v);
        self.touch(Dimension::Battery)
    }

    /// Sets the Network Information API profile. Turns on [`Dimension::Connection`].
    pub fn connection(mut self, v: ConnectionProfile) -> Self {
        self.inner.connection = Some(v);
        self.touch(Dimension::Connection)
    }

    /// Sets the synthesized speech voices. Turns on [`Dimension::Speech`] (when
    /// the list is not empty).
    pub fn speech_voices(mut self, v: Vec<SpeechVoice>) -> Self {
        let any = !v.is_empty();
        self.inner.speech_voices = v;
        if any {
            self = self.touch(Dimension::Speech);
        }
        self
    }

    /// Sets the User-Agent Client Hints. Turns on [`Dimension::ClientHints`].
    pub fn client_hints(mut self, v: ClientHints) -> Self {
        self.inner.client_hints = Some(v);
        self.touch(Dimension::ClientHints)
    }

    /// Sets the humanized input timing configuration.
    pub fn humanize(mut self, v: HumanizeConfig) -> Self {
        self.inner.humanize = v;
        self
    }

    /// Sets a custom tracker host block list. Turns on [`Dimension::Trackers`]
    /// (when the list is not empty).
    pub fn blocked_trackers(mut self, v: Vec<String>) -> Self {
        let any = !v.is_empty();
        self.inner.blocked_trackers = v;
        if any {
            self = self.touch(Dimension::Trackers);
        }
        self
    }

    /// Enables Rust/CDP-level spoofing for dimensions that the browser can
    /// override natively. When enabled, page JavaScript cannot detect the
    /// spoof through `toString` or property-descriptor inspection because
    /// there is no JS-visible override.
    pub fn native_spoofing(mut self, v: bool) -> Self {
        self.native_spoofing = Some(v);
        self
    }

    /// Builds the fingerprint.
    ///
    /// The flags are the base given with [`flags`](Self::flags) (or the
    /// default), with every dimension a value was given for switched on, then
    /// the [`with_flags`](Self::with_flags) changes. Two values are derived when
    /// they were not given: the language list from the locale, and the
    /// operating-system persona and `navigator.platform` from a recognised user
    /// agent.
    #[must_use]
    pub fn build(self) -> Fingerprint {
        let mut fp = self.inner;

        if fp.languages.is_none() {
            fp.languages = fp.locale.as_deref().map(languages_for_locale);
        }
        if let Some(os) = fp.user_agent.as_deref().and_then(os_of_user_agent) {
            if !self.os_type_set {
                fp.os_type = os;
            }
            if fp.platform.is_none() {
                fp.platform = Some(fp.os_type.platform().to_owned());
            }
        }

        let mut flags = self.base.unwrap_or_default();
        for dimension in ALL_VALUE_DIMENSIONS {
            if self.touched & dimension.bit() != 0 {
                turn_on(&mut flags, dimension);
            }
        }
        if let Some(native) = self.native_spoofing {
            flags.native_spoofing = native;
        }
        for change in &self.changes {
            (change.0)(&mut flags);
        }
        // A tracker list is itself a request to block them, so a change that
        // turned the block off again also drops the list.
        if self.touched & Dimension::Trackers.bit() != 0 && !flags.block_trackers {
            fp.blocked_trackers.clear();
        }
        fp.flags = flags;
        fp
    }
}

/// The audio seed is the canvas seed with this mask applied, so one `seed` gives
/// both noises and they differ from each other.
pub(crate) const AUDIO_SEED_MASK: u64 = 0x5DEE_CE66;

/// The dimensions a builder value can switch on.
const ALL_VALUE_DIMENSIONS: [Dimension; 18] = [
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
];

/// Switches `dimension` on in `flags` if the flags had it explicitly off. A
/// mode that is already on, or on `Auto`, is left as it is.
fn turn_on(flags: &mut StealthFlags, dimension: Dimension) {
    fn lift(mode: &mut MaskingMode) {
        if mode.is_off() {
            *mode = MaskingMode::Auto;
        }
    }
    match dimension {
        Dimension::Navigator => lift(&mut flags.navigator_masking),
        Dimension::Localization => lift(&mut flags.localization_masking),
        Dimension::Timezone => lift(&mut flags.timezone_masking),
        Dimension::Screen => lift(&mut flags.screen_masking),
        Dimension::Graphics => lift(&mut flags.graphics_masking),
        Dimension::Fonts => lift(&mut flags.fonts_masking),
        Dimension::MediaDevices => lift(&mut flags.media_devices_masking),
        Dimension::Geolocation => {
            lift(&mut flags.geolocation_masking);
            if flags.geolocation_popup == PopupMode::Prompt {
                flags.geolocation_popup = PopupMode::Auto;
            }
        }
        Dimension::WebRtc => lift(&mut flags.webrtc_masking),
        Dimension::Canvas => {
            if matches!(
                flags.canvas_noise,
                CanvasNoiseMode::Natural | CanvasNoiseMode::Disabled
            ) {
                flags.canvas_noise = CanvasNoiseMode::Auto;
            }
        }
        Dimension::Audio => lift(&mut flags.audio_masking),
        Dimension::Battery => lift(&mut flags.battery_masking),
        Dimension::Connection => lift(&mut flags.connection_masking),
        Dimension::Speech => lift(&mut flags.speech_masking),
        Dimension::ClientHints => lift(&mut flags.client_hints_masking),
        Dimension::Proxy => {
            if matches!(
                flags.proxy_masking,
                ProxyMaskingMode::Disabled | ProxyMaskingMode::Direct
            ) {
                flags.proxy_masking = ProxyMaskingMode::Auto;
            }
        }
        Dimension::Ports => {
            if matches!(flags.ports_masking, NoiseMode::Natural | NoiseMode::Off) {
                flags.ports_masking = NoiseMode::Mask;
            }
        }
        Dimension::Trackers => flags.block_trackers = true,
        _ => {}
    }
}

/// The `navigator.languages` list for `locale`, as a comma-separated string:
/// the table's list when the locale is in [`LOCALES`](super::randomize::LOCALES),
/// otherwise the locale and its bare language.
pub(crate) fn languages_for_locale(locale: &str) -> String {
    if let Some(known) = super::randomize::LOCALES
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

    #[test]
    fn builder_builds_profile() {
        let fp = Fingerprint::builder()
            .os_type(OsType::Windows)
            .user_agent("UA")
            .screen(1920, 1080)
            .hardware_concurrency(8)
            .build();

        assert_eq!(fp.os_type, OsType::Windows);
        assert_eq!(fp.user_agent.as_deref(), Some("UA"));
        assert_eq!(fp.screen_width, Some(1920));
        assert_eq!(fp.hardware_concurrency, Some(8));
    }

    #[test]
    fn presets_are_valid() {
        assert_eq!(Fingerprint::windows_desktop().os_type, OsType::Windows);
        assert_eq!(Fingerprint::macos_desktop().os_type, OsType::Macos);
        assert_eq!(Fingerprint::linux_desktop().os_type, OsType::Linux);
        assert_eq!(Fingerprint::android_mobile().os_type, OsType::Android);
        let ios = Fingerprint::ios_mobile_safari();
        assert_eq!(ios.os_type, OsType::Ios);
        assert_eq!(ios.browser_type, BrowserType::MobileSafari);
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

    #[test]
    fn presets_are_coherent() {
        for fp in [
            Fingerprint::windows_desktop(),
            Fingerprint::macos_desktop(),
            Fingerprint::linux_desktop(),
            Fingerprint::android_mobile(),
            Fingerprint::ios_mobile_safari(),
        ] {
            let report = fp.validate();
            assert!(report.is_coherent(), "preset errors: {:?}", report.errors);
        }
    }

    #[test]
    fn the_chrome_presets_claim_one_current_major_in_the_frozen_format() {
        let version = format!(
            "Chrome/{}.0.0.0",
            crate::stealth::randomize::PRESET_CHROMIUM_MAJOR
        );
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
            let ua = fp.user_agent.as_deref().unwrap_or_default();
            assert!(ua.contains(&version), "{ua}");
            assert!(ua.contains(platform), "{ua}");
        }
        let iphone = Fingerprint::ios_mobile_safari();
        assert_eq!(
            iphone.user_agent.as_deref(),
            Some(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 \
                 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1"
            )
        );
    }

    #[test]
    fn mismatched_platform_and_ua_is_incoherent() {
        // macOS user-agent with a Windows platform string is a classic tell.
        let fp = Fingerprint::builder()
            .os_type(OsType::Macos)
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
            )
            .platform("Win32")
            .build();
        let report = fp.validate();
        assert!(!report.is_coherent());
        assert!(report.errors.iter().any(|e| e.contains("platform")));
    }

    #[test]
    fn seed_is_deterministic_from_user_agent() {
        let fp = Fingerprint::windows_desktop();
        assert_eq!(fp.seed_value(), fp.seed_value());
        let explicit = Fingerprint::builder().seed(1234).build();
        assert_eq!(explicit.seed_value(), 1234);
    }

    #[test]
    fn tracker_hosts_fall_back_to_defaults() {
        let fp = Fingerprint::windows_desktop();
        assert!(!fp.tracker_hosts().is_empty());
        let custom = Fingerprint::builder()
            .blocked_trackers(vec!["example-metrics.test".to_owned()])
            .build();
        assert_eq!(custom.tracker_hosts(), vec!["example-metrics.test"]);
    }

    #[test]
    fn partial_fingerprint_deserializes() {
        let raw = r#"{
            "os_type": "Windows",
            "hardware_concurrency": 8,
            "webgl_vendor": "Google Inc. (NVIDIA)",
            "fonts": ["Arial", "Helvetica"],
            "seed": 42,
            "flags": { "navigator_masking": "Custom", "canvas_noise": "Persistent" }
        }"#;
        let fp: Fingerprint = serde_json::from_str(raw).unwrap();
        assert_eq!(fp.hardware_concurrency, Some(8));
        assert_eq!(fp.os_type, OsType::Windows);
        assert_eq!(fp.flags.navigator_masking, MaskingMode::Custom);
        assert_eq!(fp.flags.canvas_noise, CanvasNoiseMode::Persistent);
        assert_eq!(fp.fonts.len(), 2);
        assert!(fp.user_agent.is_none());
        // A flag the file does not name is `Auto`: on only for a value it holds.
        assert_eq!(fp.flags.webrtc_masking, MaskingMode::Auto);
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

    #[test]
    fn randomized_with_major_claims_that_major_and_keeps_the_rest_of_the_seed() {
        let free = Fingerprint::randomized(OsType::Macos, 12);
        let pinned = Fingerprint::randomized_with_major(OsType::Macos, 12, 155);
        assert_eq!(pinned.core_version, Some(155));
        assert!(pinned
            .user_agent
            .as_deref()
            .unwrap_or_default()
            .contains("Chrome/155.0.0.0"));
        assert_eq!(pinned.timezone, free.timezone);
        assert_eq!(pinned.webgl_renderer, free.webgl_renderer);
        assert_eq!(pinned.seed, free.seed);
        assert!(pinned.validate().is_coherent());
    }

    // ------------------------------------------------------------------
    // The value is the switch
    // ------------------------------------------------------------------

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

    /// Flags that switch every dimension off explicitly, as a preset or a saved
    /// file can.
    fn natural_flags() -> StealthFlags {
        let off = MaskingMode::Natural;
        StealthFlags {
            webrtc_masking: off,
            audio_masking: off,
            navigator_masking: off,
            localization_masking: off,
            timezone_masking: off,
            graphics_masking: off,
            fonts_masking: off,
            media_devices_masking: off,
            screen_masking: off,
            geolocation_masking: off,
            battery_masking: off,
            connection_masking: off,
            speech_masking: off,
            bluetooth_masking: off,
            client_hints_masking: off,
            native_tostring_masking: off,
            chrome_runtime_masking: off,
            headless_masking: off,
            geolocation_popup: PopupMode::Prompt,
            ports_masking: NoiseMode::Off,
            proxy_masking: ProxyMaskingMode::Disabled,
            canvas_noise: CanvasNoiseMode::Natural,
            block_trackers: false,
            ..StealthFlags::default()
        }
    }

    type Setter = fn(FingerprintBuilder) -> FingerprintBuilder;

    /// Each value setter with the dimension it belongs to.
    fn value_setters() -> Vec<(&'static str, Setter, Dimension)> {
        vec![
            (
                "user_agent",
                |b| b.user_agent("Mozilla/5.0"),
                Dimension::Navigator,
            ),
            ("platform", |b| b.platform("Win32"), Dimension::Navigator),
            (
                "hardware_concurrency",
                |b| b.hardware_concurrency(8),
                Dimension::Navigator,
            ),
            (
                "device_memory",
                |b| b.device_memory(8.0),
                Dimension::Navigator,
            ),
            (
                "max_touch_points",
                |b| b.max_touch_points(5),
                Dimension::Navigator,
            ),
            ("vendor", |b| b.vendor("Google Inc."), Dimension::Navigator),
            ("os_cpu", |b| b.os_cpu("Linux x86_64"), Dimension::Navigator),
            (
                "product_sub",
                |b| b.product_sub("20030107"),
                Dimension::Navigator,
            ),
            (
                "build_id",
                |b| b.build_id("20181001000000"),
                Dimension::Navigator,
            ),
            (
                "app_version",
                |b| b.app_version("5.0"),
                Dimension::Navigator,
            ),
            ("locale", |b| b.locale("de-DE"), Dimension::Localization),
            (
                "languages",
                |b| b.languages("de-DE,de"),
                Dimension::Localization,
            ),
            (
                "accept_languages",
                |b| b.accept_languages("de-DE"),
                Dimension::Localization,
            ),
            (
                "timezone",
                |b| b.timezone("Europe/Berlin"),
                Dimension::Timezone,
            ),
            ("screen", |b| b.screen(1440, 900), Dimension::Screen),
            (
                "avail_screen",
                |b| b.avail_screen(1440, 860),
                Dimension::Screen,
            ),
            ("pixel_ratio", |b| b.pixel_ratio(2.0), Dimension::Screen),
            ("color_depth", |b| b.color_depth(30), Dimension::Screen),
            (
                "webgl",
                |b| b.webgl("Vendor", "Renderer"),
                Dimension::Graphics,
            ),
            (
                "fonts",
                |b| b.fonts(vec!["Arial".to_owned()]),
                Dimension::Fonts,
            ),
            (
                "media_device_counts",
                |b| b.media_device_counts(1, 1, 1),
                Dimension::MediaDevices,
            ),
            (
                "media_device_specs",
                |b| {
                    b.media_device_specs(vec![MediaDeviceSpec {
                        kind: "audioinput".to_owned(),
                        device_id: "default".to_owned(),
                        group_id: "g".to_owned(),
                        label: "Mic".to_owned(),
                    }])
                },
                Dimension::MediaDevices,
            ),
            (
                "geolocation",
                |b| b.geolocation(52.5, 13.4),
                Dimension::Geolocation,
            ),
            (
                "webrtc_ips",
                |b| b.webrtc_ips("203.0.113.9", "10.0.0.2"),
                Dimension::WebRtc,
            ),
            (
                "webrtc_policy",
                |b| b.webrtc_policy(WebRtcPolicy::PublicInterfaceOnly),
                Dimension::WebRtc,
            ),
            ("canvas_seed", |b| b.canvas_seed(5), Dimension::Canvas),
            ("seed (canvas)", |b| b.seed(5), Dimension::Canvas),
            ("audio_seed", |b| b.audio_seed(5), Dimension::Audio),
            ("seed (audio)", |b| b.seed(5), Dimension::Audio),
            (
                "battery",
                |b| b.battery(BatteryProfile::default()),
                Dimension::Battery,
            ),
            (
                "connection",
                |b| b.connection(ConnectionProfile::default()),
                Dimension::Connection,
            ),
            (
                "speech_voices",
                |b| b.speech_voices(SpeechVoice::default_set()),
                Dimension::Speech,
            ),
            (
                "client_hints",
                |b| b.client_hints(ClientHints::default()),
                Dimension::ClientHints,
            ),
            (
                "proxy",
                |b| {
                    b.proxy(ProxyConfig {
                        r#type: "http".to_owned(),
                        host: "proxy.test".to_owned(),
                        port: 8080,
                        username: None,
                        password: None,
                        save_traffic: false,
                    })
                },
                Dimension::Proxy,
            ),
            (
                "blocked_ports",
                |b| b.blocked_ports(vec![9222]),
                Dimension::Ports,
            ),
            (
                "blocked_trackers",
                |b| b.blocked_trackers(vec!["tracker.test".to_owned()]),
                Dimension::Trackers,
            ),
        ]
    }

    #[test]
    fn a_fingerprint_without_values_masks_nothing_but_native_to_string() {
        let fp = Fingerprint::default();
        for dimension in EVERY_DIMENSION {
            assert_eq!(
                fp.is_masked(dimension),
                dimension == Dimension::NativeToString,
                "{dimension:?}"
            );
        }
        assert!(!Fingerprint::builder().build().is_masked(Dimension::WebRtc));
    }

    #[test]
    fn the_mode_decides_what_a_value_means() {
        let mut fp = Fingerprint::builder().timezone("Europe/Berlin").build();
        assert!(fp.is_masked(Dimension::Timezone), "a value turns it on");
        assert!(!fp.fills_defaults(Dimension::Timezone));

        fp.flags.timezone_masking = MaskingMode::Natural;
        assert!(
            !fp.is_masked(Dimension::Timezone),
            "Natural is off whatever the fingerprint holds"
        );

        fp.timezone = None;
        fp.flags.timezone_masking = MaskingMode::Auto;
        assert!(!fp.is_masked(Dimension::Timezone), "Auto needs a value");

        fp.flags.timezone_masking = MaskingMode::Mask;
        assert!(fp.is_masked(Dimension::Timezone) && fp.fills_defaults(Dimension::Timezone));

        // The older spellings keep their meaning.
        fp.flags.timezone_masking = MaskingMode::Custom;
        assert!(fp.is_masked(Dimension::Timezone));
        fp.flags.timezone_masking = MaskingMode::Disabled;
        assert!(!fp.is_masked(Dimension::Timezone));
    }

    #[test]
    fn every_value_setter_turns_its_dimension_on_whatever_the_preset_and_its_order() {
        for (name, set, dimension) in value_setters() {
            let runs: [(Option<StealthFlags>, bool); 5] = [
                (None, true),
                (Some(StealthFlags::balanced()), true),
                (Some(StealthFlags::balanced()), false),
                (Some(natural_flags()), true),
                (Some(natural_flags()), false),
            ];
            for (preset, preset_first) in runs {
                let mut builder = Fingerprint::builder();
                if let (Some(preset), true) = (&preset, preset_first) {
                    builder = builder.flags(preset.clone());
                }
                builder = set(builder);
                if let (Some(preset), false) = (&preset, preset_first) {
                    builder = builder.flags(preset.clone());
                }
                assert!(
                    builder.build().is_masked(dimension),
                    "{name} left {dimension:?} off (preset {preset:?}, preset first: {preset_first})"
                );
            }
        }
    }

    #[test]
    fn with_flags_has_the_last_word_in_any_order() {
        for (name, set, dimension) in value_setters() {
            // The override is given before the value, and after it.
            let before = set(Fingerprint::builder().with_flags(|f| *f = natural_flags())).build();
            let after = set(Fingerprint::builder())
                .with_flags(|f| *f = natural_flags())
                .build();
            for (fp, order) in [(before, "before"), (after, "after")] {
                assert!(
                    !fp.is_masked(dimension),
                    "{name}: an override given {order} the value did not switch {dimension:?} off"
                );
            }
        }
    }

    #[test]
    fn several_overrides_run_in_order_after_the_values() {
        let fp = Fingerprint::builder()
            .timezone("Asia/Tokyo")
            .with_flags(|f| f.timezone_masking = MaskingMode::Natural)
            .locale("ja-JP")
            .with_flags(|f| f.timezone_masking = MaskingMode::Mask)
            .build();
        assert!(fp.is_masked(Dimension::Timezone) && fp.fills_defaults(Dimension::Timezone));
        assert!(fp.is_masked(Dimension::Localization));
    }

    #[test]
    fn the_order_of_the_preset_and_the_values_does_not_change_the_fingerprint() {
        let before = Fingerprint::builder()
            .flags(natural_flags())
            .timezone("Asia/Tokyo")
            .locale("ja-JP")
            .build();
        let after = Fingerprint::builder()
            .timezone("Asia/Tokyo")
            .locale("ja-JP")
            .flags(natural_flags())
            .build();
        assert_eq!(before, after);
    }

    #[test]
    fn a_tracker_list_is_a_request_to_block_and_an_override_can_still_refuse() {
        let list = vec!["tracker.test".to_owned()];
        let fp = Fingerprint::builder()
            .blocked_trackers(list.clone())
            .build();
        assert!(fp.is_masked(Dimension::Trackers) && fp.flags.block_trackers);
        assert_eq!(fp.tracker_hosts(), list);

        let fp = Fingerprint::builder()
            .blocked_trackers(list)
            .with_flags(|f| f.block_trackers = false)
            .build();
        assert!(!fp.is_masked(Dimension::Trackers));
        assert!(
            fp.blocked_trackers.is_empty(),
            "a list that is not used is not kept"
        );
    }

    #[test]
    fn the_locale_gives_the_language_list_and_the_user_agent_gives_the_os() {
        let fp = Fingerprint::builder().locale("de-DE").build();
        assert!(fp
            .languages
            .as_deref()
            .is_some_and(|l| l.starts_with("de-DE")));
        let unknown = Fingerprint::builder().locale("xx-YY").build();
        assert_eq!(unknown.languages.as_deref(), Some("xx-YY,xx"));
        let given = Fingerprint::builder()
            .locale("de-DE")
            .languages("de-DE")
            .build();
        assert_eq!(given.languages.as_deref(), Some("de-DE"));

        let mac = Fingerprint::builder()
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
            )
            .build();
        assert_eq!(mac.os_type, OsType::Macos);
        assert_eq!(mac.platform.as_deref(), Some("MacIntel"));
        let phone = Fingerprint::builder()
            .user_agent(
                "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 Chrome/155.0.0.0 Mobile",
            )
            .build();
        assert_eq!(phone.os_type, OsType::Android);

        // What the caller said is kept.
        let said = Fingerprint::builder()
            .os_type(OsType::Linux)
            .platform("Linux x86_64")
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build();
        assert_eq!(said.os_type, OsType::Linux);
        assert_eq!(said.platform.as_deref(), Some("Linux x86_64"));
        assert!(!said.validate().is_coherent(), "the mismatch is reported");
    }

    #[test]
    fn a_position_allows_the_geolocation_popup_unless_the_flags_say_otherwise() {
        let none = Fingerprint::builder().build();
        assert_eq!(none.geolocation_popup(), PopupMode::Prompt);

        let placed = Fingerprint::builder().geolocation(52.5, 13.4).build();
        assert_eq!(placed.geolocation_popup(), PopupMode::Allow);

        let kept = Fingerprint::builder()
            .geolocation(52.5, 13.4)
            .with_flags(|f| f.geolocation_popup = PopupMode::Block)
            .build();
        assert_eq!(kept.geolocation_popup(), PopupMode::Block);

        // A preset that says `Prompt` does not undo the request the position makes.
        let preset = Fingerprint::builder()
            .flags(natural_flags())
            .geolocation(52.5, 13.4)
            .build();
        assert_eq!(preset.geolocation_popup(), PopupMode::Allow);
    }

    #[test]
    fn one_seed_gives_both_noises_and_each_seed_gives_its_own() {
        let both = Fingerprint::builder().seed(7).build();
        assert_eq!(both.seed, Some(7));
        assert_eq!(both.audio_seed, Some(7 ^ AUDIO_SEED_MASK));
        assert!(both.is_masked(Dimension::Canvas) && both.is_masked(Dimension::Audio));

        let canvas = Fingerprint::builder().canvas_seed(7).build();
        assert!(canvas.is_masked(Dimension::Canvas) && !canvas.is_masked(Dimension::Audio));
        let audio = Fingerprint::builder().audio_seed(7).build();
        assert!(audio.is_masked(Dimension::Audio) && !audio.is_masked(Dimension::Canvas));
    }

    #[test]
    fn a_native_spoofing_choice_survives_a_preset_given_later() {
        let fp = Fingerprint::builder()
            .native_spoofing(true)
            .flags(StealthFlags::balanced())
            .build();
        assert!(fp.flags.native_spoofing);
    }

    #[test]
    fn the_available_screen_defaults_to_the_screen_less_the_taskbar() {
        let fp = Fingerprint::builder()
            .screen(1920, 1080)
            .avail_screen(1920, 1040)
            .build();
        assert_eq!((fp.avail_width, fp.avail_height), (Some(1920), Some(1040)));
    }

    // ------------------------------------------------------------------
    // Presets and validate()
    // ------------------------------------------------------------------

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
    fn every_preset_has_a_position_in_the_city_of_its_time_zone() {
        for (name, fp) in every_preset() {
            let zone = fp.timezone.as_deref().expect("a time zone");
            let city = crate::stealth::randomize::locale_of_timezone(zone)
                .unwrap_or_else(|| panic!("{name}: {zone} is not in the table"));
            assert_eq!(fp.latitude, Some(city.latitude), "{name}");
            assert_eq!(fp.longitude, Some(city.longitude), "{name}");
            assert!(fp.is_masked(Dimension::Geolocation), "{name}");
            assert_eq!(fp.geolocation_popup(), PopupMode::Allow, "{name}");
        }
    }

    #[test]
    fn every_preset_masks_the_dimensions_its_values_and_the_generic_ones_need() {
        for (name, fp) in every_preset() {
            for dimension in [
                Dimension::Navigator,
                Dimension::Localization,
                Dimension::Timezone,
                Dimension::Screen,
                Dimension::Graphics,
                Dimension::Geolocation,
                Dimension::WebRtc,
                Dimension::Canvas,
                Dimension::Battery,
                Dimension::Connection,
                Dimension::Speech,
                Dimension::Bluetooth,
                Dimension::ChromeRuntime,
                Dimension::Headless,
                Dimension::NativeToString,
            ] {
                assert!(fp.is_masked(dimension), "{name}: {dimension:?} is off");
            }
        }
    }

    #[test]
    fn the_edge_preset_is_edge_and_not_chrome() {
        let fp = Fingerprint::edge_desktop();
        let major = crate::stealth::randomize::PRESET_CHROMIUM_MAJOR;
        let ua = fp.user_agent.as_deref().unwrap_or_default();
        assert!(ua.ends_with(&format!("Edg/{major}.0.0.0")), "{ua}");
        assert!(ua.contains(&format!("Chrome/{major}.0.0.0")), "{ua}");
        assert_eq!(fp.os_type, OsType::Windows);
        assert_eq!(fp.platform.as_deref(), Some("Win32"));
        assert_eq!(
            Fingerprint::edge_desktop().webgl_renderer,
            Fingerprint::windows_desktop().webgl_renderer
        );
    }

    #[test]
    fn the_linux_preset_reports_a_linux_graphics_stack_and_the_mac_one_metal() {
        let linux = Fingerprint::linux_desktop();
        let renderer = linux.webgl_renderer.as_deref().unwrap_or_default();
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("D3D11"),
            "{renderer}"
        );
        assert!(renderer.contains("OpenGL"), "{renderer}");
        assert_eq!(
            linux.webgl_vendor.as_deref(),
            Some("Google Inc. (NVIDIA Corporation)")
        );

        let mac = Fingerprint::macos_desktop();
        let renderer = mac.webgl_renderer.as_deref().unwrap_or_default();
        assert!(renderer.contains("Metal"), "{renderer}");
    }

    /// A report for a persona with `os` and the WebGL strings given, and nothing else.
    fn graphics_report(os: OsType, vendor: &str, renderer: &str) -> CoherenceReport {
        Fingerprint::builder()
            .os_type(os)
            .webgl(vendor, renderer)
            .build()
            .validate()
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
        let fp = Fingerprint::builder()
            .browser_type(BrowserType::MobileSafari)
            .os_type(OsType::Ios)
            .user_agent(chrome_ua)
            .max_touch_points(5)
            .build();
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
        let firefox = Fingerprint::builder()
            .browser_type(BrowserType::Firefox)
            .os_type(OsType::Linux)
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:153.0) Gecko/20100101 Firefox/153.0")
            .build();
        assert!(firefox.validate().errors.is_empty());
    }

    #[test]
    fn a_locale_from_another_country_than_the_time_zone_is_a_warning() {
        let warns = |locale: &str, zone: &str| {
            Fingerprint::builder()
                .locale(locale)
                .timezone(zone)
                .geolocation(0.0, 0.0)
                .build()
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
    fn masking_graphics_without_a_renderer_warns_when_the_fill_is_not_the_os() {
        let masked = |os: OsType| {
            Fingerprint::builder()
                .os_type(os)
                .with_flags(|f| f.graphics_masking = MaskingMode::Mask)
                .build()
                .validate()
                .warnings
                .iter()
                .any(|w| w.contains("WebGL"))
        };
        // The generic fill is an Intel renderer worded for a Mac.
        assert!(masked(OsType::Windows));
        assert!(masked(OsType::Linux));
        assert!(masked(OsType::Android));
        assert!(!masked(OsType::Macos));
        assert!(!masked(OsType::Ios));

        // With the renderer given there is nothing to fill.
        let given = Fingerprint::builder()
            .os_type(OsType::Windows)
            .webgl(
                "Google Inc. (Intel)",
                "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)",
            )
            .with_flags(|f| f.graphics_masking = MaskingMode::Mask)
            .build();
        assert!(!given
            .validate()
            .warnings
            .iter()
            .any(|w| w.contains("WebGL")));
    }

    #[test]
    fn saved_fingerprints_in_the_old_shape_still_load() {
        // `webrtc_policy` was a bare value, the modes `Custom` and `Disabled`
        // and the proxy mode `Socks5` existed, and no `Auto` or seed was saved.
        let raw = r#"{
            "os_type": "Windows",
            "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            "proxy": { "type": "socks5", "host": "proxy.test", "port": 1080, "save_traffic": false },
            "webrtc_policy": "PublicInterfaceOnly",
            "flags": {
                "navigator_masking": "Disabled",
                "localization_masking": "Custom",
                "proxy_masking": "Socks5",
                "geolocation_popup": "Prompt",
                "canvas_noise": "Mask",
                "graphics_noise": "Mask",
                "ports_masking": "Mask",
                "startup_behavior": "Custom",
                "humanize": true
            }
        }"#;
        let fp: Fingerprint = serde_json::from_str(raw).unwrap();
        assert_eq!(fp.webrtc_policy, Some(WebRtcPolicy::PublicInterfaceOnly));
        assert!(fp.is_masked(Dimension::WebRtc));
        assert!(!fp.is_masked(Dimension::Navigator), "Disabled stays off");
        assert!(fp.is_masked(Dimension::Proxy), "Socks5 is still a proxy");
        assert!(fp.is_masked(Dimension::Canvas), "Mask derives its seed");
        assert!(fp.flags.open_shadow_roots, "a flag added later is on");
    }
}
