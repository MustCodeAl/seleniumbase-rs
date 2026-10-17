---
title: "BaseCase API Reference"
---

# BaseCase API Reference

`BaseCase` is the main entry point for writing browser automation in
`seleniumbase-rs`. It wraps a WebDriver/CDP session, provides a fluent API for
navigation, interaction, waits, assertions, and exposes advanced helpers for
stealth, CDP, tours, and screenshots.

This page is a high-level reference. For exact signatures and generic bounds,
build the rustdocs with:

```bash
cargo doc --no-deps --open
```

## What you will learn

- The lifecycle methods for creating and closing sessions.
- Common navigation, interaction, and query methods.
- Wait and assertion patterns.
- Script execution, scrolling, file handling, and advanced interactions.
- CDP/UC helpers and when to use them.
- Where to find rustdocs for deeper detail.

## Lifecycle

| Method | Description |
|--------|-------------|
| `BaseCase::new(config).await` | Creates and connects a browser session from a `BrowserConfig`. |
| `BaseCase::without_session(config)` | Creates a `BaseCase` without connecting a driver (used by some runners). |
| `sb.quit().await` | Closes the browser and ends the session. |
| `sb.reconnect().await` | Replaces the WebDriver session while keeping the driver process. |
| `sb.restart().await` | Quits and restarts the browser with the same config. |

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
| `get_html_source()` | Returns the full page source. |
| `get_page_source()` | Alias for `get_html_source`. |
| `open_new_window()` | Opens a new browser window. |
| `open_new_tab()` | Opens a new browser tab. |

## Interaction

| Method | Description |
|--------|-------------|
| `click(selector)` | Clicks the element. |
| `double_click(selector)` | Double-clicks the element. |
| `context_click(selector)` | Right-clicks the element. |
| `slow_click(selector)` | Clicks after a small delay. |
| `click_if_visible(selector)` | Clicks only if the element is visible. |
| `click_visible_elements(selector)` | Clicks every visible matching element. |
| `click_with_offset(selector, x, y)` | Clicks with a pixel offset from the element center. |
| `click_chain(selectors)` | Clicks each selector in order. |
| `type_text(selector, text)` | Clears and types. |
| `add_text(selector, text)` | Appends text. |
| `send_keys(selector, text)` | Sends keystrokes to the element. |
| `press_keys(selector, text)` | Types `text` into the element one key at a time; a trailing `\n` presses Enter. |
| `press_key(key_name)` | Presses one named key (e.g. `enter`, `tab`) on the element that has focus. |
| `clear(selector)` | Clears the element. |
| `submit(selector)` | Submits the parent form. |
| `hover(selector)` | Hovers over the element. |
| `hover_and_click(selector)` | Hovers then clicks the element. |
| `hover_and_double_click(selector)` | Hovers then double-clicks the element. |
| `drag_and_drop(src, dst)` | Drags one element onto another. |
| `drag_and_drop_with_offset(src, x, y)` | Drags an element by an offset. |
| `select_option_by_text(selector, text)` | Selects by visible text. |
| `select_option_by_value(selector, value)` | Selects by value. |
| `select_option_by_index(selector, index)` | Selects by zero-based index and fires `input` and `change`; fails if there is no such option. |
| `js_click(selector)` | Clicks via JavaScript. |
| `js_type(selector, text)` | Types via JavaScript. |
| `highlight_click(selector)` | Highlights then clicks. |
| `highlight_type(selector, text)` | Highlights then types. |
| `check_if_unchecked(selector)` | Checks a checkbox/radio only if unchecked. |
| `uncheck_if_checked(selector)` | Unchecks a checkbox only if checked. |
| `is_checked(selector)` | Returns true if the element is checked. |
| `choose_file(selector, path)` | Sets a file input value. |

All `selector` arguments accept the same formats described in the
[Selectors guide](./selectors.md).

## Keyboard arrow keys

