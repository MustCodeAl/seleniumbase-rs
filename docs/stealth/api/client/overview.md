---
title: "client (StealthLayer, 42-Surface Script Bundle, 32 JS Providers, StealthPlan)"
sidebarTitle: "client"
description: "API reference for Layer 2 of seleniumbase-rs-stealth (seleniumbase_rs_stealth::client): StealthLayer presets (minimal, balanced, strict, default), StealthScriptBundle and StealthScriptOptions across 42 JS surfaces, 32 EvasionProviders, StealthPlan CDP overrides, and engine extension traits."
---

<Badge color="blue">Layer 2: client</Badge> <Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase_rs_stealth::client` (`src/client/` in [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth), feature `client`) is Layer 2 of the stealth stack. It turns a Layer 1 [`identity`](/stealth/api/identity/overview) into launch flags, CDP protocol overrides, and self-contained JavaScript init scripts, and attaches them directly to pure CDP (`LaunchOptions`, `sb_cdp::Page`) or WebDriver/UC (`BrowserConfig`, `BaseCase`).

It is pure Rust, compiles with zero BoringSSL or cmake, and keeps the core `Plugin` hook adapter `pub(crate)` internal so callers configure stealth with ergonomic methods rather than manually assembling `PluginSet`s.

## When to use it

Enable `default-features = false, features = ["client"]` when you drive Chrome directly with `sb_cdp::Browser` or `BaseCase` and want layered browser masking without pulling in scraping sessions (`session`) or BoringSSL (`network`).

Keep the default `session` feature when you also want `CdpSession`, challenge mitigation, `ProxyPool`, `CookieJar`, and `Ledger`.

<RequestExample>

```rust Apply StealthLayer levels to pure CDP or WebDriver/UC
use seleniumbase_rs_stealth::client::{
    BrowserConfigFingerprintExt, LaunchOptionsFingerprintExt, StealthLayer, StealthScriptOptions,
};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::{BrowserConfig, DriverMode};

