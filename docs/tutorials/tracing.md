# Tracing and Logging

`seleniumbase-rs` uses the `tracing` ecosystem for structured logging. Both the
`sbase` CLI and the `seleniumbase-mcp` server initialize a default
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
RUST_LOG=info cargo run --bin sbase -- open https://example.com
RUST_LOG=seleniumbase_rs=debug cargo test
RUST_LOG=warn cargo run --bin seleniumbase-mcp
```

`RUST_LOG` follows the standard `env_logger`/`tracing-subscriber` syntax:

```bash
# Show only errors from everything and debug from this crate.
RUST_LOG=error,seleniumbase_rs=debug cargo test
```

## Runtime configuration (`SB_*` environment variables)

Following [Twelve-Factor III (config in the environment)](https://12factor.net/config),
`sbase` and the library read runtime settings from `SB_*` variables:

| Variable | Default | Description |
|---|---|---|
| `SB_WEBDRIVER_URL` | `http://localhost:4444` | WebDriver endpoint. |
| `SB_CHROME_BIN` | auto-detect | Explicit Chrome/Chromium binary path. |
| `SB_PATCH_CACHE_DIR` | platform cache dir | Directory for patched Chrome/Chromedriver copies. |
| `SB_LOG_LEVEL` | `info` | Default tracing log level. |
| `SB_LOG_FORMAT` | `pretty` | `pretty` or `json` output. |
| `SB_SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout. |
| `SB_CHROMEDRIVER_PORT` | `0` | Port for auto-started chromedriver (`0` = ephemeral). |
| `SB_IMPLICIT_WAIT_SECS` | `30` | Default implicit wait timeout. |

```bash
SB_WEBDRIVER_URL=http://localhost:9515 SB_LOG_LEVEL=debug sbase open https://example.com
```

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

Enable the `full-tracing` feature for additional timing histograms and Actix
actor instrumentation.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| No log output | Subscriber not initialized | Run through `sbase`, `seleniumbase-mcp`, or call `tracing_subscriber::fmt::init()` in your `main()`. |
| Too much noise from dependencies | `RUST_LOG=debug` includes all crates | Scope to `seleniumbase_rs=debug`. |
| JSON format not working | `SB_LOG_FORMAT=json` unsupported by default subscriber | Use a custom subscriber or enable the `full-tracing` feature. |
| Shutdown timeout errors | `SB_SHUTDOWN_TIMEOUT_SECS` too low | Increase it for slow CI agents. |
