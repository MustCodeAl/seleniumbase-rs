---
title: "Chromium CDP"
sidebarTitle: "CDP discovery"
description: "Authenticated CDP discovery and CdpStealthConfig for app-owned CEF or WebView2, including fingerprint stealth and seleniumbase-rs plugins."
---

Discover the debugger of an app that owns its Chromium runtime: CEF on
macOS/Linux or WebView2 on Windows. Paths follow
`http://127.0.0.1:4445/{token}`; the [overview](/ecosystem/tauri-api/overview)
defines authentication and the reply format.

<RequestExample>

```bash Discover CDP
curl -s "http://127.0.0.1:4445/$TOKEN/cdp"
```

```bash Origin is refused
curl -s "http://127.0.0.1:4445/$TOKEN/cdp" -H 'Origin: null'
```

</RequestExample>

<ResponseExample>

```json 200 Configured
{
  "value": {
    "cdpEnabled": true,
    "cdpPort": 9222,
    "cdpUrl": "http://127.0.0.1:9222"
  }
}
```

```json 200 Not configured
{
  "value": {
    "cdpEnabled": false,
    "cdpPort": null,
    "cdpUrl": null
  }
}
```

</ResponseExample>

## Discover the endpoint

<Badge color="blue">GET</Badge> `/cdp`

No WebDriver session is needed. The route reports configuration, not whether
Chromium has finished starting. It is available without the `cdp` Cargo feature;
that feature enables the Rust configuration helper below. A valid
`SBRS_CDP_PORT` is an integer from 1 to 65535. Absent, invalid or zero, CDP is
reported as disabled. The existing automation gate still controls the server.

<ResponseField name="value" type="object" required>

<Expandable title="properties" defaultOpen>

<ResponseField name="cdpEnabled" type="boolean" required>

Whether automation is enabled and `SBRS_CDP_PORT` names a valid nonzero port.

</ResponseField>

<ResponseField name="cdpPort" type="integer | null" required>

The configured debugging port, or `null` when disabled.

</ResponseField>

<ResponseField name="cdpUrl" type="string | null" required>

`http://127.0.0.1:{cdpPort}`, or `null` when disabled.

</ResponseField>

</Expandable>

</ResponseField>

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | A supplied body is not valid JSON. |
| 403 | `unknown error` | Any `Origin` header, including `null` or an empty value. |
| 404 | `unknown command` | Wrong token, or a method other than `GET`. |

The token guards discovery; it is not authentication for the separate Chromium
socket. Both listeners must stay on loopback. The [status response](/ecosystem/tauri-api/status)
keeps its existing `ready` and `message` fields.

## Rust configuration: CdpStealthConfig

Enable the plugin's `cdp` feature with Rust 1.99. The helper calls the existing
`seleniumbase_rs::stealth::engine_spoofing_args()` through a pinned Git dependency.

| Public field | Type | Default |
|---|---|---|
| `debugging_port` | `u16` | Valid nonzero `SBRS_CDP_PORT`, otherwise 9222. An explicit zero also uses 9222 when arguments are built. |
| `stealth_args` | `bool` | `true`: include the canonical engine stealth switches. |
| `extra_args` | `Vec<String>` | Empty. Trusted additional Chromium arguments. |

| Method | Result and behavior |
|---|---|
| `CdpStealthConfig::default()` | Creates the default configuration. |
| `debugging_port(self, port: u16) -> Self` | Selects the local debugging port. |
| `stealth_args(self, enabled: bool) -> Self` | Toggles canonical stealth arguments. |
| `arg(self, arg: impl Into<String>) -> Self` | Appends an additional argument. |
| `chromium_args(&self) -> Vec<String>` | Returns debugging switches, optional canonical stealth switches, then additional arguments. |
| `unsafe apply_env(&self) -> Vec<String>` | Sets `SBRS_CDP_PORT`, appends WebView2 arguments, and returns arguments for CEF initialization. |

`chromium_args()` always includes `--remote-debugging-port={port}` and
`--remote-debugging-address=127.0.0.1`. Additional arguments containing
`remote-debugging-`, `edge-webview-switches` or a NUL byte are ignored.

Call `apply_env()` before any threads or runtime initialization. Its safety
contract requires exclusive access to the process environment. It respects
the existing automation gate and returns an empty vector without changes when
that gate is off. Existing WebView2 arguments are preserved; conflicting pipe
or nested WebView2 switches also cause it to return an empty vector.

```rust
use tauri_plugin_sbrs_automation::cdp::CdpStealthConfig;

fn main() {
    let config = CdpStealthConfig::default().debugging_port(9222);
    // SAFETY: no threads or runtime have been started in this process.
    let args = unsafe { config.apply_env() };
    // Pass args to your app's CEF initialization before creating windows.
    // WebView2 reads WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS at startup.
}
```

The app initializes CEF itself. The helper does not select a Tauri runtime;
the existing `tauri-runtime-wry` integration remains available for WebView2.
System WKWebView and WebKitGTK use the WebDriver bridge instead of CDP.

## Client attachment and stealth

| Existing client surface | Configuration |
|---|---|
| `BrowserConfig::with_cdp_url(url)` | Direct HTTP or browser WebSocket debugger URL; an authenticated plugin base URL also supports discovery. |
| `BrowserConfig::with_cdp_discovery(true)` | Discover through the existing `webdriver_url`. Explicit `cdp_url` takes precedence. |
| `LaunchOptionsBuilder::cdp_url(url)` | `Browser::launch(options)` attaches without launching a process. |
| `LaunchOptionsBuilder::browser_config(&config)` | Carries CDP selection, fingerprint and plugins into the pure engine. |
| `BrowserSession::pure_cdp_page()` | Access the full attached `Page` and its `Browser`. |
| `CdpDriver::attach(browser)` | Use the existing driver with the attached browser. |

Use `BaseCase::cdp_*` methods for the directly attached page. Fingerprints use
the existing evasion provider registry for future documents and already-open
contexts, plus native CDP overrides when `native_spoofing` is enabled.
`with_plugins(...)` or `LaunchOptionsBuilder::plugins(...)` enables the existing
attach-compatible plugin hooks and packages (feature `plugins`). Launch-only
plugins are refused, and `before_launch` is not called. Closing the connection
leaves the app running.

See the [Tauri setup guide](/ecosystem/tauri#app-owned-chromium-cef-and-webview2)
for complete client examples. Launch flags and binary patches must be applied
before the app starts. This integration is covered by mock HTTP/CDP tests;
real CEF/WebView2 app behavior has not been verified in this change.
