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
- The capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`) that both engines implement.
- The public items re-exported at the crate root and what each is for.
- Where to find rustdocs for deeper detail.

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

## Interaction

| Method | Description |
|--------|-------------|
| `click(selector)` | Clicks the element. |
| `double_click(selector)` | Double-clicks the element. |
| `context_click(selector)` | Right-clicks the element. |
| `slow_click(selector)` | Clicks after a small delay. |
| `click_if_visible(selector)` | Clicks only if the element is visible. |
| `click_visible_elements(selector)` | Clicks every visible matching element. |
| `click_with_offset(selector, x, y, center)` | Clicks at a pixel offset from the element's top-left corner, or from its centre when `center` is true, as Python does. |
| `double_click_with_offset(selector, x, y, center)` | The same with a double click. |
| `click_chain(selectors)` | Clicks each selector in order. |
| `type_text(selector, text)` | Clears and types. |
| `add_text(selector, text)` | Appends text. |
| `send_keys(selector, text)` | Sends keystrokes to the element. |
| `press_keys(selector, text)` | Types `text` into the element one key at a time; a trailing `\n` presses Enter. |
| `press_key(key_name)` | Presses one named key (e.g. `enter`, `tab`) on the element that has focus. |
| `clear(selector)` | Clears the element. |
| `submit(selector)` | Submits the parent form. |
| `hover(selector)` | Hovers over the element. |
| `hover_and_click(hover_selector, click_selector)` | Hovers over the first element, pauses 0.1 s, then clicks the second. |
| `hover_and_double_click(selector)` | Hovers then double-clicks the element. |
| `drag_and_drop(src, dst)` | Drags one element onto another. |
| `drag_and_drop_with_offset(src, dst, x, y)` | Drags `src` to `dst` with an offset (`x` and `y` are `i64`). |
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
| `wait_for_text_visible(text, selector, timeout_secs)` | Waits until the element shows the text (pass `"body"` for the whole page). |
| `wait_for_text_not_visible(text, selector, timeout_secs)` | Waits until the element no longer shows the text. |
| `wait_for_text(text, selector, timeout_secs)` | Waits until the element's text contains `text`; the same as `wait_for_text_visible`. |
| `wait_for_exact_text(text, selector, timeout_secs)` | Waits until the element's text equals `text` (whitespace at the ends ignored). |
| `wait_for_ready_state_complete()` | Waits for `document.readyState === 'complete'`. |
| `set_time_limit(seconds)` | Fails the test at its next open, wait or element action once it has run longer than this. |
| `set_timeout(seconds)` | Sets the default implicit wait timeout. |
| `set_default_timeout(seconds)` / `reset_default_timeout()` | Changes the default wait (1 to 60 seconds) and puts it back. |
| `sleep(seconds)` | Async sleep helper. |
| `wait(seconds)` | The same as `sleep`; takes whole or fractional seconds (`wait(2)`, `wait(0.5)`). |

See [Waits and Assertions](./waits_assertions.md) for detailed examples.

## Assertions

| Method | Description |
|--------|-------------|
| `assert_title(expected)` | Asserts page title equals `expected`. |
| `assert_text_visible(text, selector)` | Asserts text visible. |
| `assert_text_not_visible(text, selector)` | Asserts text not visible. |
| `assert_text(text, selector)` | Waits for the element to show the text, then asserts it. Same as `assert_text_visible`. |
| `assert_exact_text(text, selector)` | Asserts the element's text equals `text` exactly (whitespace at the ends ignored); waits like `assert_text`. |
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
| `is_selected(selector)` | Returns true if the element is selected. |
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
| `get_new_driver()` / `get_new_driver_with(config)` | Opens a new browser with the current configuration (or `config`), makes it active and returns its `DriverId`. The browser that was active stays open. |
| `switch_to_driver(id)` | Makes the browser `id` the active one. Repeating the call changes nothing. An id that does not exist or was quit returns `InvalidConfig` and changes nothing. |
| `switch_to_default_driver()` | Makes the browser the case started with (`DriverId::DEFAULT`) the active one, however many others were opened. |
| `quit_extra_driver(id)` | Quits the browser `id`. When it is the active one, the case switches to the default browser. The default browser itself is quit with `quit()`. |
| `active_driver()` | The `DriverId` of the active browser. |
| `driver_count()` | The number of browsers still open, the active one included. |

## Cookies and storage

| Method | Description |
|--------|-------------|
| `save_cookies(name)` | Saves the cookies, as JSON, to `saved_cookies/<name>.txt` in the working directory (the last path part of `name`, `.txt` added) and returns the path. |
| `load_cookies(name)` | Adds the cookies in `saved_cookies/<name>.txt`; a cookie of another domain gets the page's and the expiry is dropped. `load_cookies_with(name, CookieExpiry)` says what to do with the expiry. |
| `get_saved_cookies(name)` | Reads `saved_cookies/<name>.txt` without adding the cookies. |
| `delete_saved_cookies(name)` | Deletes `saved_cookies/<name>.txt` (not the cookies of the browser). |
| `save_cookies_to_path(path)` / `load_cookies_from_path(path, expiry)` | The same for a file at any path. |
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
| `safe_execute_script(script, default)` | Executes JavaScript; on an error returns `default` (a `serde_json::Value`) instead of failing. |
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
| `download_file(url, filename, timeout_secs)` | Navigates to `url` in a new tab, waits up to `timeout_secs` for `filename` in the downloads folder, returns to the first tab and returns the file's `PathBuf`. |
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
| `cdp_mouse_click(x, y)` | CDP mouse click at viewport coordinates. |
| `cdp_insert_text(text)` | CDP text insert into whatever has focus (`cdp_type_text` is the deprecated old name). |
| `cdp_click(selector)` | Scroll the element into view and click its centre through CDP. |
| `cdp_click_element(selector)` | Deprecated: use `cdp_click`. |
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
| `enter_mfa_code(selector, secret)` | Generates a TOTP code, types it into an input and presses Enter. |

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

## Capability traits

`seleniumbase_rs::{BrowserApi, ElementApi, AssertionApi, ScreenshotApi}` are
four small `async_trait` traits (defined in `src/api/traits.rs`). Both engines
implement all four: `BaseCase` (WebDriver) and `sb_cdp::Page` (pure CDP). A helper
written over the traits runs on either engine. None of them names an engine
type; to keep an element handle, use `BaseCase::find_element` (a
`thirtyfour::WebElement`) or `Page::locator(..)`. Every method returns
`seleniumbase_rs::Result<T>`. Selector strings are the same on both engines:
CSS by default, XPath for `//...`, link text for `link=...` (see the
[Selectors guide](./selectors.md)).

