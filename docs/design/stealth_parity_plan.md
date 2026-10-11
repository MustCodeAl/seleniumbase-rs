# Stealth parity plan: stealthscraper-rs features, JA3/JA4 and Akamai impersonation

Status (2026-10-11, branch `feat/pure-cdp-and-mcp-parity` at `c5a6f2f`): **WP0 to WP13 and WP17 are
merged. WP14 (`HttpSession`), WP15 (`MitmProxy`) and WP16 (`StealthSession`) are in progress. WP18 and
WP19 are open.** The plan was written on 2026-10-10 as design only, at `8c1394d`. Where the code differs
from what is written below, [the next section](#status-and-what-the-code-changed) says how, and the code
wins. The user-facing description of what exists is [Stealth Scraping](../tutorials/stealth_scraping.md);
the layout rules are recorded in [ADR 003](../adr/003-stealth-scraping-layout.md).
Sources: the project memory (`stealthscraper-feature-parity`, `project-priorities-2026-10-10`),
three Context7 reads of `/ypno/stealthscraper-rs`, the wreq 6.0.0-rc.31, wreq-proto 0.2.6,
http2 0.5.20 and btls 0.5.6 sources in `~/.cargo/registry`, and our code read through TokenSave.

## Status and what the code changed

| WP | Scope | State |
|---|---|---|
| 0 | Scaffold, features, lint block, purity test | Merged |
| 1 | `egress` (redact, route, geo, pool), `randomized_with_major` | Merged |
| 2 | `challenge` (detect, markers, mitigation) | Merged |
| 3 | `ledger::state`, `runtime::json_store` | Merged |
| 4 | `jar` (cookie store, `Set-Cookie`) | Merged |
| 5 | `wire::ids` | Merged |
| 6 | `wire::{client_hello, ja3, ja4, tls_spec}` | Merged |
| 7 | `wire::{akamai, h2_frames, h2_spec, header_spec}` | Merged |
| 8 | `passport` (identity, policy), `ledger::events`, tracing sink, identity file | Merged |
| 9 | `runtime::capture`, `capture_tls`, `capture_hints`, first Chrome measurement | Merged |
| 10 | `impersonate::tls_server` | Merged |
| 11 | `impersonate::capture_h2`, `wire::emulation`, Chrome HTTP/2 and header measurement | Merged |
| 12 | `impersonate::{convert, client}`, `CustomProfileBuilder`, wire probe | Merged |
| 13 | `scrape::error`, `runtime::{cdp, egress_switch, browser_scraper}` | Merged |
| 14 | `impersonate::http_session` | In progress (the file is a placeholder) |
| 15 | `impersonate::mitm` | In progress (the files are placeholders) |
| 16 | `impersonate::stealth_session` | In progress (the file is a placeholder) |
| 17 | Hardening: `session_registry`, secure-context `userAgentData`, screen through the device metrics, no `Runtime.enable` | Merged |
| 18 | Split-role security review | Open |
| 19 | Re-exports, final gates, documentation. The tutorial, the `fingerprint_stealth.md` fixes, ADR 003, ROADMAP and CHANGELOG are written; the re-exports in `lib.rs`, `scrape/mod.rs` and `impersonate/mod.rs`, and the full `--all-targets` gates, are not | Open |

Deviations from the plan that the code made. The plan's text above was not rewritten; read it with these:

- **`SpecError` is split per module.** The plan has one `SpecError` that holds every variant. In the code
  `wire::tls_spec::SpecError` holds the TLS variants, `wire::h2_spec::SpecError` the HTTP/2 variants, and
  the variant that names an unmeasured browser and system, `NotMeasured { kind, os }`, lives in
  `wire::emulation::EmulationError` (with `VersionNotMeasured`, `Parse`, `TooLarge`, `Inconsistent`,
  `Headers` and `Duplicate`). `wire::emulation::SpecError` is an alias for `EmulationError`, so the plan's
  `SpecError::NotMeasured` is reachable under that name. `ImpersonateError` wraps each of them as `Tls`,
  `Http2` and `Emulation`, next to `Ja3`, `Ja4`, `Peetprint`, `Akamai`, `Headers`, `Convert`,
  `VersionMismatch`, `Incomplete`, `Fingerprint`, `UnknownHeader` and `Build`; there is no single `Spec`
  variant.
- **`KnownGap` lives in `wire::tls_spec`**, not in `emulation`. `wire::emulation` re-exports it. The plan's
  name for the HTTP/2 spec, `Http2Spec`, is an alias of `H2Spec`.
- **`BrowserKind` has no serde.** The enum in `wire/mod.rs` derives only `Debug`, `Clone`, `Copy`,
  `PartialEq` and `Eq`. `EmulationRef` in `scrape::passport::identity` serializes it through a private
  module that knows the one spelling, `chrome`, so a new variant breaks the build there until it is handled.
- **`Mode` lives in `scrape::challenge::mitigation`.** `scrape::ledger::state` and
  `scrape::passport::policy` import it from there.
- **Chrome 155 sends a GREASE signature algorithm that the vendored BoringSSL does not write.** It is not a
  `KnownGap` variant (those are `TrustAnchorsExtension` and `MlDsaSignatureAlgorithms`). JA3 and JA4 strip
  GREASE, so only the peetprint shows it, and a wire test pins the difference and fails once a newer
  BoringSSL closes it. A `TlsSpec` read from a hello drops GREASE signature algorithms.
- **`ScrapeError` has three more variants** than §3.13 lists: `CoolingDown`, `Navigation` and
  `NoEgressAvailable`. `BrowserScraperBuilder` has more settings than the sketch: `mitigation`,
  `solve_timeout`, `clock`, `launcher` (the `BrowserLauncher` port), `fingerprint` and `session_store`.
- **`impersonate` also enables `brotli`**, for the certificate compressor of the impersonated hello (Chrome
  offers brotli certificate compression).

## 0. Decisions

1. Nothing from `stealthscraper-rs`, `impersonate-rs`, `impcurl`, `curl-impersonate-cli` or
   `wreq-util` enters Cargo.toml, optional or not. Their numbers (Chrome 153, Safari 27) are hints.
2. Two new features; the default build gains no dependency and no C code. `scrape` (pure Rust, adds
   only `psl`): domain layer, JSON state store, CDP browser leg, TLS and client-hint capture.
   `impersonate = ["scrape", ...]`: wreq `=6.0.0-rc.31` (BoringSSL through `btls`, needs cmake and a
   C/C++ toolchain), MITM proxy, `HttpSession`, `StealthSession`, HTTP/2 capture.
3. Every TLS, HTTP/2, header and client-hint value ships as JSON written by our capture tools against
   the local browsers, embedded with `include_str!`, parsed once, never typed by hand. An unmeasured
   browser/OS pair is an error (`SpecError::NotMeasured`), never a fallback to bare wreq.
4. Custom JA3 and Akamai strings overlay a measured base. What BoringSSL or wreq's HTTP/2 stack cannot
   emit is refused with a named `SpecError` before a client exists; what is accepted is proven by a
   wire round trip.
5. Hexagonal: `src/scrape/domain` is pure (no tokio, `std::fs`/`std::net`/`std::process` or clock
   reads; functions take `now`), enforced by a `syn` test that allows from sb_cdp only the data types
   `Proxy`, `Cookie`, `SameSite`, `Captcha` and the `sync::locked` helper. Adapters:
   `src/scrape/adapters` (std, tokio, sb_cdp) and `src/impersonate` (wreq, btls, hyper). Ports:
   `StateStore`, `EventSink`, `GeoResolver`, `EgressSwitch`.
6. One `thiserror` enum per module, mapped to `SeleniumBaseError::Stealth { reason }` at the crate
   boundary. `tracing` only, with redacted fields (`instrument(skip_all, fields(url = %redacted))`).
7. New modules: `#![forbid(unsafe_code)]`, the pedantic lint block `src/lib.rs` gives `sb_cdp`, files
   under 800 lines, functions under 50, named constants, `.expect()` only with a reason.

## 1. Feature map

| # | stealthscraper-rs feature | Our home | Reused as is | New |
|---|---|---|---|---|
| 1 | Local MITM `TlsSpoofingProxy`: terminate the browser's TLS, replay with the profile's ClientHello and H2, hyper streaming, hot-swappable upstream | `impersonate::mitm::MitmProxy`, `impersonate::tls_server` | `sb_cdp::Proxy`, `LaunchOptionsBuilder::{proxy, arg}`, `ProxyAuth` (answers the MITM's 407 over `Fetch.authRequired`), `common::shutdown::{spawn_cleanup, cleanup_within, timeout_from_env}` | ephemeral CA, SPKI pin, CONNECT + h1 leg, wreq replay, upstream swap |
| 2 | H2 SETTINGS and their order, connection window, HEADERS priority, pseudo-header order, header set and order | `wire::{h2_spec, header_spec, emulation}`, `impersonate::convert` | - | measured `Http2Spec`/`HeaderSpec` converted to `wreq::http2::Http2Options` and `OrigHeaderMap` |
| 3 | `impersonation_client(&BrowserProfile) -> wreq::ClientBuilder` with UA, `Sec-CH-UA*`, `Accept-Language` | `impersonate::client` | `Fingerprint`, `ClientHints`, `BrandVersion`, `randomize::chromium_hints` | `ImpersonationProfile`; identity headers set per request, after emulation |
| 4 | `BrowserProfile::random()` | `ImpersonationProfile::random(seed)` | `Fingerprint::randomized` and the `randomize.rs` machine and place tables | `Fingerprint::randomized_with_major` so the UA claims the measured major |
| 5 | `BrowserKind` picks the JA4/H2 source; fingerprint DB (hardware/software traits, JA4 suites) | `wire::BrowserKind`, `wire::emulation` | `randomize.rs` GPU tiers, screens, cores, memory | measured TLS/H2 entries, `for_kind` |
| 6 | `CloudScraper` builder (`with_geo_proxies`, `proxy_strategy`, `with_max_challenge_attempts`, `with_state_store`, `with_event_sink`), `solve_challenge` | `scrape::adapters::browser_scraper::BrowserScraper` (no MITM), `impersonate::StealthSession` (with MITM) | `Browser`, `Page`, `BrowserContext`, `ContextOptions::proxy`, `Page::{solve_captcha, human}`, `Lease::save_session` | builders, fetch/detect/mitigate loop, clearance wait |
| 7 | `rotate_profile()`: new fingerprint, same MITM port and egress | `BrowserScraper::rotate_profile`, `StealthSession::rotate_profile` | `Browser::launch`, `LaunchOptionsBuilder::fingerprint`, `Page::apply_fingerprint` | relaunch rules (§3.13) |
| 8 | Challenge `detect` and `MitigationPolicy` deciding an `Action` | `domain::{challenge, markers, mitigation}` | `sb_cdp::Captcha` (widget evidence), `RetryPolicy::delay_for_attempt` (backoff) | markers, `Detection`, decision table |
| 9 | Proxy pool with `RotationStrategy` | `domain::{proxy_pool, egress}` | `sb_cdp::Proxy::parse` (password-free `Debug`), `stealth::humanize::Rng` | strategies, health, quarantine |
| 10 | `CountryCode`, `Locale` table, `GeoResolver` port, locale led by the egress country and applied over CDP | `domain::geo`, `adapters::cdp::apply_locale` | `randomize.rs` `REGIONS` (moved into `LOCALES`), `Page::apply_fingerprint`, `Emulation::{locale, timezone, geolocation}` | `CountryCode`, `LOCALES`, `StaticGeoResolver` |
| 11 | `DomainState` + `StateStore` (memory; redb under `persistence`) | `domain::state`, `adapters::json_store` | `SessionStore`'s `Arc<Mutex<BTreeMap>>` shape, `sb_cdp::sync::locked`, `RateLimiter` (per-site pacing) | `JsonStateStore` (atomic file) instead of redb |
| 12 | `ScraperEvent` + `EventSink` (`Noop`, `Log`); credentials redacted before logs, events and state | `domain::{events, redact}`, `adapters::tracing_sink` | redacting `Debug` of `Proxy`, `ProxyConfig`, `Cookie`, `Session` | `RedactedUrl`, `Secret`, `TracingEventSink`, `CollectingEventSink` |
| 13 | Stealth injection JS (navigator, WebGL, Canvas, Audio) | `stealth::providers` (exists) | 29 builtin providers, `SB_PREAMBLE` native-shape helpers, `Identity::of` | `session_registry()` (§3.14) |
| 14 | Interactive solver (Turnstile click) | `sb_cdp::captcha` (exists) | `Page::solve_captcha`, `Captcha::ALL` | `adapters::cdp::wait_for_clearance` |
| 15 | Human behaviour (Bezier mouse, keystroke jitter) | exists | `humanize::{bezier_mouse_path, keystroke_delays}`, `stealth::behavior`, `sb_cdp::Human` | - |
| 16 | Dual-mode `StealthSession`/`StealthIdentity`; re-escalate on a fresh challenge or a near-expiry clearance | `impersonate::{stealth_session, http_session}`, `domain::{identity, session_policy, cookie_jar}` | unreviewed draft `.claude/worktrees/agent-adf311134dafc0ac4/src/sb_cdp/handoff.rs`: UA parsing, `Accept-Language`, body limits, cookie sync-back (its wreq-util preset table is dropped) | RFC 6265 jar, identity serde, mode state machine |
| 17 | Measurement tools (`capture_fingerprint`, `capture_h2`, `capture_hints`, `emulation_roundtrip`) | `adapters::capture`, `impersonate::capture_h2`, `examples/capture_*.rs`, `tests/impersonate_wire.rs` | `LaunchOptionsBuilder::arg` (`--host-resolver-rules`), `Page::evaluate`, `Browser::execute("Browser.getVersion")` | ClientHello and H2 frame readers, JA3/JA4/Akamai |
| 18 | Hardening: never `Runtime/DOM/Log/Debugger/Profiler.enable`; screen via `setDeviceMetricsOverride` 0x0; `userAgentData` only in secure contexts; no plugins/mimeTypes/pdfViewerEnabled/connection hooks; seeded noise; pipe transport | `stealth::{cdp, providers}`, `sb_cdp::identity`, audit tests | noise is already a pure function of (seed, index) | drop `Runtime.enable` in `src/stealth/cdp.rs:61`, hygiene and audit tests; pipe deferred (§6) |
| 19 | Long-lived browser (their idle-timeout bug) | `BrowserScraper` + test | no idle timeout anywhere (`BrowserPool` has only `acquire_timeout`) | idle-browser test, named lifetime constants |
| 20 | (ours) custom JA3 and Akamai strings | `wire::{ja3, akamai, tls_spec, h2_spec}`, `impersonate::client::CustomProfileBuilder` | - | parse, reproducibility check, wire round trip |

## 2. Layout, features, dependencies

> **Layout revised (2026-10-11).** The tree below was first written by hexagonal layer
> (`scrape/domain`, `scrape/adapters`). It is now laid out by concern. Work packages, file
> owners and the contents of each file are unchanged; only the paths move:
>
> - `src/scrape/domain/wire/**` -> `src/wire/**` (pure; `cfg(any(feature = "scrape", feature = "impersonate"))`): `ja3`, `ja4`, `akamai`, `ids`, `tls_spec`, `h2_spec`, `h2_frames`, `header_spec`, `client_hello`, `emulation`; `BrowserKind` stays in `wire/mod.rs`.
> - `src/scrape/domain/{challenge,markers,mitigation}.rs` -> `src/scrape/challenge/{detect,markers,mitigation}.rs`.
> - `src/scrape/domain/{egress,proxy_pool,geo,redact}.rs` -> `src/scrape/egress/{route,pool,geo,redact}.rs`.
> - `src/scrape/domain/{cookie_jar,set_cookie}.rs` -> `src/scrape/jar/{store,set_cookie}.rs`.
> - `src/scrape/domain/{state,events}.rs` -> `src/scrape/ledger/{state,events}.rs`.
> - `src/scrape/domain/{identity,session_policy}.rs` -> `src/scrape/passport/{identity,policy}.rs`.
> - `src/scrape/adapters/**` -> `src/scrape/runtime/**` (everything that does I/O).
> - `src/impersonate/**` stays where it is.
>
> The pure directories checked by `tests/scrape_domain_purity.rs` are `src/wire` and
> `src/scrape/{challenge,egress,jar,ledger,passport}`. Module names drop "domain", "adapters"
> and "ports"; the public type names in §3 are unchanged. The paths in §2.1 below and in §5
> read as the old names until this note is applied; the map above is the authority.

### 2.1 Files and owners (WP = work package, §5)

```text
src/lib.rs (mod lines, lint block: WP0; re-exports: WP19)   src/sb_cdp/mod.rs (`pub(crate) mod sync`): WP0
src/sb_cdp/{context,browser}.rs: WP13, only to port the worktree's ContextRoute
src/stealth/{randomize,fingerprint}.rs: WP1, additive (LOCALES source, randomized_with_major,
    ClientHints.wow64 / form_factors with serde default)
src/stealth/cdp.rs, src/stealth/providers/{mod,builtin}.rs, src/sb_cdp/identity.rs: WP17
src/scrape/  (feature "scrape")      mod.rs WP0 (re-exports WP19), error.rs WP13 (ScrapeError)
  domain/    (pure, enforced)        mod.rs WP0
    redact egress geo proxy_pool                                WP1
    challenge markers mitigation (+ tests/fixtures/challenge)   WP2
    state WP3 | cookie_jar set_cookie WP4 | identity session_policy events WP8
    wire/  mod.rs (BrowserKind) WP0 | ids WP5 (flags fixed by WP12's probe)
           client_hello ja3 ja4 tls_spec WP6 | akamai h2_frames h2_spec header_spec WP7
           emulation WP11 | measured/<browser>-<major>-<os>.json WP9 (TLS, hints), WP11 (H2, headers)
  adapters/  mod.rs WP0 | json_store WP3 | tracing_sink identity_file WP8 | capture WP9
             cdp browser_scraper egress_switch WP13
src/impersonate/ (feature "impersonate")  mod.rs WP0 (re-exports WP19)
  tls_server WP10 | capture_h2 WP11 | convert client WP12 | http_session WP14
  mitm/{mod,connect,forward,upstream} WP15 | stealth_session WP16
examples: capture_tls, capture_hints WP9 | capture_h2 WP11
tests: scrape_domain_purity WP0 | fixtures/measured/*.bin WP9, WP11 | impersonate_wire WP12
       scrape_browser WP13 | impersonate_http WP14 | impersonate_mitm WP15 | stealth_session WP16
       sb_cdp_domain_hygiene, stealth_audit_chrome, stealth_audit_network WP17
Cargo.toml, Cargo.lock, justfile, .github/workflows/ci.yml: WP0 only
docs/tutorials/stealth_scraping.md (new), fingerprint_stealth.md, ROADMAP.md: WP19
```

WP0 creates every listed source file with only a `//!` line, so later packages fill files they own
and never edit a shared `mod.rs`. Unit tests stay in-file; integration tests are grouped into the few
binaries above because each test binary links the whole crate (disk).

### 2.2 Cargo.toml (only WP0 edits it)

Reused, already present: tokio (net, io-util, fs, sync, time, macros), serde, serde_json, thiserror
2, tracing, async-trait, url 2.5, chrono, ring 0.17 (SHA-256 for JA4, `SystemRandom` for tokens),
base64 0.23 (SPKI pins), futures-util; dev: tokio `test-util`, syn 3. hyper 1.12.0, hyper-util
0.1.21, http-body-util 0.1.5, http 1.5.0 and bytes 1.12.1 are already in Cargo.lock through reqwest.

```toml
[dependencies]
psl = { version = "2.1.241", optional = true }
wreq = { version = "=6.0.0-rc.31", optional = true, default-features = false, features = [
    "tokio-rt", "webpki-roots", "gzip", "deflate", "brotli", "zstd", "socks", "stream"] }
btls = { version = "=0.5.6", optional = true }        # the BoringSSL wreq links; links = "boringssl" allows one
tokio-btls = { version = "=0.5.6", optional = true }
hyper = { version = "1.12", optional = true, default-features = false, features = ["server", "http1"] }
hyper-util = { version = "0.1.21", optional = true, default-features = false, features = ["tokio"] }
http-body-util = { version = "0.1.5", optional = true }
http = { version = "1.5", optional = true }
bytes = { version = "1.12", optional = true }

[features]
scrape = ["dep:psl"]
impersonate = ["scrape", "dep:wreq", "dep:btls", "dep:tokio-btls", "dep:hyper", "dep:hyper-util",
               "dep:http-body-util", "dep:http", "dep:bytes"]
# [[example]] capture_tls, capture_hints: required-features ["scrape"]; capture_h2: ["impersonate"]
```

Not enabled on purpose: wreq `cookies` (our RFC 6265 jar writes `Cookie`; redirects are followed by
hand), `ws`, `hickory-dns`, `system-proxy`. `zstd` pulls `zstd-sys` (C, built by `cc`), reachable only
under `impersonate`. Licenses: wreq and btls Apache-2.0, btls-sys MIT, tokio-btls MIT OR Apache-2.0,
psl MIT/Apache-2.0, all inside `deny.toml`'s allow-list (its `[graph] all-features` sees `impersonate`).

## 3. Public API

Conventions for the blocks below: public enums are `#[non_exhaustive]`; data types derive `Clone,
Debug, PartialEq` and `Serialize, Deserialize` when persisted; errors derive `thiserror::Error`; the
`Debug` of anything holding a secret is hand-written and prints `<redacted>` or counts.

### 3.1 Redaction and egress (`domain/{redact,egress}.rs`, WP1)

```rust
pub struct Secret(String);                        // Display/Debug "<redacted>", never Serialize
impl Secret { pub fn new(v: impl Into<String>) -> Self; pub fn expose(&self) -> &str; }
pub struct RedactedUrl(String);                   // Serialize; the only constructor redacts
impl RedactedUrl { pub fn new(raw: &str) -> Self; pub fn as_str(&self) -> &str; }
/// Values of these query keys become "***"; userinfo and fragment are always dropped.
pub const SENSITIVE_QUERY_KEYS: [&str; 12] = ["token", "access_token", "key", "api_key", "sig",
    "signature", "password", "pass", "auth", "session", "sid", "secret"];
pub struct EgressKey { pub scheme: String, pub host: String, pub port: u16, pub country: Option<CountryCode> }
pub struct EgressRef { proxy: sb_cdp::Proxy, key: EgressKey }   // Debug/Display = key; never Serialize
impl EgressRef {
    /// http, https, socks5, socks5h; `user:password@` leaves the string here, in the constructor.
    pub fn parse(spec: &str, country: Option<CountryCode>) -> Result<Self, EgressError>;
    pub fn key(&self) -> &EgressKey;
    pub fn proxy(&self) -> &sb_cdp::Proxy;        // ContextOptions::proxy
    pub(crate) fn launch_spec(&self) -> Secret;   // LaunchOptionsBuilder::proxy, MITM upstream
}
pub enum EgressError { Scheme(String), Address(RedactedUrl) }
```

Only `EgressKey` is ever serialized (state, identity, events); credentials never are.

### 3.2 Geo and proxy pool (`domain/{geo,proxy_pool}.rs`, WP1)

```rust
#[serde(try_from = "String", into = "String")]
pub struct CountryCode([u8; 2]);                  // ISO 3166-1 alpha-2, upper case
impl CountryCode { pub fn new(code: &str) -> Result<Self, GeoError>; pub fn as_str(&self) -> &str; }
pub struct Locale { pub tag: &'static str, pub country: &'static str, pub languages: &'static [&'static str],
                    pub timezone: &'static str, pub latitude: f64, pub longitude: f64 }
pub static LOCALES: &[Locale];                    // superset of randomize::REGIONS, which now derives from it
impl Locale {
    pub fn for_country(country: CountryCode) -> Option<&'static Locale>;  pub fn by_tag(tag: &str) -> Option<&'static Locale>;
    pub fn accept_language(&self) -> String;      // format checked against Chrome's measured header
    pub fn apply_to(&self, fp: &mut Fingerprint); } // locale, languages, accept_languages, timezone, lat/long
pub struct GeoInfo { pub country: CountryCode, pub timezone: Option<String>, pub coordinates: Option<(f64, f64)> }
#[async_trait] pub trait GeoResolver: Send + Sync {
    async fn resolve(&self, egress: &EgressRef) -> Result<GeoInfo, GeoError>; }
pub struct StaticGeoResolver;                     // country given with the proxy; no network lookup ships
pub enum GeoError { InvalidCountry(String), NoLocale(CountryCode), Unknown(EgressKey) }

pub enum RotationStrategy { RoundRobin, Random { seed: u64 }, LeastFailures, StickyPerSite }
pub enum EgressOutcome { Success, ConnectError, Challenged, Blocked }
pub struct ProxyPool { /* Mutex<PoolState>, locked through sb_cdp::sync::locked */ }
impl ProxyPool {
    pub fn new(egresses: Vec<EgressRef>, strategy: RotationStrategy) -> Self;
    pub fn with_geo_proxies<I, S>(items: I, strategy: RotationStrategy) -> Result<Self, EgressError>
        where I: IntoIterator<Item = (S, CountryCode)>, S: AsRef<str>;
    pub fn quarantine(self, after_failures: u32, for_secs: u64) -> Self;   // defaults: named consts 3, 600
    pub fn pick(&self, site: Option<&str>, now: u64) -> Option<EgressRef>;
    pub fn pick_in(&self, country: CountryCode, site: Option<&str>, now: u64) -> Option<EgressRef>;
    pub fn pick_other(&self, current: &EgressKey, site: Option<&str>, now: u64) -> Option<EgressRef>;
    pub fn report(&self, egress: &EgressKey, outcome: EgressOutcome, now: u64); }
```

`Fingerprint::randomized_with_major(os, seed, major)` is `randomized` with the Chromium major fixed: an
identity riding the measured TLS must claim the measured version, never 153 while Chrome 155 renders.

### 3.3 Challenge and mitigation (`domain/{challenge,markers,mitigation}.rs`, WP2)

```rust
pub const MAX_SCAN_BYTES: usize = 256 * 1024;
pub struct ResponseView<'a> { pub url: &'a str, pub status: u16, pub headers: &'a [(String, String)], pub body: &'a str }
pub enum Vendor { Cloudflare, DataDome, PerimeterX, Akamai, Imperva, Kasada, AwsWaf }
pub enum ChallengeKind { None, JsChallenge, Interactive, Blocked, RateLimited { retry_after_secs: Option<u64> } }
pub struct Evidence { pub vendor: Option<Vendor>, pub markers: Vec<&'static str>, pub widget: Option<sb_cdp::Captcha> }
pub struct Detection { pub kind: ChallengeKind, pub evidence: Evidence }
pub fn detect(view: &ResponseView<'_>) -> Detection;
pub fn clearance_cookie(vendor: Vendor) -> Option<&'static str>;   // cf_clearance, datadome, ...

pub enum Mode { Browser, Http }
pub struct AttemptContext { pub attempt: u32, pub mode: Mode, pub consecutive_challenges: u32,
                            pub can_rotate_egress: bool, pub can_escalate: bool, pub now: u64 }
pub enum Action { Proceed, Retry { after: Duration }, Escalate, Solve, RotateEgress, RotateProfile,
                  Cooldown { until: u64 }, GiveUp(GiveUpReason) }
pub enum GiveUpReason { AttemptsExhausted, Blocked, NoEgressLeft }
pub struct MitigationPolicy { pub max_challenge_attempts: u32, pub backoff: RetryPolicy,
    pub rotate_egress_after: u32, pub rotate_profile_after: u32, pub block_cooldown_secs: u64 }
impl Default for MitigationPolicy { /* 3, RetryPolicy::network(), 2, 3, 900, as named consts */ }
impl MitigationPolicy { pub fn decide(&self, d: &Detection, ctx: &AttemptContext) -> Action; }
```

A challenge needs interstitial markers (`just a moment`, `cf_chl_opt`,
`/cdn-cgi/challenge-platform/h/{b,g}/orchestrate`, form/stage ids, `cf-mitigated: challenge`, per-vendor
equivalents: one named const each, with the fixture that proves it). A bare `cf-ray`, `server:
cloudflare` or Turnstile widget gives `None` with the evidence attached. Decisions: `None` -> `Proceed`;
`RateLimited` -> `Cooldown` (`Retry-After`); JS/interactive in `Http` -> `Escalate`, in `Browser` ->
`Solve`, then `RotateEgress`, then `RotateProfile` as `consecutive_challenges` crosses thresholds;
`Blocked` -> `RotateEgress` else `Cooldown`; attempts exhausted -> `GiveUp`.

### 3.4 State (`domain/state.rs`, `adapters/json_store.rs`, WP3)

```rust
pub struct DomainState { pub site: String, pub successes: u64, pub challenges: u64, pub blocks: u64,
    pub consecutive_challenges: u32, pub last_challenge: Option<ChallengeKind>, pub last_seen: u64,
    pub cooldown_until: Option<u64>, pub preferred_mode: Option<Mode>, pub last_egress: Option<EgressKey> }
impl DomainState { pub fn record(&mut self, d: &Detection, mode: Mode, now: u64); pub fn in_cooldown(&self, now: u64) -> bool; }
#[async_trait] pub trait StateStore: Send + Sync {
    async fn get(&self, site: &str) -> Result<Option<DomainState>, StateError>;
    async fn put(&self, state: DomainState) -> Result<(), StateError>;
    async fn remove(&self, site: &str) -> Result<(), StateError>;
    async fn sites(&self) -> Result<Vec<String>, StateError>; }
pub struct InMemoryStateStore;                    // Arc<Mutex<BTreeMap<String, DomainState>>>
pub struct JsonStateStore;                        // one file { "schema": 1, "sites": { ... } }
impl JsonStateStore { pub fn open(path: impl Into<PathBuf>) -> Result<Self, StateError>; }
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()>;
pub enum StateError { Io { path: PathBuf, source: io::Error }, Corrupt { path: PathBuf, source: serde_json::Error }, Schema(u32) }
```

`atomic_write`: sibling temp file opened `0o600` (Unix `OpenOptionsExt::mode`), `write_all`,
`sync_all`, `rename`, then `sync_all` on the parent directory, inside `spawn_blocking`. A missing file
is an empty store; a corrupt one is `StateError::Corrupt`, never silently reset. One writer process
per file (documented; no cross-process lock).

### 3.5 Cookies, RFC 6265 with `psl` (`domain/{cookie_jar,set_cookie}.rs`, WP4)

```rust
pub struct StoredCookie { pub name: String, pub value: String, pub domain: String, pub host_only: bool,
    pub path: String, pub expires: Option<u64>, pub secure: bool, pub http_only: bool,
    pub same_site: Option<sb_cdp::SameSite>, pub created: u64, pub last_access: u64 }
pub struct CookieJar { /* Vec<StoredCookie>; serde; Debug prints counts only */ }
pub enum SiteContext { SameSite { top_level: bool }, CrossSite { top_level_get: bool } }
pub enum StoreOutcome { Stored, Replaced, Deleted, Ignored(IgnoreReason) }
pub enum IgnoreReason { Malformed, DomainMismatch, PublicSuffix, IpLiteralDomain, SecureFromInsecure,
                        HostPrefix, SecurePrefix, TooLarge }
impl CookieJar {
    pub fn store(&mut self, request_url: &Url, set_cookie: &str, now: u64) -> StoreOutcome;          // §5.2, §5.3
    pub fn header_for(&mut self, request_url: &Url, ctx: SiteContext, now: u64) -> Option<String>; // §5.4
    pub fn from_cdp(cookies: &[sb_cdp::Cookie], now: u64) -> Self;  pub fn to_cdp(&self, now: u64) -> Vec<sb_cdp::Cookie>;
    pub fn expiry_of(&self, name: &str, site: &str) -> Option<u64>;  pub fn remove_expired(&mut self, now: u64) -> usize; }
pub fn site_of(host: &str) -> Option<String>;     // registrable domain via psl; None for IPs and bare suffixes
```

Rules, each a named test: §5.2 attributes; §5.1.1 dates (two-digit years, any token order); `Max-Age`
beats `Expires`, `<= 0` deletes, expiry capped at 400 days (6265bis). `Domain` is lower-cased, loses a
leading dot, must domain-match the host (§5.1.3). Public suffix (§5.3 step 5): ignored unless it equals
the host, which then gets a host-only cookie. IP hosts: `Domain` must equal the IP, cookie host-only, no
suffix matching. Paths per §5.1.4. 6265bis as Chrome applies it: an insecure origin cannot set or
overwrite a `Secure` cookie; `__Secure-` needs `Secure`; `__Host-` needs `Secure`, `Path=/`, no
`Domain`. `SameSite` per `SiteContext`; order per §5.4; bounds (named consts) 4096 bytes, 180 per site,
3000 total, least-recently-accessed eviction. CDP: a leading dot in `sb_cdp::Cookie::domain` means a
domain cookie; `expires: None` is a session cookie.

### 3.6 Identity, session policy, events (`domain/{identity,session_policy,events}.rs`, WP8)

```rust
pub const IDENTITY_SCHEMA: u32 = 1;
pub struct EmulationRef { pub kind: BrowserKind, pub major: u32, pub os: OsType }
pub struct Clearance { pub site: String, pub vendor: Vendor, pub cookie: String, pub expires_at: u64 }
pub struct StealthIdentity {                      // serde; Debug: cookie count and egress key only
    pub schema: u32, pub fingerprint: Fingerprint, // UA, platform, HW, WebGL, screen, locale, tz, client_hints, seed
    pub emulation: EmulationRef, pub cookies: CookieJar,
    pub accept_ch: BTreeMap<String, Vec<String>>, // origin -> hints it asked for (Accept-CH)
    pub egress: Option<EgressKey>,                // sticky: the IP that earned the clearance
    pub clearances: Vec<Clearance>, pub created_at: u64 }
impl StealthIdentity {
    /// Refuses an incoherent fingerprint (Fingerprint::validate); clears fingerprint.proxy (may hold a password).
    pub fn new(fp: Fingerprint, emulation: EmulationRef, egress: Option<EgressKey>, now: u64) -> Result<Self, IdentityError>;
    pub fn refresh_clearances(&mut self, now: u64);   // from the jar + challenge::clearance_cookie
    pub fn clearance_for(&self, site: &str) -> Option<&Clearance>;
    pub fn check_egress(&self, egress: Option<&EgressKey>) -> Result<(), IdentityError>; }
pub enum IdentityError { EgressMismatch { expected: Option<EgressKey>, found: Option<EgressKey> }, Schema(u32),
                         Incoherent(Vec<String>), EmulationMismatch { identity: u32, measured: u32 } }

pub struct SessionPolicy { pub require_clearance_for_http: bool, pub clearance_margin: Duration,
                           pub demote_after_solve: bool, pub prefer: Mode }   // false, 120 s, true, Http
pub enum Transition { Stay, Escalate(EscalateReason), Demote(DemoteReason) }
pub enum EscalateReason { Challenge(ChallengeKind), ClearanceExpiring { remaining_secs: u64 }, NoClearance, SiteNeedsBrowser, Manual }
pub enum DemoteReason { ChallengeSolved, Manual }
impl SessionPolicy {
    pub fn before_fetch(&self, mode: Mode, id: &StealthIdentity, site: &str, st: Option<&DomainState>, now: u64) -> Transition;
    pub fn after_fetch(&self, mode: Mode, d: &Detection, a: &Action, id: &StealthIdentity) -> Transition; }
pub enum ScraperEvent {                           // Serialize
    Request { url: RedactedUrl, status: u16, mode: Mode, elapsed_ms: u64 },
    ChallengeDetected { url: RedactedUrl, kind: ChallengeKind, vendor: Option<Vendor>, mode: Mode },
    ChallengeSolved { url: RedactedUrl, kind: ChallengeKind, attempts: u32, elapsed_ms: u64 },
    ActionChosen { site: String, action: Action }, ModeChanged { from: Mode, to: Mode, reason: Transition },
    EgressRotated { from: Option<EgressKey>, to: Option<EgressKey> }, ProfileRotated { from_seed: u64, to_seed: u64 },
    ClearanceExpiring { site: String, remaining_secs: u64 }, CooldownStarted { site: String, until: u64 },
    IdentityExported { cookies: usize }, IdentityRestored { cookies: usize } }
pub trait EventSink: Send + Sync { fn emit(&self, event: &ScraperEvent); }   // must not block
pub struct NoopEventSink;  pub struct CollectingEventSink;   // adapters: TracingEventSink
```

URLs enter events only as `RedactedUrl` and proxies only as `EgressKey`, so a secret cannot reach an
event by construction. `adapters/identity_file.rs` saves through `atomic_write` (0600: the file holds
cookies) and loads with a schema check.

### 3.7 Wire parsers and specs (`domain/wire/*`, WP5 to WP7)

```rust
pub enum BrowserKind { Chrome }                   // WP11 adds Safari once its capture exists
pub const MAX_CLIENT_HELLO: usize = 64 * 1024;  pub fn is_grease(id: u16) -> bool;  // 0x?a?a, equal bytes
pub struct ClientHello { pub legacy_version: u16, pub cipher_suites: Vec<u16>, pub extensions: Vec<Extension>,
    pub sni: Option<String>, pub alpn: Vec<Vec<u8>>, /* + supported_versions, supported_groups, key_share_groups,
    point_formats, signature_algorithms, psk_modes, cert_compression, alps, record_size_limit, delegated_credentials */ }
impl ClientHello { pub fn needed_len(prefix: &[u8]) -> Result<Option<usize>, ClientHelloError>;  // incremental
                   pub fn parse(records: &[u8]) -> Result<Self, ClientHelloError>; }           // across records
pub enum ClientHelloError { NotHandshake, NotClientHello(u8), Truncated { need: usize, have: usize }, Length, TooLarge }
pub struct Ja3 { pub version: u16, pub ciphers: Vec<u16>, pub extensions: Vec<u16>, pub groups: Vec<u16>, pub point_formats: Vec<u8> }
impl Ja3 { pub fn parse(s: &str) -> Result<Self, Ja3Error>;   // strips GREASE ids given in the input
           pub fn of(hello: &ClientHello) -> Self; }          // Display: "771,4865-4866-...,0-23-...,29-23-24,0"
pub enum Ja3Error { FieldCount(usize), Number { field: Ja3Field, token: String }, Empty(Ja3Field), TooLong }
pub struct Ja4 { pub a: String, pub b: String, pub c: String }   // Display "a_b_c"
impl Ja4 { pub fn of(hello: &ClientHello) -> Self; pub fn raw(hello: &ClientHello) -> String; // JA4_r
           pub fn diff(&self, other: &Ja4) -> Ja4Diff; }
pub struct TlsSpec { pub legacy_version: u16, pub cipher_suites: Vec<u16>, pub extensions: Vec<u16>,
    pub permute_extensions: bool, pub grease: bool, pub supported_groups: Vec<u16>, pub key_share_groups: Vec<u16>,
    pub point_formats: Vec<u8>, pub signature_algorithms: Vec<u16>, pub alpn: Vec<String>, pub alps: Option<Alps>,
    /* + delegated_credentials, supported_versions, psk_modes, cert_compression, record_size_limit,
       ech_grease, ocsp_stapling, sct, session_ticket, renegotiation */ }
impl TlsSpec {
    pub fn from_client_hello(hello: &ClientHello) -> Self;          // what the capture tool records
    pub fn with_ja3(&self, ja3: &Ja3) -> Result<Self, SpecError>;   // overlay; turns permutation off
    pub fn with_signature_algorithms(&self, ids: &[u16]) -> Result<Self, SpecError>;
    pub fn check_reproducible(&self) -> Result<(), SpecError>;      // against ids::BORINGSSL_* sets
    pub fn without_known_gaps(&self, gaps: &[KnownGap]) -> Self;
    pub fn ja3(&self) -> Ja3; }
pub enum Pseudo { Method, Authority, Scheme, Path }
pub struct PriorityFrame { pub stream_id: u32, pub exclusive: bool, pub depends_on: u32, pub weight: u16 }
pub struct Akamai { pub settings: Vec<(u16, u32)>, pub window_update: u32, pub priority_frames: Vec<PriorityFrame>,
                    pub pseudo_order: [Pseudo; 4] }       // Display: "1:65536;2:0;...|15663105|0|m,a,s,p"
impl Akamai { pub fn parse(s: &str) -> Result<Self, AkamaiError>; }
pub enum AkamaiError { Sections(usize), Setting { index: usize, token: String }, DuplicateSetting(u16),
                       WindowUpdate(String), PriorityFrame { index: usize, token: String }, PseudoOrder(String) }
pub struct HeadersPriority { pub depends_on: u32, pub weight: u16, pub exclusive: bool }
pub fn read_preface(bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError>;  // None = need more bytes
pub struct Http2Spec { pub settings: Vec<(u16, u32)>, pub window_update: u32, pub priority_frames: Vec<PriorityFrame>,
    pub headers_priority: Option<HeadersPriority>, pub pseudo_order: [Pseudo; 4], pub initial_stream_id: Option<u32> }
impl Http2Spec { pub fn with_akamai(&self, a: &Akamai) -> Result<Self, SpecError>;
                 pub fn check_reproducible(&self) -> Result<(), SpecError>; pub fn akamai(&self) -> Akamai; }
pub enum RequestShape { Document, Fetch }
pub enum HeaderValue { Literal(String), UserAgent, SecChUa, SecChUaMobile, SecChUaPlatform, AcceptLanguage, Cookie }
pub struct HeaderSpec { pub document: Vec<(String, HeaderValue)>, pub fetch: Vec<(String, HeaderValue)>,
                        pub priority_by_dest: Vec<(String, String)> }   // h2-only `priority` per sec-fetch-dest
```

JA4 per FoxIO's TLS-client definition (BSD-3-Clause): `a` = `t` + highest non-GREASE version + `d`/`i`
+ two-digit cipher and extension counts + first and last ALPN character; `b`/`c` = first 12 hex of
SHA-256 (`ring::digest`) over sorted ciphers / sorted extensions minus SNI and ALPN, `_`, sigalgs in
wire order. JA3 is compared as a string (no MD5 dependency). Pseudo order comes from HPACK static-table
name indices (1-7), so no Huffman decoder; header names and values are measured over plaintext h1.

### 3.8 What BoringSSL and wreq cannot reproduce

`ids.rs` holds the IANA names and the capability sets `BORINGSSL_CIPHERS`, `BORINGSSL_CLIENT_EXTENSIONS`,
`BORINGSSL_GROUPS`, `BORINGSSL_SIGALGS`. WP5 seeds them from btls 0.5.6's constants (`ExtensionType`,
`KeyShare`, `SslSignatureAlgorithm`); WP12's probe test builds a client per id, captures the ClientHello
and fails if a set claims an id that does not appear on the wire. The probe, not this table, decides.

| JA3 field | Accepted | Rejected, with error |
|---|---|---|
| version | `771` (0x0303) | anything else: `SpecError::LegacyVersion` |
| ciphers | ids in `BORINGSSL_CIPHERS`; the TLS 1.3 trio in any order (`preserve_tls13_cipher_list`) | DHE (0x0033, 0x0039, 0x009e, 0x009f), CCM (0x1304, 0x1305), SCSVs (0x00ff, which BoringSSL replaces by ext 0xff01; 0x5600), NULL/EXPORT/RC4/anon, any id the probe does not confirm (CBC-SHA256/384 included): `UnsupportedCipher`; a TLS 1.3 subset: `Tls13Suites` |
| extensions | ids a `TlsOptions` knob turns on: 0, 5, 10, 11, 13, 16, 18, 23, 27, 28, 34, 35, 43, 45, 51, 17513, 17613, 65037, 65281; 21 (padding) only where BoringSSL's length rule puts it, checked by round trip | 41 pre_shared_key (resumption only), 42 early_data, 49 post_handshake_auth, 50 signature_algorithms_cert, 22 encrypt_then_mac, 15 heartbeat, 0xca34 trust_anchors (needs btls > 0.5.6 and wreq > rc.31: btls#168, wreq#1273), unknown ids: `UnsupportedExtension { id, reason }`; repeats: `DuplicateExtension` |
| groups | 29 x25519, 23 P-256, 24 P-384, 25 P-521, 4588 X25519MLKEM768, 25497 X25519Kyber768Draft00; MLKEM1024, P256Kyber768, ffdhe2048/3072 if the probe confirms them | 30 x448, brainpool, secp256k1, ffdhe6144/8192, others: `UnsupportedGroup` |
| point formats | `0`, or empty when ext 11 is absent | anything else: `PointFormats` |
| not in JA3 | signature algorithms, ALPN, key shares, cert compression, versions, PSK modes come from the base spec or the builder | ML-DSA 0x0904, 0x0905, 0x0906 and any id outside `BORINGSSL_SIGALGS`: `UnsupportedSigalg { id, name }` |

A custom JA3 turns extension permutation off, so its JA3 on the wire is exact. Measured Chrome keeps
permutation on, as Chrome does: its JA3 changes per connection by design, and JA4 is the target.

| Akamai part | Accepted | Rejected, with error |
|---|---|---|
| SETTINGS `id:value;...` | ids 1, 2, 3, 4, 5, 6, 8, 9, each once, any order | malformed: `AkamaiError::Setting`; a repeated id: `AkamaiError::DuplicateSetting` (http2's `SettingsOrderBuilder` would drop it silently); any other id (GREASE, experimental): `SpecError::UnsupportedSetting` (wreq-proto 0.2.6 `Http2Options` exposes no experimental settings); values against RFC 9113 6.5.2 (push or connect-protocol not 0/1, window above 2^31-1, frame size outside 2^14..2^24-1): `SettingValue`; a setting the string omits but wreq always sends (the probe tells): `SettingAlwaysSent` |
| WINDOW_UPDATE | `0` (none) up to 2^31-1-65535 | `SpecError::WindowUpdate` |
| PRIORITY `s:e:d:w,...` or `0` | odd stream ids, `e` 0/1, weight 1..=256, `d != s`; sets `initial_stream_id` above the largest | `SpecError::Priority { stream, reason }` |
| pseudo order | a permutation of `m,a,s,p` | `AkamaiError::PseudoOrder` (`PseudoOrderBuilder::build` would pad a partial list) |
| not in the string | HEADERS priority flags, header order, `Accept-Encoding` | taken from the base `Http2Spec`/`HeaderSpec` |

```rust
pub enum SpecError { LegacyVersion(u16), UnsupportedCipher(u16), Tls13Suites,
    UnsupportedExtension { id: u16, reason: &'static str }, DuplicateExtension(u16), UnsupportedGroup(u16),
    PointFormats(Vec<u8>), UnsupportedSigalg { id: u16, name: &'static str }, UnsupportedSetting(u16),
    SettingValue { id: u16, value: u32 }, SettingAlwaysSent(u16), WindowUpdate(u32),
    Priority { stream: u32, reason: &'static str }, NotMeasured { kind: BrowserKind, os: OsType } }
```

### 3.9 Measured tables (`domain/wire/emulation.rs`, WP11)

```rust
pub enum KnownGap { TrustAnchorsExtension, MlDsaSignatureAlgorithms }
pub struct Provenance { pub captured_on: String, pub tool: String, pub browser: String, pub known_gaps: Vec<KnownGap> }
pub struct Emulation { pub kind: BrowserKind, pub major: u32, pub full_version: String, pub os: OsType,
    pub user_agent: String, pub tls: TlsSpec, pub tls_websocket: Option<TlsSpec>, pub http2: Option<Http2Spec>,
    pub headers: HeaderSpec, pub client_hints: Option<ClientHints>, pub provenance: Provenance }
pub fn measured() -> &'static [Emulation];        // include_str!("measured/*.json"), parsed once (OnceLock)
pub fn for_kind(kind: BrowserKind) -> &'static Emulation;   // every variant has an entry (test-enforced)
pub fn get(kind: BrowserKind, os: OsType) -> Result<&'static Emulation, SpecError>;
```

Only the local Mac is measured: Chrome on macOS by code through sb_cdp, Safari on macOS when the user
opens the capture URL in it. Windows and Linux entries appear only when someone runs the capture tools
there (`capture_hints` also prints a console snippet for machines without Rust); until then `get`
returns `NotMeasured` and `ImpersonationProfile::random` picks measured OSes only. Client hints reuse
`ClientHints`: brand order and GREASE brand checked against `randomize::chromium_hints(major)`, the
real full version, the per-OS `platformVersion`.

### 3.10 `impersonation_client` (`impersonate/{convert,client}.rs`, WP12)

```rust
pub struct ImpersonationProfile { /* emulation; effective TlsSpec, Http2Spec, HeaderSpec; UA, hints, Accept-Language */ }
impl ImpersonationProfile {
    pub fn measured(kind: BrowserKind) -> Result<Self, ImpersonateError>;
    pub fn random(seed: u64) -> Result<Self, ImpersonateError>;     // randomized_with_major
    pub fn from_fingerprint(fp: &Fingerprint) -> Result<Self, ImpersonateError>;  // UA major must equal measured
    pub fn with_locale(self, locale: &Locale) -> Self;  pub fn custom() -> CustomProfileBuilder; }
impl CustomProfileBuilder {
    pub fn base(self, kind: BrowserKind) -> Self;                  // fills what JA3/Akamai do not carry
    pub fn ja3(self, ja3: &str) -> Result<Self, ImpersonateError>; // Ja3Error or SpecError
    pub fn akamai(self, akamai: &str) -> Result<Self, ImpersonateError>;
    pub fn signature_algorithms(self, ids: &[u16]) -> Result<Self, ImpersonateError>;
    pub fn user_agent(self, ua: impl Into<String>) -> Self;  pub fn build(self) -> Result<ImpersonationProfile, ImpersonateError>; }
pub fn impersonation_client(profile: &ImpersonationProfile) -> Result<wreq::ClientBuilder, ImpersonateError>;
pub enum ImpersonateError { Ja3(Ja3Error), Akamai(AkamaiError), Spec(SpecError),
                            VersionMismatch { browser: u32, measured: u32 }, Build(String) }
```

`convert.rs` alone names wreq types: `TlsSpec` -> `wreq::tls::TlsOptions` (`cipher_list`,
`curves_list`, `sigalgs_list`, `extension_permutation`, `permute_extensions`, `grease_enabled`,
`key_shares`, `alpn_protocols`, `alps_*`, `certificate_compressors`, `record_size_limit`,
`enable_ech_grease`, `enable_ocsp_stapling`, `enable_signed_cert_timestamps`,
`preserve_tls13_cipher_list`); `Http2Spec` -> `wreq::http2::Http2Options` (`settings_order`,
`headers_pseudo_order`, `headers_stream_dependency`, `priorities`, `initial_stream_id`, windows);
`HeaderSpec` -> `OrigHeaderMap`. UA, `Sec-CH-UA*`, `Accept-Language` and `Cookie` are set per request
after emulation. Verification stays on; tests trust a root via `tls_cert_store`, never
`tls_verify_hostname(false)`.

### 3.11 `HttpSession` (`impersonate/http_session.rs`, WP14)

```rust
pub struct HttpSession { /* Arc<wreq::Client>, profile, Mutex<CookieJar>, Option<EgressRef>, limits, sink */ }
impl HttpSession {
    pub async fn from_page(page: &Page, egress: Option<EgressRef>, opts: HttpOptions) -> Result<Self, HttpError>;
    pub fn from_identity(id: &StealthIdentity, egress: Option<EgressRef>, opts: HttpOptions) -> Result<Self, HttpError>;
    pub async fn get(&self, url: &str) -> Result<HttpReply, HttpError>;
    pub fn request(&self, method: Method, url: &str) -> HttpRequest<'_>;   // .shape() .header() .json() .body()
    pub async fn sync_cookies_back(&self, page: &Page) -> Result<usize, HttpError>;  // CookieJar::to_cdp -> Cookies::set
    pub fn export_identity(&self, base: &StealthIdentity) -> StealthIdentity; }
impl HttpReply { pub fn status(&self) -> u16; pub fn text(&self) -> Result<&str, HttpError>;
                 pub fn json<T: DeserializeOwned>(&self) -> Result<T, HttpError>; pub fn detect(&self) -> Detection; }
pub enum HttpError { Impersonate(ImpersonateError), VersionMismatch { browser: u32, measured: u32 },
    ReservedHeader(String), TooManyRedirects(usize), BodyTooLarge(usize),
    Request { url: RedactedUrl, kind: &'static str }, Browser(SeleniumBaseError) }
```

Redirects by hand (`redirect::Policy::none()`, 10 hops, `Set-Cookie` stored per hop, 303 to GET,
`Authorization` dropped across origins). Body cap 64 MiB unless raised. wreq errors are wrapped after
`without_url()` and their `Display` is never logged. A caller-set UA or `sec-ch-ua*` header is refused
(`ReservedHeader`): the profile owns them.

### 3.12 MITM proxy (`impersonate/{tls_server.rs, mitm/*}`, WP10 and WP15)

```rust
pub struct EphemeralCa { /* btls PKey + X509, memory only */ }
impl EphemeralCa {
    pub fn generate() -> Result<Self, TlsServerError>;
    pub fn spki_pins(&self) -> [String; 2];        // base64(SHA-256(SPKI)) of the CA key and the shared leaf key
    pub fn leaf_spki_pin(&self) -> &str;           // the leaf pin alone: the only pin Chrome gets
    pub fn chrome_spki_arg(&self) -> String;       // "--ignore-certificate-errors-spki-list=<leaf pin>"
    pub fn leaf_for(&self, host: &url::Host<String>) -> Result<Arc<Leaf>, TlsServerError>; }  // LRU of 256
pub fn acceptor(ca: &EphemeralCa, alpn: &[&[u8]]) -> Result<btls::ssl::SslAcceptor, TlsServerError>;
#[doc(alias = "TlsSpoofingProxy")] pub struct MitmProxy { /* Arc<Inner>; Clone */ }
impl MitmProxy {
    pub async fn start(profile: ImpersonationProfile, egress: Option<EgressRef>, opts: MitmOptions,
                       sink: Arc<dyn EventSink>) -> Result<Self, MitmError>;
    pub fn port(&self) -> u16;  pub async fn shutdown(self, grace: Duration);
    pub fn browser_proxy(&self) -> sb_cdp::Proxy;  // http://sb:<token>@127.0.0.1:<port>; password-free Debug
    pub fn chrome_args(&self) -> Vec<String>;      // [EphemeralCa::chrome_spki_arg()]: the leaf pin only
    pub fn swap_upstream(&self, egress: Option<EgressRef>) -> Result<(), MitmError>;
    pub fn swap_profile(&self, profile: ImpersonationProfile) -> Result<(), MitmError>; }
pub enum MitmError { Bind(io::Error), Tls(TlsServerError), BadTarget(String),
                     SniMismatch { connect: String, sni: String }, Upstream(ImpersonateError) }
```

Security rules, each with a WP15 test:
- CA: P-256, critical `CA:TRUE, pathlen:0`, `keyCertSign|cRLSign`, random serial, per process, never on
  disk. Leaves: one shared per-process key, SAN exactly the CONNECT host (DNS or IP via
  `url::Host::parse`, no wildcard, max 253 chars), EKU `serverAuth`, short validity. The server
  presents the leaf alone (no CA certificate in the chain).
- Chrome gets `--ignore-certificate-errors-spki-list=<leaf pin>` (honoured only with `--user-data-dir`,
  which `browser_args` always passes), never `--ignore-certificate-errors`, and never the CA pin.
  What the switch trusts, read from Chromium's `services/network/ignore_errors_cert_verifier.cc`
  (not measured on Chrome 155): Chrome hashes the SPKI of every certificate in the presented chain and,
  when one hash is in the list, accepts the connection with no other check (no signature, host name,
  validity or revocation). So it trusts any chain that contains a certificate with the pinned public
  key, from any sender; the key is public (every handshake carries it), so "this CA and its leaves"
  would be a wrong description. Narrowing it is done by pinning only the leaf key and sending the leaf
  alone, so the CA key and an intermediate are no second way in. The rest is WP15's duty: every
  connection of that Chrome profile must end at the MITM (`--proxy-server` with
  `--proxy-bypass-list=<-loopback>` so loopback hosts are intercepted too, `--disable-quic`), a CONNECT
  the MITM cannot intercept is refused rather than tunnelled blind, and no leaf certificate is logged
  or written into a capture artifact. Its real-Chrome test: a direct server on loopback that presents
  its own self-signed certificate is never served to the page with that certificate (the request goes
  through the MITM, whose upstream verification refuses it).
- Binds `127.0.0.1:0` only. A random 128-bit token (`ring::rand::SystemRandom`) is required as
  `Proxy-Authorization: Basic` on every CONNECT and absolute-form request (constant-time compare, else
  407); Chrome gets it through `ProxyAuth`, so it is never on Chrome's command line.
- SNI must equal the CONNECT host or the connection closes. The intercepted leg offers ALPN `http/1.1`.
- Stripped before replay: `Proxy-Authorization`, `Proxy-Connection`, `Connection` and its listed tokens,
  `Keep-Alive`, `TE` (but `trailers`), `Trailer`, `Transfer-Encoding`, `Upgrade` (but WebSocket).
  Chrome's header order is kept; the h2-only `priority` header comes from `priority_by_dest`.
- Upstream verification always on (webpki roots); failure answers an empty `502`. Automatic
  decompression is off in the MITM client so `Content-Encoding` passes through.
- Bodies stream both ways, never buffered whole; bounds: 256 connections, 64 KiB request head, named
  idle and connect timeouts. WebSocket upgrades go upstream over h1 with the measured `tls_websocket`
  spec (ALPN `http/1.1`, as Chrome sends), then `copy_bidirectional`.
- `swap_upstream` builds the new client first, then swaps an `RwLock<Arc<Upstream>>`; in-flight requests
  keep the old one. Logs carry `RedactedUrl`/`EgressKey` only; `Drop` uses `shutdown::spawn_cleanup`.

### 3.13 `BrowserScraper` and `StealthSession` (WP13, WP16)

```rust
// scrape::adapters, feature "scrape": CloudScraper parity without TLS spoofing
impl BrowserScraper {                             // Browser, Option<BrowserContext>, Page, pool, store, sink, policy
    pub fn builder() -> BrowserScraperBuilder;  pub fn page(&self) -> &Page;
    pub async fn fetch(&mut self, url: &str) -> Result<BrowserFetch, ScrapeError>;
    pub async fn solve_challenge(&mut self) -> Result<Detection, ScrapeError>;
    pub async fn rotate_profile(&mut self) -> Result<(), ScrapeError>;  pub async fn rotate_egress(&mut self) -> Result<(), ScrapeError>;
    pub async fn export_identity(&self) -> Result<StealthIdentity, ScrapeError>;
    pub async fn import_identity(&mut self, id: &StealthIdentity) -> Result<(), ScrapeError>;
    pub async fn close(self) -> Result<(), ScrapeError>; }
impl BrowserScraperBuilder {                      // each returns Self unless noted
    pub fn launch_options(self, b: LaunchOptionsBuilder) -> Self;
    pub fn with_geo_proxies<I, S>(self, p: I) -> Result<Self, ScrapeError> where I: IntoIterator<Item = (S, CountryCode)>, S: AsRef<str>;
    pub fn proxy_strategy(self, s: RotationStrategy) -> Self;   pub fn with_max_challenge_attempts(self, n: u32) -> Self;
    pub fn with_state_store(self, s: Arc<dyn StateStore>) -> Self; pub fn with_event_sink(self, e: Arc<dyn EventSink>) -> Self;
    pub fn geo_resolver(self, g: Arc<dyn GeoResolver>) -> Self; pub fn per_site_rate(self, calls: usize, window: Duration) -> Self;
    pub fn seed(self, seed: u64) -> Self;  pub fn egress_switch(self, s: Box<dyn EgressSwitch>) -> Self;
    pub async fn build(self) -> Result<BrowserScraper, ScrapeError>; }
#[async_trait] pub trait EgressSwitch: Send + Sync {   // port; ContextProxySwitch = new BrowserContext, no relaunch
    async fn switch(&self, browser: &Browser, egress: Option<&EgressRef>) -> Result<Page, ScrapeError>; }
// adapters::cdp: goto_with_response (Network.responseReceived, main frame), detect_page, apply_locale,
// export_cookies (Cookies::all), import_cookies (Cookies::set), wait_for_clearance (poll_until)
pub enum ScrapeError { Egress(EgressError), Geo(GeoError), State(StateError), Identity(IdentityError),
    Unsolved { url: RedactedUrl, kind: ChallengeKind, attempts: u32 }, GaveUp { url: RedactedUrl, reason: GiveUpReason },
    Browser(SeleniumBaseError) }       // StealthError (WP16) = ScrapeError | HttpError | MitmError | ImpersonateError

// impersonate, feature "impersonate": dual mode
impl StealthSession {
    pub fn builder() -> StealthSessionBuilder;    // the BrowserScraperBuilder options + .policy() .profile() .mitm() .restore(id)
    pub async fn fetch(&mut self, url: &str) -> Result<Fetched, StealthError>;
    pub async fn escalate(&mut self) -> Result<(), StealthError>;  pub async fn demote(&mut self) -> Result<(), StealthError>;
    pub async fn rotate_profile(&mut self) -> Result<(), StealthError>;  pub async fn rotate_egress(&mut self) -> Result<(), StealthError>;
    pub fn mode(&self) -> Mode;  pub fn identity(&self) -> &StealthIdentity;  pub async fn close(self) -> Result<(), StealthError>; }
```

`fetch`: (1) `site_of`, `DomainState`, per-site `RateLimiter::acquire`. (2) `before_fetch`; escalate =
launch Chrome with `fingerprint`, `browser_proxy()`, `chrome_args()`, then `import_identity`; demote =
`export_identity`, `HttpSession::from_identity` on the same egress, close Chrome. (3) Fetch, `detect`.
(4) `decide`, then apply `Solve`, `Escalate`, `RotateEgress` (`pick_other`, `swap_upstream`, new
`identity.egress`, clearances dropped: they belong to the old IP), `RotateProfile`, `Cooldown`, `Retry`
or `GiveUp`, bounded by `max_challenge_attempts`. (5) Record state; after a solve, demote if
`demote_after_solve`. Every transition emits `ModeChanged`. Both transports share one egress; `restore`
refuses an identity with another (`EgressMismatch`). `rotate_profile`: new seed,
`randomized_with_major`, Chrome relaunched behind the same MITM port and egress, cookies and clearances
dropped, `DomainState` kept. A launched browser lives until we close it: no idle timeout, never
`Instant + Duration::MAX`, lifetimes are named constants.

### 3.14 Hardening hooks (WP17)

`stealth::providers::session_registry(fp: &Fingerprint) -> EvasionRegistry`: the default registry
without `PluginsProvider` and `ConnectionProvider`; a test checks the assembled script never names
`plugins`, `mimeTypes`, `pdfViewerEnabled` or `connection`. `ClientHintsProvider` returns early when
`!isSecureContext`. `sb_cdp::identity` sends the screen through `Emulation.setDeviceMetricsOverride
{ width: 0, height: 0, screenWidth, screenHeight, ... }`. `stealth::cdp::enable_default_domains` stops
sending `Runtime.enable`.

## 4. Tests and gates

### 4.1 Wire proofs

- TLS: `TlsCapture` binds `127.0.0.1:0`, reads until `ClientHello::needed_len` (bounded size and
  timeout), recomputes JA3 (GREASE removed) and JA4. Clients dial `https://capture.test:<port>` via
  wreq `ClientBuilder::resolve` or Chrome `--host-resolver-rules=MAP capture.test 127.0.0.1:<port>`, so
  SNI is real.
- HTTP/2, our client: `H2cCapture` with `http2_only()` (prior knowledge, plain TCP) reads the preface,
  SETTINGS, WINDOW_UPDATE, PRIORITY and first HEADERS, answers SETTINGS + ACK + `:status 200` (HPACK
  `0x88`), and computes the Akamai string, HEADERS priority and pseudo order.
- HTTP/2, real browser: `capture_h2` does the same behind `tls_server::acceptor` (ALPN `h2`); `--h1`
  (ALPN `http/1.1`) records header names, values and order in plaintext.
- Oracle (ignored, when `tshark` >= 4.2 is on PATH): captured bytes wrapped in a pcap; tshark's
  `tls.handshake.ja3_full` and `tls.handshake.ja4` must equal ours.

### 4.2 Per module

| Module | Pure unit tests (in file) | Wire / integration | `#[ignore]` real Chrome or network |
|---|---|---|---|
| redact, egress | userinfo and sensitive query values never survive `Debug`, `Display`, `Serialize` (seeded loop of random secrets, `humanize::Rng`) | - | - |
| geo, proxy_pool | `CountryCode` validation; every `LOCALES` entry parses; `REGIONS` derived from it; strategies deterministic with injected `now`; quarantine and release; sticky per site | - | `accept_language()` equals Chrome's header for several `--lang` values |
| challenge, mitigation | fixture table (kind + vendor per fixture); `cf-ray`-only and bare-Turnstile negatives give `None` with evidence; `Retry-After`; scan cap; the whole decision table | - | known-challenge pages confirm the markers |
| state, json_store | round trip; missing = empty; corrupt = `Corrupt`; leftover temp file ignored; `0600` on Unix | - | - |
| cookie_jar | §5.1.1 dates, §5.1.3 domain-match, §5.1.4 paths, §5.2 attributes, §5.3 public suffix (`co.uk`, `github.io`), IP-literal rule, host-only, prefixes, Secure overwrite, `Max-Age` precedence, 400-day cap, §5.4 order, bounds, CDP round trip | - | the same `Set-Cookie` table served to Chrome: `Network.getAllCookies` must match our jar |
| identity, session_policy, events | serde round trip; `fingerprint.proxy` cleared; serialized identity and events never contain a test secret; `check_egress`; transitions incl. the 120 s margin | - | - |
| wire (client_hello, ja3, ja4, akamai, h2_frames, specs) | parse committed capture bytes; GREASE; canonical strings; every `Ja3Error`, `AkamaiError` and `SpecError` case; truncated and hostile input (seeded mutation loop) never panics | - | tshark oracle |
| convert, client | - | `tests/impersonate_wire.rs`: the measured Chrome profile's captured JA4 equals the measured JA4 except exactly the `KnownGap`s; ALPN, SNI and key-share groups equal; bare wreq differs; custom JA3 strings round trip exactly; Akamai over h2c round trips exactly; HEADERS priority and h1 header order equal the spec; rejected specs never build; capability probe | our client and local Chrome captured side by side |
| http_session | - | redirects with `Set-Cookie` per hop, body cap, reserved headers, version mismatch, sync-back to a `Browser::new_mocked` page, against a local `tls_server` site trusted through `tls_cert_store` | page-to-HTTP handoff with real Chrome |
| mitm | - | 407 without the token; `Proxy-Authorization` and hop-by-hop headers never reach the upstream; SNI mismatch closed; bad upstream cert gives 502; 64 MiB body streamed without full buffering; `swap_upstream` mid-session; leaf cache bounded; no key file written; the forwarded request's JA4 and Akamai equal the profile's | Chrome through the MITM loads a local h2 site; a different CA is rejected |
| browser_scraper | fetch, mitigate and rotate flows on `Browser::new_mocked` | - | an idle browser is still usable after a named idle window; Turnstile demo page |
| stealth_session | escalate/demote state machine on mocked transports, injected clock for near-expiry | `restore` refuses another egress; every transition emits `ModeChanged` | full flow against a local fake-challenge server; a live Cloudflare demo page (network) |
| hardening | `session_registry` content; `sb_cdp_domain_hygiene.rs`: scraper flows on `MockCtrl`, `calls_to` of `Runtime/DOM/Log/Debugger/Profiler.enable` is empty | - | `stealth_audit_chrome.rs`: `navigator.webdriver` false, patched getters print `[native code]`, WebGL/Canvas agree with the profile, no `userAgentData` on an `http:` page, screen equals the profile; `stealth_audit_network.rs`: CreepJS, SannySoft |

`#[ignore]` tests never run in CI; real-Chrome binaries run one at a time with `--test-threads=1`.

### 4.3 Gates (definition of done for every package)

```sh
# lane A (pure Rust). Lane B: CARGO_TARGET_DIR=$SCRATCH/target-imp CMAKE_BUILD_PARALLEL_LEVEL=1, --features impersonate
export CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=$SCRATCH/target-scrape
rtk cargo fmt --all -- --check
rtk cargo clippy --features scrape --profile test --lib -- -D warnings      # lib incl. #[cfg(test)]
rtk cargo clippy --features scrape --test <wp_test> --example <wp_example> -- -D warnings
rtk cargo test --features scrape --lib scrape::          # impersonate:: in lane B
rtk cargo test --features scrape --test <wp_test>
rtk cargo test --features scrape --doc scrape
cargo llvm-cov --features scrape --lib --summary-only    # domain packages: >= 80 % lines on owned files
rtk cargo audit && cargo deny check                      # WP0, any package that changes Cargo.lock, WP18, WP19
ripsecrets <owned files>                                 # packages that handle credentials or cookies
```

Never `--all-features` (it pulls `gui`, `tui`, `rustwright`); `--all-targets` clippy runs once, in
WP19. Review checklist: owned files only, no `unsafe`, files < 800 lines, functions < 50, named
constants, no secret in any `Debug`/log/event, `tracing::instrument(skip_all)` on async pub fns.

## 5. Work packages

Models: haiku = mechanical or high-volume, sonnet = primary implementation, opus = security-sensitive
design and audits. Size: S up to 400 lines with tests, M 400-900, L 900-1600. Build: `check`, `test`,
`chrome` (needs the local browser), `B` (impersonate lane, BoringSSL). Every package needs a build.

| WP | Scope (files in §2.1) | Builds on (existing) | Needs | Size | Model | Build |
|---|---|---|---|---|---|---|
| 0 | Scaffold: Cargo.toml, features, stubs, `pub(crate) mod sync`, lint block, purity test, justfile (`scrape-gate`, `imp-gate`, `measure`), CI (`scrape` in the matrix; a separate Linux `impersonate` job with cmake) | lib.rs lint block, deny.toml | - | S | haiku | test + one B build (records time, peak RSS, disk) |
| 1 | redact, egress, geo, proxy_pool; `randomized_with_major`; `ClientHints.wow64/form_factors`; `REGIONS` from `LOCALES` | `sb_cdp::Proxy`, `Fingerprint`, `randomize.rs`, `humanize::Rng`, `sync::locked` | 0 | M | sonnet | test |
| 2 | challenge, markers, mitigation, fixtures | `sb_cdp::Captcha`, `RetryPolicy` | 0 | M | sonnet | test |
| 3 | state, json_store, `atomic_write` | `SessionStore` pattern, `RateLimiter` | 1, 2 | S | sonnet | test |
| 4 | cookie_jar, set_cookie | `sb_cdp::{Cookie, SameSite}`, `url`, `chrono`, `psl` | 0 | M | opus | test |
| 5 | wire `ids.rs` (IANA names, capability seeds) | btls 0.5.6 constants | 0 | S | haiku | check |
| 6 | client_hello, ja3, ja4, tls_spec | `ring::digest` | 5 | L | sonnet | test |
| 7 | akamai, h2_frames, h2_spec, header_spec | - | 0 | M | sonnet | test |
| 8 | identity, session_policy, events, tracing_sink, identity_file | `Fingerprint`, `ClientHints`, `OsType`, WP1-4 types | 1, 2, 3, 4 | M | sonnet | test |
| 9 | capture.rs (TLS, h2c, h1 readers), `capture_tls`, `capture_hints`; first Chrome measurement (TLS + hints JSON, raw bytes) | `Browser`, `LaunchOptionsBuilder::{arg, lang}`, `Page::evaluate`, `Browser::execute` | 6, 7 | M | sonnet | test + chrome |
| 10 | tls_server: `EphemeralCa`, leaf cache, acceptor, SPKI pins | btls, tokio-btls, `base64`, `ring::rand` | 0 | M | opus | B |
| 11 | capture_h2 (+ `--h1`), `emulation.rs`, Chrome H2/header JSON; Safari JSON (user opens the URL) | WP9 capture, WP10 acceptor | 7, 9, 10 | M | sonnet | B + chrome |
| 12 | convert, client, `CustomProfileBuilder`, capability probe, `tests/impersonate_wire.rs`; fixes `ids.rs` flags | wreq `TlsOptions`, `Http2Options`, `OrigHeaderMap` | 5, 6, 7, 11 | L | sonnet | B |
| 13 | `error.rs`; adapters `cdp`, `browser_scraper`, `egress_switch`; ContextRoute port from the worktree | `Browser`, `Page`, `BrowserContext`, `ContextOptions::proxy`, `Cookies`, `Page::{solve_captcha, human, apply_fingerprint, events}`, `poll_until`, `RateLimiter`, shutdown | 1, 2, 3, 8 | L | sonnet | test + chrome |
| 14 | http_session (salvage the worktree draft, drop wreq-util) | WP4 jar, WP12 client, WP13 `adapters::cdp` | 4, 8, 12, 13 | M | sonnet | B |
| 15 | mitm (connect, forward, upstream), `tests/impersonate_mitm.rs` | `ProxyAuth`, `LaunchOptionsBuilder::proxy`, shutdown, hyper | 1, 8, 10, 11, 12 | L | opus | B + chrome |
| 16 | stealth_session, `tests/stealth_session.rs` | WP13, WP14, WP15, WP17 | 13, 14, 15, 17 | L | sonnet | B + chrome |
| 17 | hardening: `stealth/cdp.rs`, `providers/{mod,builtin}.rs`, `sb_cdp/identity.rs`, hygiene and audit tests | `MockCtrl::calls_to`, `EvasionRegistry` | 0 | M | sonnet | test + chrome |
| 18 | split-role security review. Pass 1 after WP4, 8, 10 (cookies, identity files, CA); pass 2 after WP14-16 (MITM, credentials, egress stickiness); audit, deny, ripsecrets; may patch only a reviewed package's files after it merged | - | see scope | M | opus | B |
| 19 | re-exports in `scrape/mod.rs`, `impersonate/mod.rs`, `lib.rs`; `docs/tutorials/stealth_scraping.md`; fix `fingerprint_stealth.md` lines 529-530 ("not implemented", "consider wreq-util"); ROADMAP; full `--all-targets` clippy per feature, doctests, audit, deny | - | all | S | haiku | test + B |

Schedule: at most two agents at once (8 GB M1), `CARGO_BUILD_JOBS=1`, two shared target dirs:
`target-scrape` (lane A) and `target-imp` (lane B, the only place BoringSSL compiles). Cargo's dir lock
serialises builds per lane, so at most two compilations run at once; real Chrome runs one test binary
at a time across both agents (waves 7-8 take turns). Delete both dirs after WP19.

| Wave | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Agent 1 | WP0 | WP1 (A) | WP2 (A) | WP5 (A) | WP6 (A) | WP8 (A) | WP9 (A, chrome) | WP13 (A, chrome) | WP13 | WP14 (B) | WP16 (B) | WP18 pass 2 |
| Agent 2 | - | WP10 (B) | WP4 (A) | WP7 (A) | WP3 (A) | WP17 (A, chrome) | WP18 pass 1 | WP11 (B, chrome, Safari by the user) | WP12 (B) | WP15 (B) | - | WP19 (after pass 2 fixes) |

Every agent prompt carries the tool order (TokenSave, then rtk, then rg/fd/sd/eza) and its file list;
a package that needs a file it does not own stops and reports instead of editing it.

## 6. Risks, open questions, not copied

Cannot be done yet with btls 0.5.6 / wreq rc.31:
- Extension 0xca34 (trust anchors) needs btls#168 and wreq#1273 in a release. If the local Chrome sends
  it, our JA4 differs in extension count and `c` hash: recorded as `KnownGap::TrustAnchorsExtension`;
  the wire test asserts the difference is exactly the known gaps and fails once one disappears. Fix:
  bump both exact pins in one package, rerun the probe and `just measure`.
- ML-DSA sigalgs 0x0904/0x0905/0x0906 cannot be emitted: a `KnownGap` for measured entries,
  `UnsupportedSigalg` for custom specs. Experimental/GREASE H2 settings: `UnsupportedSetting`.
- Chrome permutes extensions per connection, so a preset's JA3 varies by design (JA4 is stable); a
  custom JA3 pins the order and matches exactly.

Risks:
- Version drift: once the local Chrome updates past the measured major, `HttpSession` and the MITM
  refuse (`VersionMismatch`) until `just measure` reruns; an ignored test flags it.
- Only macOS is measured until Windows/Linux captures exist, so other personas cannot use HTTP mode or
  the MITM. Deliberate: no reasoned values.
- The h1 intercepted leg drops Chrome's h2-only `priority` header; the MITM re-adds the measured value.
  Any other h1/h2 difference WP11 finds must go into `HeaderSpec`, or it leaks.
- `ProxyAuth` relies on `Fetch` interception (WP15 measures the cost); rc pins may change API
  (`convert.rs` is the only file naming wreq types).
- BoringSSL needs cmake, a C/C++ toolchain and about 1 GB of target disk, and `psl` is a large generated
  crate; WP0 records time, peak memory and disk for both with one job.
- No HTTP/3 in HTTP mode (wreq has none; Chrome behind a proxy skips QUIC too). CHIPS cookies and
  Critical-CH retries are not modelled. `JsonStateStore` is single-writer.
- CA key material is freed, not guaranteed wiped (no zeroize in btls): acceptable for an ephemeral,
  loopback-only CA. A matching fingerprint does not make automation permitted; the tutorial says so.

Questions the plan raised, and the answers (2026-10-10):
1. `--remote-debugging-pipe` needs fds 3/4 in the child, which std only offers through `unsafe`
   `pre_exec` (`command-fds` is unsafe inside). Accept that, or keep `--remote-debugging-port=0`?
   **Answer: keep `--remote-debugging-port=0`.** No `unsafe`; the pipe transport is deferred.
2. Try ALPN `h2` on the intercepted leg (Chrome's own header block) instead of the recorded `http/1.1`?
   **Answer: keep `http/1.1`** on the browser-to-proxy leg.
3. Will you run the capture tools on Windows and Linux machines, and open the URL in Safari in WP11?
   **Answer (user): macOS only for now.** Windows, Linux and Safari personas stay refused with
   `SpecError::NotMeasured` until they are captured on those systems.
4. v2: one MITM for many `BrowserPool` contexts, routed by per-context token (`ProxyAuth::register`)?
   **Answer: later.** One proxy per context for now.
5. Accept peetprint or JA4_r strings as further custom inputs (they carry signature algorithms)?
   **Answer (user): "custom inputs are for everything".** Every part of a profile can be supplied by
   the caller, layered over a measured base: JA3, JA4 and JA4_r, peetprint, Akamai HTTP/2
   fingerprints, header names, order and values, the `User-Agent`, `Accept-Language` and the
   client-hint set. Each input is checked for what BoringSSL and wreq can emit; anything they cannot
   reproduce is refused with a named `SpecError` before a client exists, and every accepted input is
   proven by a wire round trip. WP6 and WP7 therefore also parse JA4_r and peetprint (WP6), and the
   builder in WP12 takes each of them.

Deliberately not copied: their code and names (`CloudScraper` is `BrowserScraper`/`StealthSession`);
their measured values (Chrome 153, Safari 27); `wreq-util` and its presets, including the worktree
draft's `CHROME_PROFILES`; `RedbStateStore` and `persistence`; `log` sinks (we use `tracing`);
`headless_chrome` and its idle timeout; the `no_std` domain check; a crate-wide
`#![forbid(unsafe_code)]` (the new modules get it); `--ignore-certificate-errors`; any CA key on disk;
built-in third-party IP-geolocation (port only); JA4+ beyond JA4 (JA4H, JA4S, JA4X... are under the
FoxIO License); automatic planner and reviewer agents on every change.
