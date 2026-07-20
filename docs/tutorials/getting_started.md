# Getting Started with SeleniumBase for Rust

This guide walks you through your first browser automation test using the Rust port of SeleniumBase.

## Prerequisites

- Rust toolchain (1.78 or newer)
- A Chromium-based browser (Google Chrome, Chromium, or Microsoft Edge)
- (Optional) A running WebDriver endpoint; otherwise the library auto-starts chromedriver

## Create a new Rust project

```bash
cargo new my_first_test
cd my_first_test
```

Add the dependency to `Cargo.toml`:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/SeleniumBase" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Write your first test

Replace `src/main.rs` with:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc);

    let mut sb = BaseCase::new(config).await?;

    sb.open("https://seleniumbase.io/simple/login").await?;
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;
    sb.assert_text_visible("Welcome!", "body").await?;

    sb.quit().await?;
    Ok(())
}
```

Run it:

```bash
cargo run
```

The library downloads and starts chromedriver automatically when needed.

## Next steps

- Learn about [Undetected (UC) mode](./uc_mode.md)
- Explore the full [API reference](./api_reference.md)
- Use the [CLI helper](./cli_usage.md) for quick one-off commands
