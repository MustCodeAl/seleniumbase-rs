---
title: "Stealth with Playwright mode"
sidebarTitle: "Playwright mode"
---

[Playwright mode](/help/playwright_mode) attaches rustwright beside a stealthy
browser's page. This is how a [fingerprint](/stealth/fingerprints) reaches its pages.


<Badge color="blue">Crate: seleniumbase-rs-playwright</Badge>

A fingerprint belongs to the browser, not to the Playwright client. In Stealthy
Playwright Mode the pure CDP browser is launched with the fingerprint, and
rustwright attaches to it, so Playwright-style code sees the persona the browser
presents. The bridge is the separate crate `seleniumbase-rs-playwright`; the
library has no `playwright` feature.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{tokio, BrowserApi, Fingerprint};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(&Fingerprint::windows_desktop())
        .build()?;
    let browser = Browser::launch(options).await?;

    // Our page opens the site ...
    let mut page = browser.default_page().await?;
    page.open("https://example.com").await?;

    // ... and the sidecar, attached to the same tab, evaluates in it.
    let playwright = PlaywrightSidecar::attach(&page).await?;
    // "Win32": the persona is still presented to the attached client.
    let platform = playwright
        .run(|pw| pw.evaluate("navigator.platform", None, Default::default()))
        .await?;
    println!("{platform}");

    playwright.close().await?;
    browser.close().await
}
```

How the persona's claims reach the page (launch switches, protocol overrides or
the page script) is the same as for any pure CDP browser, described above; the
bridge adds no script of its own and attaching changes nothing in the browser.
`crates/playwright/tests/playwright_chrome.rs` checks this against a real Chrome.
See [Playwright mode](/help/playwright_mode).

