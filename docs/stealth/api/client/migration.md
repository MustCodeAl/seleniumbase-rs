---
title: "Identity & Client Migration (v0.9.4 to v0.10.0)"
sidebarTitle: "Migration & quick start"
description: "Quick-start guide and v0.9.4 to v0.10.0 import migration map for Layers 1 and 2 (identity and client) of seleniumbase-rs-stealth."
---

<Badge color="orange">Stealth module</Badge> <Badge color="blue">Crate: seleniumbase-rs-stealth</Badge> <Badge color="green">no unsafe</Badge>

Core `seleniumbase-rs` ships lightweight stealth primitives (`undetected::patcher`, `undetected::dprocess`, `undetected::reactor`, and `sb_cdp::human`). When a target needs full fingerprint synthesis, multi-surface JavaScript masking, or coherent protocol overrides beyond core UC mode, import [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).

`seleniumbase-rs-stealth` is a single layered library crate (plus the standalone `h3d/` binary for HTTP/3). It re-exports all of `seleniumbase-rs` alongside four layers:

1. `identity` (always enabled, `--no-default-features`, zero BoringSSL): pure persona and fingerprint data.
2. `client` (`features = ["client"]`, pure Rust, zero BoringSSL): browser-side JS/CDP masking and engine extension traits.
3. `network` (`features = ["network-spec"]` zero BoringSSL; `features = ["network"]` and `features = ["http3"]` opt into BoringSSL/wreq + `sbrs-h3d`): wire specs, proxy routing, capture tools, `WireFormat`, `MitmProxy`, `EphemeralCa`, and HTTP/3.
4. `session` (`default = ["session"]`, zero BoringSSL; `HttpSession` and `HybridSession` unlock with `network`): `CdpSession`, `HttpSession`, `HybridSession`, challenge detection/mitigation, cookie jar, ledger, and passport persistence.

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs-stealth` is a single layered companion crate (not a user-facing plugin) re-exporting all of `seleniumbase-rs` alongside `identity`, `client`, `network`, and `session`. Layer 1 (`identity`, always on with `default-features = false`) owns the unified `fingerprint` module (`Fingerprint`, `FingerprintBuilder`, `ProfileFingerprint`, `FingerprintGenerator`, `FingerprintOptions`, `GeneratedFingerprint`, `BayesianSampler`, `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `CoherenceReport`, `CoherenceAudit`, `FingerprintComparison`, `Contradiction`, `schemas/fingerprint.schema.json`), `spoof` (`Spoof`), `policy` (`StealthPolicy`), `mimic` (`Specimen`), `profile_payloads::ProfileParams`, `capture` (`CapturedFingerprint`), and the `fingerprint!` macro. Layer 2 (`client`, pure Rust, zero BoringSSL) owns 42-surface `script` (`StealthScriptBundle`, `StealthScriptOptions`), 32 `providers` (`EvasionRegistry`, `default_registry`, `session_registry`), `evasions` (`bootstrap_script`, `session_script`, `safe_cdp_overrides`, `launch_args`), `uc`, `apply` (`StealthPlan`, `StealthLayer` with `Default`/`balanced`/`strict`/`minimal`, `LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt`), and core Layer-0 re-exports (`patcher`, `dprocess`, `reactor`, `behavior`, `humanize`, `cdp`).

</Visibility>

## Install

Choose the feature tier that matches your build:

<CodeGroup>

```toml Default (Layers 1-4 up to CdpSession, zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

```toml Layers 1-2 only (identity + client masking, zero scraping or BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["client"] }
```

```toml Layer 1 only (pure identity and fingerprint data)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false }
```

</CodeGroup>

<Note>

If you also list `seleniumbase-rs` directly in your manifest (for example to enable `turso`), use the plain git URL `https://github.com/MustCodeAl/seleniumbase-rs` so Cargo resolves both dependencies to one copy of `seleniumbase-rs` and shared types (`sb_cdp::Page`, `LaunchOptions`, `BrowserConfig`) match.

