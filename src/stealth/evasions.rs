//! JavaScript anti-detection / fingerprint spoofing payloads.
//!
//! The functions in this module build self-contained IIFE scripts from a
//! [`Fingerprint`](crate::stealth::fingerprint::Fingerprint). The returned
//! strings are suitable for:
//!
//! * `Page.addScriptToEvaluateOnNewDocument` via CDP (injected before any page
//!   JavaScript runs),
//! * `Page.evaluate` after navigation (rustwright / Playwright-compatible
//!   mode), or
//! * execution by a [`BrowserSession`](crate::browser::session::BrowserSession)
//!   helper.
//!
//! The evasions are inspired by `chromiumoxide_stealth`, `eoka`,
//! `chaser-oxide`, and `undetected-chromedriver`.

use std::collections::HashMap;

use base64::Engine;

use crate::stealth::fingerprint::{Fingerprint, OsType, ProxyMaskingMode, QuicMode};
use crate::stealth::providers::{default_registry, EvasionContext};

/// Generates a single combined bootstrap script for the given fingerprint.
///
/// This delegates to the [provider registry](crate::stealth::providers), which
/// assembles every applicable [`EvasionProvider`](crate::stealth::providers::EvasionProvider)
/// in priority order. The returned script is wrapped in an IIFE and modifies
/// prototype-level properties so spoofed values survive `getOwnPropertyNames`
/// scans and re-define attempts by page scripts.
pub fn bootstrap_script(fp: &Fingerprint) -> String {
    default_registry().bootstrap(&EvasionContext::new(fp))
}

/// Returns a script that scrubs `cdc_` / webdriver markers from `window` and
/// the prototype chain. Run inline on every navigation for WebDriver sessions.
pub fn cdc_scrub() -> String {
    "(() => {
  const re = /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/;
  function scrub(obj) {
    if (!obj) return;
    Object.getOwnPropertyNames(obj).forEach(key => {
      if (re.test(key)) {
        try { delete obj[key]; } catch (e) {}
      }
    });
  }
  let o = window;
  while (o) {
    scrub(o);
    o = Object.getPrototypeOf(o);
  }
})();"
        .to_owned()
}

