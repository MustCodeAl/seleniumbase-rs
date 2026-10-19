---
title: "Taking advantage of Rust"
sidebarTitle: "Taking advantage of Rust"
description: "How async, Result, traits, types and Cargo make browser tests faster to write and harder to break."
---

<Info>

**In short:** the same browser test in Rust gets errors you cannot forget, real
parallel browsers on one runtime, helpers that run on both engines, options the
compiler checks, and one native binary to ship. This page shows each with code
you can copy. [Why Rust?](/why-rust) explains the reasons; this page shows the
practice.

</Info>

## Errors you cannot forget

Every call that can fail returns `seleniumbase_rs::Result<T>`, so a failure is a
value, not an exception that slips past. End a call with `?` to stop the test
there, or handle the error where you want a fallback:

```rust,ignore tests/login.rs
use seleniumbase_rs::{BaseCase, BrowserConfig};

async fn sign_in(sb: &mut BaseCase) -> seleniumbase_rs::Result<()> {
    sb.open("https://seleniumbase.io/simple/login").await?;
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;

    // A fallback instead of a failure: dismiss a banner only if it is there.
    if let Err(error) = sb.click("#cookie-banner .accept").await {
        eprintln!("no cookie banner: {error}");
    }
    sb.assert_text_visible("Welcome!", "body").await
}
```

The compiler warns about a `Result` you ignore, so a check you forgot to await
or to propagate shows up at build time, not as a test that passed by accident.

## Many browsers at once

Tests and scrapers run on `tokio`, so several browsers work in parallel on one
runtime with no threads of your own. `BrowserPool` hands out pages and shares
browsers between tasks:

```rust,ignore src/main.rs
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};

#[tokio::main]
async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(2).contexts_per_browser(4).build()?,
    );

    let urls = ["https://example.com", "https://example.org", "https://example.net"];
    let tasks = urls.map(|url| {
        let pool = pool.clone(); // another handle to the same pool
        tokio::spawn(async move {
            let lease = pool.acquire().await?;
            lease.page().goto(url).await?;
            let title = lease.page().title().await?;
            lease.release().await;
            Ok::<_, seleniumbase_rs::SeleniumBaseError>(title)
        })
    });

    for task in tasks {
        println!("{}", task.await.expect("task panicked")?);
    }
    pool.close().await;
    Ok(())
}
```

A lease gives its browser back when you call `release`. The pool caps how many
browsers start and reuses them, so ten tasks do not open ten Chromes. See
[Pure CDP](/tutorials/pure_cdp).

## One helper for both engines

`BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` are traits that
both `BaseCase` (WebDriver) and `sb_cdp::Page` (pure CDP) implement. Write a
helper once against the traits and call it with either engine:

```rust,ignore src/helpers.rs
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi};

pub async fn search<B>(browser: &mut B, query: &str) -> seleniumbase_rs::Result<()>
where
    B: BrowserApi + ElementApi + AssertionApi,
{
    browser.open("https://example.com/search").await?;
    browser.type_text("input[name=q]", query).await?;
    browser.click("button[type=submit]").await?;
    browser.assert_element("#results").await
}
```

The bound says exactly what the helper needs, and the compiler refuses a type
that does not provide it. See [Capability traits](/api/traits).

## Options the compiler checks

Choices are enums, not strings, so a typo does not compile:

| Instead of | You write | A wrong value |
|---|---|---|
| `"uc"`, `"cdp"` | `DriverMode::Uc`, `DriverMode::Cdp` | does not compile |
| `"visible"` | `State::Visible`, `State::Hidden` | does not compile |
| `by="text"` | `SelectBy::Text("Large")`, `SelectBy::Index(2)` | does not compile |
| `"random"` | `Spoof::Random`, `Spoof::Value(..)`, `Spoof::Off` | does not compile |

Values that only make sense together are checked when you build them:
`LaunchOptions::builder()...build()?` and `Fingerprint::builder()...build()?`
return an error that names the problem (such as `Off` on a setting that cannot
be turned off) before any browser starts. See
[Design: fingerprint API v2](/design/fingerprint-api-v2).

## Tests are Cargo tests

A browser test is an ordinary `#[tokio::test]`, so `cargo test` runs it, filters
it by name and reports it with the rest. `sb_test!` writes the test for you and
always quits the browser, even when the test fails:

```rust,ignore tests/example.rs
use seleniumbase_rs::sb_test;

sb_test!(visit_example, seleniumbase_rs::BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_title("Example Domain").await?;
    Ok(())
});
```

```bash Terminal
cargo test visit_example            # one test
SB_HEADLESS=true cargo test         # every test, headless
```

See [Macros](/tutorials/macros) and [Writing browser tests](/rust-test-tooling).

## One binary to ship

`sbrs` and `sbrs-mcp` compile to single native binaries: no Python, no virtual
environment, no package manager on the target machine. Copy the binary into a
container or a CI image, or install it from the tag:

```bash Terminal
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs --tag v0.5.1 seleniumbase-rs-cli
```

See [Deployment](/deployment) and [Docker](/help/docker).

## Next steps

<Columns cols={2}>

<Card title="Why Rust?" icon="gauge-high" color="#1a85d4" href="/why-rust">
  Security, reliability and performance, with the numbers.
</Card>

<Card title="Quickstart" icon="rocket" color="#199f43" href="/quickstart">
  Your first test in five minutes.
</Card>

<Card title="API reference" icon="code" color="#d47628" href="/tutorials/api_reference">
  Every method, its arguments and what it returns.
</Card>

<Card title="Migrate Python tests" icon="right-left" color="#a631be" href="/python-migration">
  Convert SeleniumBase tests to Rust with sbrs import-python.
</Card>

</Columns>
