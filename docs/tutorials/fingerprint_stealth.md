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
profiles, say per setting whether the browser's own value, a drawn value, a given
value or nothing is wanted, choose how the claims are delivered, and apply
native-level spoofing through CDP. It covers what a page can read inside the
browser. The TLS and HTTP/2 side, and the scraper built on it, is a separate
project now; see [Stealth Scraping](/tutorials/stealth_scraping).

## What you will learn

- How to use built-in fingerprint presets.
- How to generate a random but coherent `Fingerprint` from a seed.
- How to build a custom `Fingerprint`, one `Spoof` per setting.
- What `Natural`, `Random`, `Value` and `Off` mean, and where `Off` exists.
- How a `StealthPolicy` delivers the claims, and why it never changes a setting.
- How to enable native-level CDP spoofing.
- Which launch paths apply a fingerprint, and what each one does.
- How an external profile payload maps onto all of this.
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
  arguments apply. The script is the one the Pure CDP engine runs,
  `evasions::session_script` (the 27 providers of `session_registry`); see
  [Built-in providers](#built-in-providers).
- With the default `DriverMode::WebDriver`, only the launch arguments apply (user
  agent, `--lang`, `--window-size`, the WebRTC policy, and the QUIC and proxy
  flags). No script is injected and no protocol override is sent.
- The Pure CDP engine and the Playwright session have their own paths, described
  below. The Pure CDP engine runs the same `session_script`. The Playwright
  session runs `bootstrap_script`, the 29 providers of `default_registry`.

## One setting, four states

Every setting of a fingerprint is one `Spoof`, and the state says everything
about it. The value and the intent are one field, so they cannot disagree.

| State | Meaning |
|---|---|
| `Natural` | The browser's own value, left alone. This is the default of every setting. |
| `Random` | A valid value, coherent with the rest, drawn when the fingerprint is built. Repeatable with `.seed(u64)`. |
| `Value(v)` | Exactly this value. A plain value converts to it: `.timezone("Europe/Berlin")` is `Value`. |
| `Off` | The API is turned off: unavailable, or denied. Only for geolocation, WebRTC, media devices, battery, Bluetooth, speech voices, network information and Client Hints. |

`build()` returns an error that names the setting when `Off` is given to any
other, so a time zone cannot be "off" by accident.

```rust
use seleniumbase_rs::stealth::fingerprint::{CanvasNoise, Fingerprint, Spoof, StealthPolicy};

let fp = Fingerprint::builder()
    .timezone("Europe/Berlin")
    .screen(1920, 1080)
    .geolocation(Spoof::Random)
    .webrtc(Spoof::Off)
    .policy(StealthPolicy::balanced())
    .canvas_noise(CanvasNoise::Random)
    .build()?;
# Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
```

The random position is a spot in Berlin, because the time zone is a `Value`.

A built fingerprint is never changed. Its fields are private and are read
through accessors (`fp.timezone()` returns `&Spoof<String>`); to change one,
start a builder from it:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::windows_desktop();
let tokyo = fp.to_builder().timezone("Asia/Tokyo").build()?;
assert_eq!(tokyo.timezone().as_str(), Some("Asia/Tokyo"));
# Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
```

## Fingerprint presets

Built-in presets set a coherent combination of user agent, platform, screen,
WebGL strings, a time zone, and the policy
`StealthPolicy::balanced()`:

| Preset | What it claims |
|---|---|
| `Fingerprint::windows_desktop()` | Chrome 156 on Windows 10, 1920x1080, 8 cores, 8 GB, `en-US`, `America/New_York`, an NVIDIA RTX 4070 Ti through ANGLE and Direct3D 11. |
| `Fingerprint::edge_desktop()` | The Windows preset as Microsoft Edge 156. The user agent ends with `Edg/156.0.0.0`, and the Client Hints name `Microsoft Edge` where Chrome names itself `Google Chrome` (`randomize::edge_hints`; Edge's full versions are not in the Chrome table, so they are the frozen `156.0.0.0`). |
| `Fingerprint::macos_desktop()` | Chrome 156 on macOS (`Macintosh; Intel Mac OS X 10_15_7`), 1920x1080, `America/Los_Angeles`, and the WebGL strings Chrome reports on a Mac: `Google Inc. (Apple)` and `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)`. The user agent is Chrome's, not Safari's. |
| `Fingerprint::linux_desktop()` | Chrome 156 on Linux x86_64, 1920x1080, `America/Chicago`, an NVIDIA RTX 4070 through ANGLE over OpenGL (`Google Inc. (NVIDIA Corporation)`), worded as a generated Linux identity words it. |
| `Fingerprint::android_mobile()` | Chrome 156 on Android, in Chrome's frozen user agent (`Linux; Android 10; K`), 412x915 at a device pixel ratio of 3, 5 touch points, Adreno 740. |
| `Fingerprint::ios_mobile_safari()` | Mobile Safari 26.0 on an iPhone (`iPhone OS 18_6`, the iOS version Safari 26 freezes in its user agent), 390x844 at a device pixel ratio of 3, 5 touch points, `Apple GPU`. Safari has no battery, network information, Web Bluetooth or Client Hints, so those are `Off`, and it has no `navigator.deviceMemory`, so none is claimed. |

Every preset also carries a battery, network information, the speech voices of
its system and Web Bluetooth (where its browser has them). It leaves the position
`Natural` on purpose: a position is answered without a prompt, and a fresh
browser answers `permissions.query` for geolocation with `prompt`, so a preset
that claimed one would stand out. `validate()` warns about a time zone without a
position only when the policy grants permissions, which hands pages the real
position. All six presets
validate without errors or warnings; `tests/mcp_webdriver.rs` and the unit tests
of `stealth::fingerprint` check it. A preset holds no `Random`, so it is the same
every time and its canvas noise is the same on every launch.

Use the `fingerprint!` macro for shorter syntax. `safari` is an alias for `ios`,
and `edge` is the Edge preset:

```rust
use seleniumbase_rs::fingerprint;

