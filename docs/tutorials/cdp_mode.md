# CDP Mode Guide

CDP Mode lets you control the browser directly through the Chrome DevTools Protocol WebSocket. It is useful for bypassing bot-detection, modifying network behavior, and performing actions that WebDriver does not expose.

## Enable CDP mode

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Cdp,
        ..Default::default()
    }).await?;

    sb.cdp_open("https://seleniumbase.io").await?;
    let text = sb.cdp_get_text("h1").await?;
    println!("{text}");

    sb.quit().await?;
    Ok(())
}
```

## CDP page API

| Method | Description |
|--------|-------------|
| `cdp_open(url)` | Navigate to a URL through CDP. |
| `cdp_click(css)` | Click via CDP DOM/Input dispatch. |
| `cdp_type(css, text)` | Type via CDP input events. |
| `cdp_get_text(css)` | Read element text via CDP. |
| `cdp_evaluate(js)` | Evaluate JavaScript in the page. |
| `cdp_screenshot(path)` | Save a screenshot via CDP. |

## Raw CDP commands

Send any CDP command through the underlying session:

```rust
let version: serde_json::Value = sb.execute_cdp("Browser.getVersion").await?;
sb.execute_cdp_with_params("Network.setCacheDisabled", json!({"cacheDisabled": true})).await?;
```

## Standalone CDP driver

For browser instances you manage yourself, use `CdpDriver` directly:

```rust
use seleniumbase_rs::stealth::cdp_driver::CdpDriver;

let driver = CdpDriver::new("ws://127.0.0.1:9222/devtools/browser/...").await?;
driver.send("Target.createTarget", json!({"url": "https://example.com"})).await?;
```

## Network reactor

The `CdpReactor` intercepts every network request and mutates headers:

```rust
use std::collections::HashMap;
use seleniumbase_rs::stealth::reactor::CdpReactor;

let mut headers = HashMap::new();
headers.insert("Accept-Language".into(), "en-US,en;q=0.9".into());
let reactor = CdpReactor::start("127.0.0.1", 9222, headers).await?;
```

## When to use CDP mode

- A page detects and blocks normal WebDriver commands.
- You need to set custom request headers or throttle the network.
- You want lower-level control over input events and evaluation.
