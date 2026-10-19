---
title: "Spoof"
sidebarTitle: "Spoof"
description: "Reference for Spoof, the four states of one fingerprint setting (Natural, Random, Value, Off): the state tests, value access, the conversions the builder accepts and the JSON form."
---

Every [`Fingerprint`](/stealth/api/core-fingerprint) setting is one `Spoof`: leave
the browser's own value, draw a random one, use this one, or turn the API off. It
is `seleniumbase_rs::stealth::spoof::Spoof` (also `seleniumbase_rs::Spoof`,
`seleniumbase_rs::stealth::Spoof` and `seleniumbase_rs::stealth::fingerprint::Spoof`),
defined in `src/stealth/spoof.rs`. Why a setting is one value and not a value
plus a flag is in [Fingerprint API v2](/design/fingerprint-api-v2).

<RequestExample>

```rust Four states
use seleniumbase_rs::Spoof;

// A plain value is `Value`, so passing one is all the intent there is.
let zone: Spoof<String> = "Europe/Berlin".into();
assert_eq!(zone.as_str(), Some("Europe/Berlin"));

// The other three states are named.
assert!(Spoof::<String>::default().is_natural());
assert!(Spoof::<String>::Random.is_random());
assert!(Spoof::<String>::Off.is_off());
```

```rust Read a setting
use seleniumbase_rs::{Fingerprint, Spoof};

let fp = Fingerprint::ios_mobile_safari();
match fp.client_hints() {
    Spoof::Value(hints) => println!("claims {}", hints.platform),
    Spoof::Off => println!("no navigator.userAgentData"),
    Spoof::Natural => println!("the browser's own"),
    Spoof::Random => unreachable!("a built fingerprint holds no Random"),
}
```

```rust JSON
use seleniumbase_rs::Spoof;

let cores: Spoof<u32> = serde_json::from_str("8")?;
assert_eq!(cores.value(), Some(&8));
let drawn: Spoof<u32> = serde_json::from_str("\"random\"")?;
assert!(drawn.is_random());
let absent: Spoof<u32> = serde_json::from_str("null")?;
assert!(absent.is_natural());
assert_eq!(serde_json::to_string(&Spoof::<u32>::Off)?, "\"off\"");
# Ok::<(), serde_json::Error>(())
```

</RequestExample>

<ResponseExample>

```text Read a setting
no navigator.userAgentData
```

</ResponseExample>

## Spoof

<Badge color="orange">enum</Badge> `seleniumbase_rs::stealth::spoof::Spoof`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Spoof<T> {
    Natural,
    Random,
    Value(T),
    Off,
}

impl<T> Default for Spoof<T> { /* Spoof::Natural */ }
```

What the browser should report for one setting. `Random` is resolved when a
fingerprint is built, so a built fingerprint holds `Natural`, `Value` or `Off`.

<ResponseField name="Spoof" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Natural" type="unit variant">The browser's own value, left alone. This is the default, whatever `T` is.</ResponseField>

<ResponseField name="Random" type="unit variant">A random valid value, coherent with the rest of the fingerprint (same operating system, region and locale), drawn from the builder's seed. See [build](/stealth/api/core-fingerprint-builder#setting-methods) for what each setting draws.</ResponseField>

<ResponseField name="Value(T)" type="tuple variant">Exactly this value.</ResponseField>

<ResponseField name="Off" type="unit variant">The API is turned off: unavailable, or denied. Only geolocation, WebRTC, media devices, battery, Bluetooth, speech voices, network information and Client Hints have an off state; on any other setting it fails the build with `FingerprintError::OffNotAvailable`, naming the setting.</ResponseField>

</Expandable>

</ResponseField>

## State tests

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn is_natural(&self) -> bool
#[must_use]
pub const fn is_random(&self) -> bool
#[must_use]
pub const fn is_value(&self) -> bool
#[must_use]
pub const fn is_off(&self) -> bool
#[must_use]
pub const fn is_set(&self) -> bool
```

<ResponseField name="is_natural, is_random, is_value, is_off" type="bool">

Whether this is the state of that name.

</ResponseField>

<ResponseField name="is_set" type="bool">

Whether the setting changes anything: every state but `Natural`. This is the
test [`Fingerprint::is_masked`](/stealth/api/core-fingerprint#fingerprint) uses
for most dimensions.

</ResponseField>

## Values

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub const fn value(&self) -> Option<&T>
#[must_use]
pub fn into_value(self) -> Option<T>
#[must_use]
pub const fn as_ref(&self) -> Spoof<&T>
pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spoof<U>

impl Spoof<String> {
    #[must_use]
    pub fn as_str(&self) -> Option<&str>
}
```

<ResponseField name="value" type="Option<&T>">

The value, if this is a `Value`.

</ResponseField>

<ResponseField name="into_value" type="Option<T>">

The value by move, if this is a `Value`.

</ResponseField>

<ResponseField name="as_ref" type="Spoof<&T>">

The same state, borrowing the value.

</ResponseField>

<ResponseField name="map" type="Spoof<U>">

The same state with the value mapped through `f`; `f` runs only for a `Value`.

</ResponseField>

<ResponseField name="as_str" type="Option<&str>">

For a `Spoof<String>`: the value as text, if this is a `Value`.

</ResponseField>

## Conversions

<Badge color="blue">impl</Badge>

```rust
impl<T> From<T> for Spoof<T>
impl From<&str> for Spoof<String>
impl From<&String> for Spoof<String>
impl From<Cow<'_, str>> for Spoof<String>
impl From<Vec<&str>> for Spoof<Vec<String>>
impl<const N: usize> From<[&str; N]> for Spoof<Vec<String>>
```

A plain value is the intent to use it, so every builder setter, which takes
`impl Into<Spoof<T>>`, accepts the value itself. The
[settings module](/stealth/api/core-settings) adds the tuples that read like the
thing they set:

| Tuple or value | Becomes |
|---|---|
| `(u32, u32)` | `Spoof<ScreenSize>` (width, height) |
| `(f64, f64)` | `Spoof<Geolocation>` (latitude, longitude) |
| `(V, R)` with `V, R: Into<String>` | `Spoof<WebGl>` (vendor, renderer) |
| `(P, L)` with `P, L: Into<String>` | `Spoof<WebRtcAddresses>` (public, local address) |
| `(u32, u32, u32)` | `Spoof<MediaDevices>` (microphones, speakers, cameras) |
| `Vec<MediaDeviceSpec>` | `Spoof<MediaDevices>` (exactly these devices) |

## JSON

<Badge color="blue">impl</Badge>

```rust
impl<T: Serialize> Serialize for Spoof<T>
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Spoof<T>
```

In JSON a `Spoof` is the value itself, or one of the words `"natural"`,
`"random"` and `"off"`. `null` reads as `Natural`. The words are read in any
case (`"Random"` and `"OFF"` work) and written in lower case. For a string
setting the three words are reserved: a value spelled like one of them reads as
the keyword. A whole [`Fingerprint`](/stealth/api/core-fingerprint#fingerprint)
leaves its `Natural` settings out when it is written.
