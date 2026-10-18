---
title: "Getting Started with SeleniumBase for Rust"
sidebarTitle: "Getting started"
---

This guide walks you through your first browser automation test with
`seleniumbase-rs`. By the end you will have a working Rust project that opens a
browser, interacts with a page, asserts on the result, and cleans up.

<Info>

**What you will learn**

- How to create a new Rust project and add `seleniumbase-rs` as a dependency.
- How to configure a `BrowserConfig`.
- How to create a `BaseCase`, navigate, interact, assert, and quit.
- How to run the test and interpret common startup errors.
- How to configure a session in code and through `SB_*` environment variables.
- A complete example that needs no network, and the same flow on the driverless CDP engine.

</Info>

<Visibility for="agents">

Summary for AI exports: add `seleniumbase-rs` and `tokio` to `Cargo.toml`, build a `BrowserConfig`, create `BaseCase::new(config).await?`, call `open`, `click`, `type_text` and the `assert_*` methods, then `quit`. Rust 1.99 or newer and a Chromium-based browser are required.

</Visibility>

## Prerequisites

- Rust toolchain 1.99 or newer (the crate's `rust-version`).
- A Chromium-based browser such as Google Chrome, Chromium, or Microsoft Edge.
- A network connection for the first build (dependencies may need to be fetched).

<Info>

The crate can connect to an existing <Tooltip tip="The Selenium protocol: a test sends commands to a driver such as chromedriver, which drives the browser.">WebDriver</Tooltip> endpoint or auto-start
`chromedriver` for you. With the default WebDriver URL it first looks for a
driver already listening on `http://localhost:4444`; if there is none, it starts
`downloaded_drivers/chromedriver` on a free port, downloading the latest stable
chromedriver into that folder first when it is missing (it does not look on your
`PATH`). `cargo run --bin sbrs -- install` fetches the driver ahead of time.

</Info>

## Create a new project

<Steps>

<Step title="Create the project">

```bash
cargo new my_first_test
cd my_first_test
```

</Step>

<Step title="Add the dependencies">

Open `Cargo.toml` and add the dependencies:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

<Note>

The `tokio` runtime is required because `BaseCase` methods are async.

</Note>

</Step>

</Steps>

## Write your first test

<Steps>

<Step title="Write the program">

Replace the contents of `src/main.rs` with the following program:

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure the browser.
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    // 2. Create the test case. This starts the browser and driver if needed.
    let mut sb = BaseCase::new(config).await?;

    // 3. Automate.
    sb.open("https://seleniumbase.io/simple/login").await?;
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;

    // 4. Assert.
    sb.assert_text_visible("Welcome!", "body").await?;

    // 5. Clean up.
    sb.quit().await?;
    Ok(())
}
```

</Step>

<Step title="Run it">

```bash
cargo run
```

<Check>

You should see the browser open, fill the login form, submit, verify the
welcome message, and close.

</Check>

</Step>

</Steps>

## How it works

- `BrowserConfig::default()` returns a default configuration pointing at
  `http://localhost:4444` with headless mode enabled.
- `with_mode(DriverMode::Uc)` enables Undetected Chromedriver mode, which
  applies anti-detection evasions.
- `with_headless(false)` runs the browser with a visible window so you can watch
  the test. In CI you usually want `true`.
- `BaseCase::new(config).await?` creates the browser session. If no WebDriver
  server is running, the crate attempts to start one.
- `sb.open(url).await?` navigates to the requested page.
- `sb.type_text(selector, text).await?` finds the element, clears it, and types.
- `sb.click(selector).await?` clicks the element.
- `sb.assert_text_visible(text, selector).await?` waits up to the default timeout
  (`sb.set_timeout(secs)`) for the text to be visible and fails if it never is,
  as Python's `assert_text` does. For a different wait, call
  `sb.wait_for_text(text, selector, timeout_secs).await?`.
- `sb.quit().await?` closes the browser and ends the session.

## Configuration

