# ABI & API Stability

This page describes the public API surface of `seleniumbase-rs`, the stability
guarantees that apply, and how to integrate the crate with foreign code.

## Public modules

The crate root exposes the following modules:

| Module | Purpose |
|---|---|
| `api` | `BaseCase`, capability traits, tours, charts, recorder, scenario runner, CDP page/driver helpers. |
| `artifacts` | Screenshot, page source, and log artifact paths. |
| `behave` | Gherkin/BDD parser, step registry, and runner. |
| `browser` | `BrowserConfig`, `BrowserSession`, driver launching, and optional Playwright session. |
| `cli` | `sbase` command-line script implementations. |
| `common` | Decorators, obfuscation, and shared exception types. |
| `config` | Settings, proxy lists, and ad-block lists. |
| `core` | Logging, reporting, download, and session helpers. |
| `error` | `SeleniumBaseError` and the `Result<T>` alias. |
| `js_code` | JavaScript snippets injected by CDP and WebDriver. |
| `macros` | `#[macro_export]` convenience macros (also re-exported at the crate root). |
| `profile_payloads` | External browser profile payload parsing and conversion. |
| `plugins` | Cloud/logging plugin interfaces. |
| `resources` | Static assets bundled with the crate. |
| `stealth` | Undetected-chrome options, evasions, the evasion **provider registry**, fingerprint profiles, humanization helpers, binary patcher, and CDP reactor. |
| `utilities` | Selenium IDE, Grid, and Python-to-Rust importer. |
| `utils` | Selectors, shadow DOM helpers, translations, and extension builders. |

## Crate-root re-exports

The most common types are re-exported from the crate root for convenience:

| Type | Re-exported from crate root |
|---|---|
| `BaseCase` | yes |
| `BrowserConfig`, `Browser`, `DriverMode`, `BrowserSession` | yes |
| `Selector` | yes |
| `Fingerprint`, `StealthFlags` | yes |
| `BatteryProfile`, `ConnectionProfile`, `SpeechVoice`, `BrandVersion`, `ClientHints`, `HumanizeConfig`, `CoherenceReport` | yes |
| `EvasionProvider`, `EvasionContext`, `EvasionConfig`, `EvasionRegistry`, `default_registry` | yes |
| `ChromedriverPatcher`, `ChromeBinaryPatcher`, `EnginePatch`, `engine_spoofing_args` | yes |
| `Chart`, `ChartType`, `TourTheme`, `Gui` | yes |
| `run_browser_test`, `BrowserTestFuture` | yes |
| `AssertionApi`, `BrowserApi`, `ElementApi`, `ScreenshotApi` | yes |
| `Result<T>`, `SeleniumBaseError`, `ResultExt` | yes |
| `import_python`, `ImportDiagnostic`, `ImportOptions`, `ImportResult`, `ImportSeverity`, `PythonSource` | yes |

`#[macro_export]` macros such as `selector!`, `sb_test!`, `sb_open!`, and
`fingerprint!` are also imported from the crate root:

```rust
use seleniumbase_rs::{selector, sb_test, sb_open, fingerprint};
```

## Capability traits

The public API is organized into capability traits in `api::traits`. These
traits are implemented by `BaseCase` and can be used to write generic helpers:

| Trait | Responsibility |
|---|---|
| `BrowserApi` | Navigation and lifecycle: `open`, `quit`, `refresh`, `go_back`, `go_forward`, `get_title`, `get_url`. |
| `ElementApi` | Finding and interacting with elements: `find_element`, `click`, `double_click`, `type_text`, `get_text`, `get_attribute`. |
| `AssertionApi` | Test assertions: `assert_title`, `assert_element`, `assert_text_visible`, `assert_no_js_errors`. |
| `ScreenshotApi` | Screenshot capture: `save_screenshot`, `screenshot_as_png`. |

Use these traits when writing helpers that should work with any type exposing
the same capability, or as a blueprint for adding new cross-cutting concerns:

