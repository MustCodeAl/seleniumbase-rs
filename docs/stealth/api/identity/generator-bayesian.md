---
title: "Generators, Bayesian Sampling & Audits"
sidebarTitle: "Generators & Bayesian"
description: "API reference for seleniumbase_rs_stealth::identity::fingerprint::{profile, generator, bayesian, compare, consistency}: ProfileFingerprint, FingerprintGenerator, BayesianSampler, HeaderGenerator, FingerprintConverter, and audit_coherence."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

These submodules under `seleniumbase_rs_stealth::identity::fingerprint` handle correlated persona generation, joint-distribution sampling, protocol conversions, and cross-surface consistency diagnostics.

## Flat profile model (`identity::fingerprint::profile`)

`ProfileFingerprint` provides a flat, JSON-serializable snapshot of a browser identity (`from_fingerprint(&fp)`, `to_fingerprint()`), alongside structured hardware and media tables:

- Display & GPU: `DisplayMetadata`, `GpuMetadata`, `WebGlParameters`, `WebGpuMetadata`, `WebGpuLimits`, `ColorScheme`, `ReducedMotion`, `ScreenOrientation`, `ScreenOrientationType`
- Audio & Media: `AudioMetadata`, `AudioDynamicsCompressor`, `MediaDeviceMetadata`, `MediaDeviceInfo`
- Environment & Hints: `BatteryMetadata`, `NetworkMetadata`, `ClientHintsHighEntropy`

## Seeded & constrained generator (`identity::fingerprint::generator`)

`FingerprintGenerator` builds a coherent `GeneratedFingerprint` from a deterministic seed and optional `FingerprintOptions`:

```rust
use seleniumbase_rs_stealth::identity::{
    ColorScheme, FingerprintGenerator, FingerprintOptions, OsType, ScreenConstraints,
};

let opts = FingerprintOptions::builder()
    .os(OsType::Windows)
    .screen(ScreenConstraints::new().min_width(1440).max_width(2560))
    .color_scheme(ColorScheme::Dark)
    .seed(42)
    .build()?;

let generated = FingerprintGenerator::generate_with(&opts);
assert!(generated.display.width >= 1440);
# Ok::<(), seleniumbase_rs_stealth::SeleniumBaseError>(())
```

Built-in `FingerprintOptions` constructors: `windows_chrome()`, `macos_chrome()`, `linux_chrome()`, `android_chrome()`, `ios_safari()`. Constraint helpers: `ScreenConstraints`, `ConnectionOverride` (`ConnectionProfile`), and `BatteryOverride` (`BatteryProfile`).

## Bayesian sampler, headers & converters (`identity::fingerprint::bayesian`)

| Type / function | Key methods |
|---|---|
| `SampleConstraints` | `new()`, `.os(...)`, `.browsers(...)`, `.device(DeviceCategory)`, `.version_range(min, max)`, `.locales(...)`, `.screen(...)`, `.seed(u64)`, `.strict(bool)` |
| `BayesianSampler` | `sample(&constraints)`, `sample_batch(&constraints, count)`, `sample_with_headers(&constraints, RequestContext)` |
| `HeaderGenerator` | `HeaderGenerator::generate(&generated, RequestContext)` (`Navigation`, `Fetch`, `Subresource`, `CrossOriginApi`) |
| `FingerprintConverter` | `to_cdp_commands(&generated)`, `to_chromium_launch_args(&generated)`, `to_playwright_context_options(&generated)`, `to_json_pretty(&generated)` |
| `audit_coherence` | `audit_coherence(&generated) -> CoherenceAudit` (`.is_clean()`) |

```rust
use seleniumbase_rs_stealth::identity::{
    audit_coherence, BayesianSampler, DeviceCategory, FingerprintConverter, RequestContext,
    SampleConstraints,
};

let constraints = SampleConstraints::new()
    .device(DeviceCategory::Desktop)
    .seed(1234);
let (generated, headers) =
    BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)?;

assert!(audit_coherence(&generated).is_clean());
assert!(!headers.is_empty());
let args = FingerprintConverter::to_chromium_launch_args(&generated);
assert!(!args.is_empty());
# Ok::<(), seleniumbase_rs_stealth::SeleniumBaseError>(())
```

## Comparison & contradiction rules (`compare` / `consistency`)

| Method / type | Output |
|---|---|
| `fp.validate()` | Returns `CoherenceReport` checking the 35 cross-surface `consistency::RULES` |
| `fp.contradictions()` | Returns `Vec<Contradiction>` with `Severity` |
| `fp.diff(&other)` | Returns `Vec<SettingDiff>` between two identities |
| `fp.compare_to(&observed)` | Returns `FingerprintComparison` (`presents()`, `is_verified()`, `contradictions()`) for live capture verification |
