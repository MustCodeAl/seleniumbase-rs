---
title: "UC Mode (Undetected Chromedriver)"
---

# UC Mode (Undetected Chromedriver)

UC Mode launches Chromium with stealth flags and applies JavaScript evasions that
mask `navigator.webdriver`, plugins, WebGL, and other fingerprints. Removing the
automation markers from the chromedriver binary itself is a separate step that
you run yourself (below).

## What you will learn

- How to enable UC mode.
- How binary patching removes injected markers.
- How engine spoofing args hide additional automation signals.
- What to do if a site still detects the browser.

## Quick enable

Set the driver mode to `Uc` in [`BrowserConfig`](crate::BrowserConfig):

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(
        BrowserConfig::default().with_mode(DriverMode::Uc),
    ).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

`BrowserConfig::default()` is headless.

<Warning>

UC mode needs Chrome, Chromium or Edge.

</Warning>

On the command line, `sbrs --uc open https://example.com` does the same (put
`--uc` before the subcommand).

## Binary patching

Even in UC mode, a stock `chromedriver` may inject `cdc_` variables into every
page.

<Note>

UC mode does not patch the driver for you.

</Note>

Patch the executable before launch to strip those signatures:

```bash
cargo run --bin sbrs -- patch-chromedriver --path /path/to/chromedriver
```

You can also patch from code:

```rust,no_run
use seleniumbase_rs::stealth::patcher::patch_chromedriver;

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
patch_chromedriver("/path/to/chromedriver")?;
# Ok(())
# }
```

See [Binary Patching](../tutorials/binary_patching.md) for the full API,
backup/restore, and safety notes.

## Engine spoofing arguments

The `engine_spoofing_args()` helper returns Chromium flags that disable
automation features such as `--disable-blink-features=AutomationControlled`.
Pass them through `BrowserConfig` or `StealthOptions`:

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

## Still detected?

If a site still flags the browser:

<Steps>

<Step title="Patch chromedriver">

Patch `chromedriver` and verify `needs_patch()` returns `false`.

</Step>

<Step title="Add the spoofing arguments">

Add `engine_spoofing_args()` to the launch flags.

</Step>

<Step title="Use a coherent fingerprint">

Use a [`Fingerprint`](/tutorials/fingerprint_stealth) with consistent
user agent, platform, screen size, WebGL vendor, and geolocation.

</Step>

<Step title="Run headful">

Run headful (`with_headless(false)`). A headless session already starts
Chrome with `--headless=new`, so there is no newer headless mode to switch to.

</Step>

<Step title="Align the egress IP">

Align egress IP with the spoofed geolocation and timezone.

</Step>

<Step title="Try the pure CDP engine">

Try the [pure CDP engine](/tutorials/pure_cdp), which needs no
chromedriver.

</Step>

</Steps>

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `navigator.webdriver` still `true` | chromedriver not patched | Run `patch-chromedriver` and verify `needs_patch()`. |
| Detection after patching | Missing engine args | Add `engine_spoofing_args()` to launch flags. |
| Detection only in headless | Headless artifacts | Run headful with `with_headless(false)` (or `sbrs --headed`). |
| Inconsistent signals | Fingerprint values mismatch | Validate with `Fingerprint::validate()`. |
