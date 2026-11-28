---
title: "Advanced stealthy scraping"
sidebarTitle: "Overview"
---

[Stealthy scraping](/stealth/session/overview) fetches with one tool at a
time. These pages are for jobs that need more: one identity that moves between
a browser and plain HTTP, cookies and identity files kept between runs,
challenge detection, and knowing where the layer stops. They are the API of
the [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
repository.

```rust,no_run
use seleniumbase_rs_stealth::session::{HybridSession, SessionError};

async fn fetch_with_hybrid() -> Result<(), SessionError> {
    let mut session = HybridSession::builder()
        .max_challenge_attempts(4)
        .build()
        .await?;

    let response = session.fetch("https://example.com/").await?;
    println!("status={} body_len={}", response.status, response.body.len());
    session.close().await
}
```

| Page | What it covers |
|---|---|
| [HybridSession](/stealth/advanced/session/hybrid-session) | A browser and an `HttpSession` sharing one identity, and moving between them |
| [Cookies, identity and challenges](/stealth/advanced/session/identity) | The cookie jar, the identity file and session policy, challenge detection |
| [Runnable examples](/stealth/advanced/session/examples) | The examples in the stealth repository, feature by feature |
| [Limits and troubleshooting](/stealth/advanced/session/limits) | What is measured and what is not, and what to do when a fetch fails |

The stealth they build on is described once, in the Stealth sections:
[Fingerprints](/stealth/identity/fingerprints), [Proxies and network](/stealth/network/proxies),
[Wire formats](/stealth/advanced/network/wire-formats) and
[MitmProxy](/stealth/advanced/network/mitm-proxy).
