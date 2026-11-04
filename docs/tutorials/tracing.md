---
title: "Tracing and Logging"
sidebarTitle: "Tracing and logs"
description: "Log levels, SB_* variables and structured errors, and how to stop a test in a prompt with --pdb, --trace, --final-debug and sb.breakpoint."
---

`seleniumbase-rs` uses the `tracing` ecosystem for structured logging. Both the
`sbrs` CLI and the `sbrs-mcp` server initialize a default
`tracing_subscriber::fmt()` subscriber on startup. This page explains how to
control verbosity, configure runtime settings through the environment, use
tracing in your own tests, and stop a test in a debug prompt with the browser
open.

<Info>

**What you will learn**

- How to set log levels with `RUST_LOG`.
- What a log line looks like, and when it has colour.
- How `SB_*` environment variables configure runtime behavior.
- How spans and structured errors help diagnose failures.
- How to use tracing in your own async code.
- How to stop a test after it failed, before it starts, or after it ends, and how to pause at a line of your own with `sb.breakpoint`.

</Info>

<Visibility for="agents">

Summary for AI exports: control verbosity with `RUST_LOG`, for example `RUST_LOG=seleniumbase_rs=debug cargo test`. `sbrs` and `sbrs-mcp` also read `SB_LOG_LEVEL` and `SB_LOG_FORMAT` (`pretty` or `json`); without a valid `SB_LOG_LEVEL` they use `RUST_LOG`, then `info`. In your own program call `init_tracing()`. On a terminal a log line is `12:34:56  INFO module: message key=value` with a coloured level and dim context; the span context (`new:connect:`), the crate name and the milliseconds show only when `SB_LOG_LEVEL` or `RUST_LOG` chose a filter. There is no colour when standard error is not a terminal, with `NO_COLOR` set, with `TERM=dumb` or with `SB_LOG_FORMAT=json`; `seleniumbase_rs::common::color` holds the decision and the palette. Error events carry `error.category`, `error.transient` and `error.hint`; `ResultExt::log_err` logs an error and passes it through. `full-tracing` adds timing histograms. Debug modes: `sbrs test --pdb` stops in a `[POST MORTEM]` prompt after the last attempt of a failed test, `--trace` in a `[TRACE]` prompt before the body, `--final-debug` (also `--ftrace`, `--fdebug`, `--final-trace`) in a `[FINAL TRACE]` prompt after each test, all with the browser open. `sb.breakpoint("message")` (on `Page`: `page.breakpoint("message")`) pauses at that line. Type `c` and Enter to go on, `q` to stop the test. Any of the three flags makes `sbrs test` run `cargo test -- --test-threads 1 --nocapture` and refuses `--runner nextest` with status 2. Where standard input is not a terminal (CI, a pipe, nextest) a prompt is skipped with a warning in the log; `SB_BREAKPOINTS=0` skips `breakpoint` calls silently.

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

