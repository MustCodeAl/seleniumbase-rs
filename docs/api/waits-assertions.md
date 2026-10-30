---
title: "Waits and assertions"
sidebarTitle: "Waits and assertions"
description: "Wait for elements and text, assert on the page, and collect deferred assertions."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Waits

| Method | Description |
|--------|-------------|
| `wait_for_element(selector)` | Waits for element visible (Python's alias of `wait_for_element_visible`), for the default timeout; `.timeout(Duration)` sets another. |

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

See [Waits and Assertions](/tutorials/waits_assertions) for detailed examples.

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
| `assert_in(item, &container)` | Asserts `item` is in `container`, as Python's `first in second`: an element of a slice, array or `Vec` (`assert_in(2, &[1, 2, 3])`), or a substring or character of a string (`assert_in("ell", "hello")`). |
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
