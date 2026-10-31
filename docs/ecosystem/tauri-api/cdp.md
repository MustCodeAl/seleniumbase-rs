---
title: "Chromium CDP"
sidebarTitle: "CDP discovery"
description: "Authenticated CDP discovery and CdpStealthConfig for app-owned CEF or WebView2, including fingerprint stealth and seleniumbase-rs plugins."
keywords: ["tauri", "cdp", "chromium", "cef", "webview2", "stealth", "debugger"]
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

```json 200 Ready
{
  "value": {
    "cdpEnabled": true,
    "cdpPort": 53412,
    "cdpUrl": "http://127.0.0.1:53412",
    "cdpState": "ready"
  }
}
```

```json 200 Not answering yet
{
  "value": {
    "cdpEnabled": false,
    "cdpPort": null,
    "cdpUrl": null,
    "cdpState": "unreachable"
  }
}
```

```json 200 Disabled
{
  "value": {
    "cdpEnabled": false,
    "cdpPort": null,
    "cdpUrl": null,
    "cdpState": "disabled"
  }
}
```

</ResponseExample>

## Discover the endpoint

<Badge color="blue">GET</Badge> `/cdp`

No WebDriver session is needed. The port comes from the launch settings below
(`CdpStealthConfig`, feature `cdp`) in the same process, read once when the
plugin starts; the environment of a parent process never sets it. The route
names that port only while Chromium answers `GET /json/version` there (it waits
up to one second): before Chromium has started, or while something else holds
the port, `cdpState` is `unreachable` and no port is named, so a client that
starts the app retries until it is `ready`. Without the `cdp` feature, or with
automation off, the route answers `disabled`. The existing automation gate
still controls the server.

**Path and headers**

<ParamField path="token" type="string" required>

`SBRS_AUTOMATION_TOKEN`, or the random token the app printed when it started.
A wrong token answers `404 unknown command`.

</ParamField>

<ParamField header="Origin" type="string">

Must be absent: a request that has one (even `null` or an empty value) answers
`403 unknown error`.

</ParamField>

**Response**

<ResponseField name="value" type="object" required>

<Expandable title="properties" defaultOpen>

<ResponseField name="cdpEnabled" type="boolean" required>

True only when `cdpState` is `ready`.

</ResponseField>

<ResponseField name="cdpPort" type="integer | null" required>

The debugging port while Chromium answers on it, else `null`.

</ResponseField>

<ResponseField name="cdpUrl" type="string | null" required>

`http://127.0.0.1:{cdpPort}`, else `null`.

</ResponseField>

<ResponseField name="cdpState" type="string" required>

`disabled` (no debugging port was turned on), `unreachable` (a port was turned
on but no Chromium debugger answers there yet, or another program holds it), or
`ready`.

</ResponseField>

</Expandable>

</ResponseField>

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | A supplied body is not valid JSON. |
| 403 | `unknown error` | Any `Origin` header, including `null` or an empty value. |
| 404 | `unknown command` | Wrong token, or a method other than `GET`. |

The probe confirms that a Chromium debugger listens on the port, not that the
browser is the app's own: another browser that took the port between the
plugin's choice and Chromium's start would pass. The token guards discovery; it
is not authentication for the separate Chromium socket. Both listeners must stay on loopback. The [status response](/ecosystem/tauri-api/status)
keeps its existing `ready` and `message` fields.

## Rust configuration: CdpStealthConfig

Enable the plugin's `cdp` feature. It adds no dependency: the few stealth flags
are kept in the plugin, so its minimum Rust version stays 1.88.

| Public field | Type | Default |
|---|---|---|
| `debugging_port` | `Option<u16>` | A valid nonzero `SBRS_CDP_PORT`, else `None`: a free loopback port, the same for the life of the process. A port that is asked for and already in use turns nothing on. |
| `stealth_args` | `bool` | `false`. Opt in to five flags that hide `navigator.webdriver` and stop background throttling; none of them turns off a protection of the app's browser (site isolation, the popup blocker and the encryption of stored data stay on). |
| `extra_args` | `Vec<String>` | Empty. Trusted additional Chromium arguments; the debugging switches are reserved. |

| Method | Result and behavior |
|---|---|
| `CdpStealthConfig::default()` | Creates the default configuration. |
| `debugging_port(self, port: u16) -> Self` | A fixed local port; zero asks for a free one, as `None` does. The port must be free when the arguments are built. |
| `stealth_args(self, enabled: bool) -> Self` | Opts in to (or out of) the stealth flags. Off by default. |
| `arg(self, arg: impl Into<String>) -> Self` | Appends an additional argument. |
| `chromium_args(&self) -> Vec<String>` | The loopback debugging switches, the opt-in stealth flags, then the additional arguments; nothing at all when the automation gate is off. |
| `port(&self) -> Option<u16>` | The port these settings use, for an app that turns on CEF's remote debugging itself; `None` when the gate is off or the port asked for is in use. |
| `unsafe apply_env(&self) -> Vec<String>` | Appends the arguments to `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` and returns them for CEF initialization; changes nothing when the gate is off. |
| `cdp::enabled_port() -> Option<u16>` | The port the launch settings turned on in this process, which the plugin reports. |

