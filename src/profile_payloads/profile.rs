use serde::{Deserialize, Serialize};

use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint as stealth_fp;
use crate::stealth::fingerprint::{
    AudioNoise, CanvasNoise, Fingerprint, FingerprintBuilder, Geolocation, MediaDeviceSpec,
    MediaDevices, OsType, QuicMode, ScreenSize, Spoof, StealthPolicy, WebGl, WebRtcAddresses,
    WebRtcPolicy,
};

/// Top-level external browser profile payload.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProfileParams {
    pub name: String,
    #[serde(default = "default_browser_type")]
    pub browser_type: String,
    #[serde(default = "default_folder_id")]
    pub folder_id: String,
    #[serde(default = "default_os_type")]
    pub os_type: String,
    #[serde(default = "default_automation")]
    pub automation: String,
    #[serde(default)]
    pub is_headless: bool,
    #[serde(default)]
    pub core_version: Option<u32>,
    #[serde(default)]
    pub core_minor_version: Option<u32>,
    #[serde(default = "default_auto_update_core")]
    pub auto_update_core: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_times")]
    pub times: u32,
    #[serde(default)]
    pub notes: String,
    pub parameters: Parameters,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Parameters {
    #[serde(default)]
    pub flags: ProfileFlags,
    #[serde(default)]
    pub fingerprint: ProfileFingerprint,
    #[serde(default)]
    pub storage: StorageOptions,
    #[serde(default)]
    pub proxy: Option<ProfileProxy>,
    #[serde(default)]
    pub custom_start_urls: Vec<String>,
}

/// The masking flags of a payload, as the strings a payload carries: a keyword
/// such as `"mask"`, `"natural"` or `"auto"`, or a literal value such as
/// `"1920x1080x24"`. [`ProfileParams::to_fingerprint`] turns them into the
/// settings and the [`StealthPolicy`] of a [`Fingerprint`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileFlags {
    #[serde(default = "default_webrtc_masking")]
    pub webrtc_masking: String,
    #[serde(default = "default_proxy_masking")]
    pub proxy_masking: String,
    #[serde(default = "default_geolocation_popup")]
    pub geolocation_popup: String,
    #[serde(default = "default_audio_masking")]
    pub audio_masking: String,
    #[serde(default = "default_graphics_noise")]
    pub graphics_noise: String,
    #[serde(default = "default_ports_masking")]
    pub ports_masking: String,
    #[serde(default = "default_navigator_masking")]
    pub navigator_masking: String,
    #[serde(default = "default_localization_masking")]
    pub localization_masking: String,
    #[serde(default = "default_timezone_masking")]
    pub timezone_masking: String,
    #[serde(default = "default_graphics_masking")]
    pub graphics_masking: String,
    #[serde(default = "default_fonts_masking")]
    pub fonts_masking: String,
    #[serde(default = "default_media_devices_masking")]
    pub media_devices_masking: String,
    #[serde(default = "default_screen_masking")]
    pub screen_masking: String,
    #[serde(default = "default_geolocation_masking")]
    pub geolocation_masking: String,
    #[serde(default = "default_quic_mode")]
    pub quic_mode: String,
    #[serde(default)]
    pub canvas_noise: Option<String>,
    #[serde(default = "default_startup_behavior")]
    pub startup_behavior: String,
    #[serde(default)]
    pub disable_csp: bool,
    #[serde(default)]
    pub grant_permissions: bool,
    #[serde(default)]
    pub native_spoofing: bool,
}

