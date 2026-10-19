---
title: "FingerprintBuilder"
sidebarTitle: "FingerprintBuilder"
description: "Reference for FingerprintBuilder: the persona, seed and setting methods, the policy and its shortcuts, how build draws Random settings and derives the rest, and FingerprintError."
---

A `FingerprintBuilder` makes a [`Fingerprint`](/stealth/api/core-fingerprint).
It is `seleniumbase_rs::stealth::fingerprint::FingerprintBuilder` (also
`seleniumbase_rs::FingerprintBuilder`), defined in `src/stealth/fingerprint.rs`;
get one from `Fingerprint::builder()` or, to change a built fingerprint,
`Fingerprint::to_builder()`. Each setting takes anything that converts to its
[`Spoof`](/stealth/api/core-spoof): a plain value is `Value`, and `Random`,
`Natural` and `Off` are named. The calls can come in any order, and a later call
for the same setting replaces an earlier one. The
[guide](/tutorials/fingerprint_stealth) shows the common recipes.

<RequestExample>

```rust Settings and policy in any order
use seleniumbase_rs::{Dimension, Fingerprint, Spoof, StealthPolicy};

let fp = Fingerprint::builder()
    .timezone("Europe/Berlin")
    .hardware_concurrency(8)
    .webrtc(Spoof::Off)
    .policy(StealthPolicy::native_only())
    .grant_permissions(true)
    .build()?;
assert!(fp.is_masked(Dimension::Timezone) && fp.is_masked(Dimension::Navigator));
assert!(fp.policy().native_spoofing && fp.policy().grant_permissions);
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Draw what agrees with a given value
use seleniumbase_rs::{Fingerprint, OsType, Spoof};

let fp = Fingerprint::builder()
    .os_type(OsType::Linux)
    .locale("fr-FR")
    .timezone(Spoof::Random)
    .geolocation(Spoof::Random)
    .webgl(Spoof::Random)
    .seed(11)
    .build()?;
// A random time zone agrees with the given locale.
assert_eq!(fp.timezone().as_str(), Some("Europe/Paris"));
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Change the policy without a shortcut
use seleniumbase_rs::{Fingerprint, StealthPolicy};

let fp = Fingerprint::builder()
    .with_policy(|p| p.open_shadow_roots(false))
    .policy(StealthPolicy::balanced()) // a base, whatever the order
    .build()?;
assert!(!fp.policy().open_shadow_roots && fp.policy().chrome_runtime);
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust A refused setting
use seleniumbase_rs::{Fingerprint, FingerprintError, Spoof};

let refused = Fingerprint::builder().timezone(Spoof::Off).build();
assert_eq!(
    refused,
    Err(FingerprintError::OffNotAvailable { setting: "timezone" })
);
```

</RequestExample>

<ResponseExample>

```text OffNotAvailable
timezone cannot be Off: only geolocation, webrtc, media_devices, battery, bluetooth, speech_voices, connection and client_hints have an off state
```

```text NoRandomValue
proxy cannot be Random: there is no value to draw for it
```

```text Incoherent
no coherent random timezone: the locale 'xx-XX' is not in the table of places, so nothing can be drawn to agree with it
```

</ResponseExample>

## FingerprintBuilder

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::fingerprint::FingerprintBuilder`

```rust
#[derive(Clone, Debug, Default)]
pub struct FingerprintBuilder { /* private: the settings, the seed, the base policy and the policy changes */ }
```

`FingerprintBuilder::default()` is `Fingerprint::builder()`: every setting
`Natural`, no seed, the default policy.

| Group | Methods |
|---|---|
| Persona and seed | `browser_type`, `os_type`, `core_version`, `seed` |
| Settings | One method per setting, plus `screen`, `screen_spoof`, `media_device_counts` |
| Profile | `local_storage`, `save_service_worker`, `custom_start_urls`, `cmd_params`, `humanize` |
| Policy | `policy`, `with_policy`, and the shortcuts `webrtc_policy`, `canvas_noise`, `audio_noise`, `native_spoofing`, `grant_permissions`, `block_trackers`, `blocked_trackers`, `blocked_ports`, `disable_csp`, `quic` |
| Finish | `build` |

## Persona and seed

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn browser_type(mut self, v: BrowserType) -> Self
#[must_use]
pub fn os_type(mut self, v: OsType) -> Self
#[must_use]
pub fn core_version(mut self, v: u32) -> Self
#[must_use]
pub fn seed(mut self, v: u64) -> Self
```

