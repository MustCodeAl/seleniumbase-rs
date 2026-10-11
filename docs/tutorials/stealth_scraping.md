# Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper

A site can tell a client apart before any page script runs: by the TLS
`ClientHello`, by the HTTP/2 connection start, and by the order and spelling of
the request headers. [Fingerprint & Stealth Profiles](fingerprint_stealth.md)
covers what a page can read from inside the browser. This page covers the
other side, the wire, and the pieces that keep an identity, its cookies and its
exit address together while a scraper works through a site's defences.

Everything here is young, and it is not a stable API yet: types and paths may
change between releases. The parts that exist are tested against local captures
and a local Chrome, on one system. Read [Limits](#limits) before you rely on any
of it.

## Status

| Part | Feature | State |
|---|---|---|
| Wire parsers: `ClientHello`, JA3, JA4 and `JA4_r`, peetprint, Akamai HTTP/2 strings | `scrape` | Available |
| Measured browser tables (Chrome 155 on macOS) and the lookup that refuses everything else | `scrape` | Available |
| Capture tools `capture_tls` and `capture_hints` | `scrape` | Available |
| Capture tool `capture_h2` | `impersonate` | Available |
| `ImpersonationProfile`, `CustomProfileBuilder`, `impersonation_client` (wreq with BoringSSL) | `impersonate` | Available |
| RFC 6265 cookie jar | `scrape` | Available |
| `StealthIdentity`, its identity file, `SessionPolicy` | `scrape` | Available |
| Challenge detection, mitigation policy, proxy pool, per-site state, events | `scrape` | Available |
| `BrowserScraper` (a real Chrome, no TLS spoofing) | `scrape` | Available |
| Ephemeral-CA TLS server | `impersonate` | Available |
| `HttpSession` | `impersonate` | Available |
| `MitmProxy` | `impersonate` | Available |
| `StealthSession` | `impersonate` | Available |

The last three are tested against loopback sites, a scripted browser and, in
tests marked `#[ignore]`, a local Chrome 155 on macOS, run headless. None of
them was checked against a live service. [Limits](#limits) lists what that
leaves open.

## What you will learn

- Which feature to turn on, and what each one costs to build.
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

## Two features

```toml
[dependencies]
# Pure Rust. Adds only the `psl` crate (the Public Suffix List).
seleniumbase-rs = { git = "https://github.com/MustCodeAl/SeleniumBase", features = ["scrape"] }

# Also builds BoringSSL through btls and drives it with wreq. Needs cmake and a
# C/C++ toolchain. It includes `scrape`.
# seleniumbase-rs = { git = "https://github.com/MustCodeAl/SeleniumBase", features = ["impersonate"] }
```

| Feature | What it adds | Cost |
|---|---|---|
| `scrape` | `seleniumbase_rs::wire` (pure wire formats and the measured tables) and `seleniumbase_rs::scrape` (jar, identity, challenge detection, egress, state, events, `BrowserScraper`, capture tools). | Pure Rust. The default build gains no dependency and no C code. |
| `impersonate` | `seleniumbase_rs::impersonate`: the wreq client, the conversion from a spec to wreq options, `capture_h2`, the ephemeral-CA TLS server, `HttpSession`, `MitmProxy` and `StealthSession`. | Builds BoringSSL: `cmake` and a C/C++ toolchain, and about 1 GB of build output (the design plan's estimate). `wreq` and `btls` are pinned to exact versions (`=6.0.0-rc.31` and `=0.5.6`). |

Neither feature is on by default. The main types are re-exported at the crate
root (`seleniumbase_rs::BrowserScraper`, `seleniumbase_rs::HttpSession`, ...) and
at the top of `wire`, `scrape` and `impersonate`; the examples below use the
module paths where the module says more. The TLS and HTTP/2 spec error enums are
both called `SpecError` in their modules, so the re-exports are `TlsSpecError`
and `H2SpecError`. The crate links only one BoringSSL, so do not add a second
crate that links it to the same build.

## What is measured, and what is refused

Every wire value this crate imitates was measured from a real browser by a
capture tool and stored as JSON in `src/wire/measured`. Nothing is typed by
hand. The files are embedded with `include_str!`, parsed once, and looked up by
browser and operating system.

Today there is one entry: **Chrome 155.0.8059.40 on macOS**, captured on
2026-10-11. Its record holds the user agent, the TLS spec (and the one Chrome
uses for WebSocket), the HTTP/2 start, the header lists and their order, the
client hints, and where it came from:

```rust
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation::{self, EmulationError};
use seleniumbase_rs::wire::BrowserKind;

# fn demo() -> Result<(), EmulationError> {
let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
assert_eq!(chrome.major, 155);
println!("{} captured on {}", chrome.provenance.browser, chrome.provenance.captured_on);
// The JA4 of the measured hello. The capture recorded t13d1517h2_8daaf6152771_cb7bf5808d99
// for it, in the file's `capture` evidence.
println!("{}", chrome.tls.ja4());

// A system nobody measured is an error, never a nearby guess.
let windows = emulation::get(BrowserKind::Chrome, OsType::Windows);
assert!(matches!(windows, Err(EmulationError::NotMeasured { .. })));
# Ok(())
# }
```

`EmulationError::NotMeasured { kind, os }` is the error the design plan calls
`SpecError::NotMeasured`; `wire::emulation::SpecError` is an alias for
`EmulationError`. The TLS and HTTP/2 refusals live in two other enums with the
same name, `wire::tls_spec::SpecError` and `wire::h2_spec::SpecError`.

Why an unmeasured system is refused rather than approximated: a made-up persona
is worse than no persona. A TLS stack that claims to be Chrome on Windows with
values copied from Chrome on macOS is a contradiction a detector can see, and
the crate has no way to know which of the two differ. Windows, Linux, Android,
iOS and Safari stay refused until someone runs the capture tools on that system
and commits the result. The same rule shows up higher up:

- `ImpersonationProfile::for_system(BrowserKind::Chrome, OsType::Windows)` and
  `ImpersonationProfile::from_fingerprint` of a Windows `Fingerprint` return
  `ImpersonateError::Emulation(EmulationError::NotMeasured { .. })`.
- `ImpersonationProfile::from_fingerprint` of a fingerprint whose user agent
  claims another Chrome major than the measured one returns
  `ImpersonateError::VersionMismatch`: Chrome 155's handshake under a Chrome 153
  user agent is a contradiction too.
- `ImpersonationProfile::random(seed)` only draws from measured entries, and
  the identity it builds claims the measured major
  (`Fingerprint::randomized_with_major`).

## Re-measuring when Chrome updates

The measured file describes the Chrome that was installed when it was captured.
When Chrome moves to a new major version, nothing updates the file for you: the
profile still describes Chrome 155, and a site sees the difference. To add the
new version:

1. Update Chrome. The tools start their own browser (the system Chrome, or the
   one `--chrome PATH` names), headed. Leave `--headless` off: a headless
   Chrome reports a different identity, so `capture_hints` refuses it
   (`CaptureError::Headless`) and the others do not measure its identity.
2. Run the three tools in this order, from the repository root. Each extends the
   same file, named `<browser>-<major>-<os>.json`, and `capture_h2` refuses a
   Chrome of another version than the file it extends.

   ```text
   cargo run --features scrape      --example capture_tls
   cargo run --features scrape      --example capture_hints
   cargo run --features impersonate --example capture_h2
   ```

   - `capture_tls` points the browser at a loopback listener that records each
     `ClientHello` and hangs up. The JA3, JA4 and peetprint strings and the raw
     bytes all come from what the browser sent. It prints the known gaps and
     whether the result can be reproduced.
   - `capture_hints` serves a small page that asks the browser for its
     high-entropy hints and records the `Sec-CH-UA*` request headers of the same
     browser. If the two disagree, nothing is written. For a browser the tool
     cannot drive, `--snippet` prints a script to paste into its developer
     console.
   - `capture_h2` serves a small HTTPS site with an [ephemeral
     CA](#the-ephemeral-ca-tls-server), starts Chrome so that it trusts only
     that CA, and reads the first request on each of five connections (a
     navigation, a `fetch`, an image, a script, a stylesheet), once over HTTP/2
     and once over HTTP/1.1. `--h1` and `--h2` run one pass only.

   They share these options: `--out DIR` (default `src/wire/measured`),
   `--fixtures DIR` (default `tests/fixtures/measured`, for the raw bytes),
   `--chrome PATH`, `--lang TAG` (sets `Accept-Language`, default `en-US`),
   `--probes N`, `--timeout SECS`, `--headless`, `--no-sandbox`, `--snippet` and
   `--help`.
3. Add the new file to the `SOURCES` list in `src/wire/emulation.rs`. The test
   `tests/wire_emulation.rs` fails when a file in `src/wire/measured` is missing
   from that list.
4. Run the wire tests, including `tests/impersonate_wire.rs`. They compare what
   our client sends with the measured file and fail, by name, when a known gap
   closes or a new difference appears.

A new Windows or Linux entry comes the same way: run the tools on that system.
Safari has no tool yet beyond the `capture_hints` console snippet.

## Reading the wire formats

All of `seleniumbase_rs::wire` is pure: no sockets, no clock, no files, and
`#![forbid(unsafe_code)]`. Every parser bounds its input and returns an error
for hostile or truncated bytes instead of panicking.

| Format | Type | What it is |
|---|---|---|
| TLS `ClientHello` | `wire::client_hello::ClientHello` | `ClientHello::parse(records)` decodes the first handshake message, across TLS records. `ClientHello::needed_len(prefix)` says how many bytes a capture loop still needs. |
| JA3 | `wire::ja3::Ja3` | Five comma-separated fields with GREASE removed. `Ja3::parse`, `Ja3::of(&hello)`, `hash()` (MD5, as JA3 defines it). |
| JA4 | `wire::ja4::Ja4` | `a_b_c` of FoxIO's TLS-client definition. `Ja4::of`, `Ja4::of_original_order` (`JA4_o`), `Ja4::parse`, `diff`. |
| `JA4_r`, `JA4_ro` | `wire::ja4::Ja4Raw` | The unhashed, sorted or wire-order variants. `Ja4::raw(&hello)`, `Ja4Raw::parse`, `Ja4Raw::hashed()`. |
| peetprint | `wire::ja3::Peetprint` | A JA3-like format that also records signature algorithms. GREASE shows as `GREASE`. |
| Akamai HTTP/2 | `wire::akamai::Akamai` | `SETTINGS \| WINDOW_UPDATE \| PRIORITY \| pseudo-header order`. |

Only JA4 itself is implemented. JA4H, JA4S, JA4X and the other JA4+ methods carry
a different licence and are not part of this crate.

```rust
use seleniumbase_rs::wire::akamai::Akamai;
use seleniumbase_rs::wire::ja3::Ja3;
use seleniumbase_rs::wire::ja4::Ja4;

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
// The Akamai string measured from Chrome 155 on macOS.
let akamai: Akamai = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p".parse()?;
assert_eq!(akamai.settings, vec![(1, 65536), (2, 0), (4, 6_291_456), (6, 262_144)]);
assert_eq!(akamai.window_update, 15_663_105);

// One JA4 and one JA3 measured from the same browser.
let ja4: Ja4 = "t13d1517h2_8daaf6152771_cb7bf5808d99".parse()?;
assert_eq!(ja4.a, "t13d1517h2");
let ja3: Ja3 = "771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,\
                13-10-35-18-65281-27-45-65037-5-0-23-17613-51-51764-43-11-16,4588-29-23-24,0"
    .parse()?;
assert_eq!(ja3.version, 771);
# Ok(())
# }
```

Chrome shuffles its TLS extensions on every connection, so its JA3 changes from
one connection to the next by design. JA4 sorts them first and stays stable.
Compare JA4 when you compare a browser with a client.

The spec types sit on top of the parsers. A `TlsSpec`
(`TlsSpec::from_client_hello`) is the whole TLS profile as plain data, and an
`H2Spec` (alias `Http2Spec`; `H2Spec::from_capture`, `H2Spec::from_akamai`) is
the HTTP/2 start. Each has `check_reproducible()`, which says whether the
BoringSSL and wreq stack can send it, and names what it cannot.

## An impersonating client

The `impersonate` feature turns a measured profile into a wreq client:

```rust
use seleniumbase_rs::impersonate::client::{
    impersonation_client, ImpersonationProfile, WireProtocol,
};
use seleniumbase_rs::wire::header_spec::RequestShape;
use seleniumbase_rs::wire::BrowserKind;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let profile = ImpersonationProfile::measured(BrowserKind::Chrome)?;
let client = impersonation_client(&profile)?.build()?;

let request = client.get("https://example.com/");
let request = profile.apply(request, RequestShape::Document, None, WireProtocol::Http2)?;
let response = request.send().await?;
# drop(response);
# Ok(())
# }
```

- `impersonation_client` returns a `wreq::ClientBuilder` with the profile's
  `ClientHello`, HTTP/2 start and default headers. You add proxies, timeouts and
  a trust store, then build. Certificate verification stays on: trust a private
  root with `tls_cert_store`, never by turning verification off.
- `profile.apply` gives one request the headers of its shape
  (`RequestShape::Document` for a navigation, `RequestShape::Fetch` for a
  script-issued request) in the measured order and spelling. The identity
  headers (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`, `Cookie`) are filled in
  per request, so rotating an identity needs no new connection setup. The
  cookie is passed in and never stored. On HTTP/2 the list ends with the
  `priority` header of the measured table for the request's `Sec-Fetch-Dest`;
  Chrome sends it on HTTP/2 only.
- The profile carries the measured TLS without its known gaps (see
  [Limits](#limits)) and keeps extension permutation on, as Chrome does.
- Cookies, redirects, body limits and the hand-over from a browser are not part
  of this client. That is what [`HttpSession`](#httpsession) is for.

Other ways to get a profile: `ImpersonationProfile::for_system(kind, os)`,
`ImpersonationProfile::from_fingerprint(&fp)` (a Chromium identity that claims
the measured major), `ImpersonationProfile::random(seed)`, and
`.with_locale(&locale)` to change the `Accept-Language`.

## Custom inputs

Every part of a profile can be supplied by the caller. `CustomProfileBuilder`
starts from the newest measured Chrome (or `base`/`base_on` another measured
entry) and layers your inputs over it. What an input does not carry comes from
the base: a JA3 has no signature algorithms, an Akamai string has no HEADERS
priority, a header list has no TLS.

```rust
use seleniumbase_rs::impersonate::client::ImpersonationProfile;

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let profile = ImpersonationProfile::custom()
    .akamai("1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p")?
    .user_agent("Mozilla/5.0 (custom)")
    .accept_language("de-DE,de;q=0.9")
    .build()?;
assert_eq!(profile.user_agent(), "Mozilla/5.0 (custom)");
# Ok(())
# }
```

| Method | What it replaces |
|---|---|
| `base(kind)`, `base_on(kind, os)` | The measured entry that supplies everything the other inputs leave out. Call first: it discards the TLS and HTTP/2 inputs given before it. |
| `ja3(&str)` | Version, ciphers, extensions, groups and point formats. Turns extension permutation off, so the JA3 arrives exactly as given. |
| `ja4_r(&str)`, `ja4_ro(&str)` | Ciphers, extensions and signature algorithms, from a sorted or wire-order raw JA4. |
| `peetprint(&str)` | Ciphers, extensions, groups, versions, ALPN, signature algorithms, PSK modes and certificate compression. |
| `signature_algorithms(&[u16])` | The signature algorithms alone. |
| `grease(bool)`, `permute_extensions(bool)` | GREASE (RFC 8701) and Chrome-style extension shuffling. Call `grease` before `ja3`: a spec that lists padding needs GREASE off. |
| `akamai(&str)` | HTTP/2 SETTINGS and their order, the connection window, PRIORITY frames and the pseudo-header order. The HEADERS priority fields stay the base's. |
| `user_agent`, `accept_language`, `client_hints` | The identity headers. |
| `header`, `without_header`, `header_order`, `priority`, `headers` | Header names, values, order and the HTTP/2 `priority` value per `Sec-Fetch-Dest`, per request shape. |

The inputs that need the base to be judged (`ja3`, `ja4_r`, `ja4_ro`,
`peetprint`, `signature_algorithms`, `akamai`) return their error at once. The
identity and header inputs are checked in `build()`. Nothing is sent before a
client exists.

### What is refused

BoringSSL and wreq cannot emit everything a browser can. Anything they cannot
reproduce is refused with a named error before a client exists, and
`tests/impersonate_wire.rs` proves on the wire that an accepted input arrives as
given. The errors are variants of `ImpersonateError`: `Tls(..)` wraps
`wire::tls_spec::SpecError`, `Http2(..)` wraps `wire::h2_spec::SpecError`,
`Convert(..)` wraps the rules that belong to the client stack itself. A string that
is not well formed at all is `Ja3(..)`, `Ja4(..)`, `Peetprint(..)` or `Akamai(..)`.

| Input | Refused | Error |
|---|---|---|
| Handshake version | Anything other than TLS 1.2 (`771`), the only value BoringSSL writes | `LegacyVersion` |
| Cipher suites | DHE suites, the TLS 1.3 CCM suites, the signalling suites, anything BoringSSL does not write; a subset of the three TLS 1.3 suites | `UnsupportedCipher`, `Tls13Suites` |
| Extensions | `pre_shared_key`, `early_data`, `post_handshake_auth`, `trust_anchors` (`0xCA34`), GREASE ids listed as extensions, any id with no BoringSSL option; a repeated id | `UnsupportedExtension { id, reason }`, `DuplicateExtension` |
| Supported groups and key shares | x448, brainpool, secp256k1, the larger FFDHE groups; a key share for a group that is not offered | `UnsupportedGroup`, `KeyShareNotOffered` |
| Signature algorithms | ML-DSA (`0x0904` to `0x0906`) and any id outside the set BoringSSL writes | `UnsupportedSigalg { id, name }` |
| HTTP/2 SETTINGS | Ids other than 1 to 6, 8 and 9 (GREASE and experimental settings); a repeated id; a value outside RFC 9113's range; leaving out `SETTINGS_INITIAL_WINDOW_SIZE`, which wreq always sends | `UnsupportedSetting`, `DuplicateSetting`, `SettingValue`, `SettingAlwaysSent` |
| HTTP/2 window and priority | A window increment that passes 2^31-1; a PRIORITY frame for stream 0, or two for one stream (wreq drops both silently) | `WindowUpdate`, `Priority { stream, reason }` |
| ALPN | Anything wreq cannot announce besides `h2` and `http/1.1` | `Convert(..)` |

For example, a JA3 that adds `trust_anchors` and a signature algorithm BoringSSL
cannot write:

```rust
use seleniumbase_rs::impersonate::client::{ImpersonateError, ImpersonationProfile};
use seleniumbase_rs::wire::tls_spec::SpecError;

# fn demo(ja3_with_trust_anchors: &str) {
let refused = ImpersonationProfile::custom().ja3(ja3_with_trust_anchors);
assert!(matches!(
    refused,
    Err(ImpersonateError::Tls(SpecError::UnsupportedExtension { id: 0xCA34, .. }))
));

let refused = ImpersonationProfile::custom().signature_algorithms(&[0x0904]);
assert!(matches!(
    refused,
    Err(ImpersonateError::Tls(SpecError::UnsupportedSigalg { id: 0x0904, .. }))
));
# }
```

A refusal is not a bug to work around. It means the pinned BoringSSL and wreq
cannot send what you asked for, and sending something close would be a different
fingerprint from the one you named.

## The cookie jar

`scrape::jar::store::CookieJar` is an RFC 6265 cookie store with the RFC 6265bis
rules Chrome ships. It does the storage model of section 5.3 and the header
building of section 5.4, checks domains against the Public Suffix List (`psl`),
and converts to and from CDP cookies. Every function that needs the time takes
`now` in seconds since the Unix epoch.

```rust
use seleniumbase_rs::scrape::jar::store::{CookieJar, SiteContext};
use url::Url;

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)?
    .as_secs();
let url = Url::parse("https://shop.example.com/cart")?;

let mut jar = CookieJar::new();
jar.store(&url, "sid=abc; Path=/; Secure; HttpOnly; SameSite=Lax", now);
let header = jar.header_for(&url, SiteContext::SameSite { top_level: true }, now);
assert_eq!(header.as_deref(), Some("sid=abc"));
# Ok(())
# }
```

What the jar enforces: `Secure` cookies need a secure origin (https or wss, or a
loopback host), `__Secure-` and `__Host-` prefixes, `SameSite=None` needs
`Secure`, and a `Domain` on a public suffix or an IP address is refused. Lifetimes
are capped at 400 days. A cookie it refuses comes back as
`StoreOutcome::Ignored(IgnoreReason)` with the reason. The jar is bounded: 180
cookies and 256 KiB per site, 3000 cookies in all, and a path or domain over its
limit is refused. `Debug` prints counts only, and a jar read back from a file is
checked against the same rules, so an edited file cannot smuggle in a cookie the
jar would never store.

`CookieJar::from_cdp(&cookies, now)` and `to_cdp(now)` convert to and from the
cookies a CDP browser reports, which is how `BrowserScraper` moves an identity
between runs.

## Identity, session policy and the identity file

A `StealthIdentity` (`scrape::passport::identity`) is what a scrape session
presents to a site: the `Fingerprint`, a reference to the measured browser its
wire behaviour comes from (`EmulationRef`), the cookies it earned, the client
hints each origin asked for, the clearances found in the jar, and the egress
those cookies were earned through.

```rust
use seleniumbase_rs::scrape::passport::identity::{EmulationRef, StealthIdentity};
use seleniumbase_rs::scrape::runtime::identity_file;
use seleniumbase_rs::stealth::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::wire::BrowserKind;

# async fn demo(now: u64) -> Result<(), Box<dyn std::error::Error>> {
let fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 7, 155);
let emulation = EmulationRef { kind: BrowserKind::Chrome, major: 155, os: OsType::Macos };
let identity = StealthIdentity::new(fingerprint, emulation, None, now)?;

identity_file::save("identity.json", &identity).await?;
let again = identity_file::load("identity.json").await?;
# drop(again);
# Ok(())
# }
```

Things to know:

- **A clearance belongs to the address that earned it.** A clearance cookie is
  bound to the exit address, so an identity is used again only through the same
  egress. `check_egress` returns `IdentityError::EgressMismatch` for any other.
  `check_emulation` refuses an identity made for another Chrome major than the
  measured one.
- **It never holds a proxy.** A proxy can carry a password, so
  `StealthIdentity::new` clears it, serialization leaves it out, and `verify`
  refuses an identity that has one. `Debug` prints the cookie count and the
  egress key only. A clearance records the cookie's name, never its value.
- **The identity file is private.** It holds cookies, which are credentials. It
  is written atomically (a synced temporary file renamed over the target) with
  mode `0600` on Unix, and an older file with wider permissions is replaced. It
  is read only if group and others have no access (`IdentityFileError::Permissions`
  names the fix: `chmod 600`, or delete it). The file is not corrected for you,
  because wider permissions mean the cookies may already have been read. The
  schema is read first, so a file from another version is reported as that
  (`IdentityError::Schema`); a parse error says where it failed (line and
  column) and in which way, never the value it met, which may be a cookie. The
  file is limited to 16 MiB. One process writes a file at a time; there is no
  lock between processes. On systems other than Unix there is no mode to check.

`SessionPolicy` (`scrape::passport::policy`) decides when a session should move
between plain HTTP and the browser, as a pure function of the mode, the identity,
the site's state and the time:

```rust
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::passport::identity::StealthIdentity;
use seleniumbase_rs::scrape::passport::policy::{SessionPolicy, Transition};

# fn demo(identity: &StealthIdentity, now: u64) {
let policy = SessionPolicy {
    require_clearance_for_http: true,
    ..SessionPolicy::default()
};
// An HTTP session whose identity holds no clearance for the site goes to the browser.
let step = policy.before_fetch(Mode::Http, identity, "example.com", None, now);
assert!(matches!(step, Transition::Escalate(_)));
# }
```

The defaults are a margin of 120 seconds before a clearance expires, demotion
back to HTTP after a solve, and a preference for HTTP. `after_fetch` turns an
`Action::Escalate` seen in HTTP mode into an escalation and a solved challenge in
the browser into a demotion when the identity now holds a clearance. The tables
are in the module documentation. `BrowserScraper` stays in the browser and never
applies them. [`HttpSession`](#httpsession) offers `preflight` and `assess` for a
caller that runs its own loop, and [`StealthSession`](#stealthsession) applies
them for you.

## Challenge detection and mitigation

`scrape::challenge::detect::detect` reads a `ResponseView` (URL, status, headers,
body) and returns a `Detection`: a `ChallengeKind` and the `Evidence` behind it.

- The kinds are `None`, `JsChallenge` (a script must run), `Interactive` (a widget
  must be completed), `Blocked` and `RateLimited { retry_after_secs }`.
- The vendors are Cloudflare, DataDome, PerimeterX, Akamai, Imperva, Kasada and
  AWS WAF, with the clearance cookie each one sets (`clearance_cookie(vendor)`;
  Kasada has none).
- A response is a challenge only when it carries an interstitial marker. A vendor
  header, a CAPTCHA widget on a normal page, or a bare 403 is evidence and
  nothing more. A 429 with no marker is `RateLimited`; the `Retry-After` header
  is read only as a number of seconds, not as a date.
- Only the first 256 KiB of a body is scanned.

`MitigationPolicy::decide(&detection, &AttemptContext)` turns a detection into one
`Action`. It is a pure function; the caller passes the attempt number, the mode,
the streak of challenges the site has shown, whether another egress exists, and
`now`.

| Detection | Decision |
|---|---|
| `None` | `Proceed` |
| Anything else, once `attempt >= max_challenge_attempts` (default 3) | `GiveUp` |
| `RateLimited` | `Cooldown` for `Retry-After` (capped at one hour), or 60 s |
| `Blocked` | `RotateEgress`, or `Cooldown` (900 s by default) when there is nothing to rotate to |
| Script or interactive challenge, HTTP mode | `Escalate` to the browser when it can; otherwise retry, rotate, or give up |
| Script or interactive challenge, browser mode | `Solve`; a streak of 2 rotates the egress (when another exists) and a streak of 3 rotates the profile |

A solve is the browser waiting for the vendor's clearance cookie, after clicking
a widget if there is one (`Page::solve_captcha`). It is an attempt, not a
guarantee: sites change their pages, and no solver here promises to pass any
service.

## BrowserScraper

`scrape::runtime::browser_scraper::BrowserScraper` drives one real Chrome through
the pure CDP engine. It does no TLS spoofing: Chrome's own handshake leaves the
machine, so the page sees a genuine browser. What it adds is the loop a
bot-protected site needs around it.

```rust
use seleniumbase_rs::scrape::egress::geo::CountryCode;
use seleniumbase_rs::scrape::runtime::browser_scraper::BrowserScraper;

# async fn demo() -> Result<(), seleniumbase_rs::scrape::error::ScrapeError> {
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

### The fetch loop

`fetch(url)` loads the URL, reads the answer and the rendered page, runs
`detect`, and asks the `MitigationPolicy` what to do:

- a real page ends the loop and comes back as a `BrowserFetch` (URL after
  redirects, status, headers, the rendered body, the `Detection`, how many loads
  it took, the challenge solved on the way, and the egress);
- a challenge is solved in the browser (`solve_challenge`), then the page is
  loaded again with the clearance it earned;
- repeated challenges change the egress (`rotate_egress`) and then the browser
  profile (`rotate_profile`). With the default of three attempts, a fetch gives
  up before the profile ladder starts unless the site's stored streak was already
  high; raise `with_max_challenge_attempts` to let one fetch climb it;
- a rate limit or a block with no egress to change to starts a cooldown for the
  site, which the next fetch respects (`ScrapeError::CoolingDown`);
- when the attempts run out the fetch fails: `ScrapeError::Unsolved` if a
  challenge was still there, `ScrapeError::GaveUp` for another reason.

What each site did is kept in a `StateStore` (successes, challenges, blocks, the
streak, a cooldown, the egress it last answered through). The default is in
memory; `JsonStateStore::open(path)` keeps one file, written atomically with
`0600` permissions, bounded to 10,000 sites, never reset when it cannot be read
(an unreadable file is an error and is left alone), and with one writer at a
time. Pass it with `with_state_store(Arc::new(store))`. `per_site_rate(calls,
window)` limits fetches per site.

### Egress and the profile

Proxies are `(spec, country)` pairs. A spec is `scheme://[user:password@]host:port`
with `http`, `https`, `socks5` or `socks5h`; the credentials are taken out of the
string at once and never appear in `Debug`, `Display`, logs, events, errors or
serialized state, which carry the `EgressKey` (scheme, host, port, country)
instead. The country decides the language and the time zone the browser presents
while it leaves through that proxy, from a built-in locale table (the
`StaticGeoResolver`; no IP lookup service ships).

`proxy_strategy` picks how the pool chooses: `RoundRobin` (the default),
`Random { seed }`, `LeastFailures` or `StickyPerSite`. An egress with three
failures in a row is out of rotation for ten minutes by default.

- `rotate_egress()` opens a new browser context behind the new proxy and drops
  the old one with its cookies, because a clearance belongs to the address that
  earned it. When the new egress is in another country, the browser is also
  started again with the same profile in that country's language and time zone,
  because the fingerprint is written into every document before its scripts run.
  It returns `ScrapeError::NoEgressAvailable` when the pool has nothing else.
- `rotate_profile()` derives a new seed, draws a new fingerprint for the same
  operating system that claims the version of the Chrome that is running, and
  starts the browser again behind the same egress, with empty cookies. What is
  known of each site stays.
- The first profile claims the installed Chrome's major version. Unless you pass
  a `fingerprint(..)`, `build()` first starts a headless Chrome once to learn
  which version is installed, and closes it again. `seed(..)` makes a run
  repeatable.

### Identities and sessions

`export_identity()` takes the identity out of the scraper: the fingerprint, the
cookies the browser holds, the clearances among them, and the egress. Save it
with the [identity file](#identity-session-policy-and-the-identity-file).
`import_identity(&id)` puts one back: it refuses an identity for another egress
or another Chrome major, restarts the browser when the fingerprint differs, and
sets the cookies. `save_session(name)` and `load_session(name)` keep cookies and
local storage in a `SessionStore`, the way a browser pool lease does.

### Events and errors

The scraper reports what it does to an `EventSink`. A `ScraperEvent` can hold a
URL only as a `RedactedUrl` (userinfo and sensitive query values removed) and a
proxy only as an `EgressKey`, so a password or token cannot reach a sink by
construction. The default sink, `TracingEventSink`, writes each event to
`tracing` on the target `seleniumbase_rs::scrape`; `CollectingEventSink` keeps
the newest events for tests. Errors are `ScrapeError`, whose URLs are redacted
too, and which map to `SeleniumBaseError::Stealth` at the crate boundary.

### Lifetime

A launched browser lives until `close()`. There is no idle timeout and no
deadline in the scraper: it can sit unused for as long as the process lives and
be used again.

## HttpSession

`impersonate::http_session::HttpSession` sends plain HTTP requests that look like
the Chrome a browser page came from. A browser spends a second or more on a page;
a request spends milliseconds. Once a page has earned its cookies (a login, a
passed check), an `HttpSession` carries them on: the measured TLS `ClientHello`
and HTTP/2 start, the header names, order and spelling of that Chrome on that
system, and the user agent, client hints and `Accept-Language` of the page or
identity. A browser or system that was not measured is refused, not
approximated.

```rust
use seleniumbase_rs::impersonate::{HttpOptions, HttpSession};
use seleniumbase_rs::sb_cdp::Page;

# async fn demo(page: Page) -> Result<(), Box<dyn std::error::Error>> {
let session = HttpSession::from_page(&page, None, HttpOptions::new()).await?;
let reply = session.get("https://example.com/api/orders").await?;
let orders: serde_json::Value = reply.json()?;
println!("{orders}");
// Write the cookies that responses set back into the browser.
session.sync_cookies_back(&page).await?;
# Ok(())
# }
```

### Making one

- `HttpSession::from_page(&page, egress, options)` (async) copies the cookies of
  the page's browser profile into the jar, reads `navigator.userAgent` and
  `navigator.languages` from the page, and follows the route its traffic takes:
  the proxy of its browser context. `egress` is the exit the page leaves
  through; `None` takes the context's own proxy or goes direct. A browser-wide
  proxy, such as a MITM proxy the browser was launched with, is not visible to a
  page, so say so with `egress`.
- `HttpSession::from_identity(&identity, egress, options)` builds one from a
  [`StealthIdentity`](#identity-session-policy-and-the-identity-file): its
  fingerprint becomes the user agent, hints and languages, and its cookies fill
  the jar.

Both refuse, before any request, what would contradict the measured handshake:

- A Chrome of another major version than the measured one:
  `HttpError::VersionMismatch { browser, measured }`. For a page, both the user
  agent and `Browser.getVersion` are checked.
- A browser that is not Chrome (Edge, Opera, Firefox and the like), a Chrome
  whose user agent says `HeadlessChrome`, or a system that was not measured:
  `HttpError::Impersonate(..)`. A headless Chrome that was given no fingerprint
  announces itself, so give it one or run it headed.
- An identity of another egress than the one passed, or a page whose context
  proxy is another than `egress`: `HttpError::Identity(..)`.

### Requests

`session.get(url)` sends a `GET`. `session.request(method, url)` returns an
`HttpRequest`; nothing is sent until `.send().await`, and a mistake in a setter
is reported there.

```rust
use seleniumbase_rs::impersonate::{HttpSession, Method};
use seleniumbase_rs::wire::header_spec::RequestShape;

# async fn demo(session: HttpSession) -> Result<(), Box<dyn std::error::Error>> {
let reply = session
    .request(Method::POST, "https://example.com/api/search")
    .shape(RequestShape::Fetch)
    .header("x-requested-with", "XMLHttpRequest")
    .json(&serde_json::json!({ "q": "shoes" }))
    .send()
    .await?;
println!("{}: {}", reply.status(), reply.text()?);
# Ok(())
# }
```

- `GET` and `HEAD` carry the headers of a navigation (`RequestShape::Document`),
  every other method those of a script's request (`RequestShape::Fetch`);
  `.shape(..)` chooses. `.header`, `.body`, `.json`, `.form` and `.timeout`
  complete the request. `Method` is `wreq::Method`, re-exported.
- The profile owns `User-Agent`, `Sec-CH-UA*`, `Accept-Language`,
  `Accept-Encoding`, `Cookie` and `priority`, and the connection owns `Host`,
  `Content-Length` and the like. Setting one is `HttpError::ReservedHeader`, not
  a silent overwrite. Any other header replaces the profile's value in place or is
  added. `CONNECT`, `TRACE` and `TRACK` are refused, and so is a URL with
  credentials in it or a scheme other than http and https.
- A status other than 2xx is a reply, not an error. `HttpReply` has `status()`,
  `headers()` (`ReplyHeaders`), `bytes()`, `text()`, `json::<T>()`, `url()` after
  redirects (and `redacted_url()`), `redirects()`, `protocol()`, `elapsed()` and
  `detect()`.

### What the session does

- **Cookies.** The jar is the RFC 6265 [`CookieJar`](#the-cookie-jar). The
  session writes the `Cookie` header and reads `Set-Cookie` itself; wreq's own
  store is off. A cookie for a public suffix, for another host, or against the
  `Secure`, `__Host-` and `SameSite` rules is refused as a browser refuses it.
  `sync_cookies_back(&page)` writes only what this session changed into the
  browser and returns how many cookies it wrote or removed; if the browser
  refuses, the changes are kept for the next attempt.
- **Redirects** are followed by hand, up to 10 by default: the cookies of each
  hop are stored and sent per URL, `303` becomes a `GET`, and the headers you set
  are dropped when a hop leaves the origin (`Accept` and `Content-Type` stay). A
  redirect is followed where it points, a private address included, so a session
  that fetches URLs for someone else must filter them before it asks.
- **Limits** are `HttpOptions`, with defaults that are safe on the open web:

  | Setting | Default | Largest accepted |
  |---|---|---|
  | `timeout`, the whole request, redirects included | 30 s | 1 hour |
  | `connect_timeout` | 10 s | |
  | `max_body_bytes`, the reply after decompression | 64 MiB | 1 GiB |
  | `max_request_bytes` | 64 MiB | |
  | `max_redirects`, `follow_redirects` | 10, on | 30 |
  | `pacing(calls, window)`, per site | off | |

  Also `event_sink`, `trust(CertStore)` for a private root (certificate
  verification is never turned off), `resolve(host, addr)`, `bypass(host)`,
  `installed_chrome_major(major)` and `clock(..)`. `HttpOptions::validate`
  refuses a value out of range before a client exists.
- **Judging replies.** `reply.detect()` runs the [challenge
  detection](#challenge-detection-and-mitigation). `session.assess(&reply,
  &Attempt, &Rules, &identity)` adds what the `MitigationPolicy` and the
  `SessionPolicy` make of it, as a `Verdict { detection, action, transition }`;
  `session.fetch(url, &attempt, &rules, &identity)` sends a `GET` and judges it
  in one call (`Fetched { reply, verdict }`); `session.preflight(url, &identity,
  state, &rules)` asks, before a request, whether the identity's clearance is
  about to end so that the browser should take the URL first. They are the
  pieces of your own loop; [`StealthSession`](#stealthsession) is that loop.

  ```rust
  use seleniumbase_rs::impersonate::http_session::{Attempt, Rules};
  use seleniumbase_rs::impersonate::HttpSession;
  use seleniumbase_rs::scrape::StealthIdentity;

  # async fn demo(session: HttpSession, identity: StealthIdentity) -> Result<(), Box<dyn std::error::Error>> {
  let fetched = session
      .fetch("https://example.com/", &Attempt::default(), &Rules::default(), &identity)
      .await?;
  if fetched.verdict.is_clear() {
      println!("{}", fetched.reply.text()?);
  } else {
      println!("{:?}", fetched.verdict);
  }
  # Ok(())
  # }
  ```
- **Identity.** `export_identity(&base)` puts the session's cookies back into a
  `StealthIdentity` and refreshes its clearances. `egress()` is the exit without
  credentials, `cookie_count()` and `profile()` read the state. Cloning gives
  another handle to the same session.
- **Secrets.** Cookies, credentials and proxy passwords never reach `Debug`, logs,
  events or errors: URLs are redacted, a failure of the HTTP client is reduced to
  a word, and header values are not quoted.

### What to know

- **The first request to an origin is written for HTTP/2.** The `priority` header
  is a Chrome HTTP/2 header. The session cannot know the protocol before it
  connects, so when an origin turns out to speak HTTP/1.1 only, that first
  request carries an HTTP/2 `priority` header. The session then remembers the
  origin and writes the HTTP/1.1 headers from the second request on. The tests
  assert the second half (the second request has no `priority` header,
  `the_protocol_an_origin_settled_on_is_remembered` in
  `tests/impersonate_http/basics.rs`); the first request's header is what the
  module documentation states, and no test asserts it.
- It does not run JavaScript, solve a challenge, open a WebSocket, or pass
  canvas, WebGL or behavioural checks, and it does not make automation permitted
  by a site's terms. A matching handshake is one signal among many.
- **What was run against a real Chrome.** `tests/impersonate_http_chrome.rs`
  (`#[ignore]`d, headless Chrome 155 on macOS, against a loopback TLS site)
  shows that the real `navigator.userAgent`, `navigator.languages`,
  `Browser.getVersion` and `Network.getAllCookies` are what `from_page` reads,
  that a host-only cookie set by a response reaches Chrome as host-only, that a
  removal reaches it too, and that a page announcing itself as headless is
  refused. The rest of `HttpSession` is tested against loopback sites
  (`tests/impersonate_http.rs`) and a scripted browser, with no Chrome.

## The ephemeral-CA TLS server

`impersonate::tls_server` is the building block under `capture_h2` and under
[`MitmProxy`](#mitmproxy). `EphemeralCa::generate()` creates a P-256
certificate authority in memory for the life of one process. `leaf_for(host,
now)` issues a leaf for one DNS name or IP address (never a wildcard), cached in
a count-bounded LRU. `acceptor(&[ALPN_HTTP1])` builds the BoringSSL server
context (TLS 1.2 and 1.3, no session resumption), and `accept` runs one
handshake and refuses the connection when the client's SNI names another host.

```rust
use std::time::SystemTime;
use seleniumbase_rs::impersonate::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};

# async fn demo(tcp: tokio::net::TcpStream) -> Result<(), Box<dyn std::error::Error>> {
let ca = EphemeralCa::generate()?;
let chrome_flag = ca.chrome_spki_arg(); // pass to Chrome together with --user-data-dir
let server = acceptor(&[ALPN_HTTP1])?;
let host = url::Host::parse("example.com")?;
let leaf = ca.leaf_for(&host, SystemTime::now())?;
let tls = accept(&server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await?;
# drop((chrome_flag, tls));
# Ok(())
# }
```

Things to know before you use it:

- It is for a local browser talking to something on loopback. Nothing in the
  module binds a socket; the caller binds `127.0.0.1` only. The CA is never
  installed in a trust store, never written to disk, and `--ignore-certificate-errors`
  is never used.
- Chrome trusts the leaves through `--ignore-certificate-errors-spki-list` with
  the pin of the shared leaf key. **That switch trusts every certificate chain that
  contains the pinned public key, from any sender, and runs no other check.** A
  public key is not secret, so the pin is safe only while every connection of that
  Chrome profile ends at your server. The module keeps the trusted set as small
  as the switch allows (only the leaf key is pinned, and the server presents the
  leaf alone). That a chain carrying the pinned key is accepted is **measured on
  Chrome 155** (headless, macOS). In `tests/impersonate_mitm_chrome.rs`,
  `without_the_proxy_the_leaf_pin_alone_trusts_a_chain_carrying_the_leaf_key`
  starts Chrome with the pin and no proxy, and Chrome loads a page from a
  loopback server whose chain is its own self-signed certificate followed by the
  proxy's leaf. Behind the proxy the same server is never served to the page
  (`a_loopback_server_carrying_the_pinned_key_is_never_served_to_the_page`).
  Both tests are `#[ignore]`d because they start Chrome. They do not separate the
  individual checks the switch skips (signature, host name, validity,
  revocation); those stay as read in the Chromium source.
- The CA lives seven days by default (at most 90), a leaf one day. Private keys
  stay in BoringSSL memory and never appear in `Debug`, logs or errors. They are
  freed, not wiped, when the CA drops, because BoringSSL offers no zeroize hook.

## MitmProxy

`HttpSession` covers the requests you make yourself. `impersonate::mitm::MitmProxy`
covers the ones a real Chrome makes. It is a loopback forward proxy that ends
the browser's TLS with a leaf of the [ephemeral CA](#the-ephemeral-ca-tls-server)
and replays each request through the impersonating client, so the origin sees the
`ClientHello`, HTTP/2 start and header spelling of the profile (the measured
ones, minus the [known gaps](#limits)) instead of whatever the installed browser
would have sent. Both legs then look alike on the wire, and a clearance earned in
the browser is valid over `HttpSession`.

```rust
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::impersonate::{ImpersonationProfile, MitmOptions, MitmProxy};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::scrape::NoopEventSink;
use seleniumbase_rs::wire::BrowserKind;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let profile = ImpersonationProfile::measured(BrowserKind::Chrome)?;
let mitm =
    MitmProxy::start(profile, None, MitmOptions::default(), Arc::new(NoopEventSink)).await?;
let options = mitm.configure_launch(LaunchOptions::builder()).build()?;
let browser = Browser::launch(options).await?;
let page = browser.new_page(Some("https://example.com/")).await?;
# drop(page);
browser.close().await?;
mitm.shutdown(Duration::from_secs(5)).await;
# Ok(())
# }
```

`MitmProxy::start(profile, egress, options, sink)` binds `127.0.0.1` on a port the
system picks. `egress` is the proxy the replayed requests leave through (or
`None`); `sink` receives redacted events. `configure_launch` points a Chrome
launch at the proxy and adds the switches it needs.

### What the proxy does with each connection

1. It requires `Proxy-Authorization: Basic` with a random 128-bit token (user
   name `sb`), compared in constant time, and answers `407` otherwise. Chrome
   gets the token through `ProxyAuth` over `Fetch.authRequired`, so it is never
   on Chrome's command line.
2. It refuses a target it cannot intercept with an empty `502` and closes the
   connection. Nothing is ever tunnelled blind: bytes that are not a TLS
   handshake for the `CONNECT` host end the connection.
3. It answers `200` and ends the browser's TLS with a leaf for exactly the
   `CONNECT` host. The browser leg offers ALPN `http/1.1` only, and the SNI must
   name the same host.
4. It reads each request, drops the hop-by-hop and proxy headers, keeps Chrome's
   header order, adds the HTTP/2-only `priority` header from the measured table,
   and sends the request upstream with certificate verification on. The reply
   streams back without being buffered whole. An upstream failure, an untrusted
   certificate included, is an empty `502`.

An absolute-form `http://` request is checked the same way and forwarded without
`CONNECT`. A WebSocket handshake inside a tunnel is relayed (see
[WebSocket](#websocket) below). The browser only ever sees an empty status (`407`,
`400`, `403`, `413`, `431` or `502`); the reason is a `MitmError` variant in the log
(`Unauthorized`, `BadTarget`, `ForbiddenDestination`, `NotIntercepted`,
`HostMismatch`, `MalformedUpgrade`, `BodyTooLarge`, `VersionMismatch`,
`UpstreamRequest`). `start` itself can fail with `Bind`, `Tls`, `InvalidOptions`,
`Random`, `Upstream` or `Egress`.

### What Chrome must be told

`mitm.configure_launch(builder)` sets the proxy and adds `mitm.chrome_args()`:
the `--ignore-certificate-errors-spki-list` switch with the leaf pin only (never
the CA pin), `--proxy-bypass-list=<-loopback>` so loopback hosts go through the
proxy too, and `--disable-quic`. Because that switch trusts any chain that
carries the pinned key (see [the CA section](#the-ephemeral-ca-tls-server)), the
pin is safe only while every connection of the profile ends at the proxy.
Without the last two switches a loopback server or a QUIC connection would reach
Chrome directly. Chrome honours the pin only together with `--user-data-dir`, so
do not reuse that profile directory for a Chrome that runs without the proxy. The
proxy refuses a request whose `User-Agent` claims a Chrome major other than the
profile's.

### Bounds and options

`MitmOptions` sets the bounds; `MitmProxy::start` refuses a value outside them
with `MitmError::InvalidOptions`.

| Setting | Default | Range |
|---|---|---|
| `max_connections`, served at once (further clients wait in the listen queue) | 256 | 1 to 4096 |
| `max_header_bytes`, the request head (a larger one is answered `431`) | 64 KiB | 8 KiB to 1 MiB |
| `max_headers` | 128 | 1 to 1024 |
| `max_body_bytes`, each body in either direction (`413` when announced larger; cut when it grows past it) | 512 MiB | at least 1 |
| `idle_timeout`, a connection waiting for its next request | 60 s | not zero |
| `handshake_timeout`, the browser's TLS handshake | 10 s | not zero |
| `connect_timeout`, the upstream connection | 30 s | not zero |
| `read_timeout`, one upstream read | 60 s | not zero |
| `websocket_idle_timeout`, a relayed WebSocket carrying no byte either way | 300 s | not zero |
| `max_websocket_bytes`, each direction of a relayed WebSocket | 512 MiB | at least 1 |

`trust_upstream_root(der)` adds a root for upstream verification; once one is
added, only the added roots are trusted, not the built-in web roots, and
verification is never turned off. `resolve(host, addr)` pins a host name for the
upstream client, and `ca(Arc<EphemeralCa>)` shares a CA.

Bodies stream both ways; nothing is read whole into memory. `mitm.stats()` gives
counts (`connections`, `tunnels`, `unauthorized`, `refused`, `forwarded`,
`upstream_errors`, `websockets`).

### WebSocket

A page's `wss://` WebSocket reaches the proxy as a `CONNECT` whose TLS the proxy
ends, followed by the opening handshake (`GET` with `Upgrade: websocket` and
`Connection: Upgrade`, RFC 6455). The proxy:

1. checks the handshake like any request (the token, the destination, `Host`,
   the Chrome major), and that it is a whole handshake: a bodiless HTTP/1.1 `GET`
   with both `Upgrade: websocket` and `Connection: upgrade`. Half of one
   (`Connection: upgrade` without `Upgrade`, or the other way round), another
   method, HTTP/1.0 or a body is an empty `400`; an upgrade to anything else
   (`h2c`, `TLS/1.2`, a list) is an empty `502`;
2. sends it upstream over HTTP/1.1 with the measured WebSocket `ClientHello`
   (`tls_websocket` in the measured file, minus the known gaps). Chrome offers
   only `http/1.1` on a WebSocket connection, so this hello's JA4 is not the
   ordinary one. The egress, the destination filter, the trusted roots and the
   timeouts are those of ordinary requests;
3. keeps the browser's header order without the hop-by-hop and proxy fields, and
   writes the `Connection: Upgrade` and `Upgrade: websocket` pair itself. Header
   names take the measured spelling, and the handshake fields the measured lists
   do not hold take Chrome's (`Sec-WebSocket-Key`, `Sec-WebSocket-Version`,
   `Sec-WebSocket-Extensions`, `Origin`, ...);
4. relays the upstream's `101 Switching Protocols` with its own headers.
   `Sec-WebSocket-Accept` is checked by the browser, as it would be without a
   proxy. A `101` that switches to another protocol is an empty `502`, and any
   other answer (a `403`, say) reaches the page as an ordinary answer;
5. then copies bytes both ways without reading frames, until a side closes, no
   byte has passed for `websocket_idle_timeout`, one direction has carried more
   than `max_websocket_bytes` (the read that crosses the limit is not passed on,
   so one frame larger than the limit ends the connection), or the proxy stops.
   Both sides are then shut down.

Chrome sends no keep-alive of its own on a WebSocket, so a site that pings less
often than `websocket_idle_timeout` loses its idle connections. A custom
profile's WebSocket connections send the measured WebSocket hello of its base
browser. Only `wss://` gets through: a handshake outside a `CONNECT` tunnel is an
empty `502`, and a tunnel that does not start with TLS is never relayed. On
Chrome 155 a page's `ws://` WebSocket is refused by the proxy and never reaches
its server.

**A WebSocket must not be a tab's first request through the proxy.** Chrome gets
the proxy's token from `ProxyAuth`, which answers over `Fetch.authRequired`. Measured
on Chrome 155 (headless): a `wss://` WebSocket opened by a blank tab, before any
other request had gone through the proxy, got the `407` on its `CONNECT` and Chrome
did not retry it with the token, so the page saw an error. Once a request has
taken the proxy's credentials (any navigation through the proxy does, the page that
opens the WebSocket included), WebSockets work.

### Changing the upstream, and stopping

`swap_upstream(egress)` and `swap_profile(profile)` build the new upstream
client first and then replace the old one. Requests under way finish on the old
one, and the listener, its port and the browser's open connections stay.
`shutdown(grace)` stops accepting, lets requests finish for up to `grace`, then
aborts what is left; dropping the last clone stops it too.

### What is not intercepted

- **Upgrades other than WebSocket are refused**, and so is `ws://` (see
  [WebSocket](#websocket)).
- QUIC is off in Chrome, so there is no HTTP/3.
- **The first request to an HTTP/1.1-only origin carries an HTTP/2 `priority`
  header.** The upstream protocol is settled by ALPN after the request was
  prepared, and the first request to an origin is prepared for HTTP/2. The proxy
  then remembers the origin and later requests do not carry it. On HTTP/1.1 the
  header names take the measured spelling; a name outside the measured lists
  keeps the lower case the browser leg delivers.

### What was run

`tests/impersonate_mitm.rs` runs the proxy against loopback sites, an egress
proxy and raw clients, with no browser: the token and the `407`, targets it
refuses, the SNI check, the bounds, the measured JA4 and Akamai string and
`priority` at the origin, header order, the refusals above, a 64 MiB body
streaming through, swapping the egress, and shutdown leaving no task. Its
`websocket` tests run a WebSocket echo site written in the test (raw tokio, no
WebSocket library on the site's side): the echo, a handshake completed by
`tokio-tungstenite` as an independent client, the measured WebSocket JA4 at the
site, the handshake's header order, a WebSocket through an egress proxy with its
credentials, and the hostile inputs (half upgrades, other
protocols, a body, an oversize frame, an idle peer, a name resolving to loopback
without `allow_private_destinations`, a handshake outside a tunnel, and a proxy
that stops under an open WebSocket).

`tests/impersonate_mitm_chrome.rs` drives the real Chrome 155 on macOS, headless,
and is `#[ignore]`d. A page loads through the proxy, and the loopback origin
sees the measured JA4 and Akamai string with the measured `priority` and a
Chrome user agent; the token reaches Chrome only through the `407`. A page
opens a `wss://` WebSocket through the proxy to a loopback echo site and gets its
echo back; the site sees the measured WebSocket JA4 and Chrome's handshake fields
in Chrome's order. A `ws://` WebSocket is refused by the proxy, and a WebSocket
that is a tab's first request fails on the `407` (pinned, see above). Two more
tests are about the pin: a loopback server carrying the pinned key is never
served to the page behind the proxy, and the same server is served to a Chrome
that has the pin and no proxy. The origin in these tests is a server of the test,
not a live site.

## StealthSession

`impersonate::stealth_session::StealthSession` joins the pieces. It presents one
`StealthIdentity` (the fingerprint, the measured browser it belongs to, the
cookies it earned and the egress they were earned through) in one of two modes
and moves between them by the policies:

- `Mode::Http`: an [`HttpSession`](#httpsession) sends the requests. It is fast
  and cheap, with the measured handshake and headers.
- `Mode::Browser`: a real Chrome sends them, behind a
  [`MitmProxy`](#mitmproxy) that replays each request with the same measured
  handshake. A clearance earned here is valid in the other mode.

```rust
use seleniumbase_rs::impersonate::StealthSession;
use seleniumbase_rs::scrape::CountryCode;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let mut session = StealthSession::builder()
    .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
    .build()
    .await?;
let page = session.fetch("https://example.com/").await?;
println!("{} bytes, {:?} mode", page.body.len(), page.mode);
session.close().await?;
# Ok(())
# }
```

### The loop

A session starts in the mode its `SessionPolicy` prefers (HTTP by default).
`fetch(url)` asks the policies three questions:

1. **Before the request**, `SessionPolicy::before_fetch`: does the identity hold
   a clearance for the site that is about to end, must it hold one and does not,
   or does the site's history say plain requests fail? Then the session goes to
   the browser first.
2. **After the answer**, `MitigationPolicy::decide`: a real page ends the loop; a
   challenge met over HTTP sends the session to the browser (an escalation); in
   the browser the challenge is solved and the cookies move into the identity; a
   block or a rate limit starts a cooldown, rotates the egress or the profile, or
   stops the fetch.
3. **After a solve**, `SessionPolicy::after_fetch`: a browser that earned a
   clearance hands the identity back to HTTP (a demotion), which carries it on.

Every change of mode emits `ScraperEvent::ModeChanged` with its reason. The
browser is closed when the session goes back to HTTP and started again, with the
same identity and egress, when it is needed. **The MITM proxy is started the first
time a browser is needed** (at `build()` when the policy prefers the browser) and
lives until `close()`, so its port and its certificate stay the same across
browsers.

A site in cooldown is not requested at all. The state of the site (successes,
challenges, cooldown) is read before the first load and saved after the last,
whether the fetch worked or not. The attempts per fetch default to 4
(`DEFAULT_ATTEMPTS`, one more than a browser-only scraper): one load meets the
challenge over HTTP, one in the browser solves it, and one over HTTP uses the
clearance.

`fetch` returns a `StealthFetch`: `url` after redirects, `status`, `headers`, the
`body` (the response body over HTTP, the rendered page in the browser), the
`detection`, the `mode` it was got in, how many loads it took (`attempts`), the
challenge `solved` on the way, and the `egress`. A page that took a challenge to
reach is usually an HTTP page: the browser earned the clearance and the session
went back to HTTP to use it. A body that is not UTF-8 is read lossily.

### Building one

`StealthSession::builder()` has a default for everything: Chrome with the
platform's launch options, no proxy, state in memory, events written to `tracing`,
HTTP preferred, and a profile drawn from a random seed.

| Setting | What it does |
|---|---|
| `launch_options(builder)`, `launcher(..)` | How the browser starts. A setting made in the options (a user agent, say) wins over the fingerprint's, and the proxy is always the MITM proxy's. |
| `with_geo_proxies([(spec, country)])`, `proxy_strategy(..)` | The egress pool and how it picks, as for `BrowserScraper`. Both legs leave through the same egress. |
| `with_max_challenge_attempts(n)`, `mitigation(policy)` | Loads per fetch (zero reads as one) and the whole mitigation policy. |
| `policy(SessionPolicy)` | The mode it prefers, whether HTTP needs a clearance, the clearance margin, whether a solve sends it back to HTTP. |
| `with_state_store(..)`, `with_event_sink(..)`, `geo_resolver(..)`, `per_site_rate(calls, window)` | As for `BrowserScraper`. The HTTP leg and the proxy report to the same sink. |
| `seed(..)`, `fingerprint(..)` | The first profile. A fingerprint must claim the measured Chrome. |
| `restore(identity)` | Starts from a saved identity: its fingerprint, cookies and clearances. It must belong to one of the proxies added, or to the direct connection when there is none. |
| `http_options(HttpOptions)`, `mitm_options(MitmOptions)` | The limits of each leg. The session's clock and event sink replace the ones in the HTTP options. |
| `solve_timeout(..)`, `clock(..)` | How long a solve is waited for, and the clock. |

`build()` fails with `StealthError::Impersonate` when this machine's system has no
measured Chrome or the profile claims another version, `StealthError::Scrape` for
an egress whose country has no locale, an identity that belongs to another egress
or no browser to start, `StealthError::Http` and `StealthError::Mitm` for the
legs.

### Moving by hand

| Call | What it does |
|---|---|
| `escalate()` | Goes to the browser now, with the identity as it stands. The cookies of the HTTP session move into the browser. |
| `demote()` | Goes back to HTTP now, with the cookies the browser holds, and closes the browser. A browser that cannot be read (it crashed) does not trap the session: the identity as of the last read is used. |
| `rotate_profile()` | A new seed derived from the old one, a new fingerprint for the same operating system that claims the measured Chrome. Cookies and clearances are dropped; what is known of each site stays. In browser mode a new Chrome starts behind the same proxy, on the same port. |
| `rotate_egress()` | Another egress of the pool, with the same fingerprint in the language and time zone of the new country. The proxy is pointed at it first, so its port stays. Cookies and clearances are dropped, because a clearance belongs to the address that earned it. `NoEgressAvailable` when the pool has nothing else. |
| `export_identity()`, `import_identity(&id)` | Take the identity out of the session, or put one in. It must belong to the egress in use. |
| `mode()`, `identity()`, `egress()`, `seed()`, `http()`, `page()` | Read the state. `http()` is the `HttpSession` in HTTP mode, for requests of your own that carry the earned cookies; `page()` is the tab in browser mode. |
| `close()` | Closes the browser if one runs and stops the proxy. A dropped session leaves both to their own drop. |

On an error from a rotation, an import or a change of mode the session is as it
was.

### Errors

`StealthError` has one variant per layer: `Scrape` (the browser leg, the site
state, an identity or the mitigation policy; `Unsolved`, `GaveUp` and
`CoolingDown` come as this variant), `Http`, `Mitm` and `Impersonate`. Each
already keeps URLs redacted and credentials out of its text. At the crate
boundary it maps to `SeleniumBaseError`.

### What to know

- The measured Chrome and the installed Chrome must have the same major version.
  A browser that is another version is refused before it loads a page
  (`StealthError::Impersonate`).
- A launched browser lives until the session leaves the browser mode or is
  closed. There is no idle timeout and no deadline in the session.
- It runs the same fetch loop as `BrowserScraper`, with plain HTTP as the first
  move. A matching handshake is one signal among many, and a session does not
  make automation permitted by a site's terms. See [Limits](#limits).

### What was run

`tests/stealth_session.rs` runs the real HTTP leg against a loopback TLS shop
that trusts only its own ephemeral authority, with a scripted browser
(`Browser::new_mocked`, feature `test-util`): every transition of the state
machine and its events, export and restore, rotating the profile and the egress,
and that secrets stay out of output. The MITM proxy is started and the browser
is launched behind it, but the traffic of a browser is not made.

`tests/stealth_session_chrome.rs` runs the story with the real Chrome 155 on
macOS, headless (`#[ignore]`d): the session asks over HTTP and meets a script
challenge on a loopback TLS shop; it starts the proxy and a Chrome behind it with
the same fingerprint and egress; Chrome runs the challenge page's script, which
earns the clearance cookie; the cookies move into the identity, Chrome closes,
and the session asks again over HTTP with the clearance. The shop sees one
`ClientHello` shape and one user agent from both legs. A second test takes a
clearance near its end back to Chrome, and a third shows that a rotated profile
is what the page sees while the proxy stays. The shop is a stand-in written for
the test: no vendor's challenge was run.

## Runnable examples

Five small programs in `examples/` run the layers on your own machine. None of them
reaches the outside network or targets a third-party site, and none tries to pass a
real detection service: each one needs nothing, or starts its own site on
`127.0.0.1`. That site, the "shop" of `http_session` and `stealth_session`
(`examples/stealth_support/shop.rs`), is a stand-in written for the examples. It is
not any vendor's challenge, and getting past it says nothing about a real service.

| Example | Feature | Needs | What it shows |
|---|---|---|---|
| `examples/cookie_jar.rs` | `scrape` | Nothing | Made-up `Set-Cookie` headers into the [cookie jar](#the-cookie-jar): which are stored, which are refused and why, and the `Cookie` header for five requests. |
| `examples/custom_profile.rs` | `impersonate` | `cmake`; Chrome 155 on macOS | The measured profile, with an Akamai string and a JA3 laid over it with `CustomProfileBuilder`, and four inputs the stack [refuses](#what-is-refused). |
| `examples/http_session.rs` | `impersonate` | `cmake`; Chrome 155 on macOS | An [`HttpSession`](#httpsession) fetching from a loopback TLS shop, its jar carrying the cookie the shop sets, and the JA4 the shop saw next to the measured one. |
| `examples/stealth_session.rs` | `impersonate` | `cmake`; Chrome 155 on macOS, installed | A [`StealthSession`](#stealthsession) that meets a challenge over HTTP, goes to a headless Chrome behind the proxy, comes back, and prints its changes of mode. |
| `examples/browser_scraper.rs` | `scrape` | A Chrome | A [`BrowserScraper`](#browserscraper) detecting and solving the same kind of challenge in a headless Chrome. |

```text
cargo run --features scrape      --example cookie_jar
cargo run --features impersonate --example custom_profile
cargo run --features impersonate --example http_session
cargo run --features impersonate --example stealth_session
cargo run --features scrape      --example browser_scraper
```

The three that use the measured emulation stop at once with the `NotMeasured`
message on any system but macOS: they do not borrow the values of another system.
`stealth_session` also needs the installed Chrome to be the measured major, or it is
refused before it loads a page. The two that drive Chrome should run one at a time,
and not beside another Chrome-driving program. The `impersonate` ones start from the
[feature costs](#two-features); the first build compiles BoringSSL.

What was run: each example was run once on macOS with Chrome 155.0.8059.40, the two
that drive Chrome headless, and each exited with status 0. `browser_scraper` solved
the challenge in about half a second and loaded the page twice; `stealth_session`
escalated to Chrome, came back to HTTP, and in two runs the shop saw one JA4 and one
user agent on every request of both legs (six and seven requests, as Chrome's favicon
requests come and go);
`http_session` printed a JA4 that differs from the measured one by the known gaps
(see [Limits](#limits)). Nothing was run on another system, so the `NotMeasured` stop
of the three is read from the code and was not observed.

## Limits

State these plainly to yourself before you build on any of this.

- **macOS and Chrome 155 only.** Exactly one browser and system pair is measured:
  Chrome 155.0.8059.40 on macOS. Every other pair is refused with
  `NotMeasured`. When Chrome updates, the profile does not follow until you
  re-measure.
- **Known gaps between Chrome's handshake and ours.** The pinned BoringSSL and
  wreq cannot send three things Chrome 155 sends:
  - `TrustAnchorsExtension`: the `trust_anchors` extension (`0xCA34`). It needs a
    newer `btls` and `wreq` than the exact pins. Our JA4 therefore differs from
    Chrome's in the extension count and the `c` hash.
  - `MlDsaSignatureAlgorithms`: the ML-DSA signature schemes (`0x0904` to
    `0x0906`) are missing from our signature algorithms, which changes the `c`
    hash too.
  - A **GREASE signature algorithm**: Chrome writes one first, and the vendored
    BoringSSL writes none. This one is not a `KnownGap` variant. JA3 and JA4 strip
    GREASE, so only the peetprint shows it. A test pins the difference and fails
    when a newer BoringSSL closes it.

  The first two are recorded in the measured file's `provenance.known_gaps`; the
  tests assert that the difference between Chrome and our client is exactly
  these, and fail when one disappears.

  `HttpSession` and `MitmProxy` send the handshake of this client, so the gaps
  apply to them. Behind the `MitmProxy` the origin sees our handshake, gaps
  included, and not Chrome's own, which `BrowserScraper` leaves with unchanged.
  That is the price of one wire identity in both modes of a `StealthSession`.
- **No HTTP/3.** wreq has none, and Chrome behind the proxy has QUIC switched off.
- **WebSocket does not pass the `MitmProxy`.** An upgrade request is answered with
  an empty `502`, so a page behind the proxy, and so the browser leg of a
  `StealthSession`, cannot open a WebSocket. `HttpSession` opens none either.
- **The first request to an HTTP/1.1-only origin carries an HTTP/2 `priority`
  header**, in `HttpSession` and in `MitmProxy`, because the protocol is known
  only after the connection opens. Later requests to that origin do not carry it.
- **Headless, one system.** The tests that drive Chrome through `MitmProxy`,
  `HttpSession::from_page` and `StealthSession` launch it headless on macOS with
  Chrome 155; none drives them headed. The measured tables were captured from a
  headed Chrome, and headless and headed Chrome differ.
- **`HttpSession` follows redirects where they point,** a private address
  included. Filter the URLs before the session sees them if they are not yours.
- **Tested against local captures and servers, not against live services.** The
  wire layer is tested against committed captures and, in tests marked
  `#[ignore]`, against a local Chrome. The `BrowserScraper` flows are tested with
  a scripted browser and with ignored tests that drive a local Chrome.
  `HttpSession`, `MitmProxy` and `StealthSession` are tested against loopback TLS
  sites and a challenge page written for the tests, with a scripted browser, and
  in ignored tests with the local Chrome. Nothing here was checked against a live
  bot-protection service. The identity file's permissions check is Unix only.
- **No guarantee against any detection service.** A matching handshake is one
  signal among many: the IP address, the behaviour, the account and the page's
  own checks all count. Nothing here promises to pass a given service, and
  services change.
- **Scrape within the rules.** Follow each site's terms of service and `robots`
  rules, respect rate limits, and obey the law where you and the site are. A
  matching fingerprint does not make automation permitted. The library does not
  decide that for you.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `NotMeasured { kind, os }` | The browser and system pair has no capture. | Run the capture tools on that system, or use the measured one. |
| `VersionMismatch { browser, measured }` | The identity's user agent claims another Chrome major than the measured profile. | Build the identity with `Fingerprint::randomized_with_major(os, seed, measured_major)`, or re-measure. |
| `UnsupportedExtension`, `UnsupportedSigalg`, `UnsupportedSetting` | The input asks for something BoringSSL or wreq cannot send. | Remove it. The error names the id. |
| The build fails while compiling BoringSSL | `cmake` or a C/C++ toolchain is missing. | Install them, or drop the `impersonate` feature. |
| `IdentityFileError::Permissions` | Group or others can read the identity file. | `chmod 600` it, or delete it and make a new one. |
| `ScrapeError::CoolingDown` | The site answered with a block or rate limit and is being left alone. | Wait until the time in the error (seconds since the Unix epoch), or give the scraper another egress. |
| `ScrapeError::Unsolved` | A challenge was still there after the attempts allowed. | Raise `with_max_challenge_attempts`, add egresses, or accept that this page needs a person. |
| `ScrapeError::NoEgressAvailable` | A rotation was asked for and the pool has nothing else in rotation. | Add proxies with `with_geo_proxies`. |
| `HttpError::VersionMismatch`, `StealthError::Impersonate` | The page, the identity or the installed Chrome is another major than the measured one. | Update or downgrade Chrome to the measured major, or run the capture tools again. |
| `HttpError::Impersonate` for a page | The page announces itself as `HeadlessChrome`, is not Chrome, or runs on a system that was not measured. | Give the browser a fingerprint or run it headed; use the measured system. |
| `HttpError::ReservedHeader` | A request set a header the profile owns (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`, `Accept-Encoding`, `Cookie`, `priority`). | Remove it. Change the identity instead. |
| `HttpError::RedirectRefused`, `TooManyRedirects` | A hop led somewhere the session refuses, or past the limit. | Request the final URL, or raise `max_redirects`. |
| `HttpError::Identity(EgressMismatch)` | The identity or the page belongs to another egress than the one passed. | Pass the egress the cookies were earned through. |
| Chrome behind `MitmProxy` shows a `502` page | The proxy refused the request or the upstream failed (an untrusted certificate, a WebSocket upgrade, a version mismatch). | Check `mitm.stats()` and the log; `trust_upstream_root` for a private site. |

---

Verified against commit `4184e74` and the re-exports added after it, by reading
`src/wire/**`, `src/scrape/**`, `src/impersonate/**`, `examples/capture_*.rs`,
`src/wire/measured/chrome-155-macos.json` and the tests that pin the behaviour
described here. The code samples were checked against the signatures in those
files; they were not compiled or run for this check. The real-Chrome tests named
here are `#[ignore]`d and were not run for this check.
