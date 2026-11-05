---
title: "Playwright bridge API"
sidebarTitle: "Playwright bridge"
description: "PlaywrightSession and StealthyPlaywright of the seleniumbase-rs-playwright crate: attach rustwright to a stealthy browser, with every method, its errors and examples."
keywords: ["playwright", "rustwright", "PlaywrightSession", "StealthyPlaywright", "connect_over_cdp", "attach", "with_page", "stealth", "sb_cdp"]
---

<Note>

This page is the reference for the crate `seleniumbase-rs-playwright`. The guide, with the three formats that map from Python and the conversion of a regular Playwright script, is [Playwright mode](/help/playwright_mode). The browser the bridge attaches to is the [pure CDP engine](/tutorials/pure_cdp) (`sb_cdp`), which is what Python's CDP Mode (`sb.cdp`) maps to.

</Note>

## The crate

Stealthy Playwright Mode, as in Python SeleniumBase: [`rustwright`](https://github.com/Skyvern-AI/rustwright), a Rust port of Playwright with a native CDP engine, attaches to a stealthy `sb_cdp` browser through its DevTools endpoint (`connect_over_cdp`). The browser keeps its fingerprint, proxy and launch switches, and the CDP API and the Playwright API then work on the same tabs.

The bridge is a separate crate, not a feature of the library. `seleniumbase-rs` has no `playwright` feature and no `rustwright` dependency; a project that does not list the bridge never compiles `rustwright`. It is a bridge, not a `Plugin`. Add it next to the library:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.8.0" }
seleniumbase-rs-playwright = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.8.0" }
```

The crate root has three public items:

| Item | What it is |
|---|---|
| `PlaywrightSession` | A rustwright client attached to a running Chromium, with one active page. |
| `StealthyPlaywright` | A stealthy `sb_cdp` browser and a `PlaywrightSession` attached to it, launched in one call. |
| `rustwright` | The whole `rustwright` crate, re-exported (`pub use rustwright;`), so a project lists one dependency. |

All examples on this page import only from `seleniumbase_rs` and `seleniumbase_rs_playwright`.

## PlaywrightSession

```rust
pub struct PlaywrightSession { /* private fields */ }
```

A `rustwright` client attached to a running Chromium, with one active page. It never starts a browser. The attached page keeps whatever fingerprint, proxy and launch switches the browser was started with; the session changes none of them.

rustwright's page calls block the thread they run on, so every `async` method of the session runs its call on Tokio's blocking pool and the runtime is never stalled. `close` only detaches: a browser that another handle owns is never shut down from the session.

`PlaywrightSession` implements `Debug`; it prints the `connected` flag and nothing else, because rustwright's handles are not `Debug`.

### Attaching

```rust
pub async fn attach(browser: &sb_cdp::Browser) -> Result<Self, SeleniumBaseError>
pub async fn connect_over_cdp(endpoint: &str) -> Result<Self, SeleniumBaseError>
pub async fn connect_with(options: ConnectOptions) -> Result<Self, SeleniumBaseError>
```

- `attach(&browser)` attaches to a browser launched through `sb_cdp`, using the HTTP address of its DevTools endpoint (`Browser::http_url()`). It calls `connect_over_cdp` with that address.
- `connect_over_cdp(endpoint)` attaches to any Chromium that is already running: an `http://host:port` address or a `ws://` URL. A chromedriver session reports its `debuggerAddress` as a bare `host:port`; put `http://` in front of it.
- `connect_with(options)` attaches as the rustwright `ConnectOptions` say, for a timeout or request headers that `connect_over_cdp` does not set. `connect_over_cdp(endpoint)` is `connect_with(ConnectOptions::new(endpoint))`.

All three work on the first page the browser has open, or on a new one if it has none. Open the page you want with the CDP API first (the order Python SeleniumBase asks for with an authenticated proxy) and the session starts on that page. Each fails with `SeleniumBaseError::Playwright` if nothing answers at the endpoint, the browser cannot be controlled, or the attach times out.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::rustwright::ConnectOptions;
use seleniumbase_rs_playwright::PlaywrightSession;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;

    // The same endpoint, attached three ways, one after the other.
    let session = PlaywrightSession::attach(&browser).await?;
    println!("attach: {}", session.is_connected());
    session.close().await?; // detaches; the browser keeps running

    let session = PlaywrightSession::connect_over_cdp(&browser.http_url()).await?;
    println!("connect_over_cdp: {}", session.is_connected());
    session.close().await?;

    let options = ConnectOptions::new(&browser.http_url());
    let session = PlaywrightSession::connect_with(options).await?;
    println!("connect_with: {}", session.is_connected());
    session.close().await?;

    browser.close().await?;
    Ok(())
}
```

### Handles

```rust
pub fn browser(&self) -> &Browser
pub fn page(&self) -> &Page
pub fn is_connected(&self) -> bool
```

- `browser()` is the `rustwright::Browser` the session is attached to.
- `page()` is the active `rustwright::Page`. It is a cheap handle: clone it to move it elsewhere. Its calls block, so use it only on a thread that may block; from async code use `with_page`.
- `is_connected()` is true while the connection to the browser is open.

### with_page

```rust
pub async fn with_page<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>
where
    F: FnOnce(Page) -> Result<T, RwError> + Send + 'static,
    T: Send + 'static,
