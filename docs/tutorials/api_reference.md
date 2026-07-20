# BaseCase API Reference

`BaseCase` is the main entry point for writing tests. All methods are `async` and return `Result<T, SeleniumBaseError>` unless noted.

## Lifecycle

| Method | Description |
|--------|-------------|
| `BaseCase::new(config).await` | Creates and connects a browser session. |
| `sb.quit().await` | Closes the browser and ends the session. |
| `sb.reconnect().await` | Replaces the WebDriver session while keeping the driver process. |

## Navigation

| Method | Description |
|--------|-------------|
| `open(url)` | Navigates to `url`. |
| `go_back()` | Browser back. |
| `go_forward()` | Browser forward. |
| `refresh_page()` | Reloads the page. |
| `get_current_url()` | Returns the current URL. |
| `get_title()` | Returns the page title. |
| `get_html_source()` | Returns the full page source. |

## Interaction

| Method | Description |
|--------|-------------|
| `click(css)` | Clicks the element. |
| `double_click(css)` | Double-clicks the element. |
| `context_click(css)` | Right-clicks the element. |
| `type(css, text)` | Clears and types. |
| `add_text(css, text)` | Appends text. |
| `clear(css)` | Clears the element. |
| `submit(css)` | Submits the parent form. |
| `hover(css)` | Hovers over the element. |
| `drag_and_drop(src, dst)` | Drags one element onto another. |
| `select_option_by_text(css, text)` | Selects by visible text. |
| `select_option_by_value(css, value)` | Selects by value. |
| `js_click(css)` | Clicks via JavaScript. |
| `js_type(css, text)` | Types via JavaScript. |
| `highlight_click(css)` | Highlights then clicks. |
| `highlight_type(css, text)` | Highlights then types. |

## Waits

| Method | Description |
|--------|-------------|
| `wait_for_element_present(css, timeout)` | Waits for element in DOM. |
| `wait_for_element_visible(css, timeout)` | Waits for element visible. |
| `wait_for_element_not_visible(css, timeout)` | Waits for element invisible. |
| `wait_for_element_clickable(css, timeout)` | Waits for element clickable. |
| `wait_for_element_absent(css, timeout)` | Waits for element removed. |
| `wait_for_ready_state_complete()` | Waits for `document.readyState === 'complete'`. |
| `set_time_limit(seconds)` | Caps wait timeouts globally. |

## Assertions

| Method | Description |
|--------|-------------|
| `assert_title(expected)` | Asserts page title. |
| `assert_text_visible(text, css)` | Asserts text visible. |
| `assert_text_not_visible(text, css)` | Asserts text not visible. |
| `assert_attribute(css, attr, value)` | Asserts attribute value. |
| `assert_no_404_errors()` | Fails on captured 404s. |
| `assert_no_js_errors()` | Fails on captured JS errors. |

## Queries

| Method | Description |
|--------|-------------|
| `is_element_present(css)` | Returns true if in DOM. |
| `is_element_visible(css)` | Returns true if visible. |
| `is_element_enabled(css)` | Returns true if enabled. |
| `is_element_selected(css)` | Returns true if selected. |
| `is_text_visible(text, css)` | Returns true if text visible. |
| `get_text(css)` | Returns element text. |
| `get_attribute(css, attr)` | Returns attribute value. |
| `get_property(css, prop)` | Returns property value. |
| `get_shadow_root(css)` | Returns the shadow root. |

## Windows and frames

| Method | Description |
|--------|-------------|
| `switch_to_frame(css)` | Enters a frame. |
| `switch_to_default_content()` | Returns to top document. |
| `switch_to_parent_frame()` | Goes to parent frame. |
| `switch_to_window(handle)` | Switches to a window. |
| `switch_to_new_window()` | Opens and switches to new window. |
| `close_window()` | Closes current window. |
| `maximize_window()` | Maximizes window. |
| `minimize_window()` | Minimizes window. |
| `set_window_size(w, h)` | Resizes window. |
| `set_window_position(x, y)` | Moves window. |
| `set_window_rect(x, y, w, h)` | Sets position and size. |

## Cookies and storage

| Method | Description |
|--------|-------------|
| `save_cookies(path)` | Saves cookies to JSON. |
| `load_cookies(path)` | Loads cookies from JSON. |
| `set_local_storage_item(key, value)` | Sets localStorage item. |
| `get_local_storage_item(key)` | Gets localStorage item. |
| `remove_local_storage_item(key)` | Removes localStorage item. |
| `clear_local_storage()` | Clears localStorage. |

## Screenshots

| Method | Description |
|--------|-------------|
| `save_screenshot(path)` | Writes screenshot to file. |
| `save_screenshot_as_png()` | Returns PNG bytes. |
| `check_window(name)` | Compares screenshot to baseline. |

## CDP / UC helpers

| Method | Description |
|--------|-------------|
| `activate_cdp_mode()` | Enables CDP domains. |
| `execute_cdp(method)` | Sends a CDP command. |
| `execute_cdp_with_params(method, params)` | Sends a CDP command with params. |
| `cdp_mouse_click(x, y)` | CDP mouse click. |
| `cdp_type_text(text)` | CDP text insert. |
| `cdp_click_element(css)` | CDP click at element center. |
| `clear_browser_cache()` | Clears cache. |
| `clear_browser_cookies()` | Clears cookies. |
| `get_cookies()` | Returns cookies as JSON. |
| `set_network_conditions(conditions)` | Network throttling. |
| `set_timezone(id)` | Sets timezone. |
| `set_geolocation(lat, lon, acc)` | Sets geolocation. |
| `uc_click(css)` | Stealth click with delay. |
| `uc_type(css, text)` | Stealth type with delay. |
| `human_click(css)` | Human-like click. |
| `human_type(css, text)` | Human-like type. |

## Tours, presentations, charts

| Method | Description |
|--------|-------------|
| `create_tour(theme)` | Creates a Shepherd tour. |
| `add_tour_step(css, title, content)` | Adds a tour step. |
| `start_tour()` | Starts the tour. |
| `export_tour(path)` | Exports tour JSON. |
| `create_presentation(name)` | Creates HTML presentation. |
| `add_slide(html)` | Adds a slide. |
| `export_presentation(path)` | Exports presentation. |
| `create_pie_chart(name, data)` | Creates pie chart. |
| `export_pie_chart(path)` | Exports chart. |

## Recorder

| Method | Description |
|--------|-------------|
| `start_recording()` | Starts recording actions. |
| `stop_recording()` | Returns generated Rust code. |
| `recorded_actions()` | Returns recorded actions. |

For full details, build the rustdocs with `cargo doc --no-deps --open`.
