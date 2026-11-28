---
title: "Low-Level Evasions, UC Script & Core Re-exports"
sidebarTitle: "Evasions, UC & core"
description: "API reference for seleniumbase_rs_stealth::client::{evasions, uc} and Layer-0 re-exports (patcher, dprocess, reactor, behavior, humanize, cdp)."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

For advanced Chrome setups and embedded runtimes that construct launch arguments, CDP commands, or init scripts directly, `seleniumbase_rs_stealth::client` exposes `evasions`, `uc`, and core Layer-0 utilities under one namespace.

## Script & CDP builders (`client::evasions`)

| Function / const | Signature | Role |
|---|---|---|
| `bootstrap_script` | `(fp: &Fingerprint) -> String` | Default per-document JS bootstrap script for `fp` |
| `session_script` | `(fp: &Fingerprint) -> String` | Slimmer session init script used by `StealthPlan` |
| `cdc_scrub` | `() -> String` | Standalone snippet scrubbing `$cdc_` and chromedriver tokens |
| `launch_args` | `(fp: &Fingerprint) -> Vec<String>` | Chromium flags for UA, language, QUIC, WebRTC, proxy, and switches |
| `safe_cdp_overrides` | `(fp: &Fingerprint) -> Vec<(String, Value)>` | Ordered Emulation / Page / Network CDP calls matching `fp` |
| `window_size` | `(fp: &Fingerprint) -> Option<(u32, u32)>` | Viewport / outer window dimensions from `fp` |
| `pdf_viewer_enabled` | `(fp: &Fingerprint) -> bool` | Whether built-in PDF plugin reporting is enabled |
| `CREDENTIAL_HEADER_METHOD` | `&str` | `"Network.setExtraHTTPHeaders"` constant |

```rust
use seleniumbase_rs_stealth::client::evasions;
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType};

let fp = Fingerprint::randomized(OsType::Macos, 7);
let args = evasions::launch_args(&fp);
let overrides = evasions::safe_cdp_overrides(&fp);
let init = evasions::session_script(&fp);

assert!(!init.is_empty());
assert!(!args.is_empty() || !overrides.is_empty());
```

## Undetected-Chrome script (`client::uc`)

`client::uc::UC_STEALTH_SCRIPT` exposes the standalone JavaScript init script paired with WebDriver UC launches (`toString` proxy disguise, `cdc_` cleanup, `navigator.webdriver = false`, `window.chrome` mocking, WebGL/media guards).

## Core Layer-0 re-exports under `client`

`seleniumbase_rs_stealth::client` re-exports core's lightweight undetected and human-input modules alongside `apply`, `script`, `providers`, `evasions`, and `uc`:

| Alias in `client` | Underlying module in `seleniumbase_rs` | Purpose |
|---|---|---|
| `client::patcher` | `seleniumbase_rs::undetected::patcher` | `ChromeBinaryPatcher`, `ChromedriverPatcher`, `EnginePatch`, `PatchPreset` |
| `client::dprocess` | `seleniumbase_rs::undetected::dprocess` | Detached browser process spawning and reaping |
| `client::reactor` | `seleniumbase_rs::undetected::reactor` | Process/handle watcher |
| `client::behavior` / `humanize` | `seleniumbase_rs::sb_cdp::human::{behavior, humanize}` | Bezier mouse trajectories and Gaussian typing |
| `client::cdp` | `seleniumbase_rs::browser::devtools` | Low-level `CdpClient` |
