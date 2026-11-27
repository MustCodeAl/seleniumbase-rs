---
title: "Stealth policies and levels"
sidebarTitle: "Policies & levels"
---

Stealth depth is controlled at two complementary layers in [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth):

1. **Layer 2 (`client`) script levels (`StealthLayer` and `StealthScriptOptions`)**: pick how many of the 42 WebIDL/JavaScript evasion surfaces run on each document (`minimal`, `balanced`, `default`, `strict` / `all`, or per-surface boolean toggles).
2. **Layer 1 (`identity`) delivery policy (`StealthPolicy`)**: control *how* a `Fingerprint`'s claims are delivered (`default`, `native_only`, `balanced`, `max`), including native CDP spoofing, WebRTC IP handling, seeded canvas/audio noise, permission grants, tracker/port blocking, and QUIC flags.

## Layer 2: `StealthLayer` levels

`StealthLayer` (`seleniumbase_rs_stealth::StealthLayer`) bundles a fingerprint, `ProfileFingerprint`, `StealthPlan`, and selective `StealthScriptBundle` into one reusable value:

| Level | Constructor | Active script surfaces | Summary |
|---|---|---|---|
| **Minimal** | `StealthLayer::minimal(os, seed)` | 4 (`webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub`) | Removes core automation markers with the lowest page overhead. |
| **Balanced** | `StealthLayer::balanced(os, seed)` | 35 (all core, worker, console canary, mouse coords, WebRTC, Intl, codecs, geometry, and media guards) | Broad detection coverage while disabling pixel/audio/SVG/timing noise so screenshots and visual diffs stay lossless. |
| **Default** | `StealthLayer::default()` | 35 (same as `balanced`) | Zero-config balanced layer automatically matching the host OS. |
| **Strict** | `StealthLayer::strict(os, seed)` | 42 (all surfaces enabled; `beacon_guard` remains opt-in) | Adds seeded canvas, `OffscreenCanvas`, `toBlob`, Web Audio, `DOMRect`, SVG, and `performance.now()` noise on top of `balanced`. |

```rust
use seleniumbase_rs_stealth::client::StealthScriptOptions;
use seleniumbase_rs_stealth::sb_cdp::LaunchOptions;
use seleniumbase_rs_stealth::{LaunchOptionsFingerprintExt, OsType, StealthLayer};

// Start from a named level and fine-tune any of the 42 surfaces
let layer = StealthLayer::balanced(OsType::Macos, 42)
    .with_script_options(
        &StealthScriptOptions::balanced()
            .canvas_noise(true)
            .beacon_guard(true),
    );

let options = LaunchOptions::builder()
    .headless(true)
    .build()?
    .with_stealth(&layer)?;
```

## Layer 2: `StealthScriptOptions` levels

When building a `StealthScriptBundle` directly (`StealthScriptBundle::from_generated_with`, `from_profile_with`, or `new_with`) or customizing a `StealthLayer` via `.with_script_options(&opts)`, start from one of the four presets:

| Preset | Enabled surfaces | Disabled surfaces |
|---|---|---|
| `StealthScriptOptions::minimal()` | `webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub` (4) | All other 38 surfaces + `beacon_guard` |
| `StealthScriptOptions::balanced()` | All 35 non-perturbing surfaces (core, plugins/MIME, WebGL/WebGPU, screen/window geometry, Client Hints, media devices, `contentWindow`/`srcdoc` iframes, battery, speech, navigator extras, CSS media queries, workers, fonts, Intl/timezone, WebRTC guard, storage quota, hardware APIs, visibility, console debug canary, mouse screen coords, codecs, history length, WebGL shader precision, media capabilities, keyboard layout, clipboard, fullscreen, pointer capabilities) | `canvas_noise`, `offscreen_canvas`, `canvas_toblob_guard`, `audio_noise`, `dom_rect_noise`, `svg_noise`, `performance_jitter`, `beacon_guard` |
| `StealthScriptOptions::strict()` / `all()` / `new()` | All 42 evasion surfaces | `beacon_guard` (opt-in via `.beacon_guard(true)`) |

See [Stealth API: client](/stealth/api/client) for the full list of constructors and extension traits.

## Layer 1: `StealthPolicy` presets

