# Playwright Mode

Playwright Mode uses the [`playwright-rs`](https://github.com/padamson/playwright-rust) crate to drive browsers through Playwright. It can bypass bot-detection systems that target WebDriver fingerprints.

## Enable the feature

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["playwright"] }
```

Or on the command line:

```bash
cargo run --example playwright_mode --features playwright
```

## Driver installation

`playwright-rs` downloads the Playwright driver during its build script. Ensure the build host can reach the Playwright CDN, or pre-install the driver:

```bash
npx playwright install
```

## Example

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Playwright,
        ..Default::default()
    }).await?;

    sb.open("https://seleniumbase.io").await?;
    sb.assert_text_visible("SeleniumBase", "body").await?;

    sb.quit().await?;
    Ok(())
}
```

## Notes

- Playwright mode is optional; the default build does not require the driver.
- If driver download fails, use `DriverMode::Uc` or `DriverMode::Cdp` instead.
