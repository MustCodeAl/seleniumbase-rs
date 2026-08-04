# UC Mode (Undetected Chromedriver)

UC Mode bypasses bot detection by removing automation markers from the
chromedriver binary, launching Chromium with stealth flags, and applying
JavaScript evasions that mask `navigator.webdriver`, plugins, WebGL, and other
fingerprints.

## What you will learn

- How to enable UC mode.
- How binary patching removes injected markers.
- How engine spoofing args hide additional automation signals.
- What to do if a site still detects the browser.

## Quick enable

Set the driver mode to `Uc` in [`BrowserConfig`](crate::BrowserConfig):

```rust
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

## Binary patching

Even in UC mode, a stock `chromedriver` may inject `cdc_` variables into every
page. Patch the executable before launch to strip those signatures:

```bash
cargo run --bin sbase -- patch-chromedriver --path /path/to/chromedriver
```

You can also patch from code:

```rust
use seleniumbase_rs::patch_chromedriver;

patch_chromedriver("/path/to/chromedriver")?;
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

1. Patch `chromedriver` and verify `needs_patch()` returns `false`.
2. Add `engine_spoofing_args()` to the launch flags.
3. Use a [`Fingerprint`](../tutorials/fingerprint_stealth.md) with consistent
   user agent, platform, screen size, WebGL vendor, and geolocation.
4. Run headful (`headless: false`) or use `--headless=new` instead of the old
   headless implementation.
5. Align egress IP with the spoofed geolocation and timezone.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `navigator.webdriver` still `true` | chromedriver not patched | Run `patch-chromedriver` and verify `needs_patch()`. |
| Detection after patching | Missing engine args | Add `engine_spoofing_args()` to launch flags. |
| Detection only in headless | Legacy headless artifacts | Run headful or pass `--headless=new`. |
| Inconsistent signals | Fingerprint values mismatch | Validate with `Fingerprint::validate()`. |
