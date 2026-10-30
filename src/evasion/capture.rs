//! Capture: what a real browser shows a page, kept as a [`Fingerprint`].
//!
//! One script ([`get_capture_js`]) reads, inside a page, what a detection
//! script could read: the navigator, the Client Hints, the screen, the time
//! zone, the WebGL strings, the media devices, the battery, the connection, the
//! speech voices, Bluetooth, and a few readings that no setting holds. The two
//! engines run it ([`Page::capture_fingerprint`](crate::sb_cdp::Page) for the pure
//! CDP engine, [`BaseCase::capture_fingerprint`](crate::BaseCase) for WebDriver)
//! and hand the answer to [`CapturedFingerprint::from_probe`], which maps it in
//! one place, so both engines give the same fingerprint for the same browser.
//!
//! A setting the page answered is a [`Value`](crate::evasion::spoof::Spoof::Value),
//! or [`Off`](crate::evasion::spoof::Spoof::Off) for an API that is not there. A
//! setting it could not answer stays [`Natural`](crate::evasion::spoof::Spoof::Natural),
//! and says why in [`CapturedFingerprint::unmapped`] under `unanswered`. The
//! result is the ordinary v2 [`Fingerprint`], so
//! [`save`](Fingerprint::save) writes the one file format every tool reads.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::evasion::capture::CapturedFingerprint;
//! use serde_json::json;
//!
//! // What the script returned for a page (shortened).
//! let readings = json!({
//!     "version": 1,
//!     "url": "https://example.test/",
//!     "secureContext": true,
//!     "navigator": {
//!         "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
//!         "platform": "Win32",
//!         "language": "en-US",
//!         "languages": ["en-US", "en"],
//!         "hardwareConcurrency": 8,
//!     },
//!     "screen": { "width": 1920, "height": 1080, "devicePixelRatio": 1 },
//!     "intl": { "timeZone": "Europe/Berlin" },
//! });
//!
//! let captured = CapturedFingerprint::from_probe(&readings, "2026-10-18T12:00:00Z")?;
//! assert_eq!(captured.fingerprint.timezone().as_str(), Some("Europe/Berlin"));
//! assert_eq!(captured.fingerprint.core_version(), Some(155));
//! assert_eq!(captured.probe.url, "https://example.test/");
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};

use super::fingerprint::{
    BatteryProfile, BrandVersion, BrowserType, ClientHints, ConnectionProfile, Fingerprint,
    FingerprintBuilder, MediaDevices, ScreenSize, SpeechVoice, StealthPolicy, WebGl,
};
use super::js::capture_js::get_capture_js;
use super::spoof::Spoof;
use crate::error::SeleniumBaseError;

/// The version of the readings the capture script returns. A reading of another
/// version is refused, not guessed at.
pub const PROBE_VERSION: u64 = 1;

/// The capture script as an expression, for an engine that evaluates one and
/// awaits the promise it gives back (the pure CDP engine).
#[must_use]
pub fn capture_expression() -> String {
    format!("(async function () {{\n{}\n}})()", get_capture_js())
}

/// The capture script as WebDriver's *Execute Async Script* takes it: it calls
/// the callback WebDriver hands over as the last argument, with the readings, or
/// with `{ version, failure }` when the script itself threw.
#[must_use]
pub fn capture_async_script() -> String {
    format!(
        "var sbrsDone = arguments[arguments.length - 1];\n\
         (async function () {{\n{}\n}})().then(sbrsDone, function (error) {{\n\
         sbrsDone({{ version: {PROBE_VERSION}, failure: String((error && error.message) || error) }});\n\
         }});",
        get_capture_js()
    )
}

/// The time to record for a capture taken now: RFC 3339, UTC, whole seconds.
pub(crate) fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Where and when a capture was taken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeInfo {
    /// The address of the page, as the page itself reports it (after redirects).
    pub url: String,
    /// When the capture was taken, as an RFC 3339 time in UTC.
    pub captured_at: String,
    /// The version of the browser: the full version from the Client Hints when
    /// the browser has them, else the version in its user agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_version: Option<String>,
}

/// What one capture read: the [`Fingerprint`] it maps to, the readings that no
/// setting holds, and where the capture was taken.
///
/// This is a Rust value, not a file format. The file is the fingerprint:
/// [`Fingerprint::save`] writes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapturedFingerprint {
    /// Every setting the page answered, as a value; the rest stay `Natural`.
    pub fingerprint: Fingerprint,
    /// The readings that have no setting, as the capture script returned them
    /// (their nested keys keep the script's spelling): `secure_context`,
    /// `webdriver`, `plugins_length`, `mime_types_length`, `cookie_enabled`,
    /// `do_not_track`, `pdf_viewer_enabled`, `pixel_depth`, `orientation`, `window`,
    /// `time_zone_offset_minutes`, `intl_locale`, `webgl_info`, `connection_type`,
    /// `permissions`, `canvas_hash`, `audio_hash` and `headless_markers`. Two more
    /// say what went wrong: `errors` (a reading that threw) and `unanswered`
    /// (a setting the page could not answer, with the reason).
    pub unmapped: Map<String, Value>,
    /// Where and when the capture was taken.
    pub probe: ProbeInfo,
}

