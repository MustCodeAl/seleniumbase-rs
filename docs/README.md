---
title: "SeleniumBase for Rust"
sidebarTitle: "Introduction"
---

Welcome to the SeleniumBase for Rust book. This guide explains how to build
browser automation scripts, end-to-end tests, and operational tooling with
`seleniumbase-rs`.

The crate is a Rust port of the Python SeleniumBase framework. It keeps the
parts that make SeleniumBase productive—the large `BaseCase` API, anti-detection
features, recorder, and CLI—while replacing the Python runtime with Rust's type
system, async runtime, and native distribution model.

<Columns cols={2}>

<Card title="Walkthrough" icon="rocket" color="#d47628" href="/tutorials/getting_started">
  Create a Rust project, add seleniumbase-rs, and run your first test.
</Card>

<Card title="Writing Browser Tests" icon="flask" color="#199f43" href="/rust-test-tooling">
  BaseCase, selectors, waits and assertions for maintainable tests.
</Card>

<Card title="Migrate from Python, Playwright or Puppeteer" icon="arrow-right-arrow-left" color="#1a85d4" href="/python-migration">
  Keep a similar API while gaining compile-time guarantees.
</Card>

<Card title="Stealth and Browser Modes" icon="user-secret" color="#a631be" href="/tutorials/uc_mode">
  UC mode, CDP, pure CDP, fingerprints and binary patching.
</Card>

<Card title="CLI Usage" icon="terminal" color="#1ca1c7" href="/tutorials/cli_usage">
  The sbrs CLI, Commander, and the MCP servers for AI agents.
</Card>

<Card title="Why Rust?" icon="gear" color="#d32a61" href="/why-rust">
  Security, reliability, performance and how it compares.
</Card>

<Card title="Plugins" icon="plug" color="#693acf" href="/plugins/overview">
  Browser plugins as Rust crates: privacy hardening, block resources, load extensions, run WebAssembly.
</Card>

<Card title="Stealth scraping" icon="fingerprint" color="#d5a910" href="/stealth/overview">
  Wire fingerprints, identities and the browser scraper (experimental).
</Card>

</Columns>

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs` is a Rust port of Python SeleniumBase. `BaseCase` (WebDriver, with UC mode and CDP helpers) and `sb_cdp` (pure CDP, no driver) are the two engines. Settings come from `sbase_config.toml`, `SB_*` environment variables, `BrowserConfig` and CLI flags, in that order of priority. The `sbrs` CLI and the `sbrs-mcp` MCP server ship with the crate.

</Visibility>

## Quickstart

Rust 1.99 or newer and a Chromium-based browser (Chrome, Chromium or Edge) are all you need.

<Steps titleSize="h3">

<Step title="Create a project" icon="folder-plus">

```bash Terminal
cargo new my_first_test
cd my_first_test
```

</Step>

<Step title="Add the dependencies" icon="box">

The crate is installed from git (it is not on crates.io). It is the only
dependency: it re-exports the Tokio runtime that runs the async code
(`seleniumbase_rs::tokio`), so the examples need no other crate. See
[Imports and dependencies](/tutorials/getting_started#imports-and-dependencies) for what else it re-exports.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
```

</Step>

<Step title="Write a test" icon="code">

Pick an engine. The pure CDP engine talks to Chrome directly and needs no
driver; `BaseCase` uses WebDriver and starts `chromedriver` for you.

<Tabs>

<Tab title="Pure CDP (no driver)" icon="bolt">

```rust,no_run src/main.rs
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;

#[tokio::main]
async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("seleniumbase.io/simple/login").await?; // https:// is added for you
    page.locator("#username").fill("demo_user").await?;
    page.locator("#password").fill("secret_pass").await?;
    page.locator("button").click().await?;
    page.locator("h1").expect().to_contain_text("Welcome").await?;

    browser.close().await?;
    Ok(())
}
```

</Tab>

<Tab title="BaseCase (WebDriver)" icon="flask">

```rust,no_run src/main.rs
use seleniumbase_rs::{tokio, BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    // Starts the browser, and chromedriver if needed.
    let mut sb = BaseCase::new(config).await?;

    sb.open("https://seleniumbase.io/simple/login").await?;
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;
    sb.assert_text_visible("Welcome!", "body").await?;

    sb.quit().await?;
    Ok(())
}
```

</Tab>

</Tabs>

</Step>

<Step title="Run it" icon="play">

```bash Terminal
cargo run
```

The test opens the login page, signs in and checks the welcome text. A failed
check ends the program with an error that names the selector.

</Step>

</Steps>

<Tip>

No Rust project yet? The `sbrs` command line drives a browser on its own:
`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli`,
then `sbrs --headless open https://example.com`. See the [CLI TL;DR](/help/cli_tldr).

</Tip>

## Who this book is for

- <Icon color="#1a85d4" icon="code" /> **Rust developers** writing browser tests for the first time.
- <Icon color="#199f43" icon="people-group" /> **Teams migrating** from Python SeleniumBase or Selenium who want to keep a
  similar API while gaining compile-time guarantees.
- <Icon color="#d47628" icon="robot" /> **Automation engineers** who need undetected browser execution, CDP control,
  or cloud artifact uploads.
- <Icon color="#a631be" icon="server" /> **Operators** who want a small native CLI (`sbrs`) and Docker image for CI/CD.

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

