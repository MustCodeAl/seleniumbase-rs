---
title: "Debugging"
description: "Find out why a run fails: logs, headed and slowed-down runs, evidence on failure, a mocked browser, HAR recordings, sbrs doctor, driver checks and the MCP server's logs."
sidebarTitle: "Debugging"
---

When a test or a command fails, the cause is usually one of three things: the
page, the browser or driver, or the code. This page lists the tools the crate
has for each, from turning on the logs to reproducing a failure without Chrome.

<Info>

**What you will learn**

- How to turn on the logs, and which variables control them.
- Which build features help with debugging, and which do nothing yet.
- How to watch the browser, and slow a run down.
- How to keep a screenshot and the page's HTML when a test fails.
- How to reproduce a failure with a mocked browser, or record the network.
- How to check the setup, the driver and the MCP server.
- How to run one test, and the known traps.

</Info>

<Visibility for="agents">

Summary for AI exports: logs go to standard error. `sbrs` and `sbrs-mcp` read the filter from `SB_LOG_LEVEL`, else `RUST_LOG`, else `info`, and `SB_LOG_FORMAT=pretty|json`. A program installs logging with `seleniumbase_rs::init_tracing()` or `init_tracing_from_runtime(&RuntimeConfig::from_env()?)`. Debug features: `tokio-console` (build with `RUSTFLAGS="--cfg tokio_unstable"`) and `full-tracing`; `error-backtrace` is declared but nothing reads it. Run headed with `BrowserConfig::with_headless(false)`, `LaunchOptions::builder().headless(false)` or `sbrs --headed`. Save evidence with `Plugins` and `ScreenshotOnFailurePlugin` / `PageSourceOnFailurePlugin`. Reproduce without Chrome with `Browser::new_mocked()` (feature `test-util`). Check the setup with `sbrs doctor`.

</Visibility>

<RequestExample>

```bash sbrs doctor
cargo run --bin sbrs -- doctor
```

</RequestExample>

<ResponseExample>

```text Example output
seleniumbase-rs environment diagnostics
========================================
SB_WEBDRIVER_URL: http://localhost:4444
SB_CHROME_BIN: (auto-detect)
SB_PATCH_CACHE_DIR: (platform cache)
SB_LOG_LEVEL: (unset: RUST_LOG, then info)
SB_LOG_FORMAT: Pretty
SB_SHUTDOWN_TIMEOUT_SECS: 30

Browser binary
  Found: <path to Chrome>

Patch cache
  Cache path: <path>

Chromedriver
  Found: downloaded_drivers/chromedriver
  Version: <major> (matches Chrome <major>)
  Status: ...
```

</ResponseExample>

## Turn on the logs

The crate logs with `tracing`. Logs go to standard error, never to standard
output: standard output carries what a program produces, such as an `sbrs`
command's answer or the protocol messages of `sbrs-mcp`.

`sbrs` and `sbrs-mcp` install the log subscriber themselves, with
`init_tracing_from_runtime`. These variables control it:

<ParamField path="SB_LOG_LEVEL" type="filter">

The filter, such as `debug` or `seleniumbase_rs=debug,info`. Unset, blank or
invalid, the next one is used.

</ParamField>

<ParamField path="RUST_LOG" type="filter">

Used when `SB_LOG_LEVEL` is unset, blank or invalid. When neither gives a
filter, it is `info`.

</ParamField>

<ParamField path="SB_LOG_FORMAT" type="pretty | json" default="pretty">

`json` writes one JSON object per line. Any other value makes
`RuntimeConfig::from_env` fail; `sbrs` warns about it and keeps the default.

</ParamField>

<ParamField path="NO_COLOR" type="any value">

Turns colour off. Colour is used only when standard error is a terminal.

</ParamField>

<CodeGroup>

```bash sbrs
SB_LOG_LEVEL=debug sbrs --headless smoke https://example.com
```

```bash Only this crate
RUST_LOG=error,seleniumbase_rs=debug sbrs --headless smoke https://example.com
```

```bash JSON lines
SB_LOG_FORMAT=json SB_LOG_LEVEL=debug sbrs --headless smoke https://example.com
```

