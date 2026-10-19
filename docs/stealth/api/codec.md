---
title: "seleniumbase-rs-codec"
sidebarTitle: "codec"
description: "API reference of seleniumbase-rs-codec: the pure wire formats of the stealth layer. ClientHello, JA3, JA4, peetprint, Akamai HTTP/2 strings, HTTP/2 frames and HPACK, the TLS, HTTP/2 and header specs, and the measured browser tables."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase-rs-codec` (library `seleniumbase_rs_codec`, in `crates/codec` of the
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
repository) holds the wire formats of the stealth layer. It is pure: no sockets,
no clock, no files, and `#![forbid(unsafe_code)]`. Every parser bounds its input
and returns an error for hostile or truncated bytes instead of panicking. The
measured browser table is embedded from `src/measured/*.json`; today it holds one
entry, Chrome 155.0.8059.40 on macOS. The guide that explains these formats is
[Wire formats](/stealth/wire-formats).

<RequestExample>

```rust The measured Chrome
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs_codec::{get_emulation, BrowserKind, EmulationError};

fn main() -> Result<(), EmulationError> {
    let chrome = get_emulation(BrowserKind::Chrome, OsType::Macos)?;
    println!("{} {}", chrome.provenance.browser, chrome.tls.ja4());
    Ok(())
}
```

```rust Parse wire strings
use seleniumbase_rs_codec::{Akamai, Ja3, Ja4};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let akamai: Akamai = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p".parse()?;
    let ja4: Ja4 = "t13d1517h2_8daaf6152771_cb7bf5808d99".parse()?;
    let ja3 = Ja3::parse("771,4865-4866-4867,0-23,29-23,0")?;
    println!("{} {} {}", akamai.window_update, ja4.a, ja3.hash());
    Ok(())
}
```

</RequestExample>

<ResponseExample>

```text The measured Chrome
Chrome 155.0.8059.40 t13d1517h2_8daaf6152771_cb7bf5808d99
```

</ResponseExample>

## Depend on it

The crate is not on crates.io. Cargo finds it inside the repository by its
package name:

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-codec = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Write `seleniumbase-rs` by its plain git URL, as above (no `branch`, `tag` or
`rev`): the codec takes the core the same way, so Cargo resolves both to one
commit and types such as `OsType` and `ClientHints` match. Its other
dependencies are `ring`, `serde`, `serde_json` and `thiserror`; there is no
BoringSSL here.

## Features

<ParamField body="test-fixtures" type="feature" default="off">

Turns on the `ClientHello` byte builders of the tests
(`client_hello::fixtures`, hidden from the documentation), for the tests of the
crates built on this one. Test support only: no release build turns it on. The
crate has no default features.

</ParamField>

## Module map

| Module | What it holds | Page |
|---|---|---|
| `emulation` | The measured browser tables, embedded from JSON, and their lookups. | [Emulation](/stealth/api/codec-emulation) |
| `client_hello` | The TLS `ClientHello` parser, `ext_id` constants and GREASE helpers. | [ClientHello](/stealth/api/codec-client-hello) |
| `ja3`, `ja3::peetprint` | JA3 and the peetprint, parsed, rendered and hashed. | [JA3 and peetprint](/stealth/api/codec-ja3) |
| `ja4` | JA4, `JA4_o`, `JA4_r` and `JA4_ro`. | [JA4](/stealth/api/codec-ja4) |
| `akamai` | Akamai HTTP/2 fingerprint strings. | [Akamai](/stealth/api/codec-akamai) |
| `h2_frames`, `h2_frames::hpack`, `h2_frames::huffman` | HTTP/2 frame reading, the client preface, HPACK and Huffman decoding. | [HTTP/2 frames](/stealth/api/codec-h2-frames) |
| `tls_spec` | The TLS profile a client must send, and the custom JA3, JA4 and peetprint overlays. | [TlsSpec](/stealth/api/codec-tls-spec) |
| `h2_spec` | The HTTP/2 profile: SETTINGS, window, PRIORITY, pseudo-header order. | [H2Spec](/stealth/api/codec-h2-spec) |
| `header_spec` | Header names, order and values per request shape. | [HeaderSpec](/stealth/api/codec-header-spec) |
| `ids` | IANA names of TLS and HTTP/2 ids, and what BoringSSL can emit. | Registries |

