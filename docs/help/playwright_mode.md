# Playwright Mode

Playwright Mode uses [`rustwright`](https://github.com/Skyvern-AI/rustwright), a
native Rust CDP engine, to drive Chromium with a Playwright-compatible API. It
can bypass bot-detection systems that target WebDriver fingerprints without
requiring a Node Playwright driver.

## What you will learn

- How to enable the optional `playwright` feature.
- How `rustwright` resolves Chromium.
- How to activate and use a Playwright session.

## Enable the feature

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["playwright"] }
```

Or on the command line:

```bash
cargo run --example playwright_mode --features playwright
```

## Chromium installation

`rustwright` discovers a system Chromium or downloads a Chromium build on first
launch. Ensure the build host can reach the Chromium CDN, or point it at a local
Chromium executable by setting `BrowserConfig::browser_binary_path`.

## Example

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.activate_playwright_mode().await?;

    if let Some(session) = sb.playwright_session() {
        session.goto("https://example.com").await?;
        let heading = session.get_text("h1").await?;
        println!("Page heading: {heading}");
        session.close().await?;
    }

    sb.quit().await?;
    Ok(())
}
```

## Notes

- Playwright mode is optional; the default build does not include the
  `rustwright` engine.
- The session API is async and mirrors the WebDriver-backed helpers where
  practical (`goto`, `click`, `type_text`, `get_text`, `evaluate`, `screenshot`,
  `close`).
- For pure WebDriver automation use `DriverMode::Uc` or `DriverMode::Cdp`
  instead.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `rustwright` not found | Feature not enabled | Build with `--features playwright`. |
| Chromium download fails | Network restricted | Pre-install Chromium and set `BrowserConfig::browser_binary_path`. |
| Session methods missing | API mismatch | Check the rustdocs for the installed `rustwright` version. |
