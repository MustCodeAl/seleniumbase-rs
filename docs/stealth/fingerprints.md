---
title: "Fingerprints"
sidebarTitle: "Fingerprints"
---

A `Fingerprint` says what the browser claims to be. It holds one `Spoof` per
setting, so for each one you say whether the browser's own value, a drawn
value, a given value or nothing is wanted. How the claims are delivered is the
[policy](/stealth/policies)'s job; which engines apply them is on the
[overview](/stealth/introduction#which-launch-paths-apply-a-fingerprint).

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
use seleniumbase_rs_stealth::evasion::fingerprint::{CanvasNoise, Fingerprint, Spoof, StealthPolicy};

let fp = Fingerprint::builder()
    .timezone("Europe/Berlin")
    .screen(1920, 1080)
    .geolocation(Spoof::Random)
    .webrtc(Spoof::Off)
    .policy(StealthPolicy::balanced())
    .canvas_noise(CanvasNoise::Random)
    .build()?;
Ok::<(), seleniumbase_rs_stealth::evasion::fingerprint::FingerprintError>(())
```

The random position is a spot in Berlin, because the time zone is a `Value`.

A built fingerprint is never changed. Its fields are private and are read
through accessors (`fp.timezone()` returns `&Spoof<String>`); to change one,
start a builder from it:

```rust
use seleniumbase_rs_stealth::Fingerprint;

let fp = Fingerprint::windows_desktop();
let tokyo = fp.to_builder().timezone("Asia/Tokyo").build()?;
assert_eq!(tokyo.timezone().as_str(), Some("Asia/Tokyo"));
Ok::<(), seleniumbase_rs_stealth::evasion::fingerprint::FingerprintError>(())
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
of `evasion::fingerprint` check it. A preset holds no `Random`, so it is the same
every time and its canvas noise is the same on every launch.

Use the `fingerprint!` macro for shorter syntax. `safari` is an alias for `ios`,
and `edge` is the Edge preset:

```rust
use seleniumbase_rs_stealth::fingerprint;

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
use seleniumbase_rs_stealth::{Fingerprint, OsType};

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
  running (see [Stealth Scraping](/scraping/stealthy/first-fetch)).
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
use seleniumbase_rs_stealth::Fingerprint;

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
Ok::<(), seleniumbase_rs_stealth::evasion::fingerprint::FingerprintError>(())
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

