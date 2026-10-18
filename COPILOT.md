# SeleniumBase Rust Port — Copilot / AI Agent Guide

This file is the single source of truth for AI agents working on this repository. Read it first before making changes.

## What this project is

A Rust port of the Python [SeleniumBase](https://github.com/seleniumbase/SeleniumBase) testing framework. It provides a `BaseCase` API, undetected/stealth browser modes, CDP automation, an `sbrs` CLI, an MCP server, and migration tools.

The experimental stealth scraping layer (wire fingerprints, TLS and HTTP/2 impersonation, formerly the `scrape` and `impersonate` features) is not in this repository: since 0.3.0 it lives in [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).

## Quick verification commands

Run these after any non-trivial change:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings
cargo test --features mcp-server
cargo deny check          # advisories, licenses, bans, sources
mdbook build              # book.toml lives at the repository root
```

The crate declares `rust-version = "1.99"`, the newest stable release when it was
last raised. Raise it deliberately and keep the `msrv` CI job and the
`Dockerfile` base image in step with it.

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
| `src/mcp/` | The MCP servers and their tools. |
| `src/cli/` | The library half of the command line (commands, scaffold, scripts). |
| `crates/cli/src/mcp_server.rs` | `sbrs-mcp` MCP server (package `seleniumbase-rs-cli`, gated by its `mcp-server` feature, on by default). |
| `crates/cli/src/mcp_install/` | `sbrs-mcp install`, `uninstall` and `clients`: registers the server with MCP clients (their config locations and formats, JSON edited as text, Codex's `config.toml`). |
| `crates/cli/src/sbrs.rs` | Main `sbrs` CLI binary (package `seleniumbase-rs-cli`), declared with the `usage` derives; `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` are generated from it. |
| `docs/` | mdBook source. Add new tutorials/help pages to `SUMMARY.md`. |
| `examples/` | Runnable examples. Add new examples to `Cargo.toml` automatically via filename. |

## Conventions

- **Error type**: `crate::Result<T>` is an alias for `Result<T, SeleniumBaseError>`.
- **Async runtime**: Tokio. Tests use `#[tokio::test]`.
- **Selectors**: Use `crate::Selector` (CSS, XPath, Id, LinkText, PartialLinkText).
- **Stealth additions**: Add JS evasions to `src/stealth/evasions.rs`, flags to `src/stealth/fingerprint.rs`, launch args to `src/stealth/options.rs` or `src/stealth/patcher.rs::engine_spoofing_args`, and expose via crate root if user-facing.
- **Macros**: `#[macro_export]` so they live at the crate root. Import as `use seleniumbase_rs::{selector, sb_click, ...};`.
- **MCP tools**: Add a `ToolDef` in `src/mcp/` (see "Adding an MCP Tool" in `CONTRIBUTING.md`).
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