```rust,no_run
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi};

async fn sign_in<E>(sb: &mut E) -> Result<(), seleniumbase_rs::SeleniumBaseError>
where
    E: BrowserApi + ElementApi + AssertionApi,
{
    sb.open("https://example.com/login").await?;
    sb.type_text("#user", "alice").await?;
    sb.click("#submit").await?;
    sb.assert_element("#dashboard").await
}
```

### BrowserApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `open(&mut self, url: &str) -> Result<()>` | Navigates the active window; a URL without a scheme gets `https://`; a file path or an unsupported scheme (`ftp:`, `javascript:`, `mailto:`) is refused with `InvalidConfig` (use `file://` for a local file). | `Page::goto(url)`. |
| `quit(&mut self) -> Result<()>` | Closes the browser session. | Closes this tab only. To close the browser call `Browser::close`. |
| `refresh(&self) -> Result<()>` | Reloads the page. | `Page::reload()`. |
| `go_back(&self) -> Result<()>` | Browser back. | `Page::back()`. |
| `go_forward(&self) -> Result<()>` | Browser forward. | `Page::forward()`. |
| `get_title(&mut self) -> Result<String>` | The page title. | `Page::title()`. |
| `get_url(&mut self) -> Result<String>` | The current URL. | `Page::url()`. |

### ElementApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `click(&mut self, css: &str) -> Result<()>` | Clicks the element. | `locator(css).click()`. |
| `double_click(&mut self, css: &str) -> Result<()>` | Double-clicks it. | `locator(css).double_click()`. |
| `type_text(&mut self, css: &str, text: &str) -> Result<()>` | Waits for the element to be visible, clears it and types. | Clears the field, then types one key at a time. |
| `get_text(&mut self, css: &str) -> Result<String>` | The element's visible text, after waiting for it to be present. | `locator(css).text()`. |
| `get_attribute(&mut self, css: &str, attr: &str) -> Result<Option<String>>` | The attribute value, `None` when absent. | `locator(css).attribute(attr)`. |

### AssertionApi

