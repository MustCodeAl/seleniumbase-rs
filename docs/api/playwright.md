---
title: "Playwright bridge API"
sidebarTitle: "Playwright bridge"
description: "PlaywrightSidecar and PlaywrightTarget of the seleniumbase-rs-playwright crate: attach rustwright beside an sb_cdp Page or a BaseCase, with every method, its errors and examples."
keywords: ["playwright", "rustwright", "PlaywrightSidecar", "PlaywrightTarget", "attach", "connect", "run", "devtools_endpoint", "stealth", "sb_cdp"]
---

<Note>

This page is the reference for the crate `seleniumbase-rs-playwright`. The guide, with the Python formats that map to Rust, the table of Playwright calls and the conversion of a regular Playwright script, is [Playwright mode](/help/playwright_mode). The browser the bridge attaches to is the [pure CDP engine](/tutorials/pure_cdp) (`sb_cdp`), which is what Python's CDP Mode (`sb.cdp`) maps to, or the Chrome of a `BaseCase` in UC mode.

</Note>

## The crate

Stealthy Playwright Mode, as in Python SeleniumBase: [`rustwright`](https://github.com/Skyvern-AI/rustwright), a Rust port of Playwright with a native CDP engine, attaches to a stealthy browser through its DevTools endpoint (`connect_over_cdp`). The browser keeps its fingerprint, proxy and launch switches.

The stealthy browser is driven with this project's own API: the capability traits `BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` that `sb_cdp::Page` and `BaseCase` implement, plus their SeleniumBase extras such as `solve_captcha`. The bridge wraps no Playwright call. Playwright joins, on the same tab, for what that API does not do.

The bridge is a separate crate, not a feature of the library. `seleniumbase-rs` has no `playwright` feature and no `rustwright` dependency; a project that does not list the bridge never compiles `rustwright`. It is a bridge, not a `Plugin`. Add it next to the library:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.8.0" }
seleniumbase-rs-playwright = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.8.0" }
```

The crate root has three public items:

| Item | What it is |
|---|---|
| `PlaywrightSidecar` | A rustwright client attached beside one of this project's handles, on the tab that handle is on. |
| `PlaywrightTarget` | The sealed trait of the handles a sidecar attaches beside: `sb_cdp::Page` and `BaseCase`. |
| `rustwright` | The whole `rustwright` crate, re-exported (`pub use rustwright;`), so a project lists one dependency. |

All examples on this page import only from `seleniumbase_rs` and `seleniumbase_rs_playwright`.

## PlaywrightSidecar

```rust
pub struct PlaywrightSidecar { /* private fields */ }
```

A `rustwright` client attached to a running Chromium, on one tab. The name follows `ProxyAuthSidecar`, a DevTools client attached beside a WebDriver session: a sidecar never starts a browser and never closes one. The attached page keeps whatever fingerprint, proxy and launch switches the browser was started with; the sidecar changes none of them. While it is attached, rustwright is one more DevTools client of the browser, so attach late and detach early.

rustwright's page calls block the thread they run on, so `run` runs its closure on Tokio's blocking pool and the runtime is never stalled. `close` only detaches: a browser that another handle owns is never shut down from the sidecar.

`PlaywrightSidecar` implements `Debug`; it prints the `connected` flag and nothing else, because rustwright's handles are not `Debug`.

### Attaching

```rust
pub async fn attach(target: &impl PlaywrightTarget) -> Result<Self, SeleniumBaseError>
pub async fn connect(endpoint: &DevToolsEndpoint) -> Result<Self, SeleniumBaseError>
```

- `attach(&target)` attaches to the browser of `target` and works on the tab `target` is on. The target is an `sb_cdp::Page` or a `BaseCase`. A user of `sb_cdp::Browser` calls `default_page()` first. The attach waits as long as the target waits by default (`Page::timeout()`, `BaseCase::timeout_secs()`).
- `connect(&endpoint)` attaches to any Chromium's DevTools endpoint (`seleniumbase_rs::sb_cdp::DevToolsEndpoint`) and works on the first page it lists, or on a new one if it has none. It is for a browser that no handle of this project drives; with a handle, `attach` picks the right tab. `Browser::devtools_endpoint()` and `BaseCase::devtools_endpoint()` give the endpoint of a browser this project drives, and `Browser::connect(host, port)` attaches the pure CDP engine to one that something else started.

Both fail with `SeleniumBaseError::Playwright` if nothing answers at the endpoint, the browser cannot be controlled, or the attach times out. `attach` also returns the errors of the target: `Unsupported` for a `BaseCase` whose browser has no DevTools endpoint to share (Firefox, Safari, a grid that hides it), and `SessionNotStarted` for a `BaseCase` after `quit`.

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
    // `title` stands in for a call only Playwright has: our own API reads it with
    // `page.title()`, and the closure sees the same tab.
    let ours = page.title().await?;
    let theirs = playwright.run(|pw| pw.title(Default::default())).await?;
    assert_eq!(ours, theirs);
    println!("{theirs}");

    playwright.close().await?; // detaches; the browser and the tab stay
    browser.close().await
}
```

A `BaseCase` is a target too. In UC mode it is Python's `SB(uc=True)` with a Playwright page beside it:

```rust,no_run
use seleniumbase_rs::tokio;
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let mut sb = BaseCase::new(BrowserConfig::default().with_mode(DriverMode::Uc)).await?;
    sb.open("https://example.com").await?;

    let playwright = PlaywrightSidecar::attach(&sb).await?; // the window `sb` is on
    // `title` stands in for a call only Playwright has: our own API reads it with
    // `sb.get_title()`, and the closure sees the same window.
    let theirs = playwright.run(|pw| pw.title(Default::default())).await?;
    assert_eq!(sb.get_title().await?, theirs);
    sb.assert_element("h1").await?;

    playwright.close().await?;
    sb.quit().await
}
```

A browser that something else started, with `connect`:

```rust,no_run
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    // A Chrome started with --remote-debugging-port=9222.
    let browser = Browser::connect("127.0.0.1", 9222).await?;

    let playwright = PlaywrightSidecar::connect(&browser.devtools_endpoint()).await?;
    // `title` stands in for a call only Playwright has: our own API reads it with
    // `browser.default_page().await?.title()`.
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");

    playwright.close().await?;
    browser.close().await // detaches; the browser keeps running
}
```

### Which tab `attach` takes

The sidecar must be on the tab its target is on, not on whichever page the browser lists first. `attach` gets two hints from the target and picks one of the browser's pages:

| Target | Tab id hint | URL hint |
|---|---|---|
| `sb_cdp::Page` | `Page::id()`, the tab's DevTools target id | `Page::url()` |
| `BaseCase` | `BaseCase::get_current_window_handle()`, when the session has one | `BaseCase::get_current_url()` |

A page whose target id is the id hint wins. Otherwise the one page whose URL is the URL hint is taken. If neither picks exactly one page, `attach` returns `SeleniumBaseError::Playwright` naming the URLs that are open: it never guesses, because a silent wrong tab is worse than an error. Two tabs on the same URL are therefore refused unless the id tells them apart: attach beside the `sb_cdp::Page` of the tab, or give the tabs different URLs. For a `BaseCase` on a direct CDP attachment (`BrowserConfig::with_cdp_url`) there is no window handle, so the URL decides. A chromedriver window handle is compared with the target id as it is; when it is not equal, the URL hint decides.

### Handles

```rust
pub fn browser(&self) -> &Browser
pub fn page(&self) -> &Page
pub fn is_connected(&self) -> bool
```

- `browser()` is the `rustwright::Browser` the sidecar is attached to.
- `page()` is the `rustwright::Page` the sidecar is on. It is a cheap handle: clone it to move it elsewhere. Its calls block, so use it only on a thread that may block; from async code use `run`.
- `is_connected()` is true while the connection to the browser is open.

### run

```rust
pub async fn run<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>
where
    F: FnOnce(Page) -> Result<T, RwError> + Send + 'static,
    T: Send + 'static,
```

Runs `work` with the rustwright page on Tokio's blocking pool and returns its result. This is the way to use any rustwright page call from async code, so everything rustwright can do is one call away. `RwError` is `rustwright::Error`.

`run` maps only the failures that mean the same here as everywhere else in the project; every other failure stays `Playwright` with the text rustwright gave. There is no selector to name, so no element error is made up:

| rustwright reports | `SeleniumBaseError` |
|---|---|
| The connection closed or dropped, the target closed, or the page crashed | `BrowserDisconnected` |
| An action timeout or a plain timeout | `WaitTimeout` |
| An action failure | The same table applied to its cause; the failure's own text if the cause is neither of the above |
| Anything else, including a failed actionability check | `Playwright`, with rustwright's text |

A panic in `work` is a `Playwright` error too, not a hang.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs_playwright::rustwright::GotoOptions;
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;
    let playwright = PlaywrightSidecar::attach(&page).await?;

    // Any rustwright call goes here. Navigating (our own is `page.goto`) only shows that the
    // rustwright page and `page` are one tab: `page.url()` follows it.
    playwright
        .run(|pw| pw.goto("https://example.com", GotoOptions::default()).map(drop))
        .await?;
    println!("{}", page.url().await?);

    playwright.close().await?;
    browser.close().await
}
```

### close

```rust
pub async fn close(&self) -> Result<(), SeleniumBaseError>
```

Detaches this client. The browser and its tabs stay; whoever started the browser shuts it down (for one from `sb_cdp`, its own `close`). Closing a sidecar whose connection has already ended, for instance after `Browser::close`, is `Ok`. It fails with `SeleniumBaseError::Playwright` if the connection does not close cleanly.

## PlaywrightTarget

```rust
pub trait PlaywrightTarget: /* sealed */ {}
```

The handles a sidecar can attach beside: `sb_cdp::Page` and `BaseCase`. The trait is sealed, so it cannot be implemented elsewhere: what a target tells the sidecar (its browser's endpoint, its tab and its timeout) is the crate's business. The same handles come out of the test fixtures: `SbCdp::run` hands the test a `&Page` and `Sb::run` a `&mut BaseCase`, and either can be passed to `attach`.

## The rustwright re-export

`seleniumbase_rs_playwright::rustwright` is the `rustwright` crate itself, so a project lists no second dependency. The types the bridge hands out are its types: `rustwright::Browser` and `rustwright::Page` from `browser()`, `page()` and `run`, and `rustwright::Error` in the signature of `run`. rustwright's Rust API has a browser and pages but no browser-context type. Its Rust surface moves between versions (this crate pins `rustwright = "0.3"`), which is why the bridge wraps none of it: check the rustdocs of the installed version (`cargo doc -p rustwright`).

## Errors

Every fallible method returns `Result<T, SeleniumBaseError>`, the same error type as the rest of the library. The variants the bridge returns:

| Variant | Returned by |
|---|---|
| `Playwright` | `attach` and `connect` (nothing answers, the browser cannot be controlled, no single open page is the target's tab), `run` for a rustwright failure the table above does not name, `close`, and a blocking task that panicked or was cancelled |
| `BrowserDisconnected`, `WaitTimeout` | `run` |
| `Unsupported`, `SessionNotStarted`, `CdpDriver` | `attach` beside a `BaseCase` whose browser has no DevTools endpoint to share, that was quit, or whose endpoint does not answer |

See [the errors of the crate root](/api/crate-root#errors) for the helpers every `SeleniumBaseError` has, such as `is_transient()` and `category()`.

## See also

- [Playwright mode](/help/playwright_mode): the guide, the Python-to-Rust mapping and the conversion of a regular Playwright script.
- [CDP and UC helpers](/api/cdp-uc): `BaseCase::devtools_endpoint()`, the endpoint a UC session shares.
- [Pure CDP engine](/tutorials/pure_cdp): the browser the bridge attaches to (`Browser`, `Page`, `LaunchOptions`).
- [Crate root](/api/crate-root): `SeleniumBaseError` and the other items of `seleniumbase_rs`.
