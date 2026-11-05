---
title: "Pure CDP"
sidebarTitle: "Overview"
description: "Drive Chrome over the DevTools Protocol, with no WebDriver: Browser, Page and Locator."
---

<Note>

The pieces below are the pure CDP engine, imported from `seleniumbase_rs::sb_cdp`. Methods are listed as `name(arguments)` on the later pages. Call them on the type named in the heading, and end an async call with `.await?`. Argument names and selector syntax are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page). A walkthrough is [Pure CDP](/tutorials/pure_cdp), not this page.

</Note>

The pure CDP engine launches Chrome itself and talks to it over the DevTools Protocol. There is no chromedriver, and none of the markers a WebDriver session leaves in the page. It is the engine behind the `cdp` MCP server.

The pieces nest the way a browser does:

| Piece | What it is |
|---|---|
| `Browser` | The process and its tabs. Holds browser-wide settings such as permissions and the download directory. |
| `BrowserContext` | An isolated context inside that browser. Its pages do not share the default context's cookies. |
| `Page` | One tab. Navigates, evaluates scripts, and hands out the handles below. Cloning a page is another handle to the same tab. |
| `Locator` | Finds elements lazily and waits before it acts, so it keeps working when the page re-renders. |
| `Mouse`, `Keyboard`, `Human` | Pointer and key input. `Mouse` and `Keyboard` deliver trusted events (`isTrusted`). |
| `Cookies`, `Storage`, `Window`, `Emulation` | State and environment attached to a tab. |
| `LocatorExpect`, `PageExpect` | Assertions that retry until they hold or the timeout runs out. |

A page the browser opens waits **7 seconds** for elements and conditions. `with_timeout` on the page, and on a locator, changes that for the handle it returns. Locators created from a page use the page's timeout.

Selectors are plain strings, classified the way SeleniumBase classifies them: `"#id"` is CSS, `"//div"` is XPath, `"link=Home"` is a link. The same rules are in [Selector syntax](/tutorials/api_reference#selector-syntax).

```rust
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("https://seleniumbase.io/simple/login").await?;
    page.locator("#username").fill("demo_user").await?;
    page.locator("#password").fill("secret_pass").await?;
    page.locator("#log-in").click().await?;
    page.locator("h1").expect().to_contain_text("Welcome").await?;

    browser.close().await?;
    Ok(())
}
```

Coming from Python SeleniumBase, `sb.cdp.click(selector)` is `page.locator(selector).click()`. Helpers such as `click_if_visible` and `click_nth_element` are compositions: `page.locator(selector).visible().first()` and `page.locator(selector).nth(index)`.

With the `test-util` feature, `Browser::new_mocked` returns a browser backed by a scripted `MockCtrl`, so page-driving code can be tested without Chrome. An app that embeds Chromium can drive its webview with `Browser::over_channel` and a `CdpTransport` instead of a debugging port.

## Pages

| Page | Covers |
|---|---|
| [Browser](/api/cdp-browser) | `Browser`, `LaunchOptions`, launch, connect, close, contexts |
| [Page](/api/cdp-page) | Navigation, scripts, frames, captcha, messenger |
| [Locator](/api/cdp-locator) | `Locator`, `LocatorExpect`, `PageExpect` |
| [Input](/api/cdp-input) | `Mouse`, `Keyboard`, `Human` |
| [Network](/api/cdp-network) | Interception, HAR, trace, downloads |
| [State](/api/cdp-state) | Cookies, storage, window, emulation, `StealthPlan` |
| [Pool](/api/cdp-pool) | `BrowserPool`, `Lease`, `SessionStore` |
