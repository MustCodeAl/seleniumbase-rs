---
title: "Playwright Mode"
---

<Badge color="blue">Feature: playwright</Badge>

Playwright Mode uses [`rustwright`](https://github.com/Skyvern-AI/rustwright), a
native Rust CDP engine, to drive Chromium with a Playwright-compatible API. It
can bypass bot-detection systems that target WebDriver fingerprints without
requiring a Node Playwright driver.

<Info>

**What you will learn**

- How to enable the optional `playwright` feature.
- How `rustwright` resolves Chromium.
- How to activate and use a Playwright session.

</Info>

<Visibility for="agents">

Summary for AI exports: Playwright Mode needs the `playwright` Cargo feature, which is off by default. `BaseCase::activate_playwright_mode` starts a second Chromium through `rustwright` next to the WebDriver one, and `sb.quit()` closes both. `PlaywrightSession::launch_with_config` starts a session on its own, and `PlaywrightSession::connect_over_cdp` attaches to a browser that is already running.

</Visibility>

## Enable the feature

The feature is off by default, so a plain build does not compile `rustwright`.
Enable it in `Cargo.toml` or on the command line:

<CodeGroup>

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["playwright"] }
```

```bash Command line
cargo run --example playwright_mode --features playwright
```

</CodeGroup>

## Chromium installation

`rustwright` finds a Chromium on the machine or downloads a build on first
launch (that is `rustwright`'s behavior, not this crate's). Ensure the build
host can reach the Chromium CDN, or install Chromium where `rustwright` looks
for it.

To use a Chromium of your own, give `BrowserConfig::browser_binary_path` (or
`SB_CHROME_BIN`); `launch_with_config` and `activate_playwright_mode` pass it on.

## Example

```rust,no_run
# #[cfg(feature = "playwright")]
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use seleniumbase_rs::{BaseCase, BrowserConfig};

let mut sb = BaseCase::new(BrowserConfig::default()).await?;

sb.activate_playwright_mode().await?;

if let Some(session) = sb.playwright_session() {
    session.goto("https://example.com").await?;
    let heading = session.get_text("h1").await?;
    println!("Page heading: {heading}");
}

// Closes the Playwright browser and the WebDriver one.
sb.quit().await?;
# Ok(())
# }
```

<Note>

`BaseCase::new` still connects a WebDriver browser first; `activate_playwright_mode`
then launches a second, separate Chromium through `rustwright`. It is started as
the `BrowserConfig` you gave `BaseCase::new` says: headless or not, proxy, user
agent, locale, fingerprint, binary path, window size and extra Chrome switches.
The one setting it leaves out is `user_data_dir`: Chrome locks a profile folder,
and the WebDriver browser holds it. `sb.quit()` closes the Playwright browser as
well, so leave that to it. The WebDriver methods of the case act on the first
browser, not on the Playwright one.

</Note>

To use the Playwright session on its own, with no WebDriver browser, launch it
directly with `launch_with_config`. The configuration decides whether it runs
without a window (`BrowserConfig::default()` is headless; `with_headless(false)`
opens one), which browser binary, proxy, user agent, window size and [`Fingerprint`](/tutorials/fingerprint_stealth) it
uses. `launch_headless`, `launch_with_fingerprint` and
`launch_headless_with_fingerprint` still work but are deprecated shortcuts for
that:

```rust,no_run
# #[cfg(feature = "playwright")]
# async fn example() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::BrowserConfig;

let config = BrowserConfig::default().with_headless(true);
let session = PlaywrightSession::launch_with_config(&config).await?;
session.goto("https://example.com").await?;
println!("{}", session.get_text("h1").await?);
session.close().await?;
# Ok(())
# }
```

To drive a browser that is already running, attach to its DevTools endpoint
with `connect_over_cdp`. The session works on the page the browser has open,
and `close` only detaches, so the browser keeps running. This is Python's own
route to Playwright, `sb.cdp.get_endpoint_url()` and `connect_over_cdp`:

```rust,no_run
# #[cfg(feature = "playwright")]
# async fn example() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

let browser = Browser::launch(LaunchOptions::default()).await?;
let session = PlaywrightSession::connect_over_cdp(&browser.http_url()).await?;
println!("{}", session.evaluate("document.title").await?);
session.close().await?; // detaches
browser.close().await?;
# Ok(())
# }
```

## Notes

- Playwright mode is optional; the default build does not include the
  `rustwright` engine.
- The session API is async and mirrors the WebDriver-backed helpers where
  practical (`goto`, `click`, `type_text`, `get_text`, `evaluate`, `screenshot`,
  `close`, plus `new_page`). `src/api/playwright.rs` has thin `pw_*` function
  wrappers over the same methods.
- For WebDriver-based stealth use `DriverMode::Uc` or `DriverMode::Cdp`. For a
  Chrome session with no WebDriver at all and no extra dependency, see the
  [pure CDP engine](/tutorials/pure_cdp).

## Troubleshooting

<AccordionGroup>

<Accordion title="`activate_playwright_mode` or `PlaywrightSession` not found" icon="puzzle-piece">

**Likely cause.** Feature not enabled.

**Fix.** Build with `--features playwright`.

</Accordion>

<Accordion title="Chromium download fails" icon="download">

**Likely cause.** Network restricted.

**Fix.** Point `BrowserConfig::browser_binary_path` (or `SB_CHROME_BIN`) at a local Chromium and launch with `PlaywrightSession::launch_with_config`.

</Accordion>

<Accordion title="A program built with `playwright` cannot load a `libpython`" icon="triangle-exclamation">

**Likely cause.** A dependency of yours turns on `rustwright-core`'s default `python` feature (Python bindings); this crate leaves it off.

**Fix.** Depend on `rustwright-core` with `default-features = false` too; nothing here needs Python.

</Accordion>

<Accordion title="Session methods missing" icon="magnifying-glass">

**Likely cause.** API mismatch.

**Fix.** Check the rustdocs for the installed `rustwright` version.

</Accordion>

</AccordionGroup>