/// Returns Chromium command-line arguments derived from a fingerprint.
pub fn launch_args(fp: &Fingerprint) -> Vec<String> {
    let mut args = vec![
        "--disable-blink-features=AutomationControlled".to_owned(),
        "--disable-infobars".to_owned(),
        "--no-default-browser-check".to_owned(),
        "--no-first-run".to_owned(),
        "--no-service-autorun".to_owned(),
        "--no-pings".to_owned(),
        "--homepage=about:blank".to_owned(),
        "--disable-background-timer-throttling".to_owned(),
        "--disable-backgrounding-occluded-windows".to_owned(),
        "--disable-renderer-backgrounding".to_owned(),
        "--disable-popup-blocking".to_owned(),
        "--disable-translate".to_owned(),
        "--disable-search-engine-choice-screen".to_owned(),
        "--enable-unsafe-extension-debugging".to_owned(),
        "--password-store=basic".to_owned(),
        "--profile-directory=Default".to_owned(),
        "--safebrowsing-disable-download-protection".to_owned(),
        "--disable-client-side-phishing-detection".to_owned(),
        "--disable-single-click-autofill".to_owned(),
        "--disable-password-generation".to_owned(),
        "--disable-save-password-bubble".to_owned(),
        "--simulate-outdated-no-au=\"Tue, 31 Dec 2099 23:59:59 GMT\"".to_owned(),
        "--disable-features=IsolateOrigins,site-per-process,Translate,InsecureDownloadWarnings,DownloadBubble,DownloadBubbleV2,OptimizationTargetPrediction,OptimizationGuideModelDownloading,SafeBrowsingEnhancedProtection,PrivacySandboxSettings4,AutofillEnableAccountWalletStorage".to_owned(),
    ];

    // Sandbox flags are required for Docker / CI environments and avoid setuid
    // failures on some Linux distributions. They are a common fingerprinting
    // trade-off; real users launching with a real user account can omit these.
    args.push("--no-sandbox".to_owned());
    args.push("--disable-setuid-sandbox".to_owned());
    args.push("--disable-dev-shm-usage".to_owned());

    if let Some(ua) = fp.user_agent.as_deref() {
        args.push(format!("--user-agent={ua}"));
    }

    if let Some(locale) = fp.locale.as_deref() {
        args.push(format!("--lang={locale}"));
    }

    if let (Some(w), Some(h)) = (fp.screen_width, fp.screen_height) {
        args.push(format!("--window-size={w},{h}"));
    }

    match fp.webrtc_policy {
        crate::stealth::fingerprint::WebRtcPolicy::DisableNonProxiedUdp => {
            args.push("--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_owned());
        }
        crate::stealth::fingerprint::WebRtcPolicy::PublicInterfaceOnly => {
            args.push("--force-webrtc-ip-handling-policy=default_public_interface_only".to_owned());
        }
        crate::stealth::fingerprint::WebRtcPolicy::PublicAndPrivateInterfaces => {
            args.push(
                "--force-webrtc-ip-handling-policy=default_public_and_private_interfaces"
                    .to_owned(),
            );
        }
    }

    if matches!(
        fp.flags.proxy_masking,
        ProxyMaskingMode::Custom
            | ProxyMaskingMode::Socks5
            | ProxyMaskingMode::Http
            | ProxyMaskingMode::Https
    ) {
        if let Some(proxy) = fp.proxy.as_ref() {
            args.push(format!("--proxy-server={}", proxy.to_url()));
        }
    }

    match fp.flags.quic_mode {
        QuicMode::Disabled | QuicMode::ForceHttp2 => args.push("--disable-quic".to_owned()),
        QuicMode::Enabled => args.push("--enable-quic".to_owned()),
        QuicMode::Natural | QuicMode::Auto => {}
    }

    for (flag, value) in &fp.cmd_params {
        if value.is_empty() {
            args.push(format!("--{flag}"));
        } else {
            args.push(format!("--{flag}={value}"));
        }
    }

    args
}

/// Returns CDP parameter overrides recommended for a fingerprint.
pub fn cdp_overrides(fp: &Fingerprint) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();

    if let (Some(lat), Some(lon)) = (fp.latitude, fp.longitude) {
        map.insert(
            "Emulation.setGeolocationOverride".to_owned(),
            serde_json::json!({
                "latitude": lat,
                "longitude": lon,
                "accuracy": fp.accuracy.unwrap_or(100.0),
                "altitude": fp.altitude.unwrap_or(0.0),
            }),
        );
    }

    if let (Some(w), Some(h)) = (fp.screen_width, fp.screen_height) {
        map.insert(
            "Emulation.setDeviceMetricsOverride".to_owned(),
            serde_json::json!({
                "width": w,
                "height": h,
                "deviceScaleFactor": fp.pixel_ratio.unwrap_or(1.0),
                "mobile": fp.os_type.is_mobile(),
            }),
        );
    }

    if let Some(ua) = fp.user_agent.as_deref() {
        let platform = fp
            .platform
            .clone()
            .unwrap_or_else(|| fp.os_type.platform().to_owned());
        let accept_language = fp
            .accept_languages
            .clone()
            .or_else(|| fp.locale.clone())
            .unwrap_or_else(|| "en-US,en;q=0.9".to_owned());
        let mut ua_override = serde_json::json!({
            "userAgent": ua,
            "acceptLanguage": accept_language,
            "platform": platform,
        });
        if let Some(meta) = build_user_agent_metadata(fp, ua) {
            ua_override
                .as_object_mut()
                .expect("object")
                .insert("userAgentMetadata".to_owned(), meta);
        }
        map.insert("Network.setUserAgentOverride".to_owned(), ua_override);
    }

    if let Some(locale) = fp.locale.as_deref() {
        map.insert(
            "Emulation.setLocaleOverride".to_owned(),
            serde_json::json!({ "locale": locale }),
        );
    }

    if let Some(tz) = fp.timezone.as_deref() {
        map.insert(
            "Emulation.setTimezoneOverride".to_owned(),
            serde_json::json!({ "timezoneId": tz }),
        );
    }

    if fp.flags.grant_permissions {
        map.insert(
            "Browser.grantPermissions".to_owned(),
            serde_json::json!({
                "permissions": [
                    "notifications", "midi", "midiSysex", "clipboardRead",
                    "clipboardWrite", "clipboardSanitizedWrite", "paymentHandler",
                    "backgroundSync", "idleDetection", "webAppInstallation"
                ]
            }),
        );
    }

    if fp.flags.block_trackers {
        let hosts: Vec<String> = fp
            .tracker_hosts()
            .iter()
            .map(|h| format!("*{h}*"))
            .collect();
        if !hosts.is_empty() {
            map.insert(
                "Network.setBlockedURLs".to_owned(),
                serde_json::json!({ "urls": hosts }),
            );
        }
    }

    if fp.flags.disable_csp {
        map.insert(
            "Page.setBypassCSP".to_owned(),
            serde_json::json!({ "enabled": true }),
        );
    }

    if matches!(
        fp.flags.headless_masking,
        crate::stealth::fingerprint::MaskingMode::Mask
            | crate::stealth::fingerprint::MaskingMode::Custom
    ) {
        map.insert(
            "Emulation.setFocusEmulationEnabled".to_owned(),
            serde_json::json!({ "enabled": true }),
        );
    }

    if let Some(proxy) = fp.proxy.as_ref() {
        if proxy.username.is_some() && proxy.password.is_some() {
            let creds = format!(
                "{}:{}",
                proxy.username.as_deref().unwrap_or_default(),
                proxy.password.as_deref().unwrap_or_default()
            );
            let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
            map.insert(
                "Network.setExtraHTTPHeaders".to_owned(),
                serde_json::json!({ "headers": { "Proxy-Authorization": format!("Basic {encoded}") } }),
            );
        }
    }

    map
}