let fp = fingerprint!(windows);
let fp = fingerprint!(edge);
let fp = fingerprint!(macos);
let fp = fingerprint!(linux);
let fp = fingerprint!(android);
let fp = fingerprint!(ios);
let fp = fingerprint!(safari); // same as ios
```

## Randomised identities

A preset is one identity; a fleet of sessions that all share it is easy to
spot. `Fingerprint::randomized(os, seed)` generates a different, believable
identity for each seed instead. It is the builder with the settings of a persona
set to `Random`, so you can draw the same way for any setting yourself:

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
| Extras | Battery, network information, speech voices and Web Bluetooth where the browser has them, and `Off` where it has not (Safari). |
| Noise | The fingerprint's `seed` feeds the canvas and audio noise of the policy, and every seed is different. |

All five `OsType` values are supported, with `StealthPolicy::balanced()`.

Things to know:

- **Stable within a release.** A seed maps to the same identity for one version
  of the crate. Adding a table row changes what a seed maps to, so store the
  generated `Fingerprint` (it is `Serialize`), not the seed, if an identity has
  to outlive an upgrade.
- **The Chromium version is a fixed window.** At this version of the crate the
  user agent claims Chromium 154, 155 or 156 (an iPhone identity claims Mobile
  Safari 18.1, 18.3.1, 18.5 or 26.0; Safari 26 names the frozen `iPhone OS 18_6`
  in its user agent). Pair the identity with a browser of that era, or change
  `user_agent`, `client_hints` and `core_version` together.
  `Fingerprint::randomized_with_major(os, seed, major)` fixes the Chromium major
  instead (Safari and iOS ignore it), and so does `.core_version(major)` on a
  builder that draws a user agent or Client Hints; the stealth scraper of
  seleniumbase-rs-stealth uses it so the identity claims the Chrome that is
  running (see [Stealth Scraping](/tutorials/stealth_scraping)).
- **iOS differs.** Safari has no `navigator.deviceMemory` and no
  `navigator.userAgentData`, so an iOS identity leaves `device_memory` `Natural`
  and turns `client_hints` `Off`.
- **Client Hints carry a real full version.** The user agent keeps Chrome's
  frozen `major.0.0.0`, but `uaFullVersion` and `fullVersionList` name a real
  build from `randomize::KNOWN_FULL_VERSIONS`: 154.0.8037.100 and
  156.0.8078.35 from Google's published stable releases, and 155.0.8059.40 as
  Chrome 155 reported it on a Mac (`tests/client_hints_chrome.rs`). A major the
  table does not list (one forced with `randomized_with_major`) falls back to
  `major.0.0.0`.
- **Platform versions.** A Mac claims macOS 15.8.1, 26.7.1, 27.0.0 or 27.0.1, in
  the `major.minor.patch` form Chrome reports (`27.0.1` was read from Chrome 155
  on macOS 27.0.1). The Windows list (`10.0.0`, `13.0.0`, `15.0.0`, `19.0.0`)
  follows Microsoft's description of the value and was not read from a Windows
  Chrome.
- **Not drawn by `randomized`:** fonts, media devices and WebRTC addresses. Ask
  for them with `Spoof::Random` on a builder; the proxy has no random value.
- **Applying it.** For the Pure CDP engine, hand it to the launch options with
  `LaunchOptions::builder().fingerprint(&fp)` (see
  [Pure CDP](/tutorials/pure_cdp#looking-like-another-browser)); `page.apply_fingerprint(&fp)`
  does the same for one tab. For `BaseCase`, pass it to
  `BrowserConfig::with_fingerprint` and use `DriverMode::Uc` or `DriverMode::Cdp`.
  You can also feed it to the evasion registry yourself.

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
    .webgl(("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 ...)"))
    .geolocation((40.7128, -74.0060))
    .media_device_counts(1, 1, 2) // audio inputs, audio outputs, video inputs
    .build()?;
# Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
```

There is no flag to flip afterwards, and none to set. Giving a value is the
intent to use it, so the fingerprint above masks its navigator, language, screen,
WebGL, time zone, position and media devices. `Fingerprint::is_masked(Dimension)`
is the one question the page-script providers, the launch arguments, the protocol
overrides and the proxy all ask, so a value is never half applied (a user agent
sent to the browser but not to the page, say).

### The settings

Every setter takes `impl Into<Spoof<T>>`. `screen(width, height)` is the common
case of `screen_spoof`. A tuple is the value of `geolocation` (`(lat, lon)`),
`webgl` (`(vendor, renderer)`), `webrtc` (`(public_ip, local_ip)`), `avail_screen`
and `media_devices` (`(audio_in, audio_out, video_in)`).

