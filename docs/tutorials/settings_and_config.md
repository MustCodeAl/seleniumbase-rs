# Settings and Global Configuration

SeleniumBase for Rust can load test configuration from a global config file,
environment variables, or programmatic `BrowserConfig` overrides.

## Configuration sources

Priority (highest to lowest):

1. Code-level `BrowserConfig`
2. Environment variables (`SB_*`)
3. Global config file (`sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`)
4. Built-in defaults

## Global config file

Create `sbase_config.toml` in the project root:

```toml
browser = "chrome"
headless = false
timeout_seconds = 30
screenshot_dir = "screenshots"
window_width = 1920
window_height = 1080
proxy = "http://proxy.example.com:8080"
mode = "webdriver"
reuse_session = false
mobile = false
threads = 2
```

## Loading settings

```rust
use seleniumbase_rs::config::settings::Settings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load from global config file, if present, with env overrides.
    let settings = Settings::load::<&str>(None)?;

    // Or load from a specific file.
    let settings = Settings::from_file("staging.toml")?;

    // Convert to BrowserConfig and start a test.
    let config = settings.to_browser_config();
    Ok(())
}
```

## Environment variables

Any setting can be overridden with an `SB_` prefixed variable:

```bash
export SB_BROWSER=firefox
export SB_HEADLESS=true
export SB_TIMEOUT=60
export SB_PROXY=http://proxy:8080
export SB_THREADS=4
```

## BrowserConfig in code

For per-test overrides, build a `BrowserConfig` directly:

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true)
    .with_proxy("http://proxy:8080");
```

## Best practices

- Commit a sample config file (e.g., `sbase_config.example.toml`) to the repo.
- Keep secrets (passwords, API keys) out of config files; use environment variables.
- Use `Settings::load_global()` in test harnesses and `BrowserConfig` overrides for individual tests.
