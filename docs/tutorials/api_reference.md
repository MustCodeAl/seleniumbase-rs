---
title: "API Reference: BaseCase, Capability Traits and the Crate Root"
sidebarTitle: "API reference"
---

`BaseCase` is the main entry point for writing browser automation in
`seleniumbase-rs`. It wraps a WebDriver/CDP session, provides a fluent API for
navigation, interaction, waits, assertions, and exposes advanced helpers for
stealth, CDP, tours, and screenshots.

This page is a high-level reference. For exact signatures and generic bounds,
build the rustdocs with:

```bash Terminal
cargo doc --no-deps --open
```

<Info>

**What you will learn**

- The lifecycle methods for creating and closing sessions.
- Common navigation, interaction, and query methods.
- Wait and assertion patterns.
- Script execution, scrolling, file handling, and advanced interactions.
- CDP/UC helpers and when to use them.
- The capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`) that both engines implement.
- The public items re-exported at the crate root and what each is for.
- Where to find rustdocs for deeper detail.

</Info>

<Visibility for="agents">

Summary for AI exports: `BaseCase` methods are grouped as lifecycle, navigation, interaction, scrolling, waits, assertions, deferred assertions, queries, windows, frames and drivers, cookies and storage, screenshots, script execution, downloads and PDF, CDP and UC helpers, GUI helpers (feature `gui`), alerts, MFA, tours and charts, recorder, and utilities. Selectors are CSS by default, XPath for `//...`, and link text for `link=...`. Methods return `seleniumbase_rs::Result<T>`, so use `?`. `assert_text(text, selector)` takes the text first. `BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` are implemented by both `BaseCase` (WebDriver) and `sb_cdp::Page` (pure CDP). For exact signatures run `cargo doc --no-deps --open`.

</Visibility>

## Reading this page

The tables list each method as `name(arguments)`, without the `sb.` in front
and the `.await?` after. So the row `set_attribute(selector, attr, value)`
stands for a call like this:

<CodeGroup>

```rust,ignore As written in a test
sb.set_attribute("#email", "placeholder", "you@example.com").await?;
```

```rust,ignore The signature
pub async fn set_attribute(&mut self, css: &str, attribute: &str, value: &str)
    -> Result<(), SeleniumBaseError>
```

</CodeGroup>

### Common arguments

The same names mean the same thing in every table. The signatures sometimes
use a longer name (`css` for `selector`, `timeout_secs` for `timeout`).

