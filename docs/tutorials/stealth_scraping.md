---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
sidebarTitle: "Stealth scraping"
---

<Badge color="orange">Experimental</Badge>

A short guide to the stealth scraping layer. It lives in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
(commit `0eb20629` is what these pages were checked against), not in this
crate. Depend on one facade; do not depend on the implementation crates, and
do not look for `wire`, `scrape` or `impersonate` on `seleniumbase_rs`.

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

<Visibility for="agents">

Summary for AI exports: one Git dependency, `seleniumbase-rs-stealth`. The facade re-exports the core API (`sb_cdp`, `stealth`, `fingerprint`) and, by default, `codec` and `scrape`. Enable `features = ["mimic"]` for BoringSSL-backed TLS/HTTP2 impersonation. Import `scrape::BrowserScraper` and `mimic::{HttpSession, MitmProxy, StealthSession}` from `seleniumbase_rs_stealth`; do not add `seleniumbase-rs` or the implementation crates as separate dependencies, and do not use the deep path `scrape::runtime::browser_scraper`. UC mode in this crate is a different path: it lowers in-browser automation signals and does not impersonate TLS. `BrowserScraper` drives a real Chrome and does no TLS spoofing. `HttpSession` and `StealthSession` need `mimic` and refuse any browser/OS pair that was not measured. Only Chrome 155 on macOS is measured. The single-page source in the stealth repository is `docs/tutorial.md`.

</Visibility>

<Tip>

This page picks a tool, shows the dependency, and runs a first fetch. The map
of every piece is [Stealth scraping](/stealth/overview). Limits and the error
table are on [Limits](/stealth/limits).

</Tip>

## Which tool

This crate and the stealth repository cover different layers. Pick one.
Stacking them is rarely the first step, and UC mode does not do what the
stealth crates do.

