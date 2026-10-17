---
title: "Tracing and Logging"
---

# Tracing and Logging

`seleniumbase-rs` uses the `tracing` ecosystem for structured logging. Both the
`sbrs` CLI and the `sbrs-mcp` server initialize a default
`tracing_subscriber::fmt()` subscriber on startup. This page explains how to
control verbosity, configure runtime settings through the environment, and use
tracing in your own tests.

## What you will learn

- How to set log levels with `RUST_LOG`.
- How `SB_*` environment variables configure runtime behavior.
- How spans and structured errors help diagnose failures.
- How to use tracing in your own async code.

## Log levels

Set the `RUST_LOG` environment variable to control verbosity:

```bash
RUST_LOG=info cargo run --bin sbrs -- open https://example.com
RUST_LOG=seleniumbase_rs=debug cargo test
RUST_LOG=warn cargo run --bin sbrs-mcp
```

`RUST_LOG` follows the standard `env_logger`/`tracing-subscriber` syntax:

```bash
# Show only errors from everything and debug from this crate.
RUST_LOG=error,seleniumbase_rs=debug cargo test
```

## Runtime configuration (`SB_*` environment variables)

Following [Twelve-Factor III (config in the environment)](https://12factor.net/config),
`sbrs` and the library read runtime settings from `SB_*` variables:

| Variable | Default | Description |
|---|---|---|
| `SB_WEBDRIVER_URL` | `http://localhost:4444` | WebDriver endpoint. |
| `SB_CHROME_BIN` | auto-detect | Explicit Chrome/Chromium binary path. |
| `SB_PATCH_CACHE_DIR` | platform cache dir | Directory for patched Chrome/Chromedriver copies. |
| `SB_LOG_LEVEL` | unset | The tracing filter (`debug`, `seleniumbase_rs=debug,info`). Unset, blank or invalid, `sbrs` and `sbrs-mcp` use `RUST_LOG`, then `info`. |
| `SB_LOG_FORMAT` | `pretty` | `pretty` or `json` output. |
| `SB_SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout. |
| `SB_CHROMEDRIVER_PORT` | `0` | Parsed into `RuntimeConfig::chromedriver_port`; nothing in the crate reads it yet. |
| `SB_IMPLICIT_WAIT_SECS` | `30` | Parsed into `RuntimeConfig::implicit_wait`; nothing in the crate reads it yet. |

```bash
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

* `error.category` — stable tag such as `element_not_found` or `browser_launch`.
* `error.transient` — whether the failure is retryable.
* `error.hint` — a short remediation suggestion.

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

When something hangs, [tokio-console](https://github.com/tokio-rs/console) shows
every async task live: which ones are waiting, for how long, and on what.

<Steps>

<Step title="Run with the instrumentation">

Build with the `tokio-console` feature and tokio's unstable instrumentation (both are
for debugging only and make the program bigger):

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tokio-console --bin sbrs -- --headless smoke https://example.com
```

</Step>

<Step title="Start the console">

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

| Symptom | Likely cause | Fix |
|---|---|---|
| No log output | Subscriber not initialized | Run through `sbrs`, `sbrs-mcp`, or call `tracing_subscriber::fmt::init()` in your `main()`. |
| Too much noise from dependencies | `RUST_LOG=debug` includes all crates | Scope to `seleniumbase_rs=debug`. |
| JSON format not working | `SB_LOG_FORMAT=json` unsupported by default subscriber | Use a custom subscriber or enable the `full-tracing` feature. |
| Shutdown timeout errors | `SB_SHUTDOWN_TIMEOUT_SECS` too low | Increase it for slow CI agents. |