/// Builds a `userAgentMetadata` object for `Network.setUserAgentOverride` from
/// the fingerprint. This populates Client Hints at the browser level so page
/// scripts see consistent brands/platform/architecture without a JS patch.
fn build_user_agent_metadata(fp: &Fingerprint, ua: &str) -> Option<serde_json::Value> {
    if let Some(ch) = fp.client_hints.as_ref() {
        let brands: Vec<serde_json::Value> = ch
            .brands
            .iter()
            .map(|b| serde_json::json!({"brand": b.brand.clone(), "version": b.version.clone()}))
            .collect();
        return Some(serde_json::json!({
            "brands": brands,
            "fullVersion": ch.ua_full_version,
            "platform": ch.platform,
            "platformVersion": ch.platform_version,
            "architecture": ch.architecture,
            "model": ch.model,
            "mobile": ch.mobile,
        }));
    }

    let platform = fp
        .platform
        .clone()
        .unwrap_or_else(|| fp.os_type.platform().to_owned());
    let (arch, platform_version, model, wow64) = match fp.os_type {
        OsType::Windows => ("x86", "10.0", "", fp.platform.as_deref() == Some("Win64")),
        OsType::Macos => ("arm", "14.0", "", false),
        OsType::Linux => ("x86", "", "", false),
        OsType::Android => ("arm", "14.0", "Pixel 7", true),
        OsType::Ios => ("arm", "17.4.1", "iPhone", true),
    };
    let full_version = fp
        .core_version
        .map(|v| format!("{v}.0.0.0"))
        .or_else(|| extract_chrome_version(ua))
        .unwrap_or_else(|| "133.0.0.0".to_owned());
    let major = full_version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(133);
    let brands = if ua.contains("Chrome") {
        vec![
            serde_json::json!({"brand": "Chromium", "version": major.to_string()}),
            serde_json::json!({"brand": "Google Chrome", "version": major.to_string()}),
            serde_json::json!({"brand": "Not(A:Brand", "version": "24"}),
        ]
    } else if ua.contains("Safari") && !ua.contains("Chrome") {
        vec![serde_json::json!({"brand": "Safari", "version": major.to_string()})]
    } else {
        vec![serde_json::json!({"brand": "Chromium", "version": major.to_string()})]
    };
    Some(serde_json::json!({
        "brands": brands,
        "fullVersion": full_version,
        "platform": platform,
        "platformVersion": platform_version,
        "architecture": arch,
        "model": model,
        "mobile": fp.os_type.is_mobile(),
        "wow64": wow64,
    }))
}

