---
title: "Playwright Mode"
---

# Playwright Mode

Playwright Mode uses [`rustwright`](https://github.com/Skyvern-AI/rustwright), a
native Rust CDP engine, to drive Chromium with a Playwright-compatible API. It
can bypass bot-detection systems that target WebDriver fingerprints without
requiring a Node Playwright driver.

## What you will learn

- How to enable the optional `playwright` feature.
- How `rustwright` resolves Chromium.
- How to activate and use a Playwright session.

## Enable the feature

The feature is off by default, so a plain build does not compile `rustwright`:

```toml
[dependencies]
seleniumbase-rs = { version = "0.1", features = ["playwright"] }
```

Or on the command line:

```bash
cargo run --example playwright_mode --features playwright
```

## Chromium installation

`rustwright` finds a Chromium on the machine or downloads a build on first
launch (that is `rustwright`'s behavior, not this crate's). Ensure the build
host can reach the Chromium CDN, or install Chromium where `rustwright` looks
for it.

This crate passes `rustwright` no executable path: `BrowserConfig::browser_binary_path`
applies to the WebDriver sessions and has no effect on Playwright mode.

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
    session.close().await?;
}

sb.quit().await?;
# Ok(())
# }
```

`BaseCase::new` still connects a WebDriver browser first; `activate_playwright_mode`
then launches a second, separate Chromium through `rustwright`, in headed mode.
`session.close()` closes that one and `sb.quit()` closes the WebDriver one.

To use the Playwright session on its own, with no WebDriver browser, launch it
directly. `launch_headless` runs without a window, and
`launch_with_fingerprint` / `launch_headless_with_fingerprint` apply a
[`Fingerprint`](../tutorials/fingerprint_stealth.md):

```rust,no_run
# #[cfg(feature = "playwright")]
# async fn example() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
use seleniumbase_rs::browser::playwright::PlaywrightSession;

let session = PlaywrightSession::launch_headless().await?;
session.goto("https://example.com").await?;
println!("{}", session.get_text("h1").await?);
session.close().await?;
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
  [pure CDP engine](../tutorials/pure_cdp.md).

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `activate_playwright_mode` or `PlaywrightSession` not found | Feature not enabled | Build with `--features playwright`. |
| Chromium download fails | Network restricted | Make a Chromium available to `rustwright` ahead of time; this crate cannot pass it a path. |
| A program built with `playwright` cannot load a `libpython` | A dependency of yours turns on `rustwright-core`'s default `python` feature (Python bindings); this crate leaves it off | Depend on `rustwright-core` with `default-features = false` too; nothing here needs Python. |
| Session methods missing | API mismatch | Check the rustdocs for the installed `rustwright` version. |
