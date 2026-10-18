---
title: "SeleniumBase for Rust"
---

Welcome to the SeleniumBase for Rust book. This guide explains how to build
browser automation scripts, end-to-end tests, and operational tooling with
`seleniumbase-rs`.

The crate is a Rust port of the Python SeleniumBase framework. It keeps the
parts that make SeleniumBase productive—the large `BaseCase` API, anti-detection
features, recorder, and CLI—while replacing the Python runtime with Rust's type
system, async runtime, and native distribution model.

<Columns cols={2}>

<Tile href="/tutorials/getting_started" title="Getting Started" description="Create a Rust project, add seleniumbase-rs, and run your first test.">
  <Icon icon="rocket" size={48} />
</Tile>

<Tile href="/rust-test-tooling" title="Writing Browser Tests" description="BaseCase, selectors, waits and assertions for maintainable tests.">
  <Icon icon="flask" size={48} />
</Tile>

<Tile href="/python-migration" title="Migrate from Python, Playwright or Puppeteer" description="Keep a similar API while gaining compile-time guarantees.">
  <Icon icon="arrow-right-arrow-left" size={48} />
</Tile>

<Tile href="/tutorials/uc_mode" title="Stealth and Browser Modes" description="UC mode, CDP, pure CDP, fingerprints and binary patching.">
  <Icon icon="user-secret" size={48} />
</Tile>

<Tile href="/tutorials/cli_usage" title="CLI Usage" description="The sbrs CLI, Commander, and the MCP servers for AI agents.">
  <Icon icon="terminal" size={48} />
</Tile>

<Tile href="/why-rust" title="Why Rust?" description="Security, reliability, performance and how it compares.">
  <Icon icon="gear" size={48} />
</Tile>

<Tile href="/plugins/overview" title="Plugins" description="Browser plugins as Rust crates: privacy hardening, block resources, load extensions, run WebAssembly.">
  <Icon icon="plug" size={48} />
</Tile>

<Tile href="/stealth/overview" title="Stealth scraping" description="Wire fingerprints, identities and the browser scraper (experimental).">
  <Icon icon="fingerprint" size={48} />
</Tile>

</Columns>

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs` is a Rust port of Python SeleniumBase. `BaseCase` (WebDriver, with UC mode and CDP helpers) and `sb_cdp` (pure CDP, no driver) are the two engines. Settings come from `sbase_config.toml`, `SB_*` environment variables, `BrowserConfig` and CLI flags, in that order of priority. The `sbrs` CLI and the `sbrs-mcp` MCP server ship with the crate.

</Visibility>

## Who this book is for

- <Icon icon="code" /> **Rust developers** writing browser tests for the first time.
- <Icon icon="people-group" /> **Teams migrating** from Python SeleniumBase or Selenium who want to keep a
  similar API while gaining compile-time guarantees.
- <Icon icon="robot" /> **Automation engineers** who need undetected browser execution, CDP control,
  or cloud artifact uploads.
- <Icon icon="server" /> **Operators** who want a small native CLI (`sbrs`) and Docker image for CI/CD.

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

- **Browser modes**: <Tooltip tip="The Selenium protocol: the test talks to chromedriver, which drives the browser.">WebDriver</Tooltip>, <Tooltip tip="Chrome DevTools Protocol: the browser's own debugging protocol, spoken over a WebSocket." cta="Pure CDP engine" href="/tutorials/pure_cdp">CDP</Tooltip>, <Tooltip tip="Undetected Chromedriver mode: a patched driver and browser launch that avoids common bot checks." cta="Undetected (UC) mode" href="/tutorials/uc_mode">UC</Tooltip> (Undetected Chromedriver), and an optional
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

<Tab title="New to the crate" icon="rocket">

Start with [Getting Started](/tutorials/getting_started)
and then read [Writing Browser Tests](/rust-test-tooling) and
[Waits and Assertions](/tutorials/waits_assertions).

</Tab>

<Tab title="Migrating from Python" icon="arrow-right-arrow-left">

Jump to [Migrate Python Tests](/python-migration)
and then read [Selectors](/tutorials/selectors) for the differences in selector
syntax and test lifecycle.

</Tab>

<Tab title="Anti-detection" icon="user-secret">

Read [Undetected (UC) Mode](/tutorials/uc_mode),
[Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth), and
[Binary Patching](/tutorials/binary_patching) in that order.

</Tab>

<Tab title="Day to day" icon="terminal">

Keep [CLI Usage](/tutorials/cli_usage) and
[Common Problems](/help/common_problems) handy.

</Tab>

</Tabs>

## Current status

Every API of Python SeleniumBase 4.55.2 has a recorded status in the
[parity table](/parity). Development happens on
[MustCodeAl/seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs),
and releases are tagged there.

<Note>

The crate is not on crates.io yet: install it
from a release tag (see [Building and installing](/building)).

</Note>

## The ecosystem

Companion projects live in their own repositories and are documented on this
site: see the [Ecosystem](/ecosystem/overview) page.

<Columns cols={2}>

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-plugins" />

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-browser-extension" />

<GitHub.Repo repo="MustCodeAl/homebrew-tap" />

</Columns>

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
