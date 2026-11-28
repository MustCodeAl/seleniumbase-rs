---
title: "42-Surface Script Bundle & 32 Evasion Providers"
sidebarTitle: "Scripts & providers"
description: "API reference for seleniumbase_rs_stealth::client::{script, providers}: StealthScriptBundle, StealthScriptOptions, EvasionProvider, EvasionRegistry, EvasionContext, default_registry, and session_registry."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`client::script` and `client::providers` give fine-grained control over every JavaScript surface injected into pages and Web Workers.

## 42-surface bundle (`client::script`)

`StealthScriptBundle` compiles one self-contained IIFE script with shared native `Function.prototype.toString` helpers and deterministic seeds derived from the identity:

| Constructor | Source type | Selective filter |
|---|---|---|
| `StealthScriptBundle::new(&fp)` / `new_with(&fp, &opts)` | `identity::Fingerprint` | Defaults to `all()` unless `opts` is passed |
| `from_profile(&profile)` / `from_profile_with(&profile, &opts)` | `identity::ProfileFingerprint` | Toggles any of the 42 surfaces |
| `from_generated(&gen)` / `from_generated_with(&gen, &opts)` | `identity::GeneratedFingerprint` | Pairs full GPU/WebGPU/Audio tables with `opts` |

`StealthScriptOptions` presets:

- `StealthScriptOptions::minimal()` — 4 core surfaces (`webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub`)
- `StealthScriptOptions::balanced()` — 35 surfaces (keeps canvas, `OffscreenCanvas`, `toBlob`, audio, `DOMRect`, SVG, and `performance.now()` noise off for lossless rendering)
- `StealthScriptOptions::strict()` / `all()` / `new()` — all 42 surfaces enabled (`beacon_guard` remains opt-in via `.beacon_guard(true)`)

```rust
use seleniumbase_rs_stealth::client::{StealthScriptBundle, StealthScriptOptions};
use seleniumbase_rs_stealth::identity::{FingerprintGenerator, OsType};

let generated = FingerprintGenerator::new(42).os(OsType::Macos).generate();
let opts = StealthScriptOptions::balanced().canvas_noise(true);
let bundle = StealthScriptBundle::from_generated_with(&generated, &opts);

assert!(!bundle.script().is_empty());
```

## Pluggable passes (`client::providers`)

| Symbol | Purpose |
|---|---|
| `EvasionProvider` trait | Custom evasion unit implementing `name()`, optional `priority()` (default `100`), `applies(&fp)`, and `script(&EvasionContext)` |
| `EvasionRegistry` | Ordered registry supporting `register_provider(provider)` and `bootstrap(&ctx)` |
| `default_registry()` | Registry holding all 32 built-in providers (`builtin::all()`) |
| `session_registry(&fp)` | Session-focused registry omitting `SESSION_OMITTED_PROVIDERS` |
| `EvasionContext` & `EvasionConfig` | Escape/numeric helpers (`escape`, `number`) plus fingerprint and seed context |
| `SB_PREAMBLE` | Prelude constant placing `sb.native`, `sb.ctor`, and `sb.getter` in provider scope |

```rust
use seleniumbase_rs_stealth::client::providers::{default_registry, EvasionContext};
use seleniumbase_rs_stealth::identity::Fingerprint;

let fp = Fingerprint::windows_desktop();
let ctx = EvasionContext::new(&fp);
let script = default_registry().bootstrap(&ctx);
assert!(script.contains("webdriver"));
```
