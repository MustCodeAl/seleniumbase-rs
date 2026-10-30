---
title: "Capability traits"
sidebarTitle: "Capability traits"
description: "BrowserApi, ElementApi, AssertionApi and ScreenshotApi, shared by both engines."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Capability traits

`seleniumbase_rs::{BrowserApi, ElementApi, AssertionApi, ScreenshotApi}` are
four small `async_trait` traits (defined in `src/api/traits.rs`). Both engines
implement all four: `BaseCase` (WebDriver) and `sb_cdp::Page` (pure CDP). A helper
written over the traits runs on either engine. None of them names an engine
type; to keep an element handle, use `BaseCase::find_element` (a
`drivers::WebElement`) or `Page::locator(..)`. Every method returns
`seleniumbase_rs::Result<T>`. Selector strings are the same on both engines:
CSS by default, XPath for `//...`, link text for `link=...` (see the
[Selectors guide](/tutorials/selectors)).

```rust,no_run
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi};

async fn sign_in<E>(sb: &mut E) -> Result<(), seleniumbase_rs::SeleniumBaseError>
where
    E: BrowserApi + ElementApi + AssertionApi,
{
    sb.open("https://example.com/login").await?;
    sb.type_text("#user", "alice").await?;
    sb.click("#submit").await?;
    sb.assert_element("#dashboard").await
}
```

### BrowserApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `open(&mut self, url: &str) -> Result<()>` | Navigates the active window; a URL without a scheme gets `https://`; a file path or an unsupported scheme (`ftp:`, `javascript:`, `mailto:`) is refused with `InvalidConfig` (use `file://` for a local file). | `Page::goto(url)`. |
| `quit(&mut self) -> Result<()>` | Closes the browser session. | Closes this tab only. To close the browser call `Browser::close`. |
| `refresh(&self) -> Result<()>` | Reloads the page. | `Page::reload()`. |
| `go_back(&self) -> Result<()>` | Browser back. | `Page::back()`. |
| `go_forward(&self) -> Result<()>` | Browser forward. | `Page::forward()`. |
| `get_title(&mut self) -> Result<String>` | The page title. | `Page::title()`. |
| `get_url(&mut self) -> Result<String>` | The current URL. | `Page::url()`. |

### ElementApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `click(&mut self, css: &str) -> Result<()>` | Clicks the element. | `locator(css).click()`. |
| `double_click(&mut self, css: &str) -> Result<()>` | Double-clicks it. | `locator(css).double_click()`. |
| `type_text(&mut self, css: &str, text: &str) -> Result<()>` | Waits for the element to be visible, clears it and types. | Clears the field, then types one key at a time. |
| `get_text(&mut self, css: &str) -> Result<String>` | The element's visible text, after waiting for it to be present. | `locator(css).text()`. |
| `get_attribute(&mut self, css: &str, attr: &str) -> Result<Option<String>>` | The attribute value, `None` when absent. | `locator(css).attribute(attr)`. |

### AssertionApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `assert_title(&mut self, expected: &str) -> Result<()>` | Fails with `AssertionFailed` unless the title equals `expected` exactly. | `expect().to_have_title(expected)`, which retries until the timeout. |
| `assert_element(&self, css: &str) -> Result<()>` | Fails unless the element is present in the DOM (visible or not). | `locator(css).expect().to_exist()`. |
| `assert_text(&mut self, expected: &str, css: &str) -> Result<()>` | Fails unless the element's text contains `expected`. | `locator(css).expect().to_contain_text(expected)`. |
| `assert_no_js_errors(&self) -> Result<()>` | Fails if the browser log holds a `SEVERE` entry (a few known CDN sources are ignored). | Fails if the document raised an uncaught script error; the message names the first. |

<Note>

The trait method `assert_text(expected, css)` takes the text first, like
`BaseCase::assert_text(text, selector)` and Python's `assert_text`.

</Note>

### ScreenshotApi

| Method | Behaviour |
|---|---|
| `save_screenshot(&self, filename: &str) -> Result<PathBuf>` | Saves a PNG into the logs directory and returns its path. `filename` must be a plain file name (no directory part, no `..`). The same for both engines. |
| `screenshot_as_png(&self) -> Result<Vec<u8>>` | The current page as PNG bytes. |
