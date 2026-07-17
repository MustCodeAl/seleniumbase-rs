## SeleniumBase Rust Port (CDP/UC/CDC Foundation)

This directory contains a complete Rust port of SeleniumBase using `thirtyfour` as the underlying WebDriver engine.

It is designed to provide 1:1 API parity with the Python SeleniumBase library for core DOM interactions, along with the powerful stealth modes and CDP integrations.

### Key Features Ported

- **BaseCase API**: Over 50+ actions, waits, assertions, and DOM manipulation methods.
  - Basic interactions: `open`, `click`, `type_text`, `submit`, `clear`, `hover`
  - Advanced interactions: `double_click`, `context_click`, `slow_click`, `drag_and_drop`
  - JS interactions: `js_click`, `js_type`, `execute_script`, `execute_async_script`
  - Selectors & Shadow DOM: `find_element`, `find_elements`, `get_shadow_root`
  - Attributes/Properties: `get_attribute`, `get_property`, `set_attribute`, `remove_attribute`
  - Windows & Frames: `switch_to_frame`, `switch_to_default_content`, `switch_to_window`, `switch_to_new_window`, `maximize_window`
  - Navigation: `go_back`, `go_forward`, `refresh`
  - Scrolling: `scroll_to`, `scroll_to_top`, `scroll_to_bottom`
  - Alerts: `accept_alert`, `dismiss_alert`, `type_alert_text`
  - Cookies & Storage: `get_cookie`, `add_cookie`, `delete_all_cookies`, `set_local_storage_item`, `clear_local_storage`, `remove_local_storage_item`
  - Uploads: `choose_file`
- **Driver Modes (`BrowserConfig`)**: `WebDriver`, `Cdp`, `Uc` (Undetected Chromedriver).
- **Stealth Integrations**:
  - UC mode (Chromium flags + `navigator.webdriver` evasion).
  - CDC stealth binary patcher to strip hardcoded signatures from the `chromedriver` executable.
- **CDP Execution**: 
  - Send raw commands: `execute_cdp`, `execute_cdp_with_params`.
  - Network & Cache: `set_network_conditions`, `clear_browser_cache`.
- **Test Artifacts**: `save_screenshot_to_logs()`, `save_page_source_to_logs()`.
- **Low-Code Runner**: JSON scenario execution with an HTML dashboard.
- **Action Recorder**: Captures browser interactions and compiles them into a JSON scenario or a standalone Rust script.
- **Interactive CLI (`sbase`)**: Execute single commands directly from the terminal.

### Quick start

```bash
cd rust-port
cargo run --bin sbase -- --cdp open https://seleniumbase.io
```

The command expects a running WebDriver endpoint at `http://localhost:4444`.
Override it with `--webdriver` when needed.

### Smoke test with assertion

```bash
cargo run --bin sbase -- --uc smoke https://seleniumbase.io --title-contains SeleniumBase
```

### Run raw CDP command

```bash
cargo run --bin sbase -- --cdp cdp --cmd Browser.getVersion
cargo run --bin sbase -- --cdp cdp --cmd Network.setCacheDisabled --params '{"cacheDisabled":true}'
```

### Save artifacts

```bash
cargo run --bin sbase -- screenshot
cargo run --bin sbase -- save-source
```

### Assertions and Waits from CLI

```bash
cargo run --bin sbase -- open https://seleniumbase.io
cargo run --bin sbase -- assert-element --css "body"
cargo run --bin sbase -- wait-for-text --css "body" --text "SeleniumBase" --timeout 15
```

### CDC Stealth Binary Patcher

You can patch your downloaded `chromedriver` executable directly to remove hardcoded CDC variables and signatures (matching SeleniumBase Python `undetected-chromedriver` patches):

```bash
cargo run --bin sbase -- patch-chromedriver --path /path/to/chromedriver
```

### Run JSON scenario and generate dashboard

```bash
cargo run --bin sbase -- run-scenario --file ./scenario.json
```

Example `scenario.json`:

```json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "assert_element", "css": "body"},
    {"action": "wait_for_text", "css": "body", "text": "SeleniumBase", "timeout": 15}
  ]
}
```
