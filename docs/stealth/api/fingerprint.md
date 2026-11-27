---
title: "Fingerprint Generation and Injection"
sidebarTitle: "Fingerprint API"
description: "Unified fingerprint module in seleniumbase-rs-stealth::identity: ProfileFingerprint, seeded and constrained generators, Bayesian sampling, header/format converters, 42-surface script bundling, and StealthLayer."
---

<Badge color="orange">Stealth module</Badge> <Badge color="green">no unsafe</Badge>

The unified `fingerprint` module lives in `seleniumbase_rs_stealth::identity::fingerprint` (`src/identity/fingerprint/`), and the 42-surface `script` and `apply` modules live in `seleniumbase_rs_stealth::client` (`src/client/script.rs`, `src/client/apply.rs`). Import them when core's basic undetected mode is not enough and you need correlated multi-surface fingerprint synthesis, statistical sampling, or selective 42-surface script injection.

## Core Models (`identity::fingerprint::profile`)

### `ProfileFingerprint`

`ProfileFingerprint` is a flat, JSON-serializable representation of a complete browser identity with zero legacy aliases, converting cleanly to and from core `Fingerprint`:

```rust
use seleniumbase_rs_stealth::identity::{Fingerprint, ProfileFingerprint};

let fp = Fingerprint::windows_desktop();
let profile = ProfileFingerprint::from_fingerprint(&fp);

assert_eq!(profile.platform.as_deref(), Some("Win32"));
assert!(profile.hardware_concurrency.unwrap_or(0) > 0);

// Roundtrip back to a validated core Fingerprint
let rebuilt = profile.to_fingerprint()?;
assert!(rebuilt.validate().is_coherent());
```

## Seeded & Constrained Generation (`identity::fingerprint::generator`)

### `FingerprintGenerator`

`FingerprintGenerator` produces correlated display geometry, HDR (`DynamicRange` / `HdrMode`), GPU/WebGL parameter tables, WebGPU adapter limits, high-entropy Client Hints with GREASE brands, Web Audio characteristics, and battery/network profiles from a deterministic seed:

```rust
use seleniumbase_rs_stealth::identity::{FingerprintGenerator, GeneratedFingerprint, OsType};

let bundle: GeneratedFingerprint = FingerprintGenerator::new(42)
    .os(OsType::Macos)
    .major(157)
    .generate();

println!("User-Agent: {:?}", bundle.profile.user_agent);
println!("WebGL Renderer: {}", bundle.gpu.renderer);
println!("Core Fingerprint: {:?}", bundle.fingerprint);
```

### Constrained Generation (`FingerprintOptions`)

Use `FingerprintOptions::builder()` (`FingerprintOptionsBuilder`) with `FingerprintGenerator::generate_with(&options)` to constrain generated personas without manual post-editing:

- `ScreenConstraints` (min/max width and height, DPR, HDR mode, color depth)
- `ConnectionOverride` (effective type, RTT, downlink, save-data)
- `BatteryOverride`, `ColorScheme`, `ReducedMotion`, `SpeechVoice`, `AudioDynamicsCompressor`, locale, timezone, fonts, WebGL overrides, and OS/version bounds

```rust
use seleniumbase_rs_stealth::identity::{
    ColorScheme, FingerprintGenerator, FingerprintOptions, OsType, ScreenConstraints,
};

let opts = FingerprintOptions::builder()
    .os(OsType::Macos)
    .screen(
        ScreenConstraints::new()
            .min_width(1440)
            .max_width(2560),
    )
    .color_scheme(ColorScheme::Dark)
    .seed(42)
    .build()?;

let generated = FingerprintGenerator::generate_with(&opts);
assert!(generated.display.width >= 1440);
```

## Bayesian Sampling, Headers & Converters (`identity::fingerprint::bayesian`)

The `identity::fingerprint::bayesian` submodule draws hardware-correlated personas from realistic joint distributions and converts them into protocol payloads:

