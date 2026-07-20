# Developer Guide

This guide is for anyone who wants to understand, extend, or maintain the Rust
port of SeleniumBase (`seleniumbase-rs`).

## Project goals

`seleniumbase-rs` aims to provide a Rust-native browser-automation and end-to-end
testing framework with parity to the Python SeleniumBase API. The public surface
is centered around `BaseCase`, a stateful test object that manages a browser
session, configuration, recorder, deferred assertions, tours, charts,
presentations, and optional stealth/CDP integrations.

## Repository layout

```text
rust-port/
├── Cargo.toml              # Crate manifest (optional cloud/Playwright/MCP features)
├── README.md               # User-facing quick-start and feature index
├── DOCS.md                 # Overview of major features
├── docs/                   # Tutorials and help pages
│   ├── tutorials/          # Step-by-step guides
│   └── DEVELOPER_GUIDE.md  # This file
├── examples/               # Runnable example programs
├── src/
│   ├── lib.rs              # Crate root, public re-exports
│   ├── api/                # Public test API
│   │   ├── base_case.rs    # BaseCase struct and core inherent impl
│   │   ├── base_case_impls/# Per-domain BaseCase method modules
│   │   ├── chart.rs        # Chart generation
│   │   ├── tour.rs         # Guided tour builder
│   │   ├── presentation.rs # HTML presentation builder
│   │   ├── deferred.rs     # Deferred assertion state
│   │   ├── recorder.rs     # Action recorder
│   │   ├── html.rs         # BeautifulSoup-style HTML parsing
│   │   ├── pdf.rs          # PDF helpers
│   │   ├── traits.rs       # Capability traits (BrowserApi, ElementApi, ...)
│   │   └── ...
│   ├── browser/            # Browser launch, session, and configuration
│   │   ├── config.rs       # BrowserConfig, Browser, DriverMode
│   │   ├── session.rs      # BrowserSession (WebDriver wrapper)
│   │   ├── launcher.rs     # WebDriver/chromedriver launching
│   │   └── playwright.rs   # Optional Playwright-backed session
│   ├── stealth/            # Stealth / undetected automation helpers
│   │   ├── dprocess.rs     # Detached browser/driver process management
│   │   ├── options.rs      # StealthOptions builder
│   │   └── reactor.rs      # CDP Fetch interceptor
│   ├── behave/             # Gherkin/BDD runner and step registry
│   ├── cli/                # Command-line tooling (sbase binary)
│   ├── common/             # Decorators, obfuscation, exceptions
│   ├── config/             # Settings and proxy/ad-block lists
│   ├── core/               # Logging, reporting, download/session helpers
│   ├── js_code/            # JavaScript snippets injected into pages
│   ├── plugins/            # Cloud/logging plugin interfaces
│   ├── utilities/          # Selenium IDE parser, Selenium Grid helpers
│   ├── utils/              # Selectors, shadow DOM, translations, extensions
│   └── bin/                # Additional binary targets (MCP server, ...)
└── tests/                  # Integration tests
```

## BaseCase architecture

`BaseCase` is a single struct that owns the test state:

```rust
pub struct BaseCase {
    session: BrowserSession,
    config: BrowserConfig,
    recorder: Arc<Mutex<ActionRecorder>>,
    tour: Option<Tour>,
    deferred: DeferredAsserts,
    presentation: Option<Presentation>,
    chart: Option<Chart>,
    qa_session: Option<MasterQaSession>,
    #[cfg(feature = "playwright")]
    playwright_session: Option<PlaywrightSession>,
    time_limit_secs: Option<u64>,
    gui_held: Option<(i32, i32)>,
}
```

The inherent implementation is split across `src/api/base_case.rs` (core
constructors and commonly used methods) and `src/api/base_case_impls/*.rs`
(domain-specific helpers). Each included file is an `impl BaseCase { ... }`
block; they rely on the imports declared at the top of `base_case.rs`.

### Adding a new `BaseCase` helper

1. Choose the appropriate domain file in `src/api/base_case_impls/` (or create
   a new one if the domain does not exist).