/// Extracts a Chrome/Chromium version such as `133.0.0.0` from a user-agent string.
fn extract_chrome_version(ua: &str) -> Option<String> {
    use regex::Regex;
    let re = Regex::new(r"Chrome/(\d+\.\d+\.\d+\.\d+)").ok()?;
    re.captures(ua)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::Fingerprint;

    #[test]
    fn bootstrap_includes_webdriver_evasion() {
        let fp = Fingerprint::windows_desktop();
        let script = bootstrap_script(&fp);
        assert!(script.contains("webdriver"));
        assert!(script.contains("Google Inc. (NVIDIA)"));
        assert!(script.contains("1920"));
    }

    #[test]
    fn launch_args_include_stealth_flags() {
        let fp = Fingerprint::windows_desktop();
        let args = launch_args(&fp);
        assert!(args.iter().any(|a| a.contains("AutomationControlled")));
        assert!(args.iter().any(|a| a.starts_with("--user-agent=")));
        assert!(args.iter().any(|a| a.starts_with("--window-size=")));
    }

    #[test]
    fn cdp_overrides_contain_screen_and_geo() {
        let fp = Fingerprint::builder()
            .screen(1920, 1080)
            .geolocation(40.0, -74.0)
            .build();
        let map = cdp_overrides(&fp);
        assert!(map.contains_key("Emulation.setDeviceMetricsOverride"));
        assert!(map.contains_key("Emulation.setGeolocationOverride"));
    }

    #[test]
    fn native_spoofing_emits_cdp_user_agent_locale() {
        let mut fp = Fingerprint::windows_desktop();
        fp.flags.native_spoofing = true;
        let map = cdp_overrides(&fp);
        let ua = map
            .get("Network.setUserAgentOverride")
            .expect("UA override");
        assert!(ua.get("userAgent").is_some());
        assert!(ua.get("acceptLanguage").is_some());
        assert!(ua.get("platform").is_some());
        assert!(ua.get("userAgentMetadata").is_some());
        assert!(map.contains_key("Emulation.setLocaleOverride"));
    }

    #[test]
    fn native_spoofing_skips_js_overrides_for_cdp_dimensions() {
        let mut fp = Fingerprint::windows_desktop();
        fp.flags.native_spoofing = true;
        let script = bootstrap_script(&fp);
        // CDP handles userAgent, platform, screen size, timezone, locale, geo.
        assert!(!script.contains("navigator.userAgent"));
        assert!(!script.contains("navigator.platform"));
        assert!(!script.contains("window.Screen.prototype, 'width'"));
        assert!(!script.contains("Intl.DateTimeFormat"));
        // Hardware properties still need JS patching.
        assert!(script.contains("hardwareConcurrency"));
    }

    #[test]
    fn ios_mobile_safari_emits_cdp_mobile_overrides() {
        let mut fp = Fingerprint::ios_mobile_safari();
        fp.flags.native_spoofing = true;
        let map = cdp_overrides(&fp);

        let metrics = map
            .get("Emulation.setDeviceMetricsOverride")
            .expect("device metrics override");
        assert_eq!(metrics.get("width").and_then(|v| v.as_u64()), Some(390));
        assert_eq!(metrics.get("height").and_then(|v| v.as_u64()), Some(844));
        assert_eq!(metrics.get("mobile").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            metrics.get("deviceScaleFactor").and_then(|v| v.as_f64()),
            Some(3.0)
        );

        let ua = map
            .get("Network.setUserAgentOverride")
            .expect("UA override");
        let meta = ua.get("userAgentMetadata").expect("userAgentMetadata");
        assert_eq!(
            meta.get("platform").and_then(|v| v.as_str()),
            Some("iPhone")
        );
        assert_eq!(
            meta.get("platformVersion").and_then(|v| v.as_str()),
            Some("17.4.1")
        );
        assert_eq!(
            meta.get("architecture").and_then(|v| v.as_str()),
            Some("arm")
        );
        assert_eq!(meta.get("model").and_then(|v| v.as_str()), Some("iPhone"));
        assert_eq!(meta.get("mobile").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn android_mobile_emits_cdp_mobile_overrides() {
        let mut fp = Fingerprint::android_mobile();
        fp.flags.native_spoofing = true;
        let map = cdp_overrides(&fp);

        let metrics = map
            .get("Emulation.setDeviceMetricsOverride")
            .expect("device metrics override");
        assert_eq!(metrics.get("width").and_then(|v| v.as_u64()), Some(412));
        assert_eq!(metrics.get("height").and_then(|v| v.as_u64()), Some(915));
        assert_eq!(metrics.get("mobile").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            metrics.get("deviceScaleFactor").and_then(|v| v.as_f64()),
            Some(3.0)
        );

        let ua = map
            .get("Network.setUserAgentOverride")
            .expect("UA override");
        let ua_str = ua.get("userAgent").and_then(|v| v.as_str()).unwrap_or("");
        assert!(ua_str.contains("Android 14"));
        let meta = ua.get("userAgentMetadata").expect("userAgentMetadata");
        assert_eq!(meta.get("mobile").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            meta.get("platform").and_then(|v| v.as_str()),
            Some("Linux armv8l")
        );
    }

    #[test]
    fn windows_desktop_native_spoofing_emits_full_cdp_stack() {
        let mut fp = Fingerprint::windows_desktop();
        fp.flags.native_spoofing = true;
        let map = cdp_overrides(&fp);

        let metrics = map
            .get("Emulation.setDeviceMetricsOverride")
            .expect("device metrics override");
        assert_eq!(metrics.get("mobile").and_then(|v| v.as_bool()), Some(false));

        let ua = map
            .get("Network.setUserAgentOverride")
            .expect("UA override");
        let meta = ua.get("userAgentMetadata").expect("userAgentMetadata");
        assert_eq!(meta.get("mobile").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(meta.get("platform").and_then(|v| v.as_str()), Some("Win32"));

        assert!(map.contains_key("Emulation.setTimezoneOverride"));
        assert!(map.contains_key("Emulation.setLocaleOverride"));
    }

    #[test]
    fn desktop_native_spoofing_drops_mobile_only_js_patches() {
        let mut fp = Fingerprint::linux_desktop();
        fp.flags.native_spoofing = true;
        let script = bootstrap_script(&fp);
        // Desktop personas must not claim touch support.
        assert!(!script.contains("maxTouchPoints = 5"));
        // CDP covers UA, platform, screen, timezone, locale.
        assert!(!script.contains("navigator.userAgent"));
        assert!(!script.contains("window.Screen.prototype, 'width'"));
    }
}
