---
title: "Screenshots and scripts"
sidebarTitle: "Screenshots and scripts"
description: "Take screenshots, run JavaScript and change the DOM."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Screenshots

| Method | Description |
|--------|-------------|
| `save_screenshot(filename)` | Writes a screenshot to the logs directory. `filename` must be a plain file name (no directory part, no `..`); a file of that name is replaced. |
| `save_screenshot_to_path(path)` | Writes screenshot to an arbitrary path. |
| `screenshot_as_png()` | Returns the current page screenshot as PNG bytes. |
| `check_window(name, level)` | Compares the page's tag structure to a baseline saved under `visual_baseline/<test>/<name>/`, as Python 4.55.2 does: level 1 the tag names in order, level 2 adds the sorted attribute names, level 3 adds the attribute values. The first run (or a missing file) saves the baseline; level 0 is a dry run that only logs a difference; levels 1 to 3 fail on a difference. The screenshot is saved, not compared. |
| `assert_visual_match(name)` | Feature `visual`, not in Python. Compares the screenshot's pixels with `visual_baseline/<test>/<name>.png`, `<test>` being the test binary, module path and function joined with `.`, by structural similarity (threshold 0.999), saving it the first time; a mismatch fails and leaves `<name>.actual.png` and `<name>.diff.png`. See [Visual regression testing](/tutorials/remaining_helpers#visual-regression-testing-pixels). |

## Script execution and DOM modification

On a [direct CDP attachment](/ecosystem/tauri-api/cdp), `execute_script()` runs
the script as a function body with `window` as `this` and returns its result.
`is_connected()` uses that transport too. Callback-style
`execute_async_script()` remains WebDriver-specific. For screenshots on a
direct attachment, use `cdp_screenshot()` or the pure `Page` screenshot API.

| Method | Description |
|--------|-------------|
| `execute_script(script)` | Executes JavaScript and returns the result. |
| `execute_async_script(script)` | Executes asynchronous JavaScript. |
| `safe_execute_script(script, default)` | Executes JavaScript; on an error returns `default` (a `seleniumbase_rs::json::Value`) instead of failing. |
| `evaluate(script)` | Alias for `execute_script`. |
| `set_attribute(selector, attr, value)` | Sets an element attribute. |
| `remove_attribute(selector, attr)` | Removes an element attribute. |
| `add_css_link(href)` | Injects a `<link rel="stylesheet">`. |
| `add_js_link(src)` | Injects a `<script src="...">`. |
| `add_css_style(css)` | Injects a `<style>` block. |
| `add_js_code(js)` | Injects a `<script>` block. |
| `add_js_code_from_link(url)` | Downloads the script at `url` and injects its code inline. |
| `load_html_string(html)` | Loads raw HTML as the current page. |
| `load_html_file(path)` | Loads an HTML file as the current page. |
| `set_content(html)` | Alias for `load_html_string`. |
| `hide_element(selector)` | Hides an element via `display: none`. |
| `show_element(selector)` | Shows a hidden element. |
| `remove_element(selector)` | Removes an element from the DOM. |
| `console_log_string(text)` | Logs `text` to the browser console via JavaScript. |
| `console_log_script(script)` | Logs the value of the JavaScript expression `script` to the console. |
| `start_recording_console_logs()` | Starts capturing console logs. |
| `get_recorded_console_logs()` | Returns captured console log lines. |