2. Add the method inside the existing `impl BaseCase { ... }` block.
3. Return `crate::Result<T>` (an alias for `Result<T, SeleniumBaseError>`).
4. Record the action with `self.record(...)` when it represents a user-facing
   test step.
5. Add a unit test when the helper does not require a live browser.
6. Run `cargo clippy --all-targets -- -D warnings` and `cargo test`.

Example:

```rust
impl BaseCase {
    /// Asserts that `css` matches exactly `count` elements.
    pub async fn assert_element_count(
        &self,
        css: &str,
        count: usize,
    ) -> crate::Result<()> {
        let elements = self.find_elements(css).await?;
        if elements.len() == count {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "expected {count} elements for '{css}', found {}",
                elements.len()
            )))
        }
    }
}
```

## Capability traits

The public API is also exposed through capability traits in
`src/api/traits.rs`:

- `BrowserApi` - navigation and lifecycle
- `ElementApi` - element finding and interaction
- `AssertionApi` - test assertions
- `ScreenshotApi` - screenshot capture

These traits are implemented for `BaseCase`. Prefer them when writing helpers
that should work with any type exposing the same capability, and use them as a
blueprint when adding new cross-cutting concerns.

## Error handling

All fallible operations return `crate::Result<T>`, defined as:

```rust
pub type Result<T> = std::result::Result<T, SeleniumBaseError>;
```

`SeleniumBaseError` is a `thiserror` enum covering WebDriver errors, I/O,
JSON, invalid configuration, assertions, CDP driver failures, GUI input, and
Playwright errors. Use `?` to propagate errors and add context in user-facing
messages.

## Feature flags

The crate uses Cargo features to keep heavy or optional dependencies off by
default:

- `playwright` - Enables `padamson/playwright-rust` integration.
- `s3` - Enables AWS S3 artifact uploads.
- `azure` - Enables Azure Blob Storage artifact uploads.
- `gcp` - Placeholder for Google Cloud integrations.
- `mcp-server` - Builds the `seleniumbase-mcp` binary using `rmcp`.

When adding a feature-gated API, use `#[cfg(feature = "...")]` and declare the
dependency as `optional = true` in `Cargo.toml`.

## Browser session model

`BrowserSession` wraps a `thirtyfour::WebDriver` and exposes higher-level
methods such as `wait_for_element`, `text`, `click`, and `execute_script`. Most
`BaseCase` helpers delegate to the session. For CDP-only operations (such as
activating a stealth driver), `CdpDriver` and `CdpPage` provide a lightweight
Chrome DevTools Protocol client.

## Stealth and undetected automation

Stealth logic lives under `src/stealth/`:

- `StealthOptions` builds launch arguments and preferences for Chromium.
- `dprocess` discovers and launches detached chromedriver/browser processes.
- `reactor` runs a background CDP `Fetch` interceptor for header and response
  overrides.

Use these through `BaseCase::activate_cdp_mode(url)` or by setting
`DriverMode::Uc` in `BrowserConfig`.

## Testing guidelines

- Add unit tests for pure helper logic (selectors, translations, HTML parsing,
  decorators, JS builders).
- Avoid tests that require a live browser in the default suite; mark them with
  `#[ignore]` or place them under `tests/`.
- Use `BaseCase::without_session(config)` to construct a `BaseCase` for testing
  helpers that do not touch the browser.
- Run the full verification matrix before pushing:

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --features s3,azure,gcp,playwright,mcp-server
cargo test --features s3,azure,gcp,playwright,mcp-server
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings
cargo publish --dry-run
```

## MCP server

The `seleniumbase-mcp` binary exposes a subset of `BaseCase` through the Model
Context Protocol. It is built when the `mcp-server` feature is enabled. See
`src/bin/mcp_server.rs` for the tool definitions and `README.md` for client
configuration examples.

## Publishing

The crate is published from the crate-only tree (`seleniumbase-rs/main`). All
dependencies must come from crates.io (no git-only dependencies). Run
`cargo publish --dry-run` before tagging a release.

## Contributing

1. Keep the public API backward-compatible when possible.
2. Follow the existing module organization.
3. Write doc comments for public items.
4. Update `README.md`, `DOCS.md`, and `docs/tutorials/` when adding user-facing
   features.
5. Run `cargo fmt` before committing.