impl Default for ProfileFlags {
    fn default() -> Self {
        Self {
            webrtc_masking: default_webrtc_masking(),
            proxy_masking: default_proxy_masking(),
            geolocation_popup: default_geolocation_popup(),
            audio_masking: default_audio_masking(),
            graphics_noise: default_graphics_noise(),
            ports_masking: default_ports_masking(),
            navigator_masking: default_navigator_masking(),
            localization_masking: default_localization_masking(),
            timezone_masking: default_timezone_masking(),
            graphics_masking: default_graphics_masking(),
            fonts_masking: default_fonts_masking(),
            media_devices_masking: default_media_devices_masking(),
            screen_masking: default_screen_masking(),
            geolocation_masking: default_geolocation_masking(),
            quic_mode: default_quic_mode(),
            canvas_noise: None,
            startup_behavior: default_startup_behavior(),
            disable_csp: false,
            grant_permissions: false,
            native_spoofing: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageOptions {
    #[serde(default = "default_is_local")]
    pub is_local: bool,
    #[serde(default = "default_save_service_worker")]
    pub save_service_worker: bool,
}

impl Default for StorageOptions {
    fn default() -> Self {
        Self {
            is_local: default_is_local(),
            save_service_worker: default_save_service_worker(),
        }
    }
}

/// The fingerprint section of a payload, in the shape of the payload (nested
/// `navigator`, `screen` and so on). [`ProfileParams::to_fingerprint`] turns it
/// into a [`Fingerprint`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProfileFingerprint {
    #[serde(default)]
    pub navigator: Option<NavigatorFingerprint>,
    #[serde(default)]
    pub localization: Option<LocalizationFingerprint>,
    #[serde(default)]
    pub max_touch_points: Option<u32>,
    #[serde(default)]
    pub timezone: Option<TimezoneFingerprint>,
    #[serde(default)]
    pub graphic: Option<GraphicFingerprint>,
    #[serde(default)]
    pub webrtc: Option<WebrtcFingerprint>,
    #[serde(default)]
    pub media_devices: Option<MediaDevicesFingerprint>,
    #[serde(default)]
    pub screen: Option<ScreenFingerprint>,
    #[serde(default)]
    pub geolocation: Option<GeolocationFingerprint>,
    #[serde(default)]
    pub ports: Vec<u16>,
    #[serde(default)]
    pub fonts: Vec<String>,
    #[serde(default)]
    pub cmd_params: CmdParams,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NavigatorFingerprint {
    #[serde(default)]
    pub hardware_concurrency: Option<u32>,
    #[serde(default)]
    pub device_memory: Option<f64>,
    pub user_agent: String,
    pub platform: String,
    #[serde(default)]
    pub os_cpu: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LocalizationFingerprint {
    pub languages: String,
    pub locale: String,
    pub accept_languages: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TimezoneFingerprint {
    pub zone: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GraphicFingerprint {
    pub renderer: String,
    pub vendor: String,
    #[serde(default)]
    pub vendor_id: String,
    #[serde(default)]
    pub renderer_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WebrtcFingerprint {
    pub public_ip: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MediaDevicesFingerprint {
    #[serde(default)]
    pub audio_outputs: u32,
    #[serde(default)]
    pub audio_inputs: u32,
    #[serde(default)]
    pub video_inputs: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ScreenFingerprint {
    pub width: u32,
    pub height: u32,
    pub pixel_ratio: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GeolocationFingerprint {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub accuracy: f64,
    #[serde(default)]
    pub altitude: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CmdParams {
    #[serde(default)]
    pub params: Vec<CmdParam>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CmdParam {
    pub flag: String,
    #[serde(default)]
    pub value: String,
}

/// The proxy section of a payload. [`ProfileParams::to_fingerprint`] turns it
/// into the [`ProxyConfig`](stealth_fp::ProxyConfig) of a [`Fingerprint`].
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ProfileProxy {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub save_traffic: bool,
}

// Written by hand so that logging a profile never prints the password.
impl std::fmt::Debug for ProfileProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileProxy")
            .field("proxy_type", &self.proxy_type)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("save_traffic", &self.save_traffic)
            .finish()
    }
}

fn default_browser_type() -> String {
    "chromium".into()
}
fn default_folder_id() -> String {
    "default".into()
}
fn default_os_type() -> String {
    "windows".into()
}
fn default_automation() -> String {
    "selenium".into()
}
fn default_auto_update_core() -> bool {
    true
}
fn default_times() -> u32 {
    1
}

// The default of a flag a payload leaves out is `auto`, so the values the
// payload holds decide and nothing is drawn that nobody asked for. The two
// exceptions keep their old meaning: the ports the payload lists are blocked,
// and QUIC is off.
fn default_webrtc_masking() -> String {
    "auto".into()
}
fn default_proxy_masking() -> String {
    "auto".into()
}
fn default_geolocation_popup() -> String {
    "auto".into()
}
fn default_audio_masking() -> String {
    "auto".into()
}
fn default_graphics_noise() -> String {
    "mask".into()
}
fn default_ports_masking() -> String {
    "mask".into()
}
fn default_navigator_masking() -> String {
    "auto".into()
}
fn default_localization_masking() -> String {
    "auto".into()
}
fn default_timezone_masking() -> String {
    "auto".into()
}
fn default_graphics_masking() -> String {
    "auto".into()
}
fn default_fonts_masking() -> String {
    "auto".into()
}
fn default_media_devices_masking() -> String {
    "auto".into()
}
fn default_screen_masking() -> String {
    "auto".into()
}
fn default_geolocation_masking() -> String {
    "auto".into()
}
fn default_quic_mode() -> String {
    "disabled".into()
}
fn default_startup_behavior() -> String {
    "recover".into()
}

fn default_is_local() -> bool {
    true
}
fn default_save_service_worker() -> bool {
    true
}

/// What one payload word says about a key.
///
/// Every flag of a payload reads the same words, whichever key it is on, and
/// the words fall into four groups. A key adds literal values of its own (a
/// screen as `1920x1080x24`, a time zone, a user agent) and a few synonyms; see
/// the table in the documentation of [`ProfileParams::to_fingerprint`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    /// The browser's own value, whatever values the payload holds: `natural`,
    /// `native`, `pass_through`, `whitelist`.
    Natural,
    /// The values the payload holds, and the browser's own for the rest:
    /// `auto`.
    Auto,
    /// The values the payload holds, and a drawn one for the rest: `mask`,
    /// `spoof`, `custom`, `generic`, `random`, `noise`.
    Random,
    /// The API turned off, for the keys where that means something: `off`,
    /// `disabled`, `block`, `empty`.
    Off,
}

/// How one flag reads: one of the words, or a literal value of the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading<'a> {
    Word(Word),
    Literal(&'a str),
}

/// The word `text` is, if it is one.
fn classify(text: &str, synonyms: &[(&str, Word)]) -> Option<Word> {
    let lower = text.trim().to_ascii_lowercase();
    if let Some((_, word)) = synonyms.iter().find(|(name, _)| *name == lower) {
        return Some(*word);
    }
    match lower.as_str() {
        "" | "auto" => Some(Word::Auto),
        "natural" | "native" | "pass_through" | "whitelist" => Some(Word::Natural),
        "mask" | "spoof" | "custom" | "generic" | "random" | "noise" => Some(Word::Random),
        "off" | "disabled" | "block" | "empty" => Some(Word::Off),
        _ => None,
    }
}

/// Reads the flag `key`: a word, else a literal value of the key, else an
/// unknown keyword, which is logged and read as `natural` rather than
/// guessed at.
fn read<'a>(
    key: &str,
    text: &'a str,
    synonyms: &[(&str, Word)],
    is_literal: impl Fn(&str) -> bool,
) -> Reading<'a> {
    if let Some(word) = classify(text, synonyms) {
        return Reading::Word(word);
    }
    if is_literal(text.trim()) {
        return Reading::Literal(text.trim());
    }
    tracing::warn!(
        key,
        keyword = text,
        "unknown keyword in a profile flag; reading it as natural"
    );
    Reading::Word(Word::Natural)
}

/// The state of one setting, given the word of its flag and the value the
/// payload holds for it.
///
/// `drawable` says whether `Random` is a state it can have, and `off_valid`
/// whether `Off` is. A word that asks for a state the setting cannot have is
/// logged and read as natural.
fn settle<T>(key: &str, word: Word, given: Option<T>, drawable: bool, off_valid: bool) -> Spoof<T> {
    match word {
        Word::Natural => Spoof::Natural,
        Word::Auto => given.map_or(Spoof::Natural, Spoof::Value),
        Word::Random if drawable => given.map_or(Spoof::Random, Spoof::Value),
        Word::Random => given.map_or_else(
            || {
                tracing::warn!(key, "this setting has no random value; leaving it natural");
                Spoof::Natural
            },
            Spoof::Value,
        ),
        Word::Off if off_valid => Spoof::Off,
        Word::Off => {
            tracing::warn!(key, "this setting has no off state; leaving it natural");
            Spoof::Natural
        }
    }
}

/// A word, or a literal read as `mask` with the value the literal names.
fn word_of(reading: Reading<'_>) -> Word {
    match reading {
        Reading::Word(word) => word,
        Reading::Literal(_) => Word::Random,
    }
}

fn parse_canvas_noise(s: &str) -> CanvasNoise {
    match s.trim().to_ascii_lowercase().as_str() {
        "random" => CanvasNoise::Random,
        "low" => CanvasNoise::Low,
        "persistent" | "mask" | "spoof" | "custom" | "generic" | "noise" => CanvasNoise::Persistent,
        other => {
            if classify(other, &[]).is_none() {
                tracing::warn!(
                    key = "canvas_noise",
                    keyword = s,
                    "unknown keyword in a profile flag; reading it as natural"
                );
            }
            CanvasNoise::Natural
        }
    }
}

fn parse_audio_noise(s: &str) -> AudioNoise {
    match s.trim().to_ascii_lowercase().as_str() {
        "random" => AudioNoise::Random,
        "mask" | "spoof" | "custom" | "generic" | "noise" | "persistent" => AudioNoise::Persistent,
        other => {
            if classify(other, &[]).is_none() {
                tracing::warn!(
                    key = "audio_masking",
                    keyword = s,
                    "unknown keyword in a profile flag; reading it as natural"
                );
            }
            AudioNoise::Natural
        }
    }
}

fn parse_quic(s: &str) -> QuicMode {
    match s.trim().to_ascii_lowercase().as_str() {
        "enabled" => QuicMode::Enabled,
        "disabled" => QuicMode::Disabled,
        "force_http2" | "forcehttp2" => QuicMode::ForceHttp2,
        other => {
            if classify(other, &[]).is_none() {
                tracing::warn!(
                    key = "quic_mode",
                    keyword = s,
                    "unknown keyword in a profile flag; reading it as natural"
                );
            }
            QuicMode::Natural
        }
    }
}

fn parse_os(s: &str) -> OsType {
    match s.to_lowercase().as_str() {
        "macos" => OsType::Macos,
        "linux" => OsType::Linux,
        "android" => OsType::Android,
        "ios" | "ipados" => OsType::Ios,
        _ => OsType::Windows,
    }
}

fn parse_dimensions(s: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() >= 2 {
        let width = parts[0].parse().ok()?;
        let height = parts[1].parse().ok()?;
        let depth = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(24);
        return Some((width, height, depth));
    }
    None
}

fn parse_coordinates(s: &str) -> Option<(f64, f64, f64)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 2 {
        if let (Ok(latitude), Ok(longitude)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
            let altitude = parts
                .get(2)
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(0.0);
            return Some((latitude, longitude, altitude));
        }
    }
    None
}

/// The text of a payload value, or `None` when it is empty or blank: a payload
/// writes `""` for a value it does not give.
fn given_text(s: &str) -> Option<String> {
    let text = s.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// A literal graphics flag: `vendor~renderer`, with both parts present.
fn parse_vendor_renderer(s: &str) -> Option<(String, String)> {
    let (vendor, renderer) = s.split_once('~')?;
    let (vendor, renderer) = (vendor.trim(), renderer.split('~').next()?.trim());
    (!vendor.is_empty() && !renderer.is_empty()).then(|| (vendor.to_string(), renderer.to_string()))
}

/// A literal navigator flag: a user agent, which opens with `Mozilla/<digit>`
/// and has its platform token in parentheses.
fn is_user_agent(s: &str) -> bool {
    s.strip_prefix("Mozilla/")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        && s.contains('(')
        && s.contains(')')
}

/// A language tag: two or three letters, then the subtags that follow (`-US`,
/// `-Hant-TW`). `needs_region` asks for at least one subtag.
fn is_language_tag(s: &str, needs_region: bool) -> bool {
    let mut parts = s.split('-');
    let language = parts.next().unwrap_or("");
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    let mut subtags = 0;
    for subtag in parts {
        if subtag.is_empty() || !subtag.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return false;
        }
        subtags += 1;
    }
    subtags > 0 || !needs_region
}

/// A literal localization flag: a locale such as `de-DE`, or an
/// `Accept-Language` list that opens with one (`en-US,en;q=0.9`). It gives the
/// locale (the first tag), the language list (the tags) and the header (the
/// text).
fn parse_localization(s: &str) -> Option<(String, String, String)> {
    let mut tags = Vec::new();
    for (index, item) in s.split(',').enumerate() {
        let (tag, weight) = match item.split_once(';') {
            Some((tag, weight)) => (tag.trim(), Some(weight.trim())),
            None => (item.trim(), None),
        };
        // The locale needs its region; a later language may be bare (`en`).
        let tag_ok = is_language_tag(tag, index == 0);
        let weight_ok = weight.is_none_or(|w| {
            w.strip_prefix("q=")
                .and_then(|q| q.trim().parse::<f32>().ok())
                .is_some_and(|q| (0.0..=1.0).contains(&q))
        });
        if !tag_ok || !weight_ok {
            return None;
        }
        tags.push(tag);
    }
    let locale = (*tags.first()?).to_owned();
    Some((locale, tags.join(","), s.trim().to_owned()))
}

/// A literal time zone flag: an IANA name such as `Europe/Berlin` or
/// `America/Argentina/Buenos_Aires`.
fn is_zone_name(s: &str) -> bool {
    let mut segments = 0;
    for segment in s.split('/') {
        let well_formed = !segment.is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+'));
        if !well_formed {
            return false;
        }
        segments += 1;
    }
    segments >= 2 && s.starts_with(|c: char| c.is_ascii_alphabetic())
}

/// A literal fonts flag: a comma-separated list. One font alone is not one,
/// because any word would be a font; a single font goes in the payload's
/// `fingerprint.fonts` section.
fn parse_font_list(s: &str) -> Option<Vec<String>> {
    if !s.contains(',') {
        return None;
    }
    let fonts: Vec<String> = s
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    (!fonts.is_empty()).then_some(fonts)
}

fn parse_media_device_specs(s: &str) -> Vec<MediaDeviceSpec> {
    s.split('|')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() >= 3 {
                Some(MediaDeviceSpec {
                    kind: parts[0].trim().to_string(),
                    device_id: parts[1].trim().to_string(),
                    group_id: "grp-0".to_string(),
                    label: parts[2].trim().to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// The devices a literal media flag names: `2_mic_1_cam`, or a `|`-separated
/// list of `kind:id:label`.
fn parse_media_devices(s: &str) -> Option<MediaDevices> {
    if s.to_ascii_lowercase().starts_with("2_mic_1_cam") {
        return Some(MediaDevices::counts(2, 1, 1));
    }
    if s.contains(':') && s.contains('|') {
        let specs = parse_media_device_specs(s);
        let count = |kind: &str| specs.iter().filter(|d| d.kind == kind).count() as u32;
        return Some(MediaDevices {
            audio_inputs: count("audioinput").max(1),
            audio_outputs: count("audiooutput").max(1),
            video_inputs: count("videoinput").max(1),
            labelled: specs,
        });
    }
    None
}

/// The ports a literal ports flag names: `block:80,81` or `80,81`.
fn parse_ports(s: &str) -> Option<Vec<u16>> {
    let lower = s.trim().to_ascii_lowercase();
    let list = lower.strip_prefix("block:").unwrap_or(&lower);
    let ports: Vec<u16> = list
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    (!ports.is_empty()).then_some(ports)
}

fn parse_proxy_url(s: &str) -> Option<stealth_fp::ProxyConfig> {
    let parsed = url::Url::parse(s.trim()).ok()?;
    let scheme = parsed.scheme();
    if !matches!(scheme, "socks5" | "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_string();
    // `url::Url` preserves brackets around IPv6 hosts; remove them for downstream consumers.
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .map(String::from)
        .unwrap_or(host);
    let port = parsed.port().unwrap_or(match scheme {
        "socks5" => 1080,
        _ => 8080,
    });
    let username = Some(parsed.username().to_string()).filter(|s| !s.is_empty());
    let password = parsed.password().map(|p| p.to_string());
    Some(stealth_fp::ProxyConfig {
        r#type: scheme.to_string(),
        host,
        port,
        username,
        password,
        save_traffic: false,
    })
}

/// The addresses a literal WebRTC flag names: `public:1.2.3.4|local:10.0.0.2`.
fn parse_webrtc_addresses(s: &str) -> Option<WebRtcAddresses> {
    let mut addresses = WebRtcAddresses::default();
    for entry in s.split('|') {
        let mut parts = entry.splitn(2, ':');
        let key = parts.next().unwrap_or("").trim().to_lowercase();
        let value = parts.next().unwrap_or("").trim();
        let value = if value.starts_with('[') && value.ends_with(']') {
            &value[1..value.len() - 1]
        } else {
            value
        };
        match key.as_str() {
            "public" => addresses.public_ip = Some(value.to_string()),
            "local" => addresses.local_ip = Some(value.to_string()),
            _ => {}
        }
    }
    (addresses.public_ip.is_some() || addresses.local_ip.is_some()).then_some(addresses)
}

/// A seed that belongs to the profile, so what is drawn for it is the same on
/// every launch: the 64-bit FNV-1a hash of its folder and its name.
fn profile_seed(folder_id: &str, name: &str) -> u64 {
    folder_id
        .bytes()
        .chain(std::iter::once(0))
        .chain(name.bytes())
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

impl ProfileParams {
    /// Converts the external profile payload into a [`Fingerprint`] that can
    /// be injected into a browser session.
    ///
    /// The payload keeps its own format. Each flag is a keyword or a literal
    /// value of its key, and the nested sections (`navigator`, `screen`, ...)
    /// hold values. Every key reads the same words, and each setting ends up
    /// in one of the four states of a [`Spoof`](stealth_fp::Spoof). The words,
    /// the literal format of each key and the keys that differ are in one table,
    /// in `docs/tutorials/fingerprint_stealth.md`.
    ///
    /// An unknown keyword, and a literal that is not in the format of its key,
    /// is logged and read as Natural, never guessed at. A section value that is
    /// an empty string is a value the payload does not give.
    ///
    /// The profile's folder and name seed the draw, so a profile gets the same
    /// drawn values on every launch. The built-in policy is
    /// [`StealthPolicy::balanced`]; the flags `canvas_noise`, `audio_masking`,
    /// `quic_mode`, `ports_masking`, `disable_csp`, `grant_permissions` and
    /// `native_spoofing` change it, and `webrtc_masking` sets the WebRTC
    /// IP-handling policy.
    ///
    /// A setting that is asked for at random but cannot be drawn (a random
    /// locale next to a time zone this crate has no table row for) is logged
    /// and left natural.
    pub fn to_fingerprint(&self) -> Fingerprint {
        let builder = self.fingerprint_builder();
        match builder.clone().build() {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "a profile asks for a value that cannot be drawn; leaving the drawn settings natural"
                );
                builder.without_random().build().unwrap_or_default()
            }
        }
    }

    /// The builder of [`to_fingerprint`](Self::to_fingerprint).
    fn fingerprint_builder(&self) -> FingerprintBuilder {
        let flags = &self.parameters.flags;
        let section = &self.parameters.fingerprint;
        let canvas = flags
            .canvas_noise
            .as_deref()
            // A payload that names no canvas mode keeps the seeded noise.
            .map_or(CanvasNoise::Persistent, parse_canvas_noise);

        let mut cmd_params = std::collections::HashMap::new();
        for p in &section.cmd_params.params {
            cmd_params.insert(p.flag.clone(), p.value.clone());
        }

        let mut builder = Fingerprint::builder()
            .os_type(parse_os(&self.os_type))
            .seed(profile_seed(&self.folder_id, &self.name))
            .policy(StealthPolicy::balanced())
            .canvas_noise(canvas)
            .audio_noise(parse_audio_noise(&flags.audio_masking))
            .quic(parse_quic(&flags.quic_mode))
            .disable_csp(flags.disable_csp)
            .grant_permissions(flags.grant_permissions)
            .native_spoofing(flags.native_spoofing)
            .local_storage(self.parameters.storage.is_local)
            .save_service_worker(self.parameters.storage.save_service_worker)
            .custom_start_urls(self.parameters.custom_start_urls.clone())
            .cmd_params(cmd_params);

        // The Chromium major a drawn user agent and drawn Client Hints claim.
        // The Firefox and Safari personas have no Chromium major, and a
        // payload that names a Chromium major with one of them means the
        // engine of its own browser, which is not a Chrome version.
        if let Some(major) = self.core_version.filter(|_| self.claims_chromium()) {
            builder = builder.core_version(major);
        }
        builder = self.blocked_ports(builder);
        builder = self.navigator(builder);
        builder = self.localization(builder);
        builder = self.timezone(builder);
        builder = self.screen(builder);
        builder = self.graphics(builder);
        builder = self.fonts(builder);
        builder = self.media_devices(builder);
        builder = self.geolocation(builder);
        builder = self.proxy(builder);
        self.webrtc(builder)
    }

    /// Whether the payload's browser and system claim a Chromium user agent.
    fn claims_chromium(&self) -> bool {
        !matches!(
            self.browser_type.as_str(),
            "firefox" | "stealthfox" | "safari" | "mobile_safari"
        ) && parse_os(&self.os_type) != OsType::Ios
    }

    /// `ports_masking`: the natural words block nothing, every other word and a
    /// list block the ports.
    fn blocked_ports(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let text = &self.parameters.flags.ports_masking;
        let listed = &self.parameters.fingerprint.ports;
        let ports = match read(
            "ports_masking",
            text,
            // For ports, `block` is not the off state: it blocks the ports.
            &[
                ("block", Word::Random),
                ("block_all", Word::Random),
                ("stealth", Word::Random),
            ],
            |s| parse_ports(s).is_some(),
        ) {
            Reading::Word(Word::Natural | Word::Off) => Vec::new(),
            Reading::Word(_) => listed.clone(),
            Reading::Literal(list) => parse_ports(list).unwrap_or_default(),
        };
        builder.blocked_ports(ports)
    }

    fn navigator(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "navigator_masking";
        let reading = read(
            key,
            &self.parameters.flags.navigator_masking,
            &[
                ("desktop_chrome", Word::Random),
                ("mobile_safari", Word::Random),
            ],
            is_user_agent,
        );
        let word = word_of(reading);
        let nav = self.parameters.fingerprint.navigator.as_ref();
        let literal_agent = match reading {
            Reading::Literal(agent) => given_text(agent),
            Reading::Word(_) => None,
        };
        let touch = self.parameters.fingerprint.max_touch_points;
        let state = |given| settle(key, word, given, true, false);
        let given_only = |given: Option<String>| match word {
            Word::Natural | Word::Off => Spoof::Natural,
            Word::Auto | Word::Random => given.map_or(Spoof::Natural, Spoof::Value),
        };
        builder
            .user_agent(state(
                nav.and_then(|n| given_text(&n.user_agent))
                    .or(literal_agent),
            ))
            .platform(state(nav.and_then(|n| given_text(&n.platform))))
            .hardware_concurrency(settle(
                key,
                word,
                nav.and_then(|n| n.hardware_concurrency),
                true,
                false,
            ))
            .device_memory(settle(
                key,
                word,
                nav.and_then(|n| n.device_memory),
                true,
                false,
            ))
            .max_touch_points(settle(key, word, touch, true, false))
            .vendor(state(None))
            .oscpu(given_only(nav.and_then(|n| given_text(&n.os_cpu))))
    }

    fn localization(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "localization_masking";
        let reading = read(key, &self.parameters.flags.localization_masking, &[], |s| {
            parse_localization(s).is_some()
        });
        let word = word_of(reading);
        let loc = self.parameters.fingerprint.localization.as_ref();
        let literal = match reading {
            Reading::Literal(text) => parse_localization(text),
            Reading::Word(_) => None,
        };
        let state = |given| settle(key, word, given, true, false);
        builder
            .locale(state(loc.and_then(|l| given_text(&l.locale)).or_else(
                || literal.as_ref().map(|(locale, ..)| locale.clone()),
            )))
            .languages(state(loc.and_then(|l| given_text(&l.languages)).or_else(
                || literal.as_ref().map(|(_, languages, _)| languages.clone()),
            )))
            .accept_languages(state(
                loc.and_then(|l| given_text(&l.accept_languages))
                    .or_else(|| literal.as_ref().map(|(.., header)| header.clone())),
            ))
    }

    fn timezone(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "timezone_masking";
        let reading = read(
            key,
            &self.parameters.flags.timezone_masking,
            &[],
            is_zone_name,
        );
        let given = self
            .parameters
            .fingerprint
            .timezone
            .as_ref()
            .and_then(|t| given_text(&t.zone))
            .or_else(|| match reading {
                Reading::Literal(zone) => given_text(zone),
                Reading::Word(_) => None,
            });
        builder.timezone(settle(key, word_of(reading), given, true, false))
    }

    fn screen(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "screen_masking";
        let reading = read(key, &self.parameters.flags.screen_masking, &[], |s| {
            parse_dimensions(s).is_some()
        });
        let word = word_of(reading);
        let section = self.parameters.fingerprint.screen.as_ref();
        let literal = match reading {
            Reading::Literal(text) => parse_dimensions(text),
            Reading::Word(_) => None,
        };
        let size = section
            .map(|s| ScreenSize::new(s.width, s.height))
            .or(literal.map(|(w, h, _)| ScreenSize::new(w, h)));
        builder
            .screen_spoof(settle(key, word, size, true, false))
            .pixel_ratio(settle(
                key,
                word,
                section.map(|s| s.pixel_ratio),
                true,
                false,
            ))
            .color_depth(settle(key, word, literal.map(|(_, _, d)| d), true, false))
    }

    fn graphics(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "graphics_masking";
        let reading = read(key, &self.parameters.flags.graphics_masking, &[], |s| {
            parse_vendor_renderer(s).is_some()
        });
        let from_section = self
            .parameters
            .fingerprint
            .graphic
            .as_ref()
            .and_then(|gpu| {
                let webgl = match (given_text(&gpu.vendor), given_text(&gpu.renderer)) {
                    (Some(vendor), Some(renderer)) => WebGl::new(vendor, renderer),
                    (None, Some(renderer)) => WebGl::renderer_only(renderer),
                    (Some(vendor), None) => WebGl::vendor_only(vendor),
                    (None, None) => return None,
                };
                Some(
                    match (given_text(&gpu.vendor_id), given_text(&gpu.renderer_id)) {
                        (None, None) => webgl,
                        (vendor_id, renderer_id) => webgl.ids(
                            vendor_id.unwrap_or_default(),
                            renderer_id.unwrap_or_default(),
                        ),
                    },
                )
            });
        let given = from_section.or_else(|| match reading {
            Reading::Literal(text) => {
                parse_vendor_renderer(text).map(|(vendor, renderer)| WebGl::new(vendor, renderer))
            }
            Reading::Word(_) => None,
        });
        builder.webgl(settle(key, word_of(reading), given, true, false))
    }

    fn fonts(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "fonts_masking";
        let reading = read(
            key,
            &self.parameters.flags.fonts_masking,
            &[("system_only", Word::Random), ("blacklist", Word::Random)],
            |s| parse_font_list(s).is_some(),
        );
        let listed: Vec<String> = self
            .parameters
            .fingerprint
            .fonts
            .iter()
            .filter_map(|font| given_text(font))
            .collect();
        let given = if listed.is_empty() {
            match reading {
                Reading::Literal(text) => parse_font_list(text),
                Reading::Word(_) => None,
            }
        } else {
            Some(listed)
        };
        builder.fonts(settle(key, word_of(reading), given, true, false))
    }

    fn media_devices(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "media_devices_masking";
        let reading = read(
            key,
            &self.parameters.flags.media_devices_masking,
            &[],
            |s| parse_media_devices(s).is_some(),
        );
        let given = match self.parameters.fingerprint.media_devices.as_ref() {
            Some(media) => Some(MediaDevices::counts(
                media.audio_inputs,
                media.audio_outputs,
                media.video_inputs,
            )),
            None => match reading {
                Reading::Literal(text) => parse_media_devices(text),
                Reading::Word(_) => None,
            },
        };
        builder.media_devices(settle(key, word_of(reading), given, true, true))
    }

    /// `geolocation_masking` and `geolocation_popup`. The popup flag was how
    /// the old model said what a request is answered with; now the state of the
    /// setting says it: a position answers, `Off` denies, `Natural` asks. So
    /// `block` denies and `prompt` asks, whatever position the payload holds.
    fn geolocation(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "geolocation_masking";
        let reading = read(key, &self.parameters.flags.geolocation_masking, &[], |s| {
            parse_coordinates(s).is_some()
        });
        let given = match self.parameters.fingerprint.geolocation.as_ref() {
            Some(geo) => {
                let place = Geolocation::new(geo.latitude, geo.longitude).altitude(geo.altitude);
                Some(if geo.accuracy > 0.0 {
                    place.accuracy(geo.accuracy)
                } else {
                    place
                })
            }
            None => match reading {
                Reading::Literal(text) => parse_coordinates(text)
                    .map(|(lat, lon, alt)| Geolocation::new(lat, lon).altitude(alt).accuracy(20.0)),
                Reading::Word(_) => None,
            },
        };
        let state = settle(key, word_of(reading), given, true, true);
        let popup = read(
            "geolocation_popup",
            &self.parameters.flags.geolocation_popup,
            &[("allow", Word::Auto), ("prompt", Word::Natural)],
            |_| false,
        );
        builder.geolocation(match popup {
            Reading::Word(Word::Off) => Spoof::Off,
            Reading::Word(Word::Natural) => Spoof::Natural,
            _ => state,
        })
    }

    /// `proxy_masking`. A proxy cannot be drawn, so the words that draw for
    /// the other keys (`mask`, `custom`, `socks5`, ...) use the proxy the
    /// payload holds, as `auto` does. The words that turn a key off read as
    /// natural, and are logged: no proxy.
    fn proxy(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "proxy_masking";
        let reading = read(
            key,
            &self.parameters.flags.proxy_masking,
            &[
                ("socks5", Word::Random),
                ("http", Word::Random),
                ("https", Word::Random),
                // No proxy is the browser's own connection.
                ("direct", Word::Natural),
            ],
            |s| parse_proxy_url(s).is_some(),
        );
        let from_payload = self
            .parameters
            .proxy
            .as_ref()
            .map(|p| stealth_fp::ProxyConfig {
                r#type: p.proxy_type.clone(),
                host: p.host.clone(),
                port: p.port,
                username: Some(p.username.clone()).filter(|s| !s.is_empty()),
                password: Some(p.password.clone()).filter(|s| !s.is_empty()),
                save_traffic: p.save_traffic,
            });
        let given = match reading {
            Reading::Literal(url) => parse_proxy_url(url).or(from_payload),
            Reading::Word(_) => from_payload,
        }
        .filter(|proxy| !proxy.host.is_empty());
        // `settle` reads an off word as natural and logs it: a proxy has no
        // off state.
        builder.proxy(settle(key, word_of(reading), given, false, false))
    }

    /// `webrtc_masking`: the setting and the IP-handling policy.
    ///
    /// The policy keeps WebRTC from leaking the real address, so every word
    /// but the natural ones passes `disable_non_proxied_udp`; a literal address
    /// passes `default_public_and_private_interfaces`, and `public_only` passes
    /// `default_public_interface_only`.
    fn webrtc(&self, builder: FingerprintBuilder) -> FingerprintBuilder {
        let key = "webrtc_masking";
        let reading = read(
            key,
            &self.parameters.flags.webrtc_masking,
            &[("public_only", Word::Auto)],
            |s| parse_webrtc_addresses(s).is_some(),
        );
        let word = word_of(reading);
        let text = self
            .parameters
            .flags
            .webrtc_masking
            .trim()
            .to_ascii_lowercase();
        let from_section = self
            .parameters
            .fingerprint
            .webrtc
            .as_ref()
            .filter(|w| !w.public_ip.is_empty())
            .map(|w| WebRtcAddresses {
                public_ip: Some(w.public_ip.clone()),
                local_ip: None,
            });
        let given = match reading {
            Reading::Literal(text) => parse_webrtc_addresses(text),
            Reading::Word(_) => None,
        }
        .or(from_section);
        let policy = match (word, &given) {
            (Word::Natural, _) => None,
            _ if text == "public_only" => Some(WebRtcPolicy::PublicInterfaceOnly),
            (_, Some(_)) => Some(WebRtcPolicy::PublicAndPrivateInterfaces),
            _ => Some(WebRtcPolicy::DisableNonProxiedUdp),
        };
        builder
            .webrtc(settle(key, word, given, true, true))
            .webrtc_policy(policy)
    }

    /// Translates the external profile payload into a `BrowserConfig` that
    /// `seleniumbase-rs` can launch.
    ///
    /// Not every anti-detect flag has a direct Selenium/Chrome capability
    /// equivalent. The conversion applies the values that do map cleanly:
    /// browser type, user agent, locale, proxy, and extra command-line flags.
    pub fn to_browser_config(&self, container_url: impl Into<String>) -> BrowserConfig {
        let mode = match self.automation.to_lowercase().as_str() {
            "playwright" | "puppeteer" => DriverMode::Cdp,
            "selenium" => DriverMode::WebDriver,
            _ if matches!(self.browser_type.as_str(), "firefox" | "stealthfox") => {
                DriverMode::WebDriver
            }
            _ => DriverMode::Uc,
        };
        let custom_start_urls: Vec<String> =
            self.start_urls().into_iter().map(str::to_owned).collect();
        // What the browser is launched with is what the fingerprint holds, so a
        // `natural` flag or an off word launches nothing of that setting, and a
        // drawn value is the one the evasions claim.
        let fingerprint = self.to_fingerprint();
        let mut config = BrowserConfig {
            webdriver_url: container_url.into(),
            cdp_url: None,
            cdp_discovery: false,
            browser: self.browser(),
            headless: self.is_headless,
            mode,
            user_agent: fingerprint.user_agent().value().cloned(),
            locale: fingerprint.locale().value().cloned(),
            proxy: fingerprint
                .proxy()
                .value()
                .map(stealth_fp::ProxyConfig::to_url),
            proxy_pac_url: None,
            user_data_dir: self.user_data_dir(),
            extension_dir: None,
            start_page: custom_start_urls.first().cloned(),
            reuse_session: false,
            mobile: self.os_type == "android" || self.os_type == "ios",
            threads: None,
            ad_block: fingerprint
                .proxy()
                .value()
                .is_some_and(|proxy| proxy.save_traffic),
            auto_start_driver: true,
            extra_args: Vec::new(),
            fingerprint: Some(fingerprint),
            browser_binary_path: None,
            remote_webdriver_timeout_seconds: None,
            launch: Default::default(),
        };

        for extra in self.extra_args() {
            config.extra_args.push(extra);
        }

        config
    }

    /// Returns the `Browser` variant inferred from `browser_type`.
    pub fn browser(&self) -> Browser {
        match self.browser_type.as_str() {
            "firefox" | "stealthfox" => Browser::Firefox,
            "safari" | "mobile_safari" => Browser::Chrome,
            _ => Browser::Chrome,
        }
    }

    /// The user agent of the payload's fingerprint, when it has one.
    ///
    /// It is the one [`to_fingerprint`](Self::to_fingerprint) builds, so the
    /// `navigator_masking` flag decides: `natural` gives none, and a drawn
    /// user agent is the drawn one.
    pub fn user_agent(&self) -> Option<String> {
        self.to_fingerprint().user_agent().value().cloned()
    }

    /// The locale of the payload's fingerprint, when it has one (see
    /// [`user_agent`](Self::user_agent)).
    pub fn locale(&self) -> Option<String> {
        self.to_fingerprint().locale().value().cloned()
    }

    /// The proxy URL of the payload's fingerprint, when it has one (see
    /// [`user_agent`](Self::user_agent)): none under `proxy_masking: natural`.
    pub fn proxy_string(&self) -> Option<String> {
        self.to_fingerprint()
            .proxy()
            .value()
            .map(stealth_fp::ProxyConfig::to_url)
    }

    /// Per-profile persistent data directory when `storage.is_local` is true.
    ///
    /// The folder is named after `folder_id` with anything but letters,
    /// digits, `-` and `_` replaced, so an id from outside the program cannot
    /// point the directory somewhere else.
    pub fn user_data_dir(&self) -> Option<String> {
        if !self.parameters.storage.is_local {
            return None;
        }
        let folder: String = self
            .folder_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let folder = if folder.is_empty() {
            "default"
        } else {
            &folder
        };
        Some(format!("./profile-data/{folder}"))
    }

    /// The pages to open at start-up: at most five, and only web pages.
    ///
    /// A payload can come from outside the program, so `file:` and other
    /// schemes that reach the machine itself are left out.
    pub fn start_urls(&self) -> Vec<&str> {
        self.parameters
            .custom_start_urls
            .iter()
            .map(String::as_str)
            .filter(|url| {
                let url = url.trim_start().to_ascii_lowercase();
                let web = url.starts_with("http://")
                    || url.starts_with("https://")
                    || url == "about:blank";
                if !web {
                    tracing::warn!("ignoring a start page that is not a web page");
                }
                web
            })
            .take(5)
            .collect()
    }

    /// Additional Chromium command-line flags parsed from `cmd_params`.
    ///
    /// Switches that run programs, load code, open remote control or move
    /// traffic are left out; see
    /// [`permitted_switch`](crate::stealth::evasions::permitted_switch).
    pub fn extra_args(&self) -> Vec<String> {
        crate::stealth::evasions::switches_to_args(
            self.parameters
                .fingerprint
                .cmd_params
                .params
                .iter()
                .map(|p| (p.flag.as_str(), p.value.as_str())),
        )
    }

    /// Applies runtime fingerprint overrides to an active `BaseCase`.
    ///
    /// This covers screen size and geolocation, which must be set after the
    /// browser session is alive. Both are the fingerprint's own: a `natural`
    /// flag, or `block` or `prompt` for the position, sets nothing.
    pub async fn apply_runtime_overrides(
        &self,
        sb: &mut crate::BaseCase,
    ) -> Result<(), SeleniumBaseError> {
        let fingerprint = self.to_fingerprint();
        if let Some(screen) = fingerprint.screen().value() {
            sb.set_window_size(screen.width, screen.height).await?;
        }
        if let Some(geo) = fingerprint.geolocation().value() {
            sb.set_geolocation(geo.latitude, geo.longitude, geo.accuracy.unwrap_or(0.0))
                .await?;
        }
        for url in self.start_urls().into_iter().skip(1) {
            sb.open(url).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::Dimension;
    use serde_json::json;

    #[test]
    fn parse_profile_payload() {
        let raw = json!({
            "name": "Profile_name",
            "browser_type": "chromium",
            "folder_id": "4500dd84-d8c5-4450-b2df-1c64daed8bad",
            "core_version": 124,
            "auto_update_core": false,
            "os_type": "windows",
            "times": 1,
            "notes": "asd",
            "parameters": {
                "flags": {
                    "webrtc_masking": "custom",
                    "startup_behavior": "custom"
                },
                "storage": {
                    "is_local": false,
                    "save_service_worker": false
                },
                "fingerprint": {
                    "navigator": {
                        "hardware_concurrency": 8,
                        "platform": "Win32",
                        "user_agent": "Mozilla/5.0",
                        "os_cpu": ""
                    },
                    "screen": { "width": 1920, "height": 1200, "pixel_ratio": 1 }
                },
                "proxy": {
                    "type": "http",
                    "host": "proxyhost.com",
                    "port": 8081,
                    "username": "user",
                    "password": "pass",
                    "save_traffic": false
                },
                "custom_start_urls": ["https://example.com"]
            }
        });
        let params: ProfileParams = serde_json::from_value(raw).unwrap();
        assert_eq!(params.name, "Profile_name");
        assert_eq!(params.browser_type, "chromium");
        assert_eq!(params.core_version, Some(124));
        assert!(!params.auto_update_core);
        assert_eq!(
            params.parameters.proxy.as_ref().unwrap().host,
            "proxyhost.com"
        );
        assert_eq!(params.user_agent().unwrap(), "Mozilla/5.0");
        assert_eq!(
            params.proxy_string().unwrap(),
            "http://user:pass@proxyhost.com:8081"
        );
    }

    #[test]
    fn defaults_are_applied() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Minimal",
            "parameters": {}
        }))
        .unwrap();
        assert_eq!(params.browser_type, "chromium");
        assert_eq!(params.os_type, "windows");
        assert_eq!(params.parameters.flags.webrtc_masking, "auto");
        assert!(params.parameters.storage.is_local);
        assert_eq!(params.times, 1);
    }

    #[test]
    fn a_folder_id_cannot_leave_the_profile_directory() {
        for (id, expected) in [
            (
                "4500dd84-d8c5-4450-b2df-1c64daed8bad",
                "4500dd84-d8c5-4450-b2df-1c64daed8bad",
            ),
            ("../../etc", "______etc"),
            ("a/b\\c", "a_b_c"),
            ("..", "__"),
            ("", "default"),
        ] {
            let params: ProfileParams = serde_json::from_value(json!({
                "name": "P", "folder_id": id, "parameters": {}
            }))
            .unwrap();
            assert_eq!(
                params.user_data_dir().as_deref(),
                Some(format!("./profile-data/{expected}").as_str()),
                "folder id {id:?}"
            );
        }
    }

    #[test]
    fn switches_that_run_programs_are_not_passed_to_chrome() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "P",
            "parameters": { "fingerprint": { "cmd_params": { "params": [
                { "flag": "renderer-cmd-prefix", "value": "/bin/sh -c evil" },
                { "flag": "load-extension", "value": "/tmp/ext" },
                { "flag": "lang", "value": "fr-FR" },
                { "flag": "disable-quic" }
            ] } } }
        }))
        .unwrap();
        assert_eq!(params.extra_args(), ["--disable-quic", "--lang=fr-FR"]);
        let config = params.to_browser_config("http://localhost:4444");
        assert!(!config.extra_args.iter().any(|a| a.contains("cmd-prefix")));
    }

    #[test]
    fn only_web_pages_are_opened_at_start_up() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "P",
            "parameters": { "custom_start_urls": [
                "file:///etc/passwd", "https://a.example", "javascript:alert(1)",
                "http://b.example", "about:blank", "https://c.example",
                "https://d.example", "https://e.example"
            ] }
        }))
        .unwrap();
        assert_eq!(
            params.start_urls(),
            [
                "https://a.example",
                "http://b.example",
                "about:blank",
                "https://c.example",
                "https://d.example"
            ]
        );
        let config = params.to_browser_config("http://localhost:4444");
        assert_eq!(config.start_page.as_deref(), Some("https://a.example"));
    }

