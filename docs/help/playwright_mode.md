---
title: "Playwright Mode"
sidebarTitle: "Playwright mode"
---

<Badge color="blue">Crate: seleniumbase-rs-playwright</Badge>

Every method, error and signature of the crate is in the [Playwright bridge API reference](/api/playwright).

**Stealthy Playwright Mode** puts [`rustwright`](https://github.com/Skyvern-AI/rustwright), a Rust
port of Playwright with a native CDP engine, beside a stealthy browser. You drive that browser
with the same API as everywhere else in these docs (`open`, `click`, `type_text`, `assert_text`,
`solve_captcha`, on an `sb_cdp::Page` or a `BaseCase`), and ask for the Playwright page only for
what that API does not do. Under the hood Playwright attaches through `connect_over_cdp`, as in
Python SeleniumBase, so it inherits the stealth of the browser.

<Info>

**What you will learn**

- How to add the `seleniumbase-rs-playwright` crate, which is not a feature of the library.
- How to drive the stealthy browser with the BaseCase API, and which of its calls replace the
  Playwright ones.
- How to attach Playwright to the same tab for what only Playwright does.
- How the three Python formats map to Rust.
- How to convert a regular Playwright script.

</Info>

<Visibility for="agents">

Summary for AI exports: the library `seleniumbase-rs` has no Playwright engine and no `playwright` feature. Stealthy Playwright Mode is the separate crate `seleniumbase-rs-playwright` (`crates/playwright` in the repository). Drive the stealthy browser with the BaseCase API (the traits `BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi` on `sb_cdp::Page` and `BaseCase`, plus `solve_captcha`); the bridge wraps no Playwright call. `PlaywrightSidecar::attach(&target)` attaches rustwright beside an `sb_cdp::Page` or a `BaseCase`, on the tab that handle is on (found by target id, then by a unique URL; two tabs that cannot be told apart are refused with the open URLs). `PlaywrightSidecar::connect(&endpoint)` attaches to any `DevToolsEndpoint` and takes its first page. `run(|pw| ..)` runs a closure with the `rustwright::Page` on the blocking pool; `page()` and `browser()` are the plain handles; `close` only detaches, and is `Ok` once the browser is gone. `BaseCase::devtools_endpoint()` and `Browser::devtools_endpoint()` are Python's `get_endpoint_url()`. rustwright is re-exported as `seleniumbase_rs_playwright::rustwright`.

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

## Use the BaseCase API on the stealthy browser

The capability traits `BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` are
implemented by `BaseCase` (WebDriver) and by `sb_cdp::Page` (pure CDP, no driver), so a helper
written once over them runs on either engine. Launch the stealthy browser, take its page, and
drive it as in the rest of these docs:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{AssertionApi, BrowserApi};

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let mut page = browser.default_page().await?;

    page.open("https://example.com").await?;
    page.assert_text("Example Domain", "h1").await?;

    browser.close().await
}
```

`Page` also carries the SeleniumBase extras the Python Stealthy Playwright examples call, such as
`solve_captcha`, so most of those scripts need no Playwright page at all. Here is what a
Playwright call becomes:

| Playwright | Pure CDP (`sb_cdp::Page`) | `BaseCase` |
|---|---|---|
| `page.goto(url)` | `Page::goto`, or `BrowserApi::open` | `BaseCase::open` |
| `page.click(sel)` | `Locator::click`, or `ElementApi::click` | `BaseCase::click` |
| `page.fill(sel, text)`, `page.type` | `Locator::fill` and `Locator::type_text`, or `ElementApi::type_text` | `BaseCase::type_text` |
| `text_content`, `inner_text` | `Locator::text`, or `ElementApi::get_text` | `BaseCase::get_text` |
| `page.get_attribute(sel, name)` | `Locator::attribute`, or `ElementApi::get_attribute` | `BaseCase::get_attribute` |
| `page.evaluate(js)` | `Page::evaluate`, `Page::evaluate_as` | `BaseCase::execute_script` |
| `page.screenshot(path)` | `ScreenshotApi::save_screenshot`, `Page::screenshot` (bytes) | `BaseCase::save_screenshot_to_path` |
| `browser.new_page()` | `Browser::new_page`, `Browser::new_page_at` | `BaseCase::open_new_tab` |
| `page.title()`, `page.url`, `page.content()` | `Page::title`, `Page::url`, `Page::content` | `get_title`, `get_current_url`, `get_page_source` |
| `reload`, `go_back`, `go_forward` | `Page::reload`, `Page::back`, `Page::forward` | `refresh`, `go_back`, `go_forward` |
| `page.wait_for_selector(sel)` | `Locator::wait_for` | `BaseCase::wait_for_element` |
| `page.wait_for_function(js)` | `Page::wait_for_function` | |
| `page.wait_for_timeout(ms)` | `seleniumbase_rs::tokio::time::sleep` | `BaseCase::sleep(seconds)` |
| `locator(..).nth(i)`, `.count()`, `.first()` | `Locator::nth`, `Locator::count`, `Locator::first` | |
| `expect(locator).to_have_text(..)`, `to_be_visible()` | `Locator::expect`, then `to_have_text`, `to_be_visible` | `BaseCase::assert_text`, `assert_element` |
| `expect(page).to_have_title(..)`, `to_have_url(..)` | `Page::expect`, then `to_have_title`, `to_have_url` | `BaseCase::assert_title` |
| `page.route(..)` | `Page::intercept` | |
| HAR recording | `Page::start_har` | |
| `page.pdf()` | `Page::pdf` | |
| Tracing (`context.tracing`) | `Page::start_trace` (this project's trace folder and viewer, not Playwright's `trace.zip`) | |
| `sb.solve_captcha()` | `Page::solve_captcha` | `BaseCase::solve_captcha` |
| `sb.get_endpoint_url()` | `Browser::devtools_endpoint` | `BaseCase::devtools_endpoint` |

What stays Playwright's is what neither API has: selector engines such as `role=`, `text=` and
`:has-text()`, `get_by_role`, `get_by_label` and `get_by_test_id`, Playwright's own `trace.zip`,
video and `FrameLocator`, where rustwright's Rust crate has them. Reach those through the
sidecar below.

## Add Playwright for what only Playwright does

`PlaywrightSidecar` attaches rustwright to the browser your `Page` or `BaseCase` drives, on the
tab that handle is on. `run` hands a closure the rustwright page. rustwright's page calls block
the thread they run on, so `run` puts the closure on Tokio's blocking pool and gives back its
result:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{AssertionApi, BrowserApi};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let mut page = browser.default_page().await?;

    // This project's API on the stealthy browser.
    page.open("https://example.com").await?;
    page.assert_text("Example Domain", "h1").await?;

    // Playwright only for what that API does not do, on the same tab.
    let playwright = PlaywrightSidecar::attach(&page).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");

    playwright.close().await?; // detaches; the browser and the tab stay
    browser.close().await
}
```

