---
title: "network (Wire Specs, Proxy Pools, BoringSSL WireFormat, MitmProxy, HTTP/3)"
sidebarTitle: "network"
description: "Unified API reference for Layer 3 of seleniumbase-rs-stealth (seleniumbase_rs_stealth::network): pure wire specs (ClientHello, JA3, JA4, peetprint, Akamai H2, H3, headers, WireMeasurement), proxy pools, BoringSSL WireFormat, MitmProxy, EphemeralCa, HTTP/3, and wire-backed sessions."
---

<Badge color="blue">Layer 3: network</Badge> <Badge color="orange">Experimental</Badge> <Badge color="blue">Features: network-spec / network / http3</Badge>

`seleniumbase_rs_stealth::network` (`src/network/` in [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)) is Layer 3 of the stealth stack. It combines two compile-time tiers under one module path:

1. **Pure Rust wire specs and proxy routing (`network-spec`, enabled by default through `session`, zero BoringSSL)**: `network::spec` (TLS `ClientHello`, JA3, JA4, peetprint, Akamai HTTP/2, HTTP/2 frames and HPACK, `TlsSpec`, `H2Spec`, `H3Spec`, `HeaderSpec`, `WireMeasurement`, and IANA registries) plus `network::proxy` (`ProxyPool`, `ProxyRoute`, `GeoResolver`, `StaticGeoResolver`, `RedactedUrl`, and `ProxySwitch`). Zero sockets, zero files, `#![forbid(unsafe_code)]`, and bounded parsers that return errors on truncated or hostile bytes instead of panicking.
2. **BoringSSL wire engine and HTTP/3 (`features = ["network"]` and `features = ["http3"]`)**: `network::wire` (`WireFormat`, `WireFormatBuilder`, `WireProtocol`), `network::convert`, `network::mitm` (`MitmProxy`, `MitmOptions`), `network::tls_server` (`EphemeralCa`, `LeafCert`), `network::capture::h2`, and `network::http3` (`Http3Client`, `Http3Pool`, `Http3Options`, `Http3Request`, `Http3Response`, `Alt-Svc` HTTP/3 upgrades through the standalone `sbrs-h3d` helper in `h3d/`). Enabling `network` also unlocks the wire-backed sessions in Layer 4 (`session::HttpSession` and `session::HybridSession`).

A browser or system that was not measured is refused. Today the embedded table (`src/network/spec/measured/*.json`) holds one measured entry: Chrome 155.0.8059.40 on macOS. See [Limits](/stealth/advanced/session/limits).

## When to use which tier

- **Default (`session`, includes `network-spec`, zero BoringSSL)**: Use when you need proxy pools, geo-aligned egress routing, wire-format parsing (JA3/JA4/Akamai/`ClientHello`), or real-Chrome scraping (`session::CdpSession`) without BoringSSL or cmake build costs.
- **`features = ["network"]` (BoringSSL/wreq)**: Use when the target judges the wire handshake, not only the page. `session::HttpSession::from_page` continues a Chrome tab as plain HTTP, `from_identity` starts synchronously from a `session::Identity`, `network::MitmProxy` puts a real Chrome behind the measured handshake, and `session::HybridSession` moves one identity between HTTP and Chrome.
- **`features = ["http3"]` (BoringSSL + `sbrs-h3d`)**: Adds `network::http3` client/pool and `Alt-Svc` HTTP/3 upgrades.

<RequestExample>

```rust Pure wire specs: measured Chrome and parsers (zero BoringSSL)
use seleniumbase_rs_stealth::browser::config::Browser;
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::network::spec::{measurement, Akamai, Ja3, Ja4};

fn inspect_wire_specs() -> Result<(), Box<dyn std::error::Error>> {
    let chrome = measurement::get(Browser::Chrome, OsType::Macos)?;
    println!("{} {}", chrome.provenance.browser, chrome.tls.ja4());

    let akamai: Akamai = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p".parse()?;
    let ja4: Ja4 = "t13d1517h2_8daaf6152771_cb7bf5808d99".parse()?;
    let ja3 = Ja3::parse("771,4865-4866-4867,0-23,29-23,0")?;
    println!("{} {} {}", akamai.window_update, ja4.a, ja3.hash());
    Ok(())
}
```

