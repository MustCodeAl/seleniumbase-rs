---
title: "Interaction"
sidebarTitle: "Interaction"
description: "Click, type, select, hover, drag, press keys and scroll."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

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
[Selectors guide](/tutorials/selectors).

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
