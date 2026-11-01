---
title: "Playwright Mode"
sidebarTitle: "Playwright mode"
---

<Badge color="blue">Crate: seleniumbase-rs-playwright</Badge>

Every method, error and signature of the crate is in the [Playwright bridge API reference](/api/playwright).

**Stealthy Playwright Mode** is a subset of the [pure CDP engine](/tutorials/pure_cdp) where
[`rustwright`](https://github.com/Skyvern-AI/rustwright), a Rust port of Playwright with a native
CDP engine, attaches to a stealthy browser session through the remote-debugging URL. Playwright
then bypasses bot detection the way the browser does, and the APIs of both libraries work on the
same tabs. Under the hood Playwright achieves stealth through `connect_over_cdp`, as in Python
SeleniumBase.

<Info>

**What you will learn**

- How to add the `seleniumbase-rs-playwright` crate, which is not a feature of the library.
- How to attach Playwright to a stealthy browser, or launch both in one call.
- How the three Python formats map to Rust.
- How to convert a regular Playwright script.

</Info>

<Visibility for="agents">

Summary for AI exports: the library `seleniumbase-rs` has no Playwright engine and no `playwright` feature. Stealthy Playwright Mode is the separate crate `seleniumbase-rs-playwright` (`crates/playwright` in the repository). `PlaywrightSession::attach(&browser)` attaches rustwright to a `seleniumbase_rs::sb_cdp::Browser` through its DevTools endpoint; `PlaywrightSession::connect_over_cdp(endpoint)` attaches to any DevTools address. `StealthyPlaywright::launch(options)` starts the stealthy browser and attaches in one call; its `cdp()` is the CDP Mode browser and its `playwright()` the Playwright session, both on the same tab. `close` on a session only detaches; `StealthyPlaywright::close` detaches and closes the browser. rustwright is re-exported as `seleniumbase_rs_playwright::rustwright`; its calls block, so `PlaywrightSession::with_page` runs them on the blocking pool.

</Visibility>

## Installation

Add the bridge crate next to the library:

<CodeGroup>

```bash Command line
cargo add seleniumbase-rs --git https://github.com/MustCodeAl/seleniumbase-rs
cargo add seleniumbase-rs-playwright --git https://github.com/MustCodeAl/seleniumbase-rs
```

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-playwright = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
```

</CodeGroup>

A plain build of the library does not compile `rustwright`; only a project that lists the bridge
does. Playwright does not launch a browser here, so there is no `playwright install` step and
nothing to download: the browser is the Chrome that `sb_cdp` starts, the system Chrome by
default, or Chromium with `LaunchOptionsBuilder::use_chromium(true)`. Point
`LaunchOptionsBuilder::executable_path` at a Chrome of your own.

## Usage

Stealthy Playwright Mode comes in three formats, like the Python one:

| Python | Rust |
|---|---|
| `sb_cdp.Chrome()`, then `connect_over_cdp(sb.get_endpoint_url())` | `Browser::launch(..)`, then `PlaywrightSession::attach(&browser)` |
| `cdp_driver.start_async()`, then `connect_over_cdp(driver.get_endpoint_url())` | `StealthyPlaywright::launch(..)`: launch and attach in one call |
| `SB(uc=True)` with `activate_cdp_mode()`, then `connect_over_cdp(sb.get_endpoint_url())` | `PlaywrightSession::connect_over_cdp("http://HOST:PORT")` for the Chrome that chromedriver started |

### 1. Attach to a browser you launched

Ideal for programs that use the pure CDP API (`sb_cdp`) and want Playwright on the same browser.
`attach` uses the HTTP address of the browser's DevTools endpoint (`Browser::http_url`), which is
what Python's `get_endpoint_url()` returns:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSession;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let session = PlaywrightSession::attach(&browser).await?;

    session.goto("https://example.com").await?;
    println!("{}", session.get_text("h1").await?);

    session.close().await?; // detaches; the browser keeps running
    browser.close().await?;
    Ok(())
}
```

The session works on the first page the browser has open, or on a new one if it has none. A
browser that is already on a page keeps it: open the page with the CDP API first and the session
starts on that page.

### 2. Launch both in one call

`StealthyPlaywright::launch` starts the stealthy browser and attaches Playwright to it. The CDP
API (`cdp()`) and the Playwright API (`playwright()`) then work on the same tab at the same time:

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
    // ... and the CDP API reads it from the same tab.
    let title = stealthy.cdp().default_page().await?.title().await?;
    println!("{title}");

    stealthy.close().await?; // detaches Playwright, then closes the browser
    Ok(())
}
```

The repository's `crates/playwright/examples/playwright_mode.rs` is this flow against a page it
serves from `127.0.0.1`:

```bash
cargo run -p seleniumbase-rs-playwright --example playwright_mode
```

### 3. A browser that chromedriver started

The WebDriver engine starts Chrome through chromedriver. Chrome reports its DevTools address in
the session's capabilities (`goog:chromeOptions.debuggerAddress`, a bare `host:port`). Put
`http://` in front of it and attach:

