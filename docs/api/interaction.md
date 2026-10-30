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
| `click_with_offset(selector, x, y)` | Clicks at the pixel offset `(x, y)` from the element's top-left corner; `x` and `y` are any numbers. Returns a call builder with `.mark(bool)` (draw a dot at the click on a canvas), `.timeout(Duration)`, `.center(bool)` (count the offset from the element's centre instead) and `.scroll(bool)` (scroll to the element first, on by default). |
| `double_click_with_offset(selector, x, y)` | The same with a double click, and the same call builder. |
| `click_nth_visible_element(selector, number)` | Clicks the `number`th visible match, counting from 1; a `number` of 0 fails with `InvalidConfig`. Returns a call builder with `.timeout(Duration)` and `.scroll(bool)` (on by default). |
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
| `hover_and_double_click(hover_selector, click_selector)` | Hovers over the first element, then double-clicks the second, which usually appears on the hover. Returns a call builder with `.timeout(Duration)`, how long to wait for each element. |
| `hover_and_js_click(hover_selector, click_selector)` | The same, but the second element is clicked by a script. Same call builder. |
| `drag_and_drop(src, dst)` | Drags one element onto another. |
| `drag_and_drop_with_offset(selector, x, y)` | Drags the element itself `x` pixels right and `y` pixels down (`x` and `y` are `i64`). Returns a call builder with `.timeout(Duration)`. |
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

A method that returns a call builder takes Python's keyword arguments as
methods of the builder and runs when it is awaited, so the plain form is still
`sb.click_with_offset("#canvas", 10, 20).await?` and a keyword is added before
the `.await`: `sb.click_with_offset("#canvas", 10, 20).center(true).await?`.
The builders are in `api::misc_calls`.

## Keyboard arrow keys

| Method | Description |
|--------|-------------|
| `press_up_arrow()` | Presses the Up arrow key. Returns a call builder with `.selector(&str)` (the element that gets the key; `body` by default) and `.times(u32)` (how many times in a row; 1 by default, 0 does nothing). |
| `press_down_arrow()` | The same with the Down arrow key. |
| `press_left_arrow()` | The same with the Left arrow key. |
| `press_right_arrow()` | The same with the Right arrow key. |

## Scrolling

| Method | Description |
|--------|-------------|
| `scroll_to(selector)` | Scrolls the element into view. |
| `scroll_into_view(selector)` | Smooth-scrolls the element to the center. |
| `scroll_to_top()` | Scrolls to the top of the page. |
| `scroll_to_bottom()` | Scrolls to the bottom of the page. |
| `scroll_up()` | Scrolls up by a percentage of the window's height, 25 by default. Returns a call builder with `.amount(impl Into<f64>)`, the percentage. |
| `scroll_down()` | Scrolls down by a percentage of the window's height, 25 by default. Same call builder. |
| `smooth_scroll_to(selector)` | Smooth-scrolls the element into view. |
| `scroll_by_y(pixels)` | Scrolls vertically by `pixels`. |
