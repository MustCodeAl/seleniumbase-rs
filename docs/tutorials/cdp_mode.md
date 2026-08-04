# CDP Mode Guide

CDP mode lets you control the browser through the Chrome DevTools Protocol
(CDP). It is useful when WebDriver is too high-level, when a site detects
WebDriver traffic, or when you need to manipulate network behavior, cache,
cookies, or headers.

This page explains how to enable CDP mode, use the CDP helpers on `BaseCase`,
send raw CDP commands, and intercept network requests.

## What you will learn

- How to enable CDP mode in `BrowserConfig`.
- Which CDP helpers are available on `BaseCase`.
- How to send raw CDP commands with and without parameters.
- How to use the `CdpReactor` to mutate request headers.

## Enable CDP mode

Set the driver mode to `Cdp`:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Cdp,
        ..Default::default()
    }).await?;

    sb.open("https://seleniumbase.io").await?;
    let text = sb.get_text("h1").await?;
    println!("{text}");

    sb.quit().await?;
    Ok(())
}
```

`DriverMode::Cdp` still uses WebDriver for the session lifecycle but activates
CDP domains so you can call raw CDP commands and use CDP-based helpers.

## Activating CDP domains

Before sending raw CDP commands, activate CDP mode on the session:

```rust
sb.activate_cdp_mode().await?;
```

This is done automatically by helpers that need CDP, but explicit activation is
useful when you send raw commands directly.

## CDP helpers on BaseCase

| Method | Description |
|--------|-------------|
| `cdp_mouse_click(x, y)` | Dispatch a CDP mouse click at screen coordinates. |
| `cdp_type_text(text)` | Insert text through CDP input events. |
| `cdp_click_element(css)` | Find an element and click its center via CDP. |
| `execute_cdp(method)` | Send a raw CDP command with no parameters. |
| `execute_cdp_with_params(method, params)` | Send a raw CDP command with JSON parameters. |
| `execute_cdp_cmd(command, params)` | Alias for `execute_cdp_with_params`. |
| `set_network_conditions(conditions)` | Throttle the connection via CDP. |
| `set_timezone(id)` | Override the browser timezone via CDP. |
| `set_geolocation(lat, lon, acc)` | Override geolocation via CDP. |
| `clear_browser_cache()` | Clear the HTTP cache via CDP. |
| `clear_browser_cookies()` | Clear cookies via CDP. |
| `get_cookies()` | Return cookies as JSON via CDP. |

## Raw CDP commands

For operations not covered by the helpers, send raw CDP commands through the
underlying session:

```rust
use serde_json::json;

sb.activate_cdp_mode().await?;

let version: serde_json::Value = sb.execute_cdp("Browser.getVersion").await?;
println!("{}", serde_json::to_string_pretty(&version)?);

sb.execute_cdp_with_params("Network.setCacheDisabled", json!({"cacheDisabled": true})).await?;
```

`execute_cdp` sends a command with no parameters. `execute_cdp_with_params`
accepts a `serde_json::Value` object of parameters.

## Common raw CDP commands

| Command | Purpose |
|---------|---------|
| `Browser.getVersion` | Get browser and protocol version. |
| `Network.setCacheDisabled` | Enable or disable the HTTP cache. |
| `Network.setUserAgentOverride` | Override user agent, accept language, and platform. |
| `Emulation.setDeviceMetricsOverride` | Override screen size and device scale factor. |
| `Emulation.setGeolocationOverride` | Override geolocation. |
| `Emulation.setTimezoneOverride` | Override timezone. |
| `Page.captureScreenshot` | Capture a screenshot. |

## Network reactor

The `CdpReactor` connects to a browser's debug WebSocket, enables the `Fetch`
domain, and continues every paused request with custom headers. This lets you
mutate requests in real time without blocking the main WebDriver loop.

```rust
use std::collections::HashMap;
use seleniumbase_rs::stealth::reactor::CdpReactor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut headers = HashMap::new();
    headers.insert("Accept-Language".into(), "en-US,en;q=0.9".into());

    let mut reactor = CdpReactor::start("127.0.0.1", 9222, headers).await?;

    // The reactor runs in the background and intercepts every request.
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    reactor.stop();
    Ok(())
}
```

`CdpReactor::start` takes the debugger host, port, and a map of header
overrides. It returns a reactor handle you can stop later with `stop()`.

## Network conditions

Throttle the connection through CDP to simulate slow networks:

```rust
use thirtyfour::extensions::cdp::NetworkConditions;

let mut conditions = NetworkConditions::new();
conditions.offline = false;
conditions.latency = 200;
conditions.download_throughput = 256 * 1024;
conditions.upload_throughput = 64 * 1024;

sb.set_network_conditions(&conditions).await?;
```

## Cache and cookies

Clear the browser cache and cookies via CDP:

```rust
sb.clear_browser_cache().await?;
sb.clear_browser_cookies().await?;
```

## When to use CDP mode

Use CDP mode when:

- A page detects and blocks normal WebDriver commands.
- You need to set custom request headers or throttle the network.
- You want lower-level control over input events and JavaScript evaluation.
- You are building custom anti-detection tooling that needs raw protocol access.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `execute_cdp` fails | CDP not activated | Call `activate_cdp_mode()` first. |
| `cdp_click_element` misses | Element scrolled out of view | Scroll to the element first. |
| Reactor does not intercept | Wrong debugger port | Verify the browser was launched with `--remote-debugging-port=9222`. |

## Related reading

- [Undetected (UC) Mode](./uc_mode.md) — combining CDP with anti-detection.
- [Fingerprint & Stealth Profiles](./fingerprint_stealth.md) — using CDP for native spoofing.
- [Binary Patching](./binary_patching.md) — removing driver-level markers.
