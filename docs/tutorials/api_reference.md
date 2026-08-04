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

Always call `quit()` in a `Drop` guard, `sb_test!` macro, or `finally` block so
browser processes do not leak when a test fails.

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
| `press_keys(key_name)` | Presses a single special key (e.g. `Enter`, `Tab`). |
| `clear(selector)` | Clears the element. |
| `submit(selector)` | Submits the parent form. |
| `hover(selector)` | Hovers over the element. |
| `hover_and_click(selector)` | Hovers then clicks the element. |
| `hover_and_double_click(selector)` | Hovers then double-clicks the element. |
| `drag_and_drop(src, dst)` | Drags one element onto another. |
| `drag_and_drop_with_offset(src, x, y)` | Drags an element by an offset. |
| `select_option_by_text(selector, text)` | Selects by visible text. |
| `select_option_by_value(selector, value)` | Selects by value. |
| `select_option_by_index(selector, index)` | Selects by zero-based index. |
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
| `press_up_arrow()` | Presses the Up arrow key. |
| `press_down_arrow()` | Presses the Down arrow key. |
| `press_left_arrow()` | Presses the Left arrow key. |
| `press_right_arrow()` | Presses the Right arrow key. |

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
| `set_time_limit(seconds)` | Caps wait timeouts globally. |
| `set_timeout(seconds)` | Sets the default implicit wait timeout. |
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

Deferred assertions collect failures and report them together when
`process_deferred_asserts` is called. Useful for soft assertions.

| Method | Description |
|--------|-------------|
| `deferred_assert_element(selector)` | Records an element-visible assertion. |
| `deferred_assert_text(text, selector)` | Records a text-visible assertion. |
| `deferred_assert_element_present(selector)` | Records an element-present assertion. |
| `deferred_assert_exact_text(text, selector)` | Records an exact-text assertion. |
| `deferred_assert_non_empty_text(selector)` | Records a non-empty-text assertion. |
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
| `set_content_to_frame(selector)` | Alias for `switch_to_frame`. |
| `set_content_to_default()` | Alias for `switch_to_default_content`. |
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
| `get_new_driver(config)` | Creates and switches to a new browser session. |
| `switch_to_driver(index)` | Switches to an extra driver created by `get_new_driver`. |
| `quit_extra_driver(index)` | Quits an extra driver. |
| `driver_count()` | Returns the number of extra drivers. |

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
| `save_screenshot(path)` | Writes screenshot to the logs directory. |
| `save_screenshot_to_path(path)` | Writes screenshot to an arbitrary path. |
| `screenshot_as_png()` | Returns the current page screenshot as PNG bytes. |
| `check_window(name, level)` | Compares a screenshot to a stored baseline. |

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
| `load_html_string(html)` | Loads raw HTML as the current page. |
| `load_html_file(path)` | Loads an HTML file as the current page. |
| `set_content(html)` | Alias for `load_html_string`. |
| `hide_element(selector)` | Hides an element via `display: none`. |
| `show_element(selector)` | Shows a hidden element. |
| `remove_element(selector)` | Removes an element from the DOM. |
| `console_log_string(text)` | Logs `text` to the browser console via JavaScript. |
| `start_recording_console_logs()` | Starts capturing console logs. |
| `get_recorded_console_logs()` | Returns captured console log lines. |

## File downloads, uploads, and PDF

| Method | Description |
|--------|-------------|
| `choose_file(selector, path)` | Sets a file input value. |
| `download_file(url, destination)` | Downloads `url` to `destination`. |
| `save_file_as(data, filename)` | Saves raw bytes to the downloads folder. |
| `print_to_pdf(filename)` | Saves the current page as a PDF. |
| `get_downloads_folder()` | Returns the default downloads directory. |
| `get_downloaded_files()` | Returns a list of downloaded file names. |
| `is_downloaded_file_present(filename)` | Returns true if the file exists in downloads. |
| `assert_downloaded_file(filename)` | Fails if the downloaded file is not present. |
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
| `clear_browser_cache()` | Clears cache. |
| `clear_browser_cookies()` | Clears cookies. |
| `get_cookies()` | Returns cookies as JSON. |
| `set_network_conditions(conditions)` | Network throttling. |
| `set_timezone(id)` | Sets timezone. |
| `set_geolocation(lat, lon, acc)` | Sets geolocation. |
| `uc_click(selector)` | Stealth click with delay. |
| `uc_type(selector, text)` | Stealth type with delay. |
| `uc_open_with_reconnect(url)` | Opens `url` after reconnecting the session. |
| `uc_open_with_disconnect(url)` | Opens `url` with a fresh session. |
| `human_click(selector)` | Human-like click. |
| `human_type(selector, text)` | Human-like type. |

## UC / GUI bypass helpers

These methods use native OS input instead of WebDriver to bypass some
anti-detection checks. They require a graphical desktop session.

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
| `export_recording_as_rust()` | Returns recorded actions as Rust source. |
| `save_recording_to_logs()` | Saves the recording as JSON and Rust source. |
| `save_recorded_actions(path)` | Saves recorded actions to a JSON file. |

## Utility / misc helpers

| Method | Description |
|--------|-------------|
| `get_beautiful_soup()` | Returns a BeautifulSoup-style parser for the page. |
| `get_beautiful_soup_object()` | Alias for `get_beautiful_soup`. |
| `convert_css_to_xpath(css)` | Converts a CSS selector to an XPath expression. |
| `convert_xpath_to_css(xpath)` | Converts an XPath expression to a CSS selector. |
| `ad_block()` | Injects an ad-blocking script. |
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
