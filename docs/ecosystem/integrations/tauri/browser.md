---
title: "Drive your Tauri app's own webview (browser)"
sidebarTitle: "Drive the app's webview (browser)"
description: "Attach to a Tauri app's own Chromium webview (CEF or WebView2) over CDP from a test, or give the webview a fingerprint inside a shipped app with no debugging port."
keywords: ["tauri", "cdp", "cef", "webview2", "chromium", "debugger", "fingerprint", "stealth", "webview"]
---

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

<Badge color="orange" shape="pill">Early</Badge> <Badge color="purple" shape="pill">Tauri 3 alpha</Badge> <Badge color="gray" shape="pill" stroke>CEF and WebView2: mock-tested only</Badge>

In this guide the browser is your app's own webview. There are two ways to
work with it, and you can use either or both:

- **From a test:** [attach to the app's Chromium webview](#attach-to-the-webview-from-a-test)
  over CDP and run the `BaseCase` or `Page` API on it.
- **From a shipped app:** [give a webview a fingerprint](#give-your-shipped-webview-a-fingerprint)
  with no debugging port, and drive it from inside the app.

This is one of three Tauri guides. To drive the app's window with WebDriver,
read [Test a Tauri app](/ecosystem/integrations/tauri/driver). To start a separate browser
from your app's Rust code, read
[Automate a browser from your app](/ecosystem/integrations/tauri/client).

<Visibility for="agents">

Summary for AI exports: this applies to an app that owns a Chromium runtime, CEF (`tauri-runtime-cef`) or WebView2 on Windows. Attach from a test: enable the plugin's `cdp` Cargo feature, call `CdpStealthConfig::default()` and `unsafe { config.apply_env() }` before any thread or runtime start (CEF: pass the returned arguments to CEF), read the port from `GET /{token}/cdp`, and attach with `BrowserConfigBuilder::webdriver_url(..).cdp_discovery(true)` or `LaunchOptions::builder().cdp_url(..)` and `Browser::launch`. Fingerprint in a shipped app: enable the plugin's `stealth` feature (Rust 1.100), add `StealthPlan::new(&fingerprint)`'s init script with `.stealth_script(&plan)` on the `WebviewWindowBuilder`, send the overrides with `DevToolsChannel::apply`, and get a `Page` from `DevToolsChannel::page`. WKWebView and WebKitGTK speak no CDP: use the driver guide.

</Visibility>

## Which webview speaks what

| Webview | Speaks CDP | Attach from a test | Fingerprint inside the app |
|---|---|---|---|
| CEF (`tauri-runtime-cef`), on macOS, Windows and Linux | yes | yes | init script and overrides |
| WebView2, on Windows | yes | yes | init script and overrides |
| WKWebView (macOS), WebKitGTK (Linux) | no | no: use [the driver guide](/ecosystem/integrations/tauri/driver) | init script only |

The app owns and initializes a Chromium runtime. The plugin does not choose a
Tauri runtime for it.

## Attach to the webview from a test

<Steps>

<Step title="Turn on the cdp feature" icon="cube">

Enable the plugin's `cdp` Cargo feature, in the same optional dependency that
the `e2e` feature of the [driver guide](/ecosystem/integrations/tauri/driver) turns on, so a
release build has neither. The feature adds no dependency.

```toml src-tauri/Cargo.toml
[features]
e2e = ["dep:tauri-plugin-sbrs-automation"]

[dependencies]
tauri-plugin-sbrs-automation = { git = "https://github.com/MustCodeAl/tauri-plugin-sbrs-automation", optional = true, features = ["cdp"] }
```

</Step>

<Step title="Set the debugging port before the runtime starts" icon="gear">

Call this at the start of `main`, before any thread and before the runtime
initializes:

```rust src-tauri/src/main.rs
fn main() {
    #[cfg(feature = "e2e")]
    {
        use tauri_plugin_sbrs_automation::cdp::CdpStealthConfig;

        // A free loopback port; `.stealth_args(true)` opts in to the stealth flags.
        let config = CdpStealthConfig::default();
        // SAFETY: main has not started threads or initialized the runtime.
        let args = unsafe { config.apply_env() };
        // CEF: pass `args` to your app's CEF initialization before opening windows.
        // WebView2: apply_env sets WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS before startup.
    }
    // Continue with the app's runtime and plugin start-up, as before.
}
```

- **WebView2:** `apply_env` appends the debugging switches to
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, which WebView2 reads when it creates
  its environment. The variable stays in the environment, so a child process
  you start inherits the switches: remove it for a child that must not have a
  debugger (`Command::env_remove`).
- **CEF:** pass `CdpStealthConfig::default().chromium_args()` to Chromium's
  command line. If you set up CEF's remote debugging yourself, take the port from
  `CdpStealthConfig::default().port()`.

The port is a free loopback port, the same for the life of the process. To ask
for one, set `SBRS_CDP_PORT` to a valid non-zero port, or call
`.debugging_port(port)`. A port that is asked for and already in use turns
nothing on, and the app says so on stderr. Nothing is produced unless the
plugin's gate is on: a debug build, or `SBRS_AUTOMATION` set to `1`, `true` or
`on`. Debugging always binds to `127.0.0.1`. Extra arguments added with `.arg(..)`
cannot replace the debugging switches. The stealth flags are opt-in and leave
the app's own browser protections on.

</Step>

<Step title="Start the app and find the port" icon="magnifying-glass">

```bash Terminal
SBRS_AUTOMATION_TOKEN=secret cargo tauri dev --features e2e
curl -s "http://127.0.0.1:4445/secret/cdp"
```

```json Reply
{ "value": { "cdpEnabled": true, "cdpPort": 51234, "cdpUrl": "http://127.0.0.1:51234", "cdpState": "ready" } }
```

`cdpState` is `ready` only while a Chromium debugger answers `GET /json/version`
on that port. It is `unreachable` when a port was chosen but nothing answers
like Chromium yet (the route waits up to one second), and `disabled` when no
port was turned on. A test that starts the app retries until the state is
`ready`. The route has the token and `Origin` guards. The [CDP discovery
reference](/ecosystem/tauri-api/cdp) has every field and status.

</Step>

<Step title="Attach from a test" icon="plug">

The client's discovery asks the plugin for the port, so the test names the
plugin's URL, with a trailing slash, and turns discovery on. With a direct CDP
attachment the whole `BaseCase` API runs through the pure CDP engine:

```rust A test with BaseCase
use seleniumbase_rs_stealth::{
    BaseCase, BrowserConfig, BrowserConfigFingerprintExt, Fingerprint, SeleniumBaseError,
};

async fn drive_the_app() -> Result<(), SeleniumBaseError> {
    let fingerprint = Fingerprint::windows_desktop()
        .to_builder()
        .native_spoofing(true)
        .build()?;
    let config = BrowserConfig::builder()
        .webdriver_url("http://127.0.0.1:4445/secret/")
        .cdp_discovery(true)
        .build()?
        .with_fingerprint(&fingerprint)?;
    let mut sb = BaseCase::new(config).await?;
    let title = sb.title().await?;
    println!("{title}");
    sb.click("#save").await?;
    sb.quit().await // Detaches; the app keeps running.
}
```

The fingerprint is optional. Without discovery, `BrowserConfigBuilder::cdp_url`
takes the debugger's HTTP or browser WebSocket URL directly, and it takes
precedence over discovery.

For the full `Page` and `Browser` API, attach with `Browser::launch`, which
launches no process when the options name a `cdp_url`:

```rust A test with Browser and Page
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::{Fingerprint, LaunchOptionsFingerprintExt, SeleniumBaseError};

async fn drive_the_page() -> Result<(), SeleniumBaseError> {
    let options = LaunchOptions::builder()
        // `cdpUrl` from GET /<token>/cdp, or ws://127.0.0.1:{port}/devtools/browser/<id>.
        .cdp_url("http://127.0.0.1:53412")
        .build()?
        .with_fingerprint(&Fingerprint::windows_desktop())?;
    let browser = Browser::launch(options).await?; // Attaches; launches no process.
    let page = browser.default_page().await?;
    page.locator("#save").click().await?;
    browser.close().await // Detaches; the app keeps running.
}
```

</Step>

<Step title="Pick the window" icon="window-restore">

An app can have several windows. Without a choice, the first one is driven.
Pick another with `BrowserConfigBuilder::cdp_target`, which takes a target id or
part of the window's URL, or with `Browser::select_page` on a `Browser`:

```rust Pick a window
let config = BrowserConfig::builder()
    .webdriver_url("http://127.0.0.1:4445/secret/")
    .cdp_discovery(true)
    .cdp_target("/settings")
    .build()?;
```

</Step>

</Steps>

### What attaching changes in the app

The browser belongs to the app, so attaching does not break or take over its
windows:

- **Only the windows you pick are prepared.** Attaching sends nothing to any
  window. The fingerprint reaches a window when the automation opens or picks
  it. The app's other windows are left as they are.
- **A proxy is refused.** `proxy`, `proxy_pac_url` and a fingerprint's proxy make
  the build fail with `SeleniumBaseError::InvalidConfig`. Configure the proxy in
  the app before it starts.
- **A user agent or locale needs a fingerprint.** On their own they are launch
  flags, which cannot reach a running app, so they are refused.
- **Closing detaches.** `Browser::close()` and `BaseCase::quit()` detach every
  session they opened, and never close the app's windows. Script that already
  ran in a page stays until the page navigates.

The [CDP reference](/ecosystem/tauri-api/cdp#what-attaching-changes-in-the-app)
has the full list. A method that needs a driver (`chromedriver_version`,
`cdp_events`, `install_addon`, Recorder Mode) answers
`SeleniumBaseError::Unsupported` and names the alternative.

<Warning>

**The Chromium debug socket has no authentication.** Any process on the machine
that can reach `127.0.0.1:{port}` can drive every window of the app, read its
pages and run script in them, without the token. The token guards the plugin's
discovery route only. `chromium_args` and `apply_env` produce no debugging
switch unless the plugin's gate is on, and `apply_env` does not export
`SBRS_CDP_PORT`. Leaving `SBRS_CDP_PORT` unset does not turn the port off: with
the gate on, an unset port is a free loopback port. Keep the gate off in a
release build, and keep the plugin behind a Cargo feature that release builds
do not enable.

</Warning>

<Note>

This path is verified against mock HTTP and CDP servers. CEF and WebView2 have
not been tested on a real app in this guide.

</Note>

## Give your shipped webview a fingerprint

The debugger above is for tests. An app that wants its own embedded browser to
carry a fingerprint in a shipped build needs no port at all. The library splits
a fingerprint's stealth into a `StealthPlan` that any channel can carry. The
plugin's `stealth` feature opens no port and no listener, and it uses neither
the automation gate nor the `cdp` feature's launch flags.

<Steps>

<Step title="Turn on the stealth feature" icon="cube">

```toml src-tauri/Cargo.toml
[dependencies]
tauri-plugin-sbrs-automation = { git = "https://github.com/MustCodeAl/tauri-plugin-sbrs-automation", features = ["stealth"] }
```

The feature brings `seleniumbase-rs` and [`seleniumbase-rs-stealth`](/stealth/api/client/migration)
(`default-features = false, features = ["client"]`, pure Rust, zero BoringSSL) over plain git URLs and re-exports `Fingerprint`,
`OsType`, `StealthPlan`, `Browser`, and `Page` from `tauri_plugin_sbrs_automation::stealth`.
Rust 1.100 is the minimum version. If your app also depends on `seleniumbase-rs`
or `seleniumbase-rs-stealth`, use the same plain git URLs so Cargo resolves one
shared copy of every type.

</Step>

<Step title="Add the init script when you build the window" icon="code">

A `StealthPlan` holds the init script, which runs in every document, frames
included, before the page's own scripts. Every runtime takes it:

```rust Build the window
use tauri_plugin_sbrs_automation::stealth::{Fingerprint, OsType, StealthPlan, StealthScriptExt};

let plan = StealthPlan::new(&Fingerprint::randomized(OsType::Windows, 7));
tauri::Builder::default().setup(move |app| {
    tauri::WebviewWindowBuilder::new(app, "browser", tauri::WebviewUrl::App("index.html".into()))
        .stealth_script(&plan)
        .build()?;
    Ok(())
});
```

On WKWebView (macOS) and WebKitGTK (Linux) this is all you get: they speak no
DevTools Protocol, so there are no overrides.

</Step>

<Step title="Send the overrides over the webview's channel (CEF and WebView2)" icon="plug">

The plan's overrides are protocol commands: `Network.enable`, then the user
agent with its Client Hints, locale, time zone, geolocation, screen metrics and
the others the fingerprint asks for. Send them once the webview exists, through
a `stealth::DevToolsChannel`:

```rust Apply the overrides
channel.apply(&plan).await?;
```

`apply` sends the overrides in order, each after the answer to the one before.
They apply from the next request and document on, so open the window at a page
of your own and go to the outside site after `apply` has returned.

The plugin builds without a runtime, so it cannot name the runtimes' DevTools
types. The app joins a bridge:

- **CEF:** `stealth::cef::channel(send, allocate_id)` returns the channel and an
  `Observer`. `send` is CEF's `send_dev_tools_message`; `allocate_id` is the
  runtime's `allocate_devtools_message_id`, because every observer sees every
  result. The app's `on_dev_tools_protocol` callback feeds
  `Observer::method_result` and `Observer::event`.
- **WebView2:** `stealth::webview2::channel` hands each command to the app as a
  `webview2::Call` (`method`, `params_json`, `session_id`, `reply`). The app
  calls `CallDevToolsProtocolMethod` on the UI thread and finishes it with
  `reply.complete`. Subscribe to each name in `webview2::EVENTS` and pass the
  event to `webview2::event`.

The plugin's README has the code for both bridges. The profile manager example
runs the CEF bridge in `src-tauri/src/app_window.rs`, with `tauri-runtime-cef`
and a `tauri` alpha that the example's `Cargo.toml` pins. This guide did not run
it.

</Step>

<Step title="Drive the view (optional)" icon="hand-pointer">

`channel.page().await` is a seleniumbase-rs `Page` for that webview, with
`goto`, locators and human input. Preparing it installs the library's page
helper script in the webview's documents. The stealth alone does not.

</Step>

</Steps>

The channel reaches one page. Such a browser opens no tabs or browser contexts,
and `Browser.*` commands, permission grants among them, are not available.
`close` ends the channel only: what was set in the page stays, since the page is
the app's. Commands that must run in order are handed over one answer at a time,
because WebView2 may run what it was given out of order.

<Note>

The bridges are tested against fake channels. The plugin's README says they have
not been compiled or run in a real CEF or WebView2 app. This guide did not run
them either.

</Note>

## Troubleshooting

| What you see | Why | What to do |
|---|---|---|
| `cdpState` is `disabled` | The gate is off, the `cdp` feature is off, the requested port is in use, or `apply_env` was not called | Use a debug build or set `SBRS_AUTOMATION=1`, enable the `cdp` feature, and call `apply_env` (or pass `chromium_args()`) before the runtime starts. Read stderr for a port in use. |
| `cdpState` is `unreachable` | A port was chosen, but nothing answers like Chromium there yet, or another program holds it | Retry until `ready`. Check that the runtime got the arguments. |
| `apply_env` changes nothing (WebView2) | The gate is off, or `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` already holds a `--remote-debugging-*`, `--remote-allow-origins` or `--edge-webview-switches` switch, or text that is not Unicode | Remove the switch from the variable. |
| The attach fails with `InvalidConfig` | A proxy, or a user agent or locale without a fingerprint, was given | Configure the proxy in the app. Give the user agent and locale through a fingerprint. |
| A `BaseCase` method answers `Unsupported` | It needs a driver | Use the alternative that the error names, or `Browser` and `Page`. |
| The app uses WKWebView or WebKitGTK | These webviews speak no CDP | Drive the window with [the driver guide](/ecosystem/integrations/tauri/driver). |
| The page reads the fingerprint's values, but the overrides (user agent and Client Hints, locale, time zone, geolocation, screen metrics) are missing | WKWebView and WebKitGTK speak no DevTools Protocol, so they take the init script only | Use a Chromium runtime (CEF or WebView2) for the overrides. |

## Next

<Columns cols={3}>

<Card color="#199f43" title="CDP reference" icon="code" href="/ecosystem/tauri-api/cdp">

`GET /cdp`, `CdpStealthConfig` and what attaching changes.

</Card>

<Card color="#1a85d4" title="Test a Tauri app" icon="vial" href="/ecosystem/integrations/tauri/driver">

Drive the app's window with WebDriver.

</Card>

<Card color="#d47628" title="Automate a browser from your app" icon="window-maximize" href="/ecosystem/integrations/tauri/client">

Start a separate browser from the app's Rust code.

</Card>

</Columns>
