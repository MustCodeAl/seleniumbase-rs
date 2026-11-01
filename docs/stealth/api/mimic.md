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
refused, not approximated. Only Chrome 155.0.8059.40 on macOS is measured today; see
[Limits](/stealth/limits). The guides are [HttpSession](/stealth/http-session),
[MITM proxy](/stealth/mitm-proxy) and [StealthSession](/stealth/stealth-session).

## When to use it

Reach for mimic when the site judges the wire, not only the page. `HttpSession`
continues a Chrome tab as plain HTTP (`from_page`) or starts from a
`scrape::StealthIdentity` (`from_identity`). `MitmProxy` puts a real Chrome
behind a loopback proxy that replays each request with the measured handshake.
`StealthSession` moves one identity between those two.

Reach for [scrape](/stealth/api/scrape) when a real Chrome is enough. scrape is
on by default, builds no BoringSSL, and runs wherever Chrome runs. mimic is off
until the `mimic` feature is enabled, needs cmake and a C/C++ toolchain, and
refuses to impersonate a browser or system that was not measured.

<RequestExample>

```rust Carry a browser's session on in plain HTTP
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

`from_page(&Page, Option<scrape::EgressRef>, HttpOptions)` is async. `None`
uses the page's context proxy, or a direct connection when that context has
none. A proxy Chrome was launched with is not visible on the page: `from_page`
then returns `HttpError::BrowserProxy` unless that egress is passed, or
`HttpOptions::browser_proxy_exits_directly(true)` says the proxy leaves from
this machine. `get(&str)` returns `HttpReply`. `json` needs a
`T: DeserializeOwned`. `Value` is the facade re-export of `serde_json::Value`,
so this example adds no `serde_json` dependency. `sync_cookies_back(&Page)`
returns how many cookies were written.

## Depend on it

Use the unified root facade from Git:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

Enable `mimic` for this module. The feature also turns `scrape` and `codec` on.
Building it compiles BoringSSL through wreq, so the machine needs cmake and a
C/C++ toolchain. `sb_cdp`, `evasion` and `fingerprint` stay available: the
facade always re-exports `seleniumbase-rs`. They are modules, not features.

## Features

The public `mimic` module requires the facade feature `mimic`, off by default.
In the stealth repository's root `Cargo.toml`, `mimic` also enables `scrape`
(and `scrape` enables `codec`). `seleniumbase-rs-mimic` declares no Cargo
features of its own. Its tests enable `test-util` on `seleniumbase-rs`.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `client` | `ImpersonationProfile` (a measured browser as data, with `known_gaps`), `impersonation_client` (a `wreq::ClientBuilder` from a profile), `CustomProfileBuilder` (a JA3 or an Akamai string laid over the measured profile), `WireProtocol`, `ImpersonateError`. | [Wire formats](/stealth/wire-formats) |
| `convert` | Plain functions from the codec's specs to wreq and BoringSSL options: `tls_options`, `http2_options`, `wreq_headers`, and `check_emittable` (can this TLS spec be sent at all); `ConvertError` names what cannot be. The only module besides `client` that names wreq types. | [Wire formats](/stealth/wire-formats) |
| `http_session` | `HttpSession`. `from_page(&Page, Option<scrape::EgressRef>, HttpOptions)` is async. `from_identity(&scrape::StealthIdentity, Option<scrape::EgressRef>, HttpOptions)` is synchronous. `get(&str)` is async and returns `HttpReply`. `request(Method, &str)` returns `HttpRequest`, sent with `HttpRequest::send`. `fetch(&str, &HttpAttempt, &HttpRules, &scrape::StealthIdentity)` is async and returns `HttpFetched`. `assess(&HttpReply, &HttpAttempt, &HttpRules, &scrape::StealthIdentity)` returns `HttpVerdict`. `preflight(&str, &scrape::StealthIdentity, Option<&scrape::DomainState>, &HttpRules)` returns `Result<scrape::Transition, HttpError>`. `export_identity` returns `scrape::StealthIdentity`. `sync_cookies_back(&Page)` is async and returns `usize`. Also `cookie_count`, `profile` and `egress`. `HttpOptions::new` uses `DEFAULT_TIMEOUT` (30s), `DEFAULT_CONNECT_TIMEOUT` (10s), `DEFAULT_MAX_BODY_BYTES` (64 MiB), `DEFAULT_MAX_REQUEST_BYTES` (64 MiB) and `DEFAULT_MAX_REDIRECTS` (10). `validate` refuses a timeout above `MAX_TIMEOUT` (1 hour), a body limit above `MAX_BODY_LIMIT` (1 GiB) or a redirect limit above `MAX_REDIRECT_LIMIT` (30). `HttpReply::status`, `text` and `bytes` read the reply; `json` parses `T: DeserializeOwned`. | [HttpSession](/stealth/http-session) |
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
measurement is `EmulationError::NotMeasured` in the codec, wrapped as
`ImpersonateError::Emulation` and, from `HttpSession`, as `HttpError::Impersonate`.
A Chrome major other than the measured one is `HttpError::VersionMismatch`
(`ImpersonateError::VersionMismatch` before that conversion). The session does
not borrow another system's values. The measured entry is Chrome 155.0.8059.40
on macOS (`crates/codec/src/measured/chrome-155-macos.json` in the stealth
repository).

## Guides

The item-by-item pages for this crate are still being written. Until then, the
[HttpSession](/stealth/http-session), [MITM proxy](/stealth/mitm-proxy) and
[StealthSession](/stealth/stealth-session) guides, the
[runnable examples](/stealth/examples), and the crate's rustdoc
(`cargo doc -p seleniumbase-rs-mimic --open` in the stealth repository) cover it.
