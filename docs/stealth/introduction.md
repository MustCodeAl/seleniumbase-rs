---
title: "Stealth"
sidebarTitle: "Overview"
---

Websites tell automated browsers from people by the signals a page can read:
`navigator.webdriver`, the user agent and Client Hints, platform, screen size,
hardware concurrency, WebGL vendor, canvas and audio noise, time zone,
location, media devices, fonts, and how input arrives. `seleniumbase-rs`
answers them with a coherent `Fingerprint`, the launch flags and page scripts
it turns into, a `StealthPolicy` that says how the claims are delivered,
profiles loaded from files, proxy and WebRTC settings, and human-paced input.

This section has one page per aspect. It covers what a page can read inside
the browser. What the network sees before any page loads (the TLS and HTTP/2
handshake) is in [Advanced stealth](/stealth/advanced/overview), and scraping
with all of this is in [Scraping](/plugins/overview#scraping).

<Visibility for="agents">

Summary for AI exports: a `Fingerprint` holds one `Spoof` per setting (`Natural`, `Random`, `Value` or `Off`). Build one from a preset such as `Fingerprint::windows_desktop()`, from `Fingerprint::randomized(os, seed)`, or with `Fingerprint::builder()`. A `StealthPolicy` decides how the claims are delivered. A fingerprint reaches the page only through a launch path that applies it: `DriverMode::Uc`, `DriverMode::Cdp`, the Pure CDP engine or the Playwright session. With `DriverMode::WebDriver` only the launch arguments apply. The Stealth pages are Fingerprints, Evasions and launch flags, Policies, Identities and profiles, Proxies and network, Human-like input and Troubleshooting.

</Visibility>

| Page | What it covers |
|---|---|
| [Fingerprints](/stealth/fingerprints) | One `Spoof` per setting, presets, randomised identities, custom fingerprints, what is spoofed |
| [Evasions and launch flags](/stealth/evasions) | The Chrome flags and page scripts a fingerprint turns into, native-level (CDP) spoofing, complementary defenses |
| [Policies](/stealth/policies) | How a `StealthPolicy` delivers the claims, fingerprints as JSON |
| [Identities and profiles](/stealth/profiles) | Profile payloads from other tools, the coherence check |
| [Proxies and network](/stealth/proxies) | Proxies and their passwords, WebRTC, QUIC |
| [Human-like input](/stealth/human-input) | Pointer paths and typing with human timing |
| [Troubleshooting](/stealth/troubleshooting) | When a site still tells the browser is automated |

## Which launch paths apply a fingerprint

A fingerprint reaches the page only through a launch path that applies it:
[UC mode](/tutorials/uc_mode) (`DriverMode::Uc`), `DriverMode::Cdp`, the pure
CDP engine ([CDP Mode](/stealth/cdp-mode)) or the Playwright session
([Stealth with Playwright mode](/stealth/advanced/playwright)). With
`DriverMode::WebDriver` only the launch arguments apply.

## Quick start

Use the unified facade for in-page stealth and scraping:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Its default features include `codec` and `scrape`, without BoringSSL or cmake.
It re-exports the core browser API, `sb_cdp`, `evasion` and `fingerprint`, so the
examples need no separate core dependency. Examples using `#[tokio::main]`
also need Tokio's runtime and macros in the application. Add the facade's
`mimic` feature only for [TLS/HTTP2 impersonation](/scraping/stealthy/overview#which-crate-to-depend-on).

```rust
use seleniumbase_rs_stealth::{BaseCase, BrowserConfig, DriverMode, Fingerprint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::windows_desktop();
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_fingerprint(fp);

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

A fingerprint reaches the page only through a launch path that applies it:

- With `DriverMode::Uc` (above) or `DriverMode::Cdp`, the session registers the
  evasion script and sends the protocol overrides described under
  [Native-level (CDP) spoofing](/stealth/evasions#native-level-cdp-spoofing), and the launch
  arguments apply. The script is the one the Pure CDP engine runs,
  `evasions::session_script` (the 27 providers of `session_registry`); see
  [Built-in providers](/stealth/advanced/providers#built-in-providers).
- With the default `DriverMode::WebDriver`, only the launch arguments apply (user
  agent, `--lang`, `--window-size`, the WebRTC policy, and the QUIC and proxy
  flags). No script is injected and no protocol override is sent.
- The Pure CDP engine and the Playwright session have their own paths, described
  below. The Pure CDP engine runs the same `session_script`. The Playwright
  session runs `bootstrap_script`, the 29 providers of `default_registry`.

## Limitations

<Warning>

* Nothing on this page changes the TLS or HTTP/2 behaviour of the browser. A
  fingerprint reaches the page through scripts and protocol overrides; the
  `ClientHello`, the HTTP/2 connection start and the header order that leave the
  machine are those of the browser you launch. JA3, JA4, peetprint and Akamai
  parsers, a measured table for one browser (Chrome 155 on macOS), a client that
  sends that handshake or one you specify, and `MitmProxy`, which gives a real
  Chrome on the Pure CDP engine the impersonated handshake, are in the separate,
  experimental seleniumbase-rs-stealth crates since 0.3.0; see
  [Stealth Scraping](/scraping/stealthy/first-fetch). The design history is the
  [Stealth and impersonation plan](/design/stealth_parity_plan).
* Some dimensions (WebGL vendor/renderer, canvas/audio noise, media devices,
  fonts, battery, etc.) have no CDP override and remain JavaScript-based even
  when `native_spoofing` is enabled.
* Chromium source patching is not performed. The binary patchers edit byte
  strings in an existing `chromedriver` or Chrome binary.

</Warning>

