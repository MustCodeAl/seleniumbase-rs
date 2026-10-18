---
title: "Tracing and Logging"
sidebarTitle: "Tracing and logs"
---

`seleniumbase-rs` uses the `tracing` ecosystem for structured logging. Both the
`sbrs` CLI and the `sbrs-mcp` server initialize a default
`tracing_subscriber::fmt()` subscriber on startup. This page explains how to
control verbosity, configure runtime settings through the environment, and use
tracing in your own tests.

<Info>

**What you will learn**

- How to set log levels with `RUST_LOG`.
- How `SB_*` environment variables configure runtime behavior.
- How spans and structured errors help diagnose failures.
- How to use tracing in your own async code.

</Info>

<Visibility for="agents">

Summary for AI exports: control verbosity with `RUST_LOG`, for example `RUST_LOG=seleniumbase_rs=debug cargo test`. `sbrs` and `sbrs-mcp` also read `SB_LOG_LEVEL` and `SB_LOG_FORMAT` (`pretty` or `json`); without a valid `SB_LOG_LEVEL` they use `RUST_LOG`, then `info`. In your own program call `init_tracing()`. Error events carry `error.category`, `error.transient` and `error.hint`; `ResultExt::log_err` logs an error and passes it through. `full-tracing` adds timing histograms.

</Visibility>

## Log levels

Set the `RUST_LOG` environment variable to control verbosity:

```bash Terminal
RUST_LOG=info cargo run --bin sbrs -- open https://example.com
RUST_LOG=seleniumbase_rs=debug cargo test
RUST_LOG=warn cargo run --bin sbrs-mcp
```

`RUST_LOG` follows the standard `env_logger`/`tracing-subscriber` syntax:

```bash Terminal
# Show only errors from everything and debug from this crate.
RUST_LOG=error,seleniumbase_rs=debug cargo test
```

## Runtime configuration (`SB_*` environment variables)