| Method | Description |
|--------|-------------|
| `press_up_arrow()` | Presses the Up arrow key once, on the element that has focus. |
| `press_down_arrow()` | Presses the Down arrow key once, on the element that has focus. |
| `press_left_arrow()` | Presses the Left arrow key once, on the element that has focus. |
| `press_right_arrow()` | Presses the Right arrow key once, on the element that has focus. |

## Scrolling

| Method | Description |
|--------|-------------|
| `scroll_to(selector)` | Scrolls the element into view. |
| `scroll_into_view(selector)` | Smooth-scrolls the element to the center. |
| `scroll_to_top()` | Scrolls to the top of the page. |
| `scroll_to_bottom()` | Scrolls to the bottom of the page. |
| `scroll_up()` | Scrolls up by one viewport. |
| `scroll_down()` | Scrolls down by one viewport. |
| `smooth_scroll_to(selector)` | Smooth-scrolls the element into view. |
| `scroll_by_y(pixels)` | Scrolls vertically by `pixels`. |

## Waits

| Method | Description |
|--------|-------------|
| `wait_for_element_present(selector, timeout)` | Waits for element in DOM. |
| `wait_for_element_visible(selector, timeout)` | Waits for element visible. |
| `wait_for_element_not_visible(selector, timeout)` | Waits for element invisible. |
| `wait_for_element_clickable(selector, timeout)` | Waits for element clickable. |
| `wait_for_element_absent(selector, timeout)` | Waits for element removed. |
| `wait_for_text_visible(text, selector, timeout)` | Waits for text to appear. |
| `wait_for_text_not_visible(text, selector, timeout)` | Waits for text to disappear. |
| `wait_for_ready_state_complete()` | Waits for `document.readyState === 'complete'`. |
| `set_time_limit(seconds)` | Fails the test at its next open, wait or element action once it has run longer than this. |
| `set_timeout(seconds)` | Sets the default implicit wait timeout. |
| `set_default_timeout(seconds)` / `reset_default_timeout()` | Changes the default wait (1 to 60 seconds) and puts it back. |
| `sleep(seconds)` | Async sleep helper. |
| `wait(seconds)` | Alias for `sleep` using whole seconds. |

See [Waits and Assertions](./waits_assertions.md) for detailed examples.

## Assertions

| Method | Description |
|--------|-------------|
| `assert_title(expected)` | Asserts page title equals `expected`. |
| `assert_text_visible(text, selector)` | Asserts text visible. |
| `assert_text_not_visible(text, selector)` | Asserts text not visible. |
| `assert_exact_text(text, selector)` | Asserts exact text match. |
| `assert_element_visible(selector)` | Asserts element is visible. |
| `assert_element_not_visible(selector)` | Asserts element is not visible. |
| `assert_attribute(selector, attr, value)` | Asserts attribute value. |
| `assert_url_contains(fragment)` | Asserts current URL contains fragment. |
| `assert_no_404_errors()` | Fails if any same-origin link returns HTTP 404. |
| `assert_link_text(text)` | Asserts a link with exact text exists. |
| `assert_any_of_elements_visible(selectors)` | Asserts at least one selector is visible. |
| `assert_equal(first, second)` | Asserts two values are equal. |
| `assert_not_equal(first, second)` | Asserts two values are not equal. |
| `assert_in(item, container)` | Asserts `item` is in `container`. |
| `assert_not_in(item, container)` | Asserts `item` is not in `container`. |
| `assert_true(value)` | Asserts `value` is true. |
| `assert_false(value)` | Asserts `value` is false. |
| `assert_raises(closure)` | Asserts `closure` returns an error. |

## Deferred assertions

Deferred assertions check the page at once, collect failures (with the URL of
the page each one saw) and report them together when `process_deferred_asserts`
is called. Useful for soft assertions.