</Note>

## Module layout

### Layer 1: `identity` (`src/identity/`)

| Module | Contents |
|---|---|
| `identity::fingerprint` | Unified fingerprint module: core v2 `Fingerprint`, `FingerprintBuilder`, `FingerprintError`, `CoherenceReport`, `OsType`, `BrowserType`, `SpeechVoice`, `ProxyConfig`, `ClientHints`, `BrandVersion`, JSON file I/O, comparison/consistency diagnostics, JSON Schema (`schemas/fingerprint.schema.json`), plus submodules `spoof` (`Spoof<T>`), `policy` (`StealthPolicy`, `CanvasNoise`, `AudioNoise`), `settings` (`ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices`, `MediaFeatures`, `ColorGamut`, `ColorScheme`, `DynamicRange`, `KeyboardLayout`, `PointerType`, `HoverType`), `mimic` (`Specimen`, `Build`, `Machine`, `Place`, `Rendering`), `randomize` (seeded persona synthesis tables), `profile` (`ProfileFingerprint`), `generator` (`FingerprintGenerator`, `FingerprintOptions`, `GeneratedFingerprint`), and `bayesian` (`BayesianSampler`, `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `CoherenceAudit`) |
| `identity::capture` | `CapturedFingerprint`, `ProbeInfo`, `capture_expression()`, `capture_async_script()` |
| `identity::profile_payloads` | `ProfileParams` external profile JSON parser (`to_fingerprint()`, `to_browser_config(url)`, `browser()`) |

The crate also exports the `fingerprint!` macro (`fingerprint!(windows)`, `macos`, `linux`, `android`, `ios`, `safari`, `edge`).

### Layer 2: `client` (`src/client/`)

| Module | Contents |
|---|---|
| `client::apply` | Module-native engine integration: `StealthPlan`, `StealthLayer` (`Default`, `balanced`, `strict`, `minimal`), `LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt`, and `PageScriptTarget` |
| `client::script` | 42-surface `StealthScriptBundle` and selective `StealthScriptOptions` (`all()`, `strict()`, `balanced()`, `minimal()`) |
| `client::providers` | 32 built-in JS `EvasionProvider` passes (`EvasionRegistry`, `default_registry()`, `session_registry(&fp)`, `SESSION_OMITTED_PROVIDERS`) |
| `client::evasions` | `bootstrap_script(&fp)`, `session_script(&fp)`, `safe_cdp_overrides(&fp)`, `launch_args(&fp)` |
| `client::uc` | Undetected-Chrome helpers layered with fingerprint support |
| Core re-exports | `patcher`, `dprocess`, `reactor` (from `seleniumbase_rs::undetected`), `behavior`, `humanize` (from `seleniumbase_rs::sb_cdp::human`), and `cdp` (from `seleniumbase_rs::browser::devtools`) |

## Applying stealth (`client::apply`)

You do not need to wire plugins or `PluginSet` manually. Import the module extension traits from `seleniumbase_rs_stealth::client` and layer a `Fingerprint` (from `seleniumbase_rs_stealth::identity`) or `StealthLayer` onto `LaunchOptions` (pure CDP), `BrowserConfig` (WebDriver / UC), or an active `Page` / `BaseCase`.

### Pure CDP (`LaunchOptionsFingerprintExt`)

`with_fingerprint(&fp)` and `with_stealth(&layer)` fill unset launch settings (user agent, language, timezone, geolocation, platform, WebRTC policy) without overwriting explicit caller overrides, and automatically apply launch flags, init scripts, and CDP protocol overrides on every new tab:

```rust
use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType};
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