- **Browser modes**: <Tooltip tip="The Selenium protocol: the test talks to chromedriver, which drives the browser.">WebDriver</Tooltip>, <Tooltip tip="Chrome DevTools Protocol: the browser's own debugging protocol, spoken over a WebSocket." cta="Pure CDP engine" href="/tutorials/pure_cdp">CDP</Tooltip>, <Tooltip tip="Undetected Chromedriver mode: a browser launch and session setup that avoid common bot checks; the driver itself is not patched." cta="Undetected (UC) mode" href="/tutorials/uc_mode">UC</Tooltip> (Undetected Chromedriver), and an optional
  Playwright bridge (the `seleniumbase-rs-playwright` crate: `rustwright` attached to a stealthy CDP browser).
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

## Key features

### Stealth and UC mode

UC mode (Undetected Chromedriver) launches the browser and sets up the session so
that common bot checks do not spot the automation. Turn it on in the configuration:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true);
let mut sb = BaseCase::new(config).await?;
```

Fingerprints, stealth policies and binary patching go further; see
[Undetected (UC) Mode](/tutorials/uc_mode) and
[Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth).

### Human-like input

Bot detectors watch typing speed and mouse movement. These methods add the pauses
a person makes:

```rust
sb.human_type("#search", "Stealthy typing").await?; // one key at a time, with pauses
sb.human_click("#submit").await?;                   // a short pause before the click
sb.smooth_scroll_to("#footer").await?;              // scrolls the element into the middle
sb.uc_type("#email", "user@example.com").await?;   // the names UC-mode scripts use
sb.uc_click("#login").await?;
```

### Smart waits

Every action waits for its element first, so a test needs no `sleep`. To wait
for a state yourself, give a time limit in seconds:

```rust
sb.wait_for_element_visible("#dynamic-content", 10).await?;
sb.wait_for_element_absent("#loading-spinner", 5).await?;
sb.assert_element_present(".user-profile").await?;
```

See [Waits and Assertions](/tutorials/utilities#waits-and-assertions).

### PDF files and HTML parsing

Print the page to a PDF, read the text of a PDF, and parse the page's HTML the
way BeautifulSoup does in Python:

```rust
sb.print_to_pdf("page.pdf").await?;
let text = sb.get_pdf_text("page.pdf")?;
let soup = sb.get_beautiful_soup_object().await?;
```

See [PDF Parsing](/tutorials/pdf_parsing).

### More of BaseCase

<Columns cols={2}>

<Card title="Tours and charts" icon="route" href="/tutorials/tours">
  Step-by-step product tours on any page, and charts saved as HTML (see also [Charts](/tutorials/charts)).
</Card>

<Card title="Shadow DOM" icon="layer-group" href="/tutorials/selectors#shadow-dom">
  Selectors that reach into shadow trees, open or closed, for every method that takes one.
</Card>

<Card title="GUI automation" icon="computer-mouse" href="/tutorials/gui_automation">
  Mouse and keyboard at the operating-system level, for native dialogs.
</Card>

<Card title="CDP mode" icon="bolt" href="/tutorials/cdp_mode">
  Chrome DevTools Protocol commands from a BaseCase test.
</Card>

<Card title="MasterQA" icon="clipboard-check" href="/tutorials/masterqa">
  Manual verification steps recorded during a run, exported as a report.
</Card>

<Card title="All of the API" icon="book" href="/tutorials/api_reference">
  Where each group of methods is documented.
</Card>

</Columns>

The HTML inspector checks the current page for accessibility and markup problems:
`sb.inspect_html().await?` lists them, and `sb.assert_no_html_issues().await?`
fails the test when there are any.

## Low-code tests

Tests do not have to start as Rust. seleniumbase-rs reads
[Selenium IDE](/tutorials/selenium_ide) recordings, runs Gherkin scenarios as
cargo tests ([Behave and Gherkin](/tutorials/utilities#gherkin-scenarios-with-rstest-bdd)), converts Python, Playwright
and Puppeteer scripts ([Migrate from Python](/python-migration)), and records your
clicks as a ready-to-run test ([Recorder Mode](/tutorials/recorder_mode)).

## The command line

`sbrs` drives a browser without a Rust project. Each command starts its own
browser and closes it at the end.

<Steps titleSize="h3">

<Step title="Open a page" icon="globe">

```bash Terminal
sbrs --headless open https://example.com
```

</Step>

<Step title="Record a test" icon="circle-dot">

```bash Terminal
sbrs mkrec my_test.rs --url https://example.com
```

Act in the browser; the test is written to `my_test.rs` and the actions to a
recording that `sbrs replay` plays back.

</Step>

<Step title="Run tests from a terminal UI" icon="table-list">

```bash Terminal
sbrs commander
```

Commander needs a build with the `tui` feature.

</Step>

</Steps>

Every command and flag is in the [CLI reference](/help/cli_reference);
[CLI Usage](/tutorials/cli_usage) explains them by task.

## Configuration

Settings come from `sbase_config.toml` in the project, `SB_*` environment
variables, `BrowserConfig` in code and command-line flags, each overriding the
one before it. See [Settings and Configuration](/tutorials/settings_and_config).

## MCP server for AI agents

`sbrs-mcp` gives MCP clients a browser. `--server cdp|driver|sb|dev` picks the
tool set (`dev` starts no browser), and `sbrs-mcp install CLIENT` registers it
with a client. The browser starts on the first `start_browser` call, so a client
can list the tools without one. See [MCP Server](/tutorials/mcp_server).

## For contributors

[API Reference](/tutorials/api_reference) maps the crate's modules; `cargo doc
--open` builds the generated documentation locally.


## How to use this book

<Tabs>

<Tab title="New to the crate" icon="rocket">

Start with [Getting Started](/tutorials/getting_started)
and then read [Writing Browser Tests](/rust-test-tooling) and
[Waits and Assertions](/tutorials/utilities#waits-and-assertions).

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
[Troubleshooting](/help/common_problems) handy.

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
