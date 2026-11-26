---
title: "network (BoringSSL wire, MITM, HTTP/3) and wire sessions"
sidebarTitle: "network & sessions"
description: "API reference of seleniumbase_rs_stealth::network (feature network / http3) and wire-backed sessions: WireFormat, MitmProxy, EphemeralCa, Http3Client, HttpSession, and HybridSession."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Feature: network / http3</Badge>

Enabling `features = ["network"]` on [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth) adds BoringSSL/wreq wire impersonation under `seleniumbase_rs_stealth::network` (`WireFormat`, `WireFormatBuilder`, `MitmProxy`, `EphemeralCa`, `capture::h2`, `http3::capture`) and unlocks the wire-backed sessions under `seleniumbase_rs_stealth::session` (`HttpSession` and `HybridSession`). Enabling `features = ["http3"]` adds `network::http3::{Http3Client, Http3Pool, Http3Options, Http3Request, Http3Response}` and `Alt-Svc` HTTP/3 upgrades through the standalone `sbrs-h3d` helper (`h3d/`).

A browser or system that was not measured is refused. Only Chrome 155.0.8059.40 on macOS is measured. See [Limits](/scraping/advanced-stealthy/limits).

## When to use it

Reach for `features = ["network"]` when the site judges the wire, not only the page. `session::HttpSession::from_page`
continues a Chrome tab as plain HTTP. `from_identity` starts from a `session::Identity`
and is synchronous. `network::MitmProxy` puts a real Chrome behind the measured handshake.
`session::HybridSession` moves one identity between those two.

Leave `network` off (the default `session` feature) when a real Chrome (`session::CdpSession`) is enough and you want zero BoringSSL or cmake build cost.

<RequestExample>

```rust Continue a page as plain HTTP
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

`from_page(&Page, Option<network::ProxyRoute>, HttpSessionOptions)` is async. `None` uses the page's
context proxy, or a direct connection when that context has none. A proxy Chrome was launched
with is not on the page: `from_page` then returns `HttpError::BrowserProxy` unless that egress
is passed, or `HttpSessionOptions::browser_proxy_exits_directly(true)` says the proxy leaves from this
machine. `Value` is the re-export of `serde_json::Value`. Every signature is on
[HttpSession](/stealth/api/wire-http-session).

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["network"] }
```

Add `features = ["http3"]` to also enable `network::http3` client/pool and `Alt-Svc` HTTP/3 upgrades (`http3` implies `network`, which implies `session`, `client`, and `network-spec`). Building `network` compiles BoringSSL through wreq, so the machine needs cmake and a C/C++ toolchain.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `network::wire` | `WireFormat`, `WireFormatBuilder`, `WireFormat::client_builder`, `WireProtocol`. | [WireFormat](/stealth/api/wire-client) |
| `network::convert` | `network::spec` types to wreq and BoringSSL options. | [Convert](/stealth/api/wire-convert) |
| `network::mitm` | `MitmProxy` and its options. | [MITM proxy](/stealth/api/wire-mitm) |
| `network::tls_server` | `EphemeralCa`, one leaf per host. | [TLS server](/stealth/api/wire-tls-server) |
| `network::http3` | `Http3Client`, `Http3Pool`, `Http3Options`, `Http3Request`, `Http3Response`, and `capture`. | [HTTP/3](/stealth/advanced/http3) |
| `network::capture::h2` | The HTTP/2 measurement tool. | [HTTP/2 capture](/stealth/api/wire-capture-h2) |
| `session::http` | `HttpSession`, `HttpSessionOptions`, `HttpResponse`, `HttpRequest`. | [HttpSession](/stealth/api/wire-http-session) |
| `session::hybrid` | `HybridSession`, `HybridSessionBuilder`, `HybridFetch`, `HybridError`. | [HybridSession](/stealth/api/wire-stealth-session) |

## Re-exports

### At `seleniumbase_rs_stealth::network`

| Re-export | Module |
|---|---|
| `WireFormat`, `WireFormatBuilder`, `WireFormatError`, `WireProtocol` | `network::wire` |
| `MitmError`, `MitmOptions`, `MitmProxy`, `MitmStats` | `network::mitm` |
| `CaOptions`, `EphemeralCa`, `LeafCert`, `TlsServerError` | `network::tls_server` |
| `Http3Client`, `Http3Error`, `Http3Options`, `Http3Pool`, `Http3Request`, `Http3RequestKind`, `Http3Response` *(feature `http3`)* | `network::http3` |

`ConvertError` and `check_emittable` stay in `network::convert`. `CaptureH2Error` stays in `network::capture::h2`.

### At `seleniumbase_rs_stealth::session`

| Re-export | Module |
|---|---|
| `HttpError`, `HttpSessionOptions`, `HttpResponse`, `HttpRequest`, `HttpSession`, `Method`, `HttpAttempt`, `HttpFetch`, `HttpRules`, `HttpVerdict`, `ResponseHeaders` | `session::http` |
| `HybridError`, `HybridFetch`, `HybridSession`, `HybridSessionBuilder` | `session::hybrid` |

## Errors

A browser or system with no measurement is `WireMeasurementError::NotMeasured` in `network::spec`, wrapped as
`WireFormatError::Measurement` and, from `HttpSession`, as `HttpError::WireFormat`. A Chrome major
other than the measured one is `HttpError::VersionMismatch`. The session does not borrow another
system's values.

## Guides

[HttpSession](/scraping/stealthy/http-session), [MITM proxy](/stealth/advanced/mitm-proxy),
[HybridSession](/scraping/advanced-stealthy/stealth-session), [Wire formats](/stealth/advanced/wire-formats),
[Runnable examples](/scraping/advanced-stealthy/examples) and [Limits](/scraping/advanced-stealthy/limits).
