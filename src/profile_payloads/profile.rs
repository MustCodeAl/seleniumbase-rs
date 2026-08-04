use serde::{Deserialize, Serialize};

use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint as stealth_fp;
use crate::stealth::fingerprint::{
    CanvasNoiseMode, Fingerprint as StealthFingerprint, MaskingMode, MediaDeviceSpec, NoiseMode,
    OsType, PopupMode, ProxyMaskingMode, QuicMode, StartupBehavior, StealthFlags, WebRtcPolicy,
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
    pub flags: Flags,
    #[serde(default)]
    pub fingerprint: Fingerprint,
    #[serde(default)]
    pub storage: StorageOptions,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub custom_start_urls: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Flags {
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

impl Default for Flags {
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Fingerprint {
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProxyConfig {
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

fn default_webrtc_masking() -> String {
    "mask".into()
}
fn default_proxy_masking() -> String {
    "disabled".into()
}
fn default_geolocation_popup() -> String {
    "prompt".into()
}
fn default_audio_masking() -> String {
    "natural".into()
}
fn default_graphics_noise() -> String {
    "mask".into()
}
fn default_ports_masking() -> String {
    "mask".into()
}
fn default_navigator_masking() -> String {
    "mask".into()
}
fn default_localization_masking() -> String {
    "mask".into()
}
fn default_timezone_masking() -> String {
    "mask".into()
}
fn default_graphics_masking() -> String {
    "mask".into()
}
fn default_fonts_masking() -> String {
    "mask".into()
}
fn default_media_devices_masking() -> String {
    "natural".into()
}
fn default_screen_masking() -> String {
    "mask".into()
}
fn default_geolocation_masking() -> String {
    "mask".into()
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

fn parse_canvas_noise(s: &str) -> CanvasNoiseMode {
    match s.to_lowercase().as_str() {
        "natural" => CanvasNoiseMode::Natural,
        "disabled" | "off" => CanvasNoiseMode::Disabled,
        "random" => CanvasNoiseMode::Random,
        "persistent" => CanvasNoiseMode::Persistent,
        "low" => CanvasNoiseMode::Low,
        _ => CanvasNoiseMode::Mask,
    }
}

fn parse_popup(s: &str) -> PopupMode {
    match s.to_lowercase().as_str() {
        "allow" => PopupMode::Allow,
        "block" => PopupMode::Block,
        _ => PopupMode::Prompt,
    }
}

fn parse_quic(s: &str) -> QuicMode {
    match s.to_lowercase().as_str() {
        "natural" | "auto" => QuicMode::Natural,
        "enabled" => QuicMode::Enabled,
        "disabled" => QuicMode::Disabled,
        "force_http2" | "forcehttp2" => QuicMode::ForceHttp2,
        _ => QuicMode::Disabled,
    }
}

fn parse_startup(s: &str) -> StartupBehavior {
    match s.to_lowercase().as_str() {
        "custom" => StartupBehavior::Custom,
        _ => StartupBehavior::Recover,
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

fn parse_screen_masking(s: &str) -> (MaskingMode, Option<(u32, u32, u32)>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "native" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "custom" | "mask" | "spoof" => return (MaskingMode::Mask, None),
        _ => {}
    }
    if let Some(dims) = parse_dimensions(s) {
        return (MaskingMode::Custom, Some(dims));
    }
    (MaskingMode::Mask, None)
}

fn parse_geolocation_masking(s: &str) -> (MaskingMode, Option<(f64, f64, f64)>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "auto" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "custom" | "mask" | "spoof" => return (MaskingMode::Mask, None),
        _ => {}
    }
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 2 {
        if let (Ok(latitude), Ok(longitude)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
            let altitude = parts
                .get(2)
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(0.0);
            return (MaskingMode::Custom, Some((latitude, longitude, altitude)));
        }
    }
    (MaskingMode::Mask, None)
}

fn parse_graphics_masking(s: &str) -> (MaskingMode, Option<(String, String)>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "pass_through" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "spoof" | "generic" | "mask" => return (MaskingMode::Mask, None),
        _ => {}
    }
    let parts: Vec<&str> = s.split('~').collect();
    if parts.len() >= 2 {
        return (
            MaskingMode::Custom,
            Some((parts[0].trim().to_string(), parts[1].trim().to_string())),
        );
    }
    (MaskingMode::Mask, None)
}

fn is_user_agent(s: &str) -> bool {
    s.starts_with("Mozilla/") || s.contains("AppleWebKit") || s.contains("Chrome/")
}

fn parse_navigator_masking(s: &str) -> (MaskingMode, Option<String>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "mask" | "spoof" | "desktop_chrome" | "mobile_safari" => return (MaskingMode::Mask, None),
        _ => {}
    }
    if is_user_agent(s) {
        return (MaskingMode::Custom, Some(s.trim().to_string()));
    }
    (MaskingMode::Mask, None)
}

fn parse_localization_masking(s: &str) -> (MaskingMode, Option<String>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "auto" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "mask" | "spoof" | "custom" => return (MaskingMode::Mask, None),
        _ => {}
    }
    if s.contains('-') {
        return (MaskingMode::Custom, Some(s.trim().to_string()));
    }
    (MaskingMode::Mask, None)
}

fn parse_timezone_masking(s: &str) -> (MaskingMode, Option<String>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "auto" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "mask" | "spoof" | "custom" => return (MaskingMode::Mask, None),
        _ => {}
    }
    if s.contains('/') {
        return (MaskingMode::Custom, Some(s.trim().to_string()));
    }
    (MaskingMode::Mask, None)
}

fn parse_fonts_masking(s: &str) -> (MaskingMode, Option<Vec<String>>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" => return (MaskingMode::Disabled, None),
        "mask" | "spoof" | "system_only" | "random" | "blacklist" => {
            return (MaskingMode::Mask, None)
        }
        _ => {}
    }
    let fonts: Vec<String> = s
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    if !fonts.is_empty() {
        return (MaskingMode::Custom, Some(fonts));
    }
    (MaskingMode::Mask, None)
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

#[allow(clippy::type_complexity)]
fn parse_media_devices_masking(
    s: &str,
) -> (MaskingMode, Option<(u32, u32, u32, Vec<MediaDeviceSpec>)>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" => return (MaskingMode::Natural, None),
        "disabled" | "block" | "empty" => return (MaskingMode::Disabled, None),
        "mask" | "spoof" | "noise" => return (MaskingMode::Mask, None),
        _ => {}
    }
    if lower.starts_with("2_mic_1_cam") {
        return (MaskingMode::Custom, Some((2, 1, 1, vec![])));
    }
    if s.contains(':') && s.contains('|') {
        let specs = parse_media_device_specs(s);
        let audio_inputs = specs.iter().filter(|d| d.kind == "audioinput").count() as u32;
        let audio_outputs = specs.iter().filter(|d| d.kind == "audiooutput").count() as u32;
        let video_inputs = specs.iter().filter(|d| d.kind == "videoinput").count() as u32;
        return (
            MaskingMode::Custom,
            Some((
                audio_inputs.max(1),
                audio_outputs.max(1),
                video_inputs.max(1),
                specs,
            )),
        );
    }
    (MaskingMode::Mask, None)
}

fn parse_ports_masking(s: &str) -> (NoiseMode, Option<Vec<u16>>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" | "disabled" => return (NoiseMode::Off, None),
        "mask" | "spoof" | "stealth" => return (NoiseMode::Mask, None),
        "block" | "block_all" => return (NoiseMode::Block, None),
        "whitelist" => return (NoiseMode::Natural, None),
        _ => {}
    }
    if lower.starts_with("block:") {
        let list = lower.strip_prefix("block:").unwrap_or("");
        let ports: Vec<u16> = list
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        return (NoiseMode::Block, Some(ports));
    }
    let ports: Vec<u16> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if !ports.is_empty() {
        return (NoiseMode::Block, Some(ports));
    }
    (NoiseMode::Mask, None)
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

fn parse_proxy_masking(s: &str) -> (ProxyMaskingMode, Option<stealth_fp::ProxyConfig>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" | "direct" => return (ProxyMaskingMode::Disabled, None),
        "disabled" => return (ProxyMaskingMode::Disabled, None),
        "socks5" => return (ProxyMaskingMode::Socks5, None),
        "http" => return (ProxyMaskingMode::Http, None),
        "https" => return (ProxyMaskingMode::Https, None),
        "custom" => return (ProxyMaskingMode::Custom, None),
        _ => {}
    }
    if let Some(cfg) = parse_proxy_url(s) {
        return (ProxyMaskingMode::Custom, Some(cfg));
    }
    (ProxyMaskingMode::Disabled, None)
}

