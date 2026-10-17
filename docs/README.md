---
title: "SeleniumBase for Rust"
---

# SeleniumBase for Rust

Welcome to the SeleniumBase for Rust book. This guide explains how to build
browser automation scripts, end-to-end tests, and operational tooling with
`seleniumbase-rs`.

The crate is a Rust port of the Python SeleniumBase framework. It keeps the
parts that make SeleniumBase productive—the large `BaseCase` API, anti-detection
features, recorder, and CLI—while replacing the Python runtime with Rust's type
system, async runtime, and native distribution model.

<Columns cols={2}>

<Card title="Getting Started" icon="rocket" href="tutorials/getting_started.md">

Create a Rust project, add `seleniumbase-rs`, and run your first test.

</Card>

<Card title="Writing Browser Tests" icon="flask" href="rust-test-tooling.md">

`BaseCase`, selectors, waits and assertions for maintainable tests.

</Card>

<Card title="Migrate from Python, Playwright or Puppeteer" icon="arrow-right-arrow-left" href="python-migration.md">

Keep a similar API while gaining compile-time guarantees.

</Card>

<Card title="Stealth and Browser Modes" icon="user-secret" href="tutorials/uc_mode.md">

UC mode, CDP, pure CDP, fingerprints and binary patching.

</Card>

<Card title="CLI Usage" icon="terminal" href="tutorials/cli_usage.md">

The `sbrs` CLI, Commander, and the MCP servers for AI agents.

</Card>

<Card title="Why Rust?" icon="gear" href="why-rust.md">

Security, reliability, performance and how it compares.

</Card>

<Card title="Plugins" icon="plug" href="plugins/overview.md">

Browser plugins as Rust crates: block resources, load extensions, run WebAssembly.

</Card>

<Card title="Stealth scraping" icon="fingerprint" href="stealth/overview.md">

Wire fingerprints, identities and the browser scraper (experimental).

</Card>

</Columns>

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs` is a Rust port of Python SeleniumBase. `BaseCase` (WebDriver, with UC mode and CDP helpers) and `sb_cdp` (pure CDP, no driver) are the two engines. Settings come from `sbase_config.toml`, `SB_*` environment variables, `BrowserConfig` and CLI flags, in that order of priority. The `sbrs` CLI and the `sbrs-mcp` MCP server ship with the crate.

</Visibility>

## Who this book is for

- **Rust developers** writing browser tests for the first time.
- **Teams migrating** from Python SeleniumBase or Selenium who want to keep a
  similar API while gaining compile-time guarantees.
- **Automation engineers** who need undetected browser execution, CDP control,
  or cloud artifact uploads.
- **Operators** who want a small native CLI (`sbrs`) and Docker image for CI/CD.

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
   and the `sbrs` CLI.
7. Operate the test suite reliably in CI/CD with tracing, Docker, and shell
   completions.

## What is included

- **Browser modes**: <Tooltip tip="The Selenium protocol: the test talks to chromedriver, which drives the browser.">WebDriver</Tooltip>, <Tooltip tip="Chrome DevTools Protocol: the browser's own debugging protocol, spoken over a WebSocket.">CDP</Tooltip>, <Tooltip tip="Undetected Chromedriver mode: a patched driver and browser launch that avoids common bot checks.">UC</Tooltip> (Undetected Chromedriver), and an optional
  `rustwright`-based Playwright-compatible engine.
- **`BaseCase` API**: more than 200 methods for navigation, element interaction,
  assertions, waits, frames, windows, cookies, storage, uploads, PDF handling,
  HTML parsing, and more.
- **Stealth stack**: `ChromedriverPatcher`, `ChromeBinaryPatcher`,
  `Fingerprint` presets, `StealthPolicy`, and a priority-ordered
  `EvasionProvider` registry.
- **Low-code tooling**: JSON scenario runner, Python importer, Selenium IDE
  parser, Gherkin runner, and action recorder.
- **Operational tooling**: `sbrs` CLI, Commander TUI, shell completions, Docker
  image, `SB_*` environment configuration, and structured `tracing` output.
- **Optional integrations**: S3, Azure Blob Storage, Google Cloud Storage, and an
  MCP server, each behind a Cargo feature flag.

## How to use this book

<Tabs>

<Tab title="New to the crate">

Start with [Getting Started](tutorials/getting_started.md)
and then read [Writing Browser Tests](rust-test-tooling.md) and
[Waits and Assertions](tutorials/waits_assertions.md).

</Tab>

<Tab title="Migrating from Python">

Jump to [Migrate Python Tests](python-migration.md)
and then read [Selectors](tutorials/selectors.md) for the differences in selector
syntax and test lifecycle.

</Tab>

<Tab title="Anti-detection">

Read [Undetected (UC) Mode](tutorials/uc_mode.md),
[Fingerprint & Stealth Profiles](tutorials/fingerprint_stealth.md), and
[Binary Patching](tutorials/binary_patching.md) in that order.

</Tab>

<Tab title="Day to day">

Keep [CLI Usage](tutorials/cli_usage.md) and
[Common Problems](help/common_problems.md) handy.

</Tab>

</Tabs>

## Current status

Every API of Python SeleniumBase 4.55.2 has a recorded status in the
[parity table](parity.md). Development happens on
[MustCodeAl/seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs),
and releases are tagged there.

<Note>

The crate is not on crates.io yet: install it
from a release tag (see [Building and installing](building.md)).

</Note>

## The ecosystem

Companion projects live in their own repositories and are documented on this
site: see the [Ecosystem](ecosystem/overview.md) page.

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-plugins" />

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-browser-extension" />

<GitHub.Repo repo="MustCodeAl/homebrew-tap" />

## Build this book locally

Install [mdBook](https://rust-lang.github.io/mdBook/guide/installation.html),
then run from the repository root:

<CodeGroup>

```bash Serve
mdbook serve --open
```

```bash Build
mdbook build
```

</CodeGroup>

`mdbook serve --open` serves the book and opens it. `mdbook build` builds a
static copy. The built HTML is written to the `book/` directory.
