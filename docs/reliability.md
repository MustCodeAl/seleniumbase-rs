---
title: "Reliability"
---

# Reliability

Reliable browser automation requires more than a good API. Tests must clean up
after themselves, wait for the right conditions, and fail loudly when something
is wrong instead of silently continuing. Rust's type system and the design of
`seleniumbase-rs` help with all three.

This page explains the lifecycle helpers, anti-patterns, and observability
practices that make a suite reliable.

## Compile-time reliability

Rust catches many harness defects before any browser launches:

- Renamed methods, changed signatures, and missing `await` are compiler errors.
- Explicit `Result` values force callers to decide what to do with failures.
- Exhaustive matching on enums prevents silently unhandled cases.

These properties do not eliminate flaky tests—page timing and selectors can
still fail at runtime—but they remove a large class of structural bugs that
plague dynamic-language suites.

## Browser lifecycle with `run_browser_test`

The safest way to write a Tokio test is with `run_browser_test`. It creates a
`BaseCase`, runs the test body, and then awaits `BaseCase::quit` even if the
body returns an error:

```rust,no_run
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

#[tokio::test]
async fn example_domain() -> Result<()> {
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("https://example.com").await?;
            sb.assert_title("Example Domain").await
        })
    })
    .await
}
```

Key properties:

- The browser is always closed, even when assertions fail.
- If both the test body and cleanup fail, the returned lifecycle error retains
  both failure messages.
- The closure receives `&mut BaseCase` and must return a boxed future whose output
  is `Result<()>`, that is `Result<(), SeleniumBaseError>`. Inside it, `?` converts
  any error that has a `From` impl for `SeleniumBaseError`.
