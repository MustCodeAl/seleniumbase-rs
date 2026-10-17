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

use crate::stealth::fingerprint::{
    BrandVersion, Dimension, Fingerprint, OsType, PopupMode, QuicMode, WebRtcPolicy,
};
use crate::stealth::providers::{default_registry, session_registry, EvasionContext};
use crate::stealth::randomize::{chromium_hints, PRESET_CHROMIUM_MAJOR};

/// Generates a single combined bootstrap script for the given fingerprint.
///
/// This delegates to the [provider registry](crate::stealth::providers), which
/// assembles every applicable [`EvasionProvider`](crate::stealth::providers::EvasionProvider)
/// in priority order. The returned script is wrapped in an IIFE and modifies
/// prototype-level properties so spoofed values survive `getOwnPropertyNames`
/// scans and re-define attempts by page scripts.
///
/// This is the script for a browser that lacks `navigator.plugins`, `mimeTypes`
/// and `navigator.connection`, since it spoofs them. A session that drives a
/// real Chrome should use [`session_script`], which leaves them to the browser.
pub fn bootstrap_script(fp: &Fingerprint) -> String {
    default_registry().bootstrap(&EvasionContext::new(fp))
}

/// The script a session that drives a real Chrome runs before any page script.
///
/// It is the [session registry](crate::stealth::providers::session_registry)'s:
/// [`bootstrap_script`] without the providers for `navigator.plugins`,
/// `mimeTypes`, `pdfViewerEnabled` and `navigator.connection`, which Chrome
/// answers for itself. The pure CDP engine and the WebDriver sessions both use
/// it, so one fingerprint gives the same page-side identity in either.
#[must_use]
pub fn session_script(fp: &Fingerprint) -> String {
    session_registry(fp).bootstrap(&EvasionContext::new(fp))
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

/// Chrome switches a fingerprint payload may not set.
///
/// A payload can come from outside the program, so a switch that runs a
/// program (`--renderer-cmd-prefix` puts any command in front of every
/// renderer), loads code, hands out remote control, or sends traffic
/// somewhere the caller did not choose is refused.
const FORBIDDEN_SWITCHES: &[&str] = &[
    "renderer-cmd-prefix",
    "utility-cmd-prefix",
    "zygote-cmd-prefix",
    "gpu-launcher",
    "ppapi-plugin-launcher",
    "plugin-launcher",
    "nacl-gdb",
    "nacl-gdb-script",
    "browser-subprocess-path",
    "load-extension",
    "load-component-extension",
    "user-data-dir",
    "remote-debugging-port",
    "remote-debugging-address",
    "remote-debugging-pipe",
    "proxy-server",
    "proxy-pac-url",
    "host-resolver-rules",
    "host-rules",
    "ignore-certificate-errors",
    "ignore-certificate-errors-spki-list",
];

/// Whether a payload's `flag` (written without the leading dashes) and
/// `value` may be handed to Chrome.
///
/// The flag must be a plain switch name, the value must be one line, and the
/// switch must not be one of the refused ones.
#[must_use]
pub fn permitted_switch(flag: &str, value: &str) -> bool {
    let plain = !flag.is_empty()
        && flag.starts_with(|c: char| c.is_ascii_alphanumeric())
        && flag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain
        && !value.contains(['\n', '\r', '\0'])
        && !FORBIDDEN_SWITCHES
            .iter()
            .any(|forbidden| forbidden.eq_ignore_ascii_case(flag))
}

/// The arguments for a payload's switches, in a fixed order, with the refused
/// ones left out.
#[must_use]
pub fn switches_to_args<'a>(switches: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut kept: Vec<(&str, &str)> = switches
        .into_iter()
        .filter(|(flag, value)| {
            let ok = permitted_switch(flag, value);
            if !ok {
                tracing::warn!(flag, "refusing a Chrome switch from a fingerprint payload");
            }
            ok
        })
        .collect();
    kept.sort_unstable();
    kept.into_iter()
        .map(|(flag, value)| {
            if value.is_empty() {
                format!("--{flag}")
            } else {
                format!("--{flag}={value}")
            }
        })
        .collect()
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

    if fp.is_masked(Dimension::Navigator) {
        if let Some(ua) = fp.user_agent.as_deref() {
            args.push(format!("--user-agent={ua}"));
        }
    }

    if fp.is_masked(Dimension::Localization) {
        if let Some(locale) = fp.locale.as_deref() {
            args.push(format!("--lang={locale}"));
        }
    }

    if fp.is_masked(Dimension::Screen) {
        if let (Some(w), Some(h)) = (fp.screen_width, fp.screen_height) {
            args.push(format!("--window-size={w},{h}"));
        }
    }

    // The policy flag is only passed when WebRTC is masked, which a policy
    // or an address makes it: a fingerprint without either leaves it alone.
    if fp.is_masked(Dimension::WebRtc) {
        let policy = fp.webrtc_policy.unwrap_or_default();
        args.push(format!(
            "--force-webrtc-ip-handling-policy={}",
            webrtc_policy_value(policy)
        ));
    }

    if fp.is_masked(Dimension::Proxy) {
        if let Some(proxy) = fp.proxy.as_ref() {
            args.push(format!("--proxy-server={}", proxy.to_url()));
        }
    }

    match fp.flags.quic_mode {
        QuicMode::Disabled | QuicMode::ForceHttp2 => args.push("--disable-quic".to_owned()),
        QuicMode::Enabled => args.push("--enable-quic".to_owned()),
        QuicMode::Natural | QuicMode::Auto => {}
    }

    args.extend(switches_to_args(
        fp.cmd_params.iter().map(|(f, v)| (f.as_str(), v.as_str())),
    ));

    args
}

