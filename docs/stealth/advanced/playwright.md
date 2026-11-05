---
title: "Stealth with Playwright mode"
sidebarTitle: "Playwright mode"
---

[Playwright mode](/help/playwright_mode) attaches rustwright to a stealthy
browser. This is how a [fingerprint](/stealth/fingerprints) reaches its pages.


<Badge color="blue">Crate: seleniumbase-rs-playwright</Badge>

A fingerprint belongs to the browser, not to the Playwright client. In Stealthy
Playwright Mode the pure CDP browser is launched with the fingerprint, and
rustwright attaches to it, so Playwright-style code sees the persona the browser
presents. The bridge is the separate crate `seleniumbase-rs-playwright`; the
library has no `playwright` feature.

```rust,no_run
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::{tokio, Fingerprint};
use seleniumbase_rs_playwright::StealthyPlaywright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(&Fingerprint::windows_desktop())
        .build()?;
    let stealthy = StealthyPlaywright::launch(options).await?;

    stealthy.playwright().goto("https://example.com").await?;
    // "Win32": the persona is still presented after the navigation.
    println!("{}", stealthy.playwright().evaluate("navigator.platform").await?);

    stealthy.close().await?;
    Ok(())
}
```

How the persona's claims reach the page (launch switches, protocol overrides or
the page script) is the same as for any pure CDP browser, described above; the
bridge adds no script of its own and attaching changes nothing in the browser.
`crates/playwright/tests/playwright_chrome.rs` checks this against a real Chrome.
See [Playwright mode](/help/playwright_mode).

