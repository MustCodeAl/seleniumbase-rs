---
title: "External Profile Payloads & Live Probe Capture"
sidebarTitle: "Payloads & capture"
description: "API reference for seleniumbase_rs_stealth::identity::{profile_payloads, capture, js}: ProfileParams, CapturedFingerprint, ProbeInfo, and CAPTURE_JS."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`identity::profile_payloads` parses third-party profile creation JSON payloads into coherent `Fingerprint`s and `BrowserConfig`s, while `identity::capture` parses in-page probe measurements back into a v2 `Fingerprint` for self-consistency checks.

## External profile JSON payloads (`identity::profile_payloads`)

`ProfileParams` models a structured `POST /profile/create` body (`Parameters`, `ProfileFlags`, `StorageOptions`, nested `ProfileFingerprint`, `NavigatorFingerprint`, `ScreenFingerprint`, `ProfileProxy`, `CmdParams`). Unknown flag keywords are logged cleanly and resolved as `Spoof::Natural`.

| Method / function | Purpose |
|---|---|
| `params.to_fingerprint()` | Converts payload fields and seeded random flags into a validated `Fingerprint` |
| `params.to_browser_config(webdriver_url)` | Produces a ready-to-launch `BrowserConfig` (requires `client` feature helpers for runtime injection) |
| `params.apply_runtime_overrides(&mut sb)` | Applies CDP overrides and window sizing onto a running WebDriver `BaseCase` |
| `switches::permitted_switch(flag, val)` / `switches_to_args` | Filters unsafe launch flags consistently |

```rust
use seleniumbase_rs_stealth::identity::profile_payloads::ProfileParams;
use seleniumbase_rs_stealth::json;

let raw = json::json!({
    "name": "worker-1",
    "browser_type": "chromium",
    "os_type": "windows",
    "parameters": {
        "flags": { "webrtc_policy": "mask", "canvas_noise": "persistent" },
        "fingerprint": {
            "navigator": { "platform": "Win32", "hardware_concurrency": 8 },
            "screen": { "width": 1920, "height": 1080, "pixel_ratio": 1 }
        }
    }
});

let params: ProfileParams = json::from_value(raw)?;
let fp = params.to_fingerprint();
assert!(fp.validate().is_coherent());
# Ok::<(), seleniumbase_rs_stealth::json::Error>(())
```

## Live browser probe capture (`identity::capture` & `identity::js`)

`capture` exposes the probe script (`identity::js::capture_js::CAPTURE_JS`) and maps probe JSON (`PROBE_VERSION == 1`) into a `CapturedFingerprint`:

| Item | Role |
|---|---|
| `capture_expression()` | Wraps `CAPTURE_JS` for CDP `Runtime.evaluate` (`awaitPromise`) |
| `capture_async_script()` | Wraps `CAPTURE_JS` for WebDriver `execute_async_script` callbacks |
| `CapturedFingerprint::from_probe(&json, timestamp)` | Parses probe readings into `captured.fingerprint`, `captured.probe` (`ProbeInfo`), and `unmapped` |
| `captured.contradictions()` | Reports self-contradictions inside what the live tab exposes to websites |

```rust
use seleniumbase_rs_stealth::identity::capture::CapturedFingerprint;
use seleniumbase_rs_stealth::json::json;

let readings = json!({
    "version": 1,
    "url": "https://example.test/",
    "secureContext": true,
    "navigator": {
        "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36",
        "platform": "Win32",
        "language": "en-US",
        "languages": ["en-US", "en"],
        "hardwareConcurrency": 8
    },
    "screen": { "width": 1920, "height": 1080, "devicePixelRatio": 1 },
    "intl": { "timeZone": "Europe/Berlin" }
});

let captured = CapturedFingerprint::from_probe(&readings, "2026-11-27T12:00:00Z")?;
assert_eq!(captured.fingerprint.timezone().as_str(), Some("Europe/Berlin"));
# Ok::<(), seleniumbase_rs_stealth::SeleniumBaseError>(())
```
