---
title: "scrape::error"
sidebarTitle: "Errors"
description: "ScrapeError, the one error enum seleniumbase-rs-scrape returns."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

Every fallible call in the scrape crate returns `ScrapeError`. A URL on an error is a `RedactedUrl`. The enum is `non_exhaustive`. `GeoError`, `EgressError`, `StateError` and `IdentityError` convert into it.

See [BrowserScraper](/scraping/stealthy/browser-scraper).

<RequestExample>

```rust Match the scrape error
use seleniumbase_rs_stealth::scrape::ScrapeError;

fn explain(error: &ScrapeError) -> &'static str {
    match error {
        ScrapeError::NoEgressAvailable => "no other proxy",
        ScrapeError::CoolingDown { .. } => "site cooldown",
        ScrapeError::Unsolved { .. } => "challenge not passed",
        _ => "other scrape error",
    }
}
```

</RequestExample>

## `ScrapeError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum ScrapeError {
    Egress(#[from] EgressError),
    Geo(#[from] GeoError),
    State(#[from] StateError),
    Identity(#[from] IdentityError),
    Fingerprint(#[from] FingerprintError),
    Unsolved {
            /// The URL that was requested, redacted.
            url: RedactedUrl,
            /// The challenge that stood in the way.
            kind: ChallengeKind,
            /// How many attempts were made.
            attempts: u32,
        },
    GaveUp {
            /// The URL that was requested, redacted.
            url: RedactedUrl,
            /// Why the policy stopped.
            reason: GiveUpReason,
        },
    CoolingDown {
            /// The site, as a registrable domain.
            site: String,
            /// When it may be tried again, in seconds since the Unix epoch.
            until: u64,
        },
    Navigation {
            /// The URL that was requested, redacted.
            url: RedactedUrl,
            /// What the browser said.
            reason: String,
        },
    NoEgressAvailable,
    Browser(SeleniumBaseError),
}
```

Why a scrape did not produce a page.

## Related

The guide is [BrowserScraper](/scraping/stealthy/browser-scraper). The overview is [scrape](/stealth/api/scrape).