The repository's `crates/playwright/examples/playwright_mode.rs` is this flow against a page it
serves from `127.0.0.1`:

```bash
cargo run -p seleniumbase-rs-playwright --example playwright_mode
```

- **Attach late, detach early.** While it is attached, rustwright is one more DevTools client of
  the browser. Do the stealth-sensitive work (opening the page, the CAPTCHA) with your own API
  first, attach for the Playwright-only step, and `close` the sidecar after it. `close` only
  detaches: the browser and its tabs stay, and closing a sidecar whose browser is already gone is
  `Ok`.
- **It is on your tab.** `attach` finds the tab the handle is on by its DevTools target id, then
  by a URL that exactly one open page has. Two tabs it cannot tell apart are refused with an
  error that names the URLs that are open, never guessed.
- **The rustwright page.** `playwright.page()` and `playwright.browser()` are the plain
  `rustwright::Page` and `rustwright::Browser` handles, for code that is already on a blocking
  thread. The crate re-exports the engine as `seleniumbase_rs_playwright::rustwright`, so your
  project lists no second dependency.
- **Errors.** `run` reports a lost browser or page as `BrowserDisconnected` and a timeout as
  `WaitTimeout`; every other rustwright failure is `Playwright` with rustwright's text. There is
  no selector to name, so no element error is made up.