```rust,no_run
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSession;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let debugger_address = "127.0.0.1:9222"; // from the session's capabilities
    let session = PlaywrightSession::connect_over_cdp(&format!("http://{debugger_address}")).await?;
    println!("{}", session.evaluate("document.title").await?);
    session.close().await?; // detaches; chromedriver still owns the browser
    Ok(())
}
```

This format needs chromedriver, which the first two do not.

### The three formats compared

- **`Browser::launch` and `attach`**: the simplest set-up. CDP launches the stealthy browser and
  there is no WebDriver.
- **`StealthyPlaywright::launch`**: the same, in one value that also closes everything in the
  right order.
- **`connect_over_cdp`**: for a browser that something else started, such as chromedriver. The
  stealth is whatever that browser was started with.

## Converting a regular Playwright script

A regular Playwright script launches its own browser:

```rust,ignore
let browser = chromium().launch(LaunchOptions { headless: Some(false), ..Default::default() })?;
let page = browser.new_page()?;
page.goto("https://example.com", GotoOptions::default())?;
```

The Stealthy Playwright Mode version starts the stealthy browser and attaches instead:

```rust,no_run
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::StealthyPlaywright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stealthy =
        StealthyPlaywright::launch(LaunchOptions::builder().headless(false).build()?).await?;
    stealthy
        .playwright()
        .with_page(|page| {
            page.goto("https://example.com", Default::default())?;
            Ok(())
        })
        .await?;
    stealthy.close().await?;
    Ok(())
}
```

## Using all of rustwright

`PlaywrightSession` has the page actions most scripts need, as `async` methods that return the
errors the other engines return: `goto`, `click`, `type_text`, `get_text`, `evaluate`,
`screenshot`, `new_page` and `close`. A missing element is `ElementNotFound`, an element that
cannot be clicked is `ElementNotInteractable`, and a wait that runs out is `WaitTimeout`.

Everything else rustwright can do is one call away. rustwright's page calls block the thread they
run on, so `with_page` runs a closure with the active page on Tokio's blocking pool and gives back
its result. `page()` and `browser()` hand out the plain rustwright handles for code that is
already on a blocking thread. The crate re-exports the engine as `seleniumbase_rs_playwright::rustwright`,
so the project lists no second dependency:

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

## Details

- The stealth is the browser's, not Playwright's. The launch switches, the
  [fingerprint](/tutorials/fingerprint_stealth) (`LaunchOptionsBuilder::fingerprint`), the proxy
  and the user agent are set when `sb_cdp` starts the browser, and the attached client inherits
  them. The attach itself changes none of them.
- rustwright's Rust API in version 0.3 has a browser and pages, but no browser-context type: the
  bridge returns `rustwright::Browser` and `rustwright::Page`, not the `browser.contexts[0].pages[0]`
  of Python. `PlaywrightSession` starts on the first open page.
