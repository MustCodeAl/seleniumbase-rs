---
title: "Fingerprint & Stealth Profiles"
---

# Fingerprint & Stealth Profiles

Websites detect automation by looking at many browser signals: `navigator.webdriver`,
user agent, platform, screen size, hardware concurrency, WebGL vendor, canvas
noise, audio noise, time zone, geolocation, media devices, fonts, and more.
`seleniumbase-rs` can spoof these signals through a coherent `Fingerprint` and a
pluggable evasion system.

This page explains how to use built-in fingerprint presets, build custom
profiles, choose masking modes, and apply native-level spoofing through CDP.
It covers what a page can read inside the browser. The TLS and HTTP/2 side, and
the scraper built on it, is in [Stealth Scraping](stealth_scraping.md).

## What you will learn

- How to use built-in fingerprint presets.
- How to generate a random but coherent `Fingerprint` from a seed.
- How to build a custom `Fingerprint`.
- How `StealthFlags` masking modes work.
- How to enable native-level CDP spoofing.
- Which launch paths apply a fingerprint, and what each one does.
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

A fingerprint reaches the page only through a launch path that applies it:

- With `DriverMode::Uc` (above) or `DriverMode::Cdp`, the session registers the
  evasion script and sends the protocol overrides described under
  [Native-level (CDP) spoofing](#native-level-cdp-spoofing), and the launch
  arguments apply.
- With the default `DriverMode::WebDriver`, only the launch arguments apply (user
  agent, `--lang`, `--window-size`, the WebRTC policy, and the QUIC and proxy
  flags). No script is injected and no protocol override is sent.
- The Pure CDP engine and the Playwright session have their own paths, described
  below. The Pure CDP engine runs the script of `session_registry` (27 providers),
  not `default_registry` (29); see [Built-in providers](#built-in-providers).

## Fingerprint presets

Built-in presets set a coherent combination of user agent, platform, screen,
WebGL strings, and masking flags (`StealthFlags::balanced()`):

| Preset | What it claims |
|---|---|
| `Fingerprint::windows_desktop()` | Chrome 124 on Windows 10, 1920x1080, 8 cores, 8 GB, `en-US`, `America/New_York`, an NVIDIA RTX 4070 Ti through ANGLE and Direct3D 11. |
| `Fingerprint::macos_desktop()` | Chrome 124 on macOS (`Macintosh; Intel Mac OS X 10_15_7`), 1920x1080, `America/Los_Angeles`, WebGL `Apple Inc.` / `Apple M3`. The user agent is Chrome's, not Safari's. |
| `Fingerprint::linux_desktop()` | Chrome 124 on Linux x86_64, 1920x1080, `America/Chicago`, and the same Direct3D 11 NVIDIA renderer string as the Windows preset, which a Linux machine would not report. Set `.webgl(..)` or use `Fingerprint::randomized` for a believable Linux GPU. |
| `Fingerprint::android_mobile()` | Chrome 124 on Android 14 (`SM-S918B`), 412x915 at a device pixel ratio of 3, 5 touch points, Adreno 740. |
| `Fingerprint::ios_mobile_safari()` | Mobile Safari 17.4.1 on an iPhone, 390x844 at a device pixel ratio of 3, 5 touch points, `Apple GPU`, Safari-style Client Hints. |

Every preset sets a time zone but no geolocation, so `validate()` reports the
warning "timezone set without geolocation" for each of them (warnings do not
make a report incoherent). Add `.geolocation(lat, lon)` to silence it.

Use the `fingerprint!` macro for shorter syntax. `safari` is an alias for `ios`,
and `edge` returns the Windows preset, whose user agent is Chrome's, not
Edge's:

```rust
use seleniumbase_rs::fingerprint;

let fp = fingerprint!(windows);
let fp = fingerprint!(macos);
let fp = fingerprint!(linux);
let fp = fingerprint!(android);
let fp = fingerprint!(ios);
let fp = fingerprint!(safari); // same as ios
let fp = fingerprint!(edge);   // same as windows
```

## Randomised identities

A preset is one identity; a fleet of sessions that all share it is easy to
spot. `Fingerprint::randomized(os, seed)` generates a different, believable
identity for each seed instead:

```rust
use seleniumbase_rs::{Fingerprint, OsType};

let fp = Fingerprint::randomized(OsType::Windows, 42);

// The same seed always gives the same identity.
assert_eq!(fp, Fingerprint::randomized(OsType::Windows, 42));

// It passes the coherence check, with no warnings either.
let report = fp.validate();
assert!(report.is_coherent() && report.warnings.is_empty());

// Another seed is another identity.
let other = Fingerprint::randomized(OsType::Windows, 43);
assert_ne!(fp, other);
```

Detectors check that signals agree with each other, so the generator never
randomises a field alone. It draws a whole machine and a whole place, then
derives every field from them:

| Dimension | What is generated |
|---|---|
| WebGL | Vendor and renderer the OS really reports: ANGLE over Direct3D 11 (Windows), ANGLE over Metal with an Apple chip (macOS), ANGLE over Mesa or the NVIDIA driver (Linux), an Adreno or Mali chip (Android), `Apple GPU` (iOS). |
| Hardware | `hardwareConcurrency`, `deviceMemory`, screen size, `colorDepth` and device pixel ratio that machine ships with: a Mac gets its chip's core count and its model's screen, a PC's class decides its cores, memory and screens, and a phone gets its own screen. |
| Place | A locale and language list, a time zone it is spoken in, and coordinates in that zone's main city, so `locale`, `timezone` and geolocation agree. |
| Software | User agent, `platform`, `vendor` and Client Hints (built the way Chromium builds them) that agree with the OS and with the claimed Chromium version. |
| Noise | A canvas and audio noise `seed`, different for every seed. |

All five `OsType` values are supported. The masking flags are
`StealthFlags::balanced()`, so the generated values are applied as they are.

Things to know:

- **Stable within a release.** A seed maps to the same identity for one version
  of the crate. Adding a table row changes what a seed maps to, so store the
  generated `Fingerprint` (it is `Serialize`), not the seed, if an identity has
  to outlive an upgrade.
- **The Chromium version is a fixed window.** At this version of the crate the
  user agent claims Chromium 153, 154 or 155 (an iPhone identity claims Mobile
  Safari 17.7, 18.1, 18.3.1 or 18.5). Pair the identity with a browser of that
  era, or change `user_agent`, `client_hints` and `core_version` together.
  `Fingerprint::randomized_with_major(os, seed, major)` fixes the Chromium major
  instead (Safari and iOS ignore it); the stealth scraper uses it so the identity
  claims the Chrome that is running (see [Stealth Scraping](stealth_scraping.md)).
- **iOS differs.** Safari has no `navigator.deviceMemory` and no
  `navigator.userAgentData`, so an iOS identity sets neither (`device_memory` and
  `client_hints` are `None`).
- **Client Hints use `major.0.0.0`** for full versions, the form the user agent
  itself uses.
- **Not set:** fonts, media devices and the proxy. Set them on the result if you
  need them.
- **Applying it.** For the Pure CDP engine, hand it to the launch options with
  `LaunchOptions::builder().fingerprint(&fp)` (see
  [Pure CDP](pure_cdp.md#looking-like-another-browser)); `page.apply_fingerprint(&fp)`
  does the same for one tab. For `BaseCase`, pass it to
  `BrowserConfig::with_fingerprint` and use `DriverMode::Uc` or `DriverMode::Cdp`.
  You can also feed it to the evasion registry yourself.

## Building a custom fingerprint

For full control, use the builder:

```rust
use seleniumbase_rs::{Fingerprint, StealthFlags};

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
    .media_devices(1, 1, 2) // audio inputs, audio outputs, video inputs
    .flags(StealthFlags::balanced())
    .build();
```

`Fingerprint::builder()` starts from `StealthFlags::default()`, where every
masking mode is `Natural`, so the JavaScript providers for navigator, screen,
WebGL, time zone and the like do not run until you set the modes: either
`.flags(StealthFlags::balanced())` as above, or one flag at a time as in the
examples below. The protocol overrides do not depend on the modes; they are sent
for every field that is set (see
[Native-level (CDP) spoofing](#native-level-cdp-spoofing)).

## Masking modes

`StealthFlags` controls which dimensions are spoofed. Most flags are a
`MaskingMode`, which has four values. This is what the built-in providers do
with each:

| Mode | What happens |
|---|---|
| `Natural` | The provider is not emitted, so the page sees the browser's real value. |
| `Mask` | The provider is emitted. It uses the `Fingerprint` field when it is set and a built-in default otherwise (the WebGL provider falls back to `Intel Inc.` / `Intel Iris OpenGL Engine`, the time zone provider to `America/New_York`). |
| `Custom` | The same as `Mask`: no built-in provider tells the two apart. The profile-payload parser sets `Custom` when a payload supplied a literal value. |
| `Disabled` | The same as `Natural`, except for `native_tostring_masking` and `headless_masking`, whose providers (`native_tostring`, `attach_shadow`) keep running unless the flag is `Disabled`. |

Some flags are not a `MaskingMode`: `graphics_noise` and `ports_masking` are a
`NoiseMode`, `canvas_noise` is a `CanvasNoiseMode`, `proxy_masking` is a
`ProxyMaskingMode`, `quic_mode` is a `QuicMode`, `geolocation_popup` is a
`PopupMode`, `startup_behavior` is a `StartupBehavior`, and the last five flags
in the table below are `bool`.

`StealthFlags::default()` leaves every `MaskingMode` at `Natural`.
`StealthFlags::balanced()` masks the dimensions the table marks `Mask`.
`StealthFlags::all_custom()` sets every `MaskingMode`, `proxy_masking` and
`startup_behavior` to `Custom`, for when every value is supplied explicitly.

## Stealth flag reference

The `default()` column is `StealthFlags::default()` and the `balanced()` column
is `StealthFlags::balanced()`, which every preset and every `randomized`
identity uses. "Provider" names are the built-in evasion providers listed under
[Built-in providers](#built-in-providers).

| Flag | Type | `default()` | `balanced()` | What it controls |
|---|---|---|---|---|
| `webrtc_masking` | `MaskingMode` | `Natural` | `Mask` | The `webrtc` provider: drops STUN and TURN servers from `RTCPeerConnection` configs and, when `webrtc_public_ip` or `webrtc_local_ip` is set, rewrites the IPs in SDP and ICE candidates. The Chrome launch flag comes from `webrtc_policy`, not from this flag. |
| `audio_masking` | `MaskingMode` | `Natural` | `Natural` | The `audio_noise` provider. |
| `graphics_noise` | `NoiseMode` | `Mask` | `Mask` | Only switches the `hairline` provider on (`Mask` or `Natural`) or off (any other value). `Low`, `Medium` and `High` do not set an intensity. |
| `geolocation_popup` | `PopupMode` | `Prompt` | `Prompt` | `Allow` makes the `geolocation` provider answer `getCurrentPosition` and `watchPosition` itself; otherwise the browser's own methods stay. |
| `navigator_masking` | `MaskingMode` | `Natural` | `Mask` | The `navigator_props`, `permissions` and `plugins` providers (`plugins` only in `default_registry`; the Pure CDP engine leaves `navigator.plugins` and `mimeTypes` to the browser). It also switches on `chrome_runtime` and `localization`. |
| `localization_masking` | `MaskingMode` | `Natural` | `Mask` | The `localization` provider (`navigator.language` and `navigator.languages`). |
| `timezone_masking` | `MaskingMode` | `Natural` | `Mask` | The `timezone` provider. |
| `graphics_masking` | `MaskingMode` | `Natural` | `Mask` | The `webgl` provider. |
| `fonts_masking` | `MaskingMode` | `Natural` | `Mask` | The `fonts` provider, and only when `Fingerprint::fonts` is not empty. |
| `media_devices_masking` | `MaskingMode` | `Natural` | `Natural` | The `media_devices` provider. |
| `screen_masking` | `MaskingMode` | `Natural` | `Mask` | The `window_geometry` provider. |
| `geolocation_masking` | `MaskingMode` | `Natural` | `Mask` | The `geolocation` provider, and only when `Fingerprint::latitude` is set. It does not affect the protocol geolocation override. |
| `ports_masking` | `NoiseMode` | `Mask` | `Mask` | Stored only. No built-in code reads it. |
| `proxy_masking` | `ProxyMaskingMode` | `Disabled` | `Disabled` | `Custom`, `Socks5`, `Http` and `Https` add `--proxy-server=<url>` from `Fingerprint::proxy` when it is set. `Direct` and `Disabled` add nothing. |
| `quic_mode` | `QuicMode` | `Natural` | `Natural` | `Disabled` and `ForceHttp2` add `--disable-quic`; `Enabled` adds `--enable-quic`; `Natural` and `Auto` add nothing. |
| `canvas_noise` | `CanvasNoiseMode` | `Mask` | `Mask` | Only `Mask` runs the `canvas_noise` provider. `Natural`, `Disabled`, `Random`, `Persistent` and `Low` add no noise. |
| `startup_behavior` | `StartupBehavior` | `Recover` | `Recover` | Stored only. No built-in code reads it. |
| `battery_masking` | `MaskingMode` | `Natural` | `Mask` | The `battery` provider. |
| `connection_masking` | `MaskingMode` | `Natural` | `Mask` | The `connection` provider (only in `default_registry`; the Pure CDP engine leaves `navigator.connection` to the browser). |
| `speech_masking` | `MaskingMode` | `Natural` | `Mask` | The `speech` provider. |
| `bluetooth_masking` | `MaskingMode` | `Natural` | `Mask` | The `bluetooth` provider. |
| `client_hints_masking` | `MaskingMode` | `Natural` | `Mask` | The `client_hints` provider. It also needs `Fingerprint::client_hints`, and it is skipped under `native_spoofing`. |
| `native_tostring_masking` | `MaskingMode` | `Natural` | `Mask` | The `native_tostring` provider runs unless this is `Disabled`. |
| `chrome_runtime_masking` | `MaskingMode` | `Natural` | `Mask` | The `chrome_runtime` provider (it also runs when `navigator_masking` is masked). |
| `headless_masking` | `MaskingMode` | `Natural` | `Mask` | The `headless` provider, and `Emulation.setFocusEmulationEnabled` for `Mask` and `Custom`. The `attach_shadow` provider runs unless this is `Disabled`. |
| `humanize` | `bool` | `false` | `false` | Stored only. `BaseCase::human_type` reads `Fingerprint::humanize` (a `HumanizeConfig`), not this flag. |
| `block_trackers` | `bool` | `false` | `false` | The `tracker_block` provider, and `Network.setBlockedURLs` for the hosts in `Fingerprint::tracker_hosts()`. |
| `disable_csp` | `bool` | `false` | `false` | `Page.setBypassCSP`. |
| `grant_permissions` | `bool` | `false` | `false` | `Browser.grantPermissions` for `notifications`, `midi`, `midiSysex`, `clipboardReadWrite`, `clipboardSanitizedWrite`, `paymentHandler`, `backgroundSync`, `idleDetection` and `webAppInstallation`. It does not grant `geolocation`. |
| `native_spoofing` | `bool` | `false` | `false` | See [Native-level (CDP) spoofing](#native-level-cdp-spoofing). |

### Literal values in JSON payloads

A profile imported through `ProfileParams` carries these flags as strings:
`webrtc_masking`, `proxy_masking`, `geolocation_popup`, `audio_masking`,
`graphics_noise`, `ports_masking`, `navigator_masking`, `localization_masking`,
`timezone_masking`, `graphics_masking`, `fonts_masking`, `media_devices_masking`,
`screen_masking`, `geolocation_masking`, `quic_mode`, `canvas_noise` and
`startup_behavior`, plus the booleans `disable_csp`, `grant_permissions` and
`native_spoofing`. Every other flag (battery, connection, speech, Bluetooth,
Client Hints, `native_tostring`, `chrome_runtime`, headless, `humanize`,
`block_trackers`) takes its `balanced()` value.

Many of the string flags accept a concrete literal. The parser detects the
literal, sets the mode to `Custom`, and writes the value into the matching
`Fingerprint` field:

| Flag | Literal example | Populated fingerprint field |
|---|---|---|
| `screen_masking` | `"1920x1080x24"` | `screen_width`, `screen_height`, `color_depth` |
| `geolocation_masking` | `"40.7128,-74.0060,10.5"` | `latitude`, `longitude`, `altitude` |
| `graphics_masking` | `"Intel Inc.~Intel(R) Iris(R) Xe Graphics"` | `webgl_vendor`, `webgl_renderer` |
| `navigator_masking` | full UA string | `user_agent` |
| `localization_masking` | `"en-US"` (any value containing `-`) | `locale`, `languages` and `accept_languages`, all set to that same string |
| `timezone_masking` | `"America/New_York"` | `timezone` |
| `fonts_masking` | `"Arial,Calibri,Consolas"` | `fonts` |
| `media_devices_masking` | `"audioinput:default:Mic1\|videoinput:default:Cam1"` | `media_devices` |
| `ports_masking` | `"block:80,443,3000"` | `ports` |
| `proxy_masking` | `"socks5://user:pass@host:1080"` | `proxy` |
| `webrtc_masking` | `"public:172.56.21.89\|local:10.0.0.5"` | `webrtc_public_ip`, `webrtc_local_ip` |

A screen literal also sets `color_depth` (24 when omitted), a geolocation literal
sets `accuracy` to 20, and a WebRTC literal sets the policy to
`WebRtcPolicy::PublicAndPrivateInterfaces`.

### Keywords in JSON payloads

Keywords are case-insensitive. A value the parser does not know gets the
"falls back to" mode.

| Payload flag | Keywords and the mode they give | Falls back to |
|---|---|---|
| `navigator_masking` | `natural`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `mask`, `spoof`, `desktop_chrome`, `mobile_safari`: `Mask`. A string starting `Mozilla/` or containing `AppleWebKit` or `Chrome/` is a user agent. | `Mask` |
| `screen_masking` | `natural`, `native`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `custom`, `mask`, `spoof`: `Mask`. | `Mask` |
| `geolocation_masking` | `natural`, `auto`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `custom`, `mask`, `spoof`: `Mask`. | `Mask` |
| `graphics_masking` | `natural`, `pass_through`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `spoof`, `generic`, `mask`: `Mask`. | `Mask` |
| `localization_masking`, `timezone_masking` | `natural`, `auto`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `mask`, `spoof`, `custom`: `Mask`. A localization value must contain `-`, a time zone must contain `/`. | `Mask` |
| `fonts_masking` | `natural`, `off`: `Natural`. `disabled`, `block`: `Disabled`. `mask`, `spoof`, `system_only`, `random`, `blacklist`: `Mask`. Anything else is read as a comma-separated font list. | `Mask` |
| `media_devices_masking` | `natural`, `off`: `Natural`. `disabled`, `block`, `empty`: `Disabled`. `mask`, `spoof`, `noise`: `Mask`. A value starting `2_mic_1_cam`: `Custom` with 2 audio inputs, 1 audio output and 1 video input. | `Mask` |
| `audio_masking` | `natural`, `off`: `Natural`. `disabled`, `block`: `Disabled`. | `Mask` |
| `webrtc_masking` | `natural`, `off`: `Natural`. `disabled`, `block`: `Disabled`, with the policy still `DisableNonProxiedUdp`. `mask`, `spoof`: `Mask`. `public_only`: `Mask` with `WebRtcPolicy::PublicInterfaceOnly`. | `Mask` |
| `proxy_masking` | `natural`, `off`, `direct`, `disabled`: `Disabled`. `socks5`, `http`, `https`, `custom`: that mode, with no proxy address. A `socks5://`, `http://` or `https://` URL is parsed into `Fingerprint::proxy` (default port 1080 for SOCKS5, 8080 otherwise). | `Disabled` |
| `graphics_noise` | `natural`: `Natural`. `off`: `Off`. `low`, `medium`, `high`: that value. | `Mask` |
| `canvas_noise` | `natural`, `disabled` (or `off`), `random`, `persistent`, `low`: that mode. If the key is absent: `Mask`. | `Mask` |
| `ports_masking` | `natural`, `off`, `disabled`: `Off`. `mask`, `spoof`, `stealth`: `Mask`. `block`, `block_all`: `Block`. `whitelist`: `Natural`. `block:80,443` or `80,443`: `Block` with those ports. | `Mask` |
| `geolocation_popup` | `allow`, `block`. | `Prompt` |
| `quic_mode` | `natural`, `auto`: `Natural`. `enabled`, `disabled`, `force_http2`. | `Disabled` |
| `startup_behavior` | `custom`. | `Recover` |

A payload that leaves out `quic_mode` gets `Disabled`, which adds `--disable-quic`;
`balanced()` leaves QUIC alone.

### Concrete mask-mode examples

#### Custom navigator (`navigator_masking: Custom`)

```rust
use seleniumbase_rs::Fingerprint;

let mut fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
    .platform("Win32")
    .hardware_concurrency(8)
    .device_memory(8.0)
    .build();
fp.flags.navigator_masking = seleniumbase_rs::MaskingMode::Custom;
```

With `DriverMode::Uc` or `DriverMode::Cdp`, the `navigator_props` provider
overrides `navigator.userAgent`, `appVersion`, `platform`, `hardwareConcurrency`
and `deviceMemory` with the values you supplied. The same user agent is also
passed as `--user-agent` and sent as `Network.setUserAgentOverride`.

#### Custom screen + geolocation (`screen_masking` / `geolocation_masking: Custom`)

```rust
use seleniumbase_rs::{Fingerprint, MaskingMode};

let mut fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation(52.52, 13.405)
    .timezone("Europe/Berlin")
    .build();
fp.flags.screen_masking = MaskingMode::Custom;
fp.flags.geolocation_masking = MaskingMode::Custom;
fp.flags.timezone_masking = MaskingMode::Mask;
```

With `DriverMode::Uc` or `DriverMode::Cdp`:

- The launch arguments include `--window-size=1920,1080`, and the `window_geometry`
  provider overrides the `screen` properties, `outerWidth`, `outerHeight` and
  `devicePixelRatio`.
- `Emulation.setDeviceMetricsOverride` and `Emulation.setGeolocationOverride` are
  sent. They depend only on the fields being set, not on the masking modes.
- The `timezone` provider patches `Intl.DateTimeFormat` and the local-time methods
  of `Date` to Berlin time, and `Emulation.setTimezoneOverride` is sent as well.
- The `geolocation` provider answers in JavaScript only when `geolocation_popup`
  is `PopupMode::Allow`. Otherwise the protocol override alone sets the position,
  and the page still needs the geolocation permission (`grant_permissions` does
  not grant it).

#### Custom proxy (`proxy_masking: Custom`)

```rust
use seleniumbase_rs::{Fingerprint, ProxyConfig, ProxyMaskingMode};

let mut fp = Fingerprint::default();
fp.proxy = Some(ProxyConfig {
    r#type: "http".to_owned(),
    host: "proxy.example.com".to_owned(),
    port: 8080,
    username: Some("alice".to_owned()),
    password: Some("secret".to_owned()),
    save_traffic: false,
});
fp.flags.proxy_masking = ProxyMaskingMode::Custom;
```

`ProxyConfig` has no `FromStr`; build it as above. `ProxyMaskingMode::Custom`,
`Socks5`, `Http` and `Https` each emit
`--proxy-server=http://alice:secret@proxy.example.com:8080` (the URL is
`<type>://<user>:<password>@<host>:<port>`; Chrome ignores the credentials in
this flag). `Disabled` and `Direct` emit nothing. `Debug` output of a
`ProxyConfig` hides the password.

On the WebDriver path (`DriverMode::Uc` or `Cdp`), the protocol overrides for a
proxy that has both a user name and a password also include
`Network.setExtraHTTPHeaders` with a `Proxy-Authorization: Basic ...` header,
which goes to every site the tab visits. The Pure CDP engine never sends that
header and answers the proxy's own challenge instead.

#### WebRTC masked

```rust
use seleniumbase_rs::{Fingerprint, MaskingMode, WebRtcPolicy};

let mut fp = Fingerprint::default();
fp.flags.webrtc_masking = MaskingMode::Mask;
fp.webrtc_policy = WebRtcPolicy::DisableNonProxiedUdp;
```

Two settings act on WebRTC, and they are independent:

- `webrtc_policy` picks a Chrome flag that is passed on every launch path that
  takes a fingerprint. `DisableNonProxiedUdp` (the default) gives
  `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`,
  `PublicInterfaceOnly` gives `default_public_interface_only`, and
  `PublicAndPrivateInterfaces` gives `default_public_and_private_interfaces`.
  There is no value that omits the flag, and `webrtc_masking: Disabled` does not
  remove it.
- `webrtc_masking` switches the JavaScript `webrtc` provider on (`Mask` or
  `Custom`): it drops STUN and TURN servers from new `RTCPeerConnection`s and
  rewrites IPs when `webrtc_public_ip` or `webrtc_local_ip` is set.

## Native-level (CDP) spoofing

When a session has a CDP connection (`DriverMode::Uc`, `DriverMode::Cdp`, or the
Pure CDP engine), a fingerprint is applied in two layers:

1. The evasion script, assembled from the providers below and registered with
   `Page.addScriptToEvaluateOnNewDocument`. Page scripts can *in principle*
   detect a JavaScript patch through `Function.prototype.toString`,
   `Object.getOwnPropertyNames`, or descriptor inspection, which is why the
   providers use the `sb` helpers to look native.
2. The protocol overrides from `evasions::cdp_overrides`. They are driven by which
   fields are set, not by the masking modes, except where the table says so:

| Command | Sent when |
|---|---|
| `Network.setUserAgentOverride` (user agent, `acceptLanguage`, platform, `userAgentMetadata`) | `user_agent` is set. `acceptLanguage` is `accept_languages`, else `locale`, else `en-US,en;q=0.9`. The metadata comes from `client_hints` when set and is derived from the OS and user agent otherwise. |
| `Emulation.setDeviceMetricsOverride` | `screen_width` and `screen_height` are set. `pixel_ratio` is the scale factor (1.0 if unset) and `mobile` follows `os_type`. `cdp_overrides` sets the viewport (`width`, `height`) to the screen, and the WebDriver path sends it as it is. The Pure CDP engine changes it for a desktop persona: it sends a `0` by `0` viewport, which the protocol reads as "no override", plus `screenWidth` and `screenHeight`, so the browser itself reports `screen.width` and `screen.height` and the page keeps the viewport its window really has. A mobile persona is a phone-sized page, so its viewport stays the screen. |
| `Emulation.setLocaleOverride` | `locale` is set. |
| `Emulation.setTimezoneOverride` | `timezone` is set. |
| `Emulation.setGeolocationOverride` | `latitude` and `longitude` are set (accuracy 100 and altitude 0 unless set). |
| `Browser.grantPermissions` | `grant_permissions` is on. |
| `Network.setBlockedURLs` | `block_trackers` is on. |
| `Page.setBypassCSP` | `disable_csp` is on. |
| `Emulation.setFocusEmulationEnabled` | `headless_masking` is `Mask` or `Custom`. |
| `Network.setExtraHTTPHeaders` | The proxy has a user name and a password (WebDriver path only; see the proxy caveat above). |

So a UC or CDP session already gets these overrides without `native_spoofing`,
and the JavaScript providers patch the same properties on top of them.
`StealthFlags::native_spoofing` makes the providers stand down where a protocol
override covers the property, so the browser reports the value itself instead of
a replaced JavaScript getter:

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

When `native_spoofing` is true:

* `navigator_props` leaves out `platform`, `userAgent` and `appVersion`.
* `window_geometry` emits only `colorDepth` and `pixelDepth`; the size and scale
  come from `Emulation.setDeviceMetricsOverride`.
* `localization`, the JavaScript `geolocation` provider and `client_hints` are
  skipped, and `timezone` is skipped when a `timezone` is set.
* The JS providers stay for everything the protocol does not cover:
  `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, WebGL
  vendor and renderer, canvas and audio noise, media devices, fonts, battery,
  connection, speech, Bluetooth, the `navigator.webdriver` getter and the
  marker cleanup.
* A WebDriver session (Chrome, Chromium or Edge) also launches a patched copy of
  the browser. If `BrowserConfig::browser_binary_path` is unset, the session
  finds the system Chrome (`SB_CHROME_BIN`, else `find_system_chrome()`), writes a
  patched copy through `ChromeBinaryPatcher` with `EnginePatch::chrome_binary()`
  (webdriver markers, `navigator.webdriver` assignments and the `HeadlessChrome`
  user-agent marker), and sets `browser_binary_path` to it. It returns an
  `InvalidConfig` error naming `SB_CHROME_BIN` if no Chrome is found. A
  `browser_binary_path` that is already set is used as it is and is not patched;
  that includes the one `BrowserConfig::from_env()` takes from `SB_CHROME_BIN`.

### Caveats

* The protocol overrides and the script need a CDP connection. In
  `DriverMode::WebDriver`, neither is applied whether or not `native_spoofing` is
  set: only the launch arguments (and, with `native_spoofing`, the patched
  binary) take effect.
* `colorDepth` and `pixelDepth` are not covered by the device metrics, so a
  minimal JS patch is still emitted for them.
* Some signals (WebGL, canvas noise, audio noise) have no protocol override and
  stay JavaScript-based in native mode.
* Whether a given detector can still tell the difference is not something this
  crate measures.

## rustwright / Playwright mode

This mode needs the `playwright` Cargo feature (`rustwright`); it is not on by
default.

```rust
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::Fingerprint;

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fp = Fingerprint::windows_desktop();
let session = PlaywrightSession::launch_with_fingerprint(&fp).await?;
session.goto("https://example.com").await?;
session.close().await?;
# Ok(())
# }
```

`launch_with_fingerprint` opens a headed browser; use
`launch_headless_with_fingerprint` for headless. The session passes the
fingerprint's launch arguments, then evaluates the evasion script in the first
page and again after every `goto` and `new_page`, because rustwright cannot
register a script to run in each new document. The script therefore runs after
navigation, not before the page's own scripts, so a page that reads the identity
while it loads can see the browser's real one. The protocol overrides listed above
are not sent in this mode.

## External profile payloads

The `profile_payloads::ProfileParams` type maps to a `Fingerprint` via
`ProfileParams::to_fingerprint()`. `ProfileParams::to_browser_config(webdriver_url)`
returns a `BrowserConfig` with that fingerprint attached.

The returned `mode` follows the payload's `automation` field: `playwright` and
`puppeteer` give `DriverMode::Cdp`; `selenium` (the default when the field is
absent) and the Firefox browser types give `DriverMode::WebDriver`; anything else
gives `DriverMode::Uc`. The script and protocol overrides are applied only in
`Cdp` and `Uc` mode, so a payload that keeps `selenium` is launched with the launch
arguments only. The payload's `cmd_params` become Chrome flags, except switches
that run a program, load an extension, set the profile directory, a proxy or
remote debugging, or ignore certificate errors; those are refused. The Pure CDP
engine does not pass `cmd_params` at all.

## Provider architecture

Every evasion is an `EvasionProvider`: a small, self-contained unit that returns
a JavaScript snippet for an `EvasionContext` (the session fingerprint plus a
deterministic seed and runtime config). Providers are held by an
`EvasionRegistry` and assembled into a single bootstrap script in priority order
(lower `priority()` runs first). The script is one function; each snippet sits in
its own `try`/`catch`, after a `// provider: <name>` comment. `EvasionConfig`
turns either of those off.

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
   `providers::SB_PREAMBLE` brings in the `sb` helpers that make a replaced
   function look native; the example below skips it for brevity.
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

`default_registry()` registers 29 providers, listed here in execution order. "Runs
when" is the provider's `applies` condition; "masked" means the flag is `Mask` or
`Custom`.

The Pure CDP engine does not use `default_registry()`. It uses
`session_registry(&fp)`, the same list without `plugins` and `connection`
(`SESSION_OMITTED_PROVIDERS`), which is 27 providers. A real Chrome already has
`navigator.plugins`, `navigator.mimeTypes`, `navigator.connection` and the PDF
viewer flag, and answers for them itself, so a replaced getter is one more thing
a page can find that a real Chrome does not have. The assembled session script
never mentions `plugins`, `mimeTypes`, `pdfViewerEnabled` or `connection`.
`default_registry()` is unchanged, for callers that want those spoofs, such as a
browser that really lacks them, and it is what the WebDriver `Uc` and `Cdp` modes
use.

| Priority | Name | Runs when | What it does |
|---|---|---|---|
| 5 | `native_tostring` | `native_tostring_masking` is not `Disabled` | Makes replaced functions print `[native code]` from `Function.prototype.toString`, and installs the `sb` helpers the other providers use. |
| 10 | `webdriver` | always | Makes `navigator.webdriver` read `false` (a getter on the navigator prototype) and deletes known automation hooks from `window` (`callPhantom`, `_phantom`, `__nightmare`, `__selenium_unwrapped`, `__webdriver_evaluate`, `__driver_evaluate`, `domAutomation`, and others). |
| 15 | `cdp_markers` | always | Deletes `$cdc_`, `__webdriver`, `__selenium`, `__driver`, `$chrome_` and `__fxdriver` properties from `window`, its prototype chain and `document`. |
| 20 | `chrome_runtime` | `chrome_runtime_masking` or `navigator_masking` is masked | `window.chrome.app`, `loadTimes`, `csi` and `runtime` stubs. |
| 30 | `navigator_props` | `navigator_masking` is masked | `userAgent`, `appVersion` and `platform` (not under `native_spoofing`), then `hardwareConcurrency`, `deviceMemory` and `maxTouchPoints` when set, `vendor`, `productSub` (`20030107` unless set), and `oscpu` and `buildID` when set. It does not touch `pdfViewerEnabled`, `plugins` or `mimeTypes`: Chrome answers for them itself. |
| 35 | `permissions` | `navigator_masking` is masked | Makes `permissions.query({ name: 'notifications' })` agree with `Notification.permission`. |
| 40 | `plugins` | `navigator_masking` is masked | Five PDF-viewer plugins and their MIME types. In `default_registry()` only. |
| 45 | `window_geometry` | `screen_masking` is masked | `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`, `pixelDepth`, plus `outerWidth`, `outerHeight` and `devicePixelRatio`. The window is as large as the persona's screen, and never smaller than the viewport it really has. Under `native_spoofing`, only `colorDepth` and `pixelDepth`. |
| 50 | `webgl` | `graphics_masking` is masked | `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL` from WebGL1 and WebGL2 `getParameter`. |
| 55 | `canvas_noise` | `canvas_noise` is `Mask` | Deterministic noise, a function of the seed and the pixel position, on `toDataURL`, `toBlob` and `getImageData`. |
| 60 | `audio_noise` | `audio_masking` is masked | Deterministic noise on `AudioBuffer.getChannelData` and `copyFromChannel` and on `AnalyserNode.getFloatFrequencyData`. |
| 65 | `webrtc` | `webrtc_masking` is masked | Drops STUN and TURN servers from `RTCPeerConnection` configs; rewrites IPs in SDP and `RTCIceCandidate` when `webrtc_public_ip` or `webrtc_local_ip` is set. |
| 70 | `battery` | `battery_masking` is masked | `navigator.getBattery()` (defaults: charging, level 0.94). |
| 72 | `connection` | `connection_masking` is masked | `NetworkInformation` `effectiveType`, `rtt`, `downlink` and `saveData` (defaults: `4g`, 50, 10, `false`). In `default_registry()` only. |
| 75 | `media_devices` | `media_devices_masking` is masked | `enumerateDevices`, from `media_devices` (explicit labels) or generated from the input and output counts (default 1, 1 and 1). |
| 76 | `fonts` | `fonts_masking` is masked and `fonts` is not empty | Limits `document.fonts.check`, `load` and `FontFace` to the configured fonts. |
| 78 | `speech` | `speech_masking` is masked | `speechSynthesis.getVoices()` (a default set of three voices unless `speech_voices` is set). |
| 80 | `bluetooth` | `bluetooth_masking` is masked | A `navigator.bluetooth` stub, only if the browser has none. |
| 85 | `headless` | `headless_masking` is masked | `matchMedia` answers (`prefers-reduced-motion: reduce` and `prefers-color-scheme: dark` false, `any-pointer: fine` true) and `document.hidden` / `visibilityState`. |
| 87 | `prepare_stack_trace` | never | Reserved name. It emits no script. |
| 88 | `client_hints` | `client_hints_masking` is masked, `client_hints` is set, and not `native_spoofing` | `navigator.userAgentData`, including `getHighEntropyValues`, but only in a secure context (`window.isSecureContext`): that is the only place Chrome has the property, so on a plain `http:` page there is none to read and none is added. `localhost` and `127.0.0.1` are secure contexts. |
| 89 | `media_codecs` | always | `canPlayType` answers for MP4, AAC and MP3 types when the browser says it cannot play them. WebM and everything else stay the browser's own. |
| 90 | `timezone` | `timezone_masking` is masked, unless `native_spoofing` is on and a `timezone` is set | `Intl.DateTimeFormat` and the local-time methods of `Date` (`getTimezoneOffset`, `getHours`, `toString`, the `toLocale*String` family and others). `new Date(y, m, d)` and `Date.parse` of a string without an offset keep the machine's zone. |
| 92 | `localization` | `localization_masking` or `navigator_masking` is masked, and not `native_spoofing` | `navigator.language` and `navigator.languages`. |
| 95 | `geolocation` | `geolocation_masking` is masked, `latitude` is set, and not `native_spoofing` | `getCurrentPosition` and `watchPosition`. They answer in JavaScript only when `geolocation_popup` is `Allow`. |
| 110 | `hairline` | `graphics_noise` is `Mask` or `Natural` | Modernizr's hairline check: `offsetHeight` of the element with id `modernizr` reads `1`. |
| 120 | `iframe` | never | Reserved name. It emits no script. |
| 125 | `attach_shadow` | `headless_masking` is not `Disabled` | Forces `Element.attachShadow` to `mode: 'open'`. |
| 130 | `tracker_block` | `block_trackers` is on | Makes `fetch`, `XMLHttpRequest` and `navigator.sendBeacon` to the tracker hosts (and their subdomains) fail like an unreachable host. Scripts, images and frames loaded by markup are not covered. |

List them at runtime with `default_registry().provider_names()` (all 29, including
the two reserved ones), `session_registry(&fp).provider_names()` (the 27 the Pure
CDP engine runs), `applicable_names(&fp)` (those that run for a fingerprint), or
over MCP with the `list_evasion_providers` tool.

## Profile coherence validation

`Fingerprint::validate()` returns a `CoherenceReport` with `errors` and
`warnings`; `is_coherent()` is true when there are no errors. It reports an error
for a `navigator.platform` that does not fit `os_type` (a macOS user agent with a
`Win32` platform), a user agent with no token for `os_type`, and a screen with a
zero dimension. It reports a warning for a mobile user agent with
`maxTouchPoints == 0`, a desktop user agent with touch points, a `locale` that
differs from the first of `languages`, and a time zone with no geolocation. It
does not look at WebGL, hardware, or whether the locale and time zone belong
together.

```rust
use seleniumbase_rs::Fingerprint;

let report = Fingerprint::windows_desktop().validate();
assert!(report.is_coherent());
for warning in &report.warnings {
    eprintln!("warning: {warning}");
}
```

The MCP `validate_fingerprint` tool returns the same report for one of the five
built-in presets (`windows`, `macos`, `linux`, `android`, `ios`); it does not take
an arbitrary fingerprint.

`tests/fingerprint_randomized.rs` asserts that `Fingerprint::randomized` gives no
errors and no warnings for every OS, for seeds 0 to 1499 and ten edge seeds
(`0`, `u64::MAX`, `1 << 63` and others). If you edit a generated profile by hand,
run `validate()` again afterwards.

## Humanized input

Detectors look for input that is too regular. The
`stealth::behavior` module plans input that is not: curved
pointer paths with a bell-shaped speed profile, Fitts's-law timing, tremor and
overshoot, and Gaussian typing with word and sentence pauses. It is pure and
seedable:

```rust
use seleniumbase_rs::sb_cdp::Point;
use seleniumbase_rs::stealth::behavior::{Behavior, Humanizer};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let mut person = Humanizer::new(Behavior::builder().typing_wpm(70.0).seed(7).build()?);
let path = person.mouse_path(Point::new(0.0, 0.0), Point::new(200.0, 90.0), 60.0);
let keys = person.typing_plan("hello");
assert_eq!(path.last().map(|step| step.at), Some(Point::new(200.0, 90.0)));
assert_eq!(keys.len(), 5);
# Ok(())
# }
```

To play a plan out on a page, use `page.human(behavior)` from the Pure CDP
engine (see [Pure CDP](pure_cdp.md#human-paced-input)). `BaseCase::human_type`
uses the same typing model, taking its pace from `Fingerprint::humanize` when the
configuration has a fingerprint.

`Behavior::from_config(&fingerprint.humanize)` turns a fingerprint's
`HumanizeConfig` (its minimum and maximum keystroke delay) into the matching
behaviour.

The older `stealth::humanize` helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a Bézier curve at evenly spaced
parameter values and draw delays uniformly between two bounds; prefer
`behavior`.

## What is spoofed

With a CDP session and the `balanced()` flags (see the provider table for the
conditions):

* `navigator.webdriver` reads `false`, and known automation hooks are deleted
* `navigator.userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`,
  `maxTouchPoints`, `vendor`, `language` and `languages`
* `window.chrome.app`, `csi`, `loadTimes` and `runtime` stubs
* `navigator.plugins` and `navigator.mimeTypes` (WebDriver `Uc` and `Cdp` modes;
  the Pure CDP engine leaves them to Chrome)
* `navigator.permissions.query` for notifications
* `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`,
  `outerWidth`, `outerHeight` and `devicePixelRatio`
* WebGL `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL`
* Canvas `toDataURL`, `toBlob` and `getImageData` noise
* Audio noise (when `audio_masking` is on; `balanced()` leaves it off)
* `navigator.mediaDevices.enumerateDevices` (when `media_devices_masking` is on;
  `balanced()` leaves it off)
* Time zone, through `Intl.DateTimeFormat`, the local-time methods of `Date`, and
  `Emulation.setTimezoneOverride`
* Geolocation, through `Emulation.setGeolocationOverride` (the JavaScript
  provider answers only when `geolocation_popup` is `Allow`)
* `$cdc_`, `__webdriver`, `__selenium` and similar marker properties
* Battery Status, Speech Synthesis, and Bluetooth APIs, and Network Information
  (WebDriver `Uc` and `Cdp` modes only)
* Client Hints (`navigator.userAgentData`), when `client_hints` is set and the
  page is a secure context
* `matchMedia` answers and page visibility, `canPlayType` for MP4/AAC/MP3, and
  `attachShadow` roots forced open
* STUN and TURN servers dropped from `RTCPeerConnection` configs, and Chrome's
  WebRTC IP-handling flag
* Launch arguments such as `--disable-blink-features=AutomationControlled`
* Optionally (`native_spoofing`), fewer properties replaced in JavaScript, and a
  patched Chrome binary for WebDriver sessions

## Complementary defenses

Runtime fingerprints work best when the browser also hides engine-level
automation markers:

* Patch the `chromedriver` binary with `ChromedriverPatcher` (for example
  `ChromedriverPatcher::new(path).patch(EnginePatch::all())?`) to remove injected
  `cdc_` and `__webdriver` signatures. See the
  [Binary Patching tutorial](./binary_patching.md).
* Pass the extra Chromium flags returned by `engine_spoofing_args()` (a
  `Vec<String>`) through `BrowserConfig::with_extra_args`, which replaces the
  list (`push_extra_arg` adds one), or through `StealthOptions::extra_args`.
* Combining a `Fingerprint`, UC mode, binary patching, and engine args covers the
  most layers this crate offers. None of it guarantees that a given detector
  passes.

## Limitations

* Nothing on this page changes the TLS or HTTP/2 behaviour of the browser. A
  fingerprint reaches the page through scripts and protocol overrides; the
  `ClientHello`, the HTTP/2 connection start and the header order that leave the
  machine are those of the browser you launch. The `scrape` and `impersonate`
  Cargo features add JA3, JA4, peetprint and Akamai parsers, a measured table for
  one browser (Chrome 155 on macOS), and a client that sends that handshake or
  one you specify; see [Stealth Scraping](stealth_scraping.md). That client is a
  separate building block. It is not applied to `BaseCase` or to the Pure CDP
  engine on its own. `MitmProxy` gives a real Chrome on the Pure CDP engine the
  impersonated handshake, and `StealthSession` uses it (macOS and Chrome 155
  only; WebSocket is relayed for `wss://` only). The design and the work still open
  are in the [Stealth and impersonation plan](../design/stealth_parity_plan.md).
* Some dimensions (WebGL vendor/renderer, canvas/audio noise, media devices,
  fonts, battery, etc.) have no CDP override and remain JavaScript-based even
  when `native_spoofing` is enabled.
* Chromium source patching is not performed. The binary patchers edit byte
  strings in an existing `chromedriver` or Chrome binary.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Still flagged by bot detection | The evasion script was never installed (`DriverMode::WebDriver`), the masking modes are `Natural`, or the site checks what this page does not change: the browser's own TLS and HTTP/2 fingerprint (see [Stealth Scraping](stealth_scraping.md)), the IP address, the browser's real version | Use `DriverMode::Uc` or the Pure CDP engine, start from `StealthFlags::balanced()`, and match the egress IP to the identity's place. |
| `navigator.webdriver` still `true` | The bootstrap script did not run: `DriverMode::WebDriver`, or no fingerprint on the config. The `webdriver` provider has no flag that turns it off. | Use `DriverMode::Uc` or `Cdp`, or `Page::apply_fingerprint` on the Pure CDP engine. For driver-level markers, see binary patching. |
| Geolocation is wrong | No `latitude` and `longitude` on the fingerprint, no CDP session, or the page lacks the geolocation permission | Set `.geolocation(lat, lon)`, use `DriverMode::Uc` or `Cdp`, and allow geolocation in the browser (`grant_permissions` does not). |
| Inconsistent locale and time zone | `locale` and `timezone` come from different regions. `validate()` does not check that pair. | Align both, or use `Fingerprint::randomized`, which draws them from one place. |
| WebRTC leaks local IP | `webrtc_masking` is `Natural`, or `webrtc_policy` is a looser policy | Set `webrtc_masking` to `Mask` and keep `WebRtcPolicy::DisableNonProxiedUdp`. |

---

Verified against commit 8e9180f, by reading `src/stealth/**`,
`src/sb_cdp/identity.rs`, `src/sb_cdp/launch.rs`, `src/browser/{config,session,playwright}.rs`,
`src/profile_payloads/profile.rs`, `src/mcp/stealth.rs` and `src/macros.rs`.
The screen, `navigator_props`, `client_hints`, `session_registry` and TLS passages
were brought up to commit c5a6f2f by reading `src/sb_cdp/identity.rs`,
`src/stealth/evasions.rs` and `src/stealth/providers/{mod,builtin}.rs` again.
The code examples were checked against the signatures in those files; they were
not compiled or run for this check.
