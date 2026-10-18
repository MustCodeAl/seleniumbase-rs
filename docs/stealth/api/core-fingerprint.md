---
title: "Fingerprint"
sidebarTitle: "Fingerprint"
description: "Reference for Fingerprint, the identity a browser claims: its accessors, the presets, randomized identities, is_masked and Dimension, validate and CoherenceReport, the fingerprint! macro and default_tracker_hosts."
---

A `Fingerprint` says what a browser should claim to be: its navigator, screen,
time zone, position, graphics, media devices, fonts, proxy and the rest of what a
page can probe. It is `seleniumbase_rs::stealth::fingerprint::Fingerprint`
(also `seleniumbase_rs::Fingerprint` and `seleniumbase_rs::stealth::Fingerprint`),
defined in `src/stealth/fingerprint.rs`. Each setting is one
[`Spoof`](/stealth/api/core-spoof); how the claims are delivered is the
[`StealthPolicy`](/stealth/api/core-policy). A fingerprint is built once with a
[`FingerprintBuilder`](/stealth/api/core-fingerprint-builder) and not changed
afterwards. The [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth)
guide walks through building, checking and applying one; giving it to a browser
is on [Evasions and launch](/stealth/api/core-evasions).

<RequestExample>

```rust Build one
use seleniumbase_rs::stealth::fingerprint::{CanvasNoise, Fingerprint, Spoof, StealthPolicy};

let fp = Fingerprint::builder()
    .timezone("Europe/Berlin")
    .screen(1920, 1080)
    .geolocation(Spoof::Random)
    .webrtc(Spoof::Off)
    .policy(StealthPolicy::balanced())
    .canvas_noise(CanvasNoise::Random)
    .build()?;

assert!(fp.timezone().is_value());
assert!(fp.webrtc().is_off());
// The random position is a spot in Berlin, because the time zone is given.
let place = fp.geolocation().value().expect("a drawn position");
assert!((place.latitude - 52.5).abs() < 0.5);
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Change a preset
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::windows_desktop();
assert_eq!(fp.screen().value().map(|s| s.width), Some(1920));

let wider = fp.to_builder().screen(2560, 1440).build()?;
assert_eq!(wider.screen().value().map(|s| s.width), Some(2560));
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust A random identity
use seleniumbase_rs::{Fingerprint, OsType};

let fp = Fingerprint::randomized(OsType::Macos, 7);
assert_eq!(fp, Fingerprint::randomized(OsType::Macos, 7));
let report = fp.validate();
assert!(report.is_coherent() && report.warnings.is_empty());
```

```rust Check coherence
use seleniumbase_rs::{Fingerprint, OsType};

let fp = Fingerprint::builder()
    .os_type(OsType::Macos)
    .platform("Win32")
    .webgl((
        "Google Inc. (NVIDIA)",
        "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)",
    ))
    .build()?;
for error in &fp.validate().errors {
    println!("error: {error}");
}
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Save and load
use seleniumbase_rs::{Fingerprint, OsType};

let fp = Fingerprint::randomized(OsType::Linux, 3);
let json = serde_json::to_string(&fp)?;
let back: Fingerprint = serde_json::from_str(&json)?;
assert_eq!(fp, back);
# Ok::<(), serde_json::Error>(())
```

</RequestExample>

<ResponseExample>

```text Check coherence
error: navigator.platform 'Win32' does not match OS persona Macos
error: WebGL renderer 'ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)' is a Direct3D renderer, which Macos does not report
```

</ResponseExample>

## Fingerprint

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::fingerprint::Fingerprint`

```rust
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Fingerprint(/* private: one field per setting */);