```rust BoringSSL wire engine: continue a Chrome page as plain HTTP (feature network)
use seleniumbase_rs_stealth::json::Value;
use seleniumbase_rs_stealth::sb_cdp::Page;
use seleniumbase_rs_stealth::session::{HttpError, HttpSession, HttpSessionOptions};

async fn orders(page: &Page) -> Result<(), HttpError> {
    let session = HttpSession::from_page(page, None, HttpSessionOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: Value = reply.json()?;
    println!("{orders}");
    session.sync_cookies_back(page).await?;
    Ok(())
}
```

</RequestExample>

<ResponseExample>

```text Measured Chrome output
Chrome 155.0.8059.40 t13d1517h2_8daaf6152771_cb7bf5808d99
```

</ResponseExample>

`HttpSession::from_page(&Page, Option<network::ProxyRoute>, HttpSessionOptions)` is async. `None` uses the page's context proxy, or a direct connection when that context has none. A proxy Chrome was launched with is not on the page: `from_page` returns `HttpError::BrowserProxy` unless that egress is passed, or `HttpSessionOptions::browser_proxy_exits_directly(true)` declares that the proxy leaves from this machine. Every signature is on [HttpSession](/stealth/api/network/http-session).

## Depend on it

<CodeGroup>

```toml Pure wire specs + proxy pools only (zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["network-spec"] }
```

```toml Default (includes identity, client, network-spec, session; zero BoringSSL)
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

```toml BoringSSL wire impersonation, MitmProxy, HttpSession, HybridSession
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["network"] }
```

```toml Full wire stack + HTTP/3 via sbrs-h3d
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

</CodeGroup>

Building `network` or `http3` compiles BoringSSL through wreq, so the build host needs cmake and a C/C++ toolchain.

## Features

- `network-spec` *(included in default `session`)*: Pure Rust `network::spec` and `network::proxy` with zero BoringSSL.
- `network`: Implies `session`, `client`, and `network-spec`. Adds `network::wire`, `network::convert`, `network::mitm`, `network::tls_server`, `network::capture::h2`, plus `session::HttpSession` and `session::HybridSession`.
- `http3`: Implies `network`. Adds `network::http3::{Http3Client, Http3Pool, Http3Options, Http3Request, Http3Response}` and `Alt-Svc` upgrades over `sbrs-h3d`.

<ParamField body="test-fixtures" type="feature" default="off">

Turns on the `ClientHello` byte builders used in unit tests (`network::spec::client_hello::fixtures`, hidden from generated docs). Test support only; no release build turns it on.

</ParamField>

## Module map

### Pure wire specs and proxy routing (`network-spec`, zero BoringSSL)

| Module | What it holds | Page |
|---|---|---|
| `network::spec::measurement` | Measured browser tables embedded from JSON and their lookups (`get`, `for_kind`, `all`). | [WireMeasurement](/stealth/api/network/measurement) |
| `network::spec::client_hello` | TLS `ClientHello` parser, `ext_id` constants, and GREASE helpers. | [ClientHello](/stealth/api/network/client-hello) |
| `network::spec::ja3`, `ja3::peetprint` | JA3 and peetprint parsing, rendering, and hashing. | [JA3 and peetprint](/stealth/api/network/ja3) |
| `network::spec::ja4` | JA4, `JA4_o`, `JA4_r`, and `JA4_ro`. | [JA4](/stealth/api/network/ja4) |
| `network::spec::akamai` | Akamai HTTP/2 fingerprint strings. | [Akamai](/stealth/api/network/akamai) |
| `network::spec::h2_frames` | HTTP/2 frame reading, client preface, HPACK, and Huffman decoding. | [HTTP/2 frames](/stealth/api/network/h2-frames) |
| `network::spec::tls_spec` | TLS profile a client must send, plus custom JA3, JA4, and peetprint overlays. | [TlsSpec](/stealth/api/network/tls-spec) |
| `network::spec::h2_spec` | HTTP/2 profile: SETTINGS, window update, PRIORITY, pseudo-header order. | [H2Spec](/stealth/api/network/h2-spec) |
| `network::spec::h3_spec` | HTTP/3 profile: SETTINGS, pseudo-header order, and QUIC transport parameters. | [H3Spec](/stealth/api/network/h3-spec) |
| `network::spec::header_spec` | Header names, order, and values per request shape. | [HeaderSpec](/stealth/api/network/header-spec) |
| `network::spec::ids` | IANA names of TLS and HTTP/2 IDs and what BoringSSL can emit. | [IANA ID registries](/stealth/api/network/ids) |
| `network::proxy` | Country-aware `ProxyPool`, `ProxyRoute`, `GeoResolver`, `StaticGeoResolver`, `RedactedUrl`, `ProxySwitch`. | [Egress and proxies](/stealth/api/session/egress) |

