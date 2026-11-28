---
title: "identity (Fingerprints, Spoof, StealthPolicy, Specimens, Bayesian Sampler)"
sidebarTitle: "identity"
description: "API reference for Layer 1 of seleniumbase-rs-stealth (seleniumbase_rs_stealth::identity): pure Rust persona modeling, Fingerprint v2, Spoof states, StealthPolicy presets, Mimic specimens, BayesianSampler, and probe capture."
---

<Badge color="blue">Layer 1: identity</Badge> <Badge color="green">Always on (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase_rs_stealth::identity` (`src/identity/` in [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)) is Layer 1 of the stealth stack. It is pure Rust, always enabled even with `default-features = false`, and compiles with zero BoringSSL, zero JS/CDP transport, and `#![forbid(unsafe_code)]`.

It models **what** a browser claims to be (`Fingerprint`, `ProfileFingerprint`, `Specimen`, `Spoof<T>`) and **how** claims should be delivered (`StealthPolicy`), plus seeded/constrained persona synthesis (`FingerprintGenerator`, `BayesianSampler`), coherence auditing (`validate()`, `audit_coherence()`), header/format conversion (`HeaderGenerator`, `FingerprintConverter`), external profile parsing (`ProfileParams`), and probe capture types (`CapturedFingerprint`).

## When to use it

Depend on `identity` alone (`default-features = false`) when you want deterministic fingerprint generation, Bayesian persona sampling, JSON schema validation, coherence checks, or profile conversion without pulling in browser drivers, scraping sessions, or BoringSSL.

Add `features = ["client"]` ([Stealth API: client](/stealth/api/client/overview)) when you want to apply that identity to a Chrome launch or live tab.

<RequestExample>

```rust Seeded identity with Bayesian sampling and coherence check
use seleniumbase_rs_stealth::identity::{
    audit_coherence, BayesianSampler, DeviceCategory, Fingerprint, OsType, RequestContext,
    SampleConstraints, StealthPolicy,
};

fn build_identities() -> Result<(), Box<dyn std::error::Error>> {
    // Seeded v2 Fingerprint with balanced policy
    let fp = Fingerprint::randomized(OsType::Macos, 42);
    assert!(fp.validate().is_coherent());

    // Custom FingerprintBuilder with explicit delivery policy
    let custom = Fingerprint::builder()
        .os_type(OsType::Windows)
        .timezone("America/New_York")
        .screen(1920, 1080)
        .policy(StealthPolicy::max())
        .build()?;
    assert!(custom.validate().is_coherent());

    // Joint-distribution Bayesian persona + aligned request headers
    let constraints = SampleConstraints::new()
        .device(DeviceCategory::Desktop)
        .seed(1234);
    let (generated, headers) =
        BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)?;
    assert!(audit_coherence(&generated).is_clean());
    assert!(!headers.is_empty());
    Ok(())
}
```

</RequestExample>

## Depend on it

<CodeGroup>

```toml Layer 1 only (zero BoringSSL, zero session runtime)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false }
```

```toml Default (Layers 1-4 up to CdpSession, zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

</CodeGroup>

## Features

`identity` requires no Cargo feature flag. Turning off default features (`default-features = false`) builds only Layer 1 (`identity`) plus the re-exported core `seleniumbase-rs` crate.

## Policy levels (`StealthPolicy`)

`StealthPolicy` controls delivery strategy and auxiliary protections without altering any `Spoof` setting:

| Preset | Delivery and protections |
|---|---|
| `StealthPolicy::default()` | `native_tostring` and `open_shadow_roots` only. No noise, no runtime stubs, no permission grants, no tracker blocking. |
| `StealthPolicy::native_only()` | Enables `native_spoofing` (CDP overrides and launch args) and `WebRtcPolicy::DisableNonProxiedUdp`. No JS-only stubs or canvas/audio noise. |
| `StealthPolicy::balanced()` | Default on every preset and `randomized` identity. Adds `WebRtcPolicy::DisableNonProxiedUdp`, seeded `CanvasNoise::Persistent` and `AudioNoise::Persistent`, `chrome_runtime`, and `headless_fixes`. |
| `StealthPolicy::max()` | Everything in `balanced()`, plus `block_trackers`, `grant_permissions`, and `QuicMode::Disabled`. |

## Module map

| Module | What it holds | Deep reference |
|---|---|---|
| `identity::fingerprint` | Core v2 `Fingerprint`, `FingerprintBuilder`, `FingerprintError`, `CoherenceReport`, `OsType`, `BrowserType`, `ClientHints`, `BrandVersion`, JSON file I/O, JSON Schema (`schemas/fingerprint.schema.json`). | [Fingerprint API](/stealth/api/identity/fingerprint) |
| `identity::fingerprint::{spoof, policy, settings}` | Five-state `Spoof<T>` enum, `StealthPolicy` (`default`, `native_only`, `balanced`, `max`), `CanvasNoise`, `AudioNoise`, and typed dimension settings (`ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices`, `MediaFeatures`, `ColorGamut`, `ColorScheme`, `DynamicRange`, `KeyboardLayout`, `QuicMode`, `WebRtcPolicy`). | [Spoof, policy & settings](/stealth/api/identity/spoof-policy) |
| `identity::fingerprint::{mimic, randomize}` | Recorded browser/hardware specimens (`Specimen`, `Build`, `Machine`, `Place`, `Rendering`, `Provenance`) and deterministic synthesis tables (`KNOWN_FULL_VERSIONS`, `edge_hints`, `chromium_hints`). | [Mimic & randomize](/stealth/api/identity/mimic-randomize) |
| `identity::fingerprint::{profile, generator, bayesian, compare, consistency}` | `ProfileFingerprint`, `FingerprintGenerator`, `FingerprintOptions`, `BayesianSampler`, `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `FingerprintComparison`, and `Contradiction`. | [Generators & Bayesian](/stealth/api/identity/generator-bayesian) |
| `identity::profile_payloads` & `identity::capture` | `ProfileParams` external JSON profile loader, `CapturedFingerprint`, `ProbeInfo`, `capture_expression()`, `capture_async_script()`, and `contradictions()`. | [Payloads & capture](/stealth/api/identity/profile-payloads-capture) |
| `identity::js` | Shared JavaScript helper builders (`CAPTURE_JS`) used by identity capture and client masking. | [Evasion providers](/stealth/advanced/client/providers) |

## Re-exports at `seleniumbase_rs_stealth::identity`

| Re-export | Source module |
|---|---|
| `Fingerprint`, `FingerprintBuilder`, `FingerprintError`, `CoherenceReport`, `Dimension`, `OsType`, `BrowserType`, `ClientHints`, `BrandVersion`, `ProxyConfig`, `QuicMode`, `WebRtcPolicy`, `HumanizeConfig` | `identity::fingerprint` |
| `ProfileFingerprint`, `AudioMetadata`, `BatteryMetadata`, `ClientHintsHighEntropy`, `ColorScheme`, `DisplayMetadata`, `GpuMetadata`, `MediaDeviceMetadata`, `NetworkMetadata`, `ReducedMotion`, `WebGlParameters`, `WebGpuLimits`, `WebGpuMetadata` | `identity::fingerprint::profile` |
| `FingerprintGenerator`, `FingerprintOptions`, `FingerprintOptionsBuilder`, `GeneratedFingerprint`, `ScreenConstraints`, `ConnectionOverride`, `ConnectionProfile`, `BatteryOverride`, `BatteryProfile`, `AudioDynamicsCompressor` | `identity::fingerprint::generator` |
| `BayesianSampler`, `SampleConstraints`, `DeviceCategory`, `HeaderGenerator`, `RequestContext`, `FingerprintConverter`, `audit_coherence`, `CoherenceAudit` | `identity::fingerprint::bayesian` |
| `FingerprintComparison`, `SettingDiff`, `Contradiction`, `Severity` | `identity::fingerprint::{compare, consistency}` |
| `ScreenSize`, `ScreenOrientation`, `ScreenOrientationType`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDeviceInfo`, `SpeechVoice`, `ColorGamut`, `HdrMode` | `identity::fingerprint::settings` |
| `StealthPolicy` | `identity::fingerprint::policy` |
| `Spoof` | `identity::fingerprint::spoof` |
| `Specimen` | `identity::fingerprint::mimic` |
| `ProfileParams` | `identity::profile_payloads` |
| `CapturedFingerprint`, `ProbeInfo` | `identity::capture` |

Import `Fingerprint` and `OsType` from `seleniumbase_rs_stealth::identity::{Fingerprint, OsType}` (or `seleniumbase_rs_stealth::identity::fingerprint::*`), and extension traits/`StealthLayer` from `seleniumbase_rs_stealth::client`.

## `fingerprint!` macro

Shorthand macro exported at the crate root for built-in presets:

```rust
use seleniumbase_rs_stealth::fingerprint;

let win = fingerprint!(windows);
let edge = fingerprint!(edge);
let mac = fingerprint!(macos);
let lin = fingerprint!(linux);
let android = fingerprint!(android);
let ios = fingerprint!(ios);
let safari = fingerprint!(safari); // alias for ios
```

## Guides

- [Stealth overview](/stealth/introduction): the four layers and stealth levels.
- [Fingerprints](/stealth/identity/fingerprints): `Spoof` states, presets, randomized identities, and consistency checks.
- [Policies & levels](/stealth/identity/policies): `StealthPolicy`, `StealthLayer`, and `StealthScriptOptions` levels.
- [Signal & hardware spoofing](/stealth/advanced/identity/signal-spoofing): WebGL/WebGPU, Canvas/OffscreenCanvas, DOMRect, Web Audio, fonts, and Client Hints.
- [Stealth API: client](/stealth/api/client/overview): applying an identity to Chrome over CDP or WebDriver.
