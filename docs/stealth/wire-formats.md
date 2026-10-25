---
title: "Wire Formats and Impersonation"
sidebarTitle: "Wire formats"
---

Examples importing `seleniumbase_rs_stealth::mimic` require the facade's
`mimic` feature. See the [single-dependency setup](/stealth/overview#which-crate-to-depend-on);
this feature builds BoringSSL and needs cmake and a C/C++ toolchain.

<Badge color="orange">Experimental</Badge>

<Visibility for="agents">

Summary for AI exports: every wire value comes from a capture tool and is stored as JSON in `crates/codec/src/measured`. Today the only entry is Chrome 155.0.8059.40 on macOS (captured 2026-10-11); any other system is refused with `NotMeasured`. `seleniumbase_rs_codec` parses the TLS `ClientHello`, JA3, JA4, peetprint and Akamai strings. `seleniumbase_rs_mimic` turns a measured `ImpersonationProfile` into a wreq client, and `ImpersonationProfile::custom()` layers your own inputs over it, refusing what BoringSSL and wreq cannot send.

</Visibility>

## What is measured, and what is refused

Every wire value this crate imitates was measured from a real browser by a
capture tool and stored as JSON in `crates/codec/src/measured`. Nothing is typed by
hand. The files are embedded with `include_str!`, parsed once, and looked up by
browser and operating system.

Today there is one entry: **Chrome 155.0.8059.40 on macOS**, captured on
2026-10-11. Its record holds the user agent, the TLS spec (and the one Chrome
uses for WebSocket), the HTTP/2 start, the header lists and their order, the
client hints, and where it came from:

```rust
use seleniumbase_rs_stealth::stealth::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::emulation::{self, EmulationError};
use seleniumbase_rs_stealth::codec::BrowserKind;

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
`SpecError::NotMeasured`; `codec::emulation::SpecError` is an alias for
`EmulationError`. The TLS and HTTP/2 refusals live in two other enums with the
same name, `codec::tls_spec::SpecError` and `codec::h2_spec::SpecError`.

<Note>

Why an unmeasured system is refused rather than approximated: a made-up persona
is worse than no persona. A TLS stack that claims to be Chrome on Windows with
values copied from Chrome on macOS is a contradiction a detector can see, and
the crate has no way to know which of the two differ. Windows, Linux, Android,
iOS and Safari stay refused until someone runs the capture tools on that system
and commits the result.

</Note>

The same rule shows up higher up:

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

<Warning>

The measured file describes the Chrome that was installed when it was captured.
When Chrome moves to a new major version, nothing updates the file for you: the
profile still describes Chrome 155, and a site sees the difference.

</Warning>

To add the new version:

<Steps>

<Step title="Update Chrome" icon="download">

Update Chrome. The tools start their own browser (the system Chrome, or the
one `--chrome PATH` names), headed. Leave `--headless` off: a headless
Chrome reports a different identity, so `capture_hints` refuses it
(`CaptureError::Headless`) and the others do not measure its identity.

</Step>

<Step title="Run the three tools" icon="terminal">

Run the three tools in this order, from the repository root. Each extends the
same file, named `<browser>-<major>-<os>.json`, and `capture_h2` refuses a
Chrome of another version than the file it extends.

```text
cargo run -p seleniumbase-rs-scrape      --example capture_tls
cargo run -p seleniumbase-rs-scrape      --example capture_hints
cargo run -p seleniumbase-rs-mimic --example capture_h2
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
  CA](/stealth/mitm-proxy#the-ephemeral-ca-tls-server), starts Chrome so that it trusts only
  that CA, and reads the first request on each of five connections (a
  navigation, a `fetch`, an image, a script, a stylesheet), once over HTTP/2
  and once over HTTP/1.1. `--h1` and `--h2` run one pass only.

They share these options: `--out DIR` (default `crates/codec/src/measured`),
`--fixtures DIR` (default `tests/fixtures/measured`, for the raw bytes),
`--chrome PATH`, `--lang TAG` (sets `Accept-Language`, default `en-US`),
`--probes N`, `--timeout SECS`, `--headless`, `--no-sandbox`, `--snippet` and
`--help`.

</Step>

<Step title="Register the new file" icon="file-code">

Add the new file to the `SOURCES` list in `crates/codec/src/emulation.rs`. The test
`crates/codec/tests/wire_emulation.rs` fails when a file in `crates/codec/src/measured` is missing
from that list.

</Step>

<Step title="Run the wire tests" icon="flask">

Run the wire tests, including `crates/mimic/tests/impersonate_wire.rs`. They compare what
our client sends with the measured file and fail, by name, when a known gap
closes or a new difference appears.

</Step>

</Steps>

A new Windows or Linux entry comes the same way: run the tools on that system.
Safari has no tool yet beyond the `capture_hints` console snippet.

## Reading the wire formats

All of `seleniumbase_rs_codec` is pure: no sockets, no clock, no files, and
`#![forbid(unsafe_code)]`. Every parser bounds its input and returns an error
for hostile or truncated bytes instead of panicking.

| Format | Type | What it is |
|---|---|---|
| TLS `ClientHello` | `codec::client_hello::ClientHello` | `ClientHello::parse(records)` decodes the first handshake message, across TLS records. `ClientHello::needed_len(prefix)` says how many bytes a capture loop still needs. |
| JA3 | `codec::ja3::Ja3` | Five comma-separated fields with <Tooltip tip="Random reserved values a client adds to its handshake so servers do not depend on a fixed list (RFC 8701).">GREASE</Tooltip> removed. `Ja3::parse`, `Ja3::of(&hello)`, `hash()` (MD5, as JA3 defines it). |
| JA4 | `codec::ja4::Ja4` | `a_b_c` of FoxIO's TLS-client definition. `Ja4::of`, `Ja4::of_original_order` (`JA4_o`), `Ja4::parse`, `diff`. |
| `JA4_r`, `JA4_ro` | `codec::ja4::Ja4Raw` | The unhashed, sorted or wire-order variants. `Ja4::raw(&hello)`, `Ja4Raw::parse`, `Ja4Raw::hashed()`. |
| peetprint | `codec::ja3::Peetprint` | A JA3-like format that also records signature algorithms. GREASE shows as `GREASE`. |
| Akamai HTTP/2 | `codec::akamai::Akamai` | `SETTINGS \| WINDOW_UPDATE \| PRIORITY \| pseudo-header order`. |

Only JA4 itself is implemented. JA4H, JA4S, JA4X and the other JA4+ methods carry
a different licence and are not part of this crate.

```rust
use seleniumbase_rs_stealth::codec::akamai::Akamai;
use seleniumbase_rs_stealth::codec::ja3::Ja3;
use seleniumbase_rs_stealth::codec::ja4::Ja4;

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

<Tip>

Chrome shuffles its TLS extensions on every connection, so its JA3 changes from
one connection to the next by design. JA4 sorts them first and stays stable.
Compare JA4 when you compare a browser with a client.

</Tip>

The spec types sit on top of the parsers. A `TlsSpec`
(`TlsSpec::from_client_hello`) is the whole TLS profile as plain data, and an
`H2Spec` (alias `Http2Spec`; `H2Spec::from_capture`, `H2Spec::from_akamai`) is
the HTTP/2 start. Each has `check_reproducible()`, which says whether the
BoringSSL and wreq stack can send it, and names what it cannot.

## An impersonating client

The `mimic` crate turns a measured profile into a wreq client:

```rust
use seleniumbase_rs_stealth::mimic::client::{
    impersonation_client, ImpersonationProfile, WireProtocol,
};
use seleniumbase_rs_stealth::codec::header_spec::RequestShape;
use seleniumbase_rs_stealth::codec::BrowserKind;

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
  [Limits](/stealth/limits#limits)) and keeps extension permutation on, as Chrome does.
- Cookies, redirects, body limits and the hand-over from a browser are not part
  of this client. That is what [`HttpSession`](/stealth/http-session#httpsession) is for.

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
use seleniumbase_rs_stealth::mimic::client::ImpersonationProfile;

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
`crates/mimic/tests/impersonate_wire.rs` proves on the wire that an accepted input arrives as
given. The errors are variants of `ImpersonateError`: `Tls(..)` wraps
`codec::tls_spec::SpecError`, `Http2(..)` wraps `codec::h2_spec::SpecError`,
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
use seleniumbase_rs_stealth::mimic::client::{ImpersonateError, ImpersonationProfile};
use seleniumbase_rs_stealth::codec::tls_spec::SpecError;

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

<Info>

A refusal is not a bug to work around. It means the pinned BoringSSL and wreq
cannot send what you asked for, and sending something close would be a different
fingerprint from the one you named.

</Info>