/// The value of Chrome's `--force-webrtc-ip-handling-policy` for `policy`.
fn webrtc_policy_value(policy: WebRtcPolicy) -> &'static str {
    match policy {
        WebRtcPolicy::DisableNonProxiedUdp => "disable_non_proxied_udp",
        WebRtcPolicy::PublicInterfaceOnly => "default_public_interface_only",
        WebRtcPolicy::PublicAndPrivateInterfaces => "default_public_and_private_interfaces",
    }
}

/// The command that would add a `Proxy-Authorization` header to every request.
///
/// The deprecated [`cdp_overrides`] contains it for a fingerprint whose proxy
/// has a user name and a password. A header added this way goes to every site
/// the browser visits, which hands the proxy's login to the sites; the proxy is
/// answered through its own challenge instead. [`safe_cdp_overrides`] leaves it
/// out.
pub const CREDENTIAL_HEADER_METHOD: &str = "Network.setExtraHTTPHeaders";

/// The overrides from [`cdp_overrides`] that are safe to send to a browser, in
/// the order of their command names.
///
/// Both ways of giving a browser a fingerprint, the pure CDP engine and the
/// WebDriver session, send these and never the raw overrides. The map has no
/// order; sorting makes the commands, and so any trace of them, the same on
/// every run.
pub fn safe_cdp_overrides(fp: &Fingerprint) -> Vec<(String, serde_json::Value)> {
    let mut overrides: Vec<(String, serde_json::Value)> = raw_overrides(fp)
        .into_iter()
        .filter(|(method, _)| method != CREDENTIAL_HEADER_METHOD)
        .collect();
    overrides.sort_by(|a, b| a.0.cmp(&b.0));
    overrides
}

/// Returns CDP parameter overrides recommended for a fingerprint.
///
/// Do not send these as they are: see [`CREDENTIAL_HEADER_METHOD`]. Use
/// [`safe_cdp_overrides`], which leaves out the header that hands the proxy's
/// login to every site.
#[deprecated(note = "use safe_cdp_overrides, which leaves out the Proxy-Authorization header")]
#[must_use]
pub fn cdp_overrides(fp: &Fingerprint) -> HashMap<String, serde_json::Value> {
    raw_overrides(fp)
}

