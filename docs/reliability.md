# Reliability

Rust moves many harness defects from runtime to compile time. Typed
configuration, explicit `Result` values, and exhaustive matching make failures
visible instead of silently continuing.

## Browser lifecycle

Use `run_browser_test` for Tokio tests. It runs the test body and then awaits
`BaseCase::quit`, including when the body returns an error:

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

If both the test and cleanup fail, the returned lifecycle error retains both
failure messages.

## Avoid hiding flaky behavior

The included nextest profile uses zero automatic retries. Retry a browser
action only when the operation is known to be transient and idempotent.
Unconditional test retries can hide race conditions and broken selectors.

Use web-first waits and specific assertions rather than sleeps. Capture
screenshots, page source, browser logs, and structured reports at the point of
failure.