| Method | `BaseCase` | `Page` |
|---|---|---|
| `assert_title(&mut self, expected: &str) -> Result<()>` | Fails with `AssertionFailed` unless the title equals `expected` exactly. | `expect().to_have_title(expected)`, which retries until the timeout. |
| `assert_element(&self, css: &str) -> Result<()>` | Fails unless the element is present in the DOM (visible or not). | `locator(css).expect().to_exist()`. |
| `assert_text(&mut self, expected: &str, css: &str) -> Result<()>` | Fails unless the element's text contains `expected`. | `locator(css).expect().to_contain_text(expected)`. |
| `assert_no_js_errors(&self) -> Result<()>` | Fails if the browser log holds a `SEVERE` entry (a few known CDN sources are ignored). | Fails if the document raised an uncaught script error; the message names the first. |

<Note>

The trait method `assert_text(expected, css)` takes the text first, like
`BaseCase::assert_text(text, selector)` and Python's `assert_text`.

</Note>

### ScreenshotApi

| Method | Behaviour |
|---|---|
| `save_screenshot(&self, filename: &str) -> Result<PathBuf>` | Saves a PNG into the logs directory and returns its path. `filename` must be a plain file name (no directory part, no `..`). The same for both engines. |
| `screenshot_as_png(&self) -> Result<Vec<u8>>` | The current page as PNG bytes. |

## Public items of the crate root

Everything below is importable as `seleniumbase_rs::Name` (from `src/lib.rs`).
Items marked with a feature need that Cargo feature; the default feature set
is empty.

### Browser, configuration and test runner