/// Every override the fingerprint asks for, the proxy header included.
fn raw_overrides(fp: &Fingerprint) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();

    if let (Some(lat), Some(lon), true) = (
        fp.latitude,
        fp.longitude,
        fp.is_masked(Dimension::Geolocation),
    ) {
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

    if let (Some(w), Some(h), true) = (
        fp.screen_width,
        fp.screen_height,
        fp.is_masked(Dimension::Screen),
    ) {
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

    if let (Some(ua), true) = (fp.user_agent.as_deref(), fp.is_masked(Dimension::Navigator)) {
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

    if let (Some(locale), true) = (fp.locale.as_deref(), fp.is_masked(Dimension::Localization)) {
        map.insert(
            "Emulation.setLocaleOverride".to_owned(),
            serde_json::json!({ "locale": locale }),
        );
    }

    if let (Some(tz), true) = (fp.timezone.as_deref(), fp.is_masked(Dimension::Timezone)) {
        map.insert(
            "Emulation.setTimezoneOverride".to_owned(),
            serde_json::json!({ "timezoneId": tz }),
        );
    }

    // The geolocation override only delivers its position to a page that may
    // ask for it; with the popup allowed nobody is there to press "Allow", so
    // the permission is granted up front.
    let mut permissions: Vec<&str> = Vec::new();
    if fp.latitude.is_some()
        && fp.longitude.is_some()
        && fp.is_masked(Dimension::Geolocation)
        && fp.geolocation_popup() == PopupMode::Allow
    {
        permissions.push("geolocation");
    }
    if fp.flags.grant_permissions {
        permissions.extend([
            "notifications",
            "midi",
            "midiSysex",
            "clipboardReadWrite",
            "clipboardSanitizedWrite",
            "paymentHandler",
            "backgroundSync",
            "idleDetection",
            "webAppInstallation",
        ]);
    }
    if !permissions.is_empty() {
        map.insert(
            "Browser.grantPermissions".to_owned(),
            serde_json::json!({ "permissions": permissions }),
        );
    }

    // One command carries the tracker hosts and the blocked ports: a second
    // `Network.setBlockedURLs` would replace the first.
    let mut blocked: Vec<String> = Vec::new();
    if fp.is_masked(Dimension::Trackers) {
        blocked.extend(fp.tracker_hosts().iter().map(|h| format!("*{h}*")));
    }
    blocked.extend(blocked_port_patterns(fp));
    if !blocked.is_empty() {
        map.insert(
            "Network.setBlockedURLs".to_owned(),
            serde_json::json!({ "urls": blocked }),
        );
    }

    if fp.flags.disable_csp {
        map.insert(
            "Page.setBypassCSP".to_owned(),
            serde_json::json!({ "enabled": true }),
        );
    }

    if fp.is_masked(Dimension::Headless) {
        map.insert(
            "Emulation.setFocusEmulationEnabled".to_owned(),
            serde_json::json!({ "enabled": true }),
        );
    }

    if let (Some(proxy), true) = (fp.proxy.as_ref(), fp.is_masked(Dimension::Proxy)) {
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

/// The loopback names a blocked port is refused on.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// `Network.setBlockedURLs` patterns that refuse the fingerprint's blocked
/// ports on every loopback name, so a page cannot probe local services (or the
/// browser's own debugging port) through them. Empty when no port is set or
/// the ports mode is `Natural` or `Off`.
fn blocked_port_patterns(fp: &Fingerprint) -> Vec<String> {
    if !fp.is_masked(Dimension::Ports) {
        return Vec::new();
    }
    fp.ports
        .iter()
        .flat_map(|port| {
            LOOPBACK_HOSTS
                .iter()
                .map(move |host| format!("*://{host}:{port}/*"))
        })
        .collect()
}

/// Builds a `userAgentMetadata` object for `Network.setUserAgentOverride` from
/// the fingerprint. This populates Client Hints at the browser level so page
/// scripts see consistent brands/platform/architecture without a JS patch.
fn build_user_agent_metadata(fp: &Fingerprint, ua: &str) -> Option<serde_json::Value> {
    if let Some(ch) = fp.client_hints.as_ref() {
        let mut metadata = serde_json::json!({
            "brands": brand_list(&ch.brands),
            "fullVersion": ch.ua_full_version,
            "platform": ch.platform,
            "platformVersion": ch.platform_version,
            "architecture": ch.architecture,
            "model": ch.model,
            "mobile": ch.mobile,
        });
        if !ch.full_version_list.is_empty() {
            metadata["fullVersionList"] = brand_list(&ch.full_version_list);
        }
        return Some(metadata);
    }

    // The fingerprint has no Client Hints, as the presets have none: derive
    // them from the user agent.
    //
    // `platform` is what `navigator.userAgentData.platform` and
    // `Sec-CH-UA-Platform` report, which is not `navigator.platform`: Chrome
    // on Windows says `Windows` there, never `Win32`.
    let platform = client_hints_platform(fp.os_type);
    // The platform versions in the format Chrome reports them, as in the tables
    // `Fingerprint::randomized` draws from (`stealth::randomize`): `10.0.0`,
    // the value Microsoft documents for Windows 10 2004 to 22H2; macOS 27.0.1
    // as Chrome 155 reported it on a Mac; Android 14 as `14.0.0`; and iOS 26.0,
    // the release of the Safari 26.0 preset.
    let (arch, platform_version, model, wow64) = match fp.os_type {
        OsType::Windows => ("x86", "10.0.0", "", fp.platform.as_deref() == Some("Win64")),
        OsType::Macos => ("arm", "27.0.1", "", false),
        OsType::Linux => ("x86", "", "", false),
        OsType::Android => ("arm", "14.0.0", "Pixel 7", true),
        OsType::Ios => ("arm", "26.0", "iPhone", true),
    };
    let (brands, full_version_list, full_version) = if ua.contains("Chrome") {
        // The brand list, decoy brand and full version Chrome `major` sends.
        let major = fp
            .core_version
            .or_else(|| version_after(ua, "Chrome/").and_then(|v| major_of(&v)))
            .unwrap_or(PRESET_CHROMIUM_MAJOR);
        let hints = chromium_hints(major);
        (
            brand_list(&hints.brands),
            brand_list(&hints.full_version_list),
            hints.ua_full_version,
        )
    } else {
        // Safari names its release after `Version/`, such as `Version/26.0`.
        let (brand, version) = if ua.contains("Safari") {
            ("Safari", version_after(ua, "Version/"))
        } else {
            ("Chromium", version_after(ua, "Chrome/"))
        };
        let version = version.unwrap_or_else(|| format!("{PRESET_CHROMIUM_MAJOR}.0.0.0"));
        let major = major_of(&version).unwrap_or(PRESET_CHROMIUM_MAJOR);
        (
            serde_json::json!([{"brand": brand, "version": major.to_string()}]),
            serde_json::json!([{"brand": brand, "version": version.clone()}]),
            version,
        )
    };
    Some(serde_json::json!({
        "brands": brands,
        "fullVersionList": full_version_list,
        "fullVersion": full_version,
        "platform": platform,
        "platformVersion": platform_version,
        "architecture": arch,
        "model": model,
        "mobile": fp.os_type.is_mobile(),
        "wow64": wow64,
    }))
}

/// A brand list in the shape `Emulation.UserAgentMetadata` takes.
fn brand_list(brands: &[BrandVersion]) -> serde_json::Value {
    brands
        .iter()
        .map(|b| serde_json::json!({"brand": b.brand.clone(), "version": b.version.clone()}))
        .collect()
}

/// The platform name Client Hints report for `os` (`Sec-CH-UA-Platform`).
fn client_hints_platform(os: OsType) -> &'static str {
    match os {
        OsType::Windows => "Windows",
        OsType::Macos => "macOS",
        OsType::Linux => "Linux",
        OsType::Android => "Android",
        OsType::Ios => "iOS",
    }
}

/// The dotted version after `product` in a user agent, such as `133.0.0.0`
/// after `Chrome/` or `26.0` after `Version/`.
fn version_after(ua: &str, product: &str) -> Option<String> {
    let rest = &ua[ua.find(product)? + product.len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let version = rest[..end].trim_end_matches('.');
    major_of(version).map(|_| version.to_owned())
}

/// The major version of a dotted version, such as `133` of `133.0.0.0`.
fn major_of(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::{Fingerprint, NoiseMode, PopupMode};

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
    fn payload_switches_that_run_programs_are_refused() {
        let args = switches_to_args([
            ("renderer-cmd-prefix", "/bin/sh -c evil"),
            ("RENDERER-CMD-PREFIX", "x"),
            ("load-extension", "/tmp/ext"),
            ("proxy-server", "http://elsewhere"),
            ("user-data-dir", "/tmp/x"),
            ("--lang", "de"),
            ("two words", ""),
            ("force-color-profile", "srgb\n--no-sandbox"),
            ("lang", "de-DE"),
            ("disable-quic", ""),
        ]);
        assert_eq!(args, vec!["--disable-quic", "--lang=de-DE"]);
    }

    #[test]
    fn payload_switches_come_out_in_a_fixed_order() {
        let first = switches_to_args([("b-flag", "1"), ("a-flag", ""), ("c-flag", "3")]);
        let second = switches_to_args([("c-flag", "3"), ("a-flag", ""), ("b-flag", "1")]);
        assert_eq!(first, second);
        assert_eq!(first, vec!["--a-flag", "--b-flag=1", "--c-flag=3"]);
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

    /// The names in the `Browser.grantPermissions` override, if there is one.
    fn granted(fp: &Fingerprint) -> Option<Vec<String>> {
        let map = cdp_overrides(fp);
        let params = map.get("Browser.grantPermissions")?;
        Some(
            params["permissions"]
                .as_array()?
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect(),
        )
    }

    /// The URL patterns in the `Network.setBlockedURLs` override, if any.
    fn blocked_urls(fp: &Fingerprint) -> Option<Vec<String>> {
        let map = cdp_overrides(fp);
        let params = map.get("Network.setBlockedURLs")?;
        Some(
            params["urls"]
                .as_array()?
                .iter()
                .filter_map(|url| url.as_str().map(str::to_owned))
                .collect(),
        )
    }

    #[test]
    fn blocked_ports_are_refused_on_every_loopback_name() {
        let fp = Fingerprint::builder()
            .blocked_ports(vec![9222, 8080])
            .build();
        let urls = blocked_urls(&fp).expect("a block list");
        for host in ["localhost", "127.0.0.1", "[::1]"] {
            for port in [9222, 8080] {
                let pattern = format!("*://{host}:{port}/*");
                assert!(urls.contains(&pattern), "{pattern} in {urls:?}");
            }
        }
        assert_eq!(urls.len(), 6, "{urls:?}");
    }

    #[test]
    fn blocked_ports_and_trackers_share_one_command() {
        let mut fp = Fingerprint::builder().blocked_ports(vec![9222]).build();
        fp.flags.block_trackers = true;
        let urls = blocked_urls(&fp).expect("a block list");
        assert!(
            urls.contains(&"*://localhost:9222/*".to_owned()),
            "{urls:?}"
        );
        assert!(urls.contains(&"*doubleclick.net*".to_owned()), "{urls:?}");
    }

    #[test]
    fn a_natural_ports_mode_blocks_nothing() {
        let mut fp = Fingerprint::builder().blocked_ports(vec![9222]).build();
        for mode in [NoiseMode::Natural, NoiseMode::Off] {
            fp.flags.ports_masking = mode;
            assert_eq!(blocked_urls(&fp), None, "{mode:?}");
        }
        assert_eq!(blocked_urls(&Fingerprint::default()), None);
    }

    #[test]
    fn an_allowed_geolocation_popup_grants_the_geolocation_permission() {
        // Without the grant, the position the override sets is never delivered:
        // the page waits on a permission prompt nobody can answer.
        let mut fp = Fingerprint::builder().geolocation(40.0, -74.0).build();
        fp.flags.geolocation_popup = PopupMode::Allow;
        assert_eq!(granted(&fp), Some(vec!["geolocation".to_owned()]));

        // The other permissions are still the flag's.
        fp.flags.grant_permissions = true;
        let names = granted(&fp).expect("a grant");
        assert!(names.contains(&"geolocation".to_owned()), "{names:?}");
        assert!(names.contains(&"notifications".to_owned()), "{names:?}");
    }

    #[test]
    fn a_geolocation_popup_that_is_not_allowed_grants_nothing() {
        let mut fp = Fingerprint::builder().geolocation(40.0, -74.0).build();
        for popup in [PopupMode::Prompt, PopupMode::Block] {
            fp.flags.geolocation_popup = popup;
            assert_eq!(granted(&fp), None, "{popup:?}");
        }
        // Allow without coordinates has nothing to deliver.
        let mut fp = Fingerprint::default();
        fp.flags.geolocation_popup = PopupMode::Allow;
        assert_eq!(granted(&fp), None);
    }

    #[test]
    fn granted_permissions_use_names_the_protocol_knows() {
        let mut fp = Fingerprint::windows_desktop();
        fp.flags.grant_permissions = true;

        let map = cdp_overrides(&fp);

        let names: Vec<&str> = map["Browser.grantPermissions"]["permissions"]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        // Chrome rejects the whole grant if one name is unknown, and it does
        // not know `clipboardRead` or `clipboardWrite` (only the combined one).
        assert!(names.contains(&"clipboardReadWrite"), "{names:?}");
        for wrong in ["clipboardRead", "clipboardWrite"] {
            assert!(
                !names.contains(&wrong),
                "{wrong} is not a protocol permission"
            );
        }
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
            Some("26.0")
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
        // Chrome on Android freezes the release at 10 and hides the model.
        assert!(ua_str.contains("Linux; Android 10; K"), "{ua_str}");
        // `navigator.platform` stays what an Android phone reports there...
        assert_eq!(
            ua.get("platform").and_then(|v| v.as_str()),
            Some("Linux armv8l")
        );
        let meta = ua.get("userAgentMetadata").expect("userAgentMetadata");
        assert_eq!(meta.get("mobile").and_then(|v| v.as_bool()), Some(true));
        // ...while the Client Hints name the platform the way Chrome does.
        assert_eq!(
            meta.get("platform").and_then(|v| v.as_str()),
            Some("Android")
        );
        assert_eq!(
            meta.get("platformVersion").and_then(|v| v.as_str()),
            Some("14.0.0")
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
        // Chrome on Windows reports `Windows` in its Client Hints; `Win32` is
        // only `navigator.platform`.
        assert_eq!(
            meta.get("platform").and_then(|v| v.as_str()),
            Some("Windows")
        );
        assert_eq!(ua.get("platform").and_then(|v| v.as_str()), Some("Win32"));
        assert_eq!(
            meta.get("platformVersion").and_then(|v| v.as_str()),
            Some("10.0.0")
        );

        assert!(map.contains_key("Emulation.setTimezoneOverride"));
        assert!(map.contains_key("Emulation.setLocaleOverride"));
    }

    /// The `userAgentMetadata` a fingerprint without Client Hints gets.
    fn fallback_metadata(fp: &Fingerprint) -> serde_json::Value {
        assert!(fp.client_hints.is_none(), "the fallback is for no hints");
        let ua = fp.user_agent.as_deref().expect("a user agent");
        build_user_agent_metadata(fp, ua).expect("metadata")
    }

    fn brands_of(list: &serde_json::Value) -> Vec<(String, String)> {
        list.as_array()
            .expect("a list")
            .iter()
            .map(|b| {
                (
                    b["brand"].as_str().unwrap_or_default().to_owned(),
                    b["version"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    }

    fn pairs(list: &[BrandVersion]) -> Vec<(String, String)> {
        list.iter()
            .map(|b| (b.brand.clone(), b.version.clone()))
            .collect()
    }

    #[test]
    fn the_fallback_sends_the_brands_and_full_versions_of_the_claimed_major() {
        let fp = Fingerprint::macos_desktop();
        let meta = fallback_metadata(&fp);
        let hints = chromium_hints(PRESET_CHROMIUM_MAJOR);
        assert_eq!(brands_of(&meta["brands"]), pairs(&hints.brands));
        assert_eq!(
            brands_of(&meta["fullVersionList"]),
            pairs(&hints.full_version_list)
        );
        assert_eq!(meta["fullVersion"], hints.ua_full_version.as_str());
        assert!(
            !hints.ua_full_version.ends_with(".0.0.0"),
            "a real build, not the frozen {}",
            hints.ua_full_version
        );
        assert_eq!(meta["platform"], "macOS");
        assert_eq!(meta["platformVersion"], "27.0.1");
    }

    #[test]
    fn the_fallback_decoy_brand_follows_the_major_instead_of_being_fixed() {
        // Chrome 155 sends `Not(A:Brand` (measured); another major sends
        // another decoy, so a fixed one gives every other major away.
        let mut fp = Fingerprint::windows_desktop();
        fp.user_agent = Some(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
                .to_owned(),
        );
        let measured = brands_of(&fallback_metadata(&fp)["brands"]);
        assert_eq!(
            measured,
            [
                ("Google Chrome".to_owned(), "155".to_owned()),
                ("Chromium".to_owned(), "155".to_owned()),
                ("Not(A:Brand".to_owned(), "24".to_owned()),
            ]
        );
        fp.core_version = Some(156);
        let newer = brands_of(&fallback_metadata(&fp)["brands"]);
        assert_eq!(newer, pairs(&chromium_hints(156).brands));
        assert!(newer.iter().all(|(brand, _)| brand != "Not(A:Brand"));
        assert_eq!(fallback_metadata(&fp)["fullVersion"], "156.0.8078.35");
    }

    #[test]
    fn the_fallback_names_safari_by_its_version_token() {
        let mut fp = Fingerprint::ios_mobile_safari();
        fp.client_hints = None;
        let meta = fallback_metadata(&fp);
        assert_eq!(
            brands_of(&meta["brands"]),
            [("Safari".to_owned(), "26".to_owned())]
        );
        assert_eq!(meta["fullVersion"], "26.0");
        assert_eq!(meta["platform"], "iOS");
    }

    #[test]
    fn client_hints_with_a_full_version_list_pass_it_on() {
        let fp = Fingerprint::randomized(OsType::Macos, 3);
        let hints = fp.client_hints.clone().expect("hints");
        let ua = fp.user_agent.clone().expect("a user agent");
        let meta = build_user_agent_metadata(&fp, &ua).expect("metadata");
        assert_eq!(
            brands_of(&meta["fullVersionList"]),
            pairs(&hints.full_version_list)
        );
        assert_eq!(meta["fullVersion"], hints.ua_full_version.as_str());
    }

    #[test]
    fn versions_are_read_after_their_product_token() {
        let chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                      Chrome/156.0.0.0 Safari/537.36";
        assert_eq!(
            version_after(chrome, "Chrome/").as_deref(),
            Some("156.0.0.0")
        );
        assert_eq!(version_after(chrome, "Version/"), None);
        assert_eq!(
            version_after("x Version/26.0 Mobile/15E148", "Version/").as_deref(),
            Some("26.0")
        );
        assert_eq!(version_after("Chrome/ nothing", "Chrome/"), None);
        assert_eq!(major_of("155.0.8059.40"), Some(155));
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