With the automation gate on (a debug build, or `SBRS_AUTOMATION` set to `1`,
`true` or `on`; any other value turns it off), `chromium_args()` includes
`--remote-debugging-port={port}` and `--remote-debugging-address=127.0.0.1`.
Additional arguments that mention `remote-debugging-`, `remote-allow-origins`
or `edge-webview-switches`, in any case and with any prefix (`--`, `-`, `/`),
or that contain a NUL byte, are dropped.

Call `apply_env()` before any threads or runtime initialization. Its safety
contract requires exclusive access to the process environment. It respects
the existing automation gate and returns an empty vector without changes when
that gate is off. Existing WebView2 arguments are preserved; reserved switches
in them, or a value that is not valid Unicode, make it return an empty vector.
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` stays in the environment, because
WebView2 reads it when it creates its environment: remove it with
`Command::env_remove` for a child process that must not inherit it.

```rust
use tauri_plugin_sbrs_automation::cdp::CdpStealthConfig;

fn main() {
    // A free loopback port; `.stealth_args(true)` opts in to the stealth flags.
    let config = CdpStealthConfig::default();
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
| `CdpDriver::attach(&mut self, browser)` | An instance method, not a static call: `let mut driver = CdpDriver::new(); driver.attach(browser).await?;` drives the attached browser's first window. |
| `BrowserConfig::with_cdp_target(window)` | The window `BaseCase` drives: a target id or part of its URL. Without it, the first window. |
| `Browser::select_page(window)` | The same choice on a `Browser`: a target id, else the first window whose URL contains the text. |

Use `BaseCase::cdp_*` methods for the directly attached page. Fingerprints use
the existing evasion provider registry for future documents and already-open
contexts, plus native CDP overrides when `native_spoofing` is enabled.
`with_plugins(...)` or `LaunchOptionsBuilder::plugins(...)` enables the existing
attach-compatible plugin hooks and packages (feature `plugins`). Launch-only
plugins are refused, and `before_launch` is not called.

### What attaching changes in the app

The browser belongs to the app, so attaching must not break or take over its
windows:

- **Only the windows you pick are prepared.** Attaching sends nothing to any
  window. The fingerprint, emulation and plugins reach a window when the
  automation opens it (`new_page`, `new_window`) or picks it: `default_page()`
  (the first window), `page(id)`, `select_page(id_or_url_part)`,
  `BrowserConfig::with_cdp_target(...)` for `BaseCase`, or `pages()`, which
  asks for every window. `list_pages()` only lists them. The app's other
  windows are left as they are.
- **A proxy is refused.** The app's network settings are not the test's to
  change, so `proxy`, `proxy_pac_url` and a fingerprint's proxy make the build
  fail with `SeleniumBaseError::InvalidConfig` (the login is never printed).
  Configure the proxy in the app before it starts.
- **A user agent or locale needs a fingerprint.** On their own they are launch
  flags, which cannot reach a running app, so they are refused. A fingerprint
  applies its own user agent and locale to the windows it prepares.
- **Closing detaches.** `Browser::close()` detaches every session it opened
  (`Target.detachFromTarget`), which ends the emulation, script injections and
  request interception of those windows, and resets the permissions and the
  download behaviour it set. It never sends `Browser.close` and never closes
  the app's windows. Every handle of that browser fails with a
  browser-disconnected error afterwards. Script that already ran in a page
  stays until the page navigates.
- **`reconnect()` re-attaches** to the endpoint in the configuration; it does
  not start a browser or a driver.

WebDriver-only helpers return `SeleniumBaseError::Unsupported` on such a
session instead of panicking (see [CDP and UC helpers](/api/cdp-uc)).

<Warning>

The Chromium debug socket itself is **not authenticated**: any process on the
machine that can reach `127.0.0.1:{port}` can drive every window of the app.
The plugin's token guards discovery only. Enable it in debug and test builds,
never in a release build.

</Warning>

See the [Tauri setup guide](/ecosystem/tauri#app-owned-chromium-cef-and-webview2)
for complete client examples. Launch flags and binary patches must be applied
before the app starts. This integration is covered by mock HTTP/CDP tests;
real CEF/WebView2 app behavior has not been verified in this change.