impl Default for Fingerprint { /* Fingerprint::builder().build() */ }
impl<'de> Deserialize<'de> for Fingerprint { /* read the way the builder builds */ }
```

The fields are private: read a fingerprint through its accessors, and change one
by taking [`to_builder`](#fingerprint-to_builder) and building again. A built
fingerprint holds no `Random`: the builder drew those values.

- **Default.** `Fingerprint::default()` equals `Fingerprint::builder().build()`:
  every setting `Natural`, the [default policy](/stealth/api/core-policy#stealthpolicy-default),
  and the seed the builder gives it.
- **Serialize.** The JSON is one object with a key per setting, named as its
  accessor. A `Natural` setting is left out; `policy` and `humanize` are always
  written. A setting is the value itself or one of the words `"random"` and
  `"off"` (see [Spoof](/stealth/api/core-spoof#json)). The `seed` is written, so
  a loaded fingerprint gets the same noise.
- **Deserialize.** A fingerprint is read the way the builder builds it: a
  `"random"` setting is drawn, an `"off"` where it means nothing is an error,
  and a key this version does not know is an error (the flat shape of the old
  versions is one).

## Fingerprint::builder

<Badge color="green">fn</Badge>

```rust
pub fn builder() -> FingerprintBuilder
```

<ResponseField name="return" type="FingerprintBuilder">

A builder with nothing set and the default policy. See
[FingerprintBuilder](/stealth/api/core-fingerprint-builder).

</ResponseField>

## Fingerprint::to_builder

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn to_builder(&self) -> FingerprintBuilder
```

<ResponseField name="return" type="FingerprintBuilder">

A builder that starts from this fingerprint: its settings, its seed, its
browser persona and its policy as the base policy. Change a setting and call
`build` again.

</ResponseField>

## Persona and seed

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub const fn browser_type(&self) -> BrowserType
#[must_use]
pub const fn os_type(&self) -> OsType
#[must_use]
pub const fn core_version(&self) -> Option<u32>
#[must_use]
pub fn seed(&self) -> u64
```

<ResponseField name="browser_type" type="BrowserType">

The browser persona: [`BrowserType`](/stealth/api/core-settings#browsertype),
`Chromium` unless set or drawn with a random user agent.

</ResponseField>

<ResponseField name="os_type" type="OsType">

The operating-system persona: [`OsType`](/stealth/api/core-settings#ostype). The
one given, else the one a given user agent names, else `Windows`.

</ResponseField>

<ResponseField name="core_version" type="Option<u32>">

The Chromium major version the identity claims, if it says.

</ResponseField>

<ResponseField name="seed" type="u64">

The seed of the deterministic noise and of the random draws: the one given with
[`seed`](/stealth/api/core-fingerprint-builder#persona-and-seed). Without one it
is fresh entropy when a setting was drawn at random, and otherwise a value
derived from the user agent (a fixed constant without one), so the same persona
gets the same noise on every launch.

</ResponseField>

## Setting accessors

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn user_agent(&self) -> &Spoof<String>
// ... one accessor per setting, each with the same shape
```

One accessor per setting, each `#[must_use] pub const fn name(&self) -> &T`.
After a build, a setting is `Natural`, `Value` or `Off`.

| Accessor | Returns | What it is |
|---|---|---|
| `user_agent` | `&Spoof<String>` | `navigator.userAgent` and the `User-Agent` header |
| `platform` | `&Spoof<String>` | `navigator.platform` |
| `hardware_concurrency` | `&Spoof<u32>` | `navigator.hardwareConcurrency` |
| `device_memory` | `&Spoof<f64>` | `navigator.deviceMemory` |
| `max_touch_points` | `&Spoof<u32>` | `navigator.maxTouchPoints` |
| `vendor` | `&Spoof<String>` | `navigator.vendor` |
| `oscpu` | `&Spoof<String>` | `navigator.oscpu` (Firefox persona) |
| `product_sub` | `&Spoof<String>` | `navigator.productSub` |
| `build_id` | `&Spoof<String>` | `navigator.buildID` (Firefox persona) |
| `app_version` | `&Spoof<String>` | `navigator.appVersion` |
| `locale` | `&Spoof<String>` | `navigator.language` |
| `languages` | `&Spoof<String>` | `navigator.languages`, comma-separated |
| `accept_languages` | `&Spoof<String>` | The `Accept-Language` header |
| `timezone` | `&Spoof<String>` | The IANA time zone |
| `screen` | `&Spoof<ScreenSize>` | `screen.width` and `screen.height` |
| `avail_screen` | `&Spoof<ScreenSize>` | `screen.availWidth` and `screen.availHeight` |
| `pixel_ratio` | `&Spoof<f64>` | `window.devicePixelRatio` |
| `color_depth` | `&Spoof<u32>` | `screen.colorDepth` |
| `webgl` | `&Spoof<WebGl>` | The unmasked WebGL vendor and renderer |
| `media_devices` | `&Spoof<MediaDevices>` | What `enumerateDevices()` lists; `Off` lists none |
| `geolocation` | `&Spoof<Geolocation>` | The position `navigator.geolocation` reports; `Off` denies the request |
| `fonts` | `&Spoof<Vec<String>>` | The installed fonts |
| `proxy` | `&Spoof<ProxyConfig>` | The proxy the browser is launched with |
| `webrtc` | `&Spoof<WebRtcAddresses>` | The addresses WebRTC reports; `Off` makes WebRTC unavailable |
| `battery` | `&Spoof<BatteryProfile>` | The Battery Status API; `Off` makes it unavailable |
| `connection` | `&Spoof<ConnectionProfile>` | The Network Information API; `Off` makes it unavailable |
| `speech_voices` | `&Spoof<Vec<SpeechVoice>>` | `speechSynthesis.getVoices()`; `Off` lists none |
| `bluetooth` | `&Spoof<bool>` | Web Bluetooth: `Value(true)` has an adapter, `Value(false)` has none, `Off` makes the API unavailable |
| `client_hints` | `&Spoof<ClientHints>` | User-Agent Client Hints; `Off` makes `navigator.userAgentData` unavailable |
| `policy` | `&StealthPolicy` | How the claims are delivered and which protections run beside them |
| `humanize` | `&HumanizeConfig` | Humanized input timing |
| `custom_start_urls` | `&Vec<String>` | The URLs the profile opens at start |
| `cmd_params` | `&HashMap<String, String>` | Extra browser command-line parameters |

