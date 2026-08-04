# Contributing to SeleniumBase-rs

Thank you for helping make `seleniumbase-rs` a powerful, idiomatic Rust
browser-automation framework. Whether you are fixing a bug, adding a new stealth
evasion, writing a tutorial, or improving the MCP server, this guide will get
you oriented quickly.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Quick Start](#quick-start)
3. [Development Environment](#development-environment)
4. [Project Layout](#project-layout)
5. [Architecture at a Glance](#architecture-at-a-glance)
6. [Code Style](#code-style)
7. [Error Handling](#error-handling)
8. [Tracing and Observability](#tracing-and-observability)
9. [Testing](#testing)
10. [Adding a Stealth Evasion Provider](#adding-a-stealth-evasion-provider)
11. [Adding a Masking Mode or Fingerprint Field](#adding-a-masking-mode-or-fingerprint-field)
12. [Adding a Macro](#adding-a-macro)
13. [Adding an MCP Tool](#adding-an-mcp-tool)
14. [Adding an `sbase` Subcommand](#adding-an-sbase-subcommand)
15. [Adding a Profile Payload Field](#adding-a-profile-payload-field)
16. [Binary Patching](#binary-patching)
17. [Writing Documentation](#writing-documentation)
18. [Opening a Pull Request](#opening-a-pull-request)
19. [Release Process](#release-process)
20. [Licensing](#licensing)

## Code of Conduct

- Be respectful and constructive.
- Assume good intent.
- Focus feedback on code and design, not individuals.
- Do not include third-party anti-detect product names, trademarks, or proprietary
  payload examples in code, docs, or tests. Use generic terminology such as
  "external profile", "anti-detect payload", or "browser fingerprint".

## Quick Start

```bash
# Clone the workspace
git clone https://github.com/MustCodeAl/SeleniumBase.git
cd SeleniumBase/rust-port

# Run the default test suite
cargo test

# Run with all optional features enabled
cargo test --features s3,azure,gcp,playwright,mcp-server

# Check formatting and clippy
cargo fmt --all -- --check
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings

# Build the mdBook
mdbook build
```

## Development Environment

### Required

- [Rust](https://rustup.rs/) 1.80 or newer (stable channel).
- `cargo` and `rustfmt`.
- Python 3.10+ with `setuptools` if you want to run Python-side examples or use
  the importer tooling.

### Optional but recommended

- [mdBook](https://rust-lang.github.io/mdBook/) for documentation:
  `cargo install mdbook`.
- [cargo-nextest](https://nexte.st/) for faster test runs:
  `cargo install cargo-nextest`.
- `libwebkit2gtk-4.1-dev` (Linux) or Xcode (macOS) if you build the Tauri
  example.

### macOS note

If tests fail to load `libpython3.14.dylib`, set the library path before running
binaries:

```bash
export DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib"
```

## Project Layout

```text
rust-port/
├── src/
│   ├── lib.rs                 # Crate root and public re-exports
│   ├── api/                   # BaseCase implementation split by topic
│   │   ├── base_case.rs       # Core driver/session management
│   │   ├── base_case_impls/   # Focused helper groups
│   │   ├── chart.rs           # Chart generation
│   │   ├── tour.rs            # Guided tour themes
│   │   └── traits.rs          # Capability traits
│   ├── browser/               # BrowserConfig, session startup, capabilities
│   ├── stealth/               # Anti-detection layer
│   │   ├── fingerprint.rs     # Fingerprint / StealthFlags structs
│   │   ├── humanize.rs        # Bézier mouse paths + keystroke timing
│   │   ├── patcher.rs         # Chromedriver binary patching
│   │   ├── reactor.rs         # CDP Fetch interceptor
│   │   ├── dprocess.rs        # Detached driver/browser process helpers
│   │   └── providers/         # Evasion provider plugin architecture
│   ├── profile_payloads/      # External anti-detect profile JSON parsing
│   ├── macros.rs              # Public macros
│   ├── bin/mcp_server.rs      # SeleniumBase MCP server
│   ├── cli/                   # sbase command-line tool
│   ├── behave/                # Gherkin/BDD runner
│   ├── utilities/             # Python importer, IDE helpers, etc.
│   └── utils/                 # Low-level utilities
├── examples/                  # Runnable examples
├── docs/                      # mdBook sources and help pages
├── tests/                     # Additional integration tests
└── book.toml                  # mdBook configuration
```

## Architecture at a Glance

`seleniumbase-rs` is organized in layers:

1. **Driver abstraction** — `BrowserConfig`, `BrowserSession`, and the
   `BaseCase` state machine. This layer is intentionally small and mostly routes
   calls to the underlying driver (Thirtyfour / Rustwright / CDP).
2. **API layer** — Capability traits (`BrowserApi`, `ElementApi`,
   `AssertionApi`, `ScreenshotApi`) and the `base_case_impls` modules. New
   helpers should be added to the smallest applicable module and then exposed
   through the trait or `BaseCase` impl.
3. **Stealth layer** — `Fingerprint` + `StealthFlags` describe what to spoof;
   `EvasionProvider`s generate the JavaScript bootstrap; `ChromedriverPatcher`
   mutates the local driver binary; `StealthReactor` intercepts network requests
   via CDP.
4. **Tooling layer** — `sbase` CLI, the MCP server, the behave runner, and the
   Python importer.

When adding a feature, try to keep each layer independent. For example, a new
fingerprint provider should not depend on `BaseCase`, and a new CLI command
should not contain browser logic directly.

## Code Style

- Follow the Rust style enforced by `cargo fmt`.
- Keep the all-features Clippy pass clean:
  `cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings`.
- Prefer explicit types on public APIs; this helps Copilot and other agents infer intent.
- Use `tracing` for logs instead of `println!` in library code.
- Avoid `unsafe` unless absolutely necessary and clearly documented.
- Keep functions small and focused. If a `BaseCase` helper grows beyond ~50
  lines, add it to the appropriate `base_case_impls` module.
- Prefer `?` and `crate::Result<T>` over manual `match` propagation.
- Use `thiserror` for new error variants and keep variants structured (avoid
  generic string payloads).

## Error Handling

`seleniumbase-rs` uses a single `SeleniumBaseError` enum in `src/error.rs`.

- Add a new variant when a failure mode needs special handling, a unique hint,
  or stable down-stream matching.
- Use helper constructors such as `SeleniumBaseError::element_not_found` instead
  of formatting strings by hand.
- Mark transient failures (network, driver not ready) by returning `true` from
  `is_transient()`.
- Provide actionable hints via `hint()`.
- Instrument error paths with `tracing::error!` and include context with
  `ResultExt::sb_context()`:

```rust
use seleniumbase_rs::{ResultExt, SeleniumBaseError};

some_fallible_op()
    .sb_context("downloading chromedriver")?;

SeleniumBaseError::driver_not_ready("chromedriver").log();
```

See `docs/ABI_API.md` and `docs/help/common_problems.md` for the error design
philosophy.

## Tracing and Observability

The crate uses `tracing` throughout. Common patterns:

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(url = %url))]
pub async fn open(&mut self, url: &str) -> crate::Result<()> {
    info!("opening page");
    // ...
}
```

Feature flags control subscriber output:

| Feature | Effect |
|---|---|
| `full-tracing` | Enables detailed span lifecycle logging. |
| `json-logs` | Emits structured JSON logs via `json-subscriber`. |
| `error-backtrace` | Captures backtraces on `SeleniumBaseError`. |

Run the CLI with tracing enabled:

```bash
RUST_LOG=info cargo run --bin sbase --features full-tracing -- open https://example.com
```

See `docs/tutorials/tracing.md` for subscriber configuration examples.

## Testing

- Add unit tests for pure logic: selectors, fingerprint generation, macros,
  providers, JSON parsing.
- Use `#[tokio::test]` for async helpers that do not require a browser.
- Browser-backed tests should live in `examples/` or behind an integration flag
  so CI stays fast.
- When writing doctests, mark browser-dependent snippets with `no_run` or
  `ignore`.

### Verification matrix (run before every PR)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings
cargo test --features s3,azure,gcp,playwright,mcp-server
mdbook build
cargo publish --dry-run
```

On macOS, prefix the test/publish commands with:

```bash
DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib" cargo test --features s3,azure,gcp,playwright,mcp-server
```

## Adding a Stealth Evasion Provider

The stealth layer uses a provider registry. `evasions::bootstrap_script(fp)` is
a thin wrapper over the registry.

1. Open `src/stealth/providers/builtin.rs` and implement `EvasionProvider`:

```rust
use seleniumbase_rs::stealth::providers::{EvasionContext, EvasionProvider};
use seleniumbase_rs::stealth::fingerprint::masked;

pub struct MyEvasionProvider;

impl EvasionProvider for MyEvasionProvider {
    fn name(&self) -> &str {
        "my_evasion" // stable, unique, snake_case
    }

    fn priority(&self) -> i32 {
        // Lower runs earlier. native_toString is 5; navigator props ~30;
        // late/self-defense providers are 110+.
        100
    }

    fn applies(&self, fp: &crate::stealth::fingerprint::Fingerprint) -> bool {
        masked(fp.flags.navigator_masking)
    }

    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let value = EvasionContext::escape(
            ctx.fingerprint.vendor.as_deref().unwrap_or("Example"),
        );
        Some(format!("(function() {{ /* use '{value}' */ }})();"))
    }
}
```

2. Register it in `all()` at the bottom of `builtin.rs` (keep sorted by priority).
3. Wrap replaced natives so `Function.prototype.toString` still reports
   `[native code]`:

```js
obj.method = (window.__sbNative || function(f){return f;})(patched, 'method');
```

The `native_to_string` provider (priority 5) installs `window.__sbNative`
before any other provider runs.

4. Use `ctx.seed` for deterministic per-session noise (canvas, audio, WebGL).
5. Add a unit test asserting the snippet contains expected markers and that
   `applies` is gated correctly.
6. Update `docs/tutorials/fingerprint_stealth.md`.

## Adding a Masking Mode or Fingerprint Field

1. Add the field to `StealthFlags` or `Fingerprint` in
   `src/stealth/fingerprint.rs`.
2. Annotate new serde-backed fields with `#[serde(default)]`.
3. Update `balanced()` and `all_custom()` constructors.
4. Add coherence validation in `Fingerprint::validate()` if the field has
   required companions.
5. Wire the field into the launcher (`browser/session.rs`), provider scripts,
   or the reactor if applicable.
6. Document the new flag in `docs/tutorials/fingerprint_stealth.md`.

## Adding a Macro

Public macros live in `src/macros.rs` and are re-exported at the crate root.

```rust
#[macro_export]
macro_rules! sb_focus {
    ($sb:expr, $css:expr) => {{
        $sb.focus($css)
    }};
}
```

1. Add the macro with `#[macro_export]`.
2. Write a compile-time unit test under the definition.
3. Update `docs/tutorials/macros.md` and the macro table in `README.md`.

## Adding an MCP Tool

The MCP server in `src/bin/mcp_server.rs` uses the `rmcp` crate.

1. Add the tool schema to the `tools()` function.
2. Add a handler branch in `call_tool()`.
3. Convert `SeleniumBaseError` to MCP error content with `sb_error_to_mcp()`.
4. Add a unit test that exercises the tool via an in-memory client.
5. Document the tool in `docs/help/mcp_server.md` and the README tool table.

## Adding an `sbase` Subcommand

1. Add the new variant to `SbaseCommand` in `src/cli/bin/sbase.rs` with an
   idiomatic `///` description.
2. Add argument parsing in the `clap` derive macro.
3. Add a handler branch in `async fn main()`.
4. If the command launches a browser, prefer creating a `BaseCase` through
   `run_browser_test()` so cleanup is deterministic.
5. Update `docs/help/commander.md` and `README.md` if the command is user-facing.

## Adding a Profile Payload Field

External profile payloads are parsed in `src/profile_payloads/profile.rs`.

1. Add the field to `ProfileParams` (or the appropriate nested struct).
2. Map it to `BrowserConfig` or `Fingerprint` in `to_browser_config()` and
   `browser()`.
3. Add a unit test for parsing and round-tripping.
4. Update `docs/tutorials/browser_profiles.md`.

## Binary Patching

`ChromedriverPatcher` modifies a local chromedriver binary to help evade
binary-signature detection.

- Patches are defined as `EnginePatch` structs in `src/stealth/patcher.rs`.
- Each patch has a human-readable `id`, byte sequence search/replace, and
  optional marker detection.
- Add new patches to `ChromedriverPatcher::default_patches()`.
- Always test patches against the driver versions listed in
  `docs/help/binary_patching.md`.
- Backups are stored next to the binary with `.sb-backup` extension.

## Writing Documentation

- All public items should have doc comments (`///`).
- Use examples in doc comments; they run as doctests.
- User-facing tutorials go in `docs/tutorials/`.
- Reference/help pages go in `docs/help/`.
- AI-agent guidance goes in `COPILOT.md`.
- After adding a page, add it to `docs/SUMMARY.md`.
- Build the book locally before pushing: `mdbook build`.

## Opening a Pull Request

1. Branch from `mustcodeal-rust-port` unless told otherwise.
2. Keep commits focused and atomic.
3. Include the standard Copilot trailer only if Copilot assisted:
   `Co-authored-by: Copilot App <223556219+Copilot@users.noreply.github.com>`.
4. Fill out the PR description with:
   - What changed and why.
   - Verification commands that passed.
   - Any breaking API changes and migration notes.
5. Ensure CI is green.

### PR review checklist

- [ ] `cargo fmt --all -- --check` is clean.
- [ ] `cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings` is clean.
- [ ] `cargo test --features s3,azure,gcp,playwright,mcp-server` passes.
- [ ] `mdbook build` succeeds.
- [ ] New public items have doc comments.
- [ ] New features are covered by tests or examples.
- [ ] Documentation reflects the change.
- [ ] No third-party anti-detect product names or trademarks were added.

## Release Process

1. Update `CHANGELOG.md` (or the unreleased section).
2. Bump the version in `Cargo.toml`.
3. Run the full verification matrix.
4. Tag the release in the monorepo.
5. Sync the crate-only orphan branch to `MustCodeAl/seleniumbase-rs/main`:

```bash
# From the monorepo worktree on the release branch
git checkout --orphan rust-crate-only-tmp
# Remove everything except rust-port/ contents, then promote them to root
git rm -rf .
cp -R rust-port/. .
rm -rf rust-port
# Add, commit, and force-push to seleniumbase-rs/main
git add .
git commit -m "Release seleniumbase-rs X.Y.Z"
git push --force upstream-seleniumbase-rs main
```

> The crate-only branch is intentionally force-pushed because it is an
> exported, history-free view of the Rust code.

## Licensing

By contributing, you agree that your contributions will be licensed under the
same license as the project (MIT).
