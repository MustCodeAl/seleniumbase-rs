---
title: "Settings"
sidebarTitle: "Settings"
description: "Reference for the value types a fingerprint setting holds: ScreenSize, Geolocation, WebGl, WebRtcAddresses, MediaDevices, MediaDeviceSpec, ProxyConfig, BatteryProfile, ConnectionProfile, SpeechVoice, BrandVersion, ClientHints, and the OsType and BrowserType personas."
---

Most [`Fingerprint`](/stealth/api/core-fingerprint) settings are a plain
`String`, `u32` or `f64`. The ones that belong together are small types of their
own, defined in `src/stealth/settings.rs` (`ScreenSize`, `Geolocation`, `WebGl`,
`WebRtcAddresses`, `MediaDevices`) and `src/stealth/fingerprint.rs` (the rest).
All of them are reachable from `seleniumbase_rs::stealth::fingerprint`; the
crate root re-exports every one but `MediaDeviceSpec`. Each converts from the
obvious tuple, so a builder call reads like the thing it sets (see
[Conversions](/stealth/api/core-spoof#conversions)).

<RequestExample>

```rust Values from tuples
use seleniumbase_rs::{Fingerprint, Spoof};

let fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation((52.52, 13.405))
    .webgl(("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, GeForce RTX 4070)"))
    .webrtc(("203.0.113.7", "192.168.1.20"))
    .media_devices((1, 1, 1))
    .build()?;
assert_eq!(fp.geolocation().value().map(|g| g.latitude), Some(52.52));
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Typed values
use seleniumbase_rs::stealth::fingerprint::MediaDeviceSpec;
use seleniumbase_rs::{Fingerprint, Geolocation, MediaDevices, WebGl};

let fp = Fingerprint::builder()
    .geolocation(Geolocation::new(48.8566, 2.3522).accuracy(35.0))
    .webgl(WebGl::renderer_only("ANGLE (Intel, Intel(R) UHD Graphics 630)"))
    .media_devices(MediaDevices::labelled(vec![MediaDeviceSpec {
        kind: "videoinput".into(),
        device_id: "cam-1".into(),
        group_id: "g-1".into(),
        label: "FaceTime HD Camera".into(),
    }]))
    .build()?;
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust A proxy
use seleniumbase_rs::{Fingerprint, ProxyConfig};

let proxy = ProxyConfig {
    r#type: "http".into(),
    host: "proxy.example.net".into(),
    port: 8080,
    username: Some("me".into()),
    password: Some("secret".into()),
    save_traffic: false,
};
assert_eq!(proxy.to_url(), "http://me:secret@proxy.example.net:8080");
let fp = Fingerprint::builder().proxy(proxy).build()?;
println!("{:?}", fp.proxy()); // the password prints as <redacted>
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

```rust Personas
use seleniumbase_rs::OsType;

assert_eq!(OsType::Macos.platform(), "MacIntel");
assert_eq!(OsType::Android.default_screen(), (412, 732));
assert!(OsType::Ios.is_mobile());
```

</RequestExample>

## ScreenSize

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::settings::ScreenSize`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

impl ScreenSize {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self
}
impl From<(u32, u32)> for ScreenSize
```

A screen size in CSS pixels; the type of the `screen` and `avail_screen`
settings.

<ResponseField name="width" type="u32">

The width, in CSS pixels.

</ResponseField>

<ResponseField name="height" type="u32">

The height, in CSS pixels.

</ResponseField>

## Geolocation

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::settings::Geolocation`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geolocation {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

impl Geolocation {
    #[must_use]
    pub const fn new(latitude: f64, longitude: f64) -> Self
    #[must_use]
    pub const fn altitude(mut self, metres: f64) -> Self
    #[must_use]
    pub const fn accuracy(mut self, metres: f64) -> Self
}
impl From<(f64, f64)> for Geolocation
```

The position `navigator.geolocation` reports; the type of the `geolocation`
setting. A position answers a request without a prompt: the DevTools override
grants the `geolocation` permission with it.

<ResponseField name="latitude" type="f64">

Degrees north.

</ResponseField>

<ResponseField name="longitude" type="f64">

Degrees east.

</ResponseField>

<ResponseField name="altitude" type="Option<f64>">

Metres above sea level; `0` when absent.

</ResponseField>

<ResponseField name="accuracy" type="Option<f64>">

How far off the position may be, in metres; `100` when absent. A random
position claims 20 to 150 metres.

</ResponseField>

## WebGl

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::settings::WebGl`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebGl {
    pub vendor: Option<String>,
    pub renderer: Option<String>,
    pub vendor_id: Option<String>,
    pub renderer_id: Option<String>,
}

impl WebGl {
    #[must_use]
    pub fn new(vendor: impl Into<String>, renderer: impl Into<String>) -> Self
    #[must_use]
    pub fn renderer_only(renderer: impl Into<String>) -> Self
    #[must_use]
    pub fn vendor_only(vendor: impl Into<String>) -> Self
    #[must_use]
    pub fn ids(mut self, vendor_id: impl Into<String>, renderer_id: impl Into<String>) -> Self
    #[must_use]
    pub const fn is_empty(&self) -> bool
}
impl<V: Into<String>, R: Into<String>> From<(V, R)> for WebGl
```

The strings WebGL's `getParameter` reports for the unmasked vendor and renderer;
the type of the `webgl` setting. A half that is `None` is the browser's own.
Every field is left out of JSON when `None`.

<ResponseField name="vendor" type="Option<String>">

`UNMASKED_VENDOR_WEBGL`, such as `Google Inc. (NVIDIA)`.

</ResponseField>

<ResponseField name="renderer" type="Option<String>">

`UNMASKED_RENDERER_WEBGL`, such as `ANGLE (NVIDIA, ...)`.

</ResponseField>

<ResponseField name="vendor_id" type="Option<String>">

The PCI vendor id of the GPU (`0x10de`). Kept for tools that read it; never shown
to a page.

</ResponseField>

<ResponseField name="renderer_id" type="Option<String>">

The PCI device id of the GPU (`0x1f91`). Kept, never shown to a page.

</ResponseField>

<ResponseField name="is_empty()" type="bool">

Whether neither string is given, so the setting would change nothing.

</ResponseField>

## WebRtcAddresses

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::settings::WebRtcAddresses`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebRtcAddresses {
    pub public_ip: Option<String>,
    pub local_ip: Option<String>,
}

impl WebRtcAddresses {
    pub fn new(public_ip: impl Into<String>, local_ip: impl Into<String>) -> Self
}
impl<P: Into<String>, L: Into<String>> From<(P, L)> for WebRtcAddresses
```

The addresses WebRTC reports; the type of the `webrtc` setting. A `Value`, even
`WebRtcAddresses::default()`, filters the STUN and TURN servers a page names, so
no peer connection learns the real address, and switches the WebRTC IP-handling
launch flag on. An address that is given replaces the one the browser would
write into the SDP.

<ResponseField name="public_ip" type="Option<String>">

The public address of the candidates.

</ResponseField>

<ResponseField name="local_ip" type="Option<String>">

The local address of the connection line.

</ResponseField>

## MediaDevices

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::settings::MediaDevices`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaDevices {
    #[serde(default)]
    pub audio_inputs: u32,
    #[serde(default)]
    pub audio_outputs: u32,
    #[serde(default)]
    pub video_inputs: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labelled: Vec<MediaDeviceSpec>,
}

impl MediaDevices {
    #[must_use]
    pub fn counts(audio_inputs: u32, audio_outputs: u32, video_inputs: u32) -> Self
    #[must_use]
    pub fn labelled(devices: Vec<MediaDeviceSpec>) -> Self
}
impl From<(u32, u32, u32)> for MediaDevices
```

What `navigator.mediaDevices.enumerateDevices()` lists; the type of the
`media_devices` setting.

<ResponseField name="audio_inputs" type="u32">

How many microphones are listed.

</ResponseField>

<ResponseField name="audio_outputs" type="u32">

How many speakers are listed.

</ResponseField>

<ResponseField name="video_inputs" type="u32">

How many cameras are listed.

</ResponseField>

<ResponseField name="labelled" type="Vec<MediaDeviceSpec>">

Devices with their own ids and labels. When this is not empty, it is the whole
list and the counts are not used.

</ResponseField>

## MediaDeviceSpec

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::fingerprint::MediaDeviceSpec`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaDeviceSpec {
    pub kind: String,
    pub device_id: String,
    pub group_id: String,
    pub label: String,
}
```

One labelled device of [`MediaDevices::labelled`](#mediadevices), as the
`media_devices` provider lists it: the `kind`, `deviceId`, `groupId` and
`label` of a `MediaDeviceInfo`, such as `kind: "videoinput"`. It is not
re-exported at the crate root.

## ProxyConfig

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::fingerprint::ProxyConfig`

```rust
#[derive(Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub r#type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_traffic: bool,
}

impl fmt::Debug for ProxyConfig { /* prints the password as <redacted> */ }

impl ProxyConfig {
    pub fn to_url(&self) -> String
}
```

The proxy a profile launches the browser with; the type of the `proxy` setting.
It has no random value, and a proxy with an empty `host` fails the build.

<ResponseField name="type" type="String">

The scheme, such as `http` or `socks5` (`r#type` in Rust, `type` in JSON).

</ResponseField>

<ResponseField name="host" type="String">

The proxy host. Must not be empty.

</ResponseField>

<ResponseField name="port" type="u16">

The proxy port.

</ResponseField>

<ResponseField name="username, password" type="Option<String>">

The login. `Debug` never prints the password.

</ResponseField>

<ResponseField name="save_traffic" type="bool">

Carried to and from profile payloads (`profile_payloads`); no launch argument
or override uses it.

</ResponseField>

<ResponseField name="to_url()" type="String">

`{type}://{host}:{port}`, with `{username}:{password}@` before the host when both
are given. It is the value of Chrome's `--proxy-server` switch.

</ResponseField>

<Warning>

`to_url` puts the password in the URL, and the deprecated
[`cdp_overrides`](/stealth/api/core-evasions#cdp_overrides) would send it as a
`Proxy-Authorization` header to every site. The browser sessions use
[`safe_cdp_overrides`](/stealth/api/core-evasions#safe_cdp_overrides), which
leaves that header out.

</Warning>

## BatteryProfile

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::BatteryProfile`

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryProfile {
    pub charging: bool,
    pub level: f64,
    #[serde(with = "infinite_as_null", default = "infinite_as_null::infinity")]
    pub charging_time: f64,
    #[serde(with = "infinite_as_null", default = "infinite_as_null::infinity")]
    pub discharging_time: f64,
}

impl Default for BatteryProfile { /* charging, level 0.94, charging_time 0, discharging_time infinity */ }
```

What `navigator.getBattery()` resolves to; the type of the `battery` setting.

<ResponseField name="charging" type="bool" default="true">

Whether the battery is charging.

</ResponseField>

<ResponseField name="level" type="f64" default="0.94">

The level, in `0.0..=1.0`.

</ResponseField>

<ResponseField name="charging_time" type="f64" default="0.0">

Seconds until fully charged; `f64::INFINITY` when unknown. JSON has no infinity,
so it is written as `null`, and `null` or a missing key reads as infinity.

</ResponseField>

<ResponseField name="discharging_time" type="f64" default="f64::INFINITY">

Seconds until empty; `f64::INFINITY` while charging. `null` in JSON, as above.

</ResponseField>

## ConnectionProfile

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::ConnectionProfile`

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub effective_type: String,
    pub rtt: u32,
    pub downlink: f64,
    pub save_data: bool,
}

impl Default for ConnectionProfile { /* "4g", 50, 10.0, false */ }
```

The Network Information API (`navigator.connection`); the type of the
`connection` setting. The session script leaves `navigator.connection` to
Chrome (see [SESSION_OMITTED_PROVIDERS](/stealth/api/core-providers#session_omitted_providers)),
so these values reach a page only through `default_registry()`.

<ResponseField name="effective_type" type="String" default="&quot;4g&quot;">

`effectiveType`.

</ResponseField>

<ResponseField name="rtt" type="u32" default="50">

`rtt`, in milliseconds.

</ResponseField>

<ResponseField name="downlink" type="f64" default="10.0">

`downlink`, in megabits per second.

</ResponseField>

<ResponseField name="save_data" type="bool" default="false">

`saveData`.

</ResponseField>

## SpeechVoice

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::SpeechVoice`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeechVoice {
    pub name: String,
    pub lang: String,
    pub default: bool,
    pub local_service: bool,
    pub voice_uri: String,
}

impl SpeechVoice {
    pub fn default_set() -> Vec<Self>
}
```

One voice of `speechSynthesis.getVoices()`; the `speech_voices` setting is a
`Vec` of them. The fields are the `SpeechSynthesisVoice` properties `name`,
`lang`, `default`, `localService` and `voiceURI`.

<ResponseField name="default_set()" type="Vec<SpeechVoice>">

A plausible English set, the one Windows has: `Microsoft David - English (United
States)` (the default), `Microsoft Zira - English (United States)` and `Google US
English` (not a local service), all `en-US`.

</ResponseField>

## BrandVersion

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::BrandVersion`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrandVersion {
    pub brand: String,
    pub version: String,
}

impl BrandVersion {
    pub fn new(brand: impl Into<String>, version: impl Into<String>) -> Self
}
```

One `(brand, version)` entry of the Client Hints `brands` and `fullVersionList`
arrays.

## ClientHints

<Badge color="blue">struct</Badge> `seleniumbase_rs::stealth::ClientHints`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientHints {
    pub brands: Vec<BrandVersion>,
    pub full_version_list: Vec<BrandVersion>,
    pub platform: String,
    pub platform_version: String,
    pub architecture: String,
    pub bitness: String,
    pub model: String,
    pub ua_full_version: String,
    pub mobile: bool,
    #[serde(default)]
    pub wow64: bool,
    #[serde(default)]
    pub form_factors: Vec<String>,
}
```

User-Agent Client Hints (`navigator.userAgentData` and the `Sec-CH-UA-*`
headers); the type of the `client_hints` setting. In JSON every field but
`wow64` and `form_factors` must be present.
[`chromium_hints`](/stealth/api/core-randomize#chromium_hints) and
[`edge_hints`](/stealth/api/core-randomize#edge_hints) build the brand lists of
a release; the caller fills in the platform fields.

<ResponseField name="brands" type="Vec<BrandVersion>">

The low-entropy brand list, majors only.

</ResponseField>

<ResponseField name="full_version_list" type="Vec<BrandVersion>">

The brand list with full versions.

</ResponseField>

<ResponseField name="platform, platform_version" type="String">

Such as `Windows` and its version.

</ResponseField>

<ResponseField name="architecture, bitness" type="String">

Such as `x86` and `64`; empty on Android, which reports neither.

</ResponseField>

<ResponseField name="model" type="String">

The device model; Android only.

</ResponseField>

<ResponseField name="ua_full_version" type="String">

The browser's full version.

</ResponseField>

<ResponseField name="mobile" type="bool">

Whether the device is mobile.

</ResponseField>

<ResponseField name="wow64" type="bool">

Whether the browser runs as a 32-bit process on a 64-bit system
(`Sec-CH-UA-WoW64`).

</ResponseField>

<ResponseField name="form_factors" type="Vec<String>">

Form factors such as `Desktop` or `Mobile` (`Sec-CH-UA-Form-Factors`).

</ResponseField>

## OsType

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs::stealth::fingerprint::OsType`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OsType {
    #[default]
    Windows,
    Macos,
    Linux,
    Android,
    #[serde(rename = "ios", alias = "ipados")]
    Ios,
}

impl OsType {
    pub fn platform(&self) -> &'static str
    pub fn default_screen(&self) -> (u32, u32)
    pub fn is_mobile(&self) -> bool
}
```

The operating-system persona. In JSON the variants are `"Windows"`, `"Macos"`,
`"Linux"`, `"Android"` and `"ios"` (`"ipados"` reads as `Ios`).

| Variant | `platform()` | `default_screen()` | `is_mobile()` |
|---|---|---|---|
| `Windows` (default) | `Win32` | `(1920, 1080)` | `false` |
| `Macos` | `MacIntel` | `(1920, 1080)` | `false` |
| `Linux` | `Linux x86_64` | `(1920, 1080)` | `false` |
| `Android` | `Linux armv8l` | `(412, 732)` | `true` |
| `Ios` (iOS and iPadOS) | `iPhone` | `(390, 844)` | `true` |

`platform()` is the `navigator.platform` the persona reports, and the one
`validate` and `build` use when none is given. `is_mobile()` says whether the
persona uses touch input; it also marks the screen override as mobile.

## BrowserType

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs::stealth::fingerprint::BrowserType`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum BrowserType {
    #[default]
    #[serde(rename = "chromium")]
    Chromium,
    #[serde(rename = "firefox")]
    Firefox,
    #[serde(rename = "mobile_safari", alias = "safari")]
    MobileSafari,
}
```

The browser persona a profile requests.

<ResponseField name="BrowserType" type="enum">

<Expandable title="variants">

<ResponseField name="Chromium" type="unit variant">A Chromium-based persona; the default. JSON `"chromium"`.</ResponseField>

<ResponseField name="Firefox" type="unit variant">A Firefox-oriented persona. It is conceptual: the WebDriver mode uses Chromium. Its own settings are `oscpu` and `build_id`. JSON `"firefox"`.</ResponseField>

<ResponseField name="MobileSafari" type="unit variant">A Mobile Safari (WebKit, iOS) persona. `validate` reports a user agent with a `Chrome/` token for it. JSON `"mobile_safari"`, and `"safari"` reads as it.</ResponseField>

</Expandable>

</ResponseField>
