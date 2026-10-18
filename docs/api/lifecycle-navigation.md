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
| `sb.quit().await` | Closes the browser and ends the session. |
| `sb.reconnect().await` | Replaces the WebDriver session while keeping the driver process. |

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