## Re-exports at the root

The main types are re-exported, so `seleniumbase_rs_codec::Ja4` is
`seleniumbase_rs_codec::ja4::Ja4`. Two enums are both called `SpecError` in their
modules, and three lookups of `emulation` have short names, so they are renamed:

| Root name | Item |
|---|---|
| `Akamai`, `AkamaiError` | `akamai::Akamai`, `akamai::AkamaiError` |
| `ClientHello`, `ClientHelloError` | `client_hello::ClientHello`, `client_hello::ClientHelloError` |
| `Emulation`, `EmulationError` | `emulation::Emulation`, `emulation::EmulationError` |
| `measured_emulations` | `emulation::measured` |
| `emulation_for_kind` | `emulation::for_kind` |
| `get_emulation` | `emulation::get` |
| `H2Spec`, `H2SpecError` | `h2_spec::H2Spec`, `h2_spec::SpecError` |
| `HeaderIdentity`, `HeaderSpec`, `HeaderSpecError`, `RequestShape` | the `header_spec` items of the same name |
| `Ja3`, `Ja3Error`, `Peetprint`, `PeetprintError` | `ja3::Ja3`, `ja3::Ja3Error`, `ja3::peetprint::Peetprint`, `ja3::peetprint::PeetprintError` |
| `Ja4`, `Ja4Error`, `Ja4Raw` | the `ja4` items of the same name |
| `KnownGap`, `TlsSpec`, `TlsSpecError` | `tls_spec::KnownGap`, `tls_spec::TlsSpec`, `tls_spec::SpecError` |

`emulation::SpecError` is a type alias of `EmulationError`.

## `BrowserKind`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    Chrome,
}
```

A browser whose wire behaviour the measured tables describe. It is defined at the
crate root and has no serde of its own: the measured JSON spells it `Chrome`, and
the identity file of the scrape crate spells it `chrome`.

<ResponseField name="BrowserKind" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Chrome">

Google Chrome, the first browser measured (on macOS).

</ResponseField>

</Expandable>

</ResponseField>

The enum is `#[non_exhaustive]`: match it with a wildcard arm. Every variant has
a measured entry; a unit test of the crate fails when one is added without a
measurement.

## Sub-pages

<Columns cols={2}>

<Card color="#1a85d4" title="Emulation" icon="database" href="/stealth/api/codec-emulation">

The measured tables: `Emulation`, `Provenance`, `get`, `get_version`, `measured`.

</Card>

<Card color="#199f43" title="ClientHello" icon="handshake" href="/stealth/api/codec-client-hello">

Parse the first TLS handshake message, across records.

</Card>

<Card color="#d47628" title="JA3 and peetprint" icon="fingerprint" href="/stealth/api/codec-ja3">

`Ja3`, `Peetprint`, `Slot`, MD5 helpers.

</Card>

<Card color="#a631be" title="JA4" icon="fingerprint" href="/stealth/api/codec-ja4">

`Ja4`, `Ja4Raw`, `Ja4Facts`, `Ja4Diff`.

</Card>

<Card color="#1ca1c7" title="Akamai" icon="code" href="/stealth/api/codec-akamai">

The HTTP/2 fingerprint string: `Akamai`, `PriorityFrame`, `Pseudo`.

</Card>

<Card color="#d32a61" title="HTTP/2 frames" icon="layer-group" href="/stealth/api/codec-h2-frames">

`read_preface`, `PrefaceReader`, `HpackDecoder`, Huffman.

</Card>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/codec-tls-spec">

The TLS profile, its reproducibility check and the overlays.

</Card>

<Card color="#d5a910" title="H2Spec" icon="server" href="/stealth/api/codec-h2-spec">

The HTTP/2 profile and what the wreq stack can send.

</Card>

<Card color="#17a5af" title="HeaderSpec" icon="list" href="/stealth/api/codec-header-spec">

Header lists, placeholders and the `priority` table.

</Card>

</Columns>
