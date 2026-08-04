# Fingerprint & Stealth Profiles

Websites detect automation by looking at many browser signals: `navigator.webdriver`,
user agent, platform, screen size, hardware concurrency, WebGL vendor, canvas
noise, audio noise, time zone, geolocation, media devices, fonts, and more.
`seleniumbase-rs` can spoof these signals through a coherent `Fingerprint` and a
pluggable evasion system.

This page explains how to use built-in fingerprint presets, build custom
profiles, choose masking modes, and apply native-level spoofing through CDP.

## What you will learn

- How to use built-in fingerprint presets.
- How to build a custom `Fingerprint`.
- How `StealthFlags` masking modes work.
- How to enable native-level CDP spoofing.
- How the evasion provider registry assembles the JavaScript bootstrap.

## Quick start

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, Fingerprint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::windows_desktop();
    let config = BrowserConfig {
        mode: DriverMode::Uc,
        fingerprint: Some(fp),
        ..BrowserConfig::default()
    };

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

## Fingerprint presets

Built-in presets set a coherent combination of user agent, platform, screen,
WebGL strings, and masking flags:

| Preset | Best for |
|---|---|---|
| `Fingerprint::windows_desktop()` | Chrome on Windows. |
| `Fingerprint::macos_desktop()` | Chrome or Safari on macOS. |
| `Fingerprint::linux_desktop()` | Chrome on Linux. |
| `Fingerprint::android_mobile()` | Chrome on Android. |
| `Fingerprint::ios_mobile_safari()` | Mobile Safari on iOS. |

Use the `fingerprint!` macro for shorter syntax:

```rust
use seleniumbase_rs::fingerprint;

let fp = fingerprint!(windows);
let fp = fingerprint!(macos);
let fp = fingerprint!(linux);
let fp = fingerprint!(android);
let fp = fingerprint!(ios);
```

## Building a custom fingerprint

For full control, use the builder:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...")
    .platform("Win32")
    .screen(1920, 1080)
    .hardware_concurrency(8)
    .device_memory(8.0)
    .locale("en-US")
    .timezone("America/New_York")
    .webgl("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 ...)")
    .geolocation(40.7128, -74.0060)
    .media_devices(1, 1, 2)
    .build();
```

## Masking modes

`StealthFlags` controls which dimensions are spoofed. Every flag is one of the
following modes:

| Mode | Meaning | When to use |
|---|---|---|
| `Natural` | Use the browser's real value. | Trust the host for audio, media devices, or fonts. |
| `Mask` | Apply a generic, deterministic spoofed value. | Hide the real WebRTC IP policy, screen size, or timezone. |
| `Custom` | Use the explicit value supplied in the `Fingerprint`. | Set a specific user agent, screen resolution, proxy, or geolocation. |
| `Disabled` | Turn the feature off entirely. | Disable WebRTC, block QUIC, or leave proxy unconfigured. |

Use `StealthFlags::balanced()` for sensible defaults or `StealthFlags::all_custom()`
when every value is supplied explicitly.

## Stealth flag reference

| Flag | Accepted modes / literals | Default |
|---|---|---|
| `navigator_masking` | `natural` / `mask` / `custom` / `disabled`, or a full UA string | `mask` |
| `screen_masking` | `natural` / `mask` / `custom` / `disabled`, or `WIDTHxHEIGHTxDEPTH` | `mask` |
| `graphics_masking` | `natural` / `mask` / `custom` / `disabled`, or `vendor~renderer` | `mask` |
| `audio_masking` | `natural` / `mask` / `custom` / `disabled` | `natural` |
| `media_devices_masking` | `natural` / `mask` / `custom` / `disabled`, or `kind:deviceId:label\|...` | `natural` |
| `canvas_noise` | `random` / `persistent` / `low` / `natural` / `disabled` | `mask` |
| `webrtc_masking` | `natural` / `mask` / `custom` / `disabled` / `public_only`, or `public:IP\|local:IP` | `mask` |
| `geolocation_masking` | `natural` / `auto` / `mask` / `custom` / `disabled`, or `lat,lon,alt` | `mask` |
| `timezone_masking` | `natural` / `auto` / `mask` / `custom` / `disabled`, or an IANA zone | `mask` |
| `localization_masking` | `natural` / `auto` / `mask` / `custom` / `disabled`, or a locale string | `mask` |
| `fonts_masking` | `natural` / `mask` / `custom` / `disabled`, or a comma-separated font list | `mask` |
| `ports_masking` | `natural` / `off` / `mask` / `block` / `block_all` / `whitelist`, or a port list | `mask` |
| `proxy_masking` | `disabled` / `direct` / `custom` / `socks5` / `http` / `https`, or a proxy URL | `disabled` |
| `quic_mode` | `enabled` / `disabled` / `force_http2` / `auto` | `disabled` |
| `graphics_noise` | `low` / `medium` / `high` / `off` / `natural` / `mask` | `mask` |
| `battery_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `connection_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `speech_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `bluetooth_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `client_hints_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `native_tostring_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `chrome_runtime_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `headless_masking` | `natural` / `mask` / `custom` / `disabled` | `mask` |
| `humanize` | bool | `false` |
| `block_trackers` | bool | `false` |
| `disable_csp` | bool | `false` |
| `grant_permissions` | bool | `false` |
| `native_spoofing` | bool | `false` |

