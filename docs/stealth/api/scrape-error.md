---
title: "session::error"
sidebarTitle: "Errors"
description: "SessionError, the one error enum seleniumbase-rs-stealth (`session`) returns."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Layer: session</Badge>

Every fallible call in the scrape crate returns `SessionError`. A URL on an error is a `RedactedUrl`. The enum is `non_exhaustive`. `ProxyRouteError`, `GeoError`, `LedgerError`, `IdentityError` and the core's `FingerprintError` convert into it.

See [CdpSession](/scraping/stealthy/browser-scraper).

<RequestExample>

```rust Match the scrape error
use seleniumbase_rs_stealth::session::SessionError;

fn explain(error: &SessionError) -> &'static str {
    match error {
        SessionError::NoProxyLeft => "no other proxy",
        SessionError::CoolingDown { .. } => "site cooldown",
        SessionError::Unsolved { .. } => "challenge not passed",
        _ => "other scrape error",
    }
}
```

</RequestExample>

## `SessionError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum SessionError {
    Proxy(#[from] ProxyRouteError),
    Geo(#[from] GeoError),
    Ledger(#[from] LedgerError),
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
    NoProxyLeft,
    Browser(SeleniumBaseError),
}
```

Why a scrape did not produce a page.

## Related

The guide is [CdpSession](/scraping/stealthy/browser-scraper). The overview is [scrape](/stealth/api/scrape).