| Setting | `Random` draws | `Off` means | Dimension | What then acts |
|---|---|---|---|---|
| `user_agent`, `platform`, `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `product_sub`, `app_version` | The machine's value for the operating system. | no `Off` | `Navigator` | The `navigator_props` and `plugins` providers, `--user-agent`, `Network.setUserAgentOverride`. |
| `oscpu`, `build_id` | nothing to draw (an error) | no `Off` | `Navigator` | The `navigator_props` provider. |
| `locale`, `languages`, `accept_languages` | The place's locale and language list. | no `Off` | `Localization` | The `localization` provider, `--lang`, `Emulation.setLocaleOverride`, and the `Accept-Language` of the user agent override. |
| `timezone` | The place's time zone. | no `Off` | `Timezone` | The `timezone` provider and `Emulation.setTimezoneOverride`. |
| `screen_spoof`, `avail_screen`, `pixel_ratio`, `color_depth` | The machine's screen. | no `Off` | `Screen` | The `window_geometry` provider, `--window-size` and `Emulation.setDeviceMetricsOverride`. |
| `webgl` | An adapter the operating system reports. | no `Off` | `Graphics` | The `webgl` provider. |
| `fonts` | The system's fonts, with a few of the less common left out. | no `Off` | `Fonts` | The `fonts` provider. |
| `media_devices` | A microphone, a speaker and, usually, a camera. | No device is listed. | `MediaDevices` | The `media_devices` provider. |
| `geolocation` | A spot in the place's main city. | The request is denied (`PERMISSION_DENIED`). | `Geolocation` | The `geolocation` provider, `Emulation.setGeolocationOverride`, and the geolocation permission for a position. |
| `webrtc` | A public and a local address. | `RTCPeerConnection` and the rest of WebRTC are not there. | `WebRtc` | The `webrtc` provider and `--force-webrtc-ip-handling-policy`. |
| `battery` | A battery that is mostly charged. | `getBattery` is not there. | `Battery` | The `battery` provider. |
| `connection` | A 4G connection. | `navigator.connection` is not there. | `Connection` | The `connection` provider. |
| `speech_voices` | The system's voices. | No voice is listed. | `Speech` | The `speech` provider. |
| `bluetooth` (`true`: an adapter, `false`: none) | Whether an adapter is there. | `navigator.bluetooth` is not there. | `Bluetooth` | The `bluetooth` provider. |
| `client_hints` | Client Hints built the way Chromium builds them. | `navigator.userAgentData` is not there. | `ClientHints` | The `client_hints` provider, and the metadata of the user agent override. |
| `proxy` | nothing to draw (an error) | no `Off` | `Proxy` | `--proxy-server`. A proxy without a host is an error. |

A `Natural` setting changes nothing, so the providers define only the values the
fingerprint holds.

### How Random stays coherent

`build()` works out the operating system (the explicit one, else the one the user
agent names, else Windows), then the place: the region of a given time zone, else
of a given locale, else a drawn one. Every `Random` setting is taken from one
seeded draw of that machine and that place, so:

- a random locale agrees with a given time zone, and a random time zone with a
  given locale;
- a random position is in the main city of that time zone;
- a random graphics adapter is one the operating system reports, and random
  Client Hints name the Chromium major the user agent names.

When nothing coherent can be drawn (a random locale next to a time zone the table
of places does not know), `build()` returns a `FingerprintError` that says which
setting and why. `.seed(u64)` makes the draw repeatable. Without it the draw uses
fresh entropy, and the seed it used is kept in the fingerprint (`fp.seed()`), so
`fp.to_builder().build()` reproduces it.

`build()` also fills two values the caller left out: `languages`, from a given
locale (the table's list for a known locale, else the locale and its language),
and `navigator.platform` and the operating-system persona, from a recognised user
agent. A value you set is never replaced.

## The policy

A setting is what the browser should *claim*. The `StealthPolicy` is *how* the
claims are delivered and which protections run beside them. It has no say over a
claim: a preset policy never changes a setting's state, and the two can be given
in any order.

```rust
use seleniumbase_rs::stealth::fingerprint::{CanvasNoise, Fingerprint, StealthPolicy};

let fp = Fingerprint::builder()
    .canvas_noise(CanvasNoise::Low)        // a shortcut, applied after the base...
    .grant_permissions(true)
    .policy(StealthPolicy::balanced())      // ...whatever the order
    .with_policy(|p| p.disable_csp(true))   // any change, in order
    .build()?;
