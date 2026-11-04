---
title: "seleniumbase-rs-mimic"
sidebarTitle: "mimic"
description: "API reference of seleniumbase-rs-mimic: browser-identical TLS and HTTP/2 through wreq and BoringSSL."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`seleniumbase_rs_stealth::mimic` (implemented by `seleniumbase-rs-mimic` in `crates/mimic` of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth))
sends requests that look like a measured Chrome on the wire: the same TLS `ClientHello`, the same
HTTP/2 start, and the same header names, order and values. It builds a wreq client from an
`ImpersonationProfile`, carries a browser's cookies on in plain HTTP (`HttpSession`), puts a real
Chrome behind a loopback proxy that replays its requests (`MitmProxy`), and moves one identity
between the two (`StealthSession`). A browser or system that was not measured is refused.
Only Chrome 155.0.8059.40 on macOS is measured. See [Limits](/scraping/advanced-stealthy/limits).

## When to use it

Reach for mimic when the site judges the wire, not only the page. `HttpSession::from_page`
continues a Chrome tab as plain HTTP. `from_identity` starts from a `scrape::StealthIdentity`
and is synchronous. `MitmProxy` puts a real Chrome behind the measured handshake.
`StealthSession` moves one identity between those two.

Reach for [scrape](/stealth/api/scrape) when a real Chrome is enough. scrape is on by default
and builds no BoringSSL. mimic is off until the `mimic` feature is enabled, needs cmake and a
C/C++ toolchain, and refuses to impersonate a system that was not measured.

<RequestExample>

```rust Continue a page as plain HTTP
use seleniumbase_rs_stealth::json::Value;
use seleniumbase_rs_stealth::mimic::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn orders(page: &Page) -> Result<(), HttpError> {
    let session = HttpSession::from_page(page, None, HttpOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: Value = reply.json()?;
    println!("{orders}");
    session.sync_cookies_back(page).await?;
    Ok(())
}
```

</RequestExample>

`from_page(&Page, Option<scrape::EgressRef>, HttpOptions)` is async. `None` uses the page's
context proxy, or a direct connection when that context has none. A proxy Chrome was launched
with is not on the page: `from_page` then returns `HttpError::BrowserProxy` unless that egress
is passed, or `HttpOptions::browser_proxy_exits_directly(true)` says the proxy leaves from this
machine. `Value` is the facade re-export of `serde_json::Value`. Every signature is on
[HttpSession](/stealth/api/mimic-http-session).

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

The `mimic` feature also turns `scrape` and `codec` on. Building it compiles BoringSSL through
wreq, so the machine needs cmake and a C/C++ toolchain. `sb_cdp`, `evasion` and `fingerprint`
stay available because the facade always re-exports `seleniumbase-rs`. They are modules, not features.

## Features

The public `mimic` module requires the facade feature `mimic`, off by default. In the stealth
repository's root `Cargo.toml`, `mimic` enables `scrape`, and `scrape` enables `codec`.
`seleniumbase-rs-mimic` declares no Cargo features of its own. Its tests enable `test-util` on
`seleniumbase-rs`.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `client` | `ImpersonationProfile`, `CustomProfileBuilder`, `impersonation_client`. | [Client](/stealth/api/mimic-client) |
| `convert` | Codec specs to wreq and BoringSSL options. | [Convert](/stealth/api/mimic-convert) |
| `http_session` | `HttpSession`, `HttpOptions`, `HttpReply`. | [HttpSession](/stealth/api/mimic-http-session) |
| `mitm` | `MitmProxy` and its options. | [MITM proxy](/stealth/api/mimic-mitm) |
| `tls_server` | `EphemeralCa`, one leaf per host. | [TLS server](/stealth/api/mimic-tls-server) |
| `stealth_session` | `StealthSession` and its builder. | [StealthSession](/stealth/api/mimic-stealth-session) |
| `capture_h2` | The HTTP/2 measurement tool. | [HTTP/2 capture](/stealth/api/mimic-capture-h2) |

## Re-exports at the root

| Root name | Module |
|---|---|
| `impersonation_client`, `CustomProfileBuilder`, `ImpersonateError`, `ImpersonationProfile`, `WireProtocol` | `client` |
| `HttpError`, `HttpOptions`, `HttpReply`, `HttpRequest`, `HttpSession`, `Method`, `ReplyProtocol` | `http_session` |
| `HttpAttempt`, `HttpFetched`, `HttpRules`, `HttpVerdict`, `ReplyHeaders` | `http_session`'s `Attempt`, `Fetched`, `Rules`, `Verdict` and `Headers` |
| `MitmError`, `MitmOptions`, `MitmProxy`, `MitmStats` | `mitm` |
| `StealthError`, `StealthFetch`, `StealthSession`, `StealthSessionBuilder` | `stealth_session` |
| `CaOptions`, `EphemeralCa`, `Leaf`, `TlsServerError` | `tls_server` |

`ConvertError` and `check_emittable` stay in `convert`. `CaptureH2Error` stays in `capture_h2`.

## Errors

A browser or system with no measurement is `EmulationError::NotMeasured` in the codec, wrapped as
`ImpersonateError::Emulation` and, from `HttpSession`, as `HttpError::Impersonate`. A Chrome major
other than the measured one is `HttpError::VersionMismatch`. The session does not borrow another
system's values.

## Guides

[HttpSession](/scraping/stealthy/http-session), [MITM proxy](/stealth/advanced/mitm-proxy),
[StealthSession](/scraping/advanced-stealthy/stealth-session), [Wire formats](/stealth/advanced/wire-formats),
[Runnable examples](/scraping/advanced-stealthy/examples) and [Limits](/scraping/advanced-stealthy/limits).