| Argument | Type | What it is |
|---|---|---|
| `selector` (also `src`, `dst`, `container`) | `&str` | Which element to act on, as CSS, XPath or link text (see [Selector syntax](#selector-syntax)). The first match is used. |
| `text`, `expected` | `&str` | Text to type, or the text an assertion expects. `assert_text(text, selector)` takes the text first. |
| `attr`, `prop` | `&str` | An attribute or property name, such as `"href"` or `"value"`. |
| `value` | `&str` | The value to set: of an attribute, a field or a storage item. |
| `url` | `&str` | A full URL, such as `"https://example.com/login"`. |
| `timeout`, `timeout_secs` | `u64` | How many seconds to wait before failing. A method that takes a `Duration` says so in its signature. |
| `seconds` | `f64` | A pause, as in `sleep(1.5)`. |
| `x`, `y` | integer | A position in pixels (`i32` or `i64`, depending on the method). |
| `path`, `filename` | `&str` or `&Path` | A file to read or write. |
| `index` | `usize` | A position in a list, such as an `<option>` of a `<select>`. |
| `key` | `&str` | A storage key (`get_local_storage_item(key)`) or a key name (`gui_press_key(key)`). |
| `name` | `&str` | A name the method looks up: a cookie, a tour, a chart, a saved baseline. |
| `script` | `&str` | JavaScript to run in the page. |

### Selector syntax

A `selector` string picks its own lookup strategy, as SeleniumBase does:

| Selector starts with | Strategy | Example |
|---|---|---|
| anything else | CSS (the default) | `"#submit"`, `"form input[name=q]"` |
| `/`, `./`, `../`, `(`, or an axis such as `parent::` | XPath | `"//button[text()='Go']"`, `"(//a)[2]"` |
| `link=`, `link_text=`, `text=` | Link text (exact) | `"link=Sign in"` |
| `partial_link_text=`, `partial_link=`, `partial_text=`, `p_link_text=`, `p_link=`, `p_text=` | Link text (contains) | `"p_text=Sign"` |

The prefixes are case-sensitive (`LINK=Home` is CSS). More in the
[Selectors guide](/tutorials/selectors).

### What methods return

Almost every method returns `seleniumbase_rs::Result<T>`, so a test ends each
call with `?` (`sleep` is an exception: it cannot fail).

| Kind of method | Returns |
|---|---|
| Actions (`click`, `type_text`, `open`, `set_attribute`, ...) and assertions | `()`: nothing, or an error |
| `get_text`, `get_title`, `get_current_url` and the like | `String` |
| `get_attribute(selector, attr)`, `get_property(selector, prop)` | `Option<String>`: `None` when it is not set |
| `find_element(selector)` | an element: a `thirtyfour::WebElement` handle to the first match, found within 10 seconds |
| `find_elements(selector)` | `Vec<thirtyfour::WebElement>`: every match, possibly none |
| `execute_script(script)` | `serde_json::Value`: what the script returned |
| `save_screenshot(filename)` | `PathBuf`: where the file was written |

An element handle is the WebDriver engine's own type. Keep the selector
instead when you can: the `BaseCase` methods look the element up again on each
call, so they do not go stale when the page re-renders. The pure CDP engine's
`Locator` (from `page.locator(selector)`) is lazy in the same way.

## Reference pages

<Columns cols={2}>

<Card title="Lifecycle and navigation" icon="rocket" href="/api/lifecycle-navigation">
  Create and close a BaseCase session, open pages and move through history.
</Card>

<Card title="Interaction" icon="hand-pointer" href="/api/interaction">
  Click, type, select, hover, drag, press keys and scroll.
</Card>

<Card title="Waits and assertions" icon="hourglass-half" href="/api/waits-assertions">
  Wait for elements and text, assert on the page, and collect deferred assertions.
</Card>

<Card title="Queries" icon="magnifying-glass" href="/api/queries">
  Read text, attributes, properties, counts and page state.
</Card>

<Card title="Windows, frames and drivers" icon="window-restore" href="/api/windows-frames">
  Switch windows, tabs and frames, and drive several browsers.
</Card>

<Card title="Cookies and storage" icon="cookie-bite" href="/api/cookies-storage">
  Read and write cookies, local storage and session storage.
</Card>

<Card title="Screenshots and scripts" icon="camera" href="/api/screenshots-scripts">
  Take screenshots, run JavaScript and change the DOM.
</Card>

<Card title="Files, downloads and PDF" icon="file-arrow-down" href="/api/files">
  Download and upload files and work with PDFs.
</Card>

<Card title="CDP and UC helpers" icon="user-secret" href="/api/cdp-uc">
  Chrome DevTools and undetected-mode helpers, including the GUI bypass helpers.
</Card>

<Card title="Alerts and MFA" icon="bell" href="/api/alerts-mfa">
  Handle JavaScript dialogs and generate TOTP codes.
</Card>

<Card title="Tours, charts, recorder and utilities" icon="map" href="/api/tours-recorder">
  Tours, presentations, charts, the recorder and other helpers.
</Card>

<Card title="Capability traits" icon="layer-group" href="/api/traits">
  BrowserApi, ElementApi, AssertionApi and ScreenshotApi, shared by both engines.
</Card>

<Card title="Public items of the crate root" icon="cube" href="/api/crate-root">
  What seleniumbase_rs re-exports at its root and what each item is for.
</Card>

</Columns>

## Common error handling pattern

Most `BaseCase` methods return `Result<T, SeleniumBaseError>`. Use `?` inside an
async function or the `sb_test!` macro:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

async fn login(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.open("https://example.com/login").await?;
    sb.type_text("#username", "alice").await?;
    sb.type_text("#password", "secret").await?;
    sb.click("#submit").await?;
    sb.assert_element_visible("#dashboard").await?;
    Ok(())
}
```

## Further reading

<Columns cols={2}>

<Card title="Selectors" icon="crosshairs" href="/tutorials/selectors" horizontal>

CSS, XPath, link text and shadow DOM: how a selector string is read.

</Card>

<Card title="Waits and Assertions" icon="clock" href="/tutorials/waits_assertions" horizontal>

Wait for the page, then check it, without sleeps.

</Card>

<Card title="CDP Mode" icon="bolt" href="/tutorials/cdp_mode" horizontal>

The CDP helpers on `BaseCase`, raw commands and the network reactor.

</Card>

<Card title="UC Mode" icon="user-secret" href="/tutorials/uc_mode" horizontal>

Run Chrome so sites see a person's browser.

</Card>

<Card title="Macros" icon="wand-magic-sparkles" href="/tutorials/macros" horizontal>

Convenience macros from the crate root.

</Card>

<Card title="GUI Automation" icon="computer-mouse" href="/tutorials/gui_automation" horizontal>

Native OS input instead of WebDriver.

</Card>

</Columns>

For exact signatures, run `cargo doc --no-deps --open`.

---

<Callout icon="clipboard-check" color="#10B981">

Checked against the source of this branch: the argument lists and argument order of the `BaseCase` tables (names and counts, by script), the capability traits and the crate-root items. The multi-driver rows (`get_new_driver` to `driver_count`) were checked earlier, against commit 8e9180f. The one-line descriptions of the other `BaseCase` methods were not all re-read.

</Callout>
