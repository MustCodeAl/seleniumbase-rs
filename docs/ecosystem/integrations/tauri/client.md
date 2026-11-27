---
title: "Automate a browser from your Tauri app (client)"
sidebarTitle: "Automate a browser (client)"
description: "Use seleniumbase-rs inside a Tauri app: a Tauri command starts a browser with the pure CDP engine, drives a page, and returns the result to the window's JavaScript."
keywords: ["tauri", "command", "invoke", "sb_cdp", "browser", "automation", "desktop app", "profile manager"]
---

<Badge color="orange" shape="pill">Early</Badge> <Badge color="purple" shape="pill">Tauri 3 alpha</Badge> <Badge color="green" icon="apple" shape="pill">Profile manager: tested on macOS</Badge>

At the end of this guide your Tauri app has a command that starts a browser
with seleniumbase-rs, loads a page, and returns the page title to the window's
JavaScript. Your app is the client: it uses seleniumbase-rs inside its own Rust
code, as a library. The browser it drives is a separate Chrome, not the app's
own webview.

This is one of three Tauri guides. To test your own app from outside, read
[Test a Tauri app](/ecosystem/integrations/tauri/driver). To drive the app's own webview, or
give it a fingerprint, read [Drive your app's own webview](/ecosystem/integrations/tauri/browser).

<Visibility for="agents">

Summary for AI exports: add `seleniumbase-rs` to the app's `src-tauri/Cargo.toml` (Rust 1.100 or newer), write an `async` `#[tauri::command]` that takes owned arguments and returns `Result<String, String>`, build `LaunchOptions::builder().headless(true).build()`, call `Browser::launch(options).await`, drive `browser.default_page().await` with `goto`, `title` and `locator`, and call `browser.close().await`. Register the command with `.invoke_handler(tauri::generate_handler![...])`. The window calls it with `window.__TAURI__.core.invoke`, which needs `app.withGlobalTauri` in `tauri.conf.json`. Chrome or Chromium must be installed on the machine that runs the app. The plugin `tauri-plugin-sbrs-automation` is not needed for this.

</Visibility>

## Before you start

- A Tauri app that builds and runs. On Tauri 3 the app names its runtime, with
  `.runtime(tauri_runtime_wry::Wry::default())` on the builder; without it, the
  app stops at start with `RuntimeNotConfigured`.
- Rust 1.100 or newer, the minimum version of `seleniumbase-rs`.
- Google Chrome or Chromium on the machine that runs the app. The pure CDP
  engine launches it. It needs no driver and no container.
- The plugin `tauri-plugin-sbrs-automation` is not needed here. It is for the
  other two guides.

## Choose an engine

| Engine | What the app needs | Calls |
|---|---|---|
| Pure CDP | Chrome or Chromium installed; each launch starts its own Chrome | `seleniumbase_rs::sb_cdp::Browser::launch` |
| WebDriver | A WebDriver server at a URL, such as a Selenium container | `BaseCase` with `BrowserConfigBuilder::webdriver_url` |

The steps below use pure CDP. [Use BaseCase instead](#use-basecase-instead)
shows the WebDriver form.

## Add a browser command

<Steps>

<Step title="Add the library" icon="cube">

```toml src-tauri/Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
```

Tauri runs `async` commands on its Tokio runtime, so the app needs no runtime
of its own for the library.

<Note>

If the app also uses the plugin's `stealth` feature, name the same git tag as the
plugin's `Cargo.toml` does, so the app and the plugin share one copy of the
library and its types (`Fingerprint`, `StealthPlan`). The profile manager
example does this.

</Note>

</Step>

<Step title="Write the command" icon="code">

An `async` command cannot take borrowed arguments such as `&str`, so it takes
an owned `String`. It returns `Result<String, String>`: an `Err` rejects the
promise in JavaScript.

```rust src-tauri/src/lib.rs
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::SeleniumBaseError;

/// Opens `url` in a Chrome the app starts, and returns the page title.
#[tauri::command]
async fn page_title(url: String) -> Result<String, String> {
    // The window decides the URL. Open web pages only.
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only http and https pages open".to_owned());
    }
    let options = LaunchOptions::builder()
        .headless(true)
        .build()
        .map_err(|error| error.to_string())?;
    let browser = Browser::launch(options)
        .await
        .map_err(|error| error.to_string())?;
    let title = read_title(&browser, &url).await;
    // Close the browser whatever the page did.
    browser.close().await.map_err(|error| error.to_string())?;
    title.map_err(|error| error.to_string())
}

async fn read_title(browser: &Browser, url: &str) -> Result<String, SeleniumBaseError> {
    let page = browser.default_page().await?;
    page.goto(url).await?;
    page.title().await
}
```

`Browser::launch` starts Chrome. With `headless(true)` it has no window.
Unset, the browser is headless on Linux and headed elsewhere.
`browser.default_page()` is the first tab. `page.goto(url)` waits for the page
to load.

</Step>

<Step title="Register the command" icon="plug">

```rust The app's builder
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![page_title])
    // ... the rest of your builder, as before
```

</Step>

<Step title="Call it from the window" icon="window-maximize">

The window reaches the command through Tauri's `invoke`. The global
`window.__TAURI__` exists only when `app.withGlobalTauri` is `true` in
`tauri.conf.json`. Without it, import `invoke` from `@tauri-apps/api/core`
instead.

```json src-tauri/tauri.conf.json
{
  "app": {
    "withGlobalTauri": true
  }
}
```

```js src/main.js
const { invoke } = window.__TAURI__.core;

document.querySelector('#go').addEventListener('click', async () => {
  const url = document.querySelector('#url').value;
  try {
    const title = await invoke('page_title', { url });
    document.querySelector('#out').textContent = title;
  } catch (message) {
    document.querySelector('#out').textContent = `Failed: ${message}`;
  }
});
```

A command registered with `invoke_handler` is allowed for every window of the
app by default, so it needs no entry in a capability file. The profile
manager's `capabilities/default.json` lists only `core:default` and
`opener:default`, and its window still calls the app's own commands, such as
`get_api_token`. To restrict commands to some windows, see
`AppManifest::commands` in Tauri's capabilities documentation.

</Step>

<Step title="Run the app" icon="play">

```bash Terminal
cargo tauri dev
```

<Check>

Type `https://example.com` in the window and press the button. The window shows
the page's title, and no Chrome process is left running.

</Check>

</Step>

</Steps>

## Drive the page

`Browser::default_page()` returns a `Page`. Find elements with
`page.locator(selector)`, which takes CSS, XPath (`//div`) or link text
(`link=Home`), and looks again on every use. A few calls:

```rust Page and Locator
let page = browser.default_page().await?;
page.goto("https://example.com").await?;
let heading = page.locator("h1").text().await?;
page.locator("a").click().await?;
```

Each call returns `Result<_, SeleniumBaseError>`. The
[pure CDP tutorial](/guides/browser-engines/pure-cdp) covers the rest of the `Page` and
`Locator` API.

## Use BaseCase instead

`BaseCase` is the Selenium-style API. In a Tauri command it connects to a
WebDriver server, for example a `selenium/standalone-chrome` container, as the
profile manager's WebDriver profiles do:

```rust A command with BaseCase
use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

#[tauri::command]
async fn page_title_webdriver(url: String) -> Result<String, String> {
    let config = BrowserConfig::builder()
        .webdriver_url("http://localhost:4444")
        .headless(true)
        .build()?;
    let mut sb = BaseCase::new(config)
        .await
        .map_err(|error| error.to_string())?;
    let title = read_title_with(&mut sb, &url).await;
    // End the session whatever the page did.
    sb.quit().await.map_err(|error| error.to_string())?;
    title.map_err(|error| error.to_string())
}

async fn read_title_with(sb: &mut BaseCase, url: &str) -> Result<String, SeleniumBaseError> {
    sb.open(url).await?;
    sb.title().await
}
```

The command checks the URL scheme first, as `page_title` does; that check is
left out here to keep the example short.

## A complete app

The [Tauri profile manager](https://github.com/MustCodeAl/tauri-plugin-sbrs-automation/tree/main/examples/tauri-profile-manager)
is an example app built this way. Each profile is its own browser identity,
driven through a WebDriver container, or through a Chrome the app launches
itself over pure CDP. A pure CDP launch applies, in order, the profile's proxy,
fingerprint, locale, geolocation and screen size, its saved cookies, and the
start URL, and it fails with the step named if one fails. The same app opens a
profile in a window of its own: that is the
[stealth feature](/ecosystem/integrations/tauri/browser#give-your-shipped-webview-a-fingerprint)
of the plugin.

## Troubleshooting

| What you see | Why | What to do |
|---|---|---|
| `window.__TAURI__` is undefined | `app.withGlobalTauri` is not `true` | Set it in `tauri.conf.json`, or import `invoke` from `@tauri-apps/api/core`. |
| The command fails with a `BrowserLaunch` error | No Chrome or Chromium was found, or it could not start | Install one, or name its binary with `LaunchOptions::builder().executable_path(path)`. |
| The build fails on a borrowed argument in an `async` command | An `async` command cannot take `&str` | Take an owned `String`. |
| The build fails on the Rust version | `seleniumbase-rs` needs Rust 1.100 | Update the toolchain. |
| A Chrome window opens | The browser is headed outside Linux unless `headless(true)` is set | Call `headless(true)`. |
| The app stops at start with `RuntimeNotConfigured` | Tauri 3 starts no runtime unless the app names one | Call `.runtime(tauri_runtime_wry::Wry::default())` on the builder. |

## Security

<Warning>

Tauri allows every command registered with `invoke_handler` to all the windows
and webviews of the app by default, and the caller picks the arguments. Check
what a command receives, as `page_title` checks the URL scheme, and keep
commands that start a browser out of reach of windows that show outside sites.
The profile manager's README says that no capability names its profile windows,
so the sites in them cannot call the app's commands.

</Warning>

## Next

<Columns cols={3}>

<Card color="#1a85d4" title="Test a Tauri app" icon="vial" href="/ecosystem/integrations/tauri/driver">

Drive the app's own window from a test.

</Card>

<Card color="#a631be" title="Drive your app's own webview" icon="globe" href="/ecosystem/integrations/tauri/browser">

Attach over CDP, or give the webview a fingerprint.

</Card>

<Card color="#199f43" title="Pure CDP engine" icon="code" href="/guides/browser-engines/pure-cdp">

The `Browser`, `Page` and `Locator` API.

</Card>

</Columns>
