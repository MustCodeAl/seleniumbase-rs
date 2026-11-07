---
title: "seleniumbase-rs-wire"
sidebarTitle: "wire"
description: "API reference of seleniumbase-rs-wire: browser-identical TLS and HTTP/2 through wreq and BoringSSL."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: wire</Badge>

`seleniumbase_rs_stealth::wire` (implemented by `seleniumbase-rs-wire` in `crates/wire` of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth))
sends requests that look like a measured Chrome on the wire: the same TLS `ClientHello`, the same
HTTP/2 start, and the same header names, order and values. It builds a wreq client from a
`WireFormat`, carries a browser's cookies on in plain HTTP (`HttpSession`), puts a real
Chrome behind a loopback proxy that replays its requests (`MitmProxy`), and moves one identity
between the two (`StealthSession`). A browser or system that was not measured is refused.
Only Chrome 155.0.8059.40 on macOS is measured. See [Limits](/scraping/advanced-stealthy/limits).

## When to use it

Reach for wire when the site judges the wire, not only the page. `HttpSession::from_page`
continues a Chrome tab as plain HTTP. `from_identity` starts from a `scrape::Identity`
and is synchronous. `MitmProxy` puts a real Chrome behind the measured handshake.
`StealthSession` moves one identity between those two.

Reach for [scrape](/stealth/api/scrape) when a real Chrome is enough. scrape is on by default
and builds no BoringSSL. wire is off until the `wire` feature is enabled, needs cmake and a
C/C++ toolchain, and refuses to impersonate a system that was not measured.

<RequestExample>

```rust Continue a page as plain HTTP
use seleniumbase_rs_stealth::json::Value;
use seleniumbase_rs_stealth::wire::{HttpError, HttpSessionOptions, HttpSession};
use seleniumbase_rs_stealth::sb_cdp::Page;

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

`from_page(&Page, Option<scrape::ProxyRoute>, HttpSessionOptions)` is async. `None` uses the page's
context proxy, or a direct connection when that context has none. A proxy Chrome was launched
with is not on the page: `from_page` then returns `HttpError::BrowserProxy` unless that egress
is passed, or `HttpSessionOptions::browser_proxy_exits_directly(true)` says the proxy leaves from this
machine. `Value` is the facade re-export of `serde_json::Value`. Every signature is on
[HttpSession](/stealth/api/wire-http-session).

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["wire"] }
```

The `wire` feature also turns `scrape` and `codec` on. Building it compiles BoringSSL through
wreq, so the machine needs cmake and a C/C++ toolchain. `sb_cdp`, `evasion` and `fingerprint`
stay available because the facade always re-exports `seleniumbase-rs`. They are modules, not features.

## Features

The public `wire` module requires the facade feature `wire`, off by default. In the stealth
repository's root `Cargo.toml`, `wire` enables `scrape`, and `scrape` enables `codec`.
`seleniumbase-rs-wire` declares no Cargo features of its own. Its tests enable `test-util` on
`seleniumbase-rs`.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `client` | `WireFormat`, `WireFormatBuilder`, `WireFormat::client_builder`. | [Client](/stealth/api/wire-client) |
| `convert` | Codec specs to wreq and BoringSSL options. | [Convert](/stealth/api/wire-convert) |
| `http_session` | `HttpSession`, `HttpSessionOptions`, `HttpResponse`. | [HttpSession](/stealth/api/wire-http-session) |
| `mitm` | `MitmProxy` and its options. | [MITM proxy](/stealth/api/wire-mitm) |
| `tls_server` | `EphemeralCa`, one leaf per host. | [TLS server](/stealth/api/wire-tls-server) |
| `stealth_session` | `StealthSession` and its builder. | [StealthSession](/stealth/api/wire-stealth-session) |
| `capture_h2` | The HTTP/2 measurement tool. | [HTTP/2 capture](/stealth/api/wire-capture-h2) |

## Re-exports at the root

| Root name | Module |
|---|---|
| `WireFormat::client_builder`, `WireFormatBuilder`, `WireFormatError`, `WireFormat`, `WireProtocol` | `client` |
| `HttpError`, `HttpSessionOptions`, `HttpResponse`, `HttpRequest`, `HttpSession`, `Method`, `WireProtocol` | `http_session` |
| `HttpAttempt`, `HttpFetched`, `HttpRules`, `HttpVerdict`, `ReplyHeaders` | `http_session`'s `Attempt`, `Fetched`, `Rules`, `Verdict` and `Headers` |
| `MitmError`, `MitmOptions`, `MitmProxy`, `MitmStats` | `mitm` |
| `StealthError`, `StealthFetch`, `StealthSession`, `StealthSessionBuilder` | `stealth_session` |
| `CaOptions`, `EphemeralCa`, `Leaf`, `TlsServerError` | `tls_server` |

`ConvertError` and `check_emittable` stay in `convert`. `CaptureH2Error` stays in `capture_h2`.

## Errors

A browser or system with no measurement is `WireMeasurementError::NotMeasured` in the codec, wrapped as
`WireFormatError::WireMeasurement` and, from `HttpSession`, as `HttpError::Impersonate`. A Chrome major
other than the measured one is `HttpError::VersionMismatch`. The session does not borrow another
system's values.

## Guides

[HttpSession](/scraping/stealthy/http-session), [MITM proxy](/stealth/advanced/mitm-proxy),
[StealthSession](/scraping/advanced-stealthy/stealth-session), [Wire formats](/stealth/advanced/wire-formats),
[Runnable examples](/scraping/advanced-stealthy/examples) and [Limits](/scraping/advanced-stealthy/limits).