## The three Python formats

Python's Stealthy Playwright Mode comes in three formats. In Rust the first two are one:

| Python | Rust |
|---|---|
| `sb_cdp.Chrome()`, then `connect_over_cdp(sb.get_endpoint_url())` | `Browser::launch(..)` and `default_page()`, then `PlaywrightSidecar::attach(&page)` |
| `cdp_driver.start_async()`, then `connect_over_cdp(driver.get_endpoint_url())` | the same: our API is `async` already |
| `SB(uc=True)` with `activate_cdp_mode()`, then `connect_over_cdp(sb.get_endpoint_url())` | `BaseCase::new` in UC mode, then `PlaywrightSidecar::attach(&sb)` |

A browser that something else started is the fourth road: `PlaywrightSidecar::connect(&endpoint)`.

### 1. A browser you launched

The example above. `Browser::devtools_endpoint()` is what Python's `get_endpoint_url()` returns,
for any other client that wants the same browser (Playwright MCP, `chrome-devtools-mcp`).

### 2. A `BaseCase` in UC mode

`BaseCase::new` starts Chrome through chromedriver with the undetected-mode patches. Chrome
reports its DevTools address in the session's capabilities, and `PlaywrightSidecar::attach(&sb)`
reads it through `BaseCase::devtools_endpoint()`. The sidecar is on the window `sb` is on, so
open the page you want first:

```rust,no_run
use seleniumbase_rs::tokio;
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let mut sb = BaseCase::new(BrowserConfig::default().with_mode(DriverMode::Uc)).await?;
    sb.open("https://example.com").await?;

    let playwright = PlaywrightSidecar::attach(&sb).await?; // the window `sb` is on
    playwright.run(|pw| pw.title(Default::default())).await?;
    sb.assert_element("h1").await?;

    playwright.close().await?;
    sb.quit().await
}
```

This format needs chromedriver, which the first does not. A browser without a DevTools endpoint
to share (Firefox, Safari, a grid that hides it) gets `Unsupported` from `attach`.

The same works inside the test fixtures: `SbCdp::run` hands the test a `&Page` and `Sb::run` a
`&mut BaseCase`, and either can be passed to `attach`.

### 3. A browser that something else started

For a Chrome that no handle of this project drives, such as one started with
`--remote-debugging-port=9222`, attach to its endpoint. `connect` works on the first page the
browser lists, or on a new one if it has none:

```rust,no_run
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let browser = Browser::connect("127.0.0.1", 9222).await?;

    let playwright = PlaywrightSidecar::connect(&browser.devtools_endpoint()).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");

    playwright.close().await?;
    browser.close().await // detaches; the browser keeps running
}
```

The stealth is whatever that browser was started with.

## Converting a regular Playwright script

A regular Playwright script launches its own browser and drives it with Playwright calls.

1. **Convert it.** For JavaScript, TypeScript or Python scripts, `sbrs import-playwright` writes
   pure CDP Rust that launches an `sb_cdp` browser and drives its page with `Page` and
   `Locator` calls: no Playwright page is left in the result ([Migrate Playwright
   tests](/playwright-migration) lists every mapping).