```

Runs `work` with the active page on Tokio's blocking pool and returns its result. This is the way to use any rustwright page call from async code, so everything rustwright can do is one call away. `RwError` is `rustwright::Error`. If `work` fails, the result is `SeleniumBaseError::Playwright` with the text of the rustwright error.

```rust,no_run
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::StealthyPlaywright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stealthy =
        StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?).await?;

    let title = stealthy
        .playwright()
        .with_page(|page| {
            page.goto("https://example.com", Default::default())?;
            page.title(Default::default())
        })
        .await?;
    println!("{title}");

    stealthy.close().await?;
    Ok(())
}
```

### Page actions

```rust
pub async fn new_page(&mut self) -> Result<(), SeleniumBaseError>
pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError>
pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError>
pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError>
pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError>
pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError>
pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError>
```

| Method | What it does | Errors |
|---|---|---|
| `new_page()` | Opens a new page in the browser and makes it the active one. It takes `&mut self`; on a `StealthyPlaywright` call it through `playwright_mut()`. | `Playwright` if the page cannot be created. |
| `goto(url)` | Navigates the active page to `url`. | `Navigation` if the page cannot be loaded. |
| `click(selector)` | Clicks the element selected by `selector`. | The element errors below. |
| `type_text(selector, text)` | Clears the element and types `text` into it (rustwright `fill`). | The element errors below. |
| `get_text(selector)` | Returns the text content of the element; an element with no text content gives an empty `String`. | The element errors below. |
| `evaluate(expression)` | Evaluates `expression` in the active page and returns the JSON result as a `seleniumbase_rs::json::Value`. | `Playwright` if the script throws. |
| `screenshot(path)` | Saves a screenshot of the active page to `path`. | `Screenshot` if the page cannot be captured or the file cannot be written. |

`click`, `type_text` and `get_text` map rustwright's failure to the errors the other engines return:

| rustwright reports | `SeleniumBaseError` |
|---|---|
| The wait ended with no element matched | `ElementNotFound` |
| The element is detached from the page, whether it never was in it or went away while rustwright waited | `ElementNotFound` |
| Any other actionability failure, for example an element that cannot be clicked | `ElementNotInteractable`, with rustwright's reason |
| An action timeout or a plain timeout | `WaitTimeout` |
| The connection closed or dropped, the target closed, or the page crashed | `BrowserDisconnected` |
| An action failure | The same table applied to its cause |
| Anything else | `Playwright` |

```rust,no_run
use std::path::Path;

