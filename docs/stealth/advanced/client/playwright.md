---
title: "Stealth with Playwright mode"
sidebarTitle: "Playwright mode"
description: "Combine seleniumbase-rs-stealth (StealthLayer, geo proxies, WebRTC shield, Humanizer, and audit checks) with rustwright via PlaywrightSidecar."
---

[Playwright mode](/guides/browser-engines/playwright-mode) attaches `rustwright` directly beside an undetected `seleniumbase-rs` browser tab. Because stealth lives on the launched browser and its CDP session rather than inside the Playwright client, Playwright calls inherit the full stealth stack automatically.

<Badge color="blue">Crate: seleniumbase-rs-playwright</Badge>

## 1. Launching with `StealthLayer` and attaching Playwright

Always configure stealth, fingerprints, and WebRTC policy on `LaunchOptions` **before** launching the browser so every target and iframe receives init scripts and protocol overrides before any page loads:

```rust,no_run
use seleniumbase_rs_playwright::rustwright::ActionOptions;
use seleniumbase_rs_playwright::PlaywrightSidecar;
use seleniumbase_rs_stealth::client::{
    LaunchOptionsFingerprintExt, PageFingerprintExt, StealthLayer,
};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::{tokio, BrowserApi};

#[tokio::main]
async fn main() -> seleniumbase_rs_stealth::Result<()> {
    // Apply all 35 balanced JS/CDP protections (or ::strict for pixel/audio noise)
    let layer = StealthLayer::balanced(OsType::Windows, 42);
    let options = LaunchOptions::builder()
        .headless(true)
        .shield_webrtc(true)
        .proxy("user:pass@proxy.example.com:8080")
        .build()?
        .with_stealth(&layer)?;

    let browser = Browser::launch(options).await?;
    let mut page = browser.default_page().await?;
    page.open("https://example.com").await?;

    // Attach Playwright to the stealthy tab
    let playwright = PlaywrightSidecar::attach(&page).await?;
    let platform = playwright
        .run(|pw| pw.evaluate("navigator.platform", None, ActionOptions::default()))
        .await?;
    println!("platform = {platform}");

    // Audit the tab for any internal contradictions after navigation
    let captured = page.capture_fingerprint().await?;
    assert!(captured.contradictions().is_empty());

    playwright.close().await?;
    browser.close().await
}
```

## 2. Isolated contexts with different identities & proxies

For multi-account or multi-region Playwright automation in one browser process, spawn isolated browser contexts with separate proxies and apply a custom `StealthLayer` per tab before navigating:

```rust,no_run
use seleniumbase_rs_playwright::rustwright::ActionOptions;
use seleniumbase_rs_playwright::PlaywrightSidecar;
use seleniumbase_rs_stealth::client::{PageFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType, StealthPolicy};
use seleniumbase_rs_stealth::sb_cdp::{Browser, ContextOptions, LaunchOptions, Proxy};

async fn run_isolated_context(browser: &Browser) -> seleniumbase_rs_stealth::Result<()> {
    let ctx = browser
        .new_context_with(ContextOptions::new().proxy(Proxy::parse("alice:secret@de.proxy.example:8080")?))
        .await?;

    let mut page = ctx.new_page().await?;
    let fp = Fingerprint::randomized(OsType::Macos, 7)
        .to_builder()
        .locale("de-DE")
        .timezone("Europe/Berlin")
        .policy(StealthPolicy::max().native_spoofing(true))
        .build()?;
    page.apply_stealth_layer(&StealthLayer::new(fp)).await?;
    page.goto("https://example.com").await?;

    let playwright = PlaywrightSidecar::attach(&page).await?;
    let tz = playwright
        .run(|pw| {
            pw.evaluate(
                "Intl.DateTimeFormat().resolvedOptions().timeZone",
                None,
                ActionOptions::default(),
            )
        })
        .await?;
    println!("timezone = {tz}");

    playwright.close().await?;
    ctx.close().await
}
```

## 3. Pairing human-paced interaction with Playwright inspection

Use `rustwright` for complex Playwright locators or interception, while dispatching sensitive form fills and clicks through `seleniumbase_rs::sb_cdp::human::Humanizer` so pointer motion and keystroke timing follow natural Fitts's-law and Gaussian curves:

```rust,no_run
use seleniumbase_rs_playwright::rustwright::ActionOptions;
use seleniumbase_rs_playwright::PlaywrightSidecar;
use seleniumbase_rs_stealth::sb_cdp::human::Humanizer;
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn human_submit_with_playwright(
    page: &Page,
) -> Result<(), Box<dyn std::error::Error>> {
    let human = Humanizer::seeded(101);
    page.human(&human).click("#username").await?;
    page.human(&human).type_text("#username", "alice").await?;
    page.human(&human).click("#submit").await?;

    let playwright = PlaywrightSidecar::attach(page).await?;
    let url = playwright.run(|pw| pw.url()).await?;
    println!("landed on {url}");
    playwright.close().await?;
    Ok(())
}
```

## 4. Using UC Mode (`BaseCase`) with `PlaywrightSidecar`

If your suite uses `BaseCase` (`DriverMode::Uc`) instead of pure CDP, attach `PlaywrightSidecar::attach(&sb)` after applying `BrowserConfigFingerprintExt::with_stealth(&layer)`:

```rust,no_run
use seleniumbase_rs_playwright::PlaywrightSidecar;
use seleniumbase_rs_stealth::client::{BrowserConfigFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::{BaseCase, BrowserConfig, DriverMode};

async fn uc_with_playwright() -> Result<(), Box<dyn std::error::Error>> {
    let layer = StealthLayer::balanced(OsType::Windows, 55);
    let config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .headless(true)
        .build()?
        .with_stealth(&layer)?;

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;

    let playwright = PlaywrightSidecar::attach(&sb).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("title = {title}");

    playwright.close().await?;
    sb.quit().await?;
    Ok(())
}
```

## Tips

- Attach `PlaywrightSidecar` to the exact `Page` or `BaseCase` tab you want to automate; detaching (`playwright.close().await?`) leaves the underlying Chromium browser and tab running cleanly.
- If a target still flags your traffic due to TLS/HTTP2 wire checks, launch Chrome behind `MitmProxy` and pass `--proxy-server` + `--ignore-certificate-errors-spki-list` on `LaunchOptions` before attaching Playwright ([MitmProxy](/stealth/advanced/network/mitm-proxy)).
- Tested against a real Chromium browser in `crates/playwright/tests/playwright_chrome.rs`. See [Playwright mode](/guides/browser-engines/playwright-mode) for setup details.
