---
title: "Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper"
sidebarTitle: "First fetch"
---

<Badge color="orange">Experimental</Badge>

A short guide to the stealth scraping layer. It lives in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
(commit `62f57baf` is what these pages were checked against), a single layered crate
that re-exports all of `seleniumbase-rs` alongside `identity`, `client`, `network`, and `session`.

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

<Visibility for="agents">

Summary for AI exports: one Git dependency, `seleniumbase-rs-stealth`. It re-exports all of `seleniumbase-rs` alongside four layers: `identity` (always on), `client`, `network`, and `session`. By default (`session` feature), `identity`, `client`, `network::spec`, `network::proxy`, and `session::CdpSession` build with zero BoringSSL or cmake. Enable `features = ["network"]` for BoringSSL-backed TLS/HTTP2 impersonation (`network::{WireFormat, MitmProxy}`, `session::{HttpSession, HybridSession}`). Import `session::CdpSession`, `session::HttpSession`, `session::HybridSession`, and `network::MitmProxy` directly from `seleniumbase_rs_stealth`. UC mode in core is Layer 0 and does not impersonate TLS. `CdpSession` drives a real Chrome with no TLS spoofing. `HttpSession` and `HybridSession` need `network` and refuse any browser/OS pair that was not measured. Only Chrome 155 on macOS is measured.

</Visibility>

<Tip>

This page picks a tool, shows the dependency, and runs a first fetch. The map
of every piece is [Stealth scraping](/stealth/session/overview). Limits and the error
table are on [Limits](/stealth/advanced/session/limits).

</Tip>

## Which tool

This crate and the stealth repository cover different layers. Pick one.
Stacking them is rarely the first step, and UC mode does not do what the
stealth crates do.