- If you forget `quit`, dropping a `BaseCase` still tries to close its browser
  (see [Shutdown](#shutdown-and-signals)), but that cleanup runs in the
  background and cannot be awaited, which is why `run_browser_test` closes it
  explicitly.

Run the test with the standard harness:

```bash
cargo test example_domain
```

Or with [cargo-nextest](https://nexte.st/):

```bash
cargo nextest run example_domain
```

## Do not hide flaky behavior

Retries are sometimes necessary for transient network or driver issues, but
unconditional retries hide race conditions and broken selectors. The repository's
included nextest profile uses zero automatic retries.

Retry a browser action only when:

- The failure is known to be transient (for example, a brief driver disconnect).
- The operation is idempotent (clicking the same button twice has no harmful side
  effect).
- You log every retry attempt.

When those hold, `RetryPolicy` (in `seleniumbase_rs::utilities::retry`, and
re-exported at the crate root) does the retrying with exponential backoff and
logs each retry at `warn`. You say which errors are transient;
`SeleniumBaseError::is_transient` is a ready-made answer (browser disconnects,
network and download failures, wait timeouts):

```rust
use seleniumbase_rs::{RetryPolicy, SeleniumBaseError};

# async fn fetch() -> Result<String, SeleniumBaseError> { Ok(String::new()) }
# async fn demo() -> Result<(), SeleniumBaseError> {
let body = RetryPolicy::network()
    .retry(|| fetch(), SeleniumBaseError::is_transient)
    .await?;
# let _ = body;
# Ok(())
# }
```

The presets are `RetryPolicy::default()` (3 attempts, 200 ms doubling to 5 s),
`webdriver()`, `network()` (5 attempts), and `seleniumbase()` (the Python
`retry_on_exception` defaults: 6 attempts, 1 s doubling to 32 s); or fill in the
four public fields yourself. `retry_blocking` is the version for synchronous
code. This module replaces the older `retry_on_exception`,
`retry_on_exception_async` and `rate_limited` functions in `common::decorators`,
which are deprecated.

Two more helpers live beside it. `poll_until(timeout, policy, op)` calls an
async operation until it returns `Some` or the deadline passes (the waits in
`BrowserSession` are built on it), and `RateLimiter::new(max_calls, window)`
keeps a loop, or several tasks sharing it behind an `Arc`, to at most that many
calls per window (`acquire().await` waits for a slot).

A better strategy is to remove the source of flakiness:

- Use web-first waits instead of fixed sleeps.
- Add stable `data-testid` or `name` attributes to the application under test.
- Capture screenshots, page source, and structured traces at the point of failure.

## Prefer explicit waits over sleeps

Browser tests fail most often because they interact with elements before the
page is ready. `BaseCase` actions wait for the target before they act, for up
to the default timeout (10 seconds; change it with `set_timeout`): `click` waits
until the element is displayed and enabled, `type_text`, `clear` and `hover`
until it is displayed, and `get_text` and `submit` until it exists. When you need
finer control, use explicit waits:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.wait_for_element_visible("#submit", 10).await?;
sb.wait_for_text("body", "Order confirmed", 15).await?;
sb.wait_for_element_not_visible(".spinner", 10).await?;
# Ok(())
# }
```

Avoid `tokio::time::sleep` whenever possible. It makes tests slower and does not
eliminate timing races.

## Errors that say what to do

Every `SeleniumBaseError` carries a hint, and `is_transient()` tells a retryable
failure (a browser disconnect, a network or download error, a wait timeout) from
one that will fail again. Element errors point out a likely mistake in the
selector, and a wait that times out reports the error that stopped it. `sbrs` prints `error:`, `caused by:` and
`hint:` lines and exits with a status that follows the error's category, so a CI
script can tell a bad selector from a missing browser.

In reports, a failed test carries its failure kind, hint, page URL and title and
links to its evidence (`FailureDetails`, `ReportPlugin::with_evidence`). When an
AI agent drives the browser through the MCP servers, a failed tool call is
answered with the call, the page's state (URL, title, ready state, script
errors, selector matches, read within two seconds), the hint and the next tools
to try.

## Match the driver to the browser

A chromedriver for a different Chrome major version is the usual cause of "the
driver keeps crashing". The launcher compares the two, downloads the driver for
the installed Chrome's major version when they differ, and writes a replaced
driver as a new file (macOS kills a signed binary that is overwritten in place).
`sbrs install` and `sbrs doctor` apply the same check, and the MCP tool
`diagnose_environment` reports the versions and the fix without starting a
browser. The pure CDP engine has no driver to match.

## Browsers that outlive a test

Pooled browsers (`BrowserPool`, `Lease`) are bounded, isolated per context and
recycled, so one test's cookies and storage do not leak into the next, and no
idle timeout closes a launched browser under a long-running service. Temporary state that the library writes
(the scraping state store, for example) is written atomically: a temp file is
synced and renamed over the target, a corrupt file is an error, and a missing
file reads as empty.

## Testing the harness itself

The pure CDP engine has a mock browser (`Browser::new_mocked`), so waits,
retries and error paths are tested without Chrome. Tests that need a real Chrome
are named `*_chrome.rs`, are `#[ignore]`d and should run one binary at a time.
Tests that fake `java` or other executables warm the script once first, because
macOS can take seconds to run a freshly written script the first time; a test
that races that start-up is flaky by construction.

## Known gaps

- Most behaviour is covered by unit tests and a mock browser; only part of it has
  been run against a real Chrome (155).
- The parity entries classified as composed or not-applicable were audited once
  (38 problems found and fixed or re-marked), but only by reading the code; the
  compositions have not been compared with Python's waits and visibility rules one
  by one.
- The `RunSettings` pause arithmetic, sleep recording and skip-js-waits are unit
  tested; the hooks inside `open`, `click`, `type_text`, `update_text` and
  `press_keys` run under no test.

## Capture artifacts on failure

When a test fails, the most useful diagnostic data is often the state of the
browser at the moment of failure:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
match sb.assert_text_visible("Welcome!", "body").await {
    Ok(()) => {}
    Err(e) => {
        let _ = sb.save_screenshot_to_logs().await;
        let _ = sb.save_page_source_to_logs().await;
        return Err(e);
    }
}
# Ok(())
# }
```

The files go to the `latest_logs` folder of the working directory, named
`screenshot_<milliseconds>.png` and `page_source_<milliseconds>.html`. The
`sbrs screenshot` and `sbrs save-source` commands write the same files, but
each starts its own fresh browser, so they capture a blank page, not your test's
failing one. For failure evidence use the code above, or the test plugins in
[Test Plugins](tutorials/test_plugins.md).

## Use configuration, not code, for environment differences

Hard-coding URLs, credentials, or timeouts in tests makes them fragile. Use
`BrowserConfig`, `sbase_config.toml`, and `SB_*` environment variables instead.
This keeps tests portable between local development, staging, and production
environments.

See [Settings and Configuration](tutorials/settings_and_config.md) for the full
configuration hierarchy.

## Shutdown and signals

A process that owns a browser has to close it when it is told to stop. A
terminal sends SIGINT on Ctrl-C and orchestrators send SIGTERM; left alone, both
end the process on the spot and leave Chrome and its driver running.
`seleniumbase_rs::common::shutdown` turns those signals into something async
code can wait on, and `sbrs` and `sbrs-mcp` use it themselves: on a
signal they close the browser, waiting at most `SB_SHUTDOWN_TIMEOUT_SECS`
seconds (default 30, `0` for no grace), and `sbrs` exits with status 130 for
SIGINT or 143 for SIGTERM.

In your own program, race the work against the signals and bound the cleanup:

```rust,no_run
use seleniumbase_rs::common::shutdown::{
    cleanup_within, run_until_shutdown, timeout_from_env, Outcome,
};

# async fn run_tests() {}
# async fn close_browsers() {}
# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
match run_until_shutdown(run_tests()).await? {
    Outcome::Completed(()) => {}
    Outcome::Interrupted(signal) => eprintln!("{signal} received; closing browsers"),
}
cleanup_within(timeout_from_env(), close_browsers()).await;
# Ok(())
# }
```

Dropping a `BrowserSession` (and so a `BaseCase`) that was not quit starts its
cleanup on the Tokio runtime with `spawn_cleanup`, because `Drop` cannot await.
Call `drain_cleanups(timeout_from_env()).await` before the program exits so that
cleanup is not cut off; `sbrs` does. Once a handler is installed, a signal no
longer ends the process by itself, so the code that installs one must keep
waiting on it.

## Reliability checklist

- [ ] Tests use `run_browser_test` or an equivalent cleanup wrapper.
- [ ] Programs that own browsers stop on SIGINT/SIGTERM and wait for cleanup
      (`common::shutdown`).
- [ ] Tests rely on explicit waits rather than sleeps.
- [ ] Selectors target stable attributes, not generated class names.
- [ ] Retries are used sparingly and only for transient, idempotent operations.
- [ ] Screenshots and page source are captured on failure.
- [ ] Configuration is externalized via environment variables or config files.
- [ ] The suite is run with `cargo nextest run` or a similar runner that does not
      silently retry failures.

