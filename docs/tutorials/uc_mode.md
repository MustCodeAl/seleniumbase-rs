# Undetected (UC) Mode Guide

Undetected (UC) mode reduces the fingerprints that websites use to identify
automated browsers. It combines Chromium launch flags, JavaScript evasions,
and optional binary patching to make the browser look more like a regular user
session.

This page explains how to enable UC mode, what it does automatically, and how to
layer additional evasions when a site is still detecting you.

## What you will learn

- How to enable UC mode with `BrowserConfig`.
- Which automation markers are masked automatically.
- How to spoof timezone, geolocation, user agent, and locale.
- How to patch `chromedriver` and add engine spoofing arguments.

## Enable UC mode

Set the driver mode to `Uc` in `BrowserConfig`:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    // Verify that navigator.webdriver is masked.
    let hidden: bool = sb.execute_script("return navigator.webdriver === undefined").await?;
    println!("navigator.webdriver hidden: {hidden}");

    sb.quit().await?;
    Ok(())
}
```

## What is patched automatically

When UC mode is active, the framework applies many evasions:

- `cdc_` variables injected by `chromedriver` are removed from the page context.
- `navigator.webdriver` is masked or removed.
- `navigator.plugins`, `navigator.languages`, `hardwareConcurrency`, and
  `deviceMemory` are mocked to realistic values.
- Chrome-only objects such as `chrome.app`, `chrome.runtime`, `chrome.csi`, and
  `chrome.loadTimes` are created.
- WebGL vendor and renderer report a common Intel profile.
- `navigator.permissions.query` returns natural values.
- `navigator.mediaDevices.enumerateDevices` returns realistic device labels.
- iframe `contentWindow` patches propagate the mask into nested frames.

## Extra evasions you can apply

### Timezone and geolocation

```rust
sb.set_timezone("America/Los_Angeles").await?;
sb.set_geolocation(34.0522, -118.2437, 100.0).await?;
```

### Custom user agent and locale

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...")
    .with_locale("en-US");
```

### Reconnect for a fresh session

```rust
sb.reconnect().await?;
```

`reconnect` obtains a fresh WebDriver session without restarting the driver
process. This is useful when a site invalidates the current session.

## Patch the chromedriver binary

Even in UC mode, a stock `chromedriver` may inject `cdc_` variables into every
page. Patch the executable to strip those signatures:

```bash
cargo run --bin sbase -- patch-chromedriver --path /path/to/chromedriver
```

From code:

```rust
use seleniumbase_rs::{ChromedriverPatcher, EnginePatch};

let patcher = ChromedriverPatcher::new("/path/to/chromedriver");
if patcher.needs_patch()? {
    patcher.patch(EnginePatch::balanced())?;
}
```

See [Binary Patching](./binary_patching.md) for backup, restore, and safety notes.

## Engine spoofing arguments

Add Chromium flags that disable automation telemetry:

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

`engine_spoofing_args()` returns flags such as
`--disable-blink-features=AutomationControlled` and disables background
networking and default apps.

## Still detected?

If a site still flags the browser, combine all layers:

1. Patch `chromedriver` with `ChromedriverPatcher` and verify `needs_patch()`
   returns `false`.
2. Add `engine_spoofing_args()` to the launch flags.
3. Use a coherent [`Fingerprint`](./fingerprint_stealth.md) with consistent user
   agent, platform, screen size, WebGL vendor, and geolocation.
4. Run headful (`headless: false`) or use `--headless=new` instead of the legacy
   headless implementation.
5. Enable `native_spoofing` to move spoofing out of JavaScript and into CDP
   where possible.

## When to use UC mode

Use UC mode when:

- A site blocks normal WebDriver traffic.
- You need to interact with Cloudflare-like challenges.
- You want the most human-like browser fingerprint possible.

Avoid UC mode when you do not need anti-detection, because the extra flags and
patches can make debugging harder and may conflict with some CI environments.

## Related reading

- [Fingerprint & Stealth Profiles](./fingerprint_stealth.md)
- [Binary Patching](./binary_patching.md)
- [CDP Mode](./cdp_mode.md)