Following [Twelve-Factor III (config in the environment)](https://12factor.net/config),
`sbrs` and the library read runtime settings from `SB_*` variables:

<ParamField body="SB_WEBDRIVER_URL" type="string" default="http://localhost:4444">

WebDriver endpoint.

</ParamField>

<ParamField body="SB_CHROME_BIN" type="string" default="auto-detect">

Explicit Chrome/Chromium binary path.

</ParamField>

<ParamField body="SB_PATCH_CACHE_DIR" type="string" default="platform cache dir">

Directory for patched Chrome/Chromedriver copies.

</ParamField>

<ParamField body="SB_LOG_LEVEL" type="string">

The tracing filter (`debug`, `seleniumbase_rs=debug,info`). Unset, blank or invalid, `sbrs` and `sbrs-mcp` use `RUST_LOG`, then `info`.

</ParamField>

<ParamField body="SB_LOG_FORMAT" type="string" default="pretty">

`pretty` or `json` output.

</ParamField>

<ParamField body="SB_SHUTDOWN_TIMEOUT_SECS" type="integer" default="30">

Graceful shutdown timeout.

</ParamField>

<ParamField body="SB_CHROMEDRIVER_PORT" type="integer" default="0">

<Badge color="gray">Not read yet</Badge>

Parsed into `RuntimeConfig::chromedriver_port`; nothing in the crate reads it yet.

</ParamField>

<ParamField body="SB_IMPLICIT_WAIT_SECS" type="integer" default="30">

<Badge color="gray">Not read yet</Badge>

Parsed into `RuntimeConfig::implicit_wait`; nothing in the crate reads it yet.

</ParamField>

```bash Terminal
SB_WEBDRIVER_URL=http://localhost:9515 SB_LOG_LEVEL=debug sbrs open https://example.com
```

One bad value does not undo the others: with `SB_SHUTDOWN_TIMEOUT_SECS=soon`
`sbrs` still honours `SB_LOG_LEVEL=debug`, uses the default timeout and logs a
warning that names the variable. A program that builds the configuration itself
can do the same with `RuntimeConfig::from_env_lenient()`, which returns the
configuration and the list of problems.

## Spans in the library

Key `BaseCase` and `BrowserSession` methods are instrumented with `tracing`
spans so you can see where time is spent:

- `BaseCase::new`
- `BaseCase::with_session`
- `BaseCase::open`, `click`, `type_text`, `quit`
- `BrowserSession::connect`

Example output:

```text
INFO  seleniumbase_rs::api::base_case: new basecase browser=Chrome mode=Uc
DEBUG seleniumbase_rs::api::base_case: opening url=https://example.com
INFO  seleniumbase_rs::api::base_case: quit
```

## Using tracing in your own tests

```rust
use tracing::{info, instrument};

#[instrument]
async fn login_flow(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting login flow");
    sb.open("https://example.com/login").await?;
    // ...
    Ok(())
}
```

## Tauri multi-profile example

The Tauri app also initializes `tracing_subscriber` in `src-tauri/src/lib.rs`.
Its REST API logs each endpoint call so you can correlate UI actions with local
HTTP requests.

## Error diagnostics

Errors are emitted as structured `tracing::error!` events. Each event includes:

<ResponseField name="error.category" type="string">

Stable tag such as `element_not_found` or `browser_launch`.

</ResponseField>

<ResponseField name="error.transient" type="boolean">

Whether the failure is retryable.

</ResponseField>

<ResponseField name="error.hint" type="string">

A short remediation suggestion.

</ResponseField>

Example output:

```text
ERROR seleniumbase_rs::stealth::patcher: binary patch failed for 'chromedriver': failed to read chromedriver: No such file error.category=patcher error.transient=false error.hint="Ensure 'chromedriver' is a valid chromedriver binary and that you have write permissions."
```

You can also log errors manually:

```rust
use seleniumbase_rs::{Result, ResultExt};

async fn example() -> Result<()> {
    // Log on error but still return the original error:
    do_something().await.log_err()?;

    // Or attach context and log:
    do_something_else()
        .sb_context("while loading profile")
        .log_err()?;
    Ok(())
}
```

<Tip>

Enable the `full-tracing` feature for additional timing histograms and Actix
actor instrumentation.

</Tip>

## Watching async tasks with tokio-console

<Badge color="blue">Feature: tokio-console</Badge>

When something hangs, [tokio-console](https://github.com/tokio-rs/console) shows
every async task live: which ones are waiting, for how long, and on what.

<Steps>

<Step title="Run with the instrumentation" icon="play">

Build with the `tokio-console` feature and tokio's unstable instrumentation (both are
for debugging only and make the program bigger):

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tokio-console --bin sbrs -- --headless smoke https://example.com
```

</Step>

<Step title="Start the console" icon="terminal">

In another terminal:

```bash
cargo install --locked tokio-console
tokio-console
```

</Step>

</Steps>

It connects to `127.0.0.1:6669`. Your own program gets the same when it calls
`seleniumbase_rs::tracing_util::init_tracing()` with the feature on.

## Troubleshooting

<AccordionGroup>

<Accordion title="No log output" icon="eye-slash">

**Likely cause:** Subscriber not initialized.

**Fix:** Run through `sbrs`, `sbrs-mcp`, or call `tracing_subscriber::fmt::init()` in your `main()`.

</Accordion>

<Accordion title="Too much noise from dependencies" icon="volume-high">

**Likely cause:** `RUST_LOG=debug` includes all crates.

**Fix:** Scope to `seleniumbase_rs=debug`.

</Accordion>

<Accordion title="JSON format not working" icon="code">

**Likely cause:** `SB_LOG_FORMAT=json` unsupported by default subscriber.

**Fix:** Use a custom subscriber or enable the `full-tracing` feature.

</Accordion>

<Accordion title="Shutdown timeout errors" icon="hourglass-half">

**Likely cause:** `SB_SHUTDOWN_TIMEOUT_SECS` too low.

**Fix:** Increase it for slow CI agents.

</Accordion>

</AccordionGroup>