| Method | Description |
|--------|-------------|
| `deferred_assert_element(selector)` | Checks the element is visible; records a failure if not. |
| `deferred_assert_text(text, selector)` | Checks the element contains the text; records a failure if not. |
| `deferred_assert_element_present(selector)` | Checks the element is in the page, visible or not. |
| `deferred_assert_exact_text(text, selector)` | Checks the element's text is exactly `text`. |
| `deferred_assert_non_empty_text(selector)` | Checks the element shows some text. |
| `deferred_check_window(name, level)` | Runs `check_window` and records a difference as a failure. |
| `process_deferred_asserts()` | Fails if any deferred assertion failed. |

## Queries

| Method | Description |
|--------|-------------|
| `is_element_present(selector)` | Returns true if in DOM. |
| `is_element_visible(selector)` | Returns true if visible. |
| `is_element_enabled(selector)` | Returns true if enabled. |
| `is_element_selected(selector)` | Returns true if selected. |
| `is_text_visible(text, selector)` | Returns true if text visible. |
| `get_text(selector)` | Returns element text. |
| `get_attribute(selector, attr)` | Returns attribute value. |
| `get_property(selector, prop)` | Returns property value. |
| `get_shadow_root(selector)` | Returns the shadow root. |
| `find_element(selector)` | Returns a `WebElement` handle. |
| `find_elements(selector)` | Returns all matching `WebElement` handles. |
| `get_unique_links()` | Returns unique `href` values from the page. |
| `get_all_links()` | Returns all `href` values from the page. |
| `get_link_status_code(url)` | Returns the HTTP status code for `url`. |

## Windows, frames, and multiple drivers

| Method | Description |
|--------|-------------|
| `switch_to_frame(selector)` | Enters a frame. |
| `switch_to_default_content()` | Returns to top document. |
| `switch_to_parent_frame()` | Goes to parent frame. |
| `set_content_to_frame(frame)` | Replaces the page with the content of an iframe on it, selected or named (an iframe with a web `src` opens in a new tab instead). |
| `switch_to_frame_of_element(selector)` | Enters the frame that holds the element; `None` if it is in this document. |
| `set_content_to_default()` | Brings back the page `set_content_to_frame` replaced; with nothing swapped, leaves the current frame like `switch_to_default_content`. |
| `set_content_to_parent()` | Undoes the last `set_content_to_frame`; with nothing swapped, like `switch_to_parent_frame`. |
| `switch_to_window(handle)` | Switches to a window. |
| `switch_to_new_window()` | Opens and switches to new window. |
| `switch_to_newest_window()` | Switches to the most recently opened window. |
| `switch_to_default_window()` | Switches back to the original window. |
| `close_window()` | Closes current window. |
| `maximize_window()` / `maximize()` | Maximizes window. |
| `minimize_window()` / `minimize()` | Minimizes window. |
| `set_window_size(w, h)` | Resizes window. |
| `set_window_position(x, y)` | Moves window. |
| `set_window_rect(x, y, w, h)` | Sets position and size. |
| `get_window_rect()` | Returns `(x, y, width, height)`. |
| `get_screen_rect()` | Returns `(width, height)`. |
| `get_new_driver(config)` | Opens a new browser session and makes it active (`None` reuses the current config). The previously active session is parked and the method returns its index. |
| `switch_to_driver(index)` | Swaps the active session with the one parked at `index`. The session you leave stays open, parked at the same index, so the same call switches back. An out-of-range index returns `InvalidConfig` and changes nothing. |
| `switch_to_default_driver()` | `switch_to_driver(0)`: a swap with slot 0, which holds the original session only until the first switch. |
| `quit_extra_driver(index)` | Quits the parked session at `index`. The slot stays as a disconnected placeholder, so indices do not shift. |
| `driver_count()` | Returns the number of parked sessions, including quit placeholders. |

## Cookies and storage

