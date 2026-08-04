# SeleniumBase Rust Port — Copilot / AI Agent Guide

This file is the single source of truth for AI agents working on the Rust port (`rust-port/`). Read it first before making changes.

## What this project is

A Rust port of the Python [SeleniumBase](https://github.com/seleniumbase/SeleniumBase) testing framework. It provides a `BaseCase` API, undetected/stealth browser modes, CDP automation, an `sbase` CLI, an MCP server, and migration tools.

## Quick verification commands

Run these after any non-trivial change:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings
cargo test --features mcp-server
mdbook build   # from rust-port/docs if book.toml exists
```

## Layout

| Directory | Purpose |
|-----------|---------|
| `src/api/` | `BaseCase` and capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`). |
| `src/api/base_case_impls/` | Modular `BaseCase` method implementations. |
| `src/browser/` | `BrowserConfig`, `BrowserSession`, `PlaywrightSession`, launcher/downloader. |
| `src/stealth/` | Anti-detection: CDP wrappers, evasions, fingerprint profiles, binary patcher, reactor. |
| `src/profile_payloads/` | External browser profile JSON parsing and conversion. |
| `src/macros.rs` | User-facing macros (`selector!`, `sb_test!`, `sb_click!`, `fingerprint!`, etc.). |
| `src/behave/` | Gherkin/BDD runner and async step registry. |
| `src/utilities/` | Python importer, Selenium IDE parser, Grid helpers. |
| `src/bin/mcp_server.rs` | `seleniumbase-mcp` MCP server (gated by `mcp-server` feature). |
| `src/cli/bin/sbase.rs` | Main `sbase` CLI binary. |
| `docs/` | mdBook source. Add new tutorials/help pages to `SUMMARY.md`. |
| `examples/` | Runnable examples. Add new examples to `Cargo.toml` automatically via filename. |

## Conventions

- **Error type**: `crate::Result<T>` is an alias for `Result<T, SeleniumBaseError>`.
- **Async runtime**: Tokio. Tests use `#[tokio::test]`.
- **Selectors**: Use `crate::Selector` (CSS, XPath, Id, LinkText, PartialLinkText).
- **Stealth additions**: Add JS evasions to `src/stealth/evasions.rs`, flags to `src/stealth/fingerprint.rs`, launch args to `src/stealth/options.rs` or `src/stealth/patcher.rs::engine_spoofing_args`, and expose via crate root if user-facing.
- **Macros**: `#[macro_export]` so they live at the crate root. Import as `use seleniumbase_rs::{selector, sb_click, ...};`.
- **MCP tools**: Add the tool schema to `tools()` and the handler to `call_tool()` in `src/bin/mcp_server.rs`.
- **Documentation**: Every public API needs a rustdoc example. Every new feature needs a tutorial or help page and an entry in `docs/SUMMARY.md`.
- **Feature flags**: `s3`, `azure`, `gcp`, `playwright`, `mcp-server`. Keep default builds minimal.

## Common gotchas

- `rustwright` is a git dependency. `cargo publish --dry-run` will fail unless the crate-only branch uses a crates.io version or drops the `playwright` feature.
- `BrowserConfig` no longer derives `Eq` because it contains `Fingerprint` with `f64` fields.
- `rand` 0.10 uses `RngExt` for `random_range`; do not switch to `rand::Rng`.
- The `macros` module is re-exported through `src/lib.rs` as `pub mod macros;`, but macro-exported macros are at the crate root.

## How to add a new feature

1. Implement in the appropriate `src/` module.
2. Add rustdoc and at least one unit test.
3. Add or update an example in `examples/`.
4. Add or update a doc page in `docs/` and link it from `README.md` and `docs/SUMMARY.md`.
5. Update MCP tools if the feature is remotely callable.
6. Run the verification commands above.