### Literal values in JSON payloads

When a profile is imported through `ProfileParams`, many flags accept a concrete
string literal. The parser detects the literal, sets the mode to `Custom`, and
writes the value into the matching `Fingerprint` field:

| Flag | Literal example | Populated fingerprint field |
|---|---|---|
| `screen_masking` | `"1920x1080x24"` | `screen_width`, `screen_height`, `color_depth` |
| `geolocation_masking` | `"40.7128,-74.0060,10.5"` | `latitude`, `longitude`, `altitude` |
| `graphics_masking` | `"Intel Inc.~Intel(R) Iris(R) Xe Graphics"` | `webgl_vendor`, `webgl_renderer` |
| `navigator_masking` | full UA string | `user_agent` |
| `localization_masking` | `"en-US,en;q=0.9"` | `locale`, `languages`, `accept_languages` |
| `timezone_masking` | `"America/New_York"` | `timezone` |
| `fonts_masking` | `"Arial,Calibri,Consolas"` | `fonts` |
| `media_devices_masking` | `"audioinput:default:Mic1\|videoinput:default:Cam1"` | `media_devices` |
| `ports_masking` | `"block:80,443,3000"` | `ports` |
| `proxy_masking` | `"socks5://user:pass@host:1080"` | `proxy` |
| `webrtc_masking` | `"public:172.56.21.89\|local:10.0.0.5"` | `webrtc_public_ip`, `webrtc_local_ip` |

### Concrete mask-mode examples

#### Custom navigator (`navigator_masking: Custom`)

```rust
use seleniumbase_rs::{Fingerprint, StealthFlags};

let mut fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
    .platform("Win32")
    .hardware_concurrency(8)
    .device_memory(8.0)
    .build();
fp.flags.navigator_masking = seleniumbase_rs::MaskingMode::Custom;
```

When the bootstrap runs, it overrides `navigator.userAgent`, `navigator.platform`,
`hardwareConcurrency`, and `deviceMemory` with the values you supplied.

#### Custom screen + geolocation (`screen_masking` / `geolocation_masking: Custom`)

```rust
use seleniumbase_rs::{Fingerprint, StealthFlags};

let mut fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation(52.52, 13.405)
    .timezone("Europe/Berlin")
    .build();
fp.flags.screen_masking = seleniumbase_rs::MaskingMode::Custom;
fp.flags.geolocation_masking = seleniumbase_rs::MaskingMode::Custom;
fp.flags.timezone_masking = seleniumbase_rs::MaskingMode::Mask;
```

The launcher resizes the browser window to 1920×1080, overrides the geolocation
via CDP, and the bootstrap spoofs `Intl.DateTimeFormat` to Berlin time.

#### Custom proxy (`proxy_masking: Custom`)

```rust
use seleniumbase_rs::{Fingerprint, ProxyMaskingMode};

let mut fp = Fingerprint::default();
fp.proxy = Some("http://alice:secret@proxy.example.com:8080".parse().unwrap());
fp.flags.proxy_masking = ProxyMaskingMode::Custom;
```

