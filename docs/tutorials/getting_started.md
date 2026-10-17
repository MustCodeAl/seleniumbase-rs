---
title: "Getting Started with SeleniumBase for Rust"
---

# Getting Started with SeleniumBase for Rust

This guide walks you through your first browser automation test with
`seleniumbase-rs`. By the end you will have a working Rust project that opens a
browser, interacts with a page, asserts on the result, and cleans up.

## What you will learn

- How to create a new Rust project and add `seleniumbase-rs` as a dependency.
- How to configure a `BrowserConfig`.
- How to create a `BaseCase`, navigate, interact, assert, and quit.
- How to run the test and interpret common startup errors.

## Prerequisites

- Rust toolchain 1.99 or newer (the crate's `rust-version`).
- A Chromium-based browser such as Google Chrome, Chromium, or Microsoft Edge.
- A network connection for the first build (dependencies may need to be fetched).

<Info>

The crate can connect to an existing WebDriver endpoint or auto-start
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
- `sb.assert_text_visible(text, selector).await?` checks at once and fails if the
  text is not visible; it does not wait. On a page that renders late, call
  `sb.wait_for_text(selector, text, timeout_secs).await?` first.
- `sb.quit().await?` closes the browser and ends the session.

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

    run_browser_test(config, |sb| {
        Box::pin(async move {
            sb.open("https://seleniumbase.io/simple/login").await?;
            sb.type_text("#username", "demo_user").await?;
            sb.type_text("#password", "secret_pass").await?;
            sb.click("#log-in").await?;
            sb.assert_text_visible("Welcome!", "body").await
        })
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
| `chromedriver` not found | The driver executable is missing. | Install it with `cargo run --bin sbrs -- install` or download it from the Chrome for Testing dashboard. |
| Chrome not found | The browser binary path is wrong. | Use `BrowserConfig::with_browser_binary_path`, or build the config with `BrowserConfig::from_env()` and set `SB_CHROME_BIN` (`BrowserConfig::default()` reads no variables). |
| Headless detected | Site blocks headless Chrome (headless sessions already use `--headless=new`). | Use `with_headless(false)`. |

## Next steps

- Learn about selectors in the [Selectors Guide](./selectors.md).
- Understand waits and assertions in [Waits and Assertions](./waits_assertions.md).
- Explore anti-detection in [Undetected (UC) Mode](./uc_mode.md).
- Slow a run down so you can watch it, or set the run-wide knobs, in
  [Run Settings](./run_settings.md).
- Browse the [API Reference](./api_reference.md) for the full method list.
- Use the [CLI helper](./cli_usage.md) for quick one-off commands.
