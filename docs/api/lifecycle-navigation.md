---
title: "Lifecycle and navigation"
sidebarTitle: "Lifecycle and navigation"
description: "Create and close a BaseCase session, open pages and move through history."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Lifecycle

| Method | Description |
|--------|-------------|
| `BaseCase::new(config).await` | Creates and connects a browser session from a `BrowserConfig`. |
| `BaseCase::without_session(config)` | Creates a `BaseCase` without connecting a driver (used by some runners). |
| `sb.quit().await` | Ends the session; closes a browser it launched, or detaches from an external app and leaves it running. |
| `sb.reconnect().await` | Replaces the WebDriver session while keeping the driver process. |

For a [direct CDP attachment](/ecosystem/tauri-api/cdp), use `cdp_*` methods or
`BrowserSession::pure_cdp_page()` for the full page API. `reconnect()` remains a
WebDriver operation; to attach afresh, call `quit()` and create a new case with
the same remote configuration. WebDriver session reuse and Recorder Mode are
refused for direct attachments.

<Warning>

Always call `quit()` in a `Drop` guard, `sb_test!` macro, or `finally` block so
browser processes do not leak when a test fails.

</Warning>

## Navigation

| Method | Description |
|--------|-------------|
| `open(url)` | Navigates to `url`. |
| `open_url(url)` | Alias for `open`. |
| `open_if_not_url(url)` | Opens `url` only if it is not the current URL. |
| `go_back()` | Browser back. |
| `go_forward()` | Browser forward. |
| `refresh_page()` | Reloads the page. |
| `get_current_url()` | Returns the current URL. |
| `get_title()` | Returns the page title. |
| `get_page_source()` | Returns the full page source. |
| `open_new_window()` | Opens a new browser window. |
| `open_new_tab()` | Opens a new browser tab. |
