---
title: "Advanced stealth"
sidebarTitle: "Overview"
---

The [Stealth](/stealth/introduction) pages cover the four-layer architecture, stealth levels, and core fingerprints. These pages go deeper into each layer: engine binary patching (Layer 0), multi-surface identity synthesis (Layer 1), custom JS evasion providers and Playwright mode (Layer 2), wire-level TLS/HTTP2/HTTP3 impersonation and MitM proxying (Layer 3), and stateful anti-bot scraping sessions (Layer 4).

Except for binary patching (core Layer 0) and Playwright mode (`seleniumbase-rs-playwright`), all advanced stealth layers live in the single [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth) crate (plus standalone `h3d/` for HTTP/3).

| Page | Layer | Feature / Crate | What it does |
|---|---|---|---|
| [Binary patching](/stealth/advanced/core/binary-patching) | Layer 0 | `seleniumbase-rs` (`undetected::patcher`) | Scrub automation byte markers from `chromedriver` and Chrome binaries; `engine_spoofing_args()` |
| [Signal & hardware spoofing](/stealth/advanced/identity/signal-spoofing) | Layers 1 & 2 | `identity` + `client` (zero BoringSSL) | Coherent WebGL 1/2, WebGPU, Canvas/`OffscreenCanvas`, `DOMRect`, Web Audio, fonts, Client Hints, `Spoof::Mimic`, `BayesianSampler` |
| [Evasion providers](/stealth/advanced/client/providers) | Layer 2 | `client` (zero BoringSSL) | Priority-ordered 32 built-in JS `EvasionProvider` passes and registering custom providers |
| [Stealth with Playwright mode](/stealth/advanced/client/playwright) | Layer 2 | `seleniumbase-rs-playwright` | Give a rustwright Playwright session a coherent `Fingerprint` |
| [Wire formats](/stealth/advanced/network/wire-formats) | Layer 3 | `network-spec` / `network` | Measured Chrome TLS/HTTP2/HTTP3 specs, JA3/JA4/peetprint/Akamai parsers, and `WireFormat` client builder |
| [MitmProxy](/stealth/advanced/network/mitm-proxy) | Layer 3 | `network` (BoringSSL) | Carry a real Chrome's traffic through a loopback proxy presenting the measured TLS/H2 handshake |
| [HTTP/3](/stealth/advanced/network/http3) | Layer 3 | `http3` + `h3d/` (BoringSSL) | Send HTTP/3 requests and `Alt-Svc` upgrades through the standalone `sbrs-h3d` helper |
| [Stealthy scraping](/stealth/session/overview) | Layer 4 | `session` (default) / `network` | `CdpSession` (zero BoringSSL) and `HttpSession` (with `network`) for sites that block automated clients |
| [Advanced stealthy scraping](/stealth/advanced/session/overview) | Layer 4 | `session` + `network` | `HybridSession`, identity passports, challenge detection and mitigation, runnable examples, limits |

## Layer API references

- Layer 1: [Stealth API: identity](/stealth/api/identity/overview) and [Fingerprint API](/stealth/api/identity/fingerprint)
- Layer 2: [Stealth API: client](/stealth/api/client/overview) and [v0.9.4 to v0.10.0 migration](/stealth/api/client/migration)
- Layer 3: [Stealth API: network](/stealth/api/network/overview)
- Layer 4: [Stealth API: session](/stealth/api/session/overview)

<Tip>

Most detection is answered by a coherent [fingerprint](/stealth/identity/fingerprints) or `StealthLayer::balanced` / `StealthLayer::strict` ([Policies & levels](/stealth/identity/policies)), the right engine ([CDP Mode](/stealth/core/cdp-mode) or [UC mode](/stealth/core/uc-mode)), and an address that fits ([Proxies and network](/stealth/network/proxies)). Check those and [Troubleshooting](/stealth/troubleshooting) before adding BoringSSL wire impersonation.

</Tip>
