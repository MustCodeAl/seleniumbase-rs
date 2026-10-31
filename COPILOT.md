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
cargo deny --workspace check  # advisories, licenses, bans, sources
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
| `src/evasion/` | Anti-detection: CDP wrappers, evasions, fingerprint profiles, binary patcher, reactor. |
| `src/profile_payloads/` | External browser profile JSON parsing and conversion. |
| `src/macros.rs` | User-facing macros (`selector!`, `sb_test!`, `sb_click!`, `fingerprint!`, etc.). |
| `src/testing/` | Feature `testing`: the rstest `sb` and `sb_cdp` fixtures for `cargo test` (reruns, time limit, evidence on failure, debug prompts); `testing/bdd` (feature `bdd`) runs Gherkin scenarios through rstest-bdd. |
| `src/utilities/` | Python importer, Selenium IDE parser, Grid helpers. |
| `src/mcp/` | The MCP servers (`cdp`, `driver`, `sb` and the browserless `dev`) and their tools; `src/mcp/dev/` holds the `dev` server. |
| `src/cli/` | The library half of the command line (commands, scaffold, scripts). |
| `crates/cli/src/mcp_server.rs` | `sbrs-mcp` MCP server (package `seleniumbase-rs-cli`, gated by its `mcp-server` feature, on by default). |
| `crates/cli/src/mcp_install/` | `sbrs-mcp install`, `uninstall` and `clients`: registers the server with MCP clients (their config locations and formats, JSON edited as text, Codex's `config.toml`). |
| `crates/cli/src/sbrs.rs` | Main `sbrs` CLI binary (package `seleniumbase-rs-cli`), declared with the `usage` derives; `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` are generated from it. |
| `docs/` | The Docs7 site and the mdBook source. Add a new page to `docs/docs.json` (the menu) and to `SUMMARY.md`; `just mdx` builds every page. |
| `examples/` | Runnable examples. Add new examples to `Cargo.toml` automatically via filename. |

## Conventions

- **Error type**: `crate::Result<T>` is an alias for `Result<T, SeleniumBaseError>`.
- **Async runtime**: Tokio. Tests use `#[tokio::test]`.
- **Selectors**: Use `crate::Selector` (CSS, XPath, Id, LinkText, PartialLinkText).
- **Stealth additions**: Add JS evasions to `src/evasion/evasions.rs`, flags to `src/evasion/fingerprint.rs`, launch args to `src/evasion/options.rs` or `src/evasion/patcher.rs::engine_spoofing_args`, and expose via crate root if user-facing.
- **Macros**: `#[macro_export]` so they live at the crate root. Import as `use seleniumbase_rs::{selector, sb_click, ...};`.
- **MCP tools**: Add a `ToolDef` in `src/mcp/` (see "Adding an MCP Tool" in `CONTRIBUTING.md`).
- **Documentation**: Every public API needs a rustdoc example. Every new feature needs a tutorial or help page and an entry in `docs/SUMMARY.md`.
- **Feature flags**: `s3`, `azure`, `gcp`, `playwright`, `mcp-server`, `mcp-client`, `plugins`, `pdf`, `turso`, `tui`, `gui`, `ocr`, `visual`, `testing`, `bdd`, `allure`. Keep default builds minimal (the library's default features are empty).

## Common gotchas

- `seleniumbase-rs-plugin` and `seleniumbase-rs-plugins-builtin` (the `plugins` feature) are git dependencies, both pinned to the same `rev` (`tests/plugin_features.rs` checks). `cargo publish` uses their `version` from crates.io, so `cargo publish --dry-run` fails until they are there. `rustwright` (the `playwright` feature) comes from crates.io.
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
