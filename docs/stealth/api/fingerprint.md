---
title: "Fingerprint Generation and Injection"
sidebarTitle: "Fingerprint API"
description: "High-level stealth profile models, seeded generator, dynamic script bundling, plugin integration, and engine extension traits."
---

<Badge color="orange">Stealth layer</Badge> <Badge color="green">no unsafe</Badge>

The stealth facade (`seleniumbase-rs-stealth`) extends the core fingerprinting engine with an empirical profile model, a seeded generator for high-entropy dimensions, and dynamic script bundles that install evasions on both WebDriver and pure CDP sessions.

## Core Models

### `ProfileFingerprint`

`ProfileFingerprint` is a serializable representation of a complete browser identity with zero legacy aliases:

```rust
use seleniumbase_rs_stealth::ProfileFingerprint;
use seleniumbase_rs::Fingerprint;

// Convert from core Fingerprint
let fp = Fingerprint::windows_desktop();
let profile = ProfileFingerprint::from_fingerprint(&fp);

// Field accessors match Rust conventions
assert_eq!(profile.platform, "Win32");
assert!(profile.hardware_concurrency > 0);
```

### `FingerprintGenerator`

`FingerprintGenerator` produces correlated display geometry, HDR/dynamic range modes, GPU/WebGL parameter tables, WebGPU adapter limits, high-entropy Client Hints with GREASE brands, Web Audio characteristics, and battery/network profiles from a deterministic seed:

```rust
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::{FingerprintGenerator, GeneratedFingerprint};

let bundle: GeneratedFingerprint = FingerprintGenerator::new(42)
    .os(OsType::Macos)
    .major(157)
    .locale("en-US")
    .generate();

println!("User-Agent: {}", bundle.profile.user_agent);
println!("WebGL Renderer: {}", bundle.profile.webgl_renderer);
println!("Core Fingerprint: {:?}", bundle.fingerprint);
```

### `StealthScriptBundle`

`StealthScriptBundle` compiles self-contained IIFE evasion scripts designed for injection via `Page.addScriptToEvaluateOnNewDocument` (pure CDP) or script evaluation (WebDriver):

```rust
use seleniumbase_rs_stealth::StealthScriptBundle;

let script_bundle = StealthScriptBundle::from_generated(&bundle);
let js_source = script_bundle.script();
assert!(!js_source.is_empty());
```

## Plugin & Engine Extension Traits

### `StealthPlugin`

`StealthPlugin` conforms to `seleniumbase_rs::plugin_api::Plugin`. It hooks into browser launch settings and automatically injects stealth scripts on every newly created document:

```rust
use seleniumbase_rs_stealth::StealthPlugin;

let plugin = StealthPlugin::from_generated(bundle);
```

### `StealthPageExt` and `StealthBaseCaseExt`

Extension traits attach direct stealth helpers to pure CDP `Page` and WebDriver `BaseCase`:

```rust
use seleniumbase_rs_stealth::{StealthPageExt, StealthBaseCaseExt};
use seleniumbase_rs::sb_cdp::Page;

async fn apply_evasions(
    page: &Page,
    bundle: &StealthScriptBundle,
) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    page.apply_stealth(bundle).await?;
    Ok(())
}
```
