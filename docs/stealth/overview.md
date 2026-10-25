---
title: "Stealth Scraping"
sidebarTitle: "Overview"
---

<Badge color="orange">Experimental</Badge>

<Note>

Use the unified root facade crate `seleniumbase-rs-stealth`. It re-exports
`seleniumbase_rs`, including `sb_cdp` and `stealth`, plus `fingerprint`, `codec`
and `scrape`. Enable its `mimic` feature for TLS/HTTP2 impersonation.
Consumer examples use this single dependency. Commands with
`cargo run -p <crate> --example <name>` run implementation-crate examples inside
the [stealth repository](https://github.com/MustCodeAl/seleniumbase-rs-stealth).

</Note>

A site can tell a client apart before any page script runs: by the TLS
`ClientHello`, by the HTTP/2 connection start, and by the order and spelling of
the request headers. [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth)
covers what a page can read from inside the browser. This page covers the
other side, the wire, and the pieces that keep an identity, its cookies and its
exit address together while a scraper works through a site's defences.

Everything here is young, and it is not a stable API yet: types and paths may
change between releases. The parts that exist are tested against local captures
and a local Chrome, on one system. Read [Limits](/stealth/limits#limits) before you rely on any
of it.

<Visibility for="agents">

Summary for AI exports: depend on the unified `seleniumbase-rs-stealth` root facade by Git. It re-exports the core API (`sb_cdp`, `stealth`, browser/configuration types) and exposes `fingerprint`, `codec` and `scrape` by default, without BoringSSL or cmake. Enable `features = ["mimic"]` for `mimic::{HttpSession, MitmProxy, StealthSession}` and TLS/HTTP2 impersonation through BoringSSL. Use `seleniumbase_rs_stealth` paths without separate core or implementation-crate dependencies. Wire impersonation remains experimental and measured only for Chrome 155 on macOS.

</Visibility>

## Status

| Part | Crate | State |
|---|---|---|
| Wire parsers: `ClientHello`, JA3, JA4 and `JA4_r`, peetprint, Akamai HTTP/2 strings | `codec` | <Badge color="green">Available</Badge> |
| Measured browser tables (Chrome 155 on macOS) and the lookup that refuses everything else | `codec` | <Badge color="green">Available</Badge> |
| Capture tools `capture_tls` and `capture_hints` | `scrape` | <Badge color="green">Available</Badge> |
| Capture tool `capture_h2` | `mimic` | <Badge color="green">Available</Badge> |
| `ImpersonationProfile`, `CustomProfileBuilder`, `impersonation_client` (wreq with BoringSSL) | `mimic` | <Badge color="green">Available</Badge> |
| RFC 6265 cookie jar | `scrape` | <Badge color="green">Available</Badge> |
| `StealthIdentity`, its identity file, `SessionPolicy` | `scrape` | <Badge color="green">Available</Badge> |
| Challenge detection, mitigation policy, proxy pool, per-site state, events | `scrape` | <Badge color="green">Available</Badge> |
| `BrowserScraper` (a real Chrome, no TLS spoofing) | `scrape` | <Badge color="green">Available</Badge> |
| Ephemeral-CA TLS server | `mimic` | <Badge color="green">Available</Badge> |
| `HttpSession` | `mimic` | <Badge color="green">Available</Badge> |
| `MitmProxy` | `mimic` | <Badge color="green">Available</Badge> |
| `StealthSession` | `mimic` | <Badge color="green">Available</Badge> |

<Warning>

The last three are tested against loopback sites, a scripted browser and, in
tests marked `#[ignore]`, a local Chrome 155 on macOS, run headless. None of
them was checked against a live service. [Limits](/stealth/limits#limits) lists what that
leaves open.

</Warning>

## Features

What the stealth layer does today, in plain words. Each claim was checked
against the code. All of it is experimental, measured only for Chrome 155 on
macOS, and tested against local servers and a local Chrome, not against live
services.

### Measured fingerprints, checked by a round trip

- The capture tools (`capture_tls` and `capture_hints` in `scrape`,
  `capture_h2` in `mimic`) record the TLS `ClientHello`, the client hints,
  `User-Agent`, `Accept-Language`, the HTTP/2 connection start and the header
  lists of a local Chrome. They write
  `crates/codec/src/measured/chrome-155-macos.json` and keep the raw bytes as
  test fixtures.
- Nothing is typed by hand. `seleniumbase_rs_stealth::codec::emulation` embeds that file.
  A system that was not measured gets `NotMeasured`, and a Chrome of another
  major version gets `VersionMismatch`; no other system's values are borrowed.
- The round trip is capture, emulate, compare. The wire test builds a client from
  the profile, connects it to a loopback TLS server of the crate, reads what
  arrived with the crate's own parsers (`ClientHello`, `h2_frames`) and compares it
  with the measurement: the TLS spec equals the measured one minus two known
  gaps (`KnownGap::TrustAnchorsExtension` and `KnownGap::MlDsaSignatureAlgorithms`,
  which BoringSSL and wreq cannot send), the Akamai HTTP/2 string equals the
  measured one, and the headers arrive in the measured order and spelling. The
  committed HTTP/2 spec is also derived again from the stored evidence and the
  raw frame fixtures.

### TLS, JA4 and HTTP/2 emulation through a MITM proxy

`MitmProxy::start` binds a proxy on `127.0.0.1` for a real Chrome. It ends
Chrome's TLS with a leaf certificate of an in-memory CA (`EphemeralCa`),
which Chrome trusts through `--ignore-certificate-errors-spki-list` with the
leaf key pin (never `--ignore-certificate-errors`; the CA is not installed
anywhere). It then replays each request through the impersonating wreq client,
so the site sees the measured `ClientHello` (and so the measured JA4),
the measured HTTP/2 SETTINGS and their order, the connection window, the PRIORITY
frames, the HEADERS priority, the pseudo-header order, and the header order and
spelling, plus Chrome's `priority` header from the measured table. Bodies are
streamed both ways within size limits. A `wss://` or `ws://` WebSocket handshake
is relayed with the measured WebSocket `ClientHello`. By default the proxy
refuses loopback, private and cloud-metadata destinations. Chrome's own TLS
handshake ends at the proxy, so the proxy is the one the site sees.

### A plain HTTP client without a browser

`impersonation_client(&profile)` takes an `ImpersonationProfile` and returns a
`wreq::ClientBuilder` (or an `ImpersonateError`) with the measured TLS and HTTP/2
options and header order set; you add proxies, a trust store and timeouts, then
build it. `ImpersonationProfile::apply` writes the identity headers of one
request. `CustomProfileBuilder` layers your own JA3, `JA4_r` or `JA4_ro`,
peetprint, Akamai string and header set over a measured base, and refuses what
the stack cannot send before a client exists. On top of it, `HttpSession` sends
requests with that client, keeps an RFC 6265 cookie jar (with the public suffix
list), follows redirects by hand, enforces limits (time, body size, redirects,
requests per window and site) and detects challenges on every reply. It does not
run JavaScript or solve challenges. A request body is held in memory; the reply
is read as a stream and capped.

### Dual-mode sessions

`StealthSession` holds one `StealthIdentity` (the fingerprint with its locale,
the measured browser, the cookies, the client hints, and the <Tooltip tip="The exit address a request leaves through: one of your proxies, or the direct connection.">egress</Tooltip> they were
earned through) and works in one of two modes. In HTTP mode an `HttpSession`
sends the requests. In browser mode a real Chrome sends them behind the
`MitmProxy`, so both modes show the same handshake and a clearance earned in one
is valid in the other. When a challenge appears in HTTP mode the session
escalates to the browser, the browser solves it, and the cookies move into the
identity; the session then goes back to HTTP (`SessionPolicy` decides, and a
clearance about to expire sends it back to the browser). The proxy and the HTTP
client use the same proxy egress. A Chrome of another major version than the
measurement is refused before it loads a page.

### The CDP client

The pure CDP engine of seleniumbase-rs (`sb_cdp`) starts Chrome itself with
`--remote-debugging-port` (a port the system picks unless you set one) and
talks to it over one WebSocket for all tabs; it does not use a pipe. It is
async (tokio). The launch arguments contain no `--enable-automation`. For
each tab it sends `Page.enable` and `Emulation.setFocusEmulationEnabled`;
`Network` and `Fetch` are switched on only for features that need them (ad
blocking, proxy authentication, request interception, HAR, a fingerprint). It
never switches on `Runtime`, `DOM`, `Log`, `Debugger` or `Profiler`, because a
page can feel that traffic; a test in seleniumbase-rs
(`tests/sb_cdp_domain_hygiene.rs`) fails if a new call does. Elements are found by running `document.querySelector`
through `Runtime.evaluate`. The `scrape` crate adds only `Network`.

### Stealth overrides in the page

A set of built-in evasion providers (`seleniumbase_rs_stealth::stealth::providers`)
writes one script that runs before any page script
(`Page.addScriptToEvaluateOnNewDocument`), applied by `Page::apply_fingerprint`
or `LaunchOptionsBuilder::fingerprint`. Replaced methods go on their prototype
and replaced getters on the prototype that owns them, so
`Object.getOwnPropertyNames` is unchanged. `Function.prototype.toString` is
masked (`NativeToStringProvider`) so patched functions report `[native code]`.
Canvas noise (`toDataURL`, `toBlob`, `getImageData`) and audio noise
(`AudioBuffer.getChannelData`, `copyFromChannel`,
`AnalyserNode.getFloatFrequencyData`) are a function of the fingerprint seed and
the position, so reading the same canvas twice gives the same bytes. Other
providers cover WebGL, navigator, plugins, permissions, WebRTC, time zone and
locale, geolocation and more.

### Challenge detection and mitigation

`challenge::detect` reads a response and returns a `Detection` with a
`ChallengeKind` (`None`, `JsChallenge`, `Interactive`, `Blocked`,
`RateLimited`) and evidence naming the `Vendor` (Cloudflare, DataDome,
PerimeterX, Akamai, Imperva, Kasada, AWS WAF). A header, a widget or a bare
403 is evidence only; the verdict needs an interstitial marker.
`MitigationPolicy::decide` is a pure function that turns a detection and the
attempt context into one `Action`: `Proceed`, `Retry`, `Escalate`, `Solve`,
`RotateEgress`, `RotateProfile`, `Cooldown` or `GiveUp`. The defaults are
3 attempts per URL, a new egress after 2 challenges in a row, a new profile
after 3, a 900 second cooldown after a block, 60 seconds after a rate limit
without `Retry-After`, and a `Retry-After` capped at 3600 seconds, with
`RetryPolicy::network()` for backoff. `BrowserScraper::solve_challenge` is the
solving step: for a challenge with a widget it moves the pointer onto the page
and calls `Page::solve_captcha` (a click or a drag on Turnstile, reCAPTCHA,
hCaptcha, Friendly Captcha or the DataDome slider; the site decides whether it
is accepted), then waits for the vendor's clearance cookie or for the page to
stop being a challenge, bounded by a timeout. A script challenge is only waited
for. There is no connection to a solving service.

### Proxy pool, rotation and locale from the proxy's country

`ProxyPool` holds `(proxy, country)` pairs and picks by `RotationStrategy`
(`RoundRobin`, `Random`, `LeastFailures`, `StickyPerSite`), quarantining a proxy
after repeated failures. Changing egress opens a new browser context behind the
new proxy (`ContextProxySwitch`) and drops the old one with its cookies, so the
browser is not relaunched. That holds when the new proxy is in a country whose
language and time zone the profile already presents; for another country the
browser is started again, because the fingerprint is written into every
document before its scripts run. The country comes from the proxy spec, and
`StaticGeoResolver` maps it through a built-in table of locales (no network
lookup): language list, `Accept-Language`, time zone and a position near the city,
derived from the fingerprint seed.

A built `Fingerprint` is never changed (fingerprint API v2), so moving one to a
locale builds the next one. `LocaleExt::localize` and `EgressLocale::localize`
return the fingerprint in that locale (language list, `Accept-Language`, time zone
and a position near the city, from the fingerprint's seed) and leave the one they
were given as it was. Every other setting is kept, and a fingerprint that denies
geolocation (`Spoof::Off`) keeps denying it.

```rust
use seleniumbase_rs_stealth::stealth::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs_stealth::scrape::egress::geo::{Locale, LocaleExt};

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 7, 155);
let german = Locale::by_tag("de-DE").ok_or("no such locale")?;
let moved = german.localize(&fingerprint)?;
assert_eq!(moved.timezone().as_str(), Some("Europe/Berlin"));
# Ok(())
# }
```

### Profile and identity rotation

`BrowserScraper::rotate_profile` draws a new fingerprint from a derived seed for
the same operating system, in the locale of the current egress, and restarts the
browser with it (cookies start empty; what is known of each site stays).
`export_identity` and `import_identity` move a `StealthIdentity` out of and into
a scraper; an identity is only accepted through the egress it names, and for
the same Chrome version. The identity file is written atomically with mode
`0600` and is refused on read if others can access it.

### Session state behind a store trait

Per-site state (successes, challenges, cooldowns) is saved through the
`StateStore` trait. Two implementations ship: `InMemoryStateStore` and
`JsonStateStore`, one file written to a temporary sibling, synced and renamed
over the target, with owner-only permissions. There is no other backend. The
JSON store keeps the whole table in memory and rewrites the file on each
change, so it suits a few thousand sites at most, and one process may write a
file at a time.

### Observability

The scraper reports typed events (`ScraperEvent`: request, challenge detected
and solved, action chosen, mode changed, egress and profile rotated,
clearance expiring, cooldown started, identity exported and restored) to an
`EventSink`. URLs and proxies are redacted by construction. Three sinks ship:
`NoopEventSink`, `CollectingEventSink` and `TracingEventSink`, which writes to
the `tracing` crate (not `log`). The proxy also keeps counters (`MitmStats`).

### Human-like input

`Page::human` (in seleniumbase-rs, not in these crates) plays a `Humanizer`
plan on a page: the pointer follows a curved, cubic Bézier stroke with a
bell-shaped speed profile (minimum jerk), a movement time from Fitts's law, a
faint hand tremor, and sometimes an overshoot and correction. The click lands
at a point normally distributed inside the element. Typing has Gaussian
intervals around a words-per-minute rate, a slowly drifting rhythm, longer
pauses after spaces and punctuation, rare hesitations and, if you ask for it,
typos that are noticed and corrected. Scrolling uses wheel notches eased at
both ends. With a seed the plans are reproducible. `BrowserScraper` uses it only
for a short pointer placement and scroll before it attempts a widget.
Pointer drift while idle is not built.

### Code quality

All three crates declare `#![forbid(unsafe_code)]`, and their error types are
`thiserror` enums.

### TLS stack and licences

`mimic` uses wreq `=6.0.0-rc.31` (Apache-2.0) over `btls` and `tokio-btls`
`=0.5.6`, which build BoringSSL (`btls-sys`) and need `cmake` and a C/C++
toolchain. The core crate uses `reqwest` with `rustls` and `ring` for its own
HTTP (the WebDriver engine, downloads), and the CDP engine talks to Chrome over
a plain WebSocket; `codec` and `mimic` also use `ring` for hashing. The three
stealth crates are MIT (workspace `Cargo.toml` and `LICENSE`); BoringSSL
carries its own licences.

<Info>

**What you will learn**

- Which crate to depend on, and what each one costs to build.
- What is measured, what is refused, and why a Windows or Linux persona gets an
  error instead of a guess.
- How to re-measure when Chrome updates.
- How to read the wire formats: JA3, JA4, peetprint and Akamai strings.
- How to build an impersonating client from a measured profile, or from your own
  JA3, JA4, peetprint and Akamai inputs, and what is refused up front.
- How the cookie jar, the stealth identity and its file, and the session policy
  behave.
- How `BrowserScraper` fetches, detects a challenge, mitigates, and rotates the
  egress and the profile.
- How `HttpSession` continues a browser's session as plain requests that carry
  the same wire fingerprint.
- What the ephemeral-CA TLS server is for, and how `MitmProxy` uses it to put a
  real Chrome behind the same fingerprint.
- How `StealthSession` moves between the two as the policies decide.

</Info>

## Which crate to depend on

Use one dependency: the root facade. It selects the core and implementation
crates together, so browser and fingerprint types share one core crate.

<Tabs>
<Tab title="In-page stealth and scraping" icon="cube">

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

The default `scrape` feature also enables `codec`. No BoringSSL or cmake is
needed. `sb_cdp`, `stealth` and `fingerprint` come from the re-exported core API.

</Tab>
<Tab title="TLS/HTTP2 impersonation" icon="lock">

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

`mimic` adds `HttpSession`, `MitmProxy` and `StealthSession`, and retains
`scrape` and `codec`. It builds BoringSSL and needs cmake and a C/C++ toolchain.

</Tab>
</Tabs>

With `features = ["mimic"]`, the combined import is:

```rust
use seleniumbase_rs_stealth::{
    fingerprint::{CanvasNoise, Fingerprint, StealthPolicy},
    scrape::BrowserScraper,
    mimic::{StealthSession, MitmProxy},
};
```

Without `mimic`, use the same `fingerprint`, `scrape`, `codec`, `sb_cdp` and
`stealth` paths; the `mimic` module is unavailable. Import all these APIs from
`seleniumbase_rs_stealth`, without separate core or implementation-crate Git
dependencies. The facade keeps their core types aligned.

The facade exposes the `codec`, `scrape` and `mimic` Cargo feature switches.
Optional core APIs retain their original feature gates; the facade does not
currently forward the core's optional feature switches.

The TLS and HTTP/2 spec errors are `TlsSpecError` and `H2SpecError` in the codec
re-exports. Impersonation links one BoringSSL; adding another crate that links
it to the same build can conflict.

## The three crates

The stealth layer lives in its own repository,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
with its root facade at `.` and three implementation crates. The table below
describes their implementation, not extra dependencies consumers need:

| Crate | What it is |
|---|---|
| `seleniumbase-rs-codec` (`crates/codec`) | Pure wire formats, no I/O: a TLS `ClientHello` parser, JA3, JA4 and peetprint, the Akamai HTTP/2 fingerprint, HTTP/2 frame and HPACK reading, the TLS, HTTP/2 and header specs, and the measured browser table (`emulation`) embedded from `src/measured/*.json`. |
| `seleniumbase-rs-scrape` (`crates/scrape`) | Stealth scraping without TLS spoofing, over the pure CDP engine of seleniumbase-rs. Pure modules for challenge detection and the mitigation policy, egress (proxy pool, geo locale, URL redaction), an RFC 6265 cookie jar, the per-site ledger and its events, and the stealth identity with its session policy; the `runtime` module holds the I/O: `BrowserScraper`, `EgressSwitch`, `JsonStateStore`, the identity file, the tracing sink and the capture tools. No TLS spoofing and no BoringSSL. |
| `seleniumbase-rs-mimic` (`crates/mimic`) | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile` and `CustomProfileBuilder`, `HttpSession` (plain HTTP that continues a browser's session), `MitmProxy` (a loopback proxy that gives a real Chrome the measured TLS), `StealthSession` (moves between plain HTTP and Chrome), `EphemeralCa`, and the HTTP/2 capture tool. Builds BoringSSL, so it needs `cmake` and a C/C++ toolchain. |

`mimic` depends on `scrape`, which depends on `codec`; all three
depend on `seleniumbase-rs`. These crates need some core items to be public (in
`seleniumbase_rs_stealth::sb_cdp` and `seleniumbase_rs_stealth::stealth::randomize`).

<Warning>

**Experimental.** The types and paths may change between commits; nothing here is
published to crates.io. **Measured only for Chrome 155 on macOS.** The wire
tables hold one entry. On any other system, or against another Chrome major
version, the impersonation refuses to run (`NotMeasured`, `VersionMismatch`)
instead of borrowing the values of another system. `codec` and
`scrape` work everywhere; `scrape` does no TLS spoofing. Everything was tested
against local servers and a local Chrome, never against live services.

</Warning>

### Building the workspace

Cargo retrieves the core dependency from Git. A sibling core checkout is not
required. Contributors changing both implementations can use an uncommitted
`.cargo/config.toml` override.

<Steps>

<Step title="Clone the stealth workspace" icon="code-branch">

```sh Terminal
git clone https://github.com/MustCodeAl/seleniumbase-rs-stealth
cd seleniumbase-rs-stealth
```

</Step>

<Step title="Check and test" icon="flask">

```sh Terminal
cargo fmt --all -- --check
cargo clippy -p seleniumbase-rs-stealth --all-targets -- -D warnings
cargo test -p seleniumbase-rs-stealth
```

</Step>

</Steps>

Use `--features mimic` for impersonation, or `--workspace` when developing all
implementation crates. These builds include BoringSSL.
The first build of `mimic` compiles BoringSSL (about 1 GB of target
directory). Tests that start a real Chrome are the `*_chrome.rs` files and are
`#[ignore]`d; run them one binary at a time with `-- --ignored --test-threads=1`.
The examples, what each needs, and how to run them are on
[Runnable examples](/stealth/examples). The repository is on GitHub:

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

## How the pieces layer

An arrow reads "is used by". The `scrape` crate has no BoringSSL; `mimic`
depends on it and adds BoringSSL. `StealthSession` moves between `HttpSession`
and a real Chrome behind `MitmProxy`.

```mermaid
flowchart TD
    NF["seleniumbase_rs_codec<br/>wire formats, measured tables"]
    SC["seleniumbase_rs_scrape<br/>jar, identity, challenges, BrowserScraper"]
    IM["seleniumbase_rs_mimic<br/>wreq client, TLS server"]
    HS["HttpSession"]
    MP["MitmProxy"]
    SS["StealthSession"]
    NF --> SC
    SC --> IM
    IM --> HS
    IM --> MP
    HS --> SS
    MP --> SS
```

*How the crates and sessions layer*

## Where to go next

<Columns cols={2}>

<Card title="Wire formats" icon="fingerprint" color="#d47628" href="/stealth/wire-formats">
  Measured tables, JA3, JA4, peetprint, Akamai strings, impersonating clients.
</Card>

<Card title="Cookies and identity" icon="cookie" color="#199f43" href="/stealth/identity">
  The cookie jar, the stealth identity, session policy, challenge detection.
</Card>

<Card title="BrowserScraper" icon="globe" color="#1a85d4" href="/stealth/browser-scraper">
  Fetch with a real Chrome, detect a challenge, mitigate, rotate egress.
</Card>

<Card title="HttpSession" icon="network-wired" color="#a631be" href="/stealth/http-session">
  Continue a browser's session as plain requests with the same fingerprint.
</Card>

<Card title="TLS server and MitmProxy" icon="shield" color="#1ca1c7" href="/stealth/mitm-proxy">
  Put a real Chrome behind the same fingerprint.
</Card>

<Card title="StealthSession" icon="shuffle" color="#d32a61" href="/stealth/stealth-session">
  Move between HTTP and a browser as the policies decide.
</Card>

<Card title="Runnable examples" icon="play" color="#693acf" href="/stealth/examples">
  Local programs that run each layer on your machine.
</Card>

<Card title="Limits and troubleshooting" icon="triangle-exclamation" color="#d5a910" href="/stealth/limits">
  What is measured, what is refused, and what the errors mean.
</Card>

</Columns>