`ProxyMaskingMode::Custom` emits `--proxy-server=http://alice:secret@proxy.example.com:8080`.
Use `ProxyMaskingMode::Disabled` to leave proxy configuration empty.

#### WebRTC masked

```rust
use seleniumbase_rs::{Fingerprint, MaskingMode, WebRtcPolicy};

let mut fp = Fingerprint::default();
fp.flags.webrtc_masking = MaskingMode::Mask;
fp.webrtc_policy = WebRtcPolicy::DisableNonProxiedUdp;
```

This adds `--force-webrtc-ip-handling-policy=disable_non_proxied_udp` so WebRTC
cannot leak the local IP address. Set `webrtc_masking: Disabled` to omit the
flag and let the browser use its default policy.

## Native-level (CDP) spoofing

By default, most spoofed values are injected through JavaScript providers. This
works everywhere, but page scripts can *in principle* detect the patch through
`Function.prototype.toString`, `Object.getOwnPropertyNames`, or descriptor
inspection.

Enable `StealthFlags::native_spoofing` to move every spoofing dimension that
can be driven by the Chrome DevTools Protocol (CDP) or Chromium launch args out
of JavaScript and into the browser's own implementation:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …")
    .platform("Win32")
    .screen(1920, 1080)
    .locale("en-US")
    .timezone("America/New_York")
    .geolocation(40.7580, -73.9855)
    .native_spoofing(true)
    .build();
```

When `native_spoofing` is true the framework:

* Sends `Network.setUserAgentOverride` with `userAgent`, `acceptLanguage`,
  `platform`, and `userAgentMetadata` so `navigator.userAgent`,
  `navigator.platform`, `navigator.languages`, and Client Hints come from the
  browser itself.
* Sends `Emulation.setDeviceMetricsOverride` so `screen.width/height`,
  `window.outerWidth/Height`, and `devicePixelRatio` are native values.
* Sends `Emulation.setTimezoneOverride` and `Emulation.setLocaleOverride` for
  time-zone and localization semantics.
* Sends `Emulation.setGeolocationOverride` for `navigator.geolocation`.
* Keeps JS providers only for dimensions that have no CDP equivalent:
  `hardwareConcurrency`, `deviceMemory`, WebGL vendor/renderer, canvas/audio
  noise, media devices, fonts, battery, connection, speech, Bluetooth, and
  `navigator.webdriver` cleanup.

Because the browser returns the fake values directly, there is no JS-visible
override to inspect. `toString()`, `getOwnPropertyNames()`, and descriptor
probes see the native getter/property, making the spoof effectively invisible
to page-side introspection.

### Caveats

* Native spoofing requires a CDP session. In pure WebDriver fallback mode the
  framework falls back to the JS providers.
* `colorDepth` / `pixelDepth` are not covered by CDP device metrics, so a
  minimal JS patch is still emitted for them.
* Some signals (WebGL, canvas noise, audio noise) have no CDP override and
  remain JS-based even in native mode.

## rustwright / Playwright mode

```rust
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::windows_desktop();
let session = PlaywrightSession::launch_with_fingerprint(&fp).await?;
session.goto("https://example.com").await?;
```

## External profile payloads

The `profile_payloads::ProfileParams` type maps to a `Fingerprint` via
`ProfileParams::to_fingerprint()`. `ProfileParams::to_browser_config()` attaches
the fingerprint to the returned `BrowserConfig`, so the spoofed values are
applied automatically at launch.

## Provider architecture

Every evasion is an [`EvasionProvider`](crate::EvasionProvider): a small,
self-contained unit that returns a JavaScript snippet for an
[`EvasionContext`](crate::EvasionContext) (the session fingerprint plus a
deterministic seed and runtime config). Providers are held by an
[`EvasionRegistry`](crate::EvasionRegistry) and assembled into a single
bootstrap script in priority order (lower `priority()` runs first).

```rust
use seleniumbase_rs::{default_registry, EvasionContext, Fingerprint};

