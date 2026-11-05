---
title: "CDP and UC helpers"
sidebarTitle: "CDP and UC helpers"
description: "Chrome DevTools and undetected-mode helpers, including the GUI bypass helpers."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## CDP / UC helpers

An [external Chromium attachment](/ecosystem/tauri-api/cdp) reports
`is_cdp_mode_active()` as true. `activate_cdp_mode()` enables Page and Network
without enabling Runtime or the other detectable domains. Use `cdp_*` methods
or the attached pure `Page`. The WebDriver-backed and UC-specific helpers below
have no driver to use on such a session: they return
`SeleniumBaseError::Unsupported`, whose message names the CDP attachment,
instead of panicking. `reconnect()` re-attaches to the same endpoint and never
starts a local browser or driver.

| Method | Description |
|--------|-------------|
| `activate_cdp_mode()` | Enables CDP domains. |
| `execute_cdp(method)` | Sends a CDP command. |
| `execute_cdp_with_params(method, params)` | Sends a CDP command with params. |
| `cdp_mouse_click(x, y)` | CDP mouse click at viewport coordinates. |
| `cdp_insert_text(text)` | CDP text insert into whatever has focus. |
| `cdp_click(selector)` | Scroll the element into view and click its centre through CDP. |
| `cdp_fast_type(selector, text)` | Clear and insert text in one operation (fast, not stealthy). |
| `cdp_wait_for_element_present(selector, timeout)` | Wait for an element to appear in the DOM. |
| `cdp_select_option_by_index(selector, index)` | Select a dropdown option by zero-based index. |
| `open_new_tab_url(url)` | Open a new tab and navigate to `url`. |
| `clear_browser_cache()` | Clears cache. |
| `clear_browser_cookies()` | Clears cookies. |
| `get_cookies()` | Returns cookies as JSON. |
| `set_network_conditions(conditions)` | Network throttling. |
| `set_timezone(id)` | Sets timezone. |
| `set_geolocation(lat, lon, acc)` | Sets geolocation. |
| `install_addon(path, temporary)` | Firefox only: installs an add-on (an `.xpi` file or an unpacked directory on the machine geckodriver runs on) through geckodriver and returns its id. An unsigned add-on needs `temporary = true`. Other browsers get `Unsupported`; use `with_extension_dir` for Chrome. |
| `uninstall_addon(id)` | Firefox only: removes an add-on by the id `install_addon` returned. |
| `human_click(selector)` | WebDriver click after a random delay of 100 to 300 ms. |
| `human_type(selector, text)` | Clicks the element, then types with the fingerprint's humanize timing, or a typical typist's timing when no fingerprint is set. Planned typos are not sent. |

UC clicks and typing are in [UC mode](#uc-mode). `human_click` and `human_type` are the methods those calls run.

## UC mode

UC mode is `DriverMode::Uc` (`"uc"`). `is_uc_mode_active()` is true only then. `is_cdp_mode_active()` is also true, because that mode keeps a CDP client.

It does not patch chromedriver. Patching the driver binary is `ChromedriverPatcher` and `sbrs patch-chromedriver`, which this mode does not call.

On a Chromium launch the mode adds the undetected-chrome arguments (`--disable-blink-features=AutomationControlled` and the rest of `default_uc_args()`), excludes the `enable-automation` switch, and sets `useAutomationExtension` to false. After connect, `enable_uc_mode` injects the UC init script into every new document, tries to rewrite a `HeadlessChrome` user agent to `Chrome` when the config names none, applies a configured fingerprint, and sets `navigator.webdriver` to undefined on the current document.

| How to turn it on | Effect |
|-------------------|--------|
| `BrowserConfig::default().with_mode(DriverMode::Uc)` | The session starts in UC mode. |
| `sbrs --uc` | Global switch. Aliases: `--undetected`, `--undetectable`. Sets the mode to `uc` (`SB_MODE`). |
| `sbrs --uc` together with `--cdp` | The mode override is `cdp`. `--cdp` is applied first. |
| `sbrs mkfile --uc` | Writes a generated test that uses `DriverMode::Uc`. That command's own `--uc` and `--cdp` cannot be combined. |

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, Result};

async fn open_quietly() -> Result<()> {
    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    let mut sb = BaseCase::new(config).await?;
    assert!(sb.is_uc_mode_active());
    sb.uc_open_with_reconnect("https://example.com").await?;
    sb.uc_click("a").await?;
    Ok(())
}
```

| Method | Description |
|--------|-------------|
| `is_uc_mode_active()` | `true` when `config.mode` is `DriverMode::Uc`. Not async. |
| `uc_click(selector)` | Alias of `human_click`: a WebDriver click after a random delay of 100 to 300 ms. Not a CDP click and not a native GUI click. |
| `uc_type(selector, text)` | Alias of `human_type`. |
| `uc_open_with_reconnect(url)` | `reconnect()`, then `open(url)`. Reconnects the WebDriver session before the navigation. |
| `uc_open_with_disconnect(url)` | Quits the active browser, connects a new session with the same config, then `open(url)`. Other browsers the case holds stay open. |
| `reconnect()` | Reconnects the underlying session. On an external CDP attachment it re-attaches to the same endpoint and does not start a local browser or driver. |

| Command | Description |
|---------|-------------|
| `sbrs uc-click --css SELECTOR` | Calls `uc_click`. It does not turn UC mode on by itself; pass `--uc` when the session should start in that mode. |
| `sbrs uc-type --css SELECTOR --text TEXT` | Calls `uc_type`. Same as `uc-click` for the mode. |

```text
sbrs --uc --url https://example.com uc-click --css a
sbrs --uc --url https://example.com uc-type --css '#q' --text hello
```

`--url` is an alias of `--start-page`. A fingerprint on the same config is applied by UC mode; see [Stealth settings](/api/stealth-settings).

## UC / GUI bypass helpers

<Badge color="blue">Feature: gui</Badge>

<Info>

These methods use native OS input instead of WebDriver to bypass some
anti-detection checks. They require a graphical desktop session.

</Info>

| Method | Description |
|--------|-------------|
| `gui_click_x_y(x, y)` | Native mouse click at screen coordinates. |
| `gui_click_element(selector)` | Native click on an element. |
| `gui_write(text)` | Native text input. |
| `gui_press_key(key)` | Native single key press. |
| `gui_press_keys(keys)` | Native key chord. |
| `uc_gui_click_x_y(x, y)` | UC alias for `gui_click_x_y`. |
| `uc_gui_write(text)` | UC alias for `gui_write`. |
| `uc_gui_press_key(key)` | UC alias for `gui_press_key`. |
| `uc_gui_handle_captcha(selector)` | Waits until the element is visible (the case timeout), then `gui_click_element`. It does not solve an image or audio challenge and does not check that the click passed. |

See the [GUI Automation](/tutorials/gui_automation) tutorial for details.