| Method | Description |
|--------|-------------|
| `save_cookies(path)` | Saves cookies to JSON. |
| `load_cookies(path)` | Loads cookies from JSON. |
| `add_cookie(name, value)` | Adds a browser cookie. |
| `get_cookie(name)` | Returns a cookie by name. |
| `delete_all_cookies()` | Deletes all cookies. |
| `set_local_storage_item(key, value)` | Sets localStorage item. |
| `get_local_storage_item(key)` | Gets localStorage item. |
| `remove_local_storage_item(key)` | Removes localStorage item. |
| `clear_local_storage()` | Clears localStorage. |
| `set_session_storage_item(key, value)` | Sets sessionStorage item. |
| `get_session_storage_item(key)` | Gets sessionStorage item. |
| `clear_session_storage()` | Clears sessionStorage. |

## Screenshots

| Method | Description |
|--------|-------------|
| `save_screenshot(filename)` | Writes a screenshot to the logs directory. `filename` must be a plain file name (no directory part, no `..`); a file of that name is replaced. |
| `save_screenshot_to_path(path)` | Writes screenshot to an arbitrary path. |
| `screenshot_as_png()` | Returns the current page screenshot as PNG bytes. |
| `check_window(name, level)` | Compares the page's tag structure to a baseline saved under `visual_baseline/<test>/<name>/`, as Python 4.55.2 does: level 1 the tag names in order, level 2 adds the sorted attribute names, level 3 adds the attribute values. The first run (or a missing file) saves the baseline; level 0 is a dry run that only logs a difference; levels 1 to 3 fail on a difference. The screenshot is saved, not compared. |
| `assert_visual_match(name)` | Feature `visual`, not in Python. Compares the screenshot's pixels with `visual_baseline/<test>/<name>.png`, `<test>` being the test binary, module path and function joined with `.`, by structural similarity (threshold 0.999), saving it the first time; a mismatch fails and leaves `<name>.actual.png` and `<name>.diff.png`. See [Visual regression testing](./remaining_helpers.md#visual-regression-testing-pixels). |

## Script execution and DOM modification

| Method | Description |
|--------|-------------|
| `execute_script(script)` | Executes JavaScript and returns the result. |
| `execute_async_script(script)` | Executes asynchronous JavaScript. |
| `safe_execute_script(script)` | Executes JavaScript and returns `Option<Value>`. |
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

## File downloads, uploads, and PDF

| Method | Description |
|--------|-------------|
| `choose_file(selector, path)` | Sets a file input value. |
| `download_file(url, destination)` | Downloads `url` to `destination`. |
| `save_file_as(data, filename)` | Saves raw bytes to the logs directory. `filename` must be a plain file name. |
| `print_to_pdf(filename)` | Saves the current page as a PDF. |
| `get_downloads_folder()` | Returns the default downloads directory. |
| `get_downloaded_files()` | Returns a list of downloaded file names. |
| `is_downloaded_file_present(filename)` | Returns true if the file exists in downloads. |
| `assert_downloaded_file(filename)` | Waits up to the default timeout for the file, then fails if it is not present. |
| `get_pdf_text(filename)` | Extracts text from a PDF file. |
| `assert_pdf_text(filename, text)` | Fails if the PDF does not contain `text`. |

## CDP / UC helpers

| Method | Description |
|--------|-------------|
| `activate_cdp_mode()` | Enables CDP domains. |
| `execute_cdp(method)` | Sends a CDP command. |
| `execute_cdp_with_params(method, params)` | Sends a CDP command with params. |
| `cdp_mouse_click(x, y)` | CDP mouse click. |
| `cdp_type_text(text)` | CDP text insert. |
| `cdp_click_element(selector)` | CDP click at element center. |
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

See the [GUI Automation](./gui_automation.md) tutorial for details.

## Alerts

| Method | Description |
|--------|-------------|
| `accept_alert()` | Accepts the current alert. |
| `dismiss_alert()` | Dismisses the current alert. |
| `switch_to_alert()` | Focuses the current alert. |
| `is_alert_present()` | Returns true if an alert is present. |
| `get_alert_text()` | Returns the alert text. |
| `type_alert_text(text)` | Types into a prompt alert. |

