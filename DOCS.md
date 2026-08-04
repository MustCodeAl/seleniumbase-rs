# SeleniumBase Rust (`seleniumbase-rs`)

Welcome to the official documentation for **SeleniumBase Rust**, a high-performance, reliable, and stealthy web automation framework. This crate is a 1:1 Rust port of the popular Python `SeleniumBase` library, bringing advanced browser automation, Undetected Chromedriver (UC Mode) capabilities, and low-code testing to the Rust ecosystem.

---

## Table of Contents
1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Key Features](#key-features)
   - [Stealth & UC Mode](#stealth--uc-mode)
   - [Humanizer Methods](#humanizer-methods)
   - [Smart Waits](#smart-waits)
4. [Low-Code Scenarios](#low-code-scenarios)
5. [Command Line Interface (CLI)](#command-line-interface-cli)
6. [MCP Server](#mcp-server)
7. [Developer Documentation](#developer-documentation)

---

## Installation

Add `seleniumbase-rs` to your `Cargo.toml`:

```toml
[dependencies]
seleniumbase-rs = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

*Note: This crate relies on an asynchronous runtime like `tokio` and uses `thirtyfour` under the hood for WebDriver communication.*

---

## Quick Start

The core of SeleniumBase Rust is the `BaseCase` struct, which encapsulates the WebDriver session and provides hundreds of convenience methods for browser automation.

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure the browser
    let config = BrowserConfig::default();
    
    // 2. Initialize BaseCase
    let mut sb = BaseCase::new(config).await?;

    // 3. Automate!
    sb.open("https://github.com").await?;
    sb.type_text("input[name='q']", "SeleniumBase").await?;
    sb.click("button[type='submit']").await?;
    
    sb.assert_text_visible("SeleniumBase", "body").await?;

    // 4. Cleanup
    sb.quit().await?;
    Ok(())
}
```

---

## Key Features

### Stealth & UC Mode
Bypass aggressive bot detection (like Cloudflare, DataDome) using **UC Mode** (Undetected Chromedriver). This mode automatically strips `HeadlessChrome` from your User-Agent and patches CDC variables in the chromedriver binary.

```rust
let mut config = BrowserConfig::default();
config.mode = DriverMode::Uc; // Enable Stealth UC Mode
config.headless = true;

let mut sb = BaseCase::new(config).await?;
sb.open("https://bot.sannysoft.com/").await?;
```

### Humanizer Methods
Bot detectors analyze typing speed and mouse movements. The `seleniumbase-rs` humanizer methods inject randomized delays to mimic real user behavior.

```rust
// Types character-by-character with random delays (30ms - 120ms)
sb.human_type("#search", "Stealthy typing...").await?;

// Waits a random duration (100ms - 300ms) before clicking
sb.human_click("#submit-btn").await?;

// Smoothly scrolls the element into the center of the viewport
sb.smooth_scroll_to("#footer").await?;

// Aliases commonly used with UC Mode
sb.uc_type("#email", "user@example.com").await?;
sb.uc_click("#login").await?;
```

### Smart Waits
Forget `sleep(5)`. SeleniumBase automatically waits for elements to be present, visible, or clickable before interacting with them.

```rust
// Waits up to 10 seconds for the element to be visible
sb.wait_for_element_visible("#dynamic-content", 10).await?;

// Waits for an element to disappear
sb.wait_for_element_absent("#loading-spinner", 5).await?;

// Validates state
sb.assert_title("Dashboard").await?;
sb.assert_element_present(".user-profile").await?;
```

### PDF, HTML Parsing, Tours & Charts
Generate PDFs from pages, parse HTML like BeautifulSoup, build interactive tours, and create charts.

```rust
// Print the current page to PDF via CDP
sb.print_to_pdf("page.pdf").await?;
let text = sb.get_pdf_text("page.pdf")?;

// Parse the current page source
let soup = sb.get_beautiful_soup_object().await?;
let heading = soup.get_text("h1")?;

// Build a themed tour
sb.create_tour_with_theme("Onboarding", TourTheme::Shepherd).await?;
sb.add_tour_step("Click the logo", Some("#logo")).await?;
sb.add_tour_step("Fill the form", None).await?;
sb.play_tour().await?;

// Create and save charts
sb.create_bar_chart("Sales").await?;
sb.add_data_point("Q1", 100).await?;
sb.save_chart("sales.html").await?;
```

---

## Low-Code Scenarios

You can write automation flows in simple JSON files and run them using the `seleniumbase-rs` engine.

**`scenario.json`**
```json
{
  "name": "GitHub Search",
  "steps": [
    { "action": "open", "target": "https://github.com" },
    { "action": "human_type", "target": "input[name='q']", "text": "Rustlang" },
    { "action": "human_click", "target": "button[type='submit']" }
  ]
}
```

Run it in Rust:
```rust
use seleniumbase_rs::scenario::run_scenario;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_scenario("scenario.json").await?;
    Ok(())
}
```

---

## Command Line Interface (CLI)

`seleniumbase-rs` comes with a powerful CLI tool named `sbase` to record scripts, run scenarios, and perform quick interactions.

### Recording a Script
Translates your browser actions into a ready-to-run Rust script:
```bash
sbase record --url https://wikipedia.org --output my_test.rs
```

### Running JSON Scenarios
```bash
sbase run-scenario --file tests/login_flow.json
```

### Quick Commands
```bash
sbase open --url https://rust-lang.org
sbase click --css "#get-started"
sbase human-type --css "#search" --text "Concurrency"
```

---

## GUI Automation

Automate the mouse and keyboard at the OS level using PyAutoGUI-style helpers. This is useful for interacting with native dialogs or browsers that do not expose a WebDriver endpoint.

```rust
sb.gui_click_x_y(100, 200).await?;
sb.gui_write("Hello from Rust").await?;
sb.gui_press_key("Enter").await?;
sb.gui_drag_and_drop("#source", "#target").await?;
```

## CDP Page / Driver Mode

When `DriverMode::Cdp` or `DriverMode::Uc` is enabled, you can interact with the page through Chrome DevTools Protocol primitives:

```rust
sb.cdp_open("https://example.com").await?;
sb.cdp_click("#button").await?;
sb.cdp_type("#input", "text").await?;
let text = sb.cdp_get_text("#result").await?;
let value = sb.cdp_evaluate("document.title").await?;
```

## Shadow DOM Selectors

Pierce through closed shadow trees with the `::shadow` combinator:

```rust
sb.shadow_click("my-app ::shadow .button").await?;
sb.shadow_type("my-app ::shadow form ::shadow input", "value").await?;
let text = sb.shadow_get_text("my-app ::shadow .label").await?;
```

## HTML Inspector

Run accessibility and markup checks on the current page:

```rust
let inspection = sb.inspect_html().await?;
for issue in &inspection.issues {
    println!("[{}] {}", issue.rule, issue.message);
}
sb.assert_no_html_issues().await?;
```

## Dialog Builder

Request user input during a test run with native dialogs:

```rust
let ok = sb.show_confirm("Continue?", "Do you want to proceed?");
let result = sb.show_prompt("Input required", "Enter your name:", Some("guest"));
if let Some(path) = sb.choose_file_dialog("Pick a file") {
    println!("Selected: {}", path);
}
```

## MasterQA / Test-Case Management

Record manual verification steps and export a Markdown test-case report:

```rust
sb.start_qa_session("Login flow");
sb.manual_verify("Login page loads correctly");
sb.manual_verify("Error message appears for bad credentials");
let report_path = sb.save_qa_report(&[], "Login flow", None::<&str>)?;
```

## Global Config File

Create `sbase_config.toml` in your project root to set defaults:

```toml
browser = "chrome"
headless = true
mode = "uc"
mobile = false
proxy = "http://proxy:8080"
user_data_dir = "/tmp/sb-profile"
extension_dir = "/path/to/extension"
threads = 4
```

Override any value with CLI flags, e.g. `sbase --mobile --proxy-pac-url http://proxy/proxy.pac open https://example.com`.

## SeleniumBase Commander

Launch the TUI test runner with:

```bash
sbase commander
```

Use arrow keys to navigate discovered tests, press Enter to run them, and `q` to quit.

## Recorder Mode

Record browser actions and export them as a Rust test:

```bash
sbase mkrec my_test.rs
sbase recorder
```

## MCP Server

The optional `seleniumbase-mcp` binary exposes browser actions to trusted MCP
clients over stdio:

```bash
cargo build --release --bin seleniumbase-mcp --features mcp-server
```

The tool catalog can be queried before WebDriver is available because the
browser session starts on the first browser tool call. Available tools cover
navigation, page metadata, element interaction, text assertions, JavaScript
execution, and session shutdown. See the
[README MCP server section](./README.md#mcp-server-optional-feature) for client
configuration and the complete tool list.

## Developer Documentation

The [Developer Guide](./docs/DEVELOPER_GUIDE.md) explains the crate layout,
`BaseCase` implementation modules, capability traits, error handling, feature
flags, testing, and the publishing workflow.

For generated API documentation, run:

```bash
cargo doc --open
```