| You need | Use | BoringSSL |
|---|---|---|
| In-page stealth in a test (navigator, WebGL, canvas) | [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth) and [UC mode](/tutorials/uc_mode) in this crate | No |
| A Cloudflare "Just a moment..." page in a UC session | [UC mode: Cloudflare challenges](/tutorials/uc_mode#cloudflare-challenges). UC mode does not answer an interactive proof. | No |
| A real Chrome fetch, with challenge detection and no TLS spoofing | [`BrowserScraper`](/stealth/browser-scraper) (`scrape`, default) | No |
| Plain HTTP that sends the measured TLS and HTTP/2 | [`HttpSession`](/stealth/http-session) (`mimic`) | Yes |
| HTTP that escalates to Chrome when a challenge appears | [`StealthSession`](/stealth/stealth-session) (`mimic`) | Yes |
| The measured tables, JA3/JA4, or re-measuring Chrome | [Wire formats](/stealth/wire-formats) (`codec`) | Only the HTTP/2 capture needs `mimic` |

<Warning>

None of this was checked against a live bot-protection service. A matching
handshake is one signal among many. Follow each site's terms and the law where
you are. [Limits](/stealth/limits) states what is measured and what is refused.

</Warning>

<Columns cols={2}>

<Card title="Overview and crates" icon="layer-group" color="#d47628" href="/stealth/overview">
  The facade, what each implementation crate is, and how the pieces layer.
</Card>

<Card title="Wire formats" icon="fingerprint" color="#199f43" href="/stealth/wire-formats">
  Measured tables, JA3, JA4, peetprint, re-measuring when Chrome updates.
</Card>

<Card title="BrowserScraper" icon="globe" color="#1a85d4" href="/stealth/browser-scraper">
  A real Chrome, challenge detection, and rotation of egress and profile.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#a631be" href="/stealth/limits">
  What is measured, what is refused, and what each error means.
</Card>

</Columns>

## Depending on it

The facade is the only dependency. Its default features are `codec` and
`scrape`, with no BoringSSL and no cmake. `mimic` is the one that builds
BoringSSL.

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

```toml Cargo.toml
[dependencies]
# Same dependency, when you also want the measured handshake.
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

`mimic` keeps `codec` and `scrape` on and needs cmake and a C/C++ toolchain.
The first build compiles BoringSSL (about 1 GB of target directory). Import
through `seleniumbase_rs_stealth` only:

```rust
use seleniumbase_rs_stealth::{
    fingerprint::{Fingerprint, OsType},
    scrape::BrowserScraper,
    // mimic is absent unless features = ["mimic"]
};
```

Do not also depend on `seleniumbase-rs`, `seleniumbase-rs-codec`,
`seleniumbase-rs-scrape` or `seleniumbase-rs-mimic`. A second copy of the core
makes the browser types disagree, and a second BoringSSL can fail the link.
The facade does not forward this crate's optional features (`mcp-server`,
`turso`, and the rest); those stay on a direct dependency of this crate, which
you only add when you are not going through the facade.

<Accordion title="The implementation crates, and what they used to be">

These are workspace members of the stealth repository. A program depends on
the facade, not on them. The names in the middle column are what this crate
called the same code before 0.3.0. They were never modules of the facade.

| Crate | Was, in this crate, before 0.3.0 | What it is |
|---|---|---|
| `seleniumbase-rs-codec` | `seleniumbase_rs::wire` | Pure wire formats: the TLS `ClientHello` parser, JA3, JA4, peetprint, the Akamai HTTP/2 fingerprint, the TLS, HTTP/2 and header specs, and the measured browser table. |
| `seleniumbase-rs-scrape` | the `scrape` feature | The cookie jar, `StealthIdentity` and `SessionPolicy`, challenge detection and mitigation, the proxy pool, per-site state and events, `BrowserScraper` (a real Chrome over the pure CDP engine, no TLS spoofing) and the capture tools `capture_tls` and `capture_hints`. Pure Rust. |
| `seleniumbase-rs-mimic` | the `impersonate` feature | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile`, `CustomProfileBuilder`, `HttpSession`, `MitmProxy`, `StealthSession`, `EphemeralCa` and the capture tool `capture_h2`. |

</Accordion>

## What is measured, and what is refused

Every wire value the layer imitates was captured from a real browser and stored
as JSON. Nothing is typed by hand. Today there is one entry: **Chrome
155.0.8059.40 on macOS**, captured on 2026-10-11. Any other browser and system
pair is `EmulationError::NotMeasured`. A fingerprint whose user agent claims
another Chrome major is `VersionMismatch`. The crate will not borrow macOS
values for Windows, Linux, Android or iOS: a persona that contradicts itself
is worse than a refusal.

```rust
use seleniumbase_rs_stealth::codec::emulation::{self, EmulationError};
use seleniumbase_rs_stealth::codec::BrowserKind;
use seleniumbase_rs_stealth::stealth::fingerprint::OsType;

# fn demo() -> Result<(), EmulationError> {
let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
assert_eq!(chrome.major, 155);

// A system nobody measured is an error, never a nearby guess.
let windows = emulation::get(BrowserKind::Chrome, OsType::Windows);
assert!(matches!(windows, Err(EmulationError::NotMeasured { .. })));
# Ok(())
# }
```

`codec` and `scrape` still compile and run on every system. Only the
impersonating client refuses. [Wire formats](/stealth/wire-formats) shows how
to read the table and how to re-measure when Chrome moves to a new major. When
it does, nothing updates the file for you.

<Warning>

The captured handshake and the one wreq can send are not the same. BoringSSL
cannot send Chrome 155's `trust_anchors` extension or its ML-DSA signature
algorithms, and it writes no GREASE signature algorithm. JA4 strips GREASE, so
the first two change the JA4 and the third shows up only in the peetprint.
`HttpSession` and `MitmProxy` send this client, gaps included.
`BrowserScraper` does not: Chrome's own handshake leaves the machine.
[Limits](/stealth/limits#limits) lists the gaps by name.

</Warning>

## A first fetch

`BrowserScraper` is re-exported at `seleniumbase_rs_stealth::scrape`. It starts
a real Chrome through the pure CDP engine of this crate. Chrome's own handshake
leaves the machine. What the scraper adds is the loop around a protected page:
detect, solve in the browser, rotate the exit address, give up.

```rust
use seleniumbase_rs_stealth::scrape::egress::geo::CountryCode;
use seleniumbase_rs_stealth::scrape::BrowserScraper;

# async fn demo() -> Result<(), seleniumbase_rs_stealth::scrape::ScrapeError> {
let mut scraper = BrowserScraper::builder()
    .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
    .with_max_challenge_attempts(4)
    .build()
    .await?;
let page = scraper.fetch("https://example.com/").await?;
println!("{} bytes, status {}", page.body.len(), page.status);
scraper.close().await?;
# Ok(())
# }
```

Drop `with_geo_proxies` when you have no proxies. Add it when the site ties a
clearance to the exit address: a spec is
`scheme://[user:password@]host:port`, and the country decides the language and
time zone the browser presents while it leaves through that proxy. The
credentials are taken out of the string at once and do not appear in `Debug`,
logs, events or errors.

`fetch` returns a `BrowserFetch` (the URL after redirects, the status, the
headers, the rendered body, the detection, and how many loads it took). A
challenge is solved in the browser and the page is loaded again. Repeated
challenges rotate the egress and then the profile. The default is three
attempts per URL, which gives up before the profile ladder starts, so raise
`with_max_challenge_attempts` when one fetch should climb that far. A rate
limit or a block with nowhere to rotate starts a cooldown
(`ScrapeError::CoolingDown`). Close the scraper yourself: there is no idle
timeout.

The decisions, the identity file and the events are on
[BrowserScraper](/stealth/browser-scraper). The cookie jar and
`StealthIdentity` are on [Identity and cookies](/stealth/identity).

## Continuing as plain HTTP

Once a page has earned its cookies, plain HTTP is faster than another browser
load, but only if the handshake matches. That is the `mimic` feature.
`HttpSession` keeps the measured TLS `ClientHello`, the HTTP/2 start, the
header order and the identity headers, and it does not run JavaScript.
`StealthSession` holds one identity and moves between the two: HTTP until a
challenge appears, then a real Chrome behind `MitmProxy`, then back.

Both refuse an unmeasured system and a Chrome of another major
(`NotMeasured`, `VersionMismatch`) instead of guessing. Build either only
behind `features = ["mimic"]`:

```rust
use seleniumbase_rs_stealth::mimic::{HttpOptions, HttpSession};
use seleniumbase_rs_stealth::sb_cdp::Page;

# async fn demo(page: Page) -> Result<(), Box<dyn std::error::Error>> {
let session = HttpSession::from_page(&page, None, HttpOptions::new()).await?;
let reply = session.get("https://example.com/api/orders").await?;
let orders: serde_json::Value = reply.json()?;
println!("{orders}");
# Ok(())
# }
```

[`HttpSession`](/stealth/http-session) covers the request API, the cookie jar
and what the session refuses. [`StealthSession`](/stealth/stealth-session)
covers the escalation loop. [`MitmProxy`](/stealth/mitm-proxy) is the loopback
proxy that puts a real Chrome behind the measured handshake.

## Running the examples

The examples are files under `crates/<crate>/examples/` in the stealth
repository, not in this one. None of them reaches the outside network. Run
them from a clone:

```sh Terminal
git clone https://github.com/MustCodeAl/seleniumbase-rs-stealth
cd seleniumbase-rs-stealth
cargo run -p seleniumbase-rs-scrape --example cookie_jar
cargo run -p seleniumbase-rs-scrape --example browser_scraper
```

`cookie_jar` needs nothing. `browser_scraper` needs a local Chrome. The three
that use the measured emulation (`custom_profile`, `http_session`,
`stealth_session`) need the `mimic` crate, so the first build compiles
BoringSSL, and they stop at once with `NotMeasured` on any system but macOS.
The full list, including the capture tools, is on
[Runnable examples](/stealth/examples).

## When Chrome updates

The measured file describes the Chrome that was installed when it was captured.
When Chrome moves to a new major, the profile still describes Chrome 155 until
you re-measure, from a clone, against a headed Chrome (a headless one reports a
different identity, and `capture_hints` refuses it):

```text Terminal
cargo run -p seleniumbase-rs-scrape --example capture_tls
cargo run -p seleniumbase-rs-scrape --example capture_hints
cargo run -p seleniumbase-rs-mimic  --example capture_h2
```

Run them in that order. Each extends the same
`crates/codec/src/measured/<browser>-<major>-<os>.json`, and `capture_h2`
refuses a Chrome of another version than the file it extends. The checks that
follow, and the `SOURCES` list the new file has to join, are on
[Wire formats](/stealth/wire-formats).

<Check>

What was checked, and what was not. The types, re-exports and example names on
this page were read from `seleniumbase-rs-stealth` at `0eb20629` (`src/lib.rs`,
`crates/scrape/src/lib.rs`, `crates/mimic/src/lib.rs`, `docs/tutorial.md`,
`docs/examples.md`). No example was run for this rewrite, and nothing was
checked against a live service. The note on
[Runnable examples](/stealth/examples) records the earlier macOS runs.

</Check>