```rust
use seleniumbase_rs::{AssertionApi, ElementApi};

async fn assert_logged_in<E>(sb: &mut E) -> Result<(), seleniumbase_rs::SeleniumBaseError>
where
    E: ElementApi + AssertionApi,
{
    sb.assert_element("#dashboard").await?;
    sb.assert_text("#user-name", "Alice").await
}
```

## ABI note

Rust does not have a stable application binary interface. Do not pass
`seleniumbase-rs` types across a dynamic-library boundary or assume a fixed
memory layout. If you need to drive the crate from another language, write a
small `extern "C"` shim that accepts C-compatible arguments and internally
builds a `BaseCase` or calls the Rust API:

```rust,ignore
#[no_mangle]
pub extern "C" fn sb_open_url(url: *const c_char) {
    let url = unsafe { CStr::from_ptr(url).to_string_lossy() };
    // spawn runtime, create BaseCase, etc.
}
```

The default toolchain, target, and compiler version can all change the layout
of public structs. Treat the Rust API as source-compatible only.

## Versioning policy

`seleniumbase-rs` follows [Semantic Versioning](https://semver.org/):

* Patch releases fix bugs without changing the public API.
* Minor releases add functionality in a backward-compatible way.
* Major releases may break the public API.

Because the crate is pre-1.0, minor releases (`0.x`) may contain breaking
changes. Pin to an exact version or a narrow range in production.

## Feature flags

The crate uses Cargo features to keep heavy dependencies off by default:

| Feature | Effect | Default |
|---|---|---|
| `playwright` | Enables the `rustwright`-backed Playwright-compatible driver. | no |
| `s3` | Enables AWS S3 artifact uploads. | no |
| `azure` | Enables Azure Blob Storage artifact uploads. | no |
| `gcp` | Enables Google Cloud Storage artifact uploads. | no |
| `mcp-server` | Builds the `seleniumbase-mcp` binary using `rmcp`. | no |
| `full-tracing` | Enables `tracing-timing` histograms and `tracing-actix` actor instrumentation. | no |
| `error-backtrace` | Reserved for future backtrace capture on every `SeleniumBaseError`. | no |

When adding feature-gated code, use `#[cfg(feature = "...")]` and declare the
dependency as `optional = true` in `Cargo.toml`.

## Error design

`SeleniumBaseError` is a rich, structured error enum:

* **Element errors** carry the selector and detected strategy:
  `ElementNotFound`, `ElementNotInteractable`, `StaleElement`,
  `InvalidSelector`.
* **Lifecycle errors** carry the binary path or URL:
  `BrowserLaunch`, `BrowserDisconnected`, `Navigation`, `SessionNotStarted`.
* **Subsystem errors** give context without string parsing:
  `Patcher`, `Stealth`, `Network`, `Download`, `Screenshot`, `Pdf`,
  `CdpDriver`, `Playwright`, `Mcp`, `PythonMigration`.

Every error exposes:

* `category()` — a stable snake_case tag for dashboards.
* `is_transient()` — `true` for retryable network/disconnect/timeout failures.
* `is_skipped()` — `true` for the `Skipped` variant.
* `hint()` — a one-sentence remediation hint when available.
* `log()` / `log_in_context(op)` — structured `tracing::error!` events.

Use the `ResultExt` trait to attach context:

```rust
use seleniumbase_rs::{Result, ResultExt};

fn read_config(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .sb_context(format!("loading config from {path}"))
}
```

## What is not covered

Internal helper modules, `#[doc(hidden)]` items, and items under
`api::base_case_impls` are implementation details. They may change without a
major version bump. Rely only on the public modules and re-exports listed
above.

## API stability checklist for consumers

- [ ] Import types from the crate root or documented public modules.
- [ ] Avoid relying on paths inside `api::base_case_impls` or `#[doc(hidden)]` items.
- [ ] Pin the crate version in production.
- [ ] Do not pass `seleniumbase-rs` types across FFI boundaries without a C shim.
- [ ] Test feature-gated code with the same feature set you ship.
