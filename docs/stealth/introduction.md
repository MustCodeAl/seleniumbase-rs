---
title: "Stealth"
sidebarTitle: "Overview"
---

Websites distinguish automated browsers from people using signals at two boundaries: what a page reads inside the browser (`navigator.webdriver`, user agent and Client Hints, platform, screen geometry, WebGL/WebGPU, canvas and audio noise, timezone, geolocation, fonts, and human input timing) and what the network sees before any page loads (TLS `ClientHello`, HTTP/2 `SETTINGS`, HTTP/3 QUIC transport, and header order).

In v0.10.0, stealth is organized into **Core Layer 0** (shipped in `seleniumbase-rs`) plus **four independent layers** in the single [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth) crate (plus standalone `h3d/` for HTTP/3). Every layer ships with sensible defaults and exposes full low-level controls underneath, so you opt into only the layer and level you need.

<Visibility for="agents">

Summary for AI exports: Core `seleniumbase-rs` provides Layer 0 (`undetected::{patcher, dprocess, reactor, uc}`, `sb_cdp::human::{behavior, humanize}`, `browser::options::StealthOptions`). `seleniumbase-rs-stealth` re-exports all of `seleniumbase-rs` alongside four strictly separated layers: Layer 1 `identity` (always on with `default-features = false`, zero BoringSSL: `Fingerprint`, `ProfileFingerprint`, `FingerprintGenerator`, `BayesianSampler`, `Spoof<T>`, `StealthPolicy`, `mimic`, `capture`), Layer 2 `client` (`features = ["client"]`, pure Rust, zero BoringSSL: `StealthLayer` levels `minimal`/`balanced`/`default`/`strict`, 42-surface `StealthScriptBundle` and `StealthScriptOptions`, 32 JS `EvasionProvider` passes, `StealthPlan`, engine extension traits), Layer 3 `network` (`network-spec` pure Rust; `network` and `http3` opt into BoringSSL/wreq + standalone `h3d/`), and Layer 4 `session` (`default = ["session"]`, zero BoringSSL for `CdpSession`; `network` unlocks `HttpSession` and `HybridSession`).

</Visibility>

## Architecture: Core Layer 0 and four stealth layers