assert!(fp.policy().chrome_runtime && fp.policy().disable_csp);
# Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
```

| Preset | What it is |
|---|---|
| `StealthPolicy::default()` | Only what the other evasions rely on: `toString` masked and shadow roots opened. |
| `StealthPolicy::balanced()` | The default, plus the WebRTC leak policy `DisableNonProxiedUdp`, seeded canvas and audio noise, the `window.chrome` stubs and the headless fixes. Every preset and every `randomized` identity uses it. |
| `StealthPolicy::native_only()` | Delivers the claims through CDP and launch arguments where the browser can (`native_spoofing`), with the WebRTC leak policy; no noise, no JavaScript-only stubs. |
| `StealthPolicy::max()` | `balanced()`, plus tracker blocking, granted permissions and QUIC off. |

| Field (builder shortcut) | What it controls |
|---|---|
| `webrtc_policy` (`.webrtc_policy(..)`) | `--force-webrtc-ip-handling-policy`. `None` passes no flag, unless the `webrtc` setting is set, which uses `DisableNonProxiedUdp`. A policy also runs the `webrtc` provider's leak filter. |
| `canvas_noise` (`.canvas_noise(..)`) | The `canvas_noise` provider: `Persistent` (seeded), `Random` (a fresh seed for each document), `Low` (one pixel in eight) or `Natural` (none). |
| `audio_noise` (`.audio_noise(..)`) | The `audio_noise` provider: `Persistent`, `Random` or `Natural`. |
| `native_spoofing` (`.native_spoofing(..)`) | See [Native-level (CDP) spoofing](#native-level-cdp-spoofing). |
| `native_tostring` | The `native_tostring` provider, on unless switched off. The other evasions rely on it. |
| `chrome_runtime` | The `chrome_runtime` provider (the `window.chrome` stubs). |
| `headless_fixes` | The `headless`, `permissions` and `hairline` providers, and `Emulation.setFocusEmulationEnabled`. |
| `grant_permissions` (`.grant_permissions(..)`) | `Browser.grantPermissions` for `notifications`, `midi`, `midiSysex`, `clipboardReadWrite`, `clipboardSanitizedWrite`, `paymentHandler`, `backgroundSync`, `idleDetection` and `webAppInstallation`. A position grants the geolocation permission itself. |
| `block_trackers`, `blocked_trackers` | The `tracker_block` provider and `Network.setBlockedURLs` for `Fingerprint::tracker_hosts()`. A tracker list is itself the request to block them. |
| `blocked_ports` (`.blocked_ports(..)`) | Ports refused on `localhost`, `127.0.0.1` and `[::1]` with `Network.setBlockedURLs`, in the same command as the tracker hosts. |
| `quic` (`.quic(..)`) | `Disabled` and `ForceHttp2` add `--disable-quic`; `Enabled` adds `--enable-quic`; `Natural` and `Auto` add nothing. |
| `disable_csp` (`.disable_csp(..)`) | `Page.setBypassCSP`. |
| `open_shadow_roots` | The `attach_shadow` provider, which forces closed shadow roots open so automation can reach into them. A page can detect that, so turn it off where it matters. |

`StealthFlags` is a deprecated alias of `StealthPolicy`, kept for one release.

### Fingerprints as JSON

A `Spoof` is written as `"natural"` (and left out), `"random"`, `"off"` or the
value itself; `null` reads as `Natural`. A fingerprint that holds `"random"` is
drawn when it is read, the same way `build()` does it, so give a `seed` for a
repeatable file. A field this version does not know, such as the flat
`screen_width` of the old shape, is an error and not a silent loss. Battery times
that are infinite (`discharging_time` of a charging battery) are written as
`null`.

## Native-level (CDP) spoofing

When a session has a CDP connection (`DriverMode::Uc`, `DriverMode::Cdp`, or the
Pure CDP engine), a fingerprint is applied in two layers:

1. The evasion script, assembled from the providers below and registered with
   `Page.addScriptToEvaluateOnNewDocument`. Page scripts can *in principle*
   detect a JavaScript patch through `Function.prototype.toString`,
   `Object.getOwnPropertyNames`, or descriptor inspection, which is why the
   providers use the `sb` helpers to look native.
2. The protocol overrides from `evasions::safe_cdp_overrides`. (The older
   `evasions::cdp_overrides` is deprecated: it also holds the `Proxy-Authorization`
   header that would hand the proxy's login to every site.) A command is sent
   for a setting that holds a value, so a `Natural` setting sends nothing:

| Command | Sent when |
|---|---|
| `Network.setUserAgentOverride` (user agent, platform, `userAgentMetadata`, and `acceptLanguage`) | `user_agent` is a value. `acceptLanguage` is sent only when `accept_languages` or `locale` is a value. The metadata comes from `client_hints` when set and is derived from the OS and user agent otherwise: Chrome's brand list, or Edge's when the user agent has an `Edg/` token. `client_hints` `Off` sends none. |
| `Emulation.setDeviceMetricsOverride` | `screen` is a value. `pixel_ratio` is the scale factor (1.0 if unset) and `mobile` follows `os_type`. `safe_cdp_overrides` sets the viewport (`width`, `height`) to the screen, and the WebDriver path sends it as it is. The Pure CDP engine changes it for a desktop persona: it sends a `0` by `0` viewport, which the protocol reads as "no override", plus `screenWidth` and `screenHeight`, so the browser itself reports `screen.width` and `screen.height` and the page keeps the viewport its window really has. A mobile persona is a phone-sized page, so its viewport stays the screen. |
| `Emulation.setLocaleOverride` | `locale` is a value. |
| `Emulation.setTimezoneOverride` | `timezone` is a value. |
| `Emulation.setGeolocationOverride` | `geolocation` is a position (accuracy 100 and altitude 0 unless set). |
| `Browser.grantPermissions` | The geolocation permission, when `geolocation` is a position, and the permissions listed under `grant_permissions` when that policy field is on. |
| `Network.setBlockedURLs` | One command, because a second would replace the first: the tracker hosts when the policy blocks trackers, and `*://localhost:PORT/*` (and `127.0.0.1`, `[::1]`) for each of `blocked_ports`. A session with ad blocking joins the ad block list to it. |
| `Page.setBypassCSP` | `disable_csp` is on. |
| `Emulation.setFocusEmulationEnabled` | `headless_fixes` is on. |

So a UC or CDP session already gets these overrides without `native_spoofing`,
and the JavaScript providers patch the same properties on top of them.
`native_spoofing` makes the providers stand down where a protocol override covers
the property, so the browser reports the value itself instead of a replaced
JavaScript getter:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …")
    .platform("Win32")
    .screen(1920, 1080)
    .locale("en-US")
    .timezone("America/New_York")
    .geolocation((40.7580, -73.9855))
    .native_spoofing(true)
    .build()?;
# Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
```

When `native_spoofing` is true:

* `navigator_props` leaves out `platform`, `userAgent` and `appVersion` (a given
  `app_version` is still scripted).
* `window_geometry` emits only `colorDepth` and `pixelDepth`; the size and scale
  come from `Emulation.setDeviceMetricsOverride`.
* `localization`, `timezone`, the JavaScript `geolocation` provider that answers a
  position, and `client_hints` are skipped. A denied geolocation (`Off`) is still
  scripted: no protocol command denies the request for one tab.
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

<Warning>

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

</Warning>

## rustwright / Playwright mode

<Info>

This mode needs the `playwright` Cargo feature (`rustwright`); it is not on by
default.

</Info>

```rust
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::{BrowserConfig, Fingerprint};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fp = Fingerprint::windows_desktop();
let config = BrowserConfig::default()
    .with_headless(false)
    .with_fingerprint(fp);