</CodeGroup>

### In your own program or test

A program, a test or an example has no subscriber until it installs one.
Install it once, at the start; a second call is ignored.

```rust src/main.rs
use seleniumbase_rs::{init_tracing_from_runtime, RuntimeConfig};

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    // SB_LOG_LEVEL, else RUST_LOG, else info; SB_LOG_FORMAT picks pretty or json.
    init_tracing_from_runtime(&RuntimeConfig::from_env()?);
    Ok(())
}
```

| Function | Filter | Format |
|---|---|---|
| `init_tracing()` | `RUST_LOG`, else `info` | pretty |
| `init_tracing_json()` | `RUST_LOG`, else `info` | JSON |
| `init_tracing_with_filter("seleniumbase_rs=debug,info")` | the string, else `RUST_LOG`, else `info` | pretty |
| `init_tracing_json_with_filter(...)` | as above | JSON |
| `init_tracing_from_runtime(&config)` | `SB_LOG_LEVEL`, else `RUST_LOG`, else `info` | `SB_LOG_FORMAT` |

All five are at the crate root. In a test, call `seleniumbase_rs::init_tracing()`
first, then run it with `RUST_LOG=seleniumbase_rs=debug`.

Errors are logged as structured events with `error.category`, `error.transient`
and `error.hint`. Log one yourself with `.log()` on a `SeleniumBaseError` or
`.log_err()` on a result, and add where it happened with
`.sb_context("while loading profile")`. See
[Tracing and Logging](/tutorials/tracing#error-diagnostics).

## Features for a debugging build

<AccordionGroup>

<Accordion title="tokio-console: watch every async task" icon="list-check" description="For hangs: which tasks wait, for how long, and on what." defaultOpen>

<Steps>

<Step title="Build with the instrumentation" icon="hammer">

```bash Terminal
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tokio-console --bin sbrs -- --headless smoke https://example.com
```

A different `RUSTFLAGS` makes cargo rebuild every crate, so expect a long first
build.

</Step>

<Step title="Start the console" icon="terminal">

In another terminal:

```bash Terminal
cargo install --locked tokio-console
tokio-console
```

It connects to `127.0.0.1:6669`.

</Step>

</Steps>

Your own program gets the same when it installs logging through
`seleniumbase_rs::tracing_util` with the feature on.

</Accordion>

<Accordion title="full-tracing: timing histograms" icon="stopwatch">

Adds a `tracing-timing` layer to the subscriber the crate installs, which
records timing histograms of spans and events. The crate itself has no code
that reads or prints them.

</Accordion>

<Accordion title="error-backtrace: declared, not used" icon="circle-exclamation">

The feature is declared in `Cargo.toml`, but no code reads it, so turning it on
changes nothing, and `SeleniumBaseError` carries no backtrace. For a panic, set
`RUST_BACKTRACE=1` (or `full`). For a returned error, add context with
`.sb_context(...)` and read its `hint()`.

</Accordion>

</AccordionGroup>

## Watch the browser

Headless is the default. To see the window, turn it off:

<Tabs>

<Tab title="WebDriver engine" icon="car">

```rust
use seleniumbase_rs::BrowserConfig;

let config = BrowserConfig::default().with_headless(false);
```

</Tab>

<Tab title="Pure CDP" icon="bolt">

```rust
use seleniumbase_rs::sb_cdp::LaunchOptions;

let options = LaunchOptions::builder().headless(false).build()?;
```

</Tab>

<Tab title="sbrs" icon="terminal">

```bash Terminal
sbrs --headed open https://example.com
SB_HEADLESS=false sbrs open https://example.com
```

`--headed` and `--headless` go before the subcommand and beat `SB_HEADLESS`.

</Tab>

</Tabs>

On Linux without a display, run under a virtual display such as Xvfb; see
[Operating systems](/platforms#linux).

### Slow the run down

<Badge color="gray">WebDriver engine only</Badge>

Demo mode outlines each element before an interactive call and pauses after it;
slow mode pauses without the outline. Both go in the configuration:

```rust tests/watch.rs
use std::time::Duration;

use seleniumbase_rs::{BrowserConfig, RunSettings};

let config = BrowserConfig::default()
    .with_headless(false)
    .with_run_settings(
        RunSettings::default()
            .with_demo_mode(true)
            .with_demo_sleep(Duration::from_millis(800)),
    );
```

<Warning>

Never leave demo or slow mode on in CI: the run waits on purpose. Which calls
pause, and for how long, is in [Run settings](/tutorials/run_settings).

</Warning>

### Keep a browser open to inspect

`sbrs devtools` starts Chrome with the pure CDP engine and a DevTools endpoint
on `127.0.0.1:9222`, and keeps it running until you press Ctrl+C. Attach a
DevTools client to it; see [DevTools MCP servers](/tutorials/devtools_mcp).

## Keep evidence when a test fails

`Plugins` runs a test like `run_browser_test` and lets plugins look at the page
when it fails, before the browser is closed. Two plugins come with the crate:

<CodeBlock language="rust" filename="tests/login.rs" highlight="1-3,9-11" lines>
{`use seleniumbase_rs::plugins::observer::Plugins;
use seleniumbase_rs::plugins::page_source::PageSourceOnFailurePlugin;
use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
use seleniumbase_rs::{BrowserConfig, Result};

#[tokio::test]
async fn login_works() -> Result<()> {
    let mut plugins = Plugins::new()
        .with(ScreenshotOnFailurePlugin::new("latest_logs"))
        .with(PageSourceOnFailurePlugin::new("latest_logs"));

    plugins
        .run_browser_test("login_works", BrowserConfig::default(), async |sb| {
            sb.open("https://example.com").await?;
            sb.assert_title("Example Domain").await
        })
        .await
}`}
</CodeBlock>

- On a failure they save `login_works_<date>_<time>.png` and `.html` in the
  folder you name. An existing file is never replaced.
- A plugin that fails (a full disk, a browser that has gone) is logged and does
  not change the test's result. A test that panics is a failure for the plugins
  too, and the panic is raised again afterwards.
- For the pure CDP engine, `plugins.run_cdp_test(name, launch_options, async |page| ...)`
  does the same with a `Page`.

To save evidence by hand: `save_screenshot("login.png")` and
`save_page_source("login.html")` write into the logs folder, `latest_logs`. The
name must be a bare file name; one with a directory part or `..` is refused.
`js_errors()` on a pure CDP `Page` lists the page's uncaught script errors, and
`assert_no_js_errors()` fails on the first, on either engine.

## Reproduce without Chrome

<Badge color="blue">feature test-util</Badge> <Badge color="green">no browser</Badge>

`Browser::new_mocked()` returns a pure CDP browser that never launches Chrome.
Every protocol command it receives is recorded and answered by the handler you
register. It has one blank tab, and every navigation finishes at once. Any other
command succeeds with an empty result until you say otherwise.

```rust tests/title.rs
use seleniumbase_rs::sb_cdp::Browser;
use serde_json::json;

#[tokio::test]
async fn reads_the_title() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let (browser, mock) = Browser::new_mocked();
    mock.reply("Runtime.evaluate", json!({ "result": { "value": "Example Domain" } }));

    let page = browser.default_page().await?;
    assert_eq!(page.title().await?, "Example Domain");
    assert!(!mock.calls_to("Runtime.evaluate").is_empty());
    Ok(())
}
```

<ResponseField name="MockCtrl" type="struct">

Scripts and inspects the mocked browser. Exported from `seleniumbase_rs::sb_cdp`.

<Expandable title="methods">

<ResponseField name="on(method, handler)" type="fn">

Answers every `method` command by calling `handler` with its parameters. A later
registration for the same command replaces an earlier one.

</ResponseField>

<ResponseField name="reply(method, result)" type="fn">

Answers every `method` command with `result`.

</ResponseField>

<ResponseField name="fail(method, message)" type="fn">

Makes every `method` command fail with a protocol error saying `message`.

</ResponseField>

<ResponseField name="calls() / calls_to(method)" type="Vec<Call>">

Every command received so far, or every `method` command, oldest first.

</ResponseField>

<ResponseField name="disconnect()" type="fn">

Makes the browser go away, as a crashed Chrome would: every later command fails.

</ResponseField>

<ResponseField name="emit(method, params, session_id)" type="fn">

Pushes a protocol event to everything listening, as the browser would.

</ResponseField>

</Expandable>

</ResponseField>

- Run the existing mock tests with
  `cargo test --features test-util --test sb_cdp_mock`. In this repository a
  test file that uses the mock starts with `#![cfg(feature = "test-util")]`.
- `page.with_timeout(Duration::from_millis(150))` makes the failure paths quick.
- The crate's own unit tests have a WebDriver counterpart, `MockWebDriver` in
  `src/api/base_case_impls/base_case_impl_common.rs`. It is `#[cfg(test)]`, so
  only those unit tests can use it.

More in [Testing without Chrome](/tutorials/pure_cdp#testing-without-chrome).

## Record the network

A <Tooltip tip="HTTP Archive 1.2: a JSON file of every request and response.">HAR</Tooltip>
shows every request a tab made, with headers, status and timings. Start the
recorder before the navigation you want to see:

```rust
let recorder = page.start_har().await?;
page.goto("https://example.com").await?;
let har = recorder.finish().await?;
har.save("latest_logs/example.har")?;
```

Chrome's and Firefox's developer tools and HAR viewers open the file.
`start_har_with(HarOptions::new().content(true))` keeps the response bodies too.
A HAR holds cookies, so treat it like the session it came from. See
[Recording a HAR](/tutorials/pure_cdp#recording-a-har).

## Check the setup

`sbrs doctor` prints the `SB_*` settings (and any it could not read), the Chrome
it found, the patch cache, and the chromedriver it found with its major version
next to Chrome's. Each problem comes with a hint. An AI agent gets the same
from the MCP tool `diagnose_environment`.

## Driver problems

<Badge color="gray">WebDriver engine only</Badge>

The pure CDP engine needs no driver, so a failure that only happens with the
WebDriver engine points at chromedriver.

<Steps>

<Step title="See what was found" icon="magnifying-glass">

Run `sbrs doctor`. The `Chromedriver` part says which driver it found and
whether its major version matches Chrome's.

</Step>

<Step title="Fetch the matching driver" icon="download">

`sbrs install` downloads the chromedriver for your Chrome into
`downloaded_drivers/`. The WebDriver engine uses that file when its major
version fits the browser.

</Step>

<Step title="Or name one" icon="file">

Set `CHROMEDRIVER_PATH` to a driver you already have.

</Step>

<Step title="Rule the driver out" icon="bolt">

Run the same steps with the pure CDP engine (`sb_cdp`). If they pass there, the
problem is the driver or the WebDriver session.

</Step>

</Steps>

Before it starts a driver, the launcher makes the file runnable
(`browser::driver_access::make_runnable`): it sets a missing execute bit,
removes the macOS quarantine flag and, after a patch on macOS, signs the binary
again ad hoc. Firefox and Edge use geckodriver and msedgedriver; see
[Operating systems](/platforms#firefox-and-edge) and
[Common problems](/help/common_problems).

## The MCP server's logs

`sbrs-mcp` speaks the protocol on standard output and logs to standard error.
Set the log variables in the client's configuration; where the client shows the
server's standard error depends on the client.

```json .mcp.json
{
  "mcpServers": {
    "seleniumbase-cdp": {
      "command": "sbrs-mcp",
      "args": ["--server", "cdp"],
      "env": { "SB_LOG_LEVEL": "debug", "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    }
  }
}
```

- An invalid `SB_*` variable is logged as a warning when the server starts, and
  its default is used.
- A failed tool call is answered with a report: a stable `kind`, the call that
  was tried, the page's state, a hint and up to three next tools to try, such as
  `get_console_messages` and `diagnose_environment`. See
  [When a call fails](/tutorials/mcp_server#when-a-call-fails).
- From a checkout, `SB_LOG_LEVEL=debug cargo run --bin sbrs-mcp -- --server cdp`
  starts the server by hand; it reads the protocol on standard input and stops
  when that closes, or on Ctrl+C.

## Debug one test

<CodeGroup>

```bash One test, output shown
cargo test --features "mcp-server test-util" --test sb_cdp_mock waiting_succeeds_once_the_element_appears -- --nocapture
```

```bash Exact name
cargo test --features "mcp-server test-util" --test sb_cdp_mock -- --exact waiting_succeeds_once_the_element_appears
```

```bash A library unit test
cargo test -p seleniumbase-rs --lib tracing_util
```

```bash Real Chrome
cargo test --test sb_cdp_chrome -- --ignored --nocapture --test-threads=1
```

```bash The sbrs binary
cargo test -p seleniumbase-rs-cli --test cli_e2e_chrome -- --ignored
```

```bash With a backtrace
RUST_BACKTRACE=1 cargo test --features "mcp-server test-util" --test sb_cdp_mock -- --nocapture
```

</CodeGroup>

- The name after `--test` is the file in `tests/` (or `crates/cli/tests/`, with
  `-p seleniumbase-rs-cli`). A word after it filters the tests by name.
- Tests that need a real Chrome are `#[ignore]`d: add `--ignored`, and run one
  binary at a time.
- `--nocapture` shows what the test prints as it runs. Logs appear only if the
  test installs a subscriber (see [In your own program or test](#in-your-own-program-or-test)).

## rust-analyzer

Use rust-analyzer, or `cargo check`, for types and references before a full
build. The repository's `rust-analyzer.toml`:

- turns on the `mcp-server` and `test-util` features, without which `src/mcp`
  and the test helpers get no diagnostics;
- checks into a target directory of its own (`targetDir = true`), so analysis
  never waits on the lock that builds and test runs hold;
- keeps two threads and no cache priming, for a small machine that is also
  compiling.

Code behind other features, such as `base_case_impl_gui.rs` (feature `gui`),
gets diagnostics only when you add that feature to your editor's rust-analyzer
settings.

## Known traps

<Callout icon="clock" color="#F59E0B">

**Headless and headed runs time differently.** Chrome answers a pointer move
only at the next frame, and the frame rate differs between the two modes. Check
which mode a test uses before you trust a timing. `BrowserConfig` is headless by
default; the MCP server's `start_browser` and `sbrs devtools` are headed except
on Linux.

</Callout>

<AccordionGroup>

<Accordion title="macOS is slow the first time it runs a new executable" icon="apple">

macOS scans a freshly written or replaced program on its first run, which can
take seconds. Tests that fake `java` or another script run it once with
`--warm` before they time it.

</Accordion>

<Accordion title="Chrome will not start as root or in a container" icon="docker">

It needs `--no-sandbox` there. The WebDriver engine always passes it on Linux.
For the pure CDP engine, set `SB_NO_SANDBOX` (any value, `0` included) or call
`LaunchOptionsBuilder::no_sandbox(true)`. The real-Chrome tests of this
repository pass it when `CI` is set. In Docker, also start the container with
`--shm-size=1g`.

</Accordion>

<Accordion title="Doctests fail with extern location does not exist" icon="link-slash">

Two checkouts share one `CARGO_TARGET_DIR`. Give each its own; see
[Build from source](/contributing/from-source#keep-builds-small).

</Accordion>

<Accordion title="sbrs commander exits with status 2" icon="terminal">

The binary was built without the `tui` feature. It prints a hint; build with
`--features tui`.

</Accordion>

</AccordionGroup>

## Next steps

<Columns cols={2}>

<Card title="Tracing and Logging" icon="scroll" href="/tutorials/tracing">

Spans, error events and the `SB_*` variables in full.

</Card>

<Card title="Common problems" icon="life-ring" href="/help/common_problems">

Error messages and what to do about them.

</Card>

<Card title="Build from source" icon="hammer" href="/contributing/from-source">

The toolchain, the checks and the real-Chrome tests.

</Card>

<Card title="Benchmarking" icon="gauge-high" href="/contributing/benchmarking">

Measure what a change costs.

</Card>

</Columns>