fn parse_webrtc_masking(s: &str) -> (MaskingMode, Option<WebRtcPolicy>, Option<(String, String)>) {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "natural" | "off" => return (MaskingMode::Natural, None, None),
        "disabled" | "block" => {
            return (
                MaskingMode::Disabled,
                Some(WebRtcPolicy::DisableNonProxiedUdp),
                None,
            )
        }
        "public_only" => {
            return (
                MaskingMode::Mask,
                Some(WebRtcPolicy::PublicInterfaceOnly),
                None,
            )
        }
        "mask" | "spoof" => {
            return (
                MaskingMode::Mask,
                Some(WebRtcPolicy::DisableNonProxiedUdp),
                None,
            )
        }
        _ => {}
    }
    let mut public_ip = None;
    let mut local_ip = None;
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
            "public" => public_ip = Some(value.to_string()),
            "local" => local_ip = Some(value.to_string()),
            _ => {}
        }
    }
    if public_ip.is_some() || local_ip.is_some() {
        return (
            MaskingMode::Custom,
            Some(WebRtcPolicy::PublicAndPrivateInterfaces),
            Some((public_ip.unwrap_or_default(), local_ip.unwrap_or_default())),
        );
    }
    (
        MaskingMode::Mask,
        Some(WebRtcPolicy::DisableNonProxiedUdp),
        None,
    )
}

