---
title: "StealthLayer, StealthPlan & Extension Traits"
sidebarTitle: "Apply & StealthLayer"
description: "API reference for seleniumbase_rs_stealth::client::apply: StealthLayer, StealthPlan, LaunchOptionsFingerprintExt, BrowserConfigFingerprintExt, PageFingerprintExt, BaseCaseFingerprintExt, and PageScriptTarget."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase_rs_stealth::client::apply` connects Layer 1 identities (`Fingerprint`, `ProfileFingerprint`, `GeneratedFingerprint`) to both browser engines (`sb_cdp` and WebDriver `BaseCase`). Import everything from `seleniumbase_rs_stealth::client` or `seleniumbase_rs_stealth::client::apply`.

## `StealthLayer`

`StealthLayer` bundles launch arguments, optional CDP protocol overrides (`StealthPlan`), and a selective 42-surface JS bundle (`StealthScriptBundle`):

```rust
use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer, StealthScriptOptions};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType};
use seleniumbase_rs_stealth::sb_cdp::LaunchOptions;

let balanced = StealthLayer::balanced(OsType::Macos, 42);
let strict = StealthLayer::strict(OsType::Windows, 7);
let minimal = StealthLayer::minimal(OsType::Linux, 9)
    .with_script_options(&StealthScriptOptions::balanced().canvas_noise(true));

let fp = Fingerprint::macos_desktop();
let explicit = StealthLayer::new(fp);

let options = LaunchOptions::builder()
    .headless(true)
    .build()?
    .with_stealth(&balanced)?;

assert!(!explicit.launch_args().is_empty() || explicit.plan().is_some());
# Ok::<(), seleniumbase_rs_stealth::SeleniumBaseError>(())
```

Inspectors on `StealthLayer`: `bundle()`, `plan()`, `profile()`, `fingerprint()`, `webrtc_policy()`, `launch_args()`.

## `StealthPlan`

`StealthPlan::new(&fp)` builds the exact ordered CDP payload for any Chromium CDP transport (`sb_cdp::Page`, `BaseCase`, or embedded Tauri WebView2/CEF):

| Method | Return | Purpose |
|---|---|---|
| `StealthPlan::new(&fp)` | `StealthPlan` | Builds `Page.addScriptToEvaluateOnNewDocument`, `Network.enable`, `safe_cdp_overrides`, and permissions |
| `plan.init_script()` | `&str` | Raw JavaScript bootstrap installed before page scripts |
| `plan.commands()` | `impl ExactSizeIterator<Item = (&str, &Value)>` | All CDP `(method, params)` pairs in order |
| `plan.overrides()` | `impl ExactSizeIterator<Item = (&str, &Value)>` | Protocol overrides after the init script |
| `plan.permission_grant(context)` | `Option<Value>` | Optional `Browser.grantPermissions` request |

## Launch & live-page extension traits

| Trait | Receiver | Key methods |
|---|---|---|
| `LaunchOptionsFingerprintExt` | `sb_cdp::LaunchOptions` | `with_fingerprint(&fp)`, `with_stealth(&layer)` |
| `BrowserConfigFingerprintExt` | `BrowserConfig` | `with_fingerprint(&fp)`, `with_stealth(&layer)` |
| `PageFingerprintExt` | `sb_cdp::Page` | `apply_fingerprint(&fp)`, `apply_profile(&p)`, `apply_generated(&g)`, `apply_stealth(&bundle)`, `apply_stealth_layer(&layer)`, `capture_fingerprint()`, `compare_fingerprint(&expected)` |
| `BaseCaseFingerprintExt` | `BaseCase` | Same 7 methods for WebDriver / UC sessions |
| `PageScriptTarget` | CDP / custom targets | Trait used internally by `Page::capture_fingerprint` and script evaluation |
