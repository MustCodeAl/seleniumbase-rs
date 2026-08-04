# Settings and Global Configuration

`seleniumbase-rs` supports multiple configuration layers: built-in defaults, a
global config file, `SB_*` environment variables, and programmatic
`BrowserConfig` overrides. This makes it easy to keep environment-specific
settings out of source code while still allowing per-test customization.

This page explains the configuration hierarchy, the available settings, and how
to load and combine them.

## Configuration hierarchy

Settings are resolved from lowest to highest priority. A higher-priority source
overrides a lower-priority one:

1. Built-in defaults.
2. Global config file (`sbase_config.toml`, `.sbase_config.toml`, or `.sbase_config`).
3. Environment variables (`SB_*`).
4. Programmatic `BrowserConfig`.
5. CLI flags (when using `sbase`).

This means a `BrowserConfig` value in code wins over an environment variable,
and a CLI flag wins over everything else.

## Global config file

Create `sbase_config.toml` in your project root:

```toml
browser = "chrome"
headless = false
timeout_seconds = 30
screenshot_dir = "screenshots"
window_width = 1920
window_height = 1080
proxy = "http://proxy.example.com:8080"
proxy_pac_url = ""
mode = "webdriver"
reuse_session = false
mobile = false
threads = 2
locale = "en-US"
user_agent = ""
user_data_dir = ""
extension_dir = ""
ad_block = false
```

The file is optional. If it is missing, built-in defaults are used. The
`Settings::load_global()` function searches for `sbase_config.toml`, then
`.sbase_config.toml`, then `.sbase_config` in the current directory.

You can also use a flat `.sbase_config` file in `KEY=VALUE` format:

```text
headless=false
locale=en-US
timeout_seconds=10
```

## Loading settings programmatically

```rust
use seleniumbase_rs::config::settings::Settings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the global config file (if present) and apply SB_* env overrides.
    let settings = Settings::load_global()?;

    // Or load from a specific file.
    let settings = Settings::from_file("staging.toml")?;

    // Convert to BrowserConfig.
    let config = settings.to_browser_config();

    Ok(())
}
```

`Settings::load` accepts an optional path. When the path is `None`, the global
config file is used and environment variables are applied on top.

## Environment variables

Every setting in `Settings` can be overridden with an `SB_` prefixed variable:

| Variable | Maps to |
|----------|---------|
| `SB_BROWSER` | `browser` |
| `SB_HEADLESS` | `headless` |
| `SB_TIMEOUT` | `timeout_seconds` |
| `SB_SCREENSHOT_DIR` | `screenshot_dir` |
| `SB_PROXY` | `proxy` |
| `SB_PROXY_PAC_URL` | `proxy_pac_url` |
| `SB_WINDOW_WIDTH` | `window_width` |
| `SB_WINDOW_HEIGHT` | `window_height` |
| `SB_USER_DATA_DIR` | `user_data_dir` |
| `SB_EXTENSION_DIR` | `extension_dir` |
| `SB_LOCALE` | `locale` |
| `SB_USER_AGENT` | `user_agent` |
| `SB_MODE` | `mode` (`webdriver`, `cdp`, or `uc`) |
| `SB_AD_BLOCK` | `ad_block` |
| `SB_REUSE_SESSION` | `reuse_session` |
| `SB_MOBILE` | `mobile` |
| `SB_THREADS` | `threads` |

Boolean values accept `true`/`false`, `1`/`0`, `yes`/`no`, or `on`/`off`.

Example:

```bash
export SB_BROWSER=firefox
export SB_HEADLESS=true
export SB_TIMEOUT=60
export SB_PROXY=http://proxy:8080
export SB_THREADS=4
```

## Runtime environment variables

In addition to `Settings`, the crate reads `SB_*` runtime configuration for
library behavior:

| Variable | Default | Purpose |
|----------|---------|---------|
| `SB_WEBDRIVER_URL` | `http://localhost:4444` | WebDriver endpoint. |
| `SB_CHROME_BIN` | auto-detect | Explicit Chrome/Chromium binary path. |
| `SB_PATCH_CACHE_DIR` | platform cache | Directory for patched Chrome/Chromedriver copies. |
| `SB_LOG_LEVEL` | `info` | Default tracing log level. |
| `SB_LOG_FORMAT` | `pretty` | `pretty` or `json` output. |
| `SB_SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout. |
| `SB_CHROMEDRIVER_PORT` | `0` | Port for auto-started chromedriver (`0` = ephemeral). |
| `SB_IMPLICIT_WAIT_SECS` | `30` | Default implicit wait timeout. |

## BrowserConfig in code

For per-test overrides, build a `BrowserConfig` directly:

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true)
    .with_proxy("http://proxy:8080")
    .with_locale("en-US")
    .with_extra_args(vec!["--disable-blink-features=AutomationControlled".into()]);
```

`BrowserConfig` also supports `from_env()` to load runtime variables directly:

```rust
use seleniumbase_rs::BrowserConfig;

let config = BrowserConfig::from_env()?;
```

## Best practices

- Commit a sample config file (e.g., `sbase_config.example.toml`) to the repo so
  new contributors know what settings exist.
- Keep secrets (passwords, API keys) out of config files; use environment
  variables or a secrets manager.
- Use `Settings::load_global()` in test harnesses and `BrowserConfig` overrides
  for individual tests.
- Use `BrowserConfig::from_env()` in deployable applications so operators can
  change behavior without rebuilding.

## Configuration checklist

- [ ] A sample config file is committed to the repository.
- [ ] Secrets are loaded from environment variables.
- [ ] Per-test overrides use `BrowserConfig` builders.
- [ ] CI uses environment variables or a dedicated config file.
- [ ] `SB_*` variables are documented for operators.

## Related reading

- [CLI Usage](./cli_usage.md) — command-line overrides and flags.
- [Tracing and Logging](./tracing.md) — `SB_LOG_LEVEL` and `SB_LOG_FORMAT`.
- [Docker Guide](../help/docker.md) — containerized configuration.
