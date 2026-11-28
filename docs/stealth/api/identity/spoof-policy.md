---
title: "Spoof States, Policy & Settings"
sidebarTitle: "Spoof, policy & settings"
description: "API reference for seleniumbase_rs_stealth::identity::fingerprint::{spoof, policy, settings}: Spoof<T>, StealthPolicy, CanvasNoise, AudioNoise, and typed dimension structs."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

The `spoof`, `policy`, and `settings` submodules live under `seleniumbase_rs_stealth::identity::fingerprint`. Their primary types (`Spoof`, `StealthPolicy`, `ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDeviceInfo`, `SpeechVoice`, `ColorGamut`, `HdrMode`) are also re-exported at `seleniumbase_rs_stealth::identity`.

## Five-state value wrapper (`identity::fingerprint::spoof`)

Every overridable field on `Fingerprint` uses `Spoof<T>` so an untouched default is never confused with a literal override or a disabled API:

```rust
use seleniumbase_rs_stealth::identity::fingerprint::spoof::Spoof;
use seleniumbase_rs_stealth::identity::fingerprint::mimic::{Machine, Specimen};

let natural: Spoof<String> = Spoof::default();
assert!(natural.is_natural());

let literal: Spoof<String> = "Europe/Berlin".into();
assert_eq!(literal.as_str(), Some("Europe/Berlin"));

let drawn: Spoof<String> = Spoof::Random;
assert!(drawn.is_random());

let off: Spoof<String> = Spoof::Off;
assert!(off.is_off());

let model = Specimen::Machine(Machine::named("macbook-13-m1")?);
let mimicked: Spoof<String> = Spoof::mimic(model);
assert_eq!(mimicked.specimen(), Some(model));
# Ok::<(), seleniumbase_rs_stealth::identity::fingerprint::mimic::UnknownSpecimen>(())
```

| Variant | Meaning | Resolved by `build()` |
|---|---|---|
| `Spoof::Natural` | Leave the browser's own value untouched (default). | Remains `Natural` |
| `Spoof::Random` | Draw a coherent value from the fingerprint seed and OS. | Resolves to `Value(T)` |
| `Spoof::Value(T)` | Claim this exact typed value. | Remains `Value(T)` |
| `Spoof::Mimic(Specimen)` | Copy values from a catalogued build, machine, place, or rendering. | Resolves to `Value(T)` |
| `Spoof::Off` | Disable or deny the API (permitted only on APIs with an off state). | Remains `Off` |

Helpers on `Spoof<T>`: `is_natural()`, `is_random()`, `is_value()`, `is_mimic()`, `is_off()`, `specimen()`, `Spoof::mimic(specimen)`, `value()` -> `Option<&T>`, and `as_str()` (for `Spoof<String>`).

## Delivery & protection policy (`identity::fingerprint::policy`)

`StealthPolicy` configures **how** claims are injected and which runtime defenses run alongside `Spoof` settings:

| Preset / enum | Values / defaults |
|---|---|
| `StealthPolicy::default()` | `native_tostring` and `open_shadow_roots` enabled; no noise, no stubs |
| `StealthPolicy::native_only()` | `native_spoofing` + `WebRtcPolicy::DisableNonProxiedUdp` |
| `StealthPolicy::balanced()` | Default on presets; adds `CanvasNoise::Persistent`, `AudioNoise::Persistent`, `chrome_runtime`, `headless_fixes`, `WebRtcPolicy::DisableNonProxiedUdp` |
| `StealthPolicy::max()` | `balanced()` plus `block_trackers`, `grant_permissions`, and `QuicMode::Disabled` |
| `CanvasNoise` | `Natural`, `Persistent`, `Random`, `Low` (`adds_noise()`, `density()`) |
| `AudioNoise` | `Natural`, `Persistent`, `Random` (`adds_noise()`) |

```rust
use seleniumbase_rs_stealth::identity::fingerprint::{
    AudioNoise, CanvasNoise, Fingerprint, StealthPolicy,
};

let policy = StealthPolicy::balanced()
    .canvas_noise(CanvasNoise::Random)
    .audio_noise(AudioNoise::Persistent)
    .native_spoofing(true);

let fp = Fingerprint::builder()
    .policy(policy)
    .build()?;
assert!(fp.policy().native_spoofing);
# Ok::<(), seleniumbase_rs_stealth::identity::fingerprint::FingerprintError>(())
```

## Typed dimension settings (`identity::fingerprint::settings`)

| Type | Key fields / variants |
|---|---|
| `ScreenSize` | Pixel width, height, and optional device pixel ratio |
| `Geolocation` | Latitude, longitude, and accuracy |
| `WebGl` | `vendor`, `renderer`, optional unmasked values, and `ShaderPrecision` |
| `WebRtcAddresses` | Public/local ICE address overrides paired with `WebRtcPolicy` |
| `MediaDevices` | Audio/video device counts and metadata (`MediaDeviceInfo`) |
| `MediaFeatures` | `ColorScheme`, `ColorGamut`, `DynamicRange` (`HdrMode`), `Contrast` |
| `KeyboardLayout` | Keyboard layout map entries |
| `PointerType` / `HoverType` | Fine vs coarse pointer and hover media-query capabilities |
| `QuicMode` | `Natural` or `Disabled` (disables UDP QUIC via launch flag) |
| `WebRtcPolicy` | Default, `DisableNonProxiedUdp`, `DefaultPublicAndPrivateInterfaces`, `Disable` |