2. **Finish the rest with `run`.** Whatever the importer does not convert (request
   interception, event handlers, popups, loops, helper functions) stays in place as a
   `// TODO(playwright): <statement>` comment. Write those statements with this project's API
   where the table above has one, and with `run` where it does not:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    // Instead of `chromium().launch(..)`: the stealthy browser.
    let browser = Browser::launch(LaunchOptions::builder().headless(false).build()?).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;

    // A statement that only Playwright has: the rustwright page, on the same tab.
    let playwright = PlaywrightSidecar::attach(&page).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");

    playwright.close().await?;
    browser.close().await
}
```

## Details

- The stealth is the browser's, not Playwright's. The launch switches, the
  [fingerprint](/stealth/introduction) (`LaunchOptionsBuilder::fingerprint`), the proxy
  and the user agent are set when `sb_cdp` starts the browser, and the attached client inherits
  them. The attach itself changes none of them. rustwright's own README says its stealth is
  partial and promises no undetectability, which is why the browser's stealth is the one to rely
  on.
- rustwright's Rust API in version 0.3 has a browser and pages, but no browser-context type: the
  bridge returns `rustwright::Browser` and `rustwright::Page`, not the `browser.contexts[0].pages[0]`
  of Python. Its Rust surface moves between versions, so the bridge wraps none of it.
- Default timeouts differ between Playwright and SeleniumBase. A 30-second default in a
  Playwright call might be 10 seconds in the matching SeleniumBase call. The attach itself waits
  as long as the handle you attach beside waits by default.
- When you give a custom timeout, Playwright counts milliseconds and SeleniumBase counts seconds.
  `page.wait_for_timeout(2000)` in Playwright is `sb.sleep(2)` in SeleniumBase.
- Hard sleeps are discouraged in general, but in stealth mode the extra waiting helps the
  automation look human.
- Playwright's `:has-text()` selector is SeleniumBase's `:contains()` selector, except that
  `:has-text()` ignores case and `:contains()` does not.
- For additional stealth, open pages with this project's API (`Page::goto`, `BrowserApi::open`)
  and attach afterwards.

## Proxy with authentication

Do two things to use an authenticated proxy in Stealthy Playwright Mode:

1. Give the proxy when the browser launches: `LaunchOptions::builder().proxy("USER:PASS@IP:PORT")`.
2. Open the page with this project's API before Playwright attaches, so the proxy settings are
   in force.

For that order, launch the browser yourself, open the page and attach afterwards:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{BrowserApi, ElementApi};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let options = LaunchOptions::builder()
        .use_chromium(true)
        .proxy("user:pass@server:port")
        .build()?;
    let browser = Browser::launch(options).await?;
    let mut page = browser.default_page().await?;
    page.open("https://example.com").await?;
    println!("{}", page.get_text("h1").await?);

    let playwright = PlaywrightSidecar::attach(&page).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");
    playwright.close().await?;
    browser.close().await
}
```

## Notes

- The Playwright engine is optional and lives outside the library: a project that does not list
  `seleniumbase-rs-playwright` never compiles it.
- `PlaywrightSidecar` is not a `Plugin` of the [plugin API](/tutorials/plugins):
  a plugin hooks into an engine's browser, and the bridge starts a second client on it.
- For stealth with a WebDriver browser use `DriverMode::Uc` or `DriverMode::Cdp`. For a Chrome
  session with no WebDriver at all, see the [pure CDP engine](/tutorials/pure_cdp).

## Troubleshooting

<AccordionGroup>

<Accordion title="`PlaywrightSidecar` not found" icon="puzzle-piece">

**Likely cause.** The crate is not a dependency. The library has no `playwright` feature, and the
session and launcher types of earlier versions of the bridge are gone: `PlaywrightSidecar`
replaces them.

**Fix.** Add `seleniumbase-rs-playwright` as shown under Installation, and import
`PlaywrightSidecar` from `seleniumbase_rs_playwright`. A browser you launch with `Browser::launch`
is attached through its page: `PlaywrightSidecar::attach(&page)` after `default_page()`.

</Accordion>

<Accordion title="`attach failed`" icon="plug">

**Likely cause.** Nothing answers at the DevTools address, or the browser has already closed.

**Fix.** Attach while the browser handle is alive. `Browser::devtools_endpoint()` and
`BaseCase::devtools_endpoint()` name the address.

</Accordion>

<Accordion title="`no single open page is the tab to attach to`" icon="window-restore">

**Likely cause.** `attach` could not tell which tab the handle is on: the target id matched no
page, and zero or several open pages have the handle's URL. The message lists the URLs that are
open.

**Fix.** Attach beside the `sb_cdp::Page` of the tab, which is always found by its id, or give
the tabs different URLs. For a `BaseCase`, open the page you want in its current window first.

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

<Accordion title="rustwright methods missing" icon="magnifying-glass">

**Likely cause.** The rustwright API differs from the Python Playwright one, and between versions.

**Fix.** Check the rustdocs of the installed `rustwright` version (`cargo doc -p rustwright`);
`run` gives your closure the plain `rustwright::Page`.

</Accordion>

</AccordionGroup>
