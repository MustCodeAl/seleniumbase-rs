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
*For more detailed API references, you can run `cargo doc --open` inside your project directory to view the generated Rustdocs.*
