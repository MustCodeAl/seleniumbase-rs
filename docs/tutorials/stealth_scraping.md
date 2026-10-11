# Stealth Scraping: Wire Fingerprints, Identities and the Browser Scraper

A site can tell a client apart before any page script runs: by the TLS
`ClientHello`, by the HTTP/2 connection start, and by the order and spelling of
the request headers. [Fingerprint & Stealth Profiles](fingerprint_stealth.md)
covers what a page can read from inside the browser. This page covers the
other side, the wire, and the pieces that keep an identity, its cookies and its
exit address together while a scraper works through a site's defences.

Everything here is young. The parts that exist are tested against local
captures and a local Chrome, on one system. Read [Limits](#limits) before you
rely on any of it.

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
| `HttpSession` | `impersonate` | NOT AVAILABLE YET (in progress) |
| `MitmProxy` | `impersonate` | NOT AVAILABLE YET (in progress) |
| `StealthSession` | `impersonate` | NOT AVAILABLE YET (in progress) |

The last three are being written. This page does not describe their API, and
nothing in the crate sends a request through an impersonating client for you
yet: the client builder below gives you the wire behaviour, and you do the
rest.

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
- What the ephemeral-CA TLS server is for.

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
| `impersonate` | `seleniumbase_rs::impersonate`: the wreq client, the conversion from a spec to wreq options, `capture_h2`, and the ephemeral-CA TLS server. | Builds BoringSSL: `cmake` and a C/C++ toolchain, and about 1 GB of build output (the design plan's estimate). `wreq` and `btls` are pinned to exact versions (`=6.0.0-rc.31` and `=0.5.6`). |

Neither feature is on by default. The new types are not re-exported at the crate
root; use the module paths shown below. The crate links only one BoringSSL, so
do not add a second crate that links it to the same build.

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
  of this client. That is what `HttpSession` is for, and it is not available
  yet. You can keep cookies with the [jar](#the-cookie-jar) in the meantime.

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
are in the module documentation. Nothing that is available today applies these
transitions: `BrowserScraper` stays in the browser, and the sessions that switch
modes are the part that is not available yet.

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

## The ephemeral-CA TLS server

`impersonate::tls_server` is the building block under `capture_h2` and under the
MITM proxy that is being written. `EphemeralCa::generate()` creates a P-256
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
  leaf alone), and documents that this behaviour was read from the Chromium source,
  not measured on Chrome 155.
- The CA lives seven days by default (at most 90), a leaf one day. Private keys
  stay in BoringSSL memory and never appear in `Debug`, logs or errors. They are
  freed, not wiped, when the CA drops, because BoringSSL offers no zeroize hook.

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
- **No HTTP/3.** wreq has none, and Chrome behind a proxy skips QUIC too.
- **Three parts are not written yet.** `HttpSession`, `MitmProxy` and
  `StealthSession` are in progress. Until they land there is no session that
  carries a clearance from the browser to plain requests, and no proxy that lets
  a real Chrome leave with an impersonated handshake.
- **Tested against local captures, not against live services.** The wire layer
  is tested against committed captures and, in tests marked `#[ignore]`, against
  a local Chrome. The `BrowserScraper` flows are tested with a scripted browser
  and with ignored tests that drive a local Chrome. Nothing here was checked
  against a live bot-protection service. The identity file's permissions check
  is Unix only.
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

---

Verified against commit `c5a6f2f`, by reading `src/wire/**`, `src/scrape/**`,
`src/impersonate/**`, `examples/capture_*.rs`, `src/wire/measured/chrome-155-macos.json`
and the tests that pin the behaviour described here. The code samples were
checked against the signatures in those files; they were not compiled or run for
this check.
