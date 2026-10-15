---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
---

# Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper

The stealth scraping layer is no longer part of `seleniumbase-rs`. Since version
0.3.0 (2026-10-15) it lives in its own repository,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
as three crates. The features `scrape` and `impersonate` and the modules
`seleniumbase_rs::wire`, `seleniumbase_rs::scrape` and
`seleniumbase_rs::impersonate` are gone from this crate.

The full tutorial moved with it: [docs/tutorial.md in seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth/blob/main/docs/tutorial.md).

| Crate | Was | What it is |
|---|---|---|
| `seleniumbase-rs-wire` | `seleniumbase_rs::wire` | Pure wire formats: the TLS `ClientHello` parser, JA3, JA4, peetprint, the Akamai HTTP/2 fingerprint, the TLS, HTTP/2 and header specs, and the measured browser table. |
| `seleniumbase-rs-scrape` | feature `scrape` | The cookie jar, `StealthIdentity` and `SessionPolicy`, challenge detection and mitigation, the proxy pool, per-site state and events, `BrowserScraper` (a real Chrome over the pure CDP engine, no TLS spoofing) and the capture tools. Pure Rust. |
| `seleniumbase-rs-impersonate` | feature `impersonate` | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile`, `CustomProfileBuilder`, `HttpSession`, `MitmProxy`, `StealthSession` and `EphemeralCa`. Builds BoringSSL, so it needs CMake and a C/C++ toolchain. |

## Depending on it

The crates are not on crates.io. Depend on them by git, next to this crate:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
# Only if you need browser-identical TLS (builds BoringSSL):
seleniumbase-rs-impersonate = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Cargo finds each crate inside the repository by its name. The stealth crates
take `seleniumbase-rs` by its plain git URL, so when your code also uses this
crate's types (`sb_cdp::Browser`, `Fingerprint`, ...), write the
`seleniumbase-rs` line exactly as above: the same source makes Cargo resolve
both to one commit. A different spelling, such as a `tag`, makes them two
different crates to Cargo, and their types do not match. The repository's
README has the current instructions.

## Status

**Experimental.** Types and paths may change between commits. Only Chrome 155
on macOS is measured; on another system or Chrome version the impersonation
refuses to run instead of guessing. Everything was tested against local servers
and a local Chrome, never against live services.

The long guide this page used to hold (the wire formats, re-measuring when
Chrome updates, the identity file, `BrowserScraper`, `HttpSession`, `MitmProxy`,
`StealthSession` and their limits) is in this repository's git history, in the
commits before 0.3.0; its home is now the stealth repository. What stays here is
the in-browser side: [Fingerprint & Stealth Profiles](fingerprint_stealth.md)
covers what a page can read from inside the browser and how this crate hides
it.