async fn launch_with_levels() -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    // 1. Default balanced preset for the current host OS
    let default_layer = StealthLayer::default();
    let cdp_opts = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&default_layer)?;

    // 2. Strict level (all 42 JS surfaces) for a seeded macOS persona
    let strict_layer = StealthLayer::strict(OsType::Macos, 42);
    let _strict_opts = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&strict_layer)?;

    // 3. Minimal level with selective per-surface toggle on WebDriver/UC
    let custom_layer = StealthLayer::minimal(OsType::Windows, 7)
        .with_script_options(&StealthScriptOptions::balanced().canvas_noise(true));
    let _uc_config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .build()?
        .with_stealth(&custom_layer)?;

    let browser = Browser::launch(cdp_opts).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    browser.close().await?;
    Ok(())
}
```

</RequestExample>

## Depend on it

<CodeGroup>

```toml Layers 1 + 2 only (identity + client masking, zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["client"] }
```

```toml Default (Layers 1-4 up to CdpSession, zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

</CodeGroup>

## Features

`client` is pure Rust and has no extra external dependencies beyond `identity`. It is enabled automatically by the default `session` feature (as well as `network` and `http3`).

## Stealth levels (`StealthLayer` and `StealthScriptOptions`)

Layer 2 exposes named stealth levels at two granularities: `StealthLayer` (complete identity + launch args + `StealthPlan` + script bundle) and `StealthScriptOptions` (fine-grained boolean toggles over the 42 WebIDL/JS surfaces).

### `StealthLayer` constructors

| Constructor | Script level | What it packages |
|---|---|---|
| `StealthLayer::minimal(os, seed)` | `StealthScriptOptions::minimal()` (4 surfaces) | Seeded identity + core `webdriver`, `chrome_runtime`, `permissions`, and `error_stack_scrub` masking. Lowest page overhead. |
| `StealthLayer::balanced(os, seed)` | `StealthScriptOptions::balanced()` (35 surfaces) | All core, worker, console, mouse, WebRTC, Intl, codec, geometry, and media guards while keeping canvas, audio, DOMRect, SVG, and `performance.now()` noise off for visually lossless rendering. |
| `StealthLayer::default()` | `StealthScriptOptions::balanced()` (35 surfaces) | Zero-config `balanced` layer tuned to the current host OS. |
| `StealthLayer::strict(os, seed)` | `StealthScriptOptions::strict()` (42 surfaces) | All 42 evasion surfaces enabled, including seeded canvas, `OffscreenCanvas`, `toBlob`, Web Audio, `DOMRect`, SVG, and `performance.now()` perturbation (`beacon_guard` remains opt-in). |
| `StealthLayer::new(fp)` | Full bundle | Wraps an explicit Layer 1 `Fingerprint` and precomputes its `StealthPlan` and `ProfileFingerprint`. |
| `StealthLayer::from_profile(profile)` | Full bundle | Wraps a flat `ProfileFingerprint` (`plan` is `None`). |
| `StealthLayer::from_generated(generated)` | Full bundle | Wraps a `GeneratedFingerprint` from `FingerprintGenerator` or `BayesianSampler`. |
| `StealthLayer::randomized(seed)` / `randomized_for(os, seed)` | Full bundle | Generates a seeded Windows (or chosen `OsType`) persona with full bundle. |

Refine any `StealthLayer` with `.with_script_options(&options)`. Inspect it with `.bundle()`, `.plan()`, `.profile()`, `.fingerprint()`, `.webrtc_policy()`, and `.launch_args()`.

### `StealthScriptOptions` presets and surfaces

| Preset | Active surfaces | Disabled by default |
|---|---|---|
| `StealthScriptOptions::minimal()` | 4: `webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub` | All other 38 surfaces and `beacon_guard` |
| `StealthScriptOptions::balanced()` | 35: core, plugins/MIME, WebGL/WebGPU, screen/window geometry, Client Hints, media devices, iframes (`contentWindow` & `srcdoc`), battery, speech, navigator extras, CSS media queries, workers, fonts, Intl/timezone, WebRTC guard, storage quota, hardware APIs, visibility, console debug canary, mouse screen coords, codecs, history length, WebGL shader precision, media capabilities, keyboard layout, clipboard, fullscreen, pointer capabilities | `canvas_noise`, `offscreen_canvas`, `canvas_toblob_guard`, `audio_noise`, `dom_rect_noise`, `svg_noise`, `performance_jitter`, `beacon_guard` |
| `StealthScriptOptions::strict()` / `all()` / `new()` | 42: every WebIDL/JS surface enabled | `beacon_guard` (opt-in via `.beacon_guard(true)`) |

Every field on `StealthScriptOptions` has a matching `const fn` builder setter (for example `.canvas_noise(true)`, `.worker_spoofing(false)`, `.beacon_guard(true)`).

## Engine extension traits (`client::apply`)

Import these traits from `seleniumbase_rs_stealth` (re-exported at the crate root) or `seleniumbase_rs_stealth::client`:

| Trait | Implemented on | Methods |
|---|---|---|
| `LaunchOptionsFingerprintExt` | `sb_cdp::LaunchOptions` | `with_fingerprint(&fp)`, `with_stealth(&layer)` (fills unset launch settings without overwriting explicit caller overrides; filters disallowed switches automatically) |
| `BrowserConfigFingerprintExt` | `BrowserConfig` | `with_fingerprint(&fp)`, `with_stealth(&layer)` |
| `PageFingerprintExt` | `sb_cdp::Page` | `apply_fingerprint(&fp)`, `apply_profile(&profile)`, `apply_generated(&generated)`, `apply_stealth(&bundle)`, `apply_stealth_layer(&layer)`, `capture_fingerprint()`, `compare_fingerprint(&expected)` |
| `BaseCaseFingerprintExt` | `BaseCase` | `apply_fingerprint(&fp)`, `apply_profile(&profile)`, `apply_generated(&generated)`, `apply_stealth(&bundle)`, `apply_stealth_layer(&layer)`, `capture_fingerprint()`, `compare_fingerprint(&expected)` |

## `StealthPlan`

`StealthPlan::new(&fp)` computes the ordered CDP commands for any transport (`sb_cdp::Page`, `BaseCase::execute_cdp_with_params`, or embedded CEF/WebView2):

| Method | Description |
|---|---|
| `StealthPlan::new(&fp)` | Builds the `Page.addScriptToEvaluateOnNewDocument` init script (`session_script`), `Network.enable`, `safe_cdp_overrides`, and context permissions. |
| `plan.init_script()` | Returns the JavaScript bootstrap source string. |
| `plan.commands()` | Iterates `(method, &Value)` pairs in execution order. |
| `plan.overrides()` | Iterates protocol overrides after the init script. |
| `plan.permission_grant(context)` | Returns optional `Browser.grantPermissions` parameters for `context` (`None` for the default context). |

## Module map

| Module | What it holds |
|---|---|
| `client::apply` | `StealthLayer`, `StealthPlan`, `PageScriptTarget`, `LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt`. |
| `client::script` | 42-surface `StealthScriptBundle` (`new`, `new_with`, `from_profile`, `from_profile_with`, `from_generated`, `from_generated_with`, `script()`) and `StealthScriptOptions`. |
| `client::providers` | 32 built-in JS `EvasionProvider` passes (`EvasionRegistry`, `default_registry()`, `session_registry(&fp)`, `EvasionConfig`, `EvasionContext`, `SESSION_OMITTED_PROVIDERS`). |
| `client::evasions` | `bootstrap_script(&fp)`, `session_script(&fp)`, `safe_cdp_overrides(&fp)`, `launch_args(&fp)`, `cdc_scrub()`, `permitted_switch()`. |
| `client::uc` | Undetected-Chrome launch helpers paired with fingerprint settings. |
| Layer 0 core re-exports | `client::patcher` (`ChromeBinaryPatcher`, `ChromedriverPatcher`, `EnginePatch`, `BinaryPatchRule`, `PatchPreset`, `engine_spoofing_args`), `client::dprocess`, `client::reactor`, `client::behavior`, `client::humanize`, `client::cdp`. |

## Re-exports at `seleniumbase_rs_stealth::client`

| Re-export | Source module |
|---|---|
| `StealthLayer`, `StealthPlan`, `PageScriptTarget`, `LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt` | `client::apply` (also re-exported at crate root except `PageScriptTarget`) |
| `StealthScriptBundle`, `StealthScriptOptions` | `client::script` |
| `EvasionProvider`, `EvasionRegistry`, `EvasionConfig`, `EvasionContext`, `default_registry`, `session_registry`, `SESSION_OMITTED_PROVIDERS` | `client::providers` |
| `ChromeBinaryPatcher`, `ChromedriverPatcher`, `EnginePatch`, `BinaryPatchRule`, `BinaryMarkerReport`, `BinaryFormat`, `MarkerCounts`, `PatchPreset`, `PatchedBrowserBundle`, `engine_spoofing_args`, `find_system_chrome`, `find_system_chrome_with` | `seleniumbase_rs::undetected::patcher` |
| `dprocess`, `patcher`, `reactor`, `behavior`, `humanize`, `cdp` | `seleniumbase_rs` Layer 0 core |

## Guides

- [Stealth overview](/stealth/introduction): choosing the right layer and level.
- [Evasions and launch flags](/stealth/client/evasions): launch flags, init scripts, and native CDP overrides.
- [Policies & levels](/stealth/identity/policies): comparing `StealthLayer`, `StealthScriptOptions`, and `StealthPolicy`.
- [Evasion providers](/stealth/advanced/client/providers): execution priority order and custom `EvasionProvider` passes.
- [v0.9.4 to v0.10.0 migration](/stealth/api/client/migration): side-by-side import migration table.