let session = PlaywrightSession::launch_with_config(&config).await?;
session.goto("https://example.com").await?;
session.close().await?;
# Ok(())
# }
```

`BrowserConfig::default()` is headless; `.with_headless(false)` opens a window.
(`launch_with_fingerprint` and `launch_headless_with_fingerprint` are
deprecated shortcuts for this.) The session passes the
fingerprint's launch arguments, then evaluates the evasion script in the first
page and again after every `goto` and `new_page`, because rustwright cannot
register a script to run in each new document.

<Warning>

The script therefore runs after navigation, not before the page's own scripts,
so a page that reads the identity while it loads can see the browser's real one.
The protocol overrides listed above are not sent in this mode.

</Warning>

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

The payload keeps its own format, because other tools write it: the flags are
strings (`ProfileFlags`) and the nested sections (`navigator`, `screen`, ...)
hold values. Every key reads the same words and ends in one of the four states of
a `Spoof`. The profile's `folder_id` and `name` seed the draws, so a profile gets
the same drawn values on every launch, and its `core_version` is the Chromium
major that a drawn user agent and drawn Client Hints claim (not for the Firefox
and Safari personas, whose core is not a Chrome version).

### Words every key reads

Keywords are case-insensitive. A flag the payload leaves out is `auto`. The
table below is the one place that lists the words and the literal format of each
key; this is what the groups of words do:

- The **natural** words give the browser's own value, whatever values the payload
  holds for it.
- **`auto`** (or no flag) gives the value the payload holds for the setting, and
  the browser's own where it holds none.
- The **mask** words give the value the payload holds, and a coherent drawn one
  where it holds none.
- The **off** words turn the API off. Only where an API can be off (geolocation,
  WebRTC, media devices); for any other key they are read as `Natural`, and logged.
- **A literal** is the value, in the format of its key. The other settings of the
  same key then follow the mask words: a held value, else drawn.

An unknown keyword is logged and read as `Natural`. So is a literal that is not in
the format of its key (`wibble` for a time zone, a user agent that does not open
with `Mozilla/` and a digit). It is never guessed at: it used to switch the
dimension on and fill it with generic values.

### Keywords in JSON payloads

One table for every key. The first row says what the words mean; each later row
names only what the key adds or does differently, and the literal formats it
reads.

| Payload key | Natural | Random | Value (literal) | Off |
|---|---|---|---|---|
| (the words above) | `natural`, `native`, `pass_through`, `whitelist`; `auto` with no value held | `mask`, `spoof`, `custom`, `generic`, `random`, `noise` | a value of the key | `off`, `disabled`, `block`, `empty` |
| `navigator_masking` | the common words | the common words, `desktop_chrome`, `mobile_safari`: user agent, platform, cores, memory, touch points and vendor | a user agent: `Mozilla/` and a digit, with the platform in parentheses (`Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...`); it sets `user_agent` | read as Natural |
| `localization_masking` | the common words | the common words: locale, languages, accept-languages of the place | a locale (`de-DE`, `zh-Hant-TW`) or an `Accept-Language` list that opens with one (`en-US,en;q=0.9`): the first tag is the `locale`, the tags the `languages`, the text the `accept_languages` | read as Natural |
| `timezone_masking` | the common words | the common words: the place's zone | an IANA name, area and city: `America/New_York` | read as Natural |
| `screen_masking` | the common words | the common words: size, pixel ratio and colour depth of the machine | `1920x1080` or `1920x1080x24` (24 when omitted); it sets `screen` and `color_depth` | read as Natural |
| `graphics_masking` | the common words | the common words: an adapter the OS reports | `vendor~renderer`, both given: `Intel Inc.~Intel(R) Iris(R) Xe Graphics` | read as Natural |
| `fonts_masking` | the common words | the common words, `system_only`, `blacklist`: the system's fonts | a comma-separated list of fonts: `Arial,Calibri,Consolas`. One font is no list: give it in `fingerprint.fonts` | read as Natural |
| `media_devices_masking` | the common words | the common words | `2_mic_1_cam`, or `kind:id:label\|kind:id:label` (`audioinput:default:Mic1\|videoinput:default:Cam1`): the counts and the labelled devices | the off words: no device is listed |
| `geolocation_masking` | the common words | the common words: a spot in the place's city | `lat,lon` or `lat,lon,altitude` (accuracy 20): `40.7128,-74.0060,10.5` | the off words: the request is denied |
| `webrtc_masking` | the common words (and no IP policy) | the common words: a public and a local address | `public:IP\|local:IP`, IPv6 in brackets: `public:172.56.21.89\|local:10.0.0.5`; it also passes the policy `PublicAndPrivateInterfaces` | the off words: WebRTC is unavailable |
| `proxy_masking` | the common words, `direct`: no proxy | none: `mask`, `custom`, `socks5`, `http`, `https` use the payload's proxy section, as `auto` does | a `socks5://`, `http://` or `https://` URL: `socks5://user:pass@host:1080` (port 1080 for SOCKS5, 8080 otherwise) | read as Natural, and logged: a proxy has no off state |
| `geolocation_popup` | `prompt`: ask, whatever position the payload holds | not used | not used | `block`: deny, whatever position the payload holds; `allow` and `auto` follow `geolocation_masking` |
| `audio_masking` | the common words and the off words: no audio noise | `random`: fresh noise for each script; `mask`, `spoof`, `custom`, `generic`, `noise`: noise seeded by the profile | not used | not used |
| `canvas_noise` | `natural`, `auto`, `off`, `disabled`: no canvas noise | `random`: fresh noise for each script | `low`: noise on one pixel in eight; `persistent`, `mask`, `spoof`, `custom`, `generic`, `noise`: seeded noise. If the key is absent: seeded noise | not used |
| `ports_masking` | the common words, the off words: nothing blocked | the common words, `block`, `block_all`, `stealth`: the ports of the payload's `fingerprint.ports` are blocked | `block:80,443` or `80,443`: those ports (the policy's `blocked_ports`) | not used |
| `quic_mode` | `natural`, `auto` | not used | `enabled`, `disabled`, `force_http2`. If the key is absent: `disabled` | not used |

The audio, canvas, ports and QUIC keys set the policy and not a setting, so their
"states" are the values of the policy field in the same columns.
`graphics_noise` and `startup_behavior` are accepted and ignored. The booleans
`disable_csp`, `grant_permissions` and `native_spoofing` set the same fields of
the policy.

The settings of one key follow the word together: `navigator_masking: "mask"`
with a `navigator` section that gives a user agent, a platform and a core count
keeps those three and draws the memory, the touch points and the vendor, in the
same machine. A key whose draw cannot be made (a random locale next to a time
zone the table of places lacks) is logged and left `Natural`.

