---
title: "seleniumbase-rs-scrape"
sidebarTitle: "scrape"
description: "API reference of seleniumbase-rs-scrape: stealth scraping without TLS spoofing over the pure CDP engine. BrowserScraper, challenge detection and mitigation, egress, the cookie jar, the stealth identity, the ledger and the capture tools."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`seleniumbase_rs_stealth::scrape` (implemented by `seleniumbase-rs-scrape` in `crates/scrape` of
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

## When to use it

Reach for scrape when a real browser has to load the page. `BrowserScraper` drives
one Chrome through `sb_cdp`: `fetch` loads the URL, the mitigation policy decides
whether to solve a challenge, rotate the proxy or rotate the profile, and the
country of the proxy sets the language and the time zone. Chrome's own TLS
handshake leaves the machine, so this crate builds no BoringSSL.

Reach for [mimic](/stealth/api/mimic) when the site judges the wire fingerprint:
the measured `ClientHello`, the HTTP/2 start and the header order, or when a
browser's cookies should continue over plain HTTP. That module is a facade
feature of its own and needs cmake.

<RequestExample>

```rust A scraper behind a German proxy
use seleniumbase_rs_stealth::scrape::{BrowserScraper, CountryCode, ScrapeError};
use seleniumbase_rs_stealth::tokio;

#[tokio::main]
async fn main() -> Result<(), ScrapeError> {
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

`BrowserScraper::builder` returns `BrowserScraperBuilder`.
`with_geo_proxies` takes `(spec, CountryCode)` pairs and returns
`Result<Self, ScrapeError>`. `CountryCode::new` returns
`Result<CountryCode, scrape::egress::geo::GeoError>`, and that error converts
into `ScrapeError`, which is why `?` sits inside the pair.
`with_max_challenge_attempts(u32)` returns the builder. `build().await` returns
`Result<BrowserScraper, ScrapeError>`. `fetch(&mut self, url)` returns
`Result<BrowserFetch, ScrapeError>`: `body` is the rendered page (`String`) and
`status` is the HTTP status (`u16`). `close(self)` takes the scraper by value
and returns `Result<(), ScrapeError>`.

## Depend on it

Use the unified root facade from Git:

```toml Cargo.toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

The facade's default feature is `scrape`, which also enables `codec`. That is
enough for this page, and it builds no BoringSSL. `sb_cdp`, `evasion` and
`fingerprint` are modules re-exported from `seleniumbase-rs`, which the facade
always depends on. They are not Cargo features. Import through
`seleniumbase_rs_stealth`. No separate core or implementation dependency is
needed.

## Features

The facade default is `scrape` (`default = ["scrape"]` in the stealth
repository's root `Cargo.toml`). `scrape` turns on `codec`. `mimic` stays off
unless the dependency asks for it.

`seleniumbase-rs-scrape` declares no Cargo features of its own. Its tests enable
`test-util` on `seleniumbase-rs` and `test-fixtures` on `seleniumbase-rs-codec`.

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

The call-by-call behaviour is on [BrowserScraper](/stealth/browser-scraper),
including the builder table. The jar, the identity file and the decision table
are on [Identity and cookies](/stealth/identity). The programs are on
[Runnable examples](/stealth/examples). The item-by-item pages for the pure
modules (`challenge`, `egress`, `jar`, `passport`, `ledger`) are still being
written; until then the re-export table above is the list of what
`seleniumbase_rs_stealth::scrape` exports, and the crate's rustdoc
(`cargo doc -p seleniumbase-rs-scrape --open` in the stealth repository) has the
rest.
