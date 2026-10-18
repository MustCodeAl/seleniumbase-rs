---
title: "CDP Mode Guide"
---

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

<Info>

`DriverMode::Cdp` still uses WebDriver for the session lifecycle but activates
CDP domains so you can call raw CDP commands and use CDP-based helpers.

</Info>

## Activating CDP domains

Before sending raw CDP commands, activate CDP mode on the session:

```rust
sb.activate_cdp_mode().await?;
```

<Tip>

This is done automatically by helpers that need CDP, but explicit activation is
useful when you send raw commands directly.

</Tip>

## CDP helpers on BaseCase

| Method | Description |
|--------|-------------|
| `cdp_mouse_click(x, y)` | Dispatch a CDP mouse click at viewport coordinates: CSS pixels from the top-left of the visible page, without the scroll position. |
| `cdp_insert_text(text)` | Insert text into whatever has focus with one CDP command (`Input.insertText`): no key events, no selector, nothing cleared. `cdp_type_text` is its deprecated old name. |
| `cdp_click(css)` | Scroll the element into view and click its centre with CDP input events. |
| `cdp_click_element(css)` | Deprecated: it is `cdp_click`. It used to click at the element's place in the document, so it missed on a scrolled page. |
| `execute_cdp(method)` | Send a raw CDP command with no parameters. |
| `execute_cdp_with_params(method, params)` | Send a raw CDP command with JSON parameters. |
| `execute_cdp_cmd(command, params)` | Alias for `execute_cdp_with_params`. |
| `set_network_conditions(conditions)` | Throttle the connection via CDP. |
| `set_timezone(id)` | Override the browser timezone via CDP. |
| `set_geolocation(lat, lon, acc)` | Override geolocation via CDP. |
| `clear_browser_cache()` | Clear the HTTP cache via CDP. |
| `clear_browser_cookies()` | Clear cookies via CDP. |
| `get_cookies()` | Return cookies as JSON via CDP. |
| `cdp_fast_type(selector, text)` | Clear the field and insert text in one operation. |
| `cdp_wait_for_element_present(selector, timeout)` | Poll until the element exists in the DOM. |
| `cdp_select_option_by_index(selector, index)` | Select a dropdown option by zero-based index. |
| `cdp_click(selector)` | Wait up to 7 seconds for the element, scroll it into view and click its center with CDP input events. |
| `cdp_evaluate(expression)` | Evaluate JavaScript and return its value, dropping a final `return` as Python's `sb.cdp.evaluate` does; a script that throws is `CdpDriver`. |
| `cdp_get_text(selector)`, `cdp_get_attribute(selector, name)` | Read an element at once, without waiting; `ElementNotFound` if it is missing (`cdp_wait_for_element_present` first for a late one), and `None` for an attribute it lacks. |

### Typing: realistic or fast

`cdp_type` sends one key event per character, which is what pages watching for
genuine keyboard input expect. `cdp_fast_type` clears the field and inserts the
whole string in a single operation: much quicker, but it emits no per-character
key events.

<Warning>

`cdp_fast_type` is the wrong choice when you are trying not to stand out.

</Warning>

```rust,no_run
# use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};
# use std::time::Duration;
# async fn demo(sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
// Wait for the field, then type it the way a person would.
sb.cdp_wait_for_element_present("#search", Duration::from_secs(10)).await?;
sb.cdp_type("#search", "seleniumbase").await?;

// Or fill it as fast as possible when stealth does not matter.
sb.cdp_fast_type("#notes", "a long block of text").await?;

// Dropdowns can be chosen by text, value, or index.
sb.cdp_select_option_by_index("#country", 2).await?;
# Ok(())
# }
```

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

<Check>

- A page detects and blocks normal WebDriver commands.
- You need to set custom request headers or throttle the network.
- You want lower-level control over input events and JavaScript evaluation.
- You are building custom anti-detection tooling that needs raw protocol access.

</Check>

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `execute_cdp` fails | CDP not activated | Call `activate_cdp_mode()` first. |
| `cdp_mouse_click` misses | The coordinates are the document's, not the viewport's, on a scrolled page | Click by selector with `cdp_click`, which scrolls the element into view first. |
| Reactor does not intercept | Wrong debugger port | Verify the browser was launched with `--remote-debugging-port=9222`. |

## Related reading

- [Undetected (UC) Mode](/tutorials/uc_mode) — combining CDP with anti-detection.
- [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth) — using CDP for native spoofing.
- [Binary Patching](/tutorials/binary_patching) — removing driver-level markers.
