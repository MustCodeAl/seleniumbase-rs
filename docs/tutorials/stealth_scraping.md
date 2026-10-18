---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
sidebarTitle: "Stealth scraping"
---

<Badge color="orange">Experimental</Badge>

The stealth scraping layer is no longer part of `seleniumbase-rs`.

<Update label="2026-10-15" description="Version 0.3.0" tags={["Moved"]}>

The layer lives in its own repository,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
as three crates. The features `scrape` and `impersonate` and the modules
`seleniumbase_rs::wire`, `seleniumbase_rs::scrape` and
`seleniumbase_rs::impersonate` are gone from this crate.

</Update>

<Visibility for="agents">

Summary for AI exports: the stealth scraping layer left this crate in 0.3.0 and lives in seleniumbase-rs-stealth, as the crates `seleniumbase-rs-codec`, `seleniumbase-rs-scrape` and `seleniumbase-rs-mimic`. They are not on crates.io: depend on them by git next to `seleniumbase-rs`, and write the `seleniumbase-rs` line with the plain git URL so that Cargo resolves one commit. The layer is experimental; only Chrome 155 on macOS is measured.

</Visibility>

<Tip>

The full tutorial is part of this site now, under
[Stealth scraping](/stealth/overview). Start there.

</Tip>

<Columns cols={2}>

<Tile href="/stealth/overview" title="Overview and crates" description="The three crates, how to depend on them, and how the pieces layer.">
  <Icon icon="layer-group" size={48} />
</Tile>

<Tile href="/stealth/wire-formats" title="Wire formats" description="Measured tables, JA3, JA4, peetprint, re-measuring when Chrome updates.">
  <Icon icon="fingerprint" size={48} />
</Tile>

<Tile href="/stealth/browser-scraper" title="BrowserScraper and HttpSession" description="A real Chrome with challenge detection, and plain requests that continue its session.">
  <Icon icon="globe" size={48} />
</Tile>

<Tile href="/stealth/limits" title="Limits" description="What is measured, what is refused, and troubleshooting.">
  <Icon icon="triangle-exclamation" size={48} />
</Tile>

</Columns>

| Crate | Was | What it is |
|---|---|---|
| `seleniumbase-rs-codec` | `seleniumbase_rs::wire` | Pure wire formats: the TLS `ClientHello` parser, JA3, JA4, peetprint, the Akamai HTTP/2 fingerprint, the TLS, HTTP/2 and header specs, and the measured browser table. |
| `seleniumbase-rs-scrape` | feature `scrape` | The cookie jar, `StealthIdentity` and `SessionPolicy`, challenge detection and mitigation, the proxy pool, per-site state and events, `BrowserScraper` (a real Chrome over the pure CDP engine, no TLS spoofing) and the capture tools. Pure Rust. |
| `seleniumbase-rs-mimic` | feature `impersonate` | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile`, `CustomProfileBuilder`, `HttpSession`, `MitmProxy`, `StealthSession` and `EphemeralCa`. Builds BoringSSL, so it needs CMake and a C/C++ toolchain. |

## Depending on it

The crates are not on crates.io. Depend on them by git, next to this crate:

```toml Cargo.toml
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