<ParamField body="browser_type" type="BrowserType">

The browser persona. When it is not set, `build` takes it from a random user
agent, and it is `Chromium` otherwise. See [BrowserType](/stealth/api/core-settings#browsertype).

</ParamField>

<ParamField body="os_type" type="OsType">

The operating-system persona. When it is not set, `build` takes it from a
recognised user agent (`iphone` or `ipad`, `android`, `windows`, `mac os`,
`linux` or `x11`), and it is `Windows` otherwise. See [OsType](/stealth/api/core-settings#ostype).

</ParamField>

<ParamField body="core_version" type="u32">

The Chromium major the identity claims. A random user agent and random Client
Hints claim it too. Without it, a given user agent's `Chrome/` major decides
what the random values claim.

</ParamField>

<ParamField body="seed" type="u64">

The seed of the random draws and of the deterministic noise, so the same calls
give the same fingerprint. Without it, the draw uses fresh entropy (when a
setting is `Random`) and the seed it used is kept in the fingerprint.

</ParamField>

## Setting methods

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn user_agent(mut self, value: impl Into<Spoof<String>>) -> Self
// ... one method per setting, each with the same shape
```

Every setting has a method of this shape: `#[must_use] pub fn name(mut self,
value: impl Into<Spoof<T>>) -> Self`. A plain `T` is a `Value`; the
[conversions](/stealth/api/core-spoof#conversions) add `&str`, tuples and arrays
where they read naturally.

| Method | `T` | Also takes | `Off` | `Random` draws |
|---|---|---|---|---|
| `user_agent` | `String` | `&str` | no | The user agent of the drawn machine (and its browser persona and major, when those were not given) |
| `platform` | `String` | `&str` | no | The OS persona's `navigator.platform` |
| `hardware_concurrency` | `u32` | | no | The machine's core count |
| `device_memory` | `f64` | | no | The machine's memory; Safari has none, so it stays `Natural` |
| `max_touch_points` | `u32` | | no | The machine's touch points |
| `vendor` | `String` | `&str` | no | `Google Inc.` or Safari's `Apple Computer, Inc.` |
| `oscpu` | `String` | `&str` | no | Nothing: `NoRandomValue` (Firefox persona) |
| `product_sub` | `String` | `&str` | no | `20030107` |
| `build_id` | `String` | `&str` | no | Nothing: `NoRandomValue` (Firefox persona) |
| `app_version` | `String` | `&str` | no | The user agent without its `Mozilla/` prefix; `Incoherent` when the user agent is not set |
| `locale` | `String` | `&str` | no | The drawn place's tag. Without `languages`, a given locale also gives the language list |
| `languages` | `String` | `&str` | no | The language list of the locale, else of the drawn place |
| `accept_languages` | `String` | `&str` | no | As `languages` |
| `timezone` | `String` | `&str` | no | The time zone of the place |
| `avail_screen` | `ScreenSize` | `(u32, u32)` | no | The screen less 40 pixels of height on a desktop, the screen on a phone |
| `pixel_ratio` | `f64` | | no | The machine's pixel ratio |
| `color_depth` | `u32` | | no | The machine's colour depth |
| `webgl` | `WebGl` | `(vendor, renderer)` | no | A graphics adapter the OS reports |
| `media_devices` | `MediaDevices` | `(u32, u32, u32)`, `Vec<MediaDeviceSpec>` | lists none | A drawn device list |
| `geolocation` | `Geolocation` | `(f64, f64)` | denies the request | A spot in the city of the place |
| `fonts` | `Vec<String>` | `Vec<&str>`, `[&str; N]` | no | The system's fonts, some dropped |
| `proxy` | `ProxyConfig` | | no | Nothing: `NoRandomValue` |
| `webrtc` | `WebRtcAddresses` | `(public_ip, local_ip)` | makes WebRTC unavailable | Drawn addresses |
| `battery` | `BatteryProfile` | | makes it unavailable | A drawn battery; `Off` on Safari |
| `connection` | `ConnectionProfile` | | makes it unavailable | A drawn connection; `Off` on Safari |
| `speech_voices` | `Vec<SpeechVoice>` | | lists none | The system's voices |
| `bluetooth` | `bool` | | makes the API unavailable | An adapter or none; `Off` on Safari |
| `client_hints` | `ClientHints` | | makes `navigator.userAgentData` unavailable | The machine's hints; `Off` on Safari |

`screen` is the exception: it takes a width and a height (below). A `Random`
`geolocation` with neither a time zone nor a locale given is in a drawn city,
and the browser's own time zone does not follow it, so draw the `timezone` with
it. A setting given `Off` where the table says "no" fails the build with
`FingerprintError::OffNotAvailable`.

## FingerprintBuilder::screen

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn screen(mut self, width: u32, height: u32) -> Self
#[must_use]
pub fn screen_spoof(mut self, value: impl Into<Spoof<ScreenSize>>) -> Self
```

<ParamField body="width, height" type="u32" required>

`screen` sets the screen size in CSS pixels: the common case of `screen_spoof`.

</ParamField>

<ParamField body="value" type="impl Into<Spoof<ScreenSize>>" required>

`screen_spoof` takes any state. `Random` draws the screen of the drawn machine;
`Off` is refused.

</ParamField>

## FingerprintBuilder::media_device_counts

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn media_device_counts(self, audio_in: u32, audio_out: u32, video_in: u32) -> Self
```

<ParamField body="audio_in, audio_out, video_in" type="u32" required>

How many microphones, speakers and cameras `enumerateDevices()` lists, with
generated labels: `media_devices(MediaDevices::counts(audio_in, audio_out, video_in))`.

</ParamField>

## Profile methods

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn local_storage(mut self, v: bool) -> Self
#[must_use]
pub fn save_service_worker(mut self, v: bool) -> Self
#[must_use]
pub fn custom_start_urls(mut self, v: Vec<String>) -> Self
#[must_use]
pub fn cmd_params(mut self, v: HashMap<String, String>) -> Self
#[must_use]
pub fn humanize(mut self, v: HumanizeConfig) -> Self
```

<ParamField body="local_storage" type="bool" default="false">

Keep `localStorage` between sessions of the profile.

</ParamField>

<ParamField body="save_service_worker" type="bool" default="false">

Keep service workers between sessions of the profile.

</ParamField>

<ParamField body="custom_start_urls" type="Vec<String>">

The URLs the profile opens at start.

</ParamField>

<ParamField body="cmd_params" type="HashMap<String, String>">

Extra browser command-line parameters, switch name (no dashes) to value. The
WebDriver launch arguments pass the ones
[`permitted_switch`](/stealth/api/core-evasions#permitted_switch) allows;
`LaunchOptionsBuilder::fingerprint` passes none.

</ParamField>

<ParamField body="humanize" type="HumanizeConfig">

The humanized input timing. See [HumanizeConfig](/stealth/api/core-humanize#humanizeconfig).

</ParamField>

## FingerprintBuilder::policy

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn policy(mut self, base: StealthPolicy) -> Self
#[must_use]
pub fn with_policy(
    mut self,
    change: impl Fn(StealthPolicy) -> StealthPolicy + Send + Sync + 'static,
) -> Self
```

<ParamField body="base" type="StealthPolicy" required>

The base policy, such as `StealthPolicy::balanced()`. The order of this call
against the settings and the changes does not matter: a later `policy` replaces
the base, and the changes apply on top of it.

</ParamField>

<ParamField body="change" type="impl Fn(StealthPolicy) -> StealthPolicy + Send + Sync + 'static" required>

A change applied after the base. Several calls, and the shortcuts, run in the
order they were made.

</ParamField>

A policy never changes a setting. The fields are on
[StealthPolicy](/stealth/api/core-policy).

## Policy shortcuts

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn webrtc_policy(self, policy: impl Into<Option<WebRtcPolicy>>) -> Self
#[must_use]
pub fn canvas_noise(self, noise: CanvasNoise) -> Self
#[must_use]
pub fn audio_noise(self, noise: AudioNoise) -> Self
#[must_use]
pub fn native_spoofing(self, on: bool) -> Self
#[must_use]
pub fn grant_permissions(self, on: bool) -> Self
#[must_use]
pub fn block_trackers(self, on: bool) -> Self
#[must_use]
pub fn blocked_trackers(self, hosts: Vec<String>) -> Self
#[must_use]
pub fn blocked_ports(self, ports: Vec<u16>) -> Self
#[must_use]
pub fn disable_csp(self, on: bool) -> Self
#[must_use]
pub fn quic(self, mode: QuicMode) -> Self
```

Each is `with_policy` with the `StealthPolicy` method of the same name, so it
applies after the base whatever the order. `native_tostring`, `chrome_runtime`,
`headless_fixes` and `open_shadow_roots` have no shortcut: use
`with_policy(|p| p.chrome_runtime(true))` and the like.

## FingerprintBuilder::build

<Badge color="green">fn</Badge>

```rust
pub fn build(self) -> Result<Fingerprint, FingerprintError>
```

<ResponseField name="return" type="Result<Fingerprint, FingerprintError>">

The fingerprint, holding no `Random`, or the first setting that could not be
built.

</ResponseField>

The steps, in order:

<Steps>

<Step title="Refuse what means nothing">

An `Off` on a setting without an off state is `OffNotAvailable`, naming the
first one in declaration order. A proxy `Value` with an empty host is
`Incoherent` (`proxy`, "the proxy has no host").

</Step>

<Step title="Work out the persona">

The OS persona is the one given, else the one a recognised user agent names,
else `Windows`. The seed is the one given, else fresh entropy when a setting is
`Random`, else one derived from the user agent.

</Step>

<Step title="Draw the Random settings">

A `Random` proxy, `oscpu` or `build_id` is `NoRandomValue`. The rest come from
one seeded draw of a machine for the OS and a place: the place of a given time
zone, else of a given locale, else a drawn one. A random locale, language list,
time zone or position needs that place to be in the
[table of places](/stealth/api/core-randomize#locales); when a given time zone
or locale is not, the build is `Incoherent`.

</Step>

<Step title="Derive the rest">

With a locale and no language list, `languages` becomes the locale's list. With
a user agent and no platform, `platform` becomes the OS persona's
`navigator.platform`.

</Step>

<Step title="Apply the policy">

The base policy, then every change in call order.

</Step>

</Steps>

## FingerprintError

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs::stealth::FingerprintError`

```rust
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FingerprintError {
    #[error("{setting} cannot be Off: only geolocation, webrtc, media_devices, battery, bluetooth, speech_voices, connection and client_hints have an off state")]
    OffNotAvailable { setting: &'static str },
    #[error("{setting} cannot be Random: there is no value to draw for it")]
    NoRandomValue { setting: &'static str },
    #[error("no coherent random {setting}: {reason}")]
    Incoherent { setting: &'static str, reason: String },
}

impl From<FingerprintError> for SeleniumBaseError { /* SeleniumBaseError::Stealth { reason } */ }
```

Why a builder could not build a fingerprint. `setting` is the name of the
setting as its builder method is named. Converted with `?` into a
`SeleniumBaseError`, it becomes `SeleniumBaseError::Stealth` with the message as
its `reason`.

<ResponseField name="FingerprintError" type="enum">

<Expandable title="variants">

<ResponseField name="OffNotAvailable { setting }" type="struct variant">A setting was given `Off`, which means nothing for it. Only geolocation, WebRTC, media devices, battery, Bluetooth, speech voices, network information and Client Hints have an off state.</ResponseField>

<ResponseField name="NoRandomValue { setting }" type="struct variant">A setting was given `Random`, but there is no value to draw for it: `proxy`, `oscpu`, `build_id`.</ResponseField>

<ResponseField name="Incoherent { setting, reason }" type="struct variant">A setting was given `Random`, but nothing coherent with the given settings can be drawn; `reason` says what the draw could not agree with. Also returned for a proxy with no host.</ResponseField>

</Expandable>

</ResponseField>
