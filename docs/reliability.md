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

```rust
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
- The closure receives `&mut BaseCase` and must return `Result<(), E>` where `E`
  converts into `SeleniumBaseError`.

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

A better strategy is to remove the source of flakiness:

- Use web-first waits instead of fixed sleeps.
- Add stable `data-testid` or `name` attributes to the application under test.
- Capture screenshots, page source, and structured traces at the point of failure.

## Prefer explicit waits over sleeps

Browser tests fail most often because they interact with elements before the
page is ready. `BaseCase` methods such as `click`, `type_text`, and `get_text`
wait automatically for the target element to be present and visible. When you
need finer control, use explicit waits:

```rust
sb.wait_for_element_visible("#submit", 10).await?;
sb.wait_for_text("body", "Order confirmed", 15).await?;
sb.wait_for_element_not_visible(".spinner", 10).await?;
```

Avoid `tokio::time::sleep` whenever possible. It makes tests slower and does not
eliminate timing races.

## Capture artifacts on failure

When a test fails, the most useful diagnostic data is often the state of the
browser at the moment of failure:

```rust
match sb.assert_text_visible("Welcome!", "body").await {
    Ok(()) => {}
    Err(e) => {
        let _ = sb.save_screenshot_to_logs().await;
        let _ = sb.save_page_source_to_logs().await;
        return Err(e);
    }
}
```

You can also save artifacts from the CLI:

```bash
cargo run --bin sbase -- screenshot
cargo run --bin sbase -- save-source
```

## Use configuration, not code, for environment differences

Hard-coding URLs, credentials, or timeouts in tests makes them fragile. Use
`BrowserConfig`, `sbase_config.toml`, and `SB_*` environment variables instead.
This keeps tests portable between local development, staging, and production
environments.

See [Settings and Configuration](tutorials/settings_and_config.md) for the full
configuration hierarchy.

## Reliability checklist

- [ ] Tests use `run_browser_test` or an equivalent cleanup wrapper.
- [ ] Tests rely on explicit waits rather than sleeps.
- [ ] Selectors target stable attributes, not generated class names.
- [ ] Retries are used sparingly and only for transient, idempotent operations.
- [ ] Screenshots and page source are captured on failure.
- [ ] Configuration is externalized via environment variables or config files.
- [ ] The suite is run with `cargo nextest run` or a similar runner that does not
      silently retry failures.

