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
four small traits (defined in `src/api/traits.rs`) whose methods are native
`async fn`s, with no `#[async_trait]` and no boxed future. The future every method
returns is `Send`, so a helper written over the traits can be handed to
`tokio::spawn`. Both engines
implement all four: `BaseCase` (WebDriver) and `sb_cdp::Page` (pure CDP). A helper
written over the traits runs on either engine. None of them names an engine
type; to keep an element handle, use `BaseCase::find_element` (an
`Element`) or `Page::locator(..)`. Every method returns
`seleniumbase_rs::Result<T>`. Selector strings are the same on both engines:
CSS by default, XPath for `//...`, link text for `link=...` (see the
[Selectors guide](/tutorials/selectors)). A selector parameter is `impl Into<Selector> + Send`: a string, or a `Selector` built with `css`, `xpath`, `link_text` or `partial_link_text` to skip the guess.

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

### Implementing the traits, spawning, and `dyn`

A type implements a trait with a plain `async fn` for each method. The trait
declares each one as `fn name(..) -> impl Future<Output = ..> + Send`, and an
`async fn` in the `impl` fills that in, so the compiler refuses an
implementation whose future is not `Send` (one that holds a `std::sync::MutexGuard`
across an `.await`, say). Code that is generic over a trait can rely on the
`Send` bound without naming it:

```rust,no_run
use seleniumbase_rs::tokio::{self, task::JoinHandle};
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi, Result};

fn spawn_sign_in<E>(mut sb: E) -> JoinHandle<Result<()>>
where
    E: BrowserApi + ElementApi + AssertionApi + Send + 'static,
{
    tokio::spawn(async move {
        sb.open("https://example.com/login").await?;
        sb.click("#submit").await?;
        sb.assert_title("Dashboard").await
    })
}
```

The traits are not dyn compatible: a method with an `async fn` or an
`impl Into<Selector>` argument cannot be called through a trait object, so
write `fn run(sb: &mut impl BrowserApi)` or a generic function, not
`&mut dyn BrowserApi`. To hold either engine in one value, use an `enum` of your
own over `BaseCase` and `sb_cdp::Page` that implements the traits by matching.


### BrowserApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `open(&mut self, url: &str) -> Result<()>` | Navigates the active window; a URL without a scheme gets `https://`; a file path or an unsupported scheme (`ftp:`, `javascript:`, `mailto:`) is refused with `InvalidConfig` (use `file://` for a local file). | `Page::goto(url)`. |
| `quit(&mut self) -> Result<()>` | Closes the browser session. | Closes this tab only. To close the browser call `Browser::close`. |
| `refresh(&self) -> Result<()>` | Reloads the page. | `Page::reload()`. |
| `go_back(&self) -> Result<()>` | Browser back. | `Page::back()`. |
| `go_forward(&self) -> Result<()>` | Browser forward. | `Page::forward()`. |
| `title(&mut self) -> Result<String>` | The page title. | `Page::title()`. |
| `url(&mut self) -> Result<String>` | The current URL. | `Page::url()`. |

### ElementApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `click(&mut self, selector: impl Into<Selector> + Send) -> Result<()>` | Clicks the element. | `locator(selector).click()`. |
| `double_click(&mut self, selector: impl Into<Selector> + Send) -> Result<()>` | Double-clicks it. | `locator(selector).double_click()`. |
| `type_text(&mut self, selector: impl Into<Selector> + Send, text: &str) -> Result<()>` | Waits for the element to be visible, clears it and types. | Clears the field, then types one key at a time. |
| `text(&mut self, selector: impl Into<Selector> + Send) -> Result<String>` | The element's visible text, after waiting for it to be present. | `locator(selector).text()`. |
| `attribute(&mut self, selector: impl Into<Selector> + Send, attr: &str) -> Result<Option<String>>` | The attribute value, `None` when absent. | `locator(selector).attribute(attr)`. |

### AssertionApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `assert_title(&mut self, expected: &str) -> Result<()>` | Fails with `AssertionFailed` unless the title equals `expected` exactly. | `expect().to_have_title(expected)`, which retries until the timeout. |
| `assert_element(&self, selector: impl Into<Selector> + Send) -> Result<()>` | Fails unless the element is present in the DOM (visible or not). | `locator(selector).expect().to_exist()`. |
| `assert_text(&mut self, expected: &str, selector: impl Into<Selector> + Send) -> Result<()>` | Fails unless the element's text contains `expected`. | `locator(selector).expect().to_contain_text(expected)`. |
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