let fp = Fingerprint::windows_desktop();
let ctx = EvasionContext::new(&fp);            // seed derived from the fingerprint
let script = default_registry().bootstrap(&ctx);
assert!(script.contains("webdriver"));
```

`evasions::bootstrap_script(&fp)` is a thin wrapper over
`default_registry().bootstrap(&EvasionContext::new(&fp))`, so existing callers
keep working while new evasions are added centrally.

### Adding your own evasion

1. Implement `EvasionProvider` for a unit struct. Return the JavaScript from
   `script`, gate inclusion with `applies`, and order it with `priority`.
2. Register it at runtime:

```rust
use seleniumbase_rs::{default_registry, EvasionContext, EvasionProvider, Fingerprint};

struct HidePrint;
impl EvasionProvider for HidePrint {
    fn name(&self) -> &str { "hide_print" }
    fn priority(&self) -> i32 { 140 }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some("window.print = function() {};".to_owned())
    }
}

let mut registry = default_registry();
registry.register_provider(Box::new(HidePrint));
let fp = Fingerprint::windows_desktop();
let script = registry.bootstrap(&EvasionContext::new(&fp));
assert!(script.contains("hide_print"));
```

See [Contributing](../CONTRIBUTING.md) for adding a built-in provider.

### Built-in providers

Registered by `default_registry()`, in execution order:

| Priority | Name | Spoofs |
|---|---|---|
| 5 | `native_tostring` | `Function.prototype.toString` returns `[native code]` for patched functions |
| 10 | `webdriver` | `navigator.webdriver`, `callPhantom`, `__selenium`, `__driver` markers |
| 15 | `cdp_markers` | scrubs `$cdc_`, `__webdriver`, `__driver` properties |
| 20 | `chrome_runtime` | `window.chrome` `runtime` / `app` / `csi` / `loadTimes` |
| 30 | `navigator_props` | userAgent, platform, languages, hardwareConcurrency, deviceMemory, vendor, oscpu, productSub, buildID, maxTouchPoints |
| 35 | `permissions` | `Notification.permission`, `permissions.query` |
| 40 | `plugins` | five PDF-viewer plugins and mimeTypes |
| 45 | `window_geometry` | screen/window dimensions, `devicePixelRatio` |
| 50 | `webgl` | WebGL1/WebGL2 `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL` |
| 55 | `canvas_noise` | deterministic per-session noise on `toDataURL` / `toBlob` / `getImageData` |
| 60 | `audio_noise` | deterministic noise on `AudioBuffer.getChannelData` and `AnalyserNode.getFloatFrequencyData` |
| 65 | `webrtc` | filters STUN/TURN servers and rewrites SDP / `RTCIceCandidate` IPs when custom IPs are configured |
| 70 | `battery` | Battery Status API |
| 72 | `connection` | Network Information API (`effectiveType`, `rtt`, `downlink`) |
| 75 | `media_devices` | `enumerateDevices` audioinput/audiooutput/videoinput (supports explicit labels) |
| 76 | `fonts` | restricts `document.fonts.load` / `check` / `FontFace` to the configured font list |
| 78 | `speech` | `speechSynthesis.getVoices` |
| 80 | `bluetooth` | `navigator.bluetooth` stub |
| 85 | `headless` | `matchMedia`, `prefers-reduced-motion`, missing-plugin tells, `outerWidth/Height`, pointer media |
| 87 | `prepare_stack_trace` | protects `Error.prepareStackTrace` from CDP stack-trace probing |
| 88 | `client_hints` | `navigator.userAgentData` brands/platform/architecture/model |
| 89 | `media_codecs` | normalizes `HTMLMediaElement.canPlayType` for H.264/AAC/MP4/WebM |
| 90 | `timezone` | `Intl.DateTimeFormat` / `Date` time zone |
| 92 | `localization` | language / locale |
| 95 | `geolocation` | `geolocation.getCurrentPosition` |
| 110 | `hairline` | image-`srcset` / device-pixel hairline feature detection |
| 120 | `iframe` | `contentWindow` self-defense for same-origin frames |
| 125 | `attach_shadow` | forces open `Element.attachShadow` roots so content remains inspectable |
| 130 | `tracker_block` | optional fetch guard for known tracker hosts |

List them at runtime with `default_registry().provider_names()`, or over MCP
with the `list_evasion_providers` tool.

## Profile coherence validation

`Fingerprint::validate()` returns a [`CoherenceReport`](crate::CoherenceReport)
that flags mismatched signals (for example a macOS user agent with a `Win32`
platform, or a mobile user agent with `maxTouchPoints == 0`):

```rust
use seleniumbase_rs::Fingerprint;