| Item | What it is |
|---|---|
| `BaseCase` | The WebDriver-engine test case; every method in the tables above. `BaseCase::new(config).await` connects, `BaseCase::without_session(config)` does not. |
| `BrowserConfig` | The session configuration. Fields and `with_*` builders are listed in [Getting Started](./getting_started.md#configuration). `BrowserConfig::default()` reads no environment variables; `BrowserConfig::from_env()` returns `Result<BrowserConfig>` and honours the `SB_*` variables. |
| `Browser` | Enum of browsers: `Chrome` (default), `Chromium`, `Edge`, `Firefox`, `Brave`, `Comet`, `Opera`, `Safari`, `InternetExplorer`, `ChromeForTesting`, `ChromeHeadlessShell`. It parses from text such as `chrome`, `edge`, `ie`, `cft`, `chs` (`FromStr`). `is_chromium_based()` and `name()` are `const fn`s. |
| `DriverMode` | `WebDriver` (default), `Cdp` or `Uc` (undetected mode). |
| `BrowserSession` | Owns the WebDriver session, an optional CDP client and an optional auto-started driver process. `BrowserSession::connect(config).await` is what `BaseCase::new` calls. |
| `RunSettings`, `PauseKind` | Run-wide behaviour, Python's `--demo`, `--slow`, `--demo-sleep`, `--message-duration`, `--skip-js-waits`, `--record-sleep` and `--visual-baseline`: built with `RunSettings::default().with_slow_mode(true)` and the other `with_*` methods, given with `BrowserConfig::with_run_settings` or `BaseCase::set_run_settings`. The default changes nothing. `pause_for(PauseKind::Full \| Short)` is the pause after an action: full when the page changed, 1/3.4 of it otherwise in demo mode. |
| `Settings`, `SettingsOverrides` | The settings file (TOML or JSON) and the `SB_*` variables: `Settings::load()` reads both, `load_from(path)` one file; `validate()` refuses an unknown browser or mode; `to_browser_config()` makes the `BrowserConfig`. `plugins` is the `[plugins]` table (with `SB_PLUGINS`). `SettingsOverrides::apply(&mut settings)` lays command-line values over them. |
| `DriverId` | Names one browser of a `BaseCase`: `DriverId::DEFAULT` for the one it started with, or the id `get_new_driver` returned; `index()` is its position. It stays the same browser while the case lives. |
| `CookieExpiry` | What `load_cookies_with` does with expiries, Python's `expiry` argument: `Session` (the default, drop it), `Day` (24 hours), `Saved` (keep the saved one) or `Minutes(n)`. |
| `run_browser_test(config, test)` | Starts a `BaseCase`, runs `test` (an async closure over `&mut BaseCase`), always calls `quit`, and returns the test's error, the cleanup error, or both. A panic in the test still closes the browser and is then raised again. A test that ends with `skip_test` counts as passed. The run settings of `config` (`with_run_settings`) go to the case first. |
| `run_cdp_test(options, test)` | The same for the Pure CDP engine: launches a browser, runs `test` (an async closure over `&Page`) on its first tab and always closes the browser. `Plugins::run_cdp_test` adds the plugins. |
| `run_browser_test_boxed`, `BrowserTestFuture<'a>` | Deprecated: the closure form that returns `Pin<Box<dyn Future<Output = Result<()>> + 'a>>`, from before async closures. |
| `shutdown_reused_sessions() -> usize` | Closes every idle browser kept for `reuse_session` and returns how many it closed. A browser a test still holds is left alone. |
| `install_tls_provider()` | Installs `ring` as the process-wide rustls crypto provider unless one is installed. Call it before you build your own `reqwest` client in a program that uses this crate, which builds `reqwest` without a provider; a client built before any provider is set panics. |
| `RuntimeConfig`, `LogFormat` | Environment configuration (below). `LogFormat` is `Pretty` (default) or `Json`. |
| `Gui` (feature `gui`) | Native mouse and keyboard through `enigo`. Not stored in `BaseCase`; build a fresh one where you need it. |

### Errors

| Item | What it is |
|---|---|
| `Result<T>` | `std::result::Result<T, SeleniumBaseError>`. |
| `SeleniumBaseError` | The one error type (a `thiserror` enum). Variants: `ElementNotFound`, `ElementNotInteractable`, `StaleElement`, `InvalidSelector`, `WaitTimeout`, `BrowserLaunch`, `BrowserDisconnected`, `SessionNotStarted`, `Navigation`, `WebDriver`, `CdpDriver`, `Playwright`, `Patcher`, `Stealth`, `Network`, `Download`, `Io`, `Json`, `Parse`, `InvalidConfig`, `AssertionFailed`, `Unsupported`, `Gui`, `Screenshot`, `Pdf`, `Authentication`, `Mcp`, `PythonMigration`, `Skipped`, `TestLifecycle`, `Database`, `Plugin`. |
| Error helpers | `is_transient()` is true for `BrowserDisconnected`, `Network`, `Download` and `WaitTimeout`. `is_skipped()`, `is_element_error()`, `category()` (a stable snake_case tag such as `wait_timeout`), `hint()` (`Option<String>`), `exit_code()` (`u8`), `log()` and `log_in_context(operation)` (a structured `tracing::error!`). Constructors such as `SeleniumBaseError::element_not_found(selector)`, `wait_timeout(operation, duration)`, `invalid_config(reason)`, `assertion_failed(reason)`. |
| `ResultExt<T, E>` | On any `Result<T, E: Display>`: `sb_context(context)` turns the error into `InvalidConfig("context: error")`, and `log_err()` logs it with `tracing::error!` and passes it through. |

### Retry, selectors and logging

| Item | What it is |
|---|---|
| `RetryPolicy` | Fields `max_attempts`, `base_delay`, `backoff_factor`, `max_delay`. `RetryPolicy::default()` is 3 attempts, 200 ms, factor 2.0, at most 5 s. Presets: `webdriver()` (3, 250 ms, 3 s), `network()` (5, 500 ms, 10 s), `seleniumbase()` (6, 1 s, 32 s, as Python's `retry_on_exception`), `every(interval)` (a fixed interval that never gives up, for `poll_until`). `attempts()` is never below 1, `delay_for_attempt(n)` is zero for the first attempt, and `retry(..)` and `retry_blocking(..)` run an operation again, with backoff, while a predicate says the error is worth retrying. `utilities::retry` also has `poll_until` and `RateLimiter`. |
| `Selector` | A borrowed selector with its kind decided: `LinkText`, `PartialLinkText`, `Css`, `XPath`, `Id`. `Selector::auto(text)` guesses from the start of the string: `link=`, `link_text=`, `text=` is exact link text; `partial_link=`, `partial_link_text=`, `partial_text=`, `p_link=`, `p_link_text=`, `p_text=` is partial link text; `/`, `./`, `../`, `(` or an XPath axis such as `parent::` is XPath; anything else is CSS. `to_by()` and `auto_by(text)` return a `thirtyfour::By` and fail with `InvalidSelector` for empty text. The pure CDP engine uses the owned twin `utils::selectors::SelectorBuf`. |
| `init_tracing()`, `init_tracing_json()` | Install the global `tracing` subscriber (text or JSON) and bridge `log`. They read `RUST_LOG` (default `info`) and ignore a second call. |
| `init_tracing_with_filter(filter)`, `init_tracing_json_with_filter(filter)` | The same with an `EnvFilter` string; an unparsable filter falls back to `RUST_LOG`, then `info`. The process environment is not changed. |
| `init_tracing_from_runtime(&RuntimeConfig)` | Uses `SB_LOG_LEVEL` and `SB_LOG_FORMAT`; without a valid `SB_LOG_LEVEL` the filter is `RUST_LOG`, then `info`. |
| `UPSTREAM_SELENIUMBASE_VERSION` | The constant `"4.55.2"`, the Python SeleniumBase version the API tracks. |

### Charts and tours

| Item | What it is |
|---|---|
| `Chart`, `ChartType` | A chart that renders to HTML with Chart.js: `Chart::new(title, chart_type)` with `ChartType::Pie`, `Bar` (horizontal), `Line`, `Area` or `Column` (vertical). Data points are `(label, i32)` pairs; extra series go in `extra_series`. See [Charts](./charts.md). |
| `TourTheme` | `SeleniumBase` (default), `Shepherd`, `IntroJs`, `DriverJs`, `Bootstrap`, `Hopscotch`; `as_str()` gives the lowercase name. See [Tours](./tours.md). |

### Fingerprint and stealth

These are re-exported for convenience; the [Fingerprint & Stealth Profiles](./fingerprint_stealth.md)
tutorial is the reference for them.

| Item | What it is |
|---|---|
| Fingerprint types | `Fingerprint` (built once with `Fingerprint::builder()`, read through accessors, changed with `to_builder()`), `FingerprintBuilder`, `FingerprintError`, `Spoof<T>` (each setting: `Natural`, `Random`, `Value(T)` or `Off`), `StealthPolicy` (how the claims are delivered; presets `default()`, `balanced()`, `native_only()`, `max()`), `Dimension`, and the value types `ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices`, `ClientHints`, `BrandVersion`, `ProxyConfig`, `BatteryProfile`, `ConnectionProfile`, `SpeechVoice`, `OsType`, `BrowserType`; the policy enums `CanvasNoise`, `AudioNoise`, `QuicMode`, `WebRtcPolicy`; `HumanizeConfig` and `CoherenceReport`. `StealthFlags` is an old name of `StealthPolicy`. |
| Evasion providers | `default_registry()` returns an `EvasionRegistry` with every built-in provider. `session_registry(&Fingerprint)` is the registry for a pure CDP tab: it leaves out the providers named in `SESSION_OMITTED_PROVIDERS` (`"plugins"` and `"connection"`), because Chrome already answers for those itself. `EvasionProvider` is the trait for your own, `EvasionContext` the data a provider's script is built from, `EvasionConfig` its settings. |
| Patching | `find_system_chrome() -> Option<PathBuf>` (checks `CHROME_BIN` and `CHROMIUM_BIN`, well-known paths, then `which`), `engine_spoofing_args() -> Vec<String>` (launch flags that reduce engine-level automation signals), `ChromeBinaryPatcher`, `ChromedriverPatcher`, `EnginePatch`. See [Binary patching](./binary_patching.md). |

### Python migration

`import_python(source: &str, options: &ImportOptions) -> ImportResult` converts
common Python SeleniumBase or Selenium statements into a Rust test.
`ImportOptions` has `source: PythonSource` (`Auto` by default, `SeleniumBase` or
`Selenium`) and `test_name` (default `imported_python_test`). `ImportResult` has
`rust` (the generated source), `diagnostics: Vec<ImportDiagnostic>` (each with
`line`, `severity` of `Warning` or `Error`, and `message`) and `detected_source`;
`is_complete()` is true when there are no error-level diagnostics. Dynamic
Python and unsupported locators stay in the output as `TODO` comments with a
diagnostic.

### Modules

The remaining public API sits in modules: `api`, `artifacts`, `behave`,
`browser`, `cli`, `common`, `config`, `core`, `error`, `js_code`, `macros`,
`plugins` (test-reporting plugins), `profile_payloads`, `resources`,
`sb_cdp` (the [pure CDP engine](./pure_cdp.md)), `stealth`, `tracing_util`,
`utilities`, `utils`; and, behind features, `mcp` (`mcp-server`), `ocr` (`ocr`),
`plugin_api` (`plugins`, the browser plugin API), `builtin_plugins` (any of
`plugin-shield`, `plugin-block-resources`, `plugin-crx` or `plugins-builtin`: the
official plugins and their `registry()`, see [Choosing plugins](../plugins/selecting.md)),
`storage` (`turso`) and `visual` (`visual`).

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

Checked against the source of this branch: the argument lists and argument order of the `BaseCase` tables (names and counts, by script), the capability traits and the crate-root items. The multi-driver rows (`get_new_driver` to `driver_count`) were checked earlier, against commit 8e9180f. The one-line descriptions of the other `BaseCase` methods were not all re-read.

</Note>
