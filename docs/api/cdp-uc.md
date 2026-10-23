---
title: "CDP and UC helpers"
sidebarTitle: "CDP and UC helpers"
description: "Chrome DevTools and undetected-mode helpers, including the GUI bypass helpers."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## CDP / UC helpers

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
| `uc_click(selector)` | Stealth click with delay. |
| `uc_type(selector, text)` | Stealth type with delay. |
| `uc_open_with_reconnect(url)` | Opens `url` after reconnecting the session. |
| `install_addon(path, temporary)` | Firefox only: installs an add-on (an `.xpi` file or an unpacked directory on the machine geckodriver runs on) through geckodriver and returns its id. An unsigned add-on needs `temporary = true`. Other browsers get `Unsupported`; use `with_extension_dir` for Chrome. |
| `uninstall_addon(id)` | Firefox only: removes an add-on by the id `install_addon` returned. |
| `uc_open_with_disconnect(url)` | Opens `url` with a fresh session. |
| `human_click(selector)` | Human-like click. |
| `human_type(selector, text)` | Human-like type. |

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
| `uc_gui_handle_captcha(selector)` | Native click fallback for simple CAPTCHAs. |

See the [GUI Automation](/tutorials/gui_automation) tutorial for details.