let report = Fingerprint::windows_desktop().validate();
assert!(report.is_coherent());
for warning in &report.warnings {
    eprintln!("warning: {warning}");
}
```

The MCP `validate_fingerprint` tool exposes the same report.

## Humanized input

Enable `StealthFlags::humanize` (and configure `Fingerprint::humanize`) to opt
into human-like timing. The [`humanize`](crate::stealth::humanize) module
provides deterministic helpers:

```rust
use seleniumbase_rs::stealth::humanize::{bezier_mouse_path, keystroke_delays, Point};

let path = bezier_mouse_path(Point::new(0.0, 0.0), Point::new(200.0, 90.0), 24, 7);
let delays = keystroke_delays("hello", 40, 180, 7);
assert_eq!(path.len(), 24);
assert_eq!(delays.len(), 5);
```

## What is spoofed

* `navigator.webdriver` removed
* `navigator.userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`, `languages`
* `window.chrome.runtime` / `window.chrome.app` stubs
* `navigator.plugins` / `navigator.mimeTypes`
* `navigator.permissions.query` for notifications
* `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`
* WebGL `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL`
* Canvas `toDataURL` noise
* Audio `AnalyserNode.getFloatFrequencyData` noise
* `navigator.mediaDevices.enumerateDevices`
* Time zone via `Intl.DateTimeFormat`
* Geolocation via CDP `Emulation.setGeolocationOverride` and JS fallback
* CDP marker scrubbing (`cdc_`, `__webdriver`, `__selenium`, etc.)
* Battery Status, Network Information, Speech Synthesis, and Bluetooth APIs
* Client Hints (`navigator.userAgentData`) brands, platform, and architecture
* WebRTC STUN/TURN filtering to prevent local-IP leaks
* Launch args such as `--disable-blink-features=AutomationControlled`
* Optional native-level spoofing via CDP so page JS cannot detect the override

## Complementary defenses

Runtime fingerprints work best when the browser also hides engine-level
automation markers:

* Patch the `chromedriver` binary with [`ChromedriverPatcher`](crate::ChromedriverPatcher)
  to remove injected `cdc_` and `__webdriver` signatures. See the
  [Binary Patching tutorial](./binary_patching.md).
* Pass extra Chromium flags returned by [`engine_spoofing_args()`](crate::engine_spoofing_args)
  through `BrowserConfig::with_extra_args` or `StealthOptions::extra_args`.
* Combine a `Fingerprint`, UC mode, binary patching, and engine args for the
  strongest anti-detection profile.

## Limitations

* TLS / JA3 / JA4 fingerprint spoofing is not implemented. For pure HTTP
  requests that need browser-faithful TLS, consider `wreq` + `wreq-util`.
* Some dimensions (WebGL vendor/renderer, canvas/audio noise, media devices,
  fonts, battery, etc.) have no CDP override and remain JavaScript-based even
  when `native_spoofing` is enabled.
* Chromium source patching is not performed; for the strongest protection use
  `ChromedriverPatcher` plus engine spoofing args in addition to fingerprints.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Still flagged by bot detection | JavaScript evasions not applied or TLS fingerprint mismatch | Use `DriverMode::Uc`, enable `native_spoofing`, and align egress IP. |
| `navigator.webdriver` still `true` | CDP markers not scrubbed | Apply binary patching or UC mode. |
| Geolocation is wrong | Masking mode is `Natural` | Set `geolocation_masking: Custom` with explicit coordinates. |
| Inconsistent locale/timezone | `locale` and `timezone` fields mismatch | Align both values to the same region. |
| WebRTC leaks local IP | `webrtc_masking` not configured | Use `WebRtcPolicy::DisableNonProxiedUdp`. |