### Wire engine, MITM, HTTP/3, and wire-backed sessions (`network` / `http3`)

| Module | What it holds | Page |
|---|---|---|
| `network::wire` | `WireFormat`, `WireFormatBuilder`, `WireFormat::client_builder`, `WireProtocol`. | [WireFormat](/stealth/api/network/wire-format) |
| `network::convert` | `network::spec` types converted to wreq and BoringSSL options. | [Convert](/stealth/api/network/convert) |
| `network::mitm` | `MitmProxy`, `MitmOptions`, `MitmStats`, `MitmError`. | [MITM proxy](/stealth/api/network/mitm-proxy) |
| `network::tls_server` | `EphemeralCa`, `CaOptions`, `LeafCert` (one leaf per host). | [TLS server](/stealth/api/network/tls-server) |
| `network::http3` | `Http3Client`, `Http3Pool`, `Http3Options`, `Http3Request`, `Http3Response`, `capture`. | [HTTP/3](/stealth/advanced/network/http3) |
| `network::capture::h2` | Local Chrome HTTP/2 measurement capture tool. | [HTTP/2 capture](/stealth/api/network/capture-h2) |
| `session::http` | `HttpSession`, `HttpSessionOptions`, `HttpResponse`, `HttpRequest`. | [HttpSession](/stealth/api/network/http-session) |
| `session::hybrid` | `HybridSession`, `HybridSessionBuilder`, `HybridFetch`, `HybridError`. | [HybridSession](/stealth/api/network/hybrid-session) |

## Re-exports

### At `seleniumbase_rs_stealth::network` (and `network::spec`)

| Re-export | Source module | Feature |
|---|---|---|
| `Akamai`, `AkamaiError`, `ClientHello`, `ClientHelloError`, `WireMeasurement`, `WireMeasurementError`, `H2Spec`, `H2SpecError`, `H3Spec`, `H3SpecError`, `TransportParam`, `ClientHeaders`, `HeaderSpec`, `HeaderSpecError`, `RequestShape`, `Ja3`, `Ja3Error`, `Peetprint`, `PeetprintError`, `Ja4`, `Ja4Error`, `Ja4Raw`, `KnownGap`, `TlsSpec`, `TlsSpecError` | `network::spec` | `network-spec` |
| `CountryCode`, `GeoResolver`, `LocaleExt`, `ProxyKey`, `ProxyPool`, `ProxyRoute`, `RedactedUrl`, `Secret`, `StaticGeoResolver` (`ProxySwitch` with `session`) | `network::proxy` | `network-spec` |
| `WireFormat`, `WireFormatBuilder`, `WireFormatError`, `WireProtocol` | `network::wire` | `network` |
| `MitmError`, `MitmOptions`, `MitmProxy`, `MitmStats` | `network::mitm` | `network` |
| `CaOptions`, `EphemeralCa`, `LeafCert`, `TlsServerError` | `network::tls_server` | `network` |
| `Http3Client`, `Http3Error`, `Http3Options`, `Http3Pool`, `Http3Request`, `Http3RequestKind`, `Http3Response` | `network::http3` | `http3` |

