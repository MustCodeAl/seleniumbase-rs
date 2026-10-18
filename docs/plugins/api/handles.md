---
title: "Browser and Page Handles"
sidebarTitle: "Handles"
description: "Reference for BrowserHandle and PageHandle, the two traits through which a plugin reaches the browser and its tabs, and what the pure CDP engine, the WebDriver engine and the test fakes do behind each method."
---

A plugin never sees `sb_cdp::Browser`, `Page` or `BaseCase`. The browser and
each tab reach it as two small traits, `BrowserHandle` and `PageHandle`, which
both engines of seleniumbase-rs implement, so one plugin works with either.
Both are in `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/lib.rs`, and both are `async` through
[`async_trait`](/plugins/api/plugin-trait#async_trait).

<RequestExample>

```rust A DevTools command per tab
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, Metadata, PageHandle, Plugin, Requirements, Result,
};
use serde_json::json;

pub struct NoImages;

#[async_trait]
impl Plugin for NoImages {
    fn name(&self) -> &str {
        "no-images"
    }

    fn requirements(&self) -> Requirements {
        Requirements::CHROMIUM_ONLY
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Cdp("Network".into()))
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.send("Network.enable", json!({})).await?;
        // The host merges this list with the tab's other blocklists.
        page.send("Network.setBlockedURLs", json!({ "urls": ["*.png", "*.jpg"] }))
            .await?;
        Ok(())
    }
}
```

```rust Scripts and values
use seleniumbase_rs_plugin::{PageHandle, Result};

// In `impl Plugin`, with page.script:* and page.evaluate declared:
async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
    page.add_init_script("window.__testRun = 'nightly';").await?;
    let agent = page.evaluate("navigator.userAgent").await?;
    if agent.as_str().is_some_and(|agent| agent.contains("HeadlessChrome")) {
        page.add_init_script("document.documentElement.dataset.headless = '1';")
            .await?;
    }
    Ok(())
}
```

```rust A browser-level command
use seleniumbase_rs_plugin::{BrowserFamily, BrowserHandle, Result};
use serde_json::json;

// In `impl Plugin`, with cdp:SystemInfo declared:
async fn on_browser(&self, browser: &dyn BrowserHandle) -> Result<()> {
    if browser.family() == BrowserFamily::Chromium {
        let info = browser.send("SystemInfo.getInfo", json!({})).await?;
        let _gpu = &info["gpu"];
    }
    Ok(())
}
```

```rust A tab of your own program
use seleniumbase_rs::plugin_api::PageHandle;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
let page = browser.new_page().await?;
// sb_cdp::Page is a PageHandle; name the trait, since Page has methods of the same names.
let agent = PageHandle::evaluate(&page, "navigator.userAgent").await?;
```

</RequestExample>

<ResponseExample>

```text Fetch is refused
Fetch.enable is not available to plugins: the host owns request interception (one Fetch.enable per tab, shared with the library's proxy login), so a plugin cannot enable, change or answer it. To stop requests, send Network.setBlockedURLs
```

```text No DevTools channel
Firefox has no Chrome DevTools Protocol channel through WebDriver, so a plugin cannot send it protocol commands
```

```text A script that throws
the script threw: ReferenceError: missing is not defined
```

</ResponseExample>

## Who implements them

| Implementation | Where | Public |
|---|---|---|
| The pure CDP engine's browser and tabs | `src/sb_cdp/plugin_hooks.rs` (`BrowserLink`, `TabLink`) | no, the hooks receive them |
| `seleniumbase_rs::sb_cdp::Page` | `src/sb_cdp/plugin_hooks.rs` | yes: `impl PageHandle for Page`, feature `plugins` |
| The WebDriver engine's browser and window | `src/browser/plugin_hooks.rs` (`DriverBrowser`, `DriverPage`) | no, the hooks receive them |
| `RecordingBrowser`, `RecordingPage` | `seleniumbase-rs-plugin-testing` | yes, see [Testing](/plugins/api/testing) |

Every engine's handle errors are `PluginError::Other`; the set wraps them as
`PluginError::Hook` with the plugin and the hook.

## BrowserHandle

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::BrowserHandle`

```rust
#[async_trait]
pub trait BrowserHandle: Send + Sync {
    fn engine(&self) -> Engine;
    fn family(&self) -> BrowserFamily;
    async fn send(&self, method: &str, params: Value) -> Result<Value>;
}
```

The running browser, as a plugin sees it in
[`on_browser`](/plugins/api/plugin-trait).

## BrowserHandle::engine

<Badge color="green">fn</Badge>

```rust
fn engine(&self) -> Engine;
```

<ResponseField name="return" type="Engine">

`Engine::Cdp` on the pure CDP engine, `Engine::WebDriver` on the WebDriver
engine.

</ResponseField>

## BrowserHandle::family

<Badge color="green">fn</Badge>

```rust
fn family(&self) -> BrowserFamily;
```

<ResponseField name="return" type="BrowserFamily">

The browser's family. Always `Chromium` on the pure CDP engine; on the
WebDriver engine, the family of the configured browser.

</ResponseField>

## BrowserHandle::send

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
async fn send(&self, method: &str, params: Value) -> Result<Value>;
```

<ParamField body="method" type="&str" required>

A browser-level Chrome DevTools Protocol method, such as `SystemInfo.getInfo`.

</ParamField>

<ParamField body="params" type="serde_json::Value" required>

Its parameters, usually `json!(...)`; `json!({})` for none.

</ParamField>

<ResponseField name="return" type="Result<Value>">

The command's result, as JSON.

</ResponseField>

Sends a browser-level command and returns its result. Browsers without the
protocol return an error.

| Engine | What happens |
|---|---|
| Pure CDP | Sent on the browser's own connection, with no tab session |
| WebDriver, Chromium | Sent through chromedriver's DevTools channel (`goog/cdp/execute`) |
| WebDriver, Firefox or Safari | Refused: `NAME has no Chrome DevTools Protocol channel through WebDriver, ...` |

On every engine the host refuses `Fetch.*` and
`Network.setRequestInterception` first; see
[the request guard](/plugins/api/host#request-guard).

## PageHandle

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::PageHandle`

```rust
#[async_trait]
pub trait PageHandle: Send + Sync {
    fn engine(&self) -> Engine;
    async fn send(&self, method: &str, params: Value) -> Result<Value>;
    async fn add_init_script(&self, source: &str) -> Result<()>;
    async fn evaluate(&self, expression: &str) -> Result<Value>;
}
```

A tab, as a plugin sees it in [`on_page_created`](/plugins/api/plugin-trait),
and as the set adds a plugin's [packages](/plugins/api/requirements#package)
to it. On the WebDriver engine it is the window the session is on.

## PageHandle::engine

<Badge color="green">fn</Badge>

```rust
fn engine(&self) -> Engine;
```

<ResponseField name="return" type="Engine">

The engine that runs the tab. A page has no `family()`: read the family in
`on_browser`, or from `LaunchContext`, if the plugin needs it.

</ResponseField>

## PageHandle::send

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
async fn send(&self, method: &str, params: Value) -> Result<Value>;
```

<ParamField body="method" type="&str" required>

A Chrome DevTools Protocol method for this tab, such as `Network.enable`.

</ParamField>

<ParamField body="params" type="serde_json::Value" required>

Its parameters.

</ParamField>

<ResponseField name="return" type="Result<Value>">

The command's result, as JSON.

</ResponseField>

Sends a command to this tab and returns its result. Browsers without the
protocol return an error.

| Engine | What happens |
|---|---|
| Pure CDP | Sent on the tab's own protocol session |
| WebDriver, Chromium | Sent through `goog/cdp/execute`, which reaches the window the session is on |
| WebDriver, Firefox or Safari | Refused, as for `BrowserHandle::send` |

Before sending, every engine applies two rules:

- `Fetch.*` and `Network.setRequestInterception` are refused, because the host
  owns request interception.
- `Network.setBlockedURLs` is merged: its `urls` are added to the tab's
  blocklist (the library's ad-block list, the tracker hosts of a fingerprint
  and every pattern a plugin sent), and the whole union is sent, each pattern
  once, in the order it first arrived. `urls` must be a list of strings; a
  command with `urlPatterns` is refused, because it cannot be merged. A plugin
  cannot take a pattern back.

## PageHandle::add_init_script

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
async fn add_init_script(&self, source: &str) -> Result<()>;
```

<ParamField body="source" type="&str" required>

JavaScript to run in every document this tab loads from now on.

</ParamField>

<ResponseField name="return" type="Result<()>">

An error when the script cannot be added, or when it throws in the current
document (`the script threw: ...`).

</ResponseField>

Runs `source` in every document this tab loads from now on, before the page's
own scripts.

| Engine | What happens |
|---|---|
| Pure CDP, and WebDriver with Chromium | `Page.addScriptToEvaluateOnNewDocument`, then `Runtime.evaluate` of the same source in the document already loaded |
| WebDriver, Firefox or Safari | A WebDriver script now, and again after each navigation of the session, so it runs after the page's own scripts. It runs as a function body: its top-level declarations stay local, so assign to `window` to share a value |

## PageHandle::evaluate

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
async fn evaluate(&self, expression: &str) -> Result<Value>;
```

<ParamField body="expression" type="&str" required>

A JavaScript expression.

</ParamField>

<ResponseField name="return" type="Result<Value>">

Its value, as JSON. An exception is an error, `the script threw: ...`.

</ResponseField>

Evaluates `expression` in the current document and returns its value.

| Engine | What happens |
|---|---|
| Pure CDP | `Runtime.evaluate` with `returnByValue` and `awaitPromise`, so a promise is waited for |
| WebDriver, every browser | A WebDriver script that returns the expression (a trailing `;` is dropped), in the frame the session is on; a promise is waited for |

## sb_cdp::Page as a PageHandle

<Badge color="blue">impl</Badge> <Badge color="blue">Feature: plugins</Badge> `impl PageHandle for seleniumbase_rs::sb_cdp::Page`

```rust
#[async_trait]
impl PageHandle for Page {
    fn engine(&self) -> Engine; // Engine::Cdp
    async fn send(&self, method: &str, params: Value) -> PluginResult<Value>;
    async fn add_init_script(&self, source: &str) -> PluginResult<()>;
    async fn evaluate(&self, expression: &str) -> PluginResult<Value>;
}
```

A tab of the pure CDP engine is a `PageHandle` too, so a program can hand one
of its own tabs to code written against the plugin API. It behaves as the tabs
the hooks receive, guard included. `Page` also has inherent methods named
`evaluate` and `add_init_script`, which differ (the inherent
`add_init_script` returns an identifier and does not run the script in the
current document), so call the trait's with `PageHandle::evaluate(&page, ...)`.