impl CapturedFingerprint {
    /// Maps the readings of the capture script to a fingerprint.
    ///
    /// `captured_at` is the time to record, an RFC 3339 string. The readings are
    /// what [`capture_expression`] or [`capture_async_script`] returned.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Stealth`] when the script itself failed, when the
    /// readings are of another version than [`PROBE_VERSION`], or when they are
    /// not shaped as the script returns them.
    pub fn from_probe(
        raw: &Value,
        captured_at: impl Into<String>,
    ) -> Result<Self, SeleniumBaseError> {
        if let Some(reason) = raw.get("failure").and_then(Value::as_str) {
            return Err(stealth(format!(
                "the capture script failed in the page: {reason}"
            )));
        }
        match raw.get("version").and_then(Value::as_u64) {
            Some(PROBE_VERSION) => {}
            other => {
                return Err(stealth(format!(
                    "the capture readings are of version {other:?}, and this build reads \
                     version {PROBE_VERSION}"
                )));
            }
        }
        let probe: RawProbe = serde_json::from_value(raw.clone())
            .map_err(|error| stealth(format!("the capture readings are malformed: {error}")))?;

        let mut unanswered = Map::new();
        let fingerprint = probe.fingerprint_builder(&mut unanswered).build()?;
        let info = ProbeInfo {
            url: probe.url.clone(),
            captured_at: captured_at.into(),
            browser_version: probe.browser_version(),
        };
        Ok(Self {
            fingerprint,
            unmapped: probe.unmapped(unanswered),
            probe: info,
        })
    }