use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::tokio;
use seleniumbase_rs::SeleniumBaseError;
use seleniumbase_rs_playwright::StealthyPlaywright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stealthy =
        StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?).await?;

    let session = stealthy.playwright_mut();
    session.new_page().await?; // a new tab, now the active page
    session.goto("https://example.com/login").await?;
    session.type_text("#user", "alice").await?;
    session.click("#submit").await?;

    match session.get_text("#welcome").await {
        Ok(text) => println!("{text}"),
        Err(SeleniumBaseError::ElementNotFound { .. }) => println!("the page shows no welcome"),
        Err(SeleniumBaseError::WaitTimeout { .. }) => println!("the page took too long"),
        Err(other) => return Err(other.into()),
    }

    let title = session.evaluate("document.title").await?;
    println!("{title}");
    session.screenshot(Path::new("login.png")).await?;

    stealthy.close().await?;
    Ok(())
}
```

### close

```rust
pub async fn close(&self) -> Result<(), SeleniumBaseError>
```

Closes this client's connection. The browser it attached to keeps running; whoever started it shuts it down (for a browser from `sb_cdp`, its own `close`). It fails with `SeleniumBaseError::Playwright` if the connection does not close cleanly.

## StealthyPlaywright

```rust
#[derive(Debug)]
pub struct StealthyPlaywright { /* private fields */ }
```

A stealthy pure CDP browser and a `PlaywrightSession` attached to it: Python SeleniumBase's Stealthy Playwright Mode in one value. The browser is launched by `sb_cdp` with all of its launch switches, fingerprint and proxy handling, and rustwright connects to it over the DevTools Protocol. The CDP API (`cdp()`) and the Playwright API (`playwright()`) then work on the same tabs at the same time.

| Method | Signature | What it does |
|---|---|---|
| `launch` | `pub async fn launch(options: LaunchOptions) -> Result<Self, SeleniumBaseError>` | Launches a stealthy browser as `options` say and attaches Playwright to it. |
| `cdp` | `pub fn cdp(&self) -> &Browser` | The pure CDP browser (`seleniumbase_rs::sb_cdp::Browser`): pages, locators, interception, input. |
| `playwright` | `pub fn playwright(&self) -> &PlaywrightSession` | The Playwright session on the same browser. |
| `playwright_mut` | `pub fn playwright_mut(&mut self) -> &mut PlaywrightSession` | The session, mutably, for `new_page`. |
| `endpoint` | `pub fn endpoint(&self) -> DevToolsEndpoint` | The DevTools endpoint of the browser, for other clients that attach to it: Playwright MCP, `chrome-devtools-mcp`, a Playwright in another language. |
| `close` | `pub async fn close(self) -> Result<(), SeleniumBaseError>` | Detaches Playwright, then closes the browser. |

`launch` returns the error of `Browser::launch` if the browser does not start, or `SeleniumBaseError::Playwright` if Playwright cannot attach. In the second case the browser that was started is closed again, so nothing is left running that nobody holds. To open a page with the CDP API before Playwright attaches, launch the browser yourself with `Browser::launch`, open the page and call `PlaywrightSession::attach`.

`close` takes `self`. It returns the first error of detaching and of closing the browser, and the browser is closed even when detaching fails.

`endpoint()` returns a `seleniumbase_rs::sb_cdp::DevToolsEndpoint`: `http_url()` (what `chrome-devtools-mcp --browserUrl` and Playwright's `--cdp-endpoint` take), `websocket_url()`, `version_url()`, and the ready-made arguments `chrome_devtools_mcp_args()` and `playwright_mcp_args()`.

```rust,no_run
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::StealthyPlaywright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stealthy =
        StealthyPlaywright::launch(LaunchOptions::builder().headless(true).build()?).await?;

    // Playwright loads the page ...
    stealthy.playwright().goto("https://example.com").await?;
    println!("{}", stealthy.playwright().get_text("h1").await?);

    // ... and the CDP API reads it back from the same tab.
    let title = stealthy.cdp().default_page().await?.title().await?;
    println!("{title}");

    // Other clients can attach to the same browser.
    let endpoint = stealthy.endpoint();
    println!("DevTools at {}", endpoint.http_url());

    stealthy.close().await?; // detaches Playwright, then closes the browser
    Ok(())
}
```

## The rustwright re-export

`seleniumbase_rs_playwright::rustwright` is the `rustwright` crate itself, so a project lists no second dependency. The types the bridge hands out are its types: `rustwright::Browser` and `rustwright::Page` from `browser()`, `page()` and `with_page`, `rustwright::ConnectOptions` for `connect_with`, and `rustwright::Error` in the signature of `with_page`. rustwright's Rust API has a browser and pages but no browser-context type, so the session starts on the first open page instead of Python's `browser.contexts[0].pages[0]`.

## Errors

Every fallible method returns `Result<T, SeleniumBaseError>`, the same error type as the rest of the library. The variants the bridge returns:

| Variant | Returned by |
|---|---|
| `Playwright` | The attach methods, `with_page`, `new_page`, `evaluate`, `close`, a rustwright failure the table above does not name, and a blocking task that panicked or was cancelled |
| `Navigation` | `goto` |
| `ElementNotFound`, `ElementNotInteractable`, `WaitTimeout`, `BrowserDisconnected` | `click`, `type_text`, `get_text` |
| `Screenshot` | `screenshot` |

`StealthyPlaywright::launch` also returns whatever `Browser::launch` returns when the browser does not start. See [the errors of the crate root](/api/crate-root#errors) for the helpers every `SeleniumBaseError` has, such as `is_transient()` and `category()`.

## See also

- [Playwright mode](/help/playwright_mode): the guide, the Python-to-Rust mapping and the conversion of a regular Playwright script.
- [Pure CDP engine](/tutorials/pure_cdp): the browser the bridge attaches to (`Browser`, `Page`, `LaunchOptions`).
- [Crate root](/api/crate-root): `SeleniumBaseError` and the other items of `seleniumbase_rs`.
