# Undetected (UC) Mode Guide

UC mode reduces the fingerprints that sites use to detect automated browsers.

## Enable UC mode

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc);

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    //navigator.webdriver is masked, plugins are mocked, and WebGL vendor is spoofed.
    let webdriver: bool = sb.execute_script("return navigator.webdriver === undefined").await?;
    println!("webdriver hidden: {}", webdriver);

    sb.quit().await?;
    Ok(())
}
```

## What is patched automatically

- `cdc_` variables injected by chromedriver are removed.
- `navigator.webdriver` is masked.
- `navigator.plugins`, `navigator.languages`, `hardwareConcurrency`, `deviceMemory` are mocked.
- Chrome-only objects (`chrome.app`, `chrome.runtime`, `chrome.csi`, `chrome.loadTimes`) are created.
- WebGL vendor/renderer report a common Intel profile.
- `navigator.permissions.query` returns natural values.
- `enumerateDevices` returns realistic media device labels.
- iframe `contentWindow` patches propagate the mask into nested frames.

## Extra evasions you can apply

```rust
// Spoof timezone and geolocation
sb.set_timezone("America/Los_Angeles").await?;
sb.set_geolocation(34.0522, -118.2437, 100.0).await?;

// Use a custom user agent and locale
let config = BrowserConfig {
    user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...".into()),
    locale: Some("en-US".into()),
    ..BrowserConfig::default().with_mode(DriverMode::Uc)
};

// Obtain a fresh WebDriver session without restarting the driver process
sb.reconnect().await?;
```

## Patch chromedriver binary

You can also patch a downloaded `chromedriver` executable to strip hardcoded CDC signatures:

```bash
cargo run --bin sbase -- patch-chromedriver --path /path/to/chromedriver
```

## Advanced: `StealthOptions`

For direct control over launch arguments, use `StealthOptions`. It builds the same Chrome/Edge flags used by UC mode and can be applied to any `ChromiumLikeCapabilities` value.

```rust
use std::collections::HashMap;
use seleniumbase_rs::stealth::options::StealthOptions;
use thirtyfour::{DesiredCapabilities, BrowserCapabilitiesHelper};

let mut caps = DesiredCapabilities::chrome();
let mut headers = HashMap::new();
headers.insert("Accept-Language".to_owned(), "en-US,en;q=0.9".to_owned());

let opts = StealthOptions {
    uc: true,
    headless: true,
    user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...".into()),
    window_size: Some("1920,1080".into()),
    extra_headers: headers,
    ..Default::default()
};
opts.apply_to(&mut caps).unwrap();
assert!(caps.args().contains(&"--disable-blink-features=AutomationControlled".into()));
```

## Detached chromedriver process helpers

`stealth::dprocess` locates and spawns chromedriver as a detached background process. This is useful when you want the driver to survive the parent process or when managing multiple browsers manually.

```rust
use seleniumbase_rs::stealth::dprocess::{find_chromedriver, start_chromedriver};

if let Some(path) = find_chromedriver() {
    let child = start_chromedriver(&path, 9514, true).unwrap();
    println!("started chromedriver with pid {:?}", child.id());
}
```

## CDP network reactor

The `CdpReactor` connects to a browser's debug WebSocket, enables the `Fetch` domain, and continues every paused request with custom headers. This lets you mutate requests in real time without blocking the main WebDriver loop.

```rust
use std::collections::HashMap;
use seleniumbase_rs::stealth::reactor::CdpReactor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut headers = HashMap::new();
    headers.insert("Accept-Language".into(), "en-US,en;q=0.9".into());

    let reactor = CdpReactor::start("127.0.0.1", 9222, headers).await?;
    // reactor runs in the background and intercepts every request.
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    reactor.stop().await;
    Ok(())
}
```

## When to use UC mode

Use UC mode when:
- A site blocks normal WebDriver traffic.
- You need to interact with cloudflare-like challenges.
- You want the most human-like browser fingerprint possible.
