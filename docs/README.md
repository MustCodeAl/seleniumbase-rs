# SeleniumBase for Rust

Welcome to the SeleniumBase for Rust book. This guide explains how to build
browser automation scripts, end-to-end tests, and operational tooling with
`seleniumbase-rs`.

The crate is a Rust port of the Python SeleniumBase framework. It keeps the
parts that make SeleniumBase productive—the large `BaseCase` API, anti-detection
features, recorder, and CLI—while replacing the Python runtime with Rust's type
system, async runtime, and native distribution model.

## Who this book is for

- **Rust developers** writing browser tests for the first time.
- **Teams migrating** from Python SeleniumBase or Selenium who want to keep a
  similar API while gaining compile-time guarantees.
- **Automation engineers** who need undetected browser execution, CDP control,
  or cloud artifact uploads.
- **Operators** who want a small native CLI (`sbase`) and Docker image for CI/CD.

## What you will learn

By reading this book you will learn how to:

1. Create a Rust project, add `seleniumbase-rs`, and run your first test.
2. Choose between WebDriver, CDP, UC, and Playwright browser modes.
3. Use selectors, waits, assertions, and the recorder to write maintainable tests.
4. Configure runs with `BrowserConfig`, `sbase_config.toml`, and `SB_*` environment
   variables.
5. Apply anti-detection measures including fingerprints, binary patching, and
   engine spoofing arguments.
6. Integrate with cloud storage, Gherkin, Selenium IDE, PDF tools, charts, tours,
   and the `sbase` CLI.
7. Operate the test suite reliably in CI/CD with tracing, Docker, and shell
   completions.

## What is included

- **Browser modes**: WebDriver, CDP, UC (Undetected Chromedriver), and an optional
  `rustwright`-based Playwright-compatible engine.
- **`BaseCase` API**: more than 200 methods for navigation, element interaction,
  assertions, waits, frames, windows, cookies, storage, uploads, PDF handling,
  HTML parsing, and more.
- **Stealth stack**: `ChromedriverPatcher`, `ChromeBinaryPatcher`,
  `Fingerprint` presets, `StealthFlags`, and a priority-ordered
  `EvasionProvider` registry.
- **Low-code tooling**: JSON scenario runner, Python importer, Selenium IDE
  parser, Gherkin runner, and action recorder.
- **Operational tooling**: `sbase` CLI, Commander TUI, shell completions, Docker
  image, `SB_*` environment configuration, and structured `tracing` output.
- **Optional integrations**: S3, Azure Blob Storage, Google Cloud Storage, and an
  MCP server, each behind a Cargo feature flag.

## How to use this book

If you are new to the crate, start with [Getting Started](tutorials/getting_started.md)
and then read [Writing Browser Tests](rust-test-tooling.md) and
[Waits and Assertions](tutorials/waits_assertions.md).

If you are migrating from Python, jump to [Migrate Python Tests](python-migration.md)
and then read [Selectors](tutorials/selectors.md) for the differences in selector
syntax and test lifecycle.

If you need anti-detection, read [Undetected (UC) Mode](tutorials/uc_mode.md),
[Fingerprint & Stealth Profiles](tutorials/fingerprint_stealth.md), and
[Binary Patching](tutorials/binary_patching.md) in that order.

For day-to-day command-line usage, keep [CLI Usage](tutorials/cli_usage.md) and
[Common Problems](help/common_problems.md) handy.

## Current status

The initial Rust port plan is complete. The crate builds, passes Clippy, and
tests cleanly on stable Rust with all feature flags enabled. Active work is
pushed to two locations:

- Monorepo feature branch: `MustCodeAl/SeleniumBase/mustcodeal-rust-port`
- Crate-only orphan branch: `MustCodeAl/seleniumbase-rs/main`

`cargo publish --dry-run` is blocked only by the `playwright` feature's
`rustwright` git dependency. The tracked tag (`v0.2.0`) is newer than the version
published on crates.io (`0.1.1`), so an upstream release is needed before the
crate can be published.

## Build this book locally

Install [mdBook](https://rust-lang.github.io/mdBook/guide/installation.html),
then run from the `rust-port` directory:

```bash
mdbook serve --open
```

To build a static copy:

```bash
mdbook build
```

The built HTML is written to the `book/` directory.