- Default timeouts differ between Playwright and SeleniumBase. A 30-second default in a
  Playwright call might be 10 seconds in the matching SeleniumBase call.
- When you give a custom timeout, Playwright counts milliseconds and SeleniumBase counts seconds.
  `page.wait_for_timeout(2000)` in Playwright is `sb.sleep(2)` in SeleniumBase.
- Hard sleeps are discouraged in general, but in stealth mode the extra waiting helps the
  automation look human.
- Playwright's `:has-text()` selector is SeleniumBase's `:contains()` selector, except that
  `:has-text()` ignores case and `:contains()` does not.
- For additional stealth, open pages with the CDP API (`stealthy.cdp().default_page().await?` and
  `goto`) instead of Playwright's `goto`.

## Proxy with authentication

Do two things to use an authenticated proxy in Stealthy Playwright Mode:

1. Give the proxy when the browser launches: `LaunchOptions::builder().proxy("USER:PASS@IP:PORT")`.
2. Open the page with the CDP API before Playwright attaches, so the proxy settings are in force.

For that order, launch the browser yourself and call `attach` afterwards, as in format 1:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSession;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = LaunchOptions::builder()
        .use_chromium(true)
        .proxy("user:pass@server:port")
        .build()?;
    let browser = Browser::launch(options).await?;
    browser.default_page().await?.goto("https://example.com").await?;

    let session = PlaywrightSession::attach(&browser).await?;
    println!("{}", session.get_text("h1").await?);
    session.close().await?;
    browser.close().await?;
    Ok(())
}
```

## Notes

- The Playwright engine is optional and lives outside the library: a project that does not list
  `seleniumbase-rs-playwright` never compiles it.
- `StealthyPlaywright` and `PlaywrightSession` are not `Plugin`s of the [plugin API](/tutorials/plugins):
  a plugin hooks into an engine's browser, and the bridge starts a second client on it.
- For stealth with a WebDriver browser use `DriverMode::Uc` or `DriverMode::Cdp`. For a Chrome
  session with no WebDriver at all, see the [pure CDP engine](/tutorials/pure_cdp).

## Troubleshooting

<AccordionGroup>

<Accordion title="`PlaywrightSession` or `StealthyPlaywright` not found" icon="puzzle-piece">

**Likely cause.** The crate is not a dependency. The library has no `playwright` feature any more.

**Fix.** Add `seleniumbase-rs-playwright` as shown under Installation, and import the types from
`seleniumbase_rs_playwright`.

</Accordion>

<Accordion title="`attach failed`" icon="plug">

**Likely cause.** Nothing answers at the DevTools address, or the browser has already closed.

**Fix.** Attach while the browser handle is alive: `Browser::http_url()` names the address. For a
chromedriver browser, check that the address starts with `http://`.

</Accordion>

<Accordion title="Chrome is not found" icon="download">

**Likely cause.** `sb_cdp` looks for the system Chrome (or Chromium with `use_chromium(true)`).

**Fix.** Install Chrome, or name one with `LaunchOptionsBuilder::executable_path`.

</Accordion>

<Accordion title="A program built with the bridge cannot load a `libpython`" icon="triangle-exclamation">

**Likely cause.** A dependency of yours turns on `rustwright-core`'s default `python` feature
(Python bindings); this crate leaves it off.

**Fix.** Depend on `rustwright-core` with `default-features = false` too; nothing here needs
Python.

</Accordion>

<Accordion title="Session methods missing" icon="magnifying-glass">

**Likely cause.** The rustwright API differs from the Python Playwright one, and between versions.

**Fix.** Check the rustdocs of the installed `rustwright` version (`cargo doc -p rustwright`),
and use `with_page` for the calls `PlaywrightSession` does not wrap.

</Accordion>

</AccordionGroup>