`BrowserConfig` is a plain struct with `with_*` builder methods. These are the
settings most programs touch (all are in `seleniumbase_rs::BrowserConfig`):

| Setting | Builder | Default |
|---|---|---|
| Browser | `with_browser(Browser::Edge)` | `Browser::Chrome` (also `Chromium`, `Firefox`, `Brave`, `Opera`, `Safari`, `ChromeForTesting`, ...) |
| Headless | `with_headless(bool)` | `true` |
| Engine mode | `with_mode(DriverMode)` | `DriverMode::WebDriver` (`Cdp` and `Uc` add DevTools helpers and anti-detection) |
| WebDriver endpoint | `with_webdriver_url(url)` | `http://localhost:4444` |
| Browser binary | `with_browser_binary_path(path)` | none (`from_env` reads `SB_CHROME_BIN`) |
| Proxy | `with_proxy(spec)` or `with_proxy_pac_url(url)` | none |
| User agent, locale | `with_user_agent(ua)`, `with_locale("de-DE")` | none |
| Profile and extensions | `with_user_data_dir(dir)`, `with_extension_dir(dir)` | none |
| Start page | `with_start_page(url)` | none |
| Ad blocking | `with_ad_block(bool)` | `false` |
| Reuse browsers between tests | `with_reuse_session(bool)` | `false` |
| Extra Chromium arguments | `with_extra_args([..])`, `push_extra_arg(arg)` | none |
| Fingerprint | `with_fingerprint(Fingerprint)` | none; see [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth) |

Two constructors build the starting point. `BrowserConfig::default()` uses the
defaults above and reads no environment variable. `BrowserConfig::from_env()`
returns a `Result` and reads the `SB_*` variables, which suits a deployed test
run because the same binary then needs no code change per environment:

| Variable | Meaning | Default |
|---|---|---|
| `SB_WEBDRIVER_URL` | WebDriver endpoint | `http://localhost:4444` |
| `SB_CHROME_BIN` | Browser binary path | system Chrome |
| `SB_PATCH_CACHE_DIR` | Where patched binaries are cached | the platform cache directory |
| `SB_LOG_LEVEL` | Tracing filter (used by `init_tracing_from_runtime`); unset, it is `RUST_LOG`, then `info` | unset |
| `SB_LOG_FORMAT` | `pretty` or `json` | `pretty` |
| `SB_SHUTDOWN_TIMEOUT_SECS` | Graceful shutdown timeout | `30` |
| `SB_CHROMEDRIVER_PORT` | Parsed, not yet used by the crate | `0` |
| `SB_IMPLICIT_WAIT_SECS` | Parsed, not yet used by the crate | `30` |
| `SB_REMOTE_WEBDRIVER_TIMEOUT` | Per-request client timeout for a remote grid, in seconds | unset |

A value that does not parse (a non-number, or a `SB_LOG_FORMAT` other than
`pretty` or `json`) makes `from_env` return `SeleniumBaseError::InvalidConfig`;
`from_env_lenient` keeps the good values and lists the bad ones.

## A complete example that needs no network

This program is the whole project: `Cargo.toml` from above and this
`src/main.rs`. It loads a page from a string, types, clicks, asserts, saves a
screenshot, and closes the browser even if an assertion fails
(`run_browser_test` always calls `quit`).

```rust,no_run
use seleniumbase_rs::{init_tracing, run_browser_test, BrowserConfig};

const PAGE: &str = r#"<!doctype html>
<title>Greeter</title>
<input id="name">
<button id="go"
  onclick="document.getElementById('out').textContent =
           'Hello, ' + document.getElementById('name').value">Go</button>
<p id="out"></p>"#;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    init_tracing(); // install the log subscriber; RUST_LOG=debug for more

    // SB_* variables from the environment, headless unless you change it.
    let config = BrowserConfig::from_env()?.with_headless(true);

    run_browser_test(config, async |sb| {
        sb.load_html_string(PAGE).await?;
        sb.type_text("#name", "Ferris").await?;
        sb.click("#go").await?;
        // The expected text comes first, the selector second, as in Python.
        sb.assert_exact_text("Hello, Ferris", "#out").await?;
        // Saved into the logs directory; the name must be a plain file name.
        sb.save_screenshot("greeter.png").await.map(|_| ())
    })
    .await
}
```