    #[test]
    fn debug_output_does_not_show_the_proxy_password() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "P",
            "parameters": { "proxy": {
                "type": "http", "host": "proxy.example", "port": 8081,
                "username": "user", "password": "hunter2"
            } }
        }))
        .unwrap();
        let shown = format!("{params:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("proxy.example"), "{shown}");
        // The fingerprint's own proxy type hides it too.
        let fp = stealth_fp::ProxyConfig {
            r#type: "http".into(),
            host: "h".into(),
            port: 1,
            username: Some("u".into()),
            password: Some("hunter2".into()),
            save_traffic: false,
        };
        assert!(!format!("{fp:?}").contains("hunter2"));
    }

    #[test]
    fn legacy_browser_type_strings_are_accepted() {
        let chromium: ProfileParams = serde_json::from_value(json!({
            "name": "Legacy Chromium",
            "browser_type": "mimic",
            "parameters": {}
        }))
        .unwrap();
        assert_eq!(chromium.browser(), Browser::Chrome);

        let firefox: ProfileParams = serde_json::from_value(json!({
            "name": "Legacy Firefox",
            "browser_type": "stealthfox",
            "parameters": {}
        }))
        .unwrap();
        assert_eq!(firefox.browser(), Browser::Firefox);
    }

    #[test]
    fn profile_automation_and_headless_map_to_browser_config() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "HeadlessPlaywright",
            "automation": "playwright",
            "is_headless": true,
            "parameters": {}
        }))
        .unwrap();
        let config = params.to_browser_config("http://localhost:4444");
        assert!(config.headless);
        assert_eq!(config.mode, DriverMode::Cdp);
    }

    #[test]
    fn profile_navigator_extras_are_applied() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Extras",
            "os_type": "android",
            "parameters": {
                "fingerprint": {
                    "navigator": {
                        "hardware_concurrency": 8,
                        "device_memory": 4.0,
                        "user_agent": "Mozilla/5.0 (Linux; Android 10)",
                        "platform": "Linux armv8l",
                        "os_cpu": "Linux armv8l"
                    },
                    "max_touch_points": 5
                }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.hardware_concurrency().value(), Some(&8));
        assert_eq!(fp.device_memory().value(), Some(&4.0));
        assert_eq!(fp.oscpu().as_str(), Some("Linux armv8l"));
        assert_eq!(fp.max_touch_points().value(), Some(&5));
    }

    #[test]
    fn profile_webgl_ids_are_applied() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "WebGLIds",
            "parameters": {
                "fingerprint": {
                    "graphic": {
                        "vendor": "NVIDIA",
                        "renderer": "GeForce RTX",
                        "vendor_id": "0x10de",
                        "renderer_id": "0x1f91"
                    }
                }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        let gl = fp.webgl().value().expect("a webgl value");
        assert_eq!(gl.vendor.as_deref(), Some("NVIDIA"));
        assert_eq!(gl.renderer.as_deref(), Some("GeForce RTX"));
        assert_eq!(gl.vendor_id.as_deref(), Some("0x10de"));
        assert_eq!(gl.renderer_id.as_deref(), Some("0x1f91"));
    }

    #[test]
    fn custom_start_urls_are_capped_at_five() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Urls",
            "parameters": {
                "custom_start_urls": [
                    "https://a.example", "https://b.example", "https://c.example",
                    "https://d.example", "https://e.example", "https://f.example"
                ]
            }
        }))
        .unwrap();
        assert_eq!(params.start_urls().len(), 5);
        let config = params.to_browser_config("http://localhost:4444");
        assert_eq!(config.start_page, Some("https://a.example".to_string()));
    }

    /// The fingerprint of a payload with `flags` and nothing else.
    fn fingerprint_of(flags: serde_json::Value) -> Fingerprint {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Flags",
            "parameters": { "flags": flags }
        }))
        .unwrap();
        params.to_fingerprint()
    }

    #[test]
    fn concrete_screen_masking_parses_dimensions() {
        let fp = fingerprint_of(json!({ "screen_masking": "1920x1080x24" }));
        assert_eq!(fp.screen().value(), Some(&ScreenSize::new(1920, 1080)));
        assert_eq!(fp.color_depth().value(), Some(&24));
    }

    #[test]
    fn concrete_geolocation_masking_parses_coordinates() {
        let fp = fingerprint_of(json!({ "geolocation_masking": "40.7128,-74.0060,10.5" }));
        let place = fp.geolocation().value().expect("a position");
        assert_eq!(place.latitude, 40.7128);
        assert_eq!(place.longitude, -74.0060);
        assert_eq!(place.altitude, Some(10.5));
    }

    #[test]
    fn concrete_graphics_masking_parses_vendor_renderer() {
        let fp = fingerprint_of(
            json!({ "graphics_masking": "Intel Inc.~Intel(R) Iris(R) Xe Graphics" }),
        );
        let gl = fp.webgl().value().expect("a webgl value");
        assert_eq!(gl.vendor.as_deref(), Some("Intel Inc."));
        assert_eq!(gl.renderer.as_deref(), Some("Intel(R) Iris(R) Xe Graphics"));
    }

    #[test]
    fn concrete_navigator_masking_parses_user_agent() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
        let fp = fingerprint_of(json!({ "navigator_masking": ua }));
        assert_eq!(fp.user_agent().as_str(), Some(ua));
    }

    #[test]
    fn concrete_localization_and_timezone_masking_parses_values() {
        let fp = fingerprint_of(json!({
            "localization_masking": "en-US,en;q=0.9",
            "timezone_masking": "America/New_York"
        }));
        // The locale is the first tag, the list is the tags, the header is the text.
        assert_eq!(fp.locale().as_str(), Some("en-US"));
        assert_eq!(fp.languages().as_str(), Some("en-US,en"));
        assert_eq!(fp.accept_languages().as_str(), Some("en-US,en;q=0.9"));
        assert_eq!(fp.timezone().as_str(), Some("America/New_York"));
        let tag = fingerprint_of(json!({ "localization_masking": "zh-Hant-TW" }));
        assert_eq!(tag.locale().as_str(), Some("zh-Hant-TW"));
        assert_eq!(tag.languages().as_str(), Some("zh-Hant-TW"));
    }

    #[test]
    fn a_literal_has_the_shape_of_its_key_or_it_is_no_literal() {
        // Each of these used to be taken for a literal because of one character.
        let fp = fingerprint_of(json!({
            "localization_masking": "no-such thing",
            "timezone_masking": "a/b c",
            "navigator_masking": "Chrome/133 is nice",
            "graphics_masking": "~",
            "fonts_masking": "Arial"
        }));
        for (key, natural) in natural_states(&fp) {
            assert!(natural, "{key}");
        }
        for literal in [
            "en",
            "en-",
            "e1-US",
            "en-US;q=0.9,",
            "en-US,en;q=x",
            "en-US,,en",
        ] {
            assert!(parse_localization(literal).is_none(), "{literal}");
        }
        for zone in ["Europe", "/Berlin", "Europe/", "Europe/Ber lin", "1/2"] {
            assert!(!is_zone_name(zone), "{zone}");
        }
        assert!(is_zone_name("America/Argentina/Buenos_Aires"));
        assert!(is_zone_name("Etc/GMT+5"));
        assert!(!is_user_agent("Mozilla/") && !is_user_agent("Mozilla/x (a)"));
        assert!(
            parse_vendor_renderer("Intel~").is_none() && parse_vendor_renderer("~Iris").is_none()
        );
        assert!(parse_font_list("Arial").is_none());
        assert_eq!(parse_font_list("Arial, Calibri").map(|f| f.len()), Some(2));
    }

    #[test]
    fn concrete_fonts_masking_parses_list() {
        let fp = fingerprint_of(json!({ "fonts_masking": "Arial,Calibri,Consolas" }));
        assert_eq!(
            fp.fonts().value(),
            Some(&vec![
                "Arial".to_owned(),
                "Calibri".to_owned(),
                "Consolas".to_owned()
            ])
        );
    }

    #[test]
    fn concrete_media_devices_masking_parses_specs() {
        let fp = fingerprint_of(json!({
            "media_devices_masking": "audioinput:default:Mic1|videoinput:default:Cam1"
        }));
        let devices = fp.media_devices().value().expect("devices");
        assert_eq!(devices.audio_inputs, 1);
        assert_eq!(devices.video_inputs, 1);
        assert_eq!(devices.labelled.len(), 2);
        assert_eq!(devices.labelled[0].label, "Mic1");
        assert_eq!(devices.labelled[1].kind, "videoinput");
    }

    #[test]
    fn concrete_ports_masking_parses_block_list() {
        let fp = fingerprint_of(json!({ "ports_masking": "block:80,443,3000,8080,9222" }));
        assert_eq!(fp.policy().blocked_ports, vec![80, 443, 3000, 8080, 9222]);
        assert!(fp.is_masked(Dimension::Ports));
    }

    #[test]
    fn concrete_proxy_masking_parses_url() {
        let fp = fingerprint_of(json!({
            "proxy_masking": "socks5://user123:pass456@192.168.1.50:1080"
        }));
        let proxy = fp.proxy().value().expect("a proxy");
        assert_eq!(proxy.r#type, "socks5");
        assert_eq!(proxy.host, "192.168.1.50");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username, Some("user123".to_string()));
        assert_eq!(proxy.password, Some("pass456".to_string()));
    }

    #[test]
    fn concrete_webrtc_masking_parses_ips() {
        let fp = fingerprint_of(json!({
            "webrtc_masking": "public:172.56.21.89|local:10.0.0.5"
        }));
        let addresses = fp.webrtc().value().expect("addresses");
        assert_eq!(addresses.public_ip.as_deref(), Some("172.56.21.89"));
        assert_eq!(addresses.local_ip.as_deref(), Some("10.0.0.5"));
        assert_eq!(
            fp.policy().webrtc_policy,
            Some(WebRtcPolicy::PublicAndPrivateInterfaces)
        );
    }

    #[test]
    fn concrete_webrtc_masking_parses_ipv6_ips() {
        let fp = fingerprint_of(json!({
            "webrtc_masking": "public:[2001:db8::1]|local:[fe80::1]"
        }));
        let addresses = fp.webrtc().value().expect("addresses");
        assert_eq!(addresses.public_ip.as_deref(), Some("2001:db8::1"));
        assert_eq!(addresses.local_ip.as_deref(), Some("fe80::1"));
    }

    #[test]
    fn concrete_proxy_masking_parses_ipv6_url() {
        let fp = fingerprint_of(json!({ "proxy_masking": "socks5://[2001:db8::1]:1080" }));
        let proxy = fp.proxy().value().expect("a proxy");
        assert_eq!(proxy.r#type, "socks5");
        assert_eq!(proxy.host, "2001:db8::1");
        assert_eq!(proxy.port, 1080);
    }

    #[test]
    fn flag_keywords_map_to_the_policy() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Keywords",
            "parameters": {
                "flags": {
                    "audio_masking": "noise",
                    "graphics_noise": "medium",
                    "canvas_noise": "random",
                    "quic_mode": "force_http2",
                    "ports_masking": "block_all"
                },
                "fingerprint": { "ports": [9222] }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.policy().audio_noise, AudioNoise::Persistent);
        assert_eq!(fp.policy().canvas_noise, CanvasNoise::Random);
        assert_eq!(fp.policy().quic, QuicMode::ForceHttp2);
        assert_eq!(fp.policy().blocked_ports, vec![9222]);
    }

    /// A payload with a value for every dimension a payload can carry, and
    /// `flags` as its flags.
    fn payload_with_values(flags: serde_json::Value) -> ProfileParams {
        serde_json::from_value(json!({
            "name": "Values",
            "parameters": {
                "flags": flags,
                "fingerprint": {
                    "navigator": {
                        "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
                        "platform": "Win32"
                    },
                    "localization": {
                        "languages": "de-DE,de", "locale": "de-DE", "accept_languages": "de-DE,de;q=0.9"
                    },
                    "timezone": { "zone": "Europe/Berlin" },
                    "graphic": { "vendor": "Intel Inc.", "renderer": "Intel Iris" },
                    "media_devices": { "audio_inputs": 1, "audio_outputs": 1, "video_inputs": 1 },
                    "screen": { "width": 1440, "height": 900, "pixel_ratio": 2 },
                    "geolocation": { "latitude": 52.52, "longitude": 13.4, "accuracy": 20 },
                    "fonts": ["Arial"],
                    "ports": [9222]
                },
                "proxy": { "type": "http", "host": "proxy.example", "port": 8080 }
            }
        }))
        .unwrap()
    }

    /// The dimensions that carry a value of a payload.
    const VALUE_DIMENSIONS: [Dimension; 9] = [
        Dimension::Navigator,
        Dimension::Localization,
        Dimension::Timezone,
        Dimension::Graphics,
        Dimension::MediaDevices,
        Dimension::Screen,
        Dimension::Geolocation,
        Dimension::Fonts,
        Dimension::Proxy,
    ];

    #[test]
    fn the_defaults_leave_each_dimension_to_its_value() {
        let flags = ProfileFlags::default();
        for (name, word) in [
            ("webrtc_masking", &flags.webrtc_masking),
            ("proxy_masking", &flags.proxy_masking),
            ("geolocation_popup", &flags.geolocation_popup),
            ("audio_masking", &flags.audio_masking),
            ("navigator_masking", &flags.navigator_masking),
            ("localization_masking", &flags.localization_masking),
            ("timezone_masking", &flags.timezone_masking),
            ("graphics_masking", &flags.graphics_masking),
            ("fonts_masking", &flags.fonts_masking),
            ("media_devices_masking", &flags.media_devices_masking),
            ("screen_masking", &flags.screen_masking),
            ("geolocation_masking", &flags.geolocation_masking),
        ] {
            assert_eq!(word, "auto", "{name}");
        }
    }

    #[test]
    fn a_value_in_the_payload_is_used_without_any_flag_for_it() {
        let fp = payload_with_values(json!({})).to_fingerprint();

        for dimension in VALUE_DIMENSIONS {
            assert!(fp.is_masked(dimension), "{dimension:?} is off");
        }
        assert!(
            fp.geolocation().is_value(),
            "a position answers the request"
        );
        assert_eq!(fp.timezone().as_str(), Some("Europe/Berlin"));
        assert_eq!(
            fp.proxy().value().map(|p| p.host.as_str()),
            Some("proxy.example")
        );
        assert_eq!(
            fp.policy().blocked_ports,
            vec![9222],
            "ports_masking defaults to mask"
        );
    }

    #[test]
    fn an_explicit_natural_flag_stays_off_whatever_values_the_payload_holds() {
        let fp = payload_with_values(json!({
            "navigator_masking": "natural",
            "localization_masking": "native",
            "timezone_masking": "natural",
            "graphics_masking": "pass_through",
            "media_devices_masking": "natural",
            "screen_masking": "natural",
            "geolocation_masking": "natural",
            "fonts_masking": "whitelist",
            "proxy_masking": "natural"
        }))
        .to_fingerprint();

        for dimension in VALUE_DIMENSIONS {
            assert!(!fp.is_masked(dimension), "{dimension:?} is on");
        }
    }

    #[test]
    fn an_off_word_turns_the_apis_that_can_be_off_off_and_leaves_the_others_natural() {
        for word in ["off", "disabled", "block", "empty"] {
            let fp = payload_with_values(json!({
                "geolocation_masking": word,
                "webrtc_masking": word,
                "media_devices_masking": word,
                "timezone_masking": word,
                "screen_masking": word,
                "proxy_masking": word
            }))
            .to_fingerprint();

            assert!(fp.geolocation().is_off(), "{word}");
            assert!(fp.webrtc().is_off(), "{word}");
            assert!(fp.media_devices().is_off(), "{word}");
            // No off state for these: natural, whatever the payload holds.
            assert!(fp.timezone().is_natural(), "{word}");
            assert!(
                fp.screen().is_natural() && fp.pixel_ratio().is_natural(),
                "{word}"
            );
            assert!(fp.proxy().is_natural(), "{word}");
        }
    }

    #[test]
    fn the_word_auto_leaves_a_dimension_to_its_value() {
        let fp = payload_with_values(json!({
            "geolocation_masking": "auto",
            "localization_masking": "auto",
            "timezone_masking": "auto",
            "proxy_masking": "auto",
            "geolocation_popup": "auto"
        }))
        .to_fingerprint();
        assert!(fp.timezone().is_value() && fp.proxy().is_value());
        assert!(fp.geolocation().is_value() && fp.locale().is_value());
        // And with no value, `auto` is the browser's own.
        let bare = fingerprint_of(json!({ "timezone_masking": "auto" }));
        assert!(bare.timezone().is_natural());
    }

    #[test]
    fn a_geolocation_popup_flag_denies_or_asks_whatever_position_the_payload_holds() {
        let popup =
            |word: &str| payload_with_values(json!({ "geolocation_popup": word })).to_fingerprint();
        assert!(popup("block").geolocation().is_off());
        assert!(popup("prompt").geolocation().is_natural());
        assert!(popup("allow").geolocation().is_value());
        assert!(popup("auto").geolocation().is_value());
    }

    #[test]
    fn blocking_webrtc_still_passes_the_policy_that_stops_the_leak() {
        for word in ["disabled", "block"] {
            let fp = payload_with_values(json!({ "webrtc_masking": word })).to_fingerprint();
            assert_eq!(
                fp.policy().webrtc_policy,
                Some(WebRtcPolicy::DisableNonProxiedUdp),
                "{word}"
            );
            assert!(fp.is_masked(Dimension::WebRtc), "{word}");
            assert!(fp.webrtc().is_off(), "{word}");
        }
        let natural = payload_with_values(json!({ "webrtc_masking": "natural" })).to_fingerprint();
        assert!(!natural.is_masked(Dimension::WebRtc));
        assert_eq!(natural.policy().webrtc_policy, None);
        // A payload that says nothing about WebRTC still keeps the leak closed.
        let silent = payload_with_values(json!({})).to_fingerprint();
        assert_eq!(
            silent.policy().webrtc_policy,
            Some(WebRtcPolicy::DisableNonProxiedUdp)
        );
        assert!(silent.webrtc().is_natural());
    }

    #[test]
    fn a_payload_without_a_canvas_flag_keeps_the_seeded_canvas_noise() {
        let fp = payload_with_values(json!({})).to_fingerprint();
        assert_eq!(fp.policy().canvas_noise, CanvasNoise::Persistent);
        assert!(fp.is_masked(Dimension::Canvas));
        let auto = payload_with_values(json!({ "canvas_noise": "auto" })).to_fingerprint();
        assert_eq!(auto.policy().canvas_noise, CanvasNoise::Natural);
        assert!(!auto.is_masked(Dimension::Canvas));
    }

    // ------------------------------------------------------------------
    // One table of words for every key
    // ------------------------------------------------------------------

    /// The settings of each persona key, as `(key, is the setting in the state)`.
    type States = Vec<(&'static str, bool)>;

    fn natural_states(fp: &Fingerprint) -> States {
        vec![
            (
                "navigator",
                fp.user_agent().is_natural() && fp.hardware_concurrency().is_natural(),
            ),
            (
                "localization",
                fp.locale().is_natural() && fp.languages().is_natural(),
            ),
            ("timezone", fp.timezone().is_natural()),
            (
                "screen",
                fp.screen().is_natural() && fp.color_depth().is_natural(),
            ),
            ("graphics", fp.webgl().is_natural()),
            ("fonts", fp.fonts().is_natural()),
            ("media_devices", fp.media_devices().is_natural()),
            ("geolocation", fp.geolocation().is_natural()),
            ("webrtc", fp.webrtc().is_natural()),
            ("proxy", fp.proxy().is_natural()),
        ]
    }

    /// A payload whose every persona key reads `word`, and nothing else.
    fn every_key(word: &str) -> Fingerprint {
        fingerprint_of(json!({
            "navigator_masking": word,
            "localization_masking": word,
            "timezone_masking": word,
            "screen_masking": word,
            "graphics_masking": word,
            "fonts_masking": word,
            "media_devices_masking": word,
            "geolocation_masking": word,
            "webrtc_masking": word,
            "proxy_masking": word
        }))
    }

    #[test]
    fn the_natural_words_and_auto_leave_every_key_natural_when_the_payload_holds_no_value() {
        for word in [
            "natural",
            "native",
            "pass_through",
            "whitelist",
            "auto",
            "",
            "NATURAL",
        ] {
            let states = natural_states(&every_key(word));
            assert!(
                states.iter().all(|(_, natural)| *natural),
                "{word:?}: {states:?}"
            );
        }
    }

    #[test]
    fn the_mask_words_draw_a_value_for_every_key_that_can_be_drawn() {
        for word in [
            "mask", "spoof", "custom", "generic", "random", "noise", "Mask",
        ] {
            let fp = every_key(word);
            let states = natural_states(&fp);
            for (key, natural) in &states {
                // A proxy cannot be drawn, so with none in the payload it stays natural.
                assert_eq!(*natural, *key == "proxy", "{word}: {key}");
            }
            // The draw is one coherent machine in one place.
            let report = fp.validate();
            assert!(report.errors.is_empty(), "{word}: {:?}", report.errors);
            assert!(report.warnings.is_empty(), "{word}: {:?}", report.warnings);
            assert_eq!(fp.locale().as_str().is_some(), fp.timezone().is_value());
        }
    }

    #[test]
    fn the_mask_words_use_a_value_the_payload_holds_and_draw_the_rest() {
        let fp = payload_with_values(json!({
            "navigator_masking": "mask",
            "screen_masking": "random",
            "proxy_masking": "custom"
        }))
        .to_fingerprint();

        // The values the payload holds are kept...
        assert_eq!(fp.platform().as_str(), Some("Win32"));
        assert!(user_agent_of(&fp).contains("Chrome/133"));
        assert_eq!(fp.screen().value(), Some(&ScreenSize::new(1440, 900)));
        assert_eq!(fp.pixel_ratio().value(), Some(&2.0));
        assert_eq!(
            fp.proxy().value().map(|p| p.host.as_str()),
            Some("proxy.example")
        );
        // ...and what it does not hold is drawn.
        assert!(fp.hardware_concurrency().is_value() && fp.vendor().is_value());
        assert!(fp.color_depth().is_value());
    }

    fn user_agent_of(fp: &Fingerprint) -> &str {
        fp.user_agent().as_str().unwrap_or_default()
    }

    #[test]
    fn the_off_words_turn_off_geolocation_webrtc_and_media_devices_and_leave_the_rest_natural() {
        for word in ["off", "disabled", "block", "empty", "DISABLED"] {
            let fp = every_key(word);
            assert!(fp.geolocation().is_off(), "{word}");
            assert!(fp.webrtc().is_off(), "{word}");
            assert!(fp.media_devices().is_off(), "{word}");
            let natural: Vec<_> = natural_states(&fp)
                .into_iter()
                .filter(|(_, natural)| *natural)
                .map(|(key, _)| key)
                .collect();
            assert_eq!(
                natural,
                [
                    "navigator",
                    "localization",
                    "timezone",
                    "screen",
                    "graphics",
                    "fonts",
                    "proxy"
                ],
                "{word}"
            );
        }
    }

    #[test]
    fn a_keyword_nobody_knows_is_natural_and_never_a_guess() {
        // An unknown word used to fall back to masking the dimension, and for
        // five keys any text with one character in it was a literal.
        let fp = every_key("wibble");
        let states = natural_states(&fp);
        assert!(states.iter().all(|(_, natural)| *natural), "{states:?}");
        let fp = fingerprint_of(json!({
            "audio_masking": "wibble",
            "canvas_noise": "wibble",
            "quic_mode": "wibble"
        }));
        assert_eq!(fp.policy().audio_noise, AudioNoise::Natural);
        assert_eq!(fp.policy().canvas_noise, CanvasNoise::Natural);
        assert_eq!(fp.policy().quic, QuicMode::Natural);
    }

    #[test]
    fn the_same_profile_draws_the_same_values_every_time() {
        let draw = |folder: &str| {
            let params: ProfileParams = serde_json::from_value(json!({
                "name": "Stable",
                "folder_id": folder,
                "parameters": { "flags": { "navigator_masking": "mask", "timezone_masking": "mask" } }
            }))
            .unwrap();
            params.to_fingerprint()
        };
        assert_eq!(draw("one"), draw("one"));
        assert_ne!(draw("one"), draw("two"));
        assert_eq!(draw("one").seed(), draw("one").seed());
    }

    #[test]
    fn what_cannot_be_drawn_is_left_natural_and_the_rest_still_is() {
        // A zone the table of places lacks has no locale to agree with.
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Unknown zone",
            "parameters": {
                "flags": { "localization_masking": "mask", "navigator_masking": "mask" },
                "fingerprint": { "timezone": { "zone": "Pacific/Chatham" } }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.timezone().as_str(), Some("Pacific/Chatham"));
        assert!(fp.locale().is_natural(), "no locale agrees with that zone");
        assert!(
            fp.user_agent().is_natural(),
            "the draw as a whole is left out"
        );
    }

    /// A payload whose sections hold only empty strings, with `flags` as flags.
    fn blank_payload(flags: serde_json::Value) -> ProfileParams {
        serde_json::from_value(json!({
            "name": "Blank",
            "parameters": {
                "flags": flags,
                "fingerprint": {
                    "navigator": { "user_agent": "", "platform": " ", "os_cpu": "" },
                    "localization": { "languages": "", "locale": "", "accept_languages": "" },
                    "timezone": { "zone": "" },
                    "graphic": { "vendor": "", "renderer": "" },
                    "fonts": ["", " "]
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn an_empty_string_in_a_section_is_a_value_the_payload_does_not_give() {
        let params = blank_payload(json!({}));
        let fp = params.to_fingerprint();
        for (key, natural) in natural_states(&fp) {
            assert!(natural, "{key}");
        }
        assert!(fp.platform().is_natural() && fp.oscpu().is_natural());
        assert!(fp.accept_languages().is_natural());
        // Nothing empty reaches the launch arguments.
        let config = params.to_browser_config("http://localhost:4444");
        assert_eq!((config.user_agent, config.locale), (None, None));

        // A mask word draws what the payload does not give, never an empty value.
        let drawn = blank_payload(json!({
            "navigator_masking": "mask",
            "localization_masking": "mask",
            "timezone_masking": "mask"
        }))
        .to_fingerprint();
        assert!(user_agent_of(&drawn).starts_with("Mozilla/"));
        assert!(drawn.locale().as_str().is_some_and(|l| !l.is_empty()));
        assert!(drawn.timezone().as_str().is_some_and(|z| z.contains('/')));
    }

    #[test]
    fn a_navigator_section_without_a_core_count_does_not_invent_one() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Cores",
            "parameters": { "fingerprint": { "navigator": {
                "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/133.0.0.0",
                "platform": "Win32"
            } } }
        }))
        .unwrap();
        assert!(params.to_fingerprint().hardware_concurrency().is_natural());
    }

    #[test]
    fn the_browser_config_launches_with_what_the_fingerprint_holds() {
        let launch = |flags: serde_json::Value| {
            payload_with_values(flags).to_browser_config("http://localhost:4444")
        };
        let auto = launch(json!({}));
        assert!(auto
            .user_agent
            .as_deref()
            .is_some_and(|ua| ua.contains("Chrome/133")));
        assert_eq!(auto.locale.as_deref(), Some("de-DE"));
        assert_eq!(auto.proxy.as_deref(), Some("http://proxy.example:8080"));

        // A natural flag launches with none of it, whatever the payload holds.
        let natural = launch(json!({
            "navigator_masking": "natural",
            "localization_masking": "natural",
            "proxy_masking": "natural"
        }));
        assert_eq!(
            (natural.user_agent, natural.locale, natural.proxy),
            (None, None, None)
        );
        // So does the word for no proxy, and the off words, which a proxy lacks.
        for word in ["direct", "off", "disabled"] {
            assert_eq!(
                launch(json!({ "proxy_masking": word })).proxy,
                None,
                "{word}"
            );
        }
        // The getters answer the same.
        let params = payload_with_values(json!({ "proxy_masking": "natural" }));
        assert_eq!(params.proxy_string(), None);
        assert!(payload_with_values(json!({})).proxy_string().is_some());
    }

    #[test]
    fn the_core_version_of_a_payload_is_the_chromium_major_a_drawn_user_agent_claims() {
        let agent = |browser: &str, os: &str| {
            let params: ProfileParams = serde_json::from_value(json!({
                "name": "Core",
                "browser_type": browser,
                "os_type": os,
                "core_version": 124,
                "parameters": { "flags": { "navigator_masking": "mask" } }
            }))
            .unwrap();
            user_agent_of(&params.to_fingerprint()).to_owned()
        };
        assert!(agent("chromium", "windows").contains("Chrome/124."));
        // The core of a Firefox or Safari payload is not a Chrome version.
        assert!(!agent("firefox", "linux").contains("Chrome/124."));
        assert!(!agent("mobile_safari", "ios").contains("Chrome/"));
    }

    /// A payload in the shape the external tool sends it: every flag key present
    /// with the words and values the old reader took, and every section.
    #[test]
    fn a_payload_in_the_old_format_loads_with_every_key_read() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Old format",
            "browser_type": "chromium",
            "folder_id": "4500dd84-d8c5-4450-b2df-1c64daed8bad",
            "os_type": "windows",
            "automation": "selenium",
            "parameters": {
                "flags": {
                    "webrtc_masking": "public:203.0.113.9|local:10.0.0.2",
                    "proxy_masking": "http://user:pass@proxy.example:3128",
                    "geolocation_popup": "allow",
                    "audio_masking": "mask",
                    "graphics_noise": "medium",
                    "ports_masking": "block:80,8080",
                    "navigator_masking": "desktop_chrome",
                    "localization_masking": "spoof",
                    "timezone_masking": "Europe/Paris",
                    "graphics_masking": "NVIDIA~GeForce RTX 3060",
                    "fonts_masking": "system_only",
                    "media_devices_masking": "2_mic_1_cam",
                    "screen_masking": "2560x1440x30",
                    "geolocation_masking": "48.8566,2.3522,35",
                    "quic_mode": "disabled",
                    "canvas_noise": "low",
                    "startup_behavior": "custom",
                    "disable_csp": true,
                    "grant_permissions": true,
                    "native_spoofing": true
                },
                "fingerprint": {
                    "navigator": {
                        "hardware_concurrency": 12, "device_memory": 8.0,
                        "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
                        "platform": "Win32", "os_cpu": ""
                    },
                    "max_touch_points": 0,
                    "localization": { "languages": "fr-FR,fr", "locale": "fr-FR", "accept_languages": "fr-FR,fr;q=0.9" },
                    "timezone": { "zone": "Europe/Paris" },
                    "graphic": { "vendor": "Google Inc. (NVIDIA)", "renderer": "ANGLE (NVIDIA, GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)" },
                    "webrtc": { "public_ip": "203.0.113.9" },
                    "media_devices": { "audio_inputs": 2, "audio_outputs": 1, "video_inputs": 1 },
                    "screen": { "width": 2560, "height": 1440, "pixel_ratio": 1.5 },
                    "geolocation": { "latitude": 48.8566, "longitude": 2.3522, "accuracy": 25, "altitude": 35 },
                    "ports": [9222],
                    "fonts": ["Arial", "Calibri"],
                    "cmd_params": { "params": [ { "flag": "disable-quic", "value": "" } ] }
                },
                "storage": { "is_local": false, "save_service_worker": false },
                "proxy": { "type": "socks5", "host": "payload-proxy.example", "port": 1080 },
                "custom_start_urls": ["https://example.com"]
            }
        }))
        .unwrap();

        let fp = params.to_fingerprint();

        // The navigator: the section's values, since they were given.
        assert_eq!(fp.hardware_concurrency().value(), Some(&12));
        assert!(user_agent_of(&fp).contains("Chrome/133"));
        assert_eq!(fp.platform().as_str(), Some("Win32"));
        // Localization and time zone.
        assert_eq!(fp.locale().as_str(), Some("fr-FR"));
        assert_eq!(fp.languages().as_str(), Some("fr-FR,fr"));
        assert_eq!(fp.timezone().as_str(), Some("Europe/Paris"));
        // Graphics, media devices, screen, geolocation, fonts.
        assert_eq!(
            fp.webgl().value().and_then(|gl| gl.vendor.as_deref()),
            Some("Google Inc. (NVIDIA)")
        );
        assert_eq!(fp.media_devices().value().map(|d| d.audio_inputs), Some(2));
        assert_eq!(fp.screen().value(), Some(&ScreenSize::new(2560, 1440)));
        assert_eq!(fp.pixel_ratio().value(), Some(&1.5));
        assert!(
            fp.color_depth().is_value(),
            "the literal 2560x1440x30 asks to mask"
        );
        let place = fp.geolocation().value().expect("a position");
        assert_eq!((place.latitude, place.accuracy), (48.8566, Some(25.0)));
        assert_eq!(fp.fonts().value().map(Vec::len), Some(2));
        // The proxy: the URL of the flag wins over the payload's proxy section.
        assert_eq!(
            fp.proxy().value().map(|p| p.host.as_str()),
            Some("proxy.example")
        );
        // WebRTC: the addresses of the flag, and the policy that goes with them.
        assert_eq!(
            fp.webrtc().value().and_then(|a| a.public_ip.as_deref()),
            Some("203.0.113.9")
        );
        assert_eq!(
            fp.policy().webrtc_policy,
            Some(WebRtcPolicy::PublicAndPrivateInterfaces)
        );
        // The policy keys.
        assert_eq!(fp.policy().audio_noise, AudioNoise::Persistent);
        assert_eq!(fp.policy().canvas_noise, CanvasNoise::Low);
        assert_eq!(fp.policy().quic, QuicMode::Disabled);
        assert_eq!(fp.policy().blocked_ports, vec![80, 8080]);
        assert!(fp.policy().disable_csp && fp.policy().grant_permissions);
        assert!(fp.policy().native_spoofing);
        // Storage and the rest.
        assert!(!fp.local_storage() && !fp.save_service_worker());
        assert_eq!(fp.custom_start_urls().len(), 1);
        assert_eq!(
            fp.cmd_params().get("disable-quic").map(String::as_str),
            Some("")
        );
        assert!(fp.validate().is_coherent(), "{:?}", fp.validate().errors);
    }
}
