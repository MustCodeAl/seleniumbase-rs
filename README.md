# SeleniumBase for Rust

`seleniumbase-rs` is a Rust port of the Python SeleniumBase browser-automation
framework. It layers an ergonomic, Python-parity `BaseCase` API on top of the
`thirtyfour` WebDriver crate and adds native stealth, CDP, and tooling features
that are usually only available in Python or Node-based stacks.

This README is the project landing page. If you are reading the mdBook, see the
[book overview](./docs/README.md) for a narrative introduction.

## Table of contents

- [What you get](#what-you-get)
- [Quick start](#quick-start)
- [Installation](#installation)
- [Architecture at a glance](#architecture-at-a-glance)
- [Feature overview](#feature-overview)
  - [BaseCase API](#basecase-api)
  - [Browser modes](#browser-modes)
  - [Stealth and fingerprinting](#stealth-and-fingerprinting)
  - [Recorder and low-code scenarios](#recorder-and-low-code-scenarios)
- [CLI highlights](#cli-highlights)
- [Examples](#examples)
- [Feature flags](#feature-flags)
- [Documentation](#documentation)
- [Project status](#project-status)
- [Verified commands](#verified-commands)
- [Known limitations](#known-limitations)

## What you get

- **A familiar test API**: `open`, `click`, `type_text`, assertions, waits,
  frames, windows, cookies, local storage, uploads, PDF handling, and more than
  200 helper methods.
- **Multiple browser backends**: WebDriver, Chrome DevTools Protocol (CDP), and
  Undetected Chromedriver (UC) mode, plus an optional `rustwright`-based
  Playwright-compatible engine.
- **Anti-detection tooling**: binary patching for `chromedriver` and Chrome, a
  priority-ordered JavaScript evasion registry, fingerprint presets, and
  CDP-level spoofing.
- **Low-code and migration tools**: JSON scenario runner, Python-to-Rust
  importer, Selenium IDE parser, Gherkin runner, and a recorder that emits Rust
  tests.
- **Operational helpers**: `sbase` CLI, shell completions, Docker image,
  Twelve-Factor `SB_*` configuration, structured tracing, and cloud artifact
  uploads.

## Quick start

```bash
# Run a UC-mode smoke test against seleniumbase.io
cd rust-port
cargo run --bin sbase -- --uc open https://seleniumbase.io

# Or assert on title from the CLI
cargo run --bin sbase -- --uc smoke https://seleniumbase.io --title-contains SeleniumBase
```

## Installation

Add the crate to a Cargo project:

```bash
cargo add seleniumbase-rs
```

Or add it manually to `Cargo.toml`:

```toml
[dependencies]
seleniumbase-rs = "0.1"
```

Optional features are enabled in `Cargo.toml`:

```toml
[dependencies]
seleniumbase-rs = { version = "0.1", features = ["playwright", "s3", "azure", "gcp", "mcp-server"] }
```

## Architecture at a glance

```text
┌─────────────────────────────────────────────────────────────┐
│  Test API / CLI / Examples / MCP server / Recorder / Behave  │
├─────────────────────────────────────────────────────────────┤
│  BaseCase  ──►  capability traits (BrowserApi, ElementApi,   │
│                 AssertionApi, ScreenshotApi)                 │
├─────────────────────────────────────────────────────────────┤
│  BrowserSession  ──►  thirtyfour WebDriver + CDP session     │
├─────────────────────────────────────────────────────────────┤
│  Stealth layer: patcher, evasion registry, fingerprint       │
│  presets, CDP overrides, engine-spoofing args                │
├─────────────────────────────────────────────────────────────┤
│  Driver: chromedriver / geckodriver / Playwright engine      │
└─────────────────────────────────────────────────────────────┘
```

The implementation of `BaseCase` is split across `src/api/base_case.rs` (the
struct, constructors, and core helpers) and focused domain files under
`src/api/base_case_impls/` (DOM, mouse, alerts, browser introspection,
navigation, storage, PDF, tours, charts, and more). See the
[Developer Guide](./docs/DEVELOPER_GUIDE.md) for how to add a new helper.

## Feature overview

### BaseCase API

`BaseCase` is the single entry point for tests. It wraps a `BrowserSession` and
exposes capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`,
`ScreenshotApi`) so helpers can depend on behavior rather than the concrete type.

Core groups of methods include:

| Group | Examples |
|-------|----------|
| Navigation | `open`, `refresh`, `go_back`, `go_forward`, `get_current_url`, `get_title` |
| Element interaction | `click`, `type_text`, `clear`, `submit`, `hover`, `double_click`, `context_click`, `drag_and_drop` |
| JavaScript execution | `execute_script`, `execute_async_script`, `js_click`, `js_type`, `set_attribute`, `remove_attribute` |
| Waits | `wait_for_element_present`, `wait_for_element_visible`, `wait_for_element_clickable`, `wait_for_text`, `wait_for_ready_state_complete` |
| Assertions | `assert_title`, `assert_text`, `assert_element`, `assert_attribute`, `assert_no_404_errors`, `assert_no_js_errors` |
| Queries | `is_element_present`, `is_element_visible`, `is_element_enabled`, `is_text_visible`, `get_text`, `get_attribute`, `get_property` |
| Frames and windows | `switch_to_frame`, `switch_to_default_content`, `switch_to_window`, `switch_to_new_window`, `maximize_window`, `set_window_size` |
| Storage | `add_cookie`, `get_cookie`, `delete_all_cookies`, `set_local_storage_item`, `get_local_storage_item`, `clear_local_storage` |
| Scroll | `scroll_to`, `scroll_to_top`, `scroll_to_bottom`, `smooth_scroll_to`, `scroll_by_y` |
| Alerts | `accept_alert`, `dismiss_alert`, `type_alert_text`, `get_alert_text` |
| Uploads | `choose_file` |
| MFA/TOTP | `get_totp_code` |
| PDF | `save_as_pdf`, `get_pdf_text`, `assert_pdf_text` |
| HTML parsing | `soup_find`, `soup_find_all`, `get_beautiful_soup_object` |

Most methods are `async`, return `Result<T, SeleniumBaseError>`, and many
automatically wait for the target element to be present and visible before
acting.

### Browser modes

`BrowserConfig::mode` selects how the browser is driven:

| Mode | Use when |
|------|----------|
| `DriverMode::WebDriver` | Standard WebDriver automation against `chromedriver` or a Selenium Grid. |
| `DriverMode::Cdp` | You want direct CDP access for network/cache/headers or lower-level control. |
| `DriverMode::Uc` | The site blocks normal WebDriver traffic and you need anti-detection evasions. |

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true)
    .with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...");
```

### Stealth and fingerprinting

UC mode and the standalone fingerprint system work together:

1. `ChromedriverPatcher` strips `cdc_` and `__webdriver` markers from the driver
   executable before launch.
2. `EnginePatch::balanced()` and `engine_spoofing_args()` disable automation
   telemetry at the Chromium launch level.
3. `Fingerprint` presets set coherent user agent, platform, screen size, WebGL
   vendor, timezone, geolocation, and other signals.
4. `EvasionProvider`s inject JavaScript to mask `navigator.webdriver`, plugins,
   permissions, canvas/audio noise, WebRTC, and more.
5. `native_spoofing` moves whatever it can out of JavaScript and into CDP
   (`Network.setUserAgentOverride`, `Emulation.setDeviceMetricsOverride`, etc.).

See the [Fingerprint & Stealth Profiles](./docs/tutorials/fingerprint_stealth.md)
and [Binary Patching](./docs/tutorials/binary_patching.md) tutorials for the full
recipe.

### Recorder and low-code scenarios

Record interactions from the CLI:

```bash
cargo run --bin sbase -- recorder --output my_test.rs
```

Or write a JSON scenario and run it:

```bash
cargo run --bin sbase -- run-scenario --file ./scenario.json --dashboard ./report.html
```

Scenario files support navigation, clicks, typing, assertions, waits, hover,
selects, drag-and-drop, frames, alerts, storage operations, JavaScript actions,
and more. The runner writes an HTML dashboard summarizing passed and failed
steps.

## CLI highlights

The `sbase` binary supports one-off browser commands, admin utilities, code
generation, and shell completions.

```bash
# Browser one-offs
cargo run --bin sbase -- --uc open https://seleniumbase.io
cargo run --bin sbase -- --uc smoke https://seleniumbase.io --title-contains SeleniumBase
cargo run --bin sbase -- screenshot
cargo run --bin sbase -- save-source

# Assertions and waits
cargo run --bin sbase -- open https://seleniumbase.io
cargo run --bin sbase -- assert-element --css "body"
cargo run --bin sbase -- wait-for-text --css "body" --text "SeleniumBase" --timeout 15

# Admin / binary patching
cargo run --bin sbase -- patch-chromedriver --path /path/to/chromedriver
cargo run --bin sbase -- doctor

# Shell completions
cargo run --bin sbase -- completions bash > sbase.bash
cargo run --bin sbase -- completions zsh > _sbase

# Interactive commander TUI
cargo run --bin sbase -- commander
```

See [CLI Usage](./docs/tutorials/cli_usage.md) for every command and option.

## Examples

| Example | Command |
|---------|---------|
| Basic test | `cargo run --example basic_test` |
| Basic snippets | `cargo run --example basic_snippets` |
| Selectors | `cargo run --example selectors` |
| Waits & assertions | `cargo run --example waits_assertions` |
| UC stealth | `cargo run --example uc_stealth` |
| CDP mode | `cargo run --example cdp_mode` |
| Shadow DOM | `cargo run --example shadow_dom` |
| Stealth options | `cargo run --example stealth_options` |
| Engine patching | `cargo run --example engine_patching` |
| Macros demo | `cargo run --example macros_demo` |
| Recorder | `cargo run --bin sbase -- recorder --output my_test.rs` |
| Screenshots & source | `cargo run --example screenshots` |
| PDF parsing | `cargo run --example pdf_example` |
| Cookies & storage | `cargo run --example cookies_storage` |
| JS execution | `cargo run --example js_execution` |
| Network conditions | `cargo run --example network_conditions` |
| GUI automation | `cargo run --example gui_automation` |
| Native dialogs | `cargo run --example dialog` |
| HTML inspector | `cargo run --example html_inspector` |
| MasterQA | `cargo run --example masterqa` |
| Tour maker | `cargo run --example tour_maker` |
| Chart maker | `cargo run --example chart_maker` |
| Cloud upload | `cargo run --example cloud_upload --features s3` |
| Playwright mode | `cargo run --example playwright_mode --features playwright` |
| TOTP | `cargo run --example totp_login` |
| Behave / Gherkin | `cargo run --example behave_feature` |
| Settings config | `cargo run --example settings_config` |
| Selenium IDE parsing | `cargo run --example selenium_ide` |
| Browser test lifecycle | `cargo run --example browser_test_runner` |

See also [`examples/tauri-profile-manager`](./examples/tauri-profile-manager) for a desktop multi-profile browser manager with a profile-compatible REST API.

### CDP page automation

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Cdp,
        ..Default::default()
    }).await?;
    sb.open("https://seleniumbase.io").await?;
    let text = sb.get_text("h1").await?;
    println!("{text}");
    sb.quit().await?;
    Ok(())
}
```

For raw CDP commands, call `execute_cdp` or `execute_cdp_with_params` after
`activate_cdp_mode()`.

### Shadow DOM piercing

```rust
sb.shadow_click("my-app ::shadow button").await?;
sb.shadow_type("my-app ::shadow input", "hello").await?;
```

### Native dialogs

```rust
sb.show_message("Welcome", "Welcome to the SeleniumBase Rust demo!");
if sb.show_confirm("Continue?", "Do you want to continue?") {
    let result = sb.show_prompt("Your name", "What is your name?", Some("guest"));
    let name = result.text.unwrap_or_else(|| "guest".to_string());
    sb.show_message("Hello", &format!("Hello, {name}!"));
}
```

### HTML Inspector

```rust
let inspection = sb.inspect_html().await?;
assert!(inspection.is_clean(), "{inspection:?}");
```

### GUI automation

```rust
sb.gui_click_x_y(100, 200)?;
sb.gui_write("hello")?;
sb.gui_press_keys(&["command", "a"])?;
```

### Run JSON scenario and generate dashboard

```bash
cargo run --bin sbase -- run-scenario --file ./scenario.json
```

Example `scenario.json`:

```json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "assert_element", "css": "body"},
    {"action": "wait_for_text", "css": "body", "text": "SeleniumBase", "timeout": 15}
  ]
}
```

### Cloud artifact uploads

Upload screenshots or logs to S3, Azure Blob Storage, or Google Cloud Storage.

```bash
# S3
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
cargo run --example cloud_upload --features s3

# Azure Blob Storage (SAS URL with write permission)
export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/myblob?sv=...&sig=..."
cargo run --example cloud_upload --features azure

# Google Cloud Storage
export GCS_ACCESS_TOKEN=$(gcloud auth print-access-token)
cargo run --example cloud_upload --features gcp
```

See [`examples/cloud_upload.rs`](./examples/cloud_upload.rs) for the full snippet.

### Playwright mode (optional feature)

Playwright-compatible stealth mode is available behind the `playwright` feature.
It uses [`rustwright`](https://github.com/Skyvern-AI/rustwright)
native Rust CDP engine, so it does **not** require a Node Playwright driver.
`rustwright` discovers or downloads a Chromium build on first launch. The
feature is disabled by default so the main build does not pull the engine:

```bash
cargo run --example playwright_mode --features playwright
```

If Chromium download fails, use the default CDP or UC modes instead.

### MCP server (optional feature)

Build the stdio MCP server:

```bash
cargo build --release --bin seleniumbase-mcp --features mcp-server
```

Configure an MCP client with the absolute path to the built binary:

```json
{
  "mcpServers": {
    "seleniumbase": {
      "command": "/absolute/path/to/target/release/seleniumbase-mcp",
      "args": []
    }
  }
}
```

The browser starts lazily when the first browser tool runs, so clients can list
tools without a running WebDriver. The default configuration connects to the
WebDriver endpoint at `http://localhost:4444`.

| Tool | Purpose |
|------|---------|
| `open_url` | Open a URL |
| `get_title` | Read the page title |
| `get_url` | Read the current URL |
| `click` | Click a CSS selector |
| `type_text` | Enter text into a CSS selector |
| `get_text` | Read visible element text |
| `assert_text` | Check element text |
| `execute_script` | Execute JavaScript in the page |
| `screenshot` | Save a screenshot of the current page |
| `patch_chromedriver` | Patch a chromedriver binary to remove automation markers |
| `list_engine_spoofing_args` | Return Chromium flags that reduce engine-level fingerprints |
| `list_fingerprint_presets` | Return the names of built-in fingerprint presets |
| `build_fingerprint` | Build a `Fingerprint` from a named preset |
| `get_stealth_bootstrap_script` | Return the JavaScript evasion bootstrap for a preset |
| `list_macros` | Return the names of convenience macros exported by the crate |
| `quit` | Close the browser session |

Only connect trusted MCP clients. The server can control the browser and
execute JavaScript in the active page.

## Feature flags

| Feature | Purpose |
|---------|---------|
| `playwright` | `rustwright` Playwright-compatible engine. |
| `s3` | AWS S3 artifact uploads. |
| `azure` | Azure Blob Storage artifact uploads. |
| `gcp` | Google Cloud Storage artifact uploads. |
| `mcp-server` | Builds the `seleniumbase-mcp` binary using `rmcp`. |
| `full-tracing` | Verbose `tracing` span events for debugging. |
| `json-logs` | Structured JSON log output. |
| `error-backtrace` | Backtraces attached to `SeleniumBaseError`. |

## Documentation

- [Developer Guide](./docs/DEVELOPER_GUIDE.md)
- [Extended Documentation](./DOCS.md)
- [Rust Book Source](./docs/README.md)
- [Why Rust?](./docs/why-rust.md)
- [Python Migration](./docs/python-migration.md)
- [Rust Test Tooling](./docs/rust-test-tooling.md)

### Tutorials

- [Getting Started](./docs/tutorials/getting_started.md)
- [Selectors](./docs/tutorials/selectors.md)
- [Waits and Assertions](./docs/tutorials/waits_assertions.md)
- [Shadow DOM](./docs/tutorials/shadow_dom.md)
- [CDP Mode](./docs/tutorials/cdp_mode.md)
- [Undetected (UC) Mode](./docs/tutorials/uc_mode.md)
- [Fingerprint & Stealth Profiles](./docs/tutorials/fingerprint_stealth.md)
- [Browser Profiles](./docs/tutorials/browser_profiles.md)
- [Recorder Mode](./docs/tutorials/recorder_mode.md)
- [GUI Automation](./docs/tutorials/gui_automation.md)
- [MasterQA](./docs/tutorials/masterqa.md)
- [Tours](./docs/tutorials/tours.md)
- [Charts](./docs/tutorials/charts.md)
- [MFA / TOTP](./docs/tutorials/mfa_totp.md)
- [PDF Parsing](./docs/tutorials/pdf_parsing.md)
- [Test Translations](./docs/tutorials/translations.md)
- [CLI Usage](./docs/tutorials/cli_usage.md)
- [API Reference](./docs/tutorials/api_reference.md)
- [Macros](./docs/tutorials/macros.md)
- [Cloud Integrations](./docs/tutorials/cloud_integrations.md)
- [Settings and Configuration](./docs/tutorials/settings_and_config.md)
- [Behave / Gherkin Support](./docs/tutorials/behave.md)
- [Selenium IDE Migration](./docs/tutorials/selenium_ide.md)
- [Remaining Helpers](./docs/tutorials/remaining_helpers.md)
- [Binary Patching](./docs/tutorials/binary_patching.md)

### Help pages

- [Customizing Test Runs](./docs/help/customizing_test_runs.md)
- [Syntax Formats](./docs/help/syntax_formats.md)
- [Commander TUI](./docs/help/commander.md)
- [Recorder CLI](./docs/help/recorder_cli.md)
- [Playwright Mode](./docs/help/playwright_mode.md)
- [Docker Guide](./docs/help/docker.md)
- [HTML Inspector](./docs/help/html_inspector.md)
- [Common Problems](./docs/help/common_problems.md)
- [ABI & API Stability](./docs/ABI_API.md)

### Build the documentation book

```bash
cargo install mdbook
mdbook serve --open
```

### Import Python tests

```bash
cargo run --bin sbase -- import-python tests/login_test.py \
  --output tests/login_test.rs
```

The static importer handles common SeleniumBase and Selenium WebDriver calls.
Unsupported or dynamic Python remains visible as diagnostics and `TODO`
comments for manual review.

## Project status

The initial Rust port plan is complete. The crate builds and tests cleanly on
stable Rust with all feature flags enabled. Work is continuously pushed to two
locations:

- Monorepo feature branch: `MustCodeAl/SeleniumBase/mustcodeal-rust-port`
- Crate-only orphan branch: `MustCodeAl/seleniumbase-rs/main`

### Why use the Rust crate?

Rust provides compile-time type checking, explicit error handling, controlled
concurrency, and native CLI distribution. Those properties can make large test
harnesses easier to refactor and operate reliably. Browser and network work
still dominate many end-to-end tests, so the project does not claim universal
speedups over Python or JavaScript. See [Why Rust?](./docs/why-rust.md) for the
tradeoffs and measurement guidance.

## Verified commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing -- -D warnings
cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing
mdbook build
```

On macOS with Python 3.14 from [mise](https://mise.jdx.dev), the `playwright`
feature tests need the Python library path exported:

```bash
export DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib:$DYLD_LIBRARY_PATH"
```

## Known limitations

`cargo publish --dry-run` currently fails because the `playwright` feature
depends on `rustwright` from a Git tag (`v0.2.0`) that is newer than the
version published on crates.io (`0.1.1`). Publishing requires an upstream
`rustwright 0.2.0` release or replacing the git dependency with a published
crate.