The value types are on [Settings](/stealth/api/core-settings), `HumanizeConfig`
on [Human input](/stealth/api/core-humanize#humanizeconfig).

## Profile switches

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub const fn local_storage(&self) -> bool
#[must_use]
pub const fn save_service_worker(&self) -> bool
```

<ResponseField name="local_storage" type="bool">

Whether `localStorage` is kept between sessions of the profile. `false` unless
set.

</ResponseField>

<ResponseField name="save_service_worker" type="bool">

Whether service workers are kept between sessions of the profile. `false` unless
set.

</ResponseField>

## Fingerprint::tracker_hosts

<Badge color="green">fn</Badge>

```rust
pub fn tracker_hosts(&self) -> Vec<String>
```

<ResponseField name="return" type="Vec<String>">

The tracker host block list: the policy's
[`blocked_trackers`](/stealth/api/core-policy#fields), or
[`default_tracker_hosts`](#default_tracker_hosts) when that list is empty. It is
the list whether or not blocking is on; [`is_masked`](#fingerprint-is_masked)
with `Dimension::Trackers` says whether it is.

</ResponseField>

## Fingerprint::is_masked

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn is_masked(&self, dimension: Dimension) -> bool
```

<ParamField body="dimension" type="Dimension" required>

A part of the browser's identity. See [Dimension](#dimension).

</ParamField>

<ResponseField name="return" type="bool">

Whether this fingerprint spoofs `dimension`.

</ResponseField>

This is the one predicate the page-script providers, the launch arguments, the
DevTools overrides and the proxy all ask. A dimension is on when a setting that
belongs to it is not `Natural`; the dimensions without a setting of their own
follow the policy.

| `Dimension` | On when |
|---|---|
| `Navigator` | Any of `user_agent`, `platform`, `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `oscpu`, `product_sub`, `build_id`, `app_version` is set |
| `Localization` | `locale`, `languages` or `accept_languages` is set |
| `Timezone` | `timezone` is set |
| `Screen` | `screen`, `avail_screen`, `pixel_ratio` or `color_depth` is set |
| `Graphics` | `webgl` is set |
| `Fonts` | `fonts` is set |
| `MediaDevices` | `media_devices` is set |
| `Geolocation` | `geolocation` is set |
| `WebRtc` | `webrtc` is set, or the policy's `webrtc_policy` is `Some` |
| `Canvas` | The policy's `canvas_noise` adds noise |
| `Audio` | The policy's `audio_noise` adds noise |
| `Battery` | `battery` is set |
| `Connection` | `connection` is set |
| `Speech` | `speech_voices` is set |
| `ClientHints` | `client_hints` is set |
| `Proxy` | `proxy` is a `Value` |
| `Ports` | The policy's `blocked_ports` is not empty |
| `Trackers` | The policy's `block_trackers` is on, or its `blocked_trackers` is not empty |
| `Bluetooth` | `bluetooth` is set |
| `ChromeRuntime` | The policy's `chrome_runtime` is on |
| `Headless` | The policy's `headless_fixes` is on |
| `NativeToString` | The policy's `native_tostring` is on (it is by default) |

"Set" is [`Spoof::is_set`](/stealth/api/core-spoof#state-tests): `Value` or `Off`.

## Fingerprint::validate

<Badge color="green">fn</Badge>

```rust
pub fn validate(&self) -> CoherenceReport
```

<ResponseField name="return" type="CoherenceReport">

Cross-field mismatches that make an otherwise convincing profile easy to
detect. See [CoherenceReport](#coherencereport).

</ResponseField>

Only values that were given are checked; a `Natural` setting is the browser's
own. `navigator.platform` is the one given, else the OS persona's.

| Kind | Reported when |
|---|---|
| error | `navigator.platform` does not fit the OS persona (`Win…` for Windows, `Mac` for macOS, `Linux` without `arm` for Linux, `arm` or `Linux` for Android, `iPhone` or `iPad` for iOS) |
| error | A given user agent has no token for the OS persona (`windows`, `mac os`, `linux` or `x11`, `android`, `iphone` or `ipad`, any case) |
| error | A given screen has a zero width or height |
| error | The WebGL strings name a graphics API another system reports: `Direct3D` or `D3D11` (Windows only), `Metal` or `Apple` (macOS and iOS only), `Mesa` (Linux only), `Adreno`, `Mali` or `PowerVR` (Android only) |
| error | The browser persona is `MobileSafari` and the user agent has a `Chrome/` token |
| warning | A mobile user agent (`mobile`, `android`, `iphone` or `ipad`) with no touch points, or a user agent that is not mobile (or none) with `max_touch_points` above 0 |
| warning | The locale differs from the first entry of `languages` |
| warning | A time zone is given, the position is `Natural`, and the policy grants permissions: a page is then handed the real position next to a made-up time zone |
| warning | The locale's region is not the country the [table of places](/stealth/api/core-randomize#locales) puts the time zone in |

Every preset and every [`randomized`](#fingerprint-randomized) identity passes
without an error or a warning.

## Fingerprint::randomized

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn randomized(os: OsType, seed: u64) -> Self
```

<ParamField body="os" type="OsType" required>

The operating-system persona to draw a machine for.

</ParamField>

<ParamField body="seed" type="u64" required>

Decides every draw. It is also the noise [`seed`](#persona-and-seed) of the result.

</ParamField>

<ResponseField name="return" type="Fingerprint">

A random but coherent identity for `os`, fully determined by `seed`.

</ResponseField>

It draws a whole machine and a whole place and derives every field from them,
because randomising fields one at a time gives the mismatches detectors look
for. It is the builder with these settings `Random`: user agent, platform,
cores, memory, touch points, vendor, locale, languages, `Accept-Language`, time
zone, position, screen, pixel ratio, colour depth, WebGL, Client Hints, battery,
network information, speech voices and Bluetooth, with
[`StealthPolicy::balanced`](/stealth/api/core-policy#stealthpolicy-balanced).

- **Graphics.** WebGL strings the system really reports: ANGLE over Direct3D 11
  on Windows, ANGLE over Metal with an Apple chip on macOS, ANGLE over Mesa or
  the NVIDIA driver on Linux, a Mali or Adreno chip on Android, `Apple GPU` on
  iOS.
- **Hardware.** The cores, memory, screen, colour depth and pixel ratio that
  machine ships with.
- **Place.** A locale, its language list, a time zone it is spoken in, and a
  position inside that time zone's main city.
- **Software.** A user agent, `navigator.platform`, `navigator.vendor` and
  Client Hints that agree with the system and the claimed Chromium major.

Fonts, media devices and WebRTC addresses are left alone and the proxy is left
off; ask for them with `Spoof::Random` on a builder. The claimed Chromium major
comes from a short window of recent releases
([`PRESET_CHROMIUM_MAJOR`](/stealth/api/core-randomize#preset_chromium_major)
is the newest). The same seed gives the same identity within one version of
the crate only: a changed table row changes what a seed maps to, so persist the
`Fingerprint` (it implements `Serialize`), not the seed.

<Note>

It describes an identity, not a browser. A fingerprint of another operating
system does not change the real browser binary's behaviour.

</Note>

## Fingerprint::randomized_with_major

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn randomized_with_major(os: OsType, seed: u64, major: u32) -> Self
```

<ParamField body="os" type="OsType" required>

As in `randomized`.

</ParamField>

<ParamField body="seed" type="u64" required>

As in `randomized`. The major is drawn anyway, so every other field is the one
this seed gives `randomized`.

</ParamField>

<ParamField body="major" type="u32" required>

The Chromium major the user agent and Client Hints claim, such as the release
a measured TLS and HTTP/2 fingerprint comes from (see the
[mimic crate](/stealth/api/mimic)). `OsType::Ios` (Safari) ignores it.

</ParamField>

<ResponseField name="return" type="Fingerprint">

`randomized(os, seed)` with the Chromium major fixed to `major`.

</ResponseField>

## Presets

<Badge color="green">fn</Badge>

```rust
pub fn windows_desktop() -> Self
pub fn edge_desktop() -> Self
pub fn macos_desktop() -> Self
pub fn linux_desktop() -> Self
pub fn android_mobile() -> Self
pub fn ios_mobile_safari() -> Self
```

Fixed identities, each built with `StealthPolicy::balanced()`. The Chrome
presets claim [`PRESET_CHROMIUM_MAJOR`](/stealth/api/core-randomize#preset_chromium_major)
in Chrome's frozen user-agent format and set no Client Hints: the evasions
derive those from the user agent. Each also carries the machine extras of its
system: a default `BatteryProfile` and `ConnectionProfile`, the system's speech
voices, and Bluetooth (`true` on macOS and Android, `false` on Windows and
Linux); on iOS the battery, network information, Bluetooth and Client Hints are
`Off` and `device_memory` is left out.

| Preset | OS | `platform` | Screen | Other values | Time zone |
|---|---|---|---|---|---|
| `windows_desktop` | Windows | `Win32` | 1920 x 1080 | 8 cores, 8 GB, NVIDIA RTX 4070 Ti over Direct3D 11 | `America/New_York` |
| `edge_desktop` | Windows | `Win32` | 1920 x 1080 | As `windows_desktop`, with Edge's `Edg/` token in the user agent | `America/New_York` |
| `macos_desktop` | macOS | `MacIntel` | 1920 x 1080 | 8 cores, 8 GB, `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)` | `America/Los_Angeles` |
| `linux_desktop` | Linux | `Linux x86_64` | 1920 x 1080 | 8 cores, 8 GB, an NVIDIA RTX 4070 as ANGLE over desktop OpenGL | `America/Chicago` |
| `android_mobile` | Android | `Linux armv8l` | 412 x 915, ratio 3 | 8 cores, 5 touch points, `Qualcomm` / `Adreno (TM) 740` | `America/New_York` |
| `ios_mobile_safari` | iOS | `iPhone` | 390 x 844, ratio 3 | Safari 26.0, `MobileSafari` persona, 4 cores, 5 touch points, `Apple Inc.` / `Apple GPU`, vendor `Apple Computer, Inc.` | `America/New_York` |

All use the locale `en-US` and the languages `en-US,en;q=0.9`.

## fingerprint! macro

<Badge color="yellow">macro</Badge> `seleniumbase_rs::fingerprint!`

```rust
#[macro_export]
macro_rules! fingerprint {
    (windows) => { $crate::Fingerprint::windows_desktop() };
    (macos) => { $crate::Fingerprint::macos_desktop() };
    (linux) => { $crate::Fingerprint::linux_desktop() };
    (android) => { $crate::Fingerprint::android_mobile() };
    (ios) => { $crate::Fingerprint::ios_mobile_safari() };
    (safari) => { $crate::Fingerprint::ios_mobile_safari() };
    (edge) => { $crate::Fingerprint::edge_desktop() };
}
```

A preset by name: `fingerprint!(macos)` is `Fingerprint::macos_desktop()`.

## Dimension

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs::stealth::fingerprint::Dimension`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dimension {
    Navigator, Localization, Timezone, Screen, Graphics, Fonts, MediaDevices,
    Geolocation, WebRtc, Canvas, Audio, Battery, Connection, Speech, ClientHints,
    Proxy, Ports, Trackers, Bluetooth, ChromeRuntime, Headless, NativeToString,
}
```

A part of a browser's identity that a fingerprint can mask; the argument of
[`is_masked`](#fingerprint-is_masked). Also `seleniumbase_rs::Dimension`.

<ResponseField name="Dimension" type="enum">

<Expandable title="variants">

<ResponseField name="Navigator" type="unit variant">User agent, platform, cores, memory, touch points, vendor and the like.</ResponseField>

<ResponseField name="Localization" type="unit variant">`navigator.language`, `navigator.languages` and `Accept-Language`.</ResponseField>

<ResponseField name="Timezone" type="unit variant">The time zone.</ResponseField>

<ResponseField name="Screen" type="unit variant">`screen.*`, the window size and the device pixel ratio.</ResponseField>

<ResponseField name="Graphics" type="unit variant">The WebGL vendor and renderer.</ResponseField>

<ResponseField name="Fonts" type="unit variant">The installed fonts.</ResponseField>

<ResponseField name="MediaDevices" type="unit variant">Camera, microphone and speaker lists.</ResponseField>

<ResponseField name="Geolocation" type="unit variant">The position `navigator.geolocation` reports.</ResponseField>

<ResponseField name="WebRtc" type="unit variant">WebRTC address leaks.</ResponseField>

<ResponseField name="Canvas" type="unit variant">Canvas read-back noise.</ResponseField>

<ResponseField name="Audio" type="unit variant">Audio read-back noise.</ResponseField>

<ResponseField name="Battery" type="unit variant">The Battery Status API.</ResponseField>

<ResponseField name="Connection" type="unit variant">The Network Information API.</ResponseField>

<ResponseField name="Speech" type="unit variant">`speechSynthesis.getVoices()`.</ResponseField>

<ResponseField name="ClientHints" type="unit variant">User-Agent Client Hints.</ResponseField>

<ResponseField name="Proxy" type="unit variant">The proxy the browser is launched with.</ResponseField>

<ResponseField name="Ports" type="unit variant">Ports blocked on the loopback names.</ResponseField>

<ResponseField name="Trackers" type="unit variant">The tracker host block list.</ResponseField>

<ResponseField name="Bluetooth" type="unit variant">The Web Bluetooth API.</ResponseField>

<ResponseField name="ChromeRuntime" type="unit variant">The `window.chrome` stubs.</ResponseField>

<ResponseField name="Headless" type="unit variant">The headless fixes (permissions, `matchMedia`, hairline).</ResponseField>

<ResponseField name="NativeToString" type="unit variant">`Function.prototype.toString` masking.</ResponseField>

</Expandable>

</ResponseField>

## CoherenceReport

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::CoherenceReport`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoherenceReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl CoherenceReport {
    pub fn is_coherent(&self) -> bool
}
```

What [`validate`](#fingerprint-validate) found.

<ResponseField name="errors" type="Vec<String>">

Hard inconsistencies likely to trip detection, such as an OS persona and a user
agent that disagree.

</ResponseField>

<ResponseField name="warnings" type="Vec<String>">

Softer issues worth reviewing.

</ResponseField>

<ResponseField name="is_coherent()" type="bool">

`true` when there are no errors. Warnings do not count.

</ResponseField>

## default_tracker_hosts

<Badge color="green">fn</Badge> `seleniumbase_rs::stealth::fingerprint::default_tracker_hosts`

```rust
pub fn default_tracker_hosts() -> Vec<String>
```

<ResponseField name="return" type="Vec<String>">

A brand-neutral starter list of tracking and fingerprinting-script hosts:
`doubleclick.net`, `googlesyndication.com`, `google-analytics.com`,
`googletagmanager.com`, `adservice.google.com`, `facebook.net`,
`connect.facebook.net`, `fpjs.io`, `fingerprintjs.com`, `cdn.fingerprint.com`,
`scorecardresearch.com`, `hotjar.com`, `segment.io`, `amplitude.com` and
`mixpanel.com`.

</ResponseField>

A policy that blocks trackers and names no hosts blocks these. They go to
`Network.setBlockedURLs` as `*host*` patterns and to the page-side
`tracker_block` provider; see [Evasions and launch](/stealth/api/core-evasions#safe_cdp_overrides).
