# SeleniumBase for Rust

This book explains how to build browser automation and end-to-end tests with
`seleniumbase-rs`. The crate combines an ergonomic `BaseCase` API with Rust's
type system, async runtime, test ecosystem, and native deployment model.

Start with [Getting Started](tutorials/getting_started.md). If you are moving
an existing suite, use [Migrate Python Tests](python-migration.md).

## What is included

- WebDriver, CDP, UC, and optional Playwright browser modes.
- Actions, waits, assertions, downloads, screenshots, PDF tools, and Shadow DOM.
- Recorder, Gherkin, Selenium IDE, and Python migration tools.
- A Rust-native test lifecycle that attempts asynchronous browser cleanup.
- Static HTML checks with stable rule IDs and source selectors.
- Shell completions and an optional Model Context Protocol server.

## Build this book

Install [mdBook](https://rust-lang.github.io/mdBook/guide/installation.html),
then run:

```bash
mdbook serve --open
```