A [fingerprint](/stealth/fingerprints) says what the browser should *claim*. Its `StealthPolicy` says *how* the claims are delivered and which protections run beside them. A policy preset never alters a setting's `Spoof` state, and the two can be configured in any order on `FingerprintBuilder`:

```rust
use seleniumbase_rs_stealth::identity::policy::CanvasNoise;
use seleniumbase_rs_stealth::identity::{Fingerprint, FingerprintError, StealthPolicy};

let fp = Fingerprint::builder()
    .canvas_noise(CanvasNoise::Low)          // shortcut, applied after the base...
    .grant_permissions(true)
    .policy(StealthPolicy::balanced())       // ...whatever the order
    .change_policy(|p| p.disable_csp(true))  // any change, in order
    .build()?;
assert!(fp.policy().chrome_runtime && fp.policy().disable_csp);
Ok::<(), FingerprintError>(())
```

| Preset | What it is |
|---|---|
| `StealthPolicy::default()` | Only what the other evasions rely on: `toString` masked and shadow roots opened. |
| `StealthPolicy::balanced()` | The default, plus the WebRTC leak policy `DisableNonProxiedUdp`, seeded canvas and audio noise, the `window.chrome` stubs, and the headless fixes. Every preset and every `randomized` identity uses it. |
| `StealthPolicy::native_only()` | Delivers the claims through CDP and launch arguments where the browser can (`native_spoofing`), with the WebRTC leak policy; no noise, no JavaScript-only stubs. |
| `StealthPolicy::max()` | `balanced()`, plus tracker blocking, granted permissions, and QUIC off. |

### `StealthPolicy` fields

| Field (builder shortcut) | What it controls |
|---|---|
| `webrtc_policy` (`.webrtc_policy(..)`) | `--force-webrtc-ip-handling-policy`. `None` passes no flag, unless the `webrtc` setting is set, which uses `DisableNonProxiedUdp`. A policy also runs the `webrtc` provider's leak filter. |
| `canvas_noise` (`.canvas_noise(..)`) | The `canvas_noise` provider: `Persistent` (seeded), `Random` (a fresh seed for each document), `Low` (one pixel in eight), or `Natural` (none). |
| `audio_noise` (`.audio_noise(..)`) | The `audio_noise` provider: `Persistent`, `Random`, or `Natural`. |
| `native_spoofing` (`.native_spoofing(..)`) | See [Native-level (CDP) spoofing](/stealth/evasions#native-level-cdp-spoofing). |
| `native_tostring` | The `native_tostring` provider, on unless switched off. The other evasions rely on it. |
| `chrome_runtime` | The `chrome_runtime` provider (the `window.chrome` stubs). |
| `headless_fixes` | The `headless`, `permissions`, and `hairline` providers, and `Emulation.setFocusEmulationEnabled`. |
| `grant_permissions` (`.grant_permissions(..)`) | `Browser.grantPermissions` for `notifications`, `midi`, `midiSysex`, `clipboardReadWrite`, `clipboardSanitizedWrite`, `paymentHandler`, `backgroundSync`, `idleDetection`, and `webAppInstallation`. A position grants the geolocation permission itself. |
| `block_trackers`, `blocked_trackers` | The `tracker_block` provider and `Network.setBlockedURLs` for `Fingerprint::tracker_hosts()`. A tracker list is itself the request to block them. |
| `blocked_ports` (`.blocked_ports(..)`) | Ports refused on `localhost`, `127.0.0.1`, and `[::1]` with `Network.setBlockedURLs`, in the same command as the tracker hosts. |
| `quic` (`.quic(..)`) | `Disabled` and `ForceHttp2` add `--disable-quic`; `Enabled` adds `--enable-quic`; `Natural` and `Auto` add nothing. |
| `disable_csp` (`.disable_csp(..)`) | `Page.setBypassCSP`. |
| `open_shadow_roots` | The `attach_shadow` provider, which forces closed shadow roots open so automation can reach into them. A page can detect this, so turn it off where it matters. |

## Fingerprints as JSON

A `Spoof` is written as `"natural"` (and left out), `"random"`, `"off"`, or the value itself; `null` reads as `Natural`. A fingerprint that holds `"random"` is drawn when it is read, the same way `build()` does it, so give a `seed` for a repeatable file. A field this version does not know, such as the flat `screen_width` of the legacy layout, is an error and not a silent loss. Battery times that are infinite (`discharging_time` of a charging battery) are written as `null`.