`pretty` (a readable, coloured line on a terminal) or `json` (one object per line, never coloured). See [What a log line looks like](#what-a-log-line-looks-like).

</ParamField>

<ParamField body="NO_COLOR" type="any value">

Turns colour off for log lines and banners. An empty value does not count. Colour is also off when the stream is not a terminal and when `TERM` is `dumb`.

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

## What a log line looks like

On a terminal, an event is one line, in the spirit of Python SeleniumBase's console
output:

```text
12:34:56  INFO api::base_case: creating BaseCase browser=Chrome headless=true
12:34:58  INFO browser::driver_access: repaired a driver the system would not start path=downloaded_drivers/chromedriver
12:35:01  WARN sbrs: ignored an invalid SB_* variable, its default is used problem=invalid configuration: SB_SHUTDOWN_TIMEOUT_SECS: invalid digit found in string
```

- The time, the module path (without the crate name) and the field names are dim.
- The level has its own colour: red `ERROR`, yellow `WARN`, green `INFO`, blue `DEBUG`,
  magenta `TRACE`.
- The values of file and folder fields (`path`, `dest`, `source`, `file`, `dir`, and
  names ending in `_path`, `_file` or `_dir`) are cyan; the values of `test`, `step`,
  `scenario` and `action` are bold magenta.
- Text that came from outside, such as a page title in a message, has its control
  characters replaced, so a page cannot send an escape sequence to your terminal.

### Quiet by default, detailed when you ask

By default a line leaves out what makes a terminal noisy: the span context that
`#[instrument]` puts in front of every line logged inside a function (`new:connect:`),
the crate name in front of the module path and the fraction of the second.
Choosing a filter, with `SB_LOG_LEVEL` or `RUST_LOG`, is asking for detail, and the
line shows all of it:

```text
12:35:01.482  INFO new:connect: seleniumbase_rs::browser::session: browser session connected
```

```bash Terminal
SB_LOG_LEVEL=info sbrs --headless smoke https://example.com
```

### When there is no colour

Colour is used only when standard error is a terminal. The same line, with no escape
sequence, is what you get:

- when standard error is a pipe or a file, as in `sbrs ... 2> run.log` or in CI;
- when `NO_COLOR` is set to anything but an empty value ([no-color.org](https://no-color.org));
- when `TERM` is `dumb`;
- with `SB_LOG_FORMAT=json`, which writes one JSON object per line and never colours.

The banners that `sbrs mkrec` prints on standard output (`* RECORDING initialized:`,
`>>> RECORDING SAVED as:`, `*** RECORDING COPIED to:`) follow the same rule for
standard output and use the colours of `sbase mkrec`. The
[debug prompts](/rust-test-tooling#debug-prompts) (`--pdb`, `--trace`, `breakpoint`) follow the
same rule for standard error. In your own code, the module
`seleniumbase_rs::common::color` holds the decision (`use_color`) and the palette
(`Style`, `palette`) that the log lines and the banners use.


## Spans in the library

Key `BaseCase` and `BrowserSession` methods are instrumented with `tracing`
spans so you can see where time is spent:

- `BaseCase::new`
- `BaseCase::with_session`
- `BaseCase::open`, `click`, `type_text`, `quit`
- `BrowserSession::connect`

Example output:

With a filter chosen (`SB_LOG_LEVEL=info`) the span context is shown in front of each line:

```text
12:34:56.101  INFO new: seleniumbase_rs::api::base_case: creating BaseCase browser=Chrome headless=true
12:34:56.934  INFO new:connect: seleniumbase_rs::browser::session: browser session connected
12:34:56.950  INFO new:open{url="https://example.com"}: seleniumbase_rs::api::base_case: opening url url=https://example.com
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

The [Tauri profile manager](https://github.com/MustCodeAl/tauri-plugin-sbrs-automation/tree/main/examples/tauri-profile-manager)
(an example of tauri-plugin-sbrs-automation) also initializes `tracing_subscriber`
in `src-tauri/src/lib.rs`.
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
ERROR seleniumbase_rs::evasion::patcher: binary patch failed for 'chromedriver': failed to read chromedriver: No such file error.category=patcher error.transient=false error.hint="Ensure 'chromedriver' is a valid chromedriver binary and that you have write permissions."
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

## Debug modes

Python SeleniumBase drops into `pdbp` for `--pdb`, `--trace`, `--final-debug` and
`breakpoint()`. A Rust test has no `pdb`, so the library gives the same four
stops a prompt of its own: the test pauses with the browser open, you look at the
page, type a command, and the test goes on or stops. The prompts work in any
test that runs through the `sb` or `sb_cdp` fixture, and in Gherkin steps.

### Stop a test

Each stop is an option of `sbrs test`, an `SB_*` variable and a key of the
`[test]` table of the settings file. Pick the one that matches what you want to
see.

<Steps>

<Step title="After a failure: --pdb" icon="skull">

```bash Terminal
sbrs test checkout --pdb
```

When a test fails and its last attempt is over, the test pauses in a
`[POST MORTEM]` prompt with the browser still open and the evidence already
written to `latest_logs/<test id>/`. The prompt shows the code of the test body,
marks the lines that name the failing selector or address with `>>`, and says
what failed. `r` runs the test again with a new browser, once more than
`--reruns` allows; `c` or `q` ends it as failed.

`SB_PDB`, key `pdb`. Python: pytest's `--pdb`.

</Step>

<Step title="Before the body: --trace" icon="flag">

```bash Terminal
sbrs test checkout --trace
```

Each test pauses in a `[TRACE]` prompt before its body runs: the browser is
open and nothing has been done yet. Use it to look at the page the browser
starts on, or to set it up by hand before the test begins.

`SB_TRACE`, key `trace`. Python: pytest's `--trace`.

</Step>

<Step title="After every test: --final-debug" icon="flag-checkered">

```bash Terminal
sbrs test checkout --final-debug
```

After a test ends, pass or fail, it pauses in a `[FINAL TRACE]` prompt before
the browser quits. With reruns it happens after each attempt. `--ftrace`,
`--fdebug` and `--final-trace` are the same flag.

`SB_FINAL_DEBUG`, key `final_debug`. Python: `--final-trace` and `--fdebug`.

</Step>

<Step title="At a line of your own: sb.breakpoint" icon="pause">

```rust tests/checkout.rs
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn checkout(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/cart").await?;
        sb.breakpoint("Paused at the cart.").await?;
        sb.click("#checkout").await
    })
    .await
}
```

The test pauses in a `[BREAKPOINT]` prompt at that call, and the listing marks
the line with `->`. On the pure CDP engine the call is
`page.breakpoint("Paused at the cart.").await?` inside `sb_cdp.run(async |page| ..)`.
It needs no flag: it pauses whenever the test runs in a terminal, and
`SB_BREAKPOINTS=0` (also `false`, `no` or `off`) turns every `breakpoint` call
off, as Python's `PYTHONBREAKPOINT=0` does.

</Step>

</Steps>

### At the prompt

```text
[BREAKPOINT]: Paused at the cart.
  test:  checkout.checkout   page: https://example.com/cart  "Cart"
Commands: c continue | s screenshot | src page source | u url | js <expr> | l list | w where | dt devtools | q quit | h help
Type 'c' and press [ENTER] to continue execution:
```

Work in the browser as long as you like, then type `c` and press Enter. `q`
stops the test, which then fails with a lifecycle error, `stopped at the debug
prompt`. `s` and
`src` save a screenshot and the page source to `latest_logs/<test id>/`, `js
<expr>` evaluates an expression in the page, and `h` lists the rest. The full
table of commands, the listing and the colours are described in [Writing Tests
with the sb Fixture](/rust-test-tooling#debug-prompts).

The time you spend at a prompt counts neither against `--time-limit` nor against
the test's own limit, so a test is not stopped while you look around.

<Warning>

A prompt reads from standard input, so it needs a terminal and one test at a
time.

- **`sbrs test` sets the terminal up for you.** Any of `--pdb`, `--trace` and
  `--final-debug` makes it run `cargo test -- --test-threads 1 --nocapture`, even
  with `-n 4`. In a plain `cargo test`, add `-- --test-threads 1 --nocapture`
  yourself (`SB_PDB=1 cargo test checkout -- --test-threads 1 --nocapture`):
  parallel tests would take turns at the prompt, and the capture would hide it.
- **nextest gives tests no standard input.** `sbrs test --pdb --runner nextest`
  exits with status 2 (`--pdb needs the cargo runner: nextest gives tests no
  standard input`), and a prompt under a plain `cargo nextest run` is skipped.
- **CI and pipes.** Where standard input is not a terminal, a prompt is skipped
  with a warning in the log (`debug prompt skipped: standard input is not a
  terminal`) and the test goes on, so a flag or a `breakpoint` left in a command
  never hangs a run. `SB_BREAKPOINTS=0` skips the breakpoints without a warning.

</Warning>

### In a Gherkin scenario

`--pdb`, `--trace`, `--final-debug` and `breakpoint` work in the steps of a
scenario as in any other test: a failed step panics, and the post-mortem prompt
opens before the browser closes. See [Gherkin scenarios with
rstest-bdd](/tutorials/utilities#gherkin-scenarios-with-rstest-bdd).

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


## Next steps

<Columns cols={3}>

<Card color="#1a85d4" title="Test options" icon="sliders" horizontal href="/help/test_options">

Every flag of `sbrs test`.

</Card>

<Card color="#199f43" title="Writing Browser Tests" icon="flask" horizontal href="/rust-test-tooling">

The prompt commands, reruns and evidence.

</Card>

<Card color="#d47628" title="Recorder Mode" icon="circle-dot" horizontal href="/tutorials/recorder_mode">

Record a test by clicking through the page.

</Card>

</Columns>
