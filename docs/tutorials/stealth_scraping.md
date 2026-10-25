---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
sidebarTitle: "Stealth scraping"
---

<Badge color="orange">Experimental</Badge>

Use the unified `seleniumbase-rs-stealth` facade for core automation, in-page
stealth and scraping, with optional TLS/HTTP2 impersonation.

<Update label="2026-10-15" description="Version 0.3.0" tags={["Moved"]}>

The layer lives in its own repository,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
as three crates. The features `scrape` and `impersonate` and the modules
`seleniumbase_rs_stealth::wire`, `seleniumbase_rs_stealth::scrape` and
`seleniumbase_rs_stealth::impersonate` are gone from this crate.

</Update>

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

<Visibility for="agents">

Summary for AI exports: use one Git dependency, `seleniumbase-rs-stealth`. Its root re-exports the core API, including `sb_cdp` and `stealth`, plus `fingerprint`, `codec` and `scrape` by default. Enable `features = ["mimic"]` for BoringSSL-backed TLS/HTTP2 impersonation through `mimic`. Import through `seleniumbase_rs_stealth`; do not add separate core or implementation-crate dependencies. Wire impersonation remains experimental and measured only for Chrome 155 on macOS.

</Visibility>

<Tip>

The full tutorial is part of this site now, under
[Stealth scraping](/stealth/overview). Start there.

</Tip>

<Columns cols={2}>

<Card title="Overview and crates" icon="layer-group" color="#d47628" href="/stealth/overview">
  The single facade dependency, its features, and the implementation layout.
</Card>

<Card title="Wire formats" icon="fingerprint" color="#199f43" href="/stealth/wire-formats">
  Measured tables, JA3, JA4, peetprint, re-measuring when Chrome updates.
</Card>

<Card title="BrowserScraper and HttpSession" icon="globe" color="#1a85d4" href="/stealth/browser-scraper">
  A real Chrome with challenge detection, and plain requests that continue its session.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#a631be" href="/stealth/limits">
  What is measured, what is refused, and troubleshooting.
</Card>

</Columns>

| Crate | Was | What it is |
|---|---|---|
| `seleniumbase-rs-codec` | `seleniumbase_rs_stealth::wire` | Pure wire formats: the TLS `ClientHello` parser, JA3, JA4, peetprint, the Akamai HTTP/2 fingerprint, the TLS, HTTP/2 and header specs, and the measured browser table. |
| `seleniumbase-rs-scrape` | feature `scrape` | The cookie jar, `StealthIdentity` and `SessionPolicy`, challenge detection and mitigation, the proxy pool, per-site state and events, `BrowserScraper` (a real Chrome over the pure CDP engine, no TLS spoofing) and the capture tools. Pure Rust. |
| `seleniumbase-rs-mimic` | feature `impersonate` | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile`, `CustomProfileBuilder`, `HttpSession`, `MitmProxy`, `StealthSession` and `EphemeralCa`. Builds BoringSSL, so it needs CMake and a C/C++ toolchain. |

## Depending on it

For in-page stealth and scraping, with no BoringSSL or cmake:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

For TLS/HTTP2 impersonation through BoringSSL:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

The root facade re-exports `seleniumbase_rs`, including `sb_cdp` and `stealth`,
alongside `fingerprint`, `codec`, `scrape` and the optional `mimic` module. Use
its module paths throughout; no separate core, codec, scrape or mimic dependency
is needed. See the [unified setup](/stealth/overview#which-crate-to-depend-on)
for the combined import and build requirements.

## Status

<Warning>

**Experimental.** Types and paths may change between commits. Only Chrome 155
on macOS is measured; on another system or Chrome version the impersonation
refuses to run instead of guessing. Everything was tested against local servers
and a local Chrome, never against live services.

</Warning>

The long guide this page used to hold (the wire formats, re-measuring when
Chrome updates, the identity file, `BrowserScraper`, `HttpSession`, `MitmProxy`,
`StealthSession` and their limits) is now the [Stealth scraping](/stealth/overview)
section of this site. What stays here is the in-browser side:
[Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth) covers what a page can
read from inside the browser and how this crate hides it.