| Item | Description |
|---|---|
| `BayesianSampler` | `sample(&constraints)`, `sample_batch(&constraints, count)`, `sample_with_headers(&constraints, RequestContext)` using `SampleConstraints` and `DeviceCategory` |
| `HeaderGenerator` | Generates ordered HTTP headers aligned with a `GeneratedFingerprint` and `RequestContext` (`Navigation`, `Fetch`, `Subresource`, `CrossOriginApi`) |
| `FingerprintConverter` | Converts a `GeneratedFingerprint` via `to_cdp_commands`, `to_chromium_launch_args`, `to_playwright_context_options`, and `to_json_pretty` |
| `audit_coherence(&fp)` | Audits cross-surface consistency across UA, platform, GPU, screen, and Client Hints, returning a `CoherenceAudit` (`score` and `issues`) |

```rust
use seleniumbase_rs_stealth::identity::{
    audit_coherence, BayesianSampler, DeviceCategory, RequestContext, SampleConstraints,
};

let constraints = SampleConstraints::new()
    .device(DeviceCategory::Desktop)
    .seed(1234);

let (generated, headers) =
    BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)?;
assert!(audit_coherence(&generated).is_clean());
assert!(!headers.is_empty());
```

## 42-Surface Script Bundling (`client::script`)

`StealthScriptBundle` compiles self-contained IIFE evasion scripts designed for `Page.addScriptToEvaluateOnNewDocument` (pure CDP) or script evaluation (WebDriver). Use `StealthScriptOptions` (`all()`, `strict()`, `balanced()`, `minimal()`, or per-surface boolean toggles) with `new_with`, `from_profile_with`, or `from_generated_with`:

```rust
use seleniumbase_rs_stealth::client::{StealthScriptBundle, StealthScriptOptions};

let full = StealthScriptBundle::from_generated(&generated);
assert!(!full.script().is_empty());

let balanced =
    StealthScriptBundle::from_generated_with(&generated, &StealthScriptOptions::balanced());
assert!(!balanced.script().is_empty());
```

## Applying Stealth at Launch or Per Page (`client::apply`)

### `StealthLayer`

`StealthLayer` packages a fingerprint, profile, optional `StealthPlan`, and selective `StealthScriptBundle` as a single reusable value. Attach it to pure CDP `LaunchOptions` or WebDriver `BrowserConfig` with `.with_stealth(&layer)`:

```rust
use seleniumbase_rs_stealth::client::StealthScriptOptions;
use seleniumbase_rs_stealth::sb_cdp::LaunchOptions;
use seleniumbase_rs_stealth::{LaunchOptionsFingerprintExt, StealthLayer};

let layer = StealthLayer::from_generated(generated)
    .with_script_options(&StealthScriptOptions::balanced());

let options = LaunchOptions::builder()
    .headless(true)
    .build()?
    .with_stealth(&layer)?;
```

### `PageFingerprintExt` and `BaseCaseFingerprintExt`

Both pure CDP `Page` and WebDriver `BaseCase` expose direct methods for applying fingerprints, profiles, generated bundles, compiled scripts, and full `StealthLayer` instances, as well as capturing and comparing live fingerprints:

```rust
use seleniumbase_rs_stealth::client::StealthScriptBundle;
use seleniumbase_rs_stealth::sb_cdp::Page;
use seleniumbase_rs_stealth::{BaseCaseFingerprintExt, PageFingerprintExt, StealthLayer};

async fn apply_evasions(
    page: &Page,
    bundle: &StealthScriptBundle,
    layer: &StealthLayer,
) -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    page.apply_stealth(bundle).await?;
    page.apply_stealth_layer(layer).await?;
    Ok(())
}
```

For core `Fingerprint` v2 settings, `Spoof`, `StealthPolicy`, `mimic` specimens, and the full Layer 1 re-export map, see [Stealth API: identity](/stealth/api/identity). For `StealthLayer` levels, `StealthScriptBundle` / `StealthScriptOptions`, `StealthPlan`, and the 32 built-in `EvasionProvider` passes, see [Stealth API: client](/stealth/api/client).
