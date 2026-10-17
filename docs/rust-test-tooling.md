---
title: "Writing Browser Tests"
---

# Writing Browser Tests

`seleniumbase-rs` does not ship its own test runner. Instead, it integrates with
standard Rust tooling: `#[tokio::test]`, `cargo test`, `cargo nextest run`, and
the `run_browser_test` lifecycle helper. This keeps tests idiomatic and lets you
use the full Rust testing ecosystem.

This page maps common testing concepts from Python and JavaScript to Rust, shows
the recommended test shape, and explains how to organize a growing suite.

## Concept mapping

| Python or JavaScript concept | Rust equivalent |
|------------------------------|-----------------|
| `pytest` async plugin | `#[tokio::test]` |
| SeleniumBase `setUp` / `tearDown` | `run_browser_test` |
| Test selection by name | `cargo test name` or nextest filters |
| Parallel test execution | `cargo nextest run` |
| Feature-specific suites | Cargo features |
| Environment configuration | `BrowserConfig`, `sbase_config.toml`, `SB_*` env vars |
| Parameterized test cases | A loop, helper function, or parameterized-test crate |

## Recommended test shape

Use `run_browser_test` to guarantee cleanup. It creates a `BaseCase`, runs your
async test body, and then calls `quit` even when the body fails:

```rust
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

#[tokio::test]
async fn user_can_open_the_home_page() -> Result<()> {
    run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_element("h1").await?;
        sb.assert_text("h1", "Example Domain").await
    })
    .await
}
```

The test is an async closure, `async |sb| { .. }` (or an `async fn` that takes
`&mut BaseCase`). It receives `&mut BaseCase` and returns `Result<()>`, that is
`Result<(), SeleniumBaseError>`, so `?` converts any error with a `From` impl
into it. The browser is closed when the test panics too, and the panic is then
raised again. Code written for older versions used `|sb| Box::pin(async move
{ .. })`; drop the `Box::pin` and write `async |sb| { .. }`. To test a page
with the Pure CDP engine instead, use `run_cdp_test(LaunchOptions::default(),
async |page| { .. })`.

Run the test:

```bash
cargo test user_can_open_the_home_page
```

## Running the full suite

For small suites, the default test runner is fine. For larger suites, install and
use [cargo-nextest](https://nexte.st/):

<CodeGroup>

```bash cargo test
cargo test
```

```bash cargo nextest
cargo install cargo-nextest
cargo nextest run
```

</CodeGroup>

<Note>

Nextest runs each test as a separate process, which isolates browser failures
and makes parallel execution safer. The repository profile intentionally does
not retry failed tests, so flaky behavior is not hidden.

</Note>

## Sharing helpers

As your suite grows, extract reusable helpers into modules or crates:

<View title="BaseCase (WebDriver)">

```rust
// tests/helpers/mod.rs
use seleniumbase_rs::{BaseCase, Result};

pub async fn login(sb: &mut BaseCase, user: &str, pass: &str) -> Result<()> {
    sb.open("https://example.com/login").await?;
    sb.type_text("#username", user).await?;
    sb.type_text("#password", pass).await?;
    sb.click("#submit").await?;
    sb.assert_element_visible("#dashboard").await
}
```

`BaseCase::find_element` is still there when you want the `thirtyfour::WebElement`
handle.

</View>

<View title="sb_cdp (pure CDP)">

Use the capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`,
`ScreenshotApi`) when you want helpers to work with any type that exposes the
same behavior:

```rust
use seleniumbase_rs::ElementApi;

async fn fill_email<E: ElementApi>(sb: &mut E, email: &str) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.type_text("#email", email).await
}
```

Both `BaseCase` (WebDriver) and `sb_cdp::Page` (Pure CDP) implement the traits,
so the same helper runs on either. `ElementApi` no longer has `find_element`,
because its `thirtyfour::WebElement` return type tied the trait to WebDriver.

</View>

## Parameterized tests

Rust does not have built-in parameterized tests, but you can achieve the same
result with a loop inside a single test or with a parameterized-test crate:

```rust
#[tokio::test]
async fn login_with_multiple_users() -> Result<()> {
    let users = [("alice", "secret1"), ("bob", "secret2")];
    for (user, pass) in users {
        run_browser_test(BrowserConfig::default(), async |sb| {
            sb.open("https://example.com/login").await?;
            sb.type_text("#username", user).await?;
            sb.type_text("#password", pass).await?;
            sb.click("#submit").await?;
            sb.assert_element_visible("#dashboard").await
        })
        .await?;
    }
    Ok(())
}
```

## Test organization

A typical project layout looks like this:

<Tree>
<Tree.Folder name="my_project" defaultOpen>
<Tree.File name="Cargo.toml" />
<Tree.File name="sbase_config.toml" />
<Tree.Folder name="src">
<Tree.File name="main.rs" />
</Tree.Folder>
<Tree.Folder name="tests" defaultOpen>
<Tree.Folder name="helpers">
<Tree.File name="mod.rs" />
</Tree.Folder>
<Tree.File name="login.rs" />
<Tree.File name="checkout.rs" />
</Tree.Folder>
</Tree.Folder>
</Tree>

Integration tests in `tests/` are compiled as separate binaries, which gives
each test its own browser process and reduces shared-state bugs.

## Configuration per test

For tests that need special configuration, build a `BrowserConfig` directly:

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode, run_browser_test, Result};

#[tokio::test]
async fn uc_smoke_test() -> Result<()> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(true);

    run_browser_test(config, async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await
    })
    .await
}
```

For global defaults, use `sbase_config.toml` or `SB_*` environment variables.
See [Settings and Configuration](tutorials/settings_and_config.md) for details.

## Macros for shorter tests

The `sb_test!` macro generates a `#[tokio::test]` wrapper and the
`run_browser_test` boilerplate:

```rust
use seleniumbase_rs::{sb_test, BrowserConfig};

sb_test!(home_page_loads, BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_text("h1", "Example Domain").await
});
```

This expands to the same shape as the manual example above. It is optional but
useful for linear tests where the reduced noise improves readability.

See [Macros](tutorials/macros.md) for the full macro catalog.

## Common pitfalls

<AccordionGroup>

<Accordion title="Using the old Box::pin closure">

`run_browser_test` takes an async closure now, so `|sb| Box::pin(async move { .. })`
does not compile ("implementation of `AsyncFnOnce` is not general enough"). Write
`async |sb| { .. }`, or call `run_browser_test_boxed` while you migrate.

</Accordion>

<Accordion title="Mixing sync and async cleanup">

`BaseCase::quit` is async. Do not rely on `Drop` for cleanup; use
`run_browser_test` or call `quit` explicitly.

</Accordion>

<Accordion title="Heavy setup in every test">

If every test logs in, consider a helper or a shared session strategy, but be
aware that shared browsers can leak state.

</Accordion>

<Accordion title="Too much parallelism">

Browsers are expensive. Start with low concurrency and increase only after
measuring.

</Accordion>

</AccordionGroup>

## Testing checklist

- [ ] Tests use `run_browser_test` or `sb_test!` for automatic cleanup.
- [ ] Helpers are extracted into `tests/helpers/` or a shared crate.
- [ ] Configuration is externalized for environment differences.
- [ ] The suite is run with `cargo test` or `cargo nextest run`.
- [ ] Tests capture screenshots or page source on failure.

