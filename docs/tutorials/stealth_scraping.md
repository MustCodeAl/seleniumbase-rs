---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
---

# Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper

<Badge color="orange">Experimental</Badge>

The stealth scraping layer is no longer part of `seleniumbase-rs`. Since version
0.3.0 (2026-10-15) it lives in its own repository,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
as three crates. The features `scrape` and `impersonate` and the modules
`seleniumbase_rs::wire`, `seleniumbase_rs::scrape` and
`seleniumbase_rs::impersonate` are gone from this crate.

<Tip>

The full tutorial is part of this site now, under
[Stealth scraping](../stealth/overview.md). Start there.

</Tip>

<Columns cols={2}>

<Card title="Overview and crates" icon="layer-group" href="/stealth/overview">

The three crates, how to depend on them, and how the pieces layer.

</Card>

<Card title="Wire formats" icon="fingerprint" href="/stealth/wire-formats">

Measured tables, JA3, JA4, peetprint, re-measuring when Chrome updates.

</Card>

<Card title="BrowserScraper and HttpSession" icon="globe" href="/stealth/browser-scraper">

A real Chrome with challenge detection, and plain requests that continue its session.

</Card>

<Card title="Limits" icon="triangle-exclamation" href="/stealth/limits">

What is measured, what is refused, and troubleshooting.

</Card>

</Columns>

| Crate | Was | What it is |
|---|---|---|
| `seleniumbase-rs-codec` | `seleniumbase_rs::wire` | Pure wire formats: the TLS `ClientHello` parser, JA3, JA4, peetprint, the Akamai HTTP/2 fingerprint, the TLS, HTTP/2 and header specs, and the measured browser table. |
| `seleniumbase-rs-scrape` | feature `scrape` | The cookie jar, `StealthIdentity` and `SessionPolicy`, challenge detection and mitigation, the proxy pool, per-site state and events, `BrowserScraper` (a real Chrome over the pure CDP engine, no TLS spoofing) and the capture tools. Pure Rust. |
| `seleniumbase-rs-mimic` | feature `impersonate` | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile`, `CustomProfileBuilder`, `HttpSession`, `MitmProxy`, `StealthSession` and `EphemeralCa`. Builds BoringSSL, so it needs CMake and a C/C++ toolchain. |

## Depending on it

The crates are not on crates.io. Depend on them by git, next to this crate:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
# Only if you need browser-identical TLS (builds BoringSSL):
seleniumbase-rs-mimic = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

<Warning>

Cargo finds each crate inside the repository by its name. The stealth crates
take `seleniumbase-rs` by its plain git URL, so when your code also uses this
crate's types (`sb_cdp::Browser`, `Fingerprint`, ...), write the
`seleniumbase-rs` line exactly as above: the same source makes Cargo resolve
both to one commit. A different spelling, such as a `tag`, makes them two
different crates to Cargo, and their types do not match. The repository's
README has the current instructions.

</Warning>

## Status

**Experimental.** Types and paths may change between commits. Only Chrome 155
on macOS is measured; on another system or Chrome version the impersonation
refuses to run instead of guessing. Everything was tested against local servers
and a local Chrome, never against live services.

The long guide this page used to hold (the wire formats, re-measuring when
Chrome updates, the identity file, `BrowserScraper`, `HttpSession`, `MitmProxy`,
`StealthSession` and their limits) is now the [Stealth scraping](../stealth/overview.md)
section of this site. What stays here is the in-browser side:
[Fingerprint & Stealth Profiles](fingerprint_stealth.md) covers what a page can
read from inside the browser and how this crate hides it.
