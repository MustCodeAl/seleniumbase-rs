---
title: "Stealth & Avoiding Bot Detection"
sidebarTitle: "Overview"
description: "Practical guide to avoiding bot detection with seleniumbase-rs and seleniumbase-rs-stealth across Pure CDP, UC mode, coherent fingerprints, evasion presets, human-paced input, geo proxies, sessions, and wire alignment."
---

Websites distinguish automated browsers from people at two boundaries: what a page reads inside the browser (`navigator.webdriver`, User-Agent and Client Hints, platform, screen geometry, WebGL/WebGPU, canvas and audio noise, timezone, geolocation, fonts, and input timing) and what the network sees before any page loads (TLS `ClientHello`, HTTP/2 `SETTINGS`, HTTP/3 QUIC parameters, and header order). Patching a single flag such as `navigator.webdriver` is no longer enough.

`seleniumbase-rs` ships core undetected launching (`sb_cdp`, `DriverMode::Uc`, binary patching, and human-paced input) out of the box with zero BoringSSL compilation. When a target needs full persona synthesis, selective JS/CDP masking, proxy rotation, session persistence, or TLS/HTTP2 wire alignment, add [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth) (it re-exports all of `seleniumbase-rs`, so one dependency is enough).

<Visibility for="agents">

Summary for AI exports: Practical guide and overview of bot-detection evasion using `seleniumbase-rs` (Core `sb_cdp`, `DriverMode::Uc`, `undetected::patcher`, `sb_cdp::human::behavior`) and `seleniumbase-rs-stealth` (`identity` fingerprints and specimens, `client` `StealthLayer` levels `minimal`/`balanced`/`strict`, 42-surface `StealthScriptBundle`, `network` `ProxyPool`/`WireFormat`/`MitmProxy`/`http3`, `session` `CdpSession`/`HttpSession`/`HybridSession`).

</Visibility>

## Avoiding bot detection: step-by-step playbook

Most targets are satisfied by Steps 1 through 6 with zero BoringSSL compilation; add Step 7 only when a site checks wire-level TLS or HTTP/2 fingerprints.

### 1. Replace standard WebDriver with Pure CDP or UC mode

Plain WebDriver launches Chrome through `chromedriver`, setting `navigator.webdriver = true`, adding `--enable-automation`, and injecting `$cdc_` markers into the document.

Prefer either the **Pure CDP engine** (`sb_cdp`, which talks directly to Chrome over a single WebSocket with no `chromedriver` binary) or **UC mode** (`DriverMode::Uc`, which strips automation switches and detaches cleanly on reconnect):

<CodeGroup>

