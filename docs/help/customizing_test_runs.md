# Customizing Test Runs

SeleniumBase for Rust can be configured through the `BrowserConfig` builder, a
global config file, environment variables, or CLI flags. This guide explains the
full hierarchy and shows how to make each source of configuration work for your
workflow.

## What you will learn

- How to configure a run with the `BrowserConfig` builder.
- How to create a global TOML or flat config file.
- How to override settings with `SB_*` environment variables.
- How CLI flags interact with programmatic and file configuration.

## BrowserConfig builder

The most direct way to configure a run is the `BrowserConfig` builder. Every
method returns `Self`, so calls can be chained fluently.

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(true)
        .with_mobile(true)
        .with_locale("en-US")
        .with_proxy("http://proxy.example.com:8080");

    let mut sb = BaseCase::new(config).await?;
    sb.set_timeout(15).await?;
    sb.open("https://seleniumbase.io").await?;
    sb.quit().await?;
    Ok(())
}
```

Key builder methods:

| Method | Effect |
|---|---|
| `with_mode(mode)` | Set `DriverMode::WebDriver`, `Cdp`, or `Uc`. |
| `with_browser(browser)` | Set `Browser::Chrome`, `Chromium`, `Edge`, or `Firefox`. |
| `with_headless(bool)` | Run without a visible window. |
| `with_mobile(bool)` | Emulate a mobile device. |
| `with_locale(s)` | Set `navigator.languages` / `Accept-Language`. |
| `with_proxy(url)` | Route traffic through an HTTP/HTTPS/SOCKS proxy. |
| `with_proxy_pac_url(url)` | Load proxy configuration from a PAC file. |
| `with_user_data_dir(path)` | Launch with an existing Chromium profile. |
| `with_extension_dir(path)` | Load an unpacked extension. |
| `with_reuse_session(bool)` | Attach to an existing session when possible. |
| `with_threads(n)` | Parallel workers when running a test suite. |
| `with_extra_args(vec)` / `push_extra_arg(s)` | Pass extra Chromium launch flags. |

## Global config file

Create `sbase_config.toml` in your project root for shared defaults:

```toml
# sbase_config.toml
browser = "chrome"
headless = false
mobile = false
locale = "en-US"
timeout_seconds = 10.0
screenshot_dir = "screenshots"
proxy = "http://proxy.example.com:8080"
```

Or a flat `.sbase_config` file:

```text
# .sbase_config
browser=chrome
headless=false
locale=en-US
timeout_seconds=10.0
```

The loader searches the current working directory and ancestor directories, so a
config placed at the workspace root applies to all crates in the workspace.

## Environment variables

Environment variables override file values. The prefix is `SB_` (not `SBASE_`):

```bash
export SB_HEADLESS=true
export SB_LOCALE=fr-FR
export SB_BROWSER=chrome
export SB_TIMEOUT=20
export SB_WEBDRIVER_URL=http://localhost:4444/wd/hub
```

Variable names map to `Settings`/`RuntimeConfig` fields with `SB_` prepended and
converted to UPPER_SNAKE_CASE. For nested fields, use double underscores (e.g.,
`SB_LOGGING__LOG_LEVEL=debug`).

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
| `-n NUM` / `--threads NUM` | Run tests with parallel browsers. |
| `--browser NAME` | Select `chrome`, `chromium`, `edge`, or `firefox`. |
| `--mode MODE` | Select `webdriver`, `cdp`, or `uc`. |

Run `sbase --help` and `sbase <subcommand> --help` for the full list of flags.

## Combining config sources

Settings are resolved in this order, with later sources winning:

1. Built-in defaults.
2. Global config file (`sbase_config.toml` or `.sbase_config`).
3. Environment variables (`SB_*`).
4. Programmatic `BrowserConfig`.
5. CLI flags.

Use the config file for stable defaults, environment variables for CI-specific
overrides, and the builder or CLI flags for one-off experiments.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Environment variable ignored | Used `SBASE_` instead of `SB_` | Use the `SB_` prefix. |
| Config file not loaded | File is in a child directory | Move it to the project or workspace root. |
| CLI flag not applied | Flag belongs to a subcommand | Place it after the subcommand: `sbase open --headless`. |
| Proxy not used | Format is invalid | Pass a full URL such as `http://host:8080`. |
