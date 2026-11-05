---
title: "Stealth policies"
sidebarTitle: "Policies"
---

A [fingerprint](/stealth/fingerprints) says what to claim; a `StealthPolicy`
says how the claims are delivered.


A setting is what the browser should *claim*. The `StealthPolicy` is *how* the
claims are delivered and which protections run beside them. It has no say over a
claim: a preset policy never changes a setting's state, and the two can be given
in any order.

```rust
use seleniumbase_rs_stealth::evasion::fingerprint::{CanvasNoise, Fingerprint, StealthPolicy};

let fp = Fingerprint::builder()
    .canvas_noise(CanvasNoise::Low)        // a shortcut, applied after the base...
    .grant_permissions(true)
    .policy(StealthPolicy::balanced())      // ...whatever the order
    .with_policy(|p| p.disable_csp(true))   // any change, in order
    .build()?;
assert!(fp.policy().chrome_runtime && fp.policy().disable_csp);
Ok::<(), seleniumbase_rs_stealth::evasion::fingerprint::FingerprintError>(())
```

| Preset | What it is |
|---|---|
| `StealthPolicy::default()` | Only what the other evasions rely on: `toString` masked and shadow roots opened. |
| `StealthPolicy::balanced()` | The default, plus the WebRTC leak policy `DisableNonProxiedUdp`, seeded canvas and audio noise, the `window.chrome` stubs and the headless fixes. Every preset and every `randomized` identity uses it. |
| `StealthPolicy::native_only()` | Delivers the claims through CDP and launch arguments where the browser can (`native_spoofing`), with the WebRTC leak policy; no noise, no JavaScript-only stubs. |
| `StealthPolicy::max()` | `balanced()`, plus tracker blocking, granted permissions and QUIC off. |

| Field (builder shortcut) | What it controls |
|---|---|
| `webrtc_policy` (`.webrtc_policy(..)`) | `--force-webrtc-ip-handling-policy`. `None` passes no flag, unless the `webrtc` setting is set, which uses `DisableNonProxiedUdp`. A policy also runs the `webrtc` provider's leak filter. |
| `canvas_noise` (`.canvas_noise(..)`) | The `canvas_noise` provider: `Persistent` (seeded), `Random` (a fresh seed for each document), `Low` (one pixel in eight) or `Natural` (none). |
| `audio_noise` (`.audio_noise(..)`) | The `audio_noise` provider: `Persistent`, `Random` or `Natural`. |
| `native_spoofing` (`.native_spoofing(..)`) | See [Native-level (CDP) spoofing](/stealth/evasions#native-level-cdp-spoofing). |
| `native_tostring` | The `native_tostring` provider, on unless switched off. The other evasions rely on it. |
| `chrome_runtime` | The `chrome_runtime` provider (the `window.chrome` stubs). |
| `headless_fixes` | The `headless`, `permissions` and `hairline` providers, and `Emulation.setFocusEmulationEnabled`. |
| `grant_permissions` (`.grant_permissions(..)`) | `Browser.grantPermissions` for `notifications`, `midi`, `midiSysex`, `clipboardReadWrite`, `clipboardSanitizedWrite`, `paymentHandler`, `backgroundSync`, `idleDetection` and `webAppInstallation`. A position grants the geolocation permission itself. |
| `block_trackers`, `blocked_trackers` | The `tracker_block` provider and `Network.setBlockedURLs` for `Fingerprint::tracker_hosts()`. A tracker list is itself the request to block them. |
| `blocked_ports` (`.blocked_ports(..)`) | Ports refused on `localhost`, `127.0.0.1` and `[::1]` with `Network.setBlockedURLs`, in the same command as the tracker hosts. |
| `quic` (`.quic(..)`) | `Disabled` and `ForceHttp2` add `--disable-quic`; `Enabled` adds `--enable-quic`; `Natural` and `Auto` add nothing. |
| `disable_csp` (`.disable_csp(..)`) | `Page.setBypassCSP`. |
| `open_shadow_roots` | The `attach_shadow` provider, which forces closed shadow roots open so automation can reach into them. A page can detect that, so turn it off where it matters. |

## Fingerprints as JSON

A `Spoof` is written as `"natural"` (and left out), `"random"`, `"off"` or the
value itself; `null` reads as `Natural`. A fingerprint that holds `"random"` is
drawn when it is read, the same way `build()` does it, so give a `seed` for a
repeatable file. A field this version does not know, such as the flat
`screen_width` of the old shape, is an error and not a silent loss. Battery times
that are infinite (`discharging_time` of a charging battery) are written as
`null`.