```rust Pure CDP (recommended)
use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layer = StealthLayer::default();
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

```rust WebDriver UC mode
use seleniumbase_rs_stealth::client::{BrowserConfigFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layer = StealthLayer::balanced(OsType::Macos, 42);
    let config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .build()?
        .with_stealth(&layer)?;

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

</CodeGroup>

If your suite must keep `chromedriver`, scrub byte-level markers beforehand with `sbrs patch-chromedriver --preset balanced` (see [Binary patching](/stealth/advanced/core/binary-patching)).

### 2. Eliminate headless tells

Classic headless Chrome leaks `HeadlessChrome` in the User-Agent and reports zero-sized outer window bounds. `sb_cdp` uses Chrome's modern headless mode (`--headless=new`), and `StealthLayer::balanced` / `StealthLayer::strict` apply `window_geometry`, `fullscreen_guard`, `visibility_guard`, `permissions`, `chrome_runtime`, and `Emulation.setFocusEmulationEnabled` automatically. Set `SB_HEADLESS=true` or `SB_HEADLESS=false` outside the code rather than hardcoding headed runs in tests.

### 3. Generate coherent fingerprints and Client Hints

Rotating `User-Agent` strings alone creates contradictions against `navigator.platform`, `Sec-CH-UA`, WebGL vendor/renderer strings, memory/CPU counts, and timezone. Generate a complete coherent persona from one seed:

```rust
use seleniumbase_rs_stealth::identity::{
    audit_coherence, BayesianSampler, DeviceCategory, Fingerprint, OsType, RequestContext,
    SampleConstraints,
};

fn build_identity() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::randomized(OsType::Windows, 42);
    let report = fp.validate();
    assert!(report.is_coherent() && report.warnings.is_empty());

    let constraints = SampleConstraints::new()
        .device(DeviceCategory::Desktop)
        .seed(1234);
    let (generated, headers) =
        BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)?;
    assert!(audit_coherence(&generated).is_clean());
    assert!(!headers.is_empty());
    Ok(())
}
```

### 4. Pick the right stealth level and native CDP overrides

| Level | Entry point | Surfaces & behavior | Best for |
|---|---|---|---|
| **Minimal** | `StealthLayer::minimal(os, seed)` | 4 core JS surfaces (`webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub`) + launch args | Lightweight smoke runs |
| **Native only** | `StealthPolicy::native_only()` | CDP overrides + launch flags + WebRTC policy, minimal JS stubs | Avoiding JS getter overrides where CDP suffices |
| **Balanced (default)** | `StealthLayer::default()` / `StealthLayer::balanced(os, seed)` | 35 JS surfaces across workers, mouse coords, WebRTC, `Intl`, codecs, geometry (lossless visually) | Everyday automation & screenshot checks |
| **Strict / Max** | `StealthLayer::strict(os, seed)` + `StealthPolicy::max()` | All 42 surfaces adding canvas, `OffscreenCanvas`, Web Audio, `DOMRect`, SVG, and timing jitter | Strict anti-bot fingerprinting suites |

Turn on `native_spoofing(true)` in `StealthPolicy` so Chrome delivers User-Agent, Client Hints, locale, timezone, geolocation, and screen overrides directly over CDP:

```rust
use seleniumbase_rs_stealth::client::{
    LaunchOptionsFingerprintExt, PageFingerprintExt, StealthLayer,
};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType, StealthPolicy};
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

async fn audit_live_tab() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::randomized(OsType::Macos, 99)
        .to_builder()
        .policy(StealthPolicy::balanced().native_spoofing(true))
        .build()?;

    let layer = StealthLayer::new(fp);
    let options = LaunchOptions::builder().headless(true).build()?.with_stealth(&layer)?;
    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;

    let captured = page.capture_fingerprint().await?;
    for issue in captured.contradictions() {
        eprintln!("contradiction: {issue}");
    }
    browser.close().await?;
    Ok(())
}
```

### 5. Use geo-aligned proxies and WebRTC protection

Pair every proxy with its country so locale, timezone, `Accept-Language`, and WebRTC stay aligned with the egress IP:

```rust
use seleniumbase_rs_stealth::network::CountryCode;
use seleniumbase_rs_stealth::session::{CdpSession, SessionError};

async fn fetch_geo_aligned() -> Result<(), SessionError> {
    let mut session = CdpSession::builder()
        .geo_proxies([
            ("http://user:pass@de.proxy.example:8080", CountryCode::try_new("DE")?),
            ("http://user:pass@gb.proxy.example:8080", CountryCode::try_new("GB")?),
        ])?
        .max_challenge_attempts(4)
        .build()
        .await?;

    let res = session.fetch("https://example.com/").await?;
    println!("status={} bytes={}", res.status, res.body.len());
    session.close().await
}
```

### 6. Simulate human-paced interactions without raw sleeps

Never insert `sleep()` loops for pacing or synchronization. Use `Humanizer` (`page.human(&human)`) for Bezier cursor paths and Gaussian typing cadence, and rely on auto-retrying waits (`locator.wait_for_visible()`):

```rust
use seleniumbase_rs_stealth::sb_cdp::human::Humanizer;
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn human_search(page: &Page) -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let human = Humanizer::seeded(42);
    page.human(&human).click("#search-input").await?;
    page.human(&human).type_text("#search-input", "rust browser automation").await?;
    page.human(&human).press("Enter").await?;
    page.locator(".results-grid").wait_for_visible().await?;
    Ok(())
}
```

### 7. Containerize persistent sessions and align wire TLS when required

- Reuse authenticated state cleanly across isolated runs with the [`browser` plugin](/plugins/builtin/browser), `BrowserPool::acquire()`, or `browser.new_context_with(...)`.
- When a target enforces JA3/JA4/peetprint or HTTP/2 fingerprinting, enable `features = ["network"]` (or `features = ["http3"]`) and combine Chrome behind `MitmProxy` with `HybridSession` ([Stealthy scraping](/stealth/session/overview)).

---

## Architecture and Cargo feature tiers

| Stack component | Crate / Feature | BoringSSL? | What it provides | Reference |
|---|---|---|---|---|
| **Core undetected** | `seleniumbase-rs` (always on) | No | Binary patching (`undetected::patcher`), detached processes, UC mode, `sb_cdp::human`, `StealthOptions` | [Settings](/settings/overview#stealth-and-fingerprint-settings) |
| **Identity & profiles** | `seleniumbase-rs-stealth` (`default-features = false`) | No | `Fingerprint`, `ProfileFingerprint`, `FingerprintGenerator`, `BayesianSampler`, `Spoof<T>`, `StealthPolicy`, `mimic` | [Stealth API: identity](/stealth/api/identity/overview) |
| **Browser evasions & client** | `seleniumbase-rs-stealth` (`features = ["client"]`) | No | `StealthLayer`, 42-surface `StealthScriptBundle`, 32 JS providers, `StealthPlan`, engine extension traits | [Stealth API: client](/stealth/api/client/overview) |
| **Proxies & network wire** | `network-spec` / `network` / `http3` | Only `network` / `http3` | `ProxyPool`, `GeoResolver`, TLS/H2/H3 specs, `WireFormat`, `MitmProxy`, `EphemeralCa` | [Stealth API: network](/stealth/api/network/overview) |
| **Scraping sessions** | `session` (**default**) | Only with `network` | `CdpSession`, `HttpSession`, `HybridSession`, challenge mitigation, `CookieJar`, `Identity` | [Stealth API: session](/stealth/api/session/overview) |

<CodeGroup>

```toml Identity & client masking only (zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["client"] }
```

```toml Default: identity, client, and CdpSession (zero BoringSSL)
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

## Topic guide

| Page | Area | What it covers |
|---|---|---|
| [Fingerprints](/stealth/identity/fingerprints) | Identity | Five-state `Spoof`, presets, randomized identities, custom builders, self-consistency checks |
| [Policies & levels](/stealth/identity/policies) | Identity & Client | `StealthLayer` levels, `StealthScriptOptions` 42 surfaces, `StealthPolicy` presets, JSON format |
| [Identities and profiles](/stealth/identity/profiles) | Identity | External JSON profile payloads (`ProfileParams`) and coherence validation |
| [Evasions and launch flags](/stealth/client/evasions) | Client | Chrome flags, init scripts, native CDP spoofing, complementary binary patching |
| [Proxies and network](/stealth/network/proxies) | Network | Authenticated proxies, WebRTC IP handling, QUIC controls |
| [Human-like input](/stealth/core/human-input) | Core Input | Minimum-jerk pointer paths, Gaussian typing rhythm, eased scrolling |
| [Troubleshooting](/stealth/troubleshooting) | Diagnostics | Auditing contradictions and resolving detection leaks |
| [Advanced stealth](/stealth/advanced/overview) | Advanced | Signal spoofing, custom JS providers, Playwright mode, wire formats, MitmProxy, scraping sessions |

## Limitations

<Warning>

* Identity and client masking alone do not alter the OS-level TLS or HTTP/2 handshake; enable `network` or `http3` for wire-level impersonation ([Stealthy scraping](/stealth/session/overview)).
* Dimensions such as WebGL vendor/renderer, canvas/audio noise, media devices, fonts, and battery remain JavaScript-based even with `native_spoofing(true)`.
* Chromium source patching is never performed; binary patchers (`undetected::patcher`) only replace equal-length byte markers inside existing binaries.

</Warning>
