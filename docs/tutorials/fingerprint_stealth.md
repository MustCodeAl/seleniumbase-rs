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
- How to generate a random but coherent `Fingerprint` from a seed.
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

A fingerprint reaches the page only through a launch path that applies it:

- With `DriverMode::Uc` (above) or `DriverMode::Cdp`, the session registers the
  evasion script and sends the protocol overrides described under
  [Native-level (CDP) spoofing](#native-level-cdp-spoofing), and the launch
  arguments apply.
- With the default `DriverMode::WebDriver`, only the launch arguments apply (user
  agent, `--lang`, `--window-size`, the WebRTC policy, and the QUIC and proxy
  flags). No script is injected and no protocol override is sent.
- The Pure CDP engine and the Playwright session have their own paths, described
  below.

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
| `navigator_masking` | `MaskingMode` | `Natural` | `Mask` | The `navigator_props`, `permissions` and `plugins` providers. It also switches on `chrome_runtime` and `localization`. |
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
| `connection_masking` | `MaskingMode` | `Natural` | `Mask` | The `connection` provider. |
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
| `Emulation.setDeviceMetricsOverride` | `screen_width` and `screen_height` are set. `pixel_ratio` is the scale factor (1.0 if unset) and `mobile` follows `os_type`. |
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
  `InvalidConfig` error naming `SB_CHROME_BIN` if no Chrome is found.

### Caveats

* The protocol overrides and the script need a CDP connection. In
  `DriverMode::WebDriver`, neither is applied whether or not `native_spoofing` is
  set: only the launch arguments (and, with `native_spoofing`, the patched
  binary) take effect.
* `colorDepth` and `pixelDepth` are not covered by the device metrics, so a
  minimal JS patch is still emitted for them.
* Some signals (WebGL, canvas noise, audio noise) have no protocol override and
  stay JavaScript-based in native mode.
* Whether a given detector can still tell is not something this crate measures;
  the Pure CDP guide records what was checked against a real Chrome.

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

A fingerprint from `Fingerprint::randomized` passes this check with no warnings
for every seed the tests try (1,500 per OS, plus edge cases). If you edit a
generated profile by hand, run `validate()` again afterwards.

## Humanized input

Detectors look for input that is too regular. The
[`behavior`](crate::stealth::behavior) module plans input that is not: curved
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

The older [`humanize`](crate::stealth::humanize) helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a constant-speed curve and uniform
delays; prefer `behavior`.

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