| You need | Use | BoringSSL |
|---|---|---|
| In-page stealth in a test (navigator, WebGL, canvas) | [Stealth overview](/stealth/introduction) (`identity` + `client` layers, `StealthLayer` levels) and [UC mode](/stealth/core/uc-mode) | No |
| A Cloudflare "Just a moment..." page in a UC session | [UC mode: Cloudflare challenges](/stealth/core/uc-mode#cloudflare-challenges). UC mode does not answer an interactive proof. | No |
| A real Chrome fetch, with challenge detection and no TLS spoofing | [`CdpSession`](/stealth/session/cdp-session) (`session`, default) | No |
| Plain HTTP that sends the measured TLS and HTTP/2 | [`HttpSession`](/stealth/advanced/session/http-session) (`network` feature) | Yes |
| HTTP that escalates to Chrome when a challenge appears | [`HybridSession`](/stealth/advanced/session/hybrid-session) (`network` feature) | Yes |
| The measured tables, JA3/JA4, or re-measuring Chrome | [Wire formats](/stealth/advanced/network/wire-formats) (`network-spec`) | Only the HTTP/2 capture needs `network` |

<Warning>

None of this was checked against a live bot-protection service. A matching
handshake is one signal among many. Follow each site's terms and the law where
you are. [Limits](/stealth/advanced/session/limits) states what is measured and what is refused.

</Warning>

<Columns cols={2}>

<Card title="Overview and crates" icon="layer-group" color="#d47628" href="/stealth/session/overview">
  The facade, what each implementation crate is, and how the pieces layer.
</Card>

<Card title="Wire formats" icon="fingerprint" color="#199f43" href="/stealth/advanced/network/wire-formats">
  Measured tables, JA3, JA4, peetprint, re-measuring when Chrome updates.
</Card>

<Card title="CdpSession" icon="globe" color="#1a85d4" href="/stealth/session/cdp-session">
  A real Chrome, challenge detection, and rotation of egress and profile.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#a631be" href="/stealth/advanced/session/limits">
  What is measured, what is refused, and what each error means.
</Card>

</Columns>

## Depending on it

`seleniumbase-rs-stealth` is the only dependency. Its default `session` feature enables `identity`, `client`, `network::spec`, `network::proxy`, and `session::CdpSession` with no BoringSSL and no cmake. `network` (and `http3`) opt into BoringSSL.

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

```toml Cargo.toml
[dependencies]
# Same dependency, when you also want the measured TLS/HTTP2 handshake.
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["network"] }
```

`network` implies `session`, `client`, and `network-spec` and needs cmake and a C/C++ toolchain.
The first build compiles BoringSSL (about 1 GB of target directory). Import through `seleniumbase_rs_stealth`:

```rust
use seleniumbase_rs_stealth::{
    identity::{Fingerprint, OsType},
    session::CdpSession,
    // network::{MitmProxy, WireFormat} and session::{HttpSession, HybridSession}
    // unlock when features = ["network"] is enabled.
};
```

Do not add a second copy of `seleniumbase-rs` pointing at a different revision or path, or shared browser types will disagree.
`seleniumbase-rs-stealth` does not forward core's optional features (`mcp-server`, `turso`, and the rest).

<Accordion title="The four layers, and what they replaced">

In v0.10.0, the earlier multi-crate stealth workspace (`evasion`, `codec`, `scrape`, `wire`) was unified into four strictly separated modules inside the single `seleniumbase-rs-stealth` crate:

| Layer | Previous crates / features | What it is |
|---|---|---|
| `identity` (always on) | `seleniumbase-rs-evasion` (data/persona half) | Unified `fingerprint` (`Fingerprint`, `ProfileFingerprint`, `FingerprintGenerator`, `BayesianSampler`, `audit_coherence`), `Spoof`, `StealthPolicy`, `mimic` specimens, `profile_payloads`, and `capture`. Pure Rust. |
| `client` (`client` feature) | `seleniumbase-rs-evasion` (browser masking half) | `StealthLayer` levels (`minimal`/`balanced`/`default`/`strict`), 42-surface `StealthScriptBundle` / `StealthScriptOptions`, 32 JS `EvasionProvider`s, `StealthPlan`, engine extension traits, and Layer 0 core re-exports. Pure Rust. |
| `network` (`network-spec` / `network` / `http3`) | `seleniumbase-rs-codec` + `seleniumbase-rs-wire` (transport) | Pure `spec` (TLS `ClientHello`, JA3/JA4/peetprint, Akamai H2, `H3Spec`, `HeaderSpec`, measured tables) and `proxy` (`ProxyPool`, `GeoResolver`); opt-in BoringSSL `WireFormat`, `MitmProxy`, `EphemeralCa`, wire `capture`, and unified `http3` (`Http3Client`, `Http3Pool`, `h3d/`). |
| `session` (`session`, default) | `seleniumbase-rs-scrape` + `HttpSession`/`HybridSession` | `CdpSession` (zero BoringSSL), plus `HttpSession` and `HybridSession` (with `network`), challenge detection and `MitigationPolicy`, `CookieJar`, `Ledger`, `Identity` passport, and `SessionEvent`. |

</Accordion>

## What is measured, and what is refused

Every wire value the layer imitates was captured from a real browser and stored
as JSON. Nothing is typed by hand. Today there is one entry: **Chrome
155.0.8059.40 on macOS**, captured on 2026-10-11. Any other browser and system
pair is `WireMeasurementError::NotMeasured`. A fingerprint whose user agent claims
another Chrome major is `VersionMismatch`. The crate will not borrow macOS
values for Windows, Linux, Android or iOS: a persona that contradicts itself
is worse than a refusal.

```rust
use seleniumbase_rs_stealth::network::spec::measurement::{self, WireMeasurementError};
use seleniumbase_rs::browser::config::Browser;
use seleniumbase_rs_stealth::identity::fingerprint::OsType;

fn demo() -> Result<(), WireMeasurementError> {
    let chrome = measurement::get(Browser::Chrome, OsType::Macos)?;
    assert_eq!(chrome.major, 155);

    // A system nobody measured is an error, never a nearby guess.
    let windows = measurement::get(Browser::Chrome, OsType::Windows);
    assert!(matches!(windows, Err(WireMeasurementError::NotMeasured { .. })));
    Ok(())
}
```

`identity`, `client`, `network::spec`, and `session::CdpSession` still compile and run on every system. Only the
impersonating client (`features = ["network"]`) refuses unmeasured systems. [Wire formats](/stealth/advanced/network/wire-formats) shows how
to read the table and how to re-measure when Chrome moves to a new major. When
it does, nothing updates the file for you.

<Warning>

The captured handshake and the one wreq can send are not the same. BoringSSL
cannot send Chrome 155's `trust_anchors` extension or its ML-DSA signature
algorithms, and it writes no GREASE signature algorithm. JA4 strips GREASE, so
the first two change the JA4 and the third shows up only in the peetprint.
`HttpSession` and `MitmProxy` send this client, gaps included.
`CdpSession` does not: Chrome's own handshake leaves the machine.
[Limits](/stealth/advanced/session/limits#limits) lists the gaps by name.

</Warning>

## A first fetch

`CdpSession` is re-exported at `seleniumbase_rs_stealth::session`. It starts
a real Chrome through the pure CDP engine of this crate. Chrome's own handshake
leaves the machine. What the scraper adds is the loop around a protected page:
detect, solve in the browser, rotate the exit address, give up.

```rust
use seleniumbase_rs_stealth::network::proxy::geo::CountryCode;
use seleniumbase_rs_stealth::session::CdpSession;

async fn demo() -> Result<(), seleniumbase_rs_stealth::session::SessionError> {
    let mut scraper = CdpSession::builder()
        .geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::try_new("DE")?)])?
        .max_challenge_attempts(4)
        .build()
        .await?;
    let page = scraper.fetch("https://example.com/").await?;
    println!("{} bytes, status {}", page.body.len(), page.status);
    scraper.close().await?;
    Ok(())
}
```

Drop `geo_proxies` when you have no proxies. Add it when the site ties a
clearance to the exit address: a spec is
`scheme://[user:password@]host:port`, and the country decides the language and
time zone the browser presents while it leaves through that proxy. The
credentials are taken out of the string at once and do not appear in `Debug`,
logs, events or errors.

`fetch` returns a `CdpFetch` (the URL after redirects, the status, the
headers, the rendered body, the detection, and how many loads it took). A
challenge is solved in the browser and the page is loaded again. Repeated
challenges rotate the egress and then the profile. The default is three
attempts per URL, which gives up before the profile ladder starts, so raise
`max_challenge_attempts` when one fetch should climb that far. A rate
limit or a block with nowhere to rotate starts a cooldown
(`SessionError::CoolingDown`). Close the scraper yourself: there is no idle
timeout.

The decisions, the identity file and the events are on
[CdpSession](/stealth/session/cdp-session). The cookie jar and
`Identity` are on [Identity and cookies](/stealth/advanced/session/identity).

## Continuing as plain HTTP

Once a page has earned its cookies, plain HTTP is faster than another browser
load, but only if the handshake matches. That is the `network` feature.
`HttpSession` keeps the measured TLS `ClientHello`, the HTTP/2 start, the
header order and the identity headers, and it does not run JavaScript.
`HybridSession` holds one identity and moves between the two: HTTP until a
challenge appears, then a real Chrome behind `MitmProxy`, then back.

Both refuse an unmeasured system and a Chrome of another major
(`NotMeasured`, `VersionMismatch`) instead of guessing. Build either only
behind `features = ["network"]`:

```rust
use seleniumbase_rs_stealth::session::{HttpSession, HttpSessionOptions};
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn demo(page: Page) -> Result<(), Box<dyn std::error::Error>> {
    let session = HttpSession::from_page(&page, None, HttpSessionOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: serde_json::Value = reply.json()?;
    println!("{orders}");
    Ok(())
}
```

[`HttpSession`](/stealth/advanced/session/http-session) covers the request API, the cookie jar
and what the session refuses. [`HybridSession`](/stealth/advanced/session/hybrid-session)
covers the escalation loop. [`MitmProxy`](/stealth/advanced/network/mitm-proxy) is the loopback
proxy that puts a real Chrome behind the measured handshake.

## Running the examples

The examples are files under `examples/` in the stealth
repository, not in this one. None of them reaches the outside network. Run
them from a clone:

```sh Terminal
git clone https://github.com/MustCodeAl/seleniumbase-rs-stealth
cd seleniumbase-rs-stealth
cargo run --example cookie_jar
cargo run --example cdp_session
```

`cookie_jar` needs nothing. `cdp_session` needs a local Chrome. The three
that use the measured emulation (`custom_profile`, `http_session`,
`hybrid_session`) need `--features network`, so the first build compiles
BoringSSL, and they stop at once with `NotMeasured` on any system but macOS.
The full list, including the capture tools, is on
[Runnable examples](/stealth/advanced/session/examples).

## When Chrome updates

The measured file describes the Chrome that was installed when it was captured.
When Chrome moves to a new major, the profile still describes Chrome 155 until
you re-measure, from a clone, against a headed Chrome (a headless one reports a
different identity, and `capture_hints` refuses it):

```text Terminal
cargo run --example capture_tls
cargo run --example capture_hints
cargo run --features network --example capture_h2
```

Run them in that order. Each extends the same
`src/network/spec/measured/<browser>-<major>-<os>.json`, and `capture_h2`
refuses a Chrome of another version than the file it extends. The checks that
follow, and the `SOURCES` list the new file has to join, are on
[Wire formats](/stealth/advanced/network/wire-formats).

<Check>

What was checked, and what was not. The types, re-exports and example names on
this page were read from `seleniumbase-rs-stealth` at `62f57baf` (`src/lib.rs`,
`src/identity/mod.rs`, `src/client/mod.rs`, `src/network/mod.rs`, `src/session/mod.rs`).
No example was run for this rewrite, and nothing was
checked against a live service. The note on
[Runnable examples](/stealth/advanced/session/examples) records the earlier macOS runs.

</Check>