## MFA / TOTP

| Method | Description |
|--------|-------------|
| `get_totp_code(secret)` | Generates a TOTP code from a base32 secret. |
| `get_mfa_code(secret)` | Alias for `get_totp_code`. |
| `enter_mfa_code(secret, selector)` | Generates and types a TOTP code into an input. |

## Tours, presentations, charts

| Method | Description |
|--------|-------------|
| `create_tour(name)` | Creates a tour with the default theme. |
| `create_tour_with_theme(name, theme)` | Creates a tour with a specific `TourTheme`. |
| `create_shepherd_tour(name)` / `create_introjs_tour(name)` / `create_driverjs_tour(name)` / `create_bootstrap_tour(name)` / `create_hopscotch_tour(name)` | Convenience themed constructors. |
| `add_tour_step(message, target)` | Adds a tour step; `target` is an optional CSS selector. |
| `play_tour()` / `start_tour()` | Plays the tour in the current page. |
| `export_tour(path)` | Exports tour HTML. |
| `create_presentation(name)` | Creates HTML presentation. |
| `add_presentation_slide(html)` | Adds a slide. |
| `save_presentation(path)` | Saves presentation. |
| `create_pie_chart(name)` / `create_bar_chart(name)` / ... | Creates a chart. |
| `add_data_point(label, value)` | Adds data to the current chart. |
| `save_chart(path)` | Saves chart HTML. |

See the [Tours](./tours.md) and [Charts](./charts.md) pages for details.

## Recorder

| Method | Description |
|--------|-------------|
| `activate_recorder()` | Injects the browser-side action recorder. |
| `recorded_actions()` | Returns actions captured so far. |
| `export_recording_as_rust()` | Returns recorded actions as Rust source (pure CDP code when `rec_sb_cdp()` is on). |
| `export_recording_as_cdp_rust()` | Returns recorded actions as a pure CDP program (`Page` and `Locator` calls). |
| `save_recording_to_logs()` | Saves the recording as JSON and Rust source. |
| `save_recorded_actions(path)` | Saves recorded actions to a JSON file. |

## Utility / misc helpers

| Method | Description |
|--------|-------------|
| `get_beautiful_soup()` | Returns a BeautifulSoup-style parser for the page. |
| `get_beautiful_soup_object()` | Alias for `get_beautiful_soup`. |
| `convert_css_to_xpath(css)` | Converts a CSS selector to an XPath expression. |
| `convert_xpath_to_css(xpath)` | Converts a simple XPath expression (a tag with attribute tests) to a CSS selector; any other XPath is an `InvalidSelector` error. |
| `ad_block()` | Removes the ad elements of upstream's `AD_BLOCK_LIST` from the current page. |
| `disable_beforeunload()` | Disables `beforeunload` prompts. |
| `print_unique_links_with_status_codes()` | Prints all unique links and their HTTP status codes. |

## Common error handling pattern

Most `BaseCase` methods return `Result<T, SeleniumBaseError>`. Use `?` inside an
async function or the `sb_test!` macro:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

async fn login(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.open("https://example.com/login").await?;
    sb.type_text("#username", "alice").await?;
    sb.type_text("#password", "secret").await?;
    sb.click("#submit").await?;
    sb.assert_element_visible("#dashboard").await?;
    Ok(())
}
```

## Further reading

- [Selectors](./selectors.md)
- [Waits and Assertions](./waits_assertions.md)
- [CDP Mode](./cdp_mode.md)
- [UC Mode](./uc_mode.md)
- [Macros](./macros.md)
- [GUI Automation](./gui_automation.md)

For exact signatures, run `cargo doc --no-deps --open`.

---

<Note>

Verified against commit 8e9180f: only the multi-driver rows (`get_new_driver` to `driver_count`) were re-checked against the code in this pass; the rest of this page was not.

</Note>