fn parse_audio_masking(s: &str) -> MaskingMode {
    match s.to_lowercase().as_str() {
        "natural" | "off" => MaskingMode::Natural,
        "disabled" | "block" => MaskingMode::Disabled,
        _ => MaskingMode::Mask,
    }
}

fn parse_graphics_noise(s: &str) -> NoiseMode {
    match s.to_lowercase().as_str() {
        "natural" => NoiseMode::Natural,
        "off" => NoiseMode::Off,
        "low" => NoiseMode::Low,
        "medium" => NoiseMode::Medium,
        "high" => NoiseMode::High,
        _ => NoiseMode::Mask,
    }
}

impl ProfileParams {
    /// Converts the external profile payload into a [`StealthFingerprint`] that can
    /// be injected into a browser session.
    ///
    /// Masking flags may be either behavioural keywords (e.g. `"mask"`,
    /// `"custom"`) or concrete literal values (e.g. `"1920x1080x24"`,
    /// `"America/New_York"`, `"socks5://user:pass@host:1080"`). When a literal
    /// value is supplied, the corresponding mode is set to `Custom` and the
    /// value is written into the fingerprint so providers and launch args can
    /// use it.
    pub fn to_fingerprint(&self) -> StealthFingerprint {
        let (screen_mode, screen_dims) =
            parse_screen_masking(&self.parameters.flags.screen_masking);
        let (geo_mode, geo_coords) =
            parse_geolocation_masking(&self.parameters.flags.geolocation_masking);
        let (gfx_mode, gfx_vendor_renderer) =
            parse_graphics_masking(&self.parameters.flags.graphics_masking);
        let (nav_mode, nav_ua) = parse_navigator_masking(&self.parameters.flags.navigator_masking);
        let (loc_mode, loc_value) =
            parse_localization_masking(&self.parameters.flags.localization_masking);
        let (tz_mode, tz_zone) = parse_timezone_masking(&self.parameters.flags.timezone_masking);
        let (fonts_mode, fonts_list) = parse_fonts_masking(&self.parameters.flags.fonts_masking);
        let (media_mode, media_spec) =
            parse_media_devices_masking(&self.parameters.flags.media_devices_masking);
        let (ports_mode, ports_list) = parse_ports_masking(&self.parameters.flags.ports_masking);
        let (proxy_mode, proxy_cfg) = parse_proxy_masking(&self.parameters.flags.proxy_masking);
        let (webrtc_mode, webrtc_policy, webrtc_ips) =
            parse_webrtc_masking(&self.parameters.flags.webrtc_masking);

        let mut builder = StealthFingerprint::builder()
            .os_type(parse_os(&self.os_type))
            .flags(StealthFlags {
                webrtc_masking: webrtc_mode,
                audio_masking: parse_audio_masking(&self.parameters.flags.audio_masking),
                graphics_noise: parse_graphics_noise(&self.parameters.flags.graphics_noise),
                geolocation_popup: parse_popup(&self.parameters.flags.geolocation_popup),
                navigator_masking: nav_mode,
                localization_masking: loc_mode,
                timezone_masking: tz_mode,
                graphics_masking: gfx_mode,
                fonts_masking: fonts_mode,
                media_devices_masking: media_mode,
                screen_masking: screen_mode,
                geolocation_masking: geo_mode,
                ports_masking: ports_mode,
                proxy_masking: proxy_mode,
                quic_mode: parse_quic(&self.parameters.flags.quic_mode),
                canvas_noise: self
                    .parameters
                    .flags
                    .canvas_noise
                    .as_deref()
                    .map(parse_canvas_noise)
                    .unwrap_or_default(),
                startup_behavior: parse_startup(&self.parameters.flags.startup_behavior),
                disable_csp: self.parameters.flags.disable_csp,
                grant_permissions: self.parameters.flags.grant_permissions,
                native_spoofing: self.parameters.flags.native_spoofing,
                ..StealthFlags::balanced()
            })
            .local_storage(self.parameters.storage.is_local)
            .save_service_worker(self.parameters.storage.save_service_worker)
            .custom_start_urls(self.parameters.custom_start_urls.clone())
            .blocked_ports(ports_list.unwrap_or_else(|| self.parameters.fingerprint.ports.clone()));

        if let Some(nav) = self.parameters.fingerprint.navigator.as_ref() {
            builder = builder
                .user_agent(nav.user_agent.clone())
                .platform(nav.platform.clone())
                .hardware_concurrency(nav.hardware_concurrency.unwrap_or(8));
            if let Some(mem) = nav.device_memory {
                builder = builder.device_memory(mem);
            }
            if !nav.os_cpu.is_empty() {
                builder = builder.os_cpu(nav.os_cpu.clone());
            }
        } else if let Some(ua) = nav_ua {
            builder = builder.user_agent(ua);
        }
        if let Some(touch) = self.parameters.fingerprint.max_touch_points {
            builder = builder.max_touch_points(touch);
        }

        if let Some(loc) = self.parameters.fingerprint.localization.as_ref() {
            builder = builder
                .locale(loc.locale.clone())
                .languages(loc.languages.clone())
                .accept_languages(loc.accept_languages.clone());
        } else if let Some(value) = loc_value {
            builder = builder
                .locale(value.clone())
                .languages(value.clone())
                .accept_languages(value);
        }

        if let Some(tz) = self.parameters.fingerprint.timezone.as_ref() {
            builder = builder.timezone(tz.zone.clone());
        } else if let Some(zone) = tz_zone {
            builder = builder.timezone(zone);
        }

        if let Some(gpu) = self.parameters.fingerprint.graphic.as_ref() {
            builder = builder.webgl(gpu.vendor.clone(), gpu.renderer.clone());
            if !gpu.vendor_id.is_empty() || !gpu.renderer_id.is_empty() {
                builder = builder.webgl_ids(gpu.vendor_id.clone(), gpu.renderer_id.clone());
            }
        } else if let Some((vendor, renderer)) = gfx_vendor_renderer {
            builder = builder.webgl(vendor, renderer);
        }

        if let Some(media) = self.parameters.fingerprint.media_devices.as_ref() {
            builder =
                builder.media_devices(media.audio_inputs, media.audio_outputs, media.video_inputs);
        } else if let Some((audio_in, audio_out, video_in, specs)) = media_spec {
            builder = builder
                .media_devices(audio_in, audio_out, video_in)
                .media_device_specs(specs);
        }

        if let Some(screen) = self.parameters.fingerprint.screen.as_ref() {
            builder = builder
                .screen(screen.width, screen.height)
                .pixel_ratio(screen.pixel_ratio);
        } else if let Some((w, h, d)) = screen_dims {
            builder = builder.screen(w, h).color_depth(d);
        }

        if let Some(geo) = self.parameters.fingerprint.geolocation.as_ref() {
            builder = builder
                .geolocation(geo.latitude, geo.longitude)
                .altitude(geo.altitude)
                .accuracy(geo.accuracy);
        } else if let Some((lat, lon, alt)) = geo_coords {
            builder = builder.geolocation(lat, lon).altitude(alt).accuracy(20.0);
        }

        let proxy_from_payload = self
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
        if let Some(proxy) = proxy_cfg.or(proxy_from_payload) {
            builder = builder.proxy(proxy);
        }

        let mut webrtc_policy_to_apply = webrtc_policy;
        let mut webrtc_ips_to_apply = webrtc_ips;
        if let Some(webrtc) = self.parameters.fingerprint.webrtc.as_ref() {
            if webrtc_ips_to_apply.is_none() && !webrtc.public_ip.is_empty() {
                webrtc_ips_to_apply = Some((webrtc.public_ip.clone(), String::new()));
                webrtc_policy_to_apply.get_or_insert(
                    crate::stealth::fingerprint::WebRtcPolicy::PublicAndPrivateInterfaces,
                );
            }
        }
        if let Some(policy) = webrtc_policy_to_apply {
            builder = builder.webrtc_policy(policy);
        }
        if let Some((public_ip, local_ip)) = webrtc_ips_to_apply {
            builder = builder.webrtc_ips(public_ip, local_ip);
        }

        let fonts = fonts_list
            .or_else(|| Some(self.parameters.fingerprint.fonts.clone()))
            .unwrap_or_default();
        if !fonts.is_empty() {
            builder = builder.fonts(fonts);
        }

        let mut cmd_params = std::collections::HashMap::new();
        for p in self.parameters.fingerprint.cmd_params.params.iter() {
            cmd_params.insert(p.flag.clone(), p.value.clone());
        }
        builder = builder.cmd_params(cmd_params);

        builder.build()
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
        let custom_start_urls: Vec<String> = self
            .parameters
            .custom_start_urls
            .iter()
            .take(5)
            .cloned()
            .collect();
        let mut config = BrowserConfig {
            webdriver_url: container_url.into(),
            browser: self.browser(),
            headless: self.is_headless,
            mode,
            user_agent: self.user_agent(),
            locale: self.locale(),
            proxy: self.proxy_string(),
            proxy_pac_url: None,
            user_data_dir: self.user_data_dir(),
            extension_dir: None,
            start_page: custom_start_urls.first().cloned(),
            reuse_session: false,
            mobile: self.os_type == "android" || self.os_type == "ios",
            threads: None,
            ad_block: self
                .parameters
                .proxy
                .as_ref()
                .map(|p| p.save_traffic)
                .unwrap_or(false),
            auto_start_driver: true,
            extra_args: Vec::new(),
            fingerprint: Some(self.to_fingerprint()),
            browser_binary_path: None,
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

    /// User agent extracted from custom navigator fingerprint when available.
    pub fn user_agent(&self) -> Option<String> {
        self.parameters
            .fingerprint
            .navigator
            .as_ref()
            .map(|n| n.user_agent.clone())
            .filter(|s| !s.is_empty())
    }

    /// Locale extracted from custom localization fingerprint when available.
    pub fn locale(&self) -> Option<String> {
        self.parameters
            .fingerprint
            .localization
            .as_ref()
            .map(|l| l.locale.clone())
            .filter(|s| !s.is_empty())
    }

    /// Proxy URL built from `parameters.proxy`.
    pub fn proxy_string(&self) -> Option<String> {
        self.parameters.proxy.as_ref().map(|p| {
            if p.username.is_empty() {
                format!("{}://{}:{}", p.proxy_type, p.host, p.port)
            } else {
                format!(
                    "{}://{}:{}@{}:{}",
                    p.proxy_type, p.username, p.password, p.host, p.port
                )
            }
        })
    }

    /// Per-profile persistent data directory when `storage.is_local` is true.
    pub fn user_data_dir(&self) -> Option<String> {
        if self.parameters.storage.is_local {
            Some(format!("./profile-data/{}", self.folder_id))
        } else {
            None
        }
    }

    /// Additional Chromium command-line flags parsed from `cmd_params`.
    pub fn extra_args(&self) -> Vec<String> {
        self.parameters
            .fingerprint
            .cmd_params
            .params
            .iter()
            .map(|p| {
                if p.value.is_empty() {
                    format!("--{}", p.flag)
                } else {
                    format!("--{}={}", p.flag, p.value)
                }
            })
            .collect()
    }

    /// Applies runtime fingerprint overrides to an active `BaseCase`.
    ///
    /// This covers screen size and geolocation, which must be set after the
    /// browser session is alive.
    pub async fn apply_runtime_overrides(
        &self,
        sb: &mut crate::BaseCase,
    ) -> Result<(), SeleniumBaseError> {
        if let Some(screen) = &self.parameters.fingerprint.screen {
            sb.set_window_size(screen.width, screen.height).await?;
        }
        if let Some(geo) = &self.parameters.fingerprint.geolocation {
            sb.set_geolocation(geo.latitude, geo.longitude, geo.accuracy)
                .await?;
        }
        for url in self.parameters.custom_start_urls.iter().skip(1) {
            sb.open(url).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(params.parameters.flags.webrtc_masking, "mask");
        assert!(params.parameters.storage.is_local);
        assert_eq!(params.times, 1);
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
        assert_eq!(fp.hardware_concurrency, Some(8));
        assert_eq!(fp.device_memory, Some(4.0));
        assert_eq!(fp.oscpu.as_deref(), Some("Linux armv8l"));
        assert_eq!(fp.max_touch_points, Some(5));
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
        assert_eq!(fp.webgl_vendor.as_deref(), Some("NVIDIA"));
        assert_eq!(fp.webgl_renderer.as_deref(), Some("GeForce RTX"));
        assert_eq!(fp.webgl_vendor_id.as_deref(), Some("0x10de"));
        assert_eq!(fp.webgl_renderer_id.as_deref(), Some("0x1f91"));
    }

    #[test]
    fn custom_start_urls_are_capped_at_five() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Urls",
            "parameters": {
                "custom_start_urls": ["a", "b", "c", "d", "e", "f"]
            }
        }))
        .unwrap();
        let config = params.to_browser_config("http://localhost:4444");
        assert_eq!(config.start_page, Some("a".to_string()));
    }

    #[test]
    fn concrete_screen_masking_parses_dimensions() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Screen",
            "parameters": { "flags": { "screen_masking": "1920x1080x24" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.screen_masking, MaskingMode::Custom);
        assert_eq!(fp.screen_width, Some(1920));
        assert_eq!(fp.screen_height, Some(1080));
        assert_eq!(fp.color_depth, Some(24));
    }

    #[test]
    fn concrete_geolocation_masking_parses_coordinates() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Geo",
            "parameters": { "flags": { "geolocation_masking": "40.7128,-74.0060,10.5" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.geolocation_masking, MaskingMode::Custom);
        assert_eq!(fp.latitude, Some(40.7128));
        assert_eq!(fp.longitude, Some(-74.0060));
        assert_eq!(fp.altitude, Some(10.5));
    }

    #[test]
    fn concrete_graphics_masking_parses_vendor_renderer() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "GPU",
            "parameters": { "flags": { "graphics_masking": "Intel Inc.~Intel(R) Iris(R) Xe Graphics" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.graphics_masking, MaskingMode::Custom);
        assert_eq!(fp.webgl_vendor, Some("Intel Inc.".to_string()));
        assert_eq!(
            fp.webgl_renderer,
            Some("Intel(R) Iris(R) Xe Graphics".to_string())
        );
    }

    #[test]
    fn concrete_navigator_masking_parses_user_agent() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "UA",
            "parameters": { "flags": { "navigator_masking": ua } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.navigator_masking, MaskingMode::Custom);
        assert_eq!(fp.user_agent, Some(ua.to_string()));
    }

    #[test]
    fn concrete_localization_and_timezone_masking_parses_values() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Locale",
            "parameters": {
                "flags": {
                    "localization_masking": "en-US,en;q=0.9",
                    "timezone_masking": "America/New_York"
                }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.localization_masking, MaskingMode::Custom);
        assert_eq!(fp.flags.timezone_masking, MaskingMode::Custom);
        assert_eq!(fp.locale, Some("en-US,en;q=0.9".to_string()));
        assert_eq!(fp.timezone, Some("America/New_York".to_string()));
    }

    #[test]
    fn concrete_fonts_masking_parses_list() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Fonts",
            "parameters": { "flags": { "fonts_masking": "Arial,Calibri,Consolas" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.fonts_masking, MaskingMode::Custom);
        assert_eq!(fp.fonts, vec!["Arial", "Calibri", "Consolas"]);
    }

    #[test]
    fn concrete_media_devices_masking_parses_specs() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Media",
            "parameters": { "flags": { "media_devices_masking": "audioinput:default:Mic1|videoinput:default:Cam1" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.media_devices_masking, MaskingMode::Custom);
        assert_eq!(fp.audio_inputs, Some(1));
        assert_eq!(fp.video_inputs, Some(1));
        assert_eq!(fp.media_devices.len(), 2);
        assert_eq!(fp.media_devices[0].label, "Mic1");
        assert_eq!(fp.media_devices[1].kind, "videoinput");
    }

    #[test]
    fn concrete_ports_masking_parses_block_list() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Ports",
            "parameters": { "flags": { "ports_masking": "block:80,443,3000,8080,9222" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.ports_masking, NoiseMode::Block);
        assert_eq!(fp.ports, vec![80, 443, 3000, 8080, 9222]);
    }

    #[test]
    fn concrete_proxy_masking_parses_url() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Proxy",
            "parameters": { "flags": { "proxy_masking": "socks5://user123:pass456@192.168.1.50:1080" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.proxy_masking, ProxyMaskingMode::Custom);
        let proxy = fp.proxy.unwrap();
        assert_eq!(proxy.r#type, "socks5");
        assert_eq!(proxy.host, "192.168.1.50");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username, Some("user123".to_string()));
        assert_eq!(proxy.password, Some("pass456".to_string()));
    }

    #[test]
    fn concrete_webrtc_masking_parses_ips() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "WebRTC",
            "parameters": { "flags": { "webrtc_masking": "public:172.56.21.89|local:10.0.0.5" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.webrtc_masking, MaskingMode::Custom);
        assert_eq!(fp.webrtc_public_ip, Some("172.56.21.89".to_string()));
        assert_eq!(fp.webrtc_local_ip, Some("10.0.0.5".to_string()));
        assert_eq!(
            fp.webrtc_policy,
            crate::stealth::fingerprint::WebRtcPolicy::PublicAndPrivateInterfaces
        );
    }

    #[test]
    fn concrete_webrtc_masking_parses_ipv6_ips() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "WebRTCv6",
            "parameters": { "flags": { "webrtc_masking": "public:[2001:db8::1]|local:[fe80::1]" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.webrtc_masking, MaskingMode::Custom);
        assert_eq!(fp.webrtc_public_ip, Some("2001:db8::1".to_string()));
        assert_eq!(fp.webrtc_local_ip, Some("fe80::1".to_string()));
    }

    #[test]
    fn concrete_proxy_masking_parses_ipv6_url() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Proxyv6",
            "parameters": { "flags": { "proxy_masking": "socks5://[2001:db8::1]:1080" } }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.proxy_masking, ProxyMaskingMode::Custom);
        let proxy = fp.proxy.unwrap();
        assert_eq!(proxy.r#type, "socks5");
        assert_eq!(proxy.host, "2001:db8::1");
        assert_eq!(proxy.port, 1080);
    }

    #[test]
    fn flag_keywords_still_map_to_modes() {
        let params: ProfileParams = serde_json::from_value(json!({
            "name": "Keywords",
            "parameters": {
                "flags": {
                    "audio_masking": "noise",
                    "graphics_noise": "medium",
                    "canvas_noise": "random",
                    "quic_mode": "force_http2",
                    "ports_masking": "block_all"
                }
            }
        }))
        .unwrap();
        let fp = params.to_fingerprint();
        assert_eq!(fp.flags.audio_masking, MaskingMode::Mask);
        assert_eq!(fp.flags.graphics_noise, NoiseMode::Medium);
        assert_eq!(fp.flags.canvas_noise, CanvasNoiseMode::Random);
        assert_eq!(
            fp.flags.quic_mode,
            crate::stealth::fingerprint::QuicMode::ForceHttp2
        );
        assert_eq!(fp.flags.ports_masking, NoiseMode::Block);
    }
}