    /// The names of the settings the page could not answer, each with the reason.
    ///
    /// A setting that is not in this list and is `Natural` is one that no page
    /// can read (the position, the fonts, the proxy, the WebRTC addresses).
    #[must_use]
    pub fn unanswered(&self) -> Vec<(&str, &str)> {
        self.unmapped
            .get("unanswered")
            .and_then(Value::as_object)
            .map(|reasons| {
                reasons
                    .iter()
                    .map(|(setting, reason)| (setting.as_str(), reason.as_str().unwrap_or("")))
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn stealth(reason: String) -> SeleniumBaseError {
    SeleniumBaseError::Stealth { reason }
}

// ---------------------------------------------------------------------------
// The readings, as the script returns them.
// ---------------------------------------------------------------------------

/// An optional API: not there, there with readings, or not asked in a context
/// that could tell.
#[derive(Debug)]
enum Reading<T> {
    /// `{ supported: false }`: the browser has no such API.
    Absent,
    /// `{ supported: true, .. }`.
    Present(T),
    /// `{ unknown: reason }`, a failure, or nothing at all.
    Unknown(Option<String>),
}

impl<T> Default for Reading<T> {
    fn default() -> Self {
        Self::Unknown(None)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Reading<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let reason = value
            .get("unknown")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(match value.get("supported").and_then(Value::as_bool) {
            Some(false) => Self::Absent,
            Some(true) => match serde_json::from_value::<T>(value) {
                Ok(reading) => Self::Present(reading),
                Err(error) => Self::Unknown(Some(format!("a malformed reading: {error}"))),
            },
            None => Self::Unknown(reason),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawProbe {
    url: String,
    secure_context: bool,
    navigator: RawNavigator,
    user_agent_data: Reading<RawUserAgentData>,
    screen: Option<RawScreen>,
    window: Option<Value>,
    intl: Option<RawIntl>,
    webgl: Reading<RawWebGl>,
    media_devices: Reading<RawMediaDevices>,
    battery: Reading<RawBattery>,
    connection: Reading<RawConnection>,
    speech: Reading<RawSpeech>,
    bluetooth: Reading<RawBluetooth>,
    permissions: Map<String, Value>,
    canvas_hash: Option<String>,
    audio_hash: Option<String>,
    headless: Vec<String>,
    errors: Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawNavigator {
    user_agent: Option<String>,
    platform: Option<String>,
    language: Option<String>,
    languages: Option<Vec<String>>,
    hardware_concurrency: Option<f64>,
    device_memory: Option<f64>,
    max_touch_points: Option<f64>,
    vendor: Option<String>,
    product_sub: Option<String>,
    app_version: Option<String>,
    oscpu: Option<String>,
    #[serde(rename = "buildID")]
    build_id: Option<String>,
    webdriver: Option<bool>,
    plugins_length: Option<f64>,
    mime_types_length: Option<f64>,
    cookie_enabled: Option<bool>,
    do_not_track: Option<String>,
    pdf_viewer_enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawScreen {
    width: Option<f64>,
    height: Option<f64>,
    avail_width: Option<f64>,
    avail_height: Option<f64>,
    color_depth: Option<f64>,
    pixel_depth: Option<f64>,
    device_pixel_ratio: Option<f64>,
    orientation: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawIntl {
    time_zone: Option<String>,
    locale: Option<String>,
    offset_minutes: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawWebGl {
    vendor: Option<String>,
    renderer: Option<String>,
    version: Option<String>,
    shading_language_version: Option<String>,
    unmasked_vendor: Option<String>,
    unmasked_renderer: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawBrand {
    brand: String,
    version: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawUserAgentData {
    brands: Vec<RawBrand>,
    mobile: Option<bool>,
    platform: Option<String>,
    architecture: Option<String>,
    bitness: Option<String>,
    model: Option<String>,
    platform_version: Option<String>,
    ua_full_version: Option<String>,
    full_version_list: Vec<RawBrand>,
    wow64: Option<bool>,
    form_factors: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawMediaDevices {
    audio_inputs: Option<f64>,
    audio_outputs: Option<f64>,
    video_inputs: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawBattery {
    charging: Option<bool>,
    level: Option<f64>,
    charging_time: Option<f64>,
    discharging_time: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawConnection {
    effective_type: Option<String>,
    rtt: Option<f64>,
    downlink: Option<f64>,
    save_data: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawVoice {
    name: String,
    lang: String,
    default: bool,
    local_service: bool,
    #[serde(rename = "voiceURI")]
    voice_uri: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawSpeech {
    voices: Vec<RawVoice>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawBluetooth {
    available: Option<bool>,
}

// ---------------------------------------------------------------------------
// The mapping.
// ---------------------------------------------------------------------------

/// A whole, non-negative number from a reading that may arrive as `8` or `8.0`.
fn whole(number: Option<f64>) -> Option<u32> {
    number
        .filter(|n| n.is_finite() && (0.0..=f64::from(u32::MAX)).contains(n))
        .map(|n| n.round() as u32)
}

/// A positive, finite number.
fn positive(number: Option<f64>) -> Option<f64> {
    number.filter(|n| n.is_finite() && *n > 0.0)
}

/// A text that says something.
fn non_empty(text: &Option<String>) -> Option<&str> {
    text.as_deref().filter(|text| !text.is_empty())
}

/// The version after `token` in a user agent (`Chrome/155.0.0.0` gives
/// `155.0.0.0`).
fn token_version<'a>(user_agent: &'a str, token: &str) -> Option<&'a str> {
    let rest = user_agent.split(token).nth(1)?;
    let version = rest
        .split(|c: char| c.is_whitespace() || c == ';' || c == ')')
        .next()?;
    (!version.is_empty()).then_some(version)
}

/// The persona a user agent names.
fn browser_type_of(user_agent: &str) -> BrowserType {
    let ios = user_agent.contains("iPhone") || user_agent.contains("iPad");
    if user_agent.contains("Firefox/") {
        BrowserType::Firefox
    } else if ios && !user_agent.contains("Chrome/") {
        BrowserType::MobileSafari
    } else {
        BrowserType::Chromium
    }
}

/// The `Accept-Language` header Chromium builds from `navigator.languages`: the
/// first tag as it is, the others with falling quality values (`en-US,en;q=0.9`).
fn chromium_accept_language(languages: &[String]) -> String {
    languages
        .iter()
        .enumerate()
        .map(|(index, tag)| {
            if index == 0 {
                tag.clone()
            } else {
                format!("{tag};q=0.{}", 10_usize.saturating_sub(index).max(1))
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn brands(raw: &[RawBrand]) -> Vec<BrandVersion> {
    raw.iter()
        .map(|b| BrandVersion::new(b.brand.clone(), b.version.clone()))
        .collect()
}

impl RawProbe {
    /// The builder for the settings the readings answer. A setting that was not
    /// answered because a reading was unknown is named in `unanswered`.
    fn fingerprint_builder(&self, unanswered: &mut Map<String, Value>) -> FingerprintBuilder {
        let nav = &self.navigator;
        let mut builder = Fingerprint::builder().policy(StealthPolicy::balanced());

        let user_agent = non_empty(&nav.user_agent);
        if let Some(user_agent) = user_agent {
            builder = builder
                .user_agent(user_agent)
                .browser_type(browser_type_of(user_agent));
            if let Some(major) = token_version(user_agent, "Chrome/")
                .and_then(|version| version.split('.').next())
                .and_then(|major| major.parse::<u32>().ok())
            {
                builder = builder.core_version(major);
            }
        }
        if let Some(platform) = non_empty(&nav.platform) {
            builder = builder.platform(platform);
        }
        if let Some(cores) = whole(nav.hardware_concurrency).filter(|n| *n > 0) {
            builder = builder.hardware_concurrency(cores);
        }
        if let Some(memory) = positive(nav.device_memory) {
            builder = builder.device_memory(memory);
        }
        if let Some(points) = whole(nav.max_touch_points) {
            builder = builder.max_touch_points(points);
        }
        // An empty vendor is Firefox's own answer, so it is a value.
        if let Some(vendor) = &nav.vendor {
            builder = builder.vendor(vendor.as_str());
        }
        if let Some(text) = non_empty(&nav.oscpu) {
            builder = builder.oscpu(text);
        }
        if let Some(text) = non_empty(&nav.product_sub) {
            builder = builder.product_sub(text);
        }
        if let Some(text) = non_empty(&nav.build_id) {
            builder = builder.build_id(text);
        }
        if let Some(text) = non_empty(&nav.app_version) {
            builder = builder.app_version(text);
        }
        if let Some(locale) = non_empty(&nav.language) {
            builder = builder.locale(locale);
        }
        let languages: Vec<String> = nav
            .languages
            .iter()
            .flatten()
            .filter(|tag| !tag.is_empty())
            .cloned()
            .collect();
        if !languages.is_empty() {
            builder = builder.languages(languages.join(","));
            // A page cannot read the header, but Chromium builds it from the
            // list by a fixed rule; without it the user agent override would
            // send the locale alone.
            if user_agent.is_some_and(|ua| browser_type_of(ua) == BrowserType::Chromium) {
                builder = builder.accept_languages(chromium_accept_language(&languages));
            }
        }
        if let Some(zone) = self
            .intl
            .as_ref()
            .and_then(|intl| non_empty(&intl.time_zone))
        {
            builder = builder.timezone(zone);
        }

        if let Some(screen) = &self.screen {
            if let (Some(width), Some(height)) = (
                whole(screen.width).filter(|n| *n > 0),
                whole(screen.height).filter(|n| *n > 0),
            ) {
                builder = builder.screen(width, height);
            }
            if let (Some(width), Some(height)) = (
                whole(screen.avail_width).filter(|n| *n > 0),
                whole(screen.avail_height).filter(|n| *n > 0),
            ) {
                builder = builder.avail_screen(ScreenSize::new(width, height));
            }
            if let Some(ratio) = positive(screen.device_pixel_ratio) {
                builder = builder.pixel_ratio(ratio);
            }
            if let Some(depth) = whole(screen.color_depth).filter(|n| *n > 0) {
                builder = builder.color_depth(depth);
            }
        }

        builder = self.map_graphics(builder, unanswered);
        builder = self.map_devices(builder, unanswered);
        builder = self.map_client_hints(builder, unanswered);
        builder
    }

    fn map_graphics(
        &self,
        builder: FingerprintBuilder,
        unanswered: &mut Map<String, Value>,
    ) -> FingerprintBuilder {
        match &self.webgl {
            Reading::Present(gl) => {
                match (
                    non_empty(&gl.unmasked_vendor),
                    non_empty(&gl.unmasked_renderer),
                ) {
                    (Some(vendor), Some(renderer)) => builder.webgl(WebGl::new(vendor, renderer)),
                    _ => {
                        unanswered.insert(
                            "webgl".to_owned(),
                            json!("the browser hides the unmasked WebGL strings"),
                        );
                        builder
                    }
                }
            }
            Reading::Absent => {
                unanswered.insert("webgl".to_owned(), json!("WebGL is not available"));
                builder
            }
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "webgl", reason.as_deref());
                builder
            }
        }
    }

    fn map_devices(
        &self,
        mut builder: FingerprintBuilder,
        unanswered: &mut Map<String, Value>,
    ) -> FingerprintBuilder {
        builder = match &self.media_devices {
            Reading::Present(devices) => match (
                whole(devices.audio_inputs),
                whole(devices.audio_outputs),
                whole(devices.video_inputs),
            ) {
                (Some(audio_in), Some(audio_out), Some(video_in)) => {
                    builder.media_devices(MediaDevices::counts(audio_in, audio_out, video_in))
                }
                _ => builder,
            },
            Reading::Absent => builder.media_devices(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "media_devices", reason.as_deref());
                builder
            }
        };
        builder = match &self.battery {
            Reading::Present(battery) => match (battery.charging, battery.level) {
                (Some(charging), Some(level)) => builder.battery(BatteryProfile {
                    charging,
                    level,
                    charging_time: battery.charging_time.unwrap_or(f64::INFINITY),
                    discharging_time: battery.discharging_time.unwrap_or(f64::INFINITY),
                }),
                _ => builder,
            },
            Reading::Absent => builder.battery(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "battery", reason.as_deref());
                builder
            }
        };
        builder = match &self.connection {
            Reading::Present(link) => match (
                non_empty(&link.effective_type),
                whole(link.rtt),
                link.downlink.filter(|d| d.is_finite() && *d >= 0.0),
            ) {
                (Some(effective_type), Some(rtt), Some(downlink)) => {
                    builder.connection(ConnectionProfile {
                        effective_type: effective_type.to_owned(),
                        rtt,
                        downlink,
                        save_data: link.save_data.unwrap_or(false),
                    })
                }
                _ => builder,
            },
            Reading::Absent => builder.connection(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "connection", reason.as_deref());
                builder
            }
        };
        builder = match &self.speech {
            // No voice listed is an answer too: the browser lists none.
            Reading::Present(speech) if speech.voices.is_empty() => {
                builder.speech_voices(Spoof::Off)
            }
            Reading::Present(speech) => builder.speech_voices(
                speech
                    .voices
                    .iter()
                    .map(|voice| SpeechVoice {
                        name: voice.name.clone(),
                        lang: voice.lang.clone(),
                        default: voice.default,
                        local_service: voice.local_service,
                        voice_uri: voice.voice_uri.clone(),
                    })
                    .collect::<Vec<_>>(),
            ),
            Reading::Absent => builder.speech_voices(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "speech_voices", reason.as_deref());
                builder
            }
        };
        match &self.bluetooth {
            Reading::Present(bluetooth) => match bluetooth.available {
                Some(available) => builder.bluetooth(available),
                None => {
                    note_unanswered(
                        unanswered,
                        "bluetooth",
                        Some("the API is there but does not say whether an adapter is"),
                    );
                    builder
                }
            },
            Reading::Absent => builder.bluetooth(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "bluetooth", reason.as_deref());
                builder
            }
        }
    }

    fn map_client_hints(
        &self,
        builder: FingerprintBuilder,
        unanswered: &mut Map<String, Value>,
    ) -> FingerprintBuilder {
        match &self.user_agent_data {
            Reading::Present(data) => builder.client_hints(ClientHints {
                brands: brands(&data.brands),
                full_version_list: brands(&data.full_version_list),
                platform: data.platform.clone().unwrap_or_default(),
                platform_version: data.platform_version.clone().unwrap_or_default(),
                architecture: data.architecture.clone().unwrap_or_default(),
                bitness: data.bitness.clone().unwrap_or_default(),
                model: data.model.clone().unwrap_or_default(),
                ua_full_version: data.ua_full_version.clone().unwrap_or_default(),
                mobile: data.mobile.unwrap_or(false),
                wow64: data.wow64.unwrap_or(false),
                form_factors: data.form_factors.clone().unwrap_or_default(),
            }),
            Reading::Absent => builder.client_hints(Spoof::Off),
            Reading::Unknown(reason) => {
                note_unanswered(unanswered, "client_hints", reason.as_deref());
                builder
            }
        }
    }

    /// The version of the browser, for the record of the capture.
    fn browser_version(&self) -> Option<String> {
        if let Reading::Present(data) = &self.user_agent_data {
            for brand in [
                "Google Chrome",
                "Microsoft Edge",
                "Brave",
                "Opera",
                "Chromium",
            ] {
                if let Some(found) = data.full_version_list.iter().find(|b| b.brand == brand) {
                    return Some(found.version.clone());
                }
            }
            if let Some(version) = non_empty(&data.ua_full_version) {
                return Some(version.to_owned());
            }
        }
        let user_agent = non_empty(&self.navigator.user_agent)?;
        ["Firefox/", "Edg/", "Chrome/", "Version/"]
            .iter()
            .find_map(|token| token_version(user_agent, token))
            .map(str::to_owned)
    }

    /// The readings that no setting holds, with the notes of what went wrong.
    fn unmapped(&self, unanswered: Map<String, Value>) -> Map<String, Value> {
        let nav = &self.navigator;
        let mut out = Map::new();
        let mut put = |key: &str, value: Option<Value>| {
            if let Some(value) = value.filter(|value| !value.is_null()) {
                out.insert(key.to_owned(), value);
            }
        };
        put("secure_context", Some(json!(self.secure_context)));
        put("webdriver", nav.webdriver.map(Value::from));
        put("plugins_length", whole(nav.plugins_length).map(Value::from));
        put(
            "mime_types_length",
            whole(nav.mime_types_length).map(Value::from),
        );
        put("cookie_enabled", nav.cookie_enabled.map(Value::from));
        put("do_not_track", nav.do_not_track.clone().map(Value::from));
        put(
            "pdf_viewer_enabled",
            nav.pdf_viewer_enabled.map(Value::from),
        );
        if let Some(screen) = &self.screen {
            put("pixel_depth", whole(screen.pixel_depth).map(Value::from));
            put("orientation", screen.orientation.clone());
        }
        put("window", self.window.clone());
        if let Some(intl) = &self.intl {
            put(
                "time_zone_offset_minutes",
                intl.offset_minutes.map(json_number),
            );
            put("intl_locale", intl.locale.clone().map(Value::from));
        }
        if let Reading::Present(gl) = &self.webgl {
            put(
                "webgl_info",
                Some(json!({
                    "vendor": gl.vendor,
                    "renderer": gl.renderer,
                    "version": gl.version,
                    "shading_language_version": gl.shading_language_version,
                })),
            );
        }
        if let Reading::Present(link) = &self.connection {
            put("connection_type", link.kind.clone().map(Value::from));
        }
        if !self.permissions.is_empty() {
            put("permissions", Some(Value::Object(self.permissions.clone())));
        }
        put("canvas_hash", self.canvas_hash.clone().map(Value::from));
        put("audio_hash", self.audio_hash.clone().map(Value::from));
        put("headless_markers", Some(json!(self.headless)));
        if !self.errors.is_empty() {
            put("errors", Some(Value::Object(self.errors.clone())));
        }
        if !unanswered.is_empty() {
            put("unanswered", Some(Value::Object(unanswered)));
        }
        out
    }
}

/// A number as JSON writes it: a whole value as an integer, so that `-120.0` and `-120`
/// are one reading.
fn json_number(number: f64) -> Value {
    if number.is_finite() && number.fract() == 0.0 && number.abs() < 9.0e15 {
        Value::from(number as i64)
    } else {
        Value::from(number)
    }
}

/// Records why a setting was left to the browser.
fn note_unanswered(unanswered: &mut Map<String, Value>, setting: &str, reason: Option<&str>) {
    unanswered.insert(
        setting.to_owned(),
        json!(reason.unwrap_or("the capture script did not answer")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evasion::fingerprint::{Dimension, OsType};

    const CHROME_MACOS: &str =
        include_str!("../../tests/fixtures/fingerprint/probe_chrome_macos.json");
    const FIREFOX_LINUX: &str =
        include_str!("../../tests/fixtures/fingerprint/probe_firefox_linux.json");
    const CHROME_INSECURE: &str =
        include_str!("../../tests/fixtures/fingerprint/probe_chrome_insecure.json");
    const SAFARI_IOS: &str = include_str!("../../tests/fixtures/fingerprint/probe_safari_ios.json");
    /// Recorded by `tests/sb_cdp_capture_chrome.rs` from a headless Chrome 155 on a Mac.
    const REAL_HEADLESS: &str =
        include_str!("../../tests/fixtures/fingerprint/probe_chrome_real_headless.json");

    const AT: &str = "2026-10-18T12:00:00Z";

    fn capture(fixture: &str) -> CapturedFingerprint {
        let raw: Value = serde_json::from_str(fixture).expect("the fixture is JSON");
        CapturedFingerprint::from_probe(&raw, AT).expect("the fixture maps")
    }

    #[test]
    fn a_chrome_probe_maps_every_answered_setting_to_a_value() {
        let fp = capture(CHROME_MACOS).fingerprint;

        assert!(fp
            .user_agent()
            .as_str()
            .is_some_and(|ua| ua.contains("Chrome/155")));
        assert_eq!(fp.platform().as_str(), Some("MacIntel"));
        assert_eq!(fp.hardware_concurrency().value(), Some(&8));
        assert_eq!(fp.device_memory().value(), Some(&8.0));
        assert_eq!(fp.max_touch_points().value(), Some(&0));
        assert_eq!(fp.vendor().as_str(), Some("Google Inc."));
        assert_eq!(fp.product_sub().as_str(), Some("20030107"));
        assert_eq!(fp.locale().as_str(), Some("en-US"));
        assert_eq!(fp.languages().as_str(), Some("en-US,en,de"));
        assert_eq!(fp.timezone().as_str(), Some("Europe/Berlin"));
        assert_eq!(fp.screen().value(), Some(&ScreenSize::new(1512, 982)));
        assert_eq!(fp.avail_screen().value(), Some(&ScreenSize::new(1512, 944)));
        assert_eq!(fp.pixel_ratio().value(), Some(&2.0));
        assert_eq!(fp.color_depth().value(), Some(&30));
        let gl = fp.webgl().value().expect("webgl is a value");
        assert_eq!(gl.vendor.as_deref(), Some("Google Inc. (Apple)"));
        assert!(gl
            .renderer
            .as_deref()
            .is_some_and(|r| r.contains("Apple M3 Pro")));
        assert_eq!(
            fp.media_devices().value(),
            Some(&MediaDevices::counts(1, 1, 1))
        );
        assert_eq!(fp.bluetooth().value(), Some(&true));
        assert_eq!(fp.speech_voices().value().map(Vec::len), Some(2));
        let battery = fp.battery().value().expect("battery is a value");
        assert!(battery.charging && (battery.level - 1.0).abs() < f64::EPSILON);
        assert!(
            battery.discharging_time.is_infinite(),
            "a null time is infinite"
        );
        let link = fp.connection().value().expect("connection is a value");
        assert_eq!((link.effective_type.as_str(), link.rtt), ("4g", 50));
    }

    #[test]
    fn client_hints_carry_the_low_and_the_high_entropy_values() {
        let fp = capture(CHROME_MACOS).fingerprint;

        let hints = fp.client_hints().value().expect("client hints are a value");
        assert_eq!(hints.platform, "macOS");
        assert_eq!(hints.platform_version, "27.0.1");
        assert_eq!(hints.architecture, "arm");
        assert_eq!(hints.ua_full_version, "155.0.8059.40");
        assert_eq!(hints.form_factors, ["Desktop"]);
        assert!(!hints.mobile && !hints.wow64);
        assert_eq!(hints.brands.len(), 3);
        assert_eq!(
            hints.full_version_list.last().map(|b| b.version.as_str()),
            Some("155.0.8059.40")
        );
    }

    #[test]
    fn the_persona_and_the_measured_chrome_come_from_the_user_agent() {
        let fp = capture(CHROME_MACOS).fingerprint;

        assert_eq!(fp.os_type(), OsType::Macos);
        assert_eq!(fp.browser_type(), BrowserType::Chromium);
        assert_eq!(
            fp.core_version(),
            Some(155),
            "it must claim the Chrome that was measured"
        );
        assert!(fp.policy().chrome_runtime, "the policy of every preset");
    }

    #[test]
    fn what_a_page_cannot_read_stays_natural() {
        let fp = capture(CHROME_MACOS).fingerprint;

        assert!(fp.geolocation().is_natural());
        assert!(fp.fonts().is_natural());
        assert!(fp.proxy().is_natural());
        assert!(fp.webrtc().is_natural());
        assert!(!fp.is_masked(Dimension::Geolocation));
    }

    #[test]
    fn a_chromium_capture_derives_the_accept_language_header_from_the_list() {
        let fp = capture(CHROME_MACOS).fingerprint;

        assert_eq!(
            fp.accept_languages().as_str(),
            Some("en-US,en;q=0.9,de;q=0.8")
        );
    }

    #[test]
    fn accept_language_quality_values_fall_by_a_tenth_and_stop_at_a_tenth() {
        let tags: Vec<String> = ["a", "b", "c"].iter().map(ToString::to_string).collect();
        assert_eq!(chromium_accept_language(&tags), "a,b;q=0.9,c;q=0.8");
        assert_eq!(chromium_accept_language(&[]), "");

        let many: Vec<String> = (0..14).map(|n| format!("l{n}")).collect();
        let header = chromium_accept_language(&many);
        assert!(header.ends_with("l13;q=0.1"), "{header}");
        assert!(header.contains("l9;q=0.1,l10;q=0.1"), "{header}");
    }

    #[test]
    fn an_api_the_browser_lacks_is_off_and_a_missing_value_is_left_alone() {
        let fp = capture(FIREFOX_LINUX).fingerprint;

        assert!(fp.client_hints().is_off(), "Firefox has no userAgentData");
        assert!(fp.battery().is_off());
        assert!(fp.connection().is_off());
        assert!(fp.bluetooth().is_off());
        assert!(fp.speech_voices().is_off(), "no voice listed is an answer");
        assert_eq!(
            fp.media_devices().value(),
            Some(&MediaDevices::counts(0, 0, 0))
        );
        assert!(
            fp.device_memory().is_natural(),
            "Firefox has no deviceMemory"
        );
    }

    #[test]
    fn a_firefox_probe_gives_the_firefox_persona_and_its_own_navigator_values() {
        let captured = capture(FIREFOX_LINUX);
        let fp = &captured.fingerprint;

        assert_eq!(fp.browser_type(), BrowserType::Firefox);
        assert_eq!(fp.os_type(), OsType::Linux);
        assert_eq!(fp.core_version(), None, "there is no Chrome major to claim");
        assert_eq!(fp.oscpu().as_str(), Some("Linux x86_64"));
        assert_eq!(fp.build_id().as_str(), Some("20181001000000"));
        assert_eq!(fp.vendor().as_str(), Some(""), "Firefox's vendor is empty");
        assert!(
            fp.accept_languages().is_natural(),
            "Firefox builds its header another way"
        );
        assert_eq!(captured.probe.browser_version.as_deref(), Some("130.0"));
    }

    #[test]
    fn a_safari_probe_gives_the_mobile_safari_persona() {
        let fp = capture(SAFARI_IOS).fingerprint;

        assert_eq!(fp.browser_type(), BrowserType::MobileSafari);
        assert_eq!(fp.os_type(), OsType::Ios);
        assert_eq!(fp.max_touch_points().value(), Some(&5));
        assert!(fp.client_hints().is_off());
        assert!(fp.accept_languages().is_natural());
    }

    #[test]
    fn an_api_that_needs_a_secure_context_is_not_called_absent_in_an_insecure_one() {
        let captured = capture(CHROME_INSECURE);
        let fp = &captured.fingerprint;

        // Their absence says nothing about the browser, so they stay Natural.
        assert!(fp.client_hints().is_natural());
        assert!(fp.media_devices().is_natural());
        assert!(fp.battery().is_natural());
        assert!(fp.bluetooth().is_natural());
        // What an insecure page can read is read.
        assert!(fp.connection().is_value());
        assert!(fp.speech_voices().is_value());
        let unanswered = captured.unanswered();
        for setting in ["client_hints", "media_devices", "battery", "bluetooth"] {
            assert!(
                unanswered.iter().any(|(name, _)| *name == setting),
                "{setting} should be unanswered: {unanswered:?}"
            );
        }
        assert!(unanswered
            .iter()
            .all(|(_, reason)| reason.contains("secure context")));
        assert_eq!(captured.unmapped["secure_context"], json!(false));
    }

    #[test]
    fn the_readings_without_a_setting_are_kept() {
        let unmapped = capture(CHROME_MACOS).unmapped;

        assert_eq!(unmapped["secure_context"], json!(true));
        assert_eq!(unmapped["webdriver"], json!(false));
        assert_eq!(unmapped["plugins_length"], json!(5));
        assert_eq!(unmapped["mime_types_length"], json!(2));
        assert_eq!(unmapped["canvas_hash"], json!("5fa3c2d1"));
        assert_eq!(unmapped["audio_hash"], json!("124.04347527516074"));
        assert_eq!(unmapped["headless_markers"], json!([]));
        assert_eq!(unmapped["window"]["innerWidth"], json!(1440));
        assert_eq!(unmapped["permissions"]["geolocation"], json!("prompt"));
        assert_eq!(unmapped["time_zone_offset_minutes"], json!(-120));
        assert_eq!(unmapped["pixel_depth"], json!(30));
        assert_eq!(unmapped["webgl_info"]["renderer"], json!("WebKit WebGL"));
        assert!(!unmapped.contains_key("errors"), "nothing went wrong");
        assert!(!unmapped.contains_key("unanswered"));
        // A reading that is also a setting is not repeated here.
        assert!(!unmapped.contains_key("hardware_concurrency"));
    }

    #[test]
    fn the_record_of_the_capture_names_the_page_the_time_and_the_browser() {
        let captured = capture(CHROME_MACOS);

        assert_eq!(captured.probe.url, "https://example.test/");
        assert_eq!(captured.probe.captured_at, AT);
        assert_eq!(
            captured.probe.browser_version.as_deref(),
            Some("155.0.8059.40")
        );
    }

    #[test]
    fn the_browser_version_falls_back_to_the_user_agent_without_client_hints() {
        let insecure = capture(CHROME_INSECURE);
        let safari = capture(SAFARI_IOS);

        assert_eq!(insecure.probe.browser_version.as_deref(), Some("155.0.0.0"));
        assert_eq!(safari.probe.browser_version.as_deref(), Some("26.0"));
    }

    #[test]
    fn a_captured_chrome_is_coherent() {
        for fixture in [CHROME_MACOS, CHROME_INSECURE, FIREFOX_LINUX, SAFARI_IOS] {
            let report = capture(fixture).fingerprint.validate();

            assert!(report.is_coherent(), "{:?}", report.errors);
        }
        let report = capture(CHROME_MACOS).fingerprint.validate();
        // The locale is English, the zone German: a real person's browser can be.
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(report.warnings[0].contains("Europe/Berlin"));
    }

    #[test]
    fn the_readings_recorded_from_a_real_headless_chrome_map_to_a_coherent_fingerprint() {
        let captured = capture(REAL_HEADLESS);
        let fp = &captured.fingerprint;

        assert!(fp.validate().is_coherent(), "{:?}", fp.validate().errors);
        assert_eq!(fp.core_version(), Some(155));
        assert_eq!(fp.os_type(), OsType::Macos);
        assert!(fp
            .user_agent()
            .as_str()
            .is_some_and(|ua| ua.contains("HeadlessChrome/155")));
        assert_eq!(
            captured.probe.browser_version.as_deref(),
            Some("155.0.8059.85")
        );
        let hints = fp.client_hints().value().expect("client hints");
        assert_eq!(hints.brands[0].brand, "Google Chrome");
        assert_eq!(hints.platform_version, "27.0.1");
        let battery = fp.battery().value().expect("battery");
        assert!(!battery.charging && battery.charging_time.is_infinite());
        assert!((battery.discharging_time - 3660.0).abs() < f64::EPSILON);
        assert_eq!(fp.speech_voices().value().map(Vec::len), Some(19));
        assert_eq!(fp.accept_languages().as_str(), Some("en-US,en;q=0.9"));
        // A headless Chrome says so, and is automated.
        assert_eq!(
            captured.unmapped["headless_markers"],
            json!(["user_agent_says_headless", "navigator_webdriver_true"])
        );
        assert_eq!(captured.unmapped["webdriver"], json!(true));
        assert!(
            captured.unanswered().is_empty(),
            "{:?}",
            captured.unanswered()
        );
    }

    #[test]
    fn a_capture_round_trips_through_its_json() {
        let captured = capture(CHROME_MACOS);

        let text = serde_json::to_string(&captured).unwrap();
        let back: CapturedFingerprint = serde_json::from_str(&text).unwrap();

        assert_eq!(back, captured);
    }

    #[test]
    fn the_fingerprint_of_a_capture_is_the_plain_v2_json() {
        let captured = capture(CHROME_MACOS);

        let text = serde_json::to_string(&captured.fingerprint).unwrap();
        let back: Fingerprint = serde_json::from_str(&text).unwrap();

        assert_eq!(back, captured.fingerprint);
        assert!(
            !text.contains("\"geolocation\""),
            "natural settings are left out: {text}"
        );
    }

    #[test]
    fn numbers_arrive_as_whole_numbers_or_as_floats() {
        let mut raw: Value = serde_json::from_str(CHROME_MACOS).unwrap();
        raw["navigator"]["hardwareConcurrency"] = json!(8.0);
        raw["screen"]["width"] = json!(1512.0);

        let fp = CapturedFingerprint::from_probe(&raw, AT)
            .unwrap()
            .fingerprint;

        assert_eq!(fp.hardware_concurrency().value(), Some(&8));
        assert_eq!(fp.screen().value().map(|s| s.width), Some(1512));
    }

    #[test]
    fn a_reading_that_is_missing_or_nonsense_leaves_its_setting_natural() {
        let mut raw: Value = serde_json::from_str(CHROME_MACOS).unwrap();
        raw["navigator"]["hardwareConcurrency"] = json!(0);
        raw["screen"]["width"] = json!(0);
        raw["screen"]["devicePixelRatio"] = json!(-1);
        raw["navigator"]["deviceMemory"] = Value::Null;
        raw["intl"] = Value::Null;
        raw["battery"] = json!("garbage");

        let captured = CapturedFingerprint::from_probe(&raw, AT).unwrap();
        let fp = &captured.fingerprint;

        assert!(fp.hardware_concurrency().is_natural());
        assert!(fp.screen().is_natural());
        assert!(fp.pixel_ratio().is_natural());
        assert!(fp.device_memory().is_natural());
        assert!(fp.timezone().is_natural());
        assert!(fp.battery().is_natural());
        assert!(captured
            .unanswered()
            .iter()
            .any(|(name, _)| *name == "battery"));
    }

    #[test]
    fn a_script_that_failed_is_an_error_that_says_so() {
        let raw = json!({ "version": 1, "failure": "ReferenceError: boom" });

        let error = CapturedFingerprint::from_probe(&raw, AT).unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Stealth { .. }));
        assert!(
            error.to_string().contains("ReferenceError: boom"),
            "{error}"
        );
    }

    #[test]
    fn readings_of_another_version_are_refused() {
        for raw in [
            json!({ "version": 2 }),
            json!({}),
            json!(null),
            json!({ "version": "1" }),
        ] {
            let error = CapturedFingerprint::from_probe(&raw, AT).unwrap_err();

            assert!(error.to_string().contains("version"), "{error}");
        }
    }

    #[test]
    fn readings_of_the_wrong_shape_are_refused() {
        let raw = json!({ "version": 1, "navigator": "not an object" });

        let error = CapturedFingerprint::from_probe(&raw, AT).unwrap_err();

        assert!(error.to_string().contains("malformed"), "{error}");
    }

    #[test]
    fn the_cdp_expression_is_an_awaited_async_function() {
        let script = capture_expression();

        assert!(script.starts_with("(async function () {"));
        assert!(script.trim_end().ends_with("})()"));
        assert!(script.contains("getHighEntropyValues"));
    }

    #[test]
    fn the_webdriver_script_calls_back_with_the_readings_or_the_failure() {
        let script = capture_async_script();

        assert!(script.starts_with("var sbrsDone = arguments[arguments.length - 1];"));
        assert!(script.contains(".then(sbrsDone, function (error)"));
        assert!(script.contains("failure: String("));
        assert!(script.contains(&format!("version: {PROBE_VERSION}")));
    }

    #[test]
    fn the_script_asks_for_no_permission_and_writes_nothing() {
        let script = get_capture_js();

        for forbidden in [
            "requestPermission",
            "getUserMedia",
            "getCurrentPosition",
            "requestDevice",
            "localStorage",
            "sessionStorage",
            "document.cookie",
            "fetch(",
            "XMLHttpRequest",
            "innerHTML",
            "appendChild",
        ] {
            assert!(!script.contains(forbidden), "the script uses {forbidden}");
        }
    }
}
