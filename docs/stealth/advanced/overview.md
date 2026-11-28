---
title: "Advanced stealth"
sidebarTitle: "Overview"
description: "Deep dive into binary patching, full graphics and API spoofing, custom JS evasion providers, Playwright stealth, TLS/H2/H3 wire formats, MitmProxy, and hybrid scraping sessions."
---

The [Stealth](/stealth/introduction) overview covers everyday evasion presets and core fingerprints. These pages go deeper into every subsystem: pre-launch binary patching, coherent WebGL/WebGPU/Canvas/Audio spoofing, custom JS evasion providers, Stealthy Playwright mode, TLS/HTTP2/HTTP3 wire impersonation, MitmProxy, and hybrid browser-plus-HTTP sessions.

## Quick start: compose full-stack stealth

```rust,no_run
use seleniumbase_rs_stealth::client::{
    LaunchOptionsFingerprintExt, PageFingerprintExt, StealthLayer,
};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType, StealthPolicy};
use seleniumbase_rs_stealth::sb_cdp::human::Humanizer;
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::randomized(OsType::Macos, 2026)
        .to_builder()
        .policy(StealthPolicy::max().native_spoofing(true))
        .build()?;
    let layer = StealthLayer::new(fp);

    let options = LaunchOptions::builder()
        .headless(true)
        .shield_webrtc(true)
        .build()?
        .with_stealth(&layer)?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;

    let human = Humanizer::seeded(2026);
    page.human(&human).click("a").await?;

    let captured = page.capture_fingerprint().await?;
    assert!(captured.contradictions().is_empty());
    browser.close().await?;
    Ok(())
}
```

| Topic | Feature / Crate | What it does |
|---|---|---|
| [Binary patching](/stealth/advanced/core/binary-patching) | `seleniumbase-rs` (`undetected::patcher`) | Scrub automation byte markers from `chromedriver` and Chrome binaries; `engine_spoofing_args()` |
| [Browser & graphics spoofing](/stealth/advanced/identity/signal-spoofing) | `identity` + `client` (zero BoringSSL) | Coherent WebGL 1/2, WebGPU, Canvas/`OffscreenCanvas`, `DOMRect`, Web Audio, fonts, Client Hints, `Spoof::Mimic`, `BayesianSampler` |
| [Evasion providers](/stealth/advanced/client/providers) | `client` (zero BoringSSL) | Priority-ordered 32 built-in JS `EvasionProvider` passes and registering custom providers |
| [Stealth with Playwright mode](/stealth/advanced/client/playwright) | `seleniumbase-rs-playwright` | Combine `StealthLayer`, isolated proxy contexts, `Humanizer`, and `PlaywrightSidecar` |
| [Wire formats](/stealth/advanced/network/wire-formats) | `network-spec` / `network` | Measured Chrome TLS/HTTP2/HTTP3 specs, JA3/JA4/peetprint/Akamai parsers, and `WireFormat` client builder |
| [MitmProxy](/stealth/advanced/network/mitm-proxy) | `network` (BoringSSL) | Carry a real Chrome's traffic through a loopback proxy presenting the measured TLS/H2 handshake |
| [HTTP/3](/stealth/advanced/network/http3) | `http3` + `h3d/` (BoringSSL) | Send HTTP/3 requests and `Alt-Svc` upgrades through the standalone `sbrs-h3d` helper |
| [Stealthy scraping](/stealth/session/overview) | `session` (default) / `network` | `CdpSession` (zero BoringSSL) and `HttpSession` (with `network`) for sites that block automated clients |
| [Advanced stealthy scraping](/stealth/advanced/session/overview) | `session` + `network` | `HybridSession`, identity passports, challenge detection and mitigation, runnable examples, limits |

## API references

- [Stealth API: identity](/stealth/api/identity/overview) and [Fingerprint API](/stealth/api/identity/fingerprint)
- [Stealth API: client](/stealth/api/client/overview) and [v0.9.4 to v0.10.0 migration](/stealth/api/client/migration)
- [Stealth API: network](/stealth/api/network/overview)
- [Stealth API: session](/stealth/api/session/overview)

<Tip>

Most detection is resolved by a coherent [fingerprint](/stealth/identity/fingerprints) or `StealthLayer::balanced` / `StealthLayer::strict` ([Policies & levels](/stealth/identity/policies)), the right engine ([CDP Mode](/stealth/core/cdp-mode) or [UC mode](/stealth/core/uc-mode)), and an address that fits ([Proxies and network](/stealth/network/proxies)). Check those and [Troubleshooting](/stealth/troubleshooting) before adding BoringSSL wire impersonation.

</Tip>
