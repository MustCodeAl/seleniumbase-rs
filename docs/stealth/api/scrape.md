---
title: "session (CdpSession, challenge, jar, ledger, passport)"
sidebarTitle: "session"
description: "API reference of seleniumbase_rs_stealth::session: CdpSession, challenge detection and mitigation, CookieJar, Ledger, Identity, and (with feature network) HttpSession and HybridSession."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Layer: session</Badge>

`seleniumbase_rs_stealth::session` (`src/session/` in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth), enabled by default)
owns stateful anti-bot sessions. With default features (zero BoringSSL), `CdpSession` drives a real Chrome through `sb_cdp` with challenge solving, proxy/fingerprint rotation, cookie jar, per-site ledger, and passport persistence. Enabling `features = ["network"]` unlocks `HttpSession` and `HybridSession` inside the same `session` module.

## When to use it

Reach for `session::CdpSession` when a real browser has to load the page: a challenge that runs in the document,
a script that must execute, or the DOM after render. It builds no BoringSSL and runs wherever Chrome does.

Reach for `features = ["network"]` ([network & sessions](/stealth/api/wire)) when the site judges the wire fingerprint, or when a
browser's cookies should continue over plain HTTP (`session::HttpSession`) or escalate automatically between HTTP and Chrome behind `MitmProxy` (`session::HybridSession`). That feature builds BoringSSL and needs cmake.

<RequestExample>

```rust A CdpSession behind a German proxy
use seleniumbase_rs_stealth::network::CountryCode;
use seleniumbase_rs_stealth::session::{CdpSession, SessionError};
use seleniumbase_rs_stealth::tokio;

#[tokio::main]
async fn main() -> Result<(), SessionError> {
    let mut session = CdpSession::builder()
        .geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::try_new("DE")?)])?
        .max_challenge_attempts(4)
        .build()
        .await?;
    let page = session.fetch("https://example.com/").await?;
    println!("{} bytes, status {}", page.body.len(), page.status);
    session.close().await
}
```

</RequestExample>

`CdpSession::builder` returns `CdpSessionBuilder`. `geo_proxies` takes
`(spec, CountryCode)` pairs and returns `Result`. `CountryCode::try_new` returns
`Result<CountryCode, network::proxy::geo::GeoError>`, and that error converts into `SessionError`.
`fetch(&mut self, url)` returns `CdpFetch` (`body: String`, `status: u16`).
`close(self)` takes the session by value. Every signature is on
[CdpSession](/stealth/api/scrape-browser-scraper).

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

The default feature is `session`, which turns on `client` and `network-spec` with zero BoringSSL.
`sb_cdp` and core types are re-exported from `seleniumbase-rs`.

## Features

`default = ["session"]` in `Cargo.toml`. `session` turns on `client` and `network-spec`.
`network` and `http3` stay off unless enabled.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `session::cdp_session` | `CdpSession`, `CdpSessionBuilder`, `CdpFetch`, `Launcher`. | [CdpSession](/stealth/api/scrape-browser-scraper) |
| `session::http` *(feature `network`)* | `HttpSession`, `HttpSessionOptions`, `HttpRequest`, `HttpResponse`. | [HttpSession](/stealth/api/wire-http-session) |
| `session::hybrid` *(feature `network`)* | `HybridSession`, `HybridSessionBuilder`, `HybridFetch`, `HybridError`. | [HybridSession](/stealth/api/wire-stealth-session) |
| `session::challenge` | Detection, interstitial markers, the mitigation policy. | [Challenges](/stealth/api/scrape-challenge) |
| `network::proxy` | Proxy specs, the pool, countries, URL redaction, `ProxySwitch`. | [Proxy routing](/stealth/api/scrape-egress) |
| `session::jar` | The cookie jar and `Set-Cookie`. | [Cookie jar](/stealth/api/scrape-jar) |
| `session::passport` | `Identity` and `EscalationPolicy`. | [Identity](/stealth/api/scrape-passport) |
| `session::ledger` | Per-site state and `SessionEvent`. | [Ledger](/stealth/api/scrape-ledger) |
| `session::io` / `network::capture` | `JsonLedger`, `identity_file`, `TracingEventSink`, capture tools. | [I/O & capture](/stealth/api/scrape-runtime) |
| `session::error` | `SessionError`. | [Errors](/stealth/api/scrape-error) |

## Re-exports at `session`

| Re-export | Module |
|---|---|
| `ChallengeKind`, `Detection`, `ChallengeEvidence`, `ResponseView`, `ProtectionVendor` | `session::challenge::detect` |
| `Mitigation`, `AttemptContext`, `GiveUpReason`, `MitigationPolicy`, `FetchKind` | `session::challenge::mitigation` |
| `SessionError` | `session::error` |
| `CookieJar` | `session::jar::store` |
| `CollectingEventSink`, `EventSink`, `NoopEventSink`, `SessionEvent` | `session::ledger::events` |
| `SiteRecord`, `InMemoryLedger`, `Ledger` | `session::ledger::state` |
| `WireMeasurementRef`, `IdentityError`, `Identity` | `session::passport::identity` |
| `EscalationPolicy`, `Escalation` | `session::passport::policy` |
| `CdpFetch`, `CdpSession`, `CdpSessionBuilder`, `Launcher` | `session::cdp_session` |
| `JsonLedger` | `session::io::json_store` |
| `TracingEventSink` | `session::io::tracing_sink` |
| `HttpSession`, `HttpSessionOptions`, `HttpRequest`, `HttpResponse`, `HttpError`, `HttpAttempt`, `HttpFetch`, `HttpRules`, `HttpVerdict`, `Method`, `ResponseHeaders` *(feature `network`)* | `session::http` |
| `HybridSession`, `HybridSessionBuilder`, `HybridFetch`, `HybridError` *(feature `network`)* | `session::hybrid` |

Proxy types (`CountryCode`, `GeoResolver`, `LocaleExt`, `StaticGeoResolver`, `ProxyPool`, `ProxyKey`, `ProxyRoute`, `ProxySwitch`, `RedactedUrl`, `Secret`) live at `seleniumbase_rs_stealth::network`.

## Guides

[CdpSession](/scraping/stealthy/browser-scraper) is the fetch loop. [Identity and cookies](/scraping/advanced-stealthy/identity)
is the jar and the decision table. [Runnable examples](/scraping/advanced-stealthy/examples) lists the programs.
[Limits](/scraping/advanced-stealthy/limits) covers what is measured.