A literal in a flag is the value for the setting it names (the Value column
above). A section of the payload that gives the same setting wins over a literal,
and a section value that is an empty string is a value the payload does not give.
What the browser is launched with (`to_browser_config`) and what the runtime
overrides set (`apply_runtime_overrides`) are the built fingerprint's: a `natural`
flag launches without a `--user-agent`, a locale or a proxy, whatever the payload
holds.

`webrtc_masking` also sets the IP-handling policy that keeps WebRTC from leaking
the real address: every word but the natural ones passes `disable_non_proxied_udp`
(including a payload that says nothing), `public_only` passes
`default_public_interface_only`, and a literal address passes
`default_public_and_private_interfaces`.

### Payloads from older versions

A payload written for an older version loads unchanged, with the words above. The
differences you can see: an absent flag is `auto` (it was `mask` for the
navigator, localization, time zone, graphics, fonts, screen, geolocation and
WebRTC flags, which filled in generic values nobody asked for); `off` and
`disabled` turn geolocation, WebRTC and media devices off instead of leaving them
natural; an unknown word is `Natural` instead of `mask`.

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
keep working while new evasions are added centrally. `evasions::session_script(&fp)`
is the same over `session_registry`, the script a browser session runs.

### Adding your own evasion

<Steps>

<Step title="Implement EvasionProvider">

Implement `EvasionProvider` for a unit struct. Return the JavaScript from
`script`, gate inclusion with `applies`, and order it with `priority`.
`providers::SB_PREAMBLE` brings in the `sb` helpers that make a replaced
function look native; the example below skips it for brevity.

</Step>

<Step title="Register it at runtime">

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

</Step>

</Steps>

See [Contributing](/CONTRIBUTING) for adding a built-in provider.

### Built-in providers