`measurement::all`, `measurement::for_kind`, and `measurement::get` are called through `network::spec::measurement`. `ConvertError` and `check_emittable` stay in `network::convert`; `CaptureH2Error` stays in `network::capture::h2`.

### Wire-backed sessions at `seleniumbase_rs_stealth::session` (`feature = "network"`)

| Re-export | Source module |
|---|---|
| `HttpError`, `HttpSessionOptions`, `HttpResponse`, `HttpRequest`, `HttpSession`, `Method`, `HttpAttempt`, `HttpFetch`, `HttpRules`, `HttpVerdict`, `ResponseHeaders` | `session::http` |
| `HybridError`, `HybridFetch`, `HybridSession`, `HybridSessionBuilder` | `session::hybrid` |

## Browsers and errors

The wire spec layer uses core's `seleniumbase_rs::browser::config::Browser` (`Browser::Chrome` is the measured browser). A browser or OS without a recorded measurement returns `WireMeasurementError::NotMeasured` in `network::spec`, wrapped as `WireFormatError::Measurement` and, from `HttpSession`, as `HttpError::WireFormat`. A Chrome major version other than the measured one returns `HttpError::VersionMismatch`; the stack never borrows another system's wire values.

## Sub-pages

<Columns cols={2}>

<Card color="#1a85d4" title="WireMeasurement" icon="database" href="/stealth/api/network/measurement">

Measured browser tables: `WireMeasurement`, `Provenance`, `get`, `version`, `measured`.

</Card>

<Card color="#199f43" title="ClientHello" icon="handshake" href="/stealth/api/network/client-hello">

Parse the first TLS handshake message across records.

</Card>

<Card color="#d47628" title="JA3 and peetprint" icon="fingerprint" href="/stealth/api/network/ja3">

`Ja3`, `Peetprint`, `Slot`, and MD5 helpers.

</Card>

<Card color="#a631be" title="JA4" icon="fingerprint" href="/stealth/api/network/ja4">

`Ja4`, `Ja4Raw`, `Ja4Facts`, `Ja4Diff`.

</Card>

<Card color="#1ca1c7" title="Akamai" icon="code" href="/stealth/api/network/akamai">

HTTP/2 fingerprint strings: `Akamai`, `PriorityFrame`, `PseudoHeader`.

</Card>

<Card color="#d32a61" title="HTTP/2 frames" icon="layer-group" href="/stealth/api/network/h2-frames">

`read_preface`, `PrefaceReader`, `HpackDecoder`, Huffman.

</Card>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/network/tls-spec">

TLS profiles, reproducibility checks, and overlays.

</Card>

<Card color="#d5a910" title="H2Spec & H3Spec" icon="server" href="/stealth/api/network/h2-spec">

HTTP/2 and HTTP/3 wire profiles and what wreq / `sbrs-h3d` can emit.

</Card>

<Card color="#17a5af" title="HeaderSpec" icon="list" href="/stealth/api/network/header-spec">

Header lists, placeholders, and the `priority` table.

</Card>

<Card color="#1a85d4" title="WireFormat" icon="network-wired" href="/stealth/api/network/wire-format">

`WireFormat`, `WireFormatBuilder`, `WireProtocol`, and BoringSSL client construction.

</Card>

<Card color="#199f43" title="MITM proxy & TLS server" icon="shield-halved" href="/stealth/api/network/mitm-proxy">

Loopback `MitmProxy` and `EphemeralCa` leaf certificates.

</Card>

<Card color="#d47628" title="HttpSession & HybridSession" icon="bolt" href="/stealth/api/network/http-session">

Continue a Chrome tab as plain HTTP or escalate between HTTP and Chrome automatically.

</Card>

</Columns>

## Guides

[Wire formats](/stealth/advanced/network/wire-formats), [MITM proxy](/stealth/advanced/network/mitm-proxy),
[HTTP/3](/stealth/advanced/network/http3), [HttpSession](/stealth/advanced/session/http-session),
[HybridSession](/stealth/advanced/session/hybrid-session), [Runnable examples](/stealth/advanced/session/examples),
and [Limits](/stealth/advanced/session/limits).