`cargo run` starts (or downloads, then starts) chromedriver, opens a headless
Chrome, and exits with status 0 when every step passes. On a failed assertion
the error is printed and the exit status is non-zero.

## The same page without a driver

The pure CDP engine (`seleniumbase_rs::sb_cdp`) talks to Chrome directly, so
there is no chromedriver to download or start. It uses the same `tokio`
dependency:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("seleniumbase.io/simple/login").await?; // https:// is added for you
    page.locator("#username").fill("demo_user").await?;
    page.locator("#password").fill("secret_pass").await?;
    page.locator("button").click().await?;
    page.locator("h1").expect().to_contain_text("Welcome").await?;

    browser.close().await?;
    Ok(())
}
```

`Page` also implements the capability traits (`BrowserApi`, `ElementApi`,
`AssertionApi`, `ScreenshotApi`), so a helper written over those traits runs on
both engines; see [Capability traits](/tutorials/api_reference#capability-traits).
The full engine is in [Pure CDP](/tutorials/pure_cdp).

## Writing a `#[tokio::test]` instead of `main`

For real test suites, use the test harness:

```rust,no_run
// tests/login.rs
use seleniumbase_rs::{run_browser_test, BrowserConfig, DriverMode, Result};

#[tokio::test]
async fn login_succeeds() -> Result<()> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    run_browser_test(config, async |sb| {
        sb.open("https://seleniumbase.io/simple/login").await?;
        sb.type_text("#username", "demo_user").await?;
        sb.type_text("#password", "secret_pass").await?;
        sb.click("#log-in").await?;
        sb.assert_text_visible("Welcome!", "body").await
    })
    .await
}
```

Run the test:

```bash
cargo test login_succeeds
```

<Tip>

`run_browser_test` guarantees that `quit` is called even when assertions fail.

</Tip>

## Common startup errors

| Problem | Cause | Solution |
|---------|-------|----------|
| `WebDriver` connection error | No driver running, and auto-start does not apply: it needs `auto_start_driver` (on by default) and the default WebDriver URL `http://localhost:4444`. | Remove the custom URL, or start `chromedriver` yourself and pass its URL. |
| `chromedriver` not found | The driver executable is missing. | Install it with `sbrs install` or download it from the Chrome for Testing dashboard. |
| Chrome not found | The browser binary path is wrong. | Use `BrowserConfig::with_browser_binary_path`, or build the config with `BrowserConfig::from_env()` and set `SB_CHROME_BIN` (`BrowserConfig::default()` reads no variables). |
| Headless detected | Site blocks headless Chrome (headless sessions already use `--headless=new`). | Use `with_headless(false)`. |

## Next steps

<CardGroup cols={2}>
  <Card title="Selectors" icon="crosshairs" href="/tutorials/selectors">
    CSS, XPath, link text and shadow DOM: how a selector string is read.
  </Card>
  <Card title="Waits and assertions" icon="clock" href="/tutorials/waits_assertions">
    Wait for the page, then check it, without sleeps.
  </Card>
  <Card title="Undetected (UC) mode" icon="user-secret" href="/tutorials/uc_mode">
    Run Chrome so sites see a person's browser.
  </Card>
  <Card title="Run settings" icon="sliders" href="/tutorials/run_settings">
    Slow a run down to watch it, and the other run-wide switches.
  </Card>
  <Card title="API reference" icon="code" href="/tutorials/api_reference">
    Every `BaseCase` method, the capability traits and the crate root.
  </Card>
  <Card title="Command line" icon="terminal" href="/tutorials/cli_usage">
    One-off commands with `sbrs`, no project needed.
  </Card>
</CardGroup>
