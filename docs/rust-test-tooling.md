# Writing Browser Tests

`seleniumbase-rs` uses standard Rust test tools instead of reproducing Python
runner decorators.

## Tool mapping

| Python or JavaScript concept | Rust equivalent |
|------------------------------|-----------------|
| `pytest` async plugin | `#[tokio::test]` |
| SeleniumBase setup and teardown | `run_browser_test` |
| Test selection | `cargo test name` or nextest filters |
| Parallel test processes | `cargo nextest run` |
| Feature-specific suites | Cargo features |
| Environment configuration | `BrowserConfig`, TOML settings, environment variables |
| Parameterized cases | A loop, helper function, or a parameterized-test crate |

## Recommended test shape

```rust
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

#[tokio::test]
async fn user_can_open_the_home_page() -> Result<()> {
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("https://example.com").await?;
            sb.assert_element("h1").await?;
            sb.assert_text("h1", "Example Domain").await
        })
    })
    .await
}
```

Run the standard harness with `cargo test`. If
[`cargo-nextest`](https://nexte.st/) is installed, run `cargo nextest run`.
The repository profile intentionally does not retry failed tests.

