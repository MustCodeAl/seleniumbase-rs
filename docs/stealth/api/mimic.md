---
title: "seleniumbase-rs-mimic"
sidebarTitle: "mimic"
description: "API reference of seleniumbase-rs-mimic: browser-identical TLS and HTTP/2 through wreq and BoringSSL. The impersonation profile and client, HttpSession, the MITM proxy for a real Chrome, StealthSession, the ephemeral CA and the HTTP/2 capture."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`seleniumbase_rs_stealth::mimic` (implemented by `seleniumbase-rs-mimic` in `crates/mimic` of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth))
sends requests that look like a measured Chrome on the wire: the same TLS
`ClientHello`, the same HTTP/2 start, and the same header names, order and
values. It builds a wreq client from an `ImpersonationProfile`, carries a
browser's cookies on in plain HTTP (`HttpSession`), puts a real Chrome behind a
loopback proxy that replays its requests with the measured handshake
(`MitmProxy`), and moves one identity between the two (`StealthSession`). Every
wire value comes from a measurement embedded in the
[codec](/stealth/api/codec): a browser or system that was not measured is
refused, not approximated. Only Chrome 155 on macOS is measured today; see
[Limits](/stealth/limits). The guides are [HttpSession](/stealth/http-session),
[MITM proxy](/stealth/mitm-proxy) and [StealthSession](/stealth/stealth-session).

<RequestExample>

```rust Carry a browser's session on in plain HTTP
use seleniumbase_rs_stealth::sb_cdp::Page;
use seleniumbase_rs_stealth::mimic::http_session::{HttpOptions, HttpSession};

async fn orders(page: Page) -> Result<(), Box<dyn std::error::Error>> {
    let session = HttpSession::from_page(&page, None, HttpOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: serde_json::Value = reply.json()?;
    println!("{orders}");
    session.sync_cookies_back(&page).await?;
    Ok(())
}
```

</RequestExample>

## Depend on it

Use the unified root facade from Git:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

Enable `mimic` for this module and its BoringSSL-backed TLS/HTTP2
impersonation. It needs cmake and a C/C++ toolchain. `scrape`, `codec` and the
core API remain available through the same dependency.

## Features

The public `mimic` module requires the facade's `mimic` feature, disabled by
default. The implementation crate has no Cargo features; its own tests enable
`test-util` of the core.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `client` | `ImpersonationProfile` (a measured browser as data, with `known_gaps`), `impersonation_client` (a `wreq::ClientBuilder` from a profile), `CustomProfileBuilder` (a JA3 or an Akamai string laid over the measured profile), `WireProtocol`, `ImpersonateError`. | [Wire formats](/stealth/wire-formats) |
| `convert` | Plain functions from the codec's specs to wreq and BoringSSL options: `tls_options`, `http2_options`, `wreq_headers`, and `check_emittable` (can this TLS spec be sent at all); `ConvertError` names what cannot be. The only module besides `client` that names wreq types. | [Wire formats](/stealth/wire-formats) |
| `http_session` | `HttpSession` (`from_page`, `from_identity`, `get`, `request` (a builder sent with `HttpRequest::send`), `fetch`, `assess`, `export_identity`, `sync_cookies_back`, `cookie_count`, `profile`), `HttpOptions` and its limits (`DEFAULT_TIMEOUT`, `DEFAULT_MAX_BODY_BYTES`, `MAX_REDIRECT_LIMIT`, ...), `HttpRequest`, `HttpReply`, `HttpError`, `Method`, and the judging types `Attempt`, `Fetched`, `Rules`, `Verdict`. Cookies, redirects, pacing and challenge detection are done by hand, as a browser does them. | [HttpSession](/stealth/http-session) |
| `mitm` | `MitmProxy` (a loopback forward proxy that ends Chrome's TLS and replays each request with the profile's fingerprint), `MitmOptions`, `MitmStats`, `MitmError`. It requires a random proxy token, refuses what it cannot intercept, and never tunnels blind. | [MITM proxy](/stealth/mitm-proxy) |
| `stealth_session` | `StealthSession` and `StealthSessionBuilder`: one `StealthIdentity` in HTTP mode or in a real Chrome behind the proxy, escalating on a challenge and demoting after a clearance; `StealthFetch`, `StealthError`. | [StealthSession](/stealth/stealth-session) |
| `tls_server` | `EphemeralCa` (an in-memory P-256 CA, one leaf per host, pinned by SPKI, never written to disk), `CaOptions`, `Leaf`, the acceptor, `TlsServerError`. | [MITM proxy](/stealth/mitm-proxy) |
| `capture_h2` | The `capture_h2` tool: serves a local HTTPS site, points the local Chrome at it, and records its HTTP/2 frames and header block (and an HTTP/1.1 pass) for a new measurement; `CaptureH2Error`. | [Examples](/stealth/examples) |

## Re-exports at the root

| Root name | Module |
|---|---|
| `impersonation_client`, `CustomProfileBuilder`, `ImpersonateError`, `ImpersonationProfile`, `WireProtocol` | `client` |
| `HttpError`, `HttpOptions`, `HttpReply`, `HttpRequest`, `HttpSession`, `Method`, `ReplyProtocol` | `http_session` |
| `HttpAttempt`, `HttpFetched`, `HttpRules`, `HttpVerdict`, `ReplyHeaders` | `http_session`'s `Attempt`, `Fetched`, `Rules`, `Verdict` and `Headers`, renamed so they do not read as something else at the root |
| `MitmError`, `MitmOptions`, `MitmProxy`, `MitmStats` | `mitm` |
| `StealthError`, `StealthFetch`, `StealthSession`, `StealthSessionBuilder` | `stealth_session` |
| `CaOptions`, `EphemeralCa`, `Leaf`, `TlsServerError` | `tls_server` |

## Errors

Each module has its own error type (`ImpersonateError`, `HttpError`,
`MitmError`, `StealthError`, `TlsServerError`, `CaptureH2Error`), built with
thiserror; each module's rustdoc lists its variants. A browser or system with no
measurement is refused with a `NotMeasured` error rather than sent with guessed
values.

## Guides

The item-by-item pages for this crate are still being written. Until then, the
[HttpSession](/stealth/http-session), [MITM proxy](/stealth/mitm-proxy) and
[StealthSession](/stealth/stealth-session) guides, the
[runnable examples](/stealth/examples), and the crate's rustdoc
(`cargo doc -p seleniumbase-rs-mimic --open` in the stealth repository) cover it.