| Layer | Crate / Feature | BoringSSL? | What it owns | API reference |
|---|---|---|---|---|
| **0. Core undetected** | `seleniumbase-rs` (always on) | No | Binary patching (`undetected::patcher`), detached processes (`undetected::dprocess`), UC mode (`undetected::uc`), human-paced input (`sb_cdp::human::{behavior, humanize}`), `StealthOptions` | [Stealth settings](/api/stealth-settings) |
| **1. `identity`** | `seleniumbase-rs-stealth` (`default-features = false`) | No | Persona and fingerprint data: `Fingerprint`, `ProfileFingerprint`, `FingerprintGenerator`, `FingerprintOptions`, `BayesianSampler`, `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `Spoof<T>`, `StealthPolicy`, `mimic` specimens, `profile_payloads`, `capture` | [Stealth API: identity](/stealth/api/identity) |
| **2. `client`** | `seleniumbase-rs-stealth` (`features = ["client"]`) | No | Browser-side masking: `StealthLayer` (`default`, `minimal`, `balanced`, `strict`), 42-surface `StealthScriptBundle` / `StealthScriptOptions`, 32 JS `EvasionProvider`s, `StealthPlan`, engine extension traits (`LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt`) | [Stealth API: client](/stealth/api/client) |
| **3. `network`** | `network-spec` *(pure)* / `network` + `http3` *(BoringSSL)* | Only `network` / `http3` | Pure `spec` (TLS `ClientHello`, JA3/JA4/peetprint, Akamai H2, H3, headers, measured browser tables) and `proxy` (`ProxyPool`, `GeoResolver`); opt-in BoringSSL `WireFormat`, `MitmProxy`, `EphemeralCa`, wire `capture`, and `http3` (`Http3Client`, `Http3Pool`, standalone `h3d/`) | [network::spec](/stealth/api/codec) / [network & sessions](/stealth/api/wire) |
| **4. `session`** | `session` (**default**) | Only with `network` | Stateful anti-bot sessions: `CdpSession` (pure CDP, default), `HttpSession` (wire HTTP/2+3, with `network`), `HybridSession` (dual-mode HTTP and Chrome behind `MitmProxy`, with `network`), challenge detection and `MitigationPolicy`, `CookieJar`, `Ledger`, `Identity` passport | [Stealth API: session](/stealth/api/scrape) |

## Stealth levels

Choose your protection level at two places: **Cargo feature tier** (which layers you compile) and **runtime stealth level** (how aggressively Layer 1 and Layer 2 mask the browser):

| Level | Entry point | Surfaces & behavior | Best for |
|---|---|---|---|
| **Minimal** | `StealthLayer::minimal(os, seed)` / `StealthScriptOptions::minimal()` | 4 core JS surfaces (`webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub`) plus launch args | Fast smoke runs and lightweight automation with minimal page overhead |
| **Native only** | `StealthPolicy::native_only()` | CDP overrides and launch flags where Chrome supports them, plus WebRTC leak policy; no JS-only stubs or canvas/audio noise | Avoiding JS getter overrides on properties covered natively by CDP |
| **Balanced (default)** | `StealthLayer::default()` / `StealthLayer::balanced(os, seed)` / `StealthScriptOptions::balanced()` | 35 JS surfaces (core, workers, console canary, mouse coords, WebRTC, Intl, codecs, geometry, media) with pixel/audio/SVG/timing noise off for visually lossless output | Everyday browser automation and screenshot-sensitive pages |
| **Strict / Max** | `StealthLayer::strict(os, seed)` + `StealthPolicy::max()` | All 42 JS surfaces (adds seeded canvas, `OffscreenCanvas`, `toBlob`, Web Audio, `DOMRect`, SVG, and `performance.now()` jitter) plus tracker blocking, granted permissions, and QUIC off (`beacon_guard` opt-in) | Strict anti-bot fingerprinting checks |

See [Policies & levels](/stealth/policies) for the complete field-by-field breakdown of `StealthLayer`, `StealthScriptOptions`, and `StealthPolicy`.

## Topic pages

| Page | Layer | What it covers |
|---|---|---|
| [Fingerprints](/stealth/fingerprints) | Layer 1 (`identity`) | Five-state `Spoof`, presets, randomized identities, custom builders, self-consistency checks |
| [Evasions and launch flags](/stealth/evasions) | Layer 2 (`client`) | Chrome flags, init scripts, native-level (CDP) spoofing, complementary binary patching |
| [Policies & levels](/stealth/policies) | Layers 1 & 2 | `StealthLayer` levels, `StealthScriptOptions` 42 surfaces, `StealthPolicy` presets, JSON format |
| [Identities and profiles](/stealth/profiles) | Layer 1 (`identity`) | External JSON profile payloads (`ProfileParams`) and coherence validation |
| [Proxies and network](/stealth/proxies) | Layers 1 & 3 | Authenticated proxies, WebRTC IP handling, QUIC controls |
| [Human-like input](/stealth/human-input) | Layer 0 (core) | Minimum-jerk pointer paths, Gaussian typing rhythm, eased scrolling |
| [Troubleshooting](/stealth/troubleshooting) | All layers | Diagnosing leaks when a site still flags automation |
| [Advanced stealth](/stealth/advanced/overview) | Layers 0 to 4 | Providers, binary patching, Playwright mode, wire formats, MitmProxy, HTTP/3, stealthy scraping |

## Which launch paths apply a fingerprint or `StealthLayer`

A `Fingerprint` or `StealthLayer` reaches the page through any launch path that has a CDP channel:
[Pure CDP](/stealth/cdp-mode) (`LaunchOptionsFingerprintExt`),
[UC mode](/tutorials/uc_mode) (`DriverMode::Uc` via `BrowserConfigFingerprintExt`),
`DriverMode::Cdp`, or the Playwright session ([Stealth with Playwright mode](/stealth/advanced/playwright)).
With plain `DriverMode::WebDriver`, only launch arguments apply (no init script or CDP protocol overrides).

## Quick start

Pick the Cargo feature tier that fits your stack. `seleniumbase-rs-stealth` re-exports all of `seleniumbase-rs`, so one dependency is enough:

<CodeGroup>

```toml Layers 1 + 2 only (zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["client"] }
```

```toml Default: Layers 1-4 up to CdpSession (zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

```toml Full stack + TLS/HTTP2 impersonation (BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["network"] }
```

```toml Full stack + HTTP/3 via sbrs-h3d (BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

</CodeGroup>

### Pure CDP with `StealthLayer` levels

```rust
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::{LaunchOptionsFingerprintExt, OsType, StealthLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Zero-config balanced preset for this host OS (or use ::minimal / ::strict)
    let layer = StealthLayer::default();
    let _strict = StealthLayer::strict(OsType::Macos, 42);

    let options = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&layer)?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    browser.close().await?;
    Ok(())
}
```

### WebDriver UC mode with `Fingerprint` or `StealthLayer`

```rust
use seleniumbase_rs_stealth::{
    BaseCase, BrowserConfig, BrowserConfigFingerprintExt, DriverMode, Fingerprint,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::windows_desktop();
    let config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .build()?
        .with_fingerprint(&fp)?;

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

## Limitations

<Warning>

* Layers 1 and 2 (`identity` and `client`) do not change the TLS or HTTP/2 handshake leaving the machine; Chrome sends its own wire handshake. For JA3/JA4/peetprint/Akamai parsing, measured browser tables (Chrome 155 on macOS), `WireFormat`, `MitmProxy`, `HttpSession`, and `HybridSession`, enable Layer 3 (`features = ["network"]` or `features = ["http3"]`) and see [Stealthy scraping](/scraping/stealthy/overview).
* Some dimensions (WebGL vendor/renderer, canvas and audio noise, media devices, fonts, battery) have no CDP override and remain JavaScript-based even when `native_spoofing` is enabled.
* Chromium source patching is not performed. Layer 0 binary patchers (`undetected::patcher`) edit equal-length byte markers in an existing `chromedriver` or Chrome binary.

</Warning>