async fn launch_masked() -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    // Option A: zero-config balanced preset for this host OS
    let layer = StealthLayer::default();
    let options = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&layer)?;

    // Option B: seeded preset (balanced / strict / minimal) or core Fingerprint
    let strict = StealthLayer::strict(OsType::Macos, 42);
    let _options_b = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&strict)?;

    let fp = Fingerprint::randomized(OsType::Macos, 42);
    let _options_c = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_fingerprint(&fp)?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    browser.close().await?;
    Ok(())
}
```

### WebDriver / UC (`BrowserConfigFingerprintExt`)

The same `with_fingerprint(&fp)` and `with_stealth(&layer)` methods work on `BrowserConfig`:

```rust
use seleniumbase_rs_stealth::client::{BrowserConfigFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::{BaseCase, BrowserConfig, DriverMode};

let layer = StealthLayer::balanced(OsType::Macos, 42);
let config = BrowserConfig::builder()
    .mode(DriverMode::Uc)
    .build()?
    .with_stealth(&layer)?;
```

### `StealthPlan`

`StealthPlan::new(&fp)` builds the ordered protocol plan for any CDP transport (WebSocket `sb_cdp::Page`, `BaseCase::execute_cdp_with_params`, or an embedded Tauri CEF/WebView2 channel):

| Method | Description |
|---|---|
| `StealthPlan::new(&fp)` | Builds the init script (`session_script`), `Network.enable`, safe CDP overrides, and context permission grants |
| `plan.init_script()` | Returns the JavaScript source installed via `Page.addScriptToEvaluateOnNewDocument` |
| `plan.commands()` | Iterates `(method, &Value)` pairs in execution order |
| `plan.overrides()` | Iterates CDP overrides after the init script |
| `plan.permission_grant(context)` | Returns `Option<Value>` parameters for `Browser.grantPermissions` in `context` (`None` for the default context) |

### Live-page traits (`PageFingerprintExt` and `BaseCaseFingerprintExt`)

One unified extension trait per engine covers direct injection, capture, and comparison:

| Trait | Methods |
|---|---|
| `PageFingerprintExt` (on `sb_cdp::Page`) | `apply_fingerprint(&fp)`, `apply_profile(&profile)`, `apply_generated(&generated)`, `apply_stealth(&bundle)`, `apply_stealth_layer(&layer)`, `capture_fingerprint()`, `compare_fingerprint(&expected)` |
| `BaseCaseFingerprintExt` (on `BaseCase`) | `apply_fingerprint(&fp)`, `apply_profile(&profile)`, `apply_generated(&generated)`, `apply_stealth(&bundle)`, `apply_stealth_layer(&layer)`, `capture_fingerprint()`, `compare_fingerprint(&expected)` |

## Migrating from v0.9.4 to v0.10.0

| v0.9.4 (`seleniumbase-rs`) | v0.10.0 |
|---|---|
| `seleniumbase_rs::evasion::patcher` / `dprocess` / `reactor` / `uc` | `seleniumbase_rs::undetected::{patcher, dprocess, reactor, uc}` |
| `seleniumbase_rs::evasion::{behavior, humanize}` | `seleniumbase_rs::sb_cdp::human::{behavior, humanize}` |
| `seleniumbase_rs::evasion::options::StealthOptions` | `seleniumbase_rs::browser::options::StealthOptions` |
| `seleniumbase_rs::{Fingerprint, Spoof, StealthPolicy, OsType}` | `seleniumbase_rs_stealth::identity::{Fingerprint, Spoof, StealthPolicy, OsType}` |
| `seleniumbase_rs::profile_payloads::ProfileParams` | `seleniumbase_rs_stealth::identity::profile_payloads::ProfileParams` |
| `seleniumbase_rs::sb_cdp::StealthPlan` | `seleniumbase_rs_stealth::client::StealthPlan` |
| `LaunchOptions::builder().fingerprint(&fp).build()?` | `LaunchOptions::builder().build()?.with_fingerprint(&fp)?` (`LaunchOptionsFingerprintExt`) |
| `page.apply_fingerprint(&fp)` / `page.capture_fingerprint()` | Import `PageFingerprintExt` from `seleniumbase_rs_stealth::client` |
| `seleniumbase_rs::fingerprint!(windows)` | `seleniumbase_rs_stealth::fingerprint!(windows)` |
