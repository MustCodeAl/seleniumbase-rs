---
title: "seleniumbase-rs-scrape"
sidebarTitle: "scrape"
description: "API reference of seleniumbase-rs-scrape: stealth scraping without TLS spoofing over the pure CDP engine. BrowserScraper, challenge detection and mitigation, egress, the cookie jar, the stealth identity, the ledger and the capture tools."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`seleniumbase-rs-scrape` (library `seleniumbase_rs_scrape`, in `crates/scrape` of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth))
does stealth scraping without TLS spoofing, over the pure CDP engine of
seleniumbase-rs (`sb_cdp`). Its pure modules hold challenge detection and the
mitigation policy, egress (proxy pool, geo locale, URL redaction), an RFC 6265
cookie jar, the per-site ledger and its events, and the stealth identity with its
session policy. The `runtime` module holds the I/O: `BrowserScraper`,
`EgressSwitch`, `JsonStateStore`, the identity file, the tracing sink and the
capture tools. It builds no BoringSSL and runs wherever Chrome does. The guides are
[BrowserScraper](/stealth/browser-scraper) and
[Identity and cookies](/stealth/identity).

<RequestExample>

```rust A scraper behind a German proxy
use seleniumbase_rs_scrape::{BrowserScraper, CountryCode, ScrapeError};

async fn run() -> Result<(), ScrapeError> {
    let mut scraper = BrowserScraper::builder()
        .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
        .with_max_challenge_attempts(4)
        .build()
        .await?;
    let page = scraper.fetch("https://example.com/").await?;
    println!("{} bytes, status {}", page.body.len(), page.status);
    scraper.close().await
}
```

</RequestExample>

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Write `seleniumbase-rs` by its plain git URL, exactly as above, so Cargo resolves
it to the commit the crate uses and types such as `sb_cdp::Browser` and
`Fingerprint` match. The crate depends on
[`seleniumbase-rs-codec`](/stealth/api/codec), and uses tokio, `psl` (the Public
Suffix List), `url`, `chrono`, `tracing` and `tracing-subscriber`.

## Features

The crate declares no Cargo features. Its own tests turn on `test-util` of the core
and `test-fixtures` of the codec.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `runtime::browser_scraper` | `BrowserScraper`, its builder, `BrowserFetch`, the launcher port. | BrowserScraper |
| `challenge::detect`, `challenge::markers`, `challenge::mitigation` | Challenge detection, the interstitial markers, the mitigation policy. | Challenges |
| `egress::route`, `egress::pool`, `egress::geo`, `egress::redact` | Proxy specs, the rotating pool, countries and locales, URL redaction. | Egress |
| `jar::store`, `jar::set_cookie` | The RFC 6265 cookie jar and the `Set-Cookie` parser. | Cookie jar |
| `passport::identity`, `passport::policy`, `runtime::identity_file` | `StealthIdentity`, `SessionPolicy`, the identity file. | Identity |
| `ledger::state`, `ledger::events`, `runtime::json_store`, `runtime::tracing_sink` | Per-site state and its stores, events and their sinks. | Ledger |
| `runtime::cdp`, `runtime::egress_switch` | CDP helpers and the egress switch. | Runtime helpers |
| `runtime::capture` | The capture tools behind `capture_tls` and `capture_hints`. | Capture |
| `error` | `ScrapeError`. | Errors |

## Re-exports at the root

| Root name | Module |
|---|---|
| `ChallengeKind`, `Detection`, `Evidence`, `ResponseView`, `Vendor` | `challenge::detect` |
| `Action`, `AttemptContext`, `GiveUpReason`, `MitigationPolicy`, `Mode` | `challenge::mitigation` |
| `CountryCode`, `GeoResolver`, `LocaleExt`, `StaticGeoResolver` | `egress::geo` |
| `ProxyPool`, `RotationStrategy` | `egress::pool` |
| `RedactedUrl`, `Secret` | `egress::redact` |
| `EgressKey`, `EgressRef` | `egress::route` |
| `ScrapeError` | `error` |
| `CookieJar` | `jar::store` |
| `CollectingEventSink`, `EventSink`, `NoopEventSink`, `ScraperEvent` | `ledger::events` |
| `DomainState`, `InMemoryStateStore`, `StateStore` | `ledger::state` |
| `EmulationRef`, `IdentityError`, `StealthIdentity` | `passport::identity` |
| `SessionPolicy`, `Transition` | `passport::policy` |
| `BrowserFetch`, `BrowserLauncher`, `BrowserScraper`, `BrowserScraperBuilder`, `ChromeLauncher` | `runtime::browser_scraper` |
| `ContextProxySwitch`, `EgressSwitch` | `runtime::egress_switch` |
| `JsonStateStore` | `runtime::json_store` |
| `TracingEventSink` | `runtime::tracing_sink` |

## Guides

The item-by-item pages for this crate are still being written. Until then, the
[BrowserScraper guide](/stealth/browser-scraper) and the
[runnable examples](/stealth/examples) show it in use.
