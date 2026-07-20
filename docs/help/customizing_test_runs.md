# Customizing Test Runs

SeleniumBase for Rust can be configured through the `BrowserConfig` builder, a global config file, or CLI flags.

## BrowserConfig

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true)
    .with_mobile(true)
    .with_proxy("http://proxy.example.com:8080".into());

let mut sb = BaseCase::new(config).await?;
```

## Global config file

Create `sbase_config.toml` in your project root:

```toml
headless = false
mobile = false
locale = "en-US"
timeout = 10.0
```

Or a flat `.sbase_config` file:

```text
headless=false
locale=en-US
timeout=10.0
```

Environment variables override file values. Prefix variables with `SBASE_`:

```bash
export SBASE_HEADLESS=true
export SBASE_LOCALE=fr-FR
```

## CLI flags

When using the `sbase` binary:

```bash
cargo run --bin sbase -- open https://seleniumbase.io --headless --proxy http://proxy.example.com:8080
```

Common flags:

| Flag | Description |
|------|-------------|
| `--headless` | Run browser without a visible window. |
| `--mobile` | Emulate a mobile device. |
| `--proxy HOST:PORT` | Route traffic through a proxy. |
| `--proxy-pac-url URL` | Use a PAC file for proxy configuration. |
| `--user-data-dir DIR` | Load a Chromium user data directory. |
| `--extension-dir DIR` | Load a Chromium extension. |
| `--reuse-session` / `--rs` | Reuse an existing browser session. |
| `-n NUM` | Run tests with parallel browsers. |

## Combining config sources

Settings are resolved in this order, with later sources winning:

1. Default values
2. Global config file
3. Environment variables
4. Programmatic `BrowserConfig`
5. CLI flags
