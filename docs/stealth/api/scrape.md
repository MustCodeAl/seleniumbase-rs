---
title: "seleniumbase-rs-scrape"
sidebarTitle: "scrape"
description: "API reference of seleniumbase-rs-scrape: stealth scraping without TLS spoofing, over a real Chrome."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`seleniumbase_rs_stealth::scrape` (implemented by `seleniumbase-rs-scrape` in `crates/scrape` of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth))
does stealth scraping without TLS spoofing. A real Chrome, driven through `sb_cdp`, loads the page.
The pure modules decide what the response is, which proxy to use, and which cookies and identity
to keep. The `runtime` module does the I/O. The crate builds no BoringSSL and runs wherever Chrome does.

## When to use it

Reach for scrape when a real browser has to load the page: a challenge that runs in the document,
a script that must execute, or the DOM after render. `BrowserScraper` is the entry point.

Reach for [wire](/stealth/api/wire) when the site judges the wire fingerprint, or when a
browser's cookies should continue over plain HTTP. That crate is a separate facade feature and
needs cmake.

<RequestExample>

```rust A scraper behind a German proxy
use seleniumbase_rs_stealth::scrape::{BrowserScraper, CountryCode, ScrapeError};
use seleniumbase_rs_stealth::tokio;

#[tokio::main]
async fn main() -> Result<(), ScrapeError> {
    let mut scraper = BrowserScraper::builder()
        .geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
        .max_challenge_attempts(4)
        .build()
        .await?;
    let page = scraper.fetch("https://example.com/").await?;
    println!("{} bytes, status {}", page.body.len(), page.status);
    scraper.close().await
}
```

</RequestExample>

`BrowserScraper::builder` returns `BrowserScraperBuilder`. `geo_proxies` takes
`(spec, CountryCode)` pairs and returns `Result`. `CountryCode::new` returns
`Result<CountryCode, egress::geo::GeoError>`, and that error converts into `ScrapeError`.
`fetch(&mut self, url)` returns `BrowserFetch` (`body: String`, `status: u16`).
`close(self)` takes the scraper by value. Every signature is on
[BrowserScraper](/stealth/api/scrape-browser-scraper).

## Depend on it

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

The facade's default feature is `scrape`, which also enables `codec`. That build has no
BoringSSL. `sb_cdp`, `evasion` and `fingerprint` are modules re-exported from `seleniumbase-rs`,
which the facade always depends on. They are not Cargo features. Import through
`seleniumbase_rs_stealth`.

## Features

`default = ["scrape"]` in the stealth repository's root `Cargo.toml`. `scrape` turns on `codec`.
`wire` stays off unless the dependency asks for it.

`seleniumbase-rs-scrape` declares no Cargo features of its own. Its tests enable `test-util` on
`seleniumbase-rs` and `test-fixtures` on `seleniumbase-rs-codec`.

## Module map

| Module | What it holds | Page |
|---|---|---|
| `runtime::browser_scraper` | `BrowserScraper`, its builder, `BrowserFetch`, the launcher. | [BrowserScraper](/stealth/api/scrape-browser-scraper) |
| `challenge` | Detection, interstitial markers, the mitigation policy. | [Challenges](/stealth/api/scrape-challenge) |
| `egress` | Proxy specs, the pool, countries, URL redaction. | [Egress](/stealth/api/scrape-egress) |
| `jar` | The cookie jar and `Set-Cookie`. | [Cookie jar](/stealth/api/scrape-jar) |
| `passport` | `Identity` and `EscalationPolicy`. | [Identity](/stealth/api/scrape-passport) |
| `ledger` | Per-site state and events. | [Ledger](/stealth/api/scrape-ledger) |
| `runtime` (the rest) | Egress switch, identity file, stores, capture tools. | [Runtime](/stealth/api/scrape-runtime) |
| `error` | `ScrapeError`. | [Errors](/stealth/api/scrape-error) |

## Re-exports at the root

| Root name | Module |
|---|---|
| `ChallengeKind`, `Detection`, `ChallengeEvidence`, `ResponseView`, `ProtectionVendor` | `challenge::detect` |
| `Mitigation`, `AttemptContext`, `GiveUpReason`, `MitigationPolicy`, `FetchKind` | `challenge::mitigation` |
| `CountryCode`, `GeoResolver`, `LocaleExt`, `StaticGeoResolver` | `egress::geo` |
| `ProxyPool`, `RotationStrategy` | `egress::pool` |
| `RedactedUrl`, `Secret` | `egress::redact` |
| `ProxyKey`, `ProxyRoute` | `egress::route` |
| `ScrapeError` | `error` |
| `CookieJar` | `jar::store` |
| `CollectingEventSink`, `EventSink`, `NoopEventSink`, `ScrapeEvent` | `ledger::events` |
| `SiteRecord`, `InMemoryLedger`, `Ledger` | `ledger::state` |
| `WireMeasurementRef`, `IdentityError`, `Identity` | `passport::identity` |
| `EscalationPolicy`, `Escalation` | `passport::policy` |
| `BrowserFetch`, `BrowserLauncher`, `BrowserScraper`, `BrowserScraperBuilder`, `ChromeLauncher` | `runtime::browser_scraper` |
| `ContextProxySwitch`, `ProxySwitch` | `runtime::proxy_switch` |
| `JsonLedger` | `runtime::json_store` |
| `TracingEventSink` | `runtime::tracing_sink` |

`detect`, `GeoError` and `StateError` stay in their modules: `challenge::detect::detect`,
`egress::geo::GeoError`, `ledger::state::StateError`.

## Guides

[BrowserScraper](/scraping/stealthy/browser-scraper) is the fetch loop. [Identity and cookies](/scraping/advanced-stealthy/identity)
is the jar and the decision table. [Runnable examples](/scraping/advanced-stealthy/examples) is the programs.
[Limits](/scraping/advanced-stealthy/limits) is what is measured.