`default_registry()` registers 29 providers, listed here in execution order. "Runs
when" is the provider's `applies` condition. A dimension "is on" when
`Fingerprint::is_masked` says so: a setting that belongs to it is not `Natural`,
or, for the dimensions without a setting of their own, the policy switches it on
(see [One setting, four states](#one-setting-four-states)).

The Pure CDP engine and the WebDriver `Uc` and `Cdp` modes do not use
`default_registry()`. They use `evasions::session_script(&fp)`, which is
`session_registry(&fp)`: the same list without `plugins` and `connection`
(`SESSION_OMITTED_PROVIDERS`), which is 27 providers. A real Chrome already has
`navigator.plugins`, `navigator.mimeTypes`, `navigator.connection` and the PDF
viewer flag, and answers for them itself, so a replaced getter is one more thing
a page can find that a real Chrome does not have. The assembled session script
never mentions `plugins`, `mimeTypes`, `pdfViewerEnabled` or `connection`.
`default_registry()` is unchanged, for callers that want those spoofs, such as a
browser that really lacks them, and it is what `evasions::bootstrap_script` and
the Playwright session use.

| Priority | Name | Runs when | What it does |
|---|---|---|---|
| 5 | `native_tostring` | the policy's `native_tostring` is on | Makes replaced functions print `[native code]` from `Function.prototype.toString`, and installs the `sb` helpers the other providers use. |
| 10 | `webdriver` | always | Makes `navigator.webdriver` read `false` (a getter on the navigator prototype) and deletes known automation hooks from `window` (`callPhantom`, `_phantom`, `__nightmare`, `__selenium_unwrapped`, `__webdriver_evaluate`, `__driver_evaluate`, `domAutomation`, and others). |
| 15 | `cdp_markers` | always | Deletes `$cdc_`, `__webdriver`, `__selenium`, `__driver`, `$chrome_` and `__fxdriver` properties from `window`, its prototype chain and `document`. |
| 20 | `chrome_runtime` | the policy's `chrome_runtime` is on | `window.chrome.app`, `loadTimes`, `csi` and `runtime` stubs. |
| 30 | `navigator_props` | Navigator is on | `userAgent`, `appVersion` and `platform` (not under `native_spoofing`), then `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, `productSub`, and `oscpu` and `buildID`, each only when it is a value. A given `app_version` is scripted even under `native_spoofing`. It does not touch `pdfViewerEnabled`, `plugins` or `mimeTypes`: Chrome answers for them itself. |
| 35 | `permissions` | the policy's `headless_fixes` is on | Makes `permissions.query({ name: 'notifications' })` agree with `Notification.permission`. |
| 40 | `plugins` | Navigator is on | Five PDF-viewer plugins and their MIME types. In `default_registry()` only. |
| 45 | `window_geometry` | Screen is on | `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`, `pixelDepth`, plus `outerWidth`, `outerHeight` and `devicePixelRatio`, each only when it is a value. `avail_screen` sets the available size; without it it is the screen less 40 pixels in height. The window is as large as the persona's screen, and never smaller than the viewport it really has. Under `native_spoofing`, only `colorDepth` and `pixelDepth`. |
| 50 | `webgl` | Graphics is on | `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL` from WebGL1 and WebGL2 `getParameter`. A half the `WebGl` value lacks stays the browser's. |
| 55 | `canvas_noise` | the policy's `canvas_noise` adds noise | Deterministic noise, a function of the seed and the pixel position, on `toDataURL`, `toBlob` and `getImageData`. `Low` touches one pixel in eight and `Random` draws a new seed for each script. |
| 60 | `audio_noise` | the policy's `audio_noise` adds noise | Deterministic noise on `AudioBuffer.getChannelData` and `copyFromChannel` and on `AnalyserNode.getFloatFrequencyData`, seeded by the fingerprint's seed (a fresh one for `Random`). |
| 65 | `webrtc` | `webrtc` is set, or the policy names a WebRTC IP policy | `Off`: removes `RTCPeerConnection`, `RTCSessionDescription`, `RTCIceCandidate` and the rest of WebRTC. Otherwise drops STUN and TURN servers from `RTCPeerConnection` configs, and rewrites IPs in SDP and `RTCIceCandidate` for the addresses the value holds. |
| 70 | `battery` | Battery is on | `navigator.getBattery()` from the `BatteryProfile`; `Off` removes it. |
| 72 | `connection` | Connection is on | `NetworkInformation` `effectiveType`, `rtt`, `downlink` and `saveData` from the `ConnectionProfile`; `Off` removes `connection`. In `default_registry()` only. |
| 75 | `media_devices` | MediaDevices is on | `enumerateDevices`, from the labelled devices or generated from the input and output counts; `Off` lists none. |
| 76 | `fonts` | `fonts` is a non-empty list | Limits `document.fonts.check`, `load` and `FontFace` to the configured fonts. |
| 78 | `speech` | Speech is on | `speechSynthesis.getVoices()` from the voices; `Off` lists none. |
| 80 | `bluetooth` | Bluetooth is on | A `navigator.bluetooth` stub whose `getAvailability` answers the value, only if the browser has none; `Off` removes `navigator.bluetooth`. |
| 85 | `headless` | the policy's `headless_fixes` is on | `matchMedia` answers (`prefers-reduced-motion: reduce` and `prefers-color-scheme: dark` false, `any-pointer: fine` true) and `document.hidden` / `visibilityState`. |
| 87 | `prepare_stack_trace` | never | Reserved name. It emits no script. |
| 88 | `client_hints` | ClientHints is on, and not `native_spoofing` | `navigator.userAgentData`, including `getHighEntropyValues`, but only in a secure context (`window.isSecureContext`): that is the only place Chrome has the property, so on a plain `http:` page there is none to read and none is added. `localhost` and `127.0.0.1` are secure contexts. `Off` removes `userAgentData`. |
| 89 | `media_codecs` | always | `canPlayType` answers for MP4, AAC and MP3 types when the browser says it cannot play them. WebM and everything else stay the browser's own. |
| 90 | `timezone` | `timezone` is a value, and not `native_spoofing` | `Intl.DateTimeFormat` and the local-time methods of `Date` (`getTimezoneOffset`, `getHours`, `toString`, the `toLocale*String` family and others). `new Date(y, m, d)` and `Date.parse` of a string without an offset keep the machine's zone. |
| 92 | `localization` | Localization is on, and not `native_spoofing` | `navigator.language` and `navigator.languages`: the `languages` the fingerprint holds, else its `accept_languages`, else the list its `locale` implies. |
| 95 | `geolocation` | `geolocation` is a position and not `native_spoofing`, or it is `Off` | `getCurrentPosition` and `watchPosition`. They answer in JavaScript with the position, and refuse with `PERMISSION_DENIED` for `Off`. |
| 110 | `hairline` | the policy's `headless_fixes` is on | Modernizr's hairline check: `offsetHeight` of the element with id `modernizr` reads `1`. |
| 120 | `iframe` | never | Reserved name. It emits no script. |
| 125 | `attach_shadow` | the policy's `open_shadow_roots` is on | Forces `Element.attachShadow` to `mode: 'open'`. |
| 130 | `tracker_block` | the policy blocks trackers | Makes `fetch`, `XMLHttpRequest` and `navigator.sendBeacon` to the tracker hosts (and their subdomains) fail like an unreachable host. Scripts, images and frames loaded by markup are not covered. |

List them at runtime with `default_registry().provider_names()` (all 29, including
the two reserved ones), `session_registry(&fp).provider_names()` (the 27 a
browser session runs), `applicable_names(&fp)` (those that run for a fingerprint),
or over MCP with the `list_evasion_providers` tool, which lists the session ones.

## Profile coherence validation

`Fingerprint::validate()` returns a `CoherenceReport` with `errors` and
`warnings`; `is_coherent()` is true when there are no errors. Only values that
were given are checked: a `Natural` setting is the browser's own.

It reports an error for:

- a `navigator.platform` that does not fit `os_type` (a macOS user agent with a
  `Win32` platform), and a user agent with no token for `os_type`;
- a screen with a zero dimension;
- a WebGL renderer from another operating system's graphics API: Direct3D off
  Windows, Metal or an Apple GPU off macOS and iOS, Mesa off Linux, and Adreno,
  Mali or PowerVR off Android;
- a `MobileSafari` persona whose user agent has a `Chrome/` token.

It reports a warning for:

- a mobile user agent with `maxTouchPoints == 0`, and a desktop user agent with
  touch points;
- a `locale` that differs from the first of `languages`;
- a time zone with no geolocation (a geolocation that is `Off` is a choice, and
  does not warn);
- a locale from another country than the time zone, judged with the table of
  places `randomize::LOCALES` (a zone the table does not hold, or a locale with
  no region, is not judged).

It does not look at hardware.

```rust
use seleniumbase_rs::Fingerprint;

let report = Fingerprint::windows_desktop().validate();
assert!(report.is_coherent() && report.warnings.is_empty());
for warning in &report.warnings {
    eprintln!("warning: {warning}");
}
```

The MCP `validate_fingerprint` tool returns the same report for one of the six
built-in presets (`windows`, `edge`, `macos`, `linux`, `android`, `ios`); it does
not take an arbitrary fingerprint.

`tests/fingerprint_randomized.rs` asserts that `Fingerprint::randomized` gives no
errors and no warnings for every OS, for seeds 0 to 1499 and ten edge seeds
(`0`, `u64::MAX`, `1 << 63` and others). If you edit a generated profile with
`to_builder()`, run `validate()` again afterwards.

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
engine (see [Pure CDP](/tutorials/pure_cdp#human-paced-input)). `BaseCase::human_type`
uses the same typing model, taking its pace from `Fingerprint::humanize()` when the
configuration has a fingerprint.

`Behavior::from_config(fingerprint.humanize())` turns a fingerprint's
`HumanizeConfig` (its minimum and maximum keystroke delay) into the matching
behaviour.

<Tip>

The older `stealth::humanize` helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a Bézier curve at evenly spaced
parameter values and draw delays uniformly between two bounds; prefer
`behavior`.

</Tip>

## What is spoofed

With a CDP session and the `balanced()` policy (see the provider table for the
conditions), for the values the fingerprint holds:

* `navigator.webdriver` reads `false`, and known automation hooks are deleted
* `navigator.userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`,
  `maxTouchPoints`, `vendor`, `language` and `languages`
* `window.chrome.app`, `csi`, `loadTimes` and `runtime` stubs
* `navigator.plugins` and `navigator.mimeTypes` (`bootstrap_script` and the
  Playwright session; the session script leaves them to Chrome)
* `navigator.permissions.query` for notifications
* `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`,
  `outerWidth`, `outerHeight` and `devicePixelRatio`
* WebGL `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL`
* Canvas `toDataURL`, `toBlob` and `getImageData` noise
* Audio noise (when the policy's `audio_noise` is not `Natural`, which
  `balanced()` and so every preset and every `randomized` identity set)
* `navigator.mediaDevices.enumerateDevices` (when `media_devices` is a value)
* Time zone, through `Intl.DateTimeFormat`, the local-time methods of `Date`, and
  `Emulation.setTimezoneOverride`
* Geolocation, through `Emulation.setGeolocationOverride`, the JavaScript
  provider, and the geolocation permission
* `$cdc_`, `__webdriver`, `__selenium` and similar marker properties
* Battery Status, Speech Synthesis, and Bluetooth APIs, and Network Information
  (the last only in `bootstrap_script`)
* Client Hints (`navigator.userAgentData`), when `client_hints` is a value and the
  page is a secure context; `Off` removes it, as it removes `getBattery`,
  `navigator.connection`, `navigator.bluetooth` and WebRTC
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
  [Binary Patching tutorial](/tutorials/binary_patching).
* Pass the extra Chromium flags returned by `engine_spoofing_args()` (a
  `Vec<String>`) through `BrowserConfig::with_extra_args`, which replaces the
  list (`push_extra_arg` adds one), or through `StealthOptions::extra_args`.
* Combining a `Fingerprint`, UC mode, binary patching, and engine args covers the
  most layers this crate offers. None of it guarantees that a given detector
  passes.

## Limitations

<Warning>

* Nothing on this page changes the TLS or HTTP/2 behaviour of the browser. A
  fingerprint reaches the page through scripts and protocol overrides; the
  `ClientHello`, the HTTP/2 connection start and the header order that leave the
  machine are those of the browser you launch. JA3, JA4, peetprint and Akamai
  parsers, a measured table for one browser (Chrome 155 on macOS), a client that
  sends that handshake or one you specify, and `MitmProxy`, which gives a real
  Chrome on the Pure CDP engine the impersonated handshake, are in the separate,
  experimental seleniumbase-rs-stealth crates since 0.3.0; see
  [Stealth Scraping](/tutorials/stealth_scraping). The design history is the
  [Stealth and impersonation plan](/design/stealth_parity_plan).
* Some dimensions (WebGL vendor/renderer, canvas/audio noise, media devices,
  fonts, battery, etc.) have no CDP override and remain JavaScript-based even
  when `native_spoofing` is enabled.
* Chromium source patching is not performed. The binary patchers edit byte
  strings in an existing `chromedriver` or Chrome binary.

</Warning>

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Still flagged by bot detection | The evasion script was never installed (`DriverMode::WebDriver`), the setting is `Natural`, or the site checks what this page does not change: the browser's own TLS and HTTP/2 fingerprint (see [Stealth Scraping](/tutorials/stealth_scraping)), the IP address, the browser's real version | Use `DriverMode::Uc` or the Pure CDP engine, build the fingerprint from a preset or `Fingerprint::randomized`, check `fp.is_masked(Dimension::...)` for the dimension, and match the egress IP to the identity's place. |
| A value was given and the page does not show it | A later call for the same setting replaced it (`Natural` included), or the profile payload's flag for it is a `natural` word, which beats the values the payload holds | Read the setting back (`fp.timezone()`), and remove the later call or the `natural` word. |
| `build()` fails with "cannot be Off" or "cannot be Random" | `Off` was given to a setting with no off state, or `Random` to one with nothing to draw (`proxy`, `oscpu`, `build_id`) | Give a value or leave it `Natural`; the message names the setting. |
| `build()` fails with "no coherent random" | A random locale, language or position next to a time zone or locale the table of places lacks | Give the locale and the position as values, or use a zone the table holds (`randomize::locale_of_timezone`). |
| `navigator.webdriver` still `true` | The bootstrap script did not run: `DriverMode::WebDriver`, or no fingerprint on the config. The `webdriver` provider has no flag that turns it off. | Use `DriverMode::Uc` or `Cdp`, or `Page::apply_fingerprint` on the Pure CDP engine. For driver-level markers, see binary patching. |
| Geolocation is wrong or never answers | The `geolocation` setting is `Natural` or `Off`, or there is no CDP session | Set `.geolocation((lat, lon))` and use `DriverMode::Uc` or `Cdp`. The position grants the permission itself; `Off` refuses it on purpose. |
| Inconsistent locale and time zone | `locale` and `timezone` come from different countries. `validate()` warns about this pair. | Align both, or use `Fingerprint::randomized`, which draws them from one place. |
| WebRTC leaks local IP | The policy has no `webrtc_policy` or a looser one, and the `webrtc` setting is `Natural` | Use `StealthPolicy::balanced()` (the presets do), which keeps `WebRtcPolicy::DisableNonProxiedUdp`, or turn WebRTC `Off`. |

---

This page was brought up to date with the change that makes a setting one `Spoof`
and moves the behaviour into the `StealthPolicy` by reading `src/stealth/**`,
`src/sb_cdp/{identity,launch}.rs`, `src/browser/{config,session,playwright}.rs`,
`src/profile_payloads/**`, `src/mcp/stealth.rs` and `src/macros.rs` again. The code examples were compiled
against that source as a test of the crate; the tables were checked against the
`applies` conditions of the providers.
