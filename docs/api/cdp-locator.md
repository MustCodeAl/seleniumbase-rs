---
title: "Pure CDP: Locator"
sidebarTitle: "Locator"
description: "Find an element, act on it, and retry an assertion until it holds."
---

<Note>

Methods are listed as `name(arguments)`, called on a `Locator` unless the heading names an expectation. End an async call with `.await?`. Selectors are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

`page.locator(selector)` does not query the page yet. Every later call looks again, so a locator survives a re-render. A plain locator acts on the **first** match. It waits up to its timeout (the page's, **7 seconds** unless `with_timeout` changed it) for the element to be ready.

A locator searches into frames. It stops at the first frame that holds a match. `strict()` counts matches in every frame, including frames of another origin.

## Narrowing

These return another `Locator`. They do not query yet.

| Method | Description |
|--------|-------------|
| `page()` | The `Page` this locator belongs to. |
| `timeout()` | How long this locator waits. |
| `with_timeout(timeout)` | The same locator with a different wait. |
| `strict()` | Fail with `StrictModeViolation` (how many matched) as soon as more than one element matches. `count`, `infos`, `all` and `all_waiting` stay allowed, and so do `first`, `last` and `nth`. |
| `locator(selector)` | A locator for matches inside this one. |
| `nth(index)` | The match at `index` (zero-based). |
| `first()` | The first match. |
| `last()` | The last match. |
| `visible()` | Only a match that is visible. |
| `parent()` | The parent of the match. |

## Reading

| Method | Description |
|--------|-------------|
| `count()` | How many match. Does not fail strict mode. |
| `all()` | A `Locator` for each current match. |
| `all_waiting()` | The same, after waiting for at least one. |
| `wait_for(state)` | Waits until the match reaches a `State`: `Present`, `Visible`, `Hidden` or `Absent`. |
| `info()` | An `ElementInfo` snapshot: `tag`, `text` (`innerText`), `html`, `attributes`, `rect`, `visible`. |
| `infos()` | One `ElementInfo` per match. |
| `text()` | Rendered text of the first match. |
| `html()` | Outer HTML of the first match. |
| `attribute(name)` | The attribute, or `None` when it is absent. |
| `bounding_box()` | A `Rect` in the document, not the viewport (`x`, `y`, `width`, `height`, CSS pixels). |
| `viewport_rect()` | A `Rect` in the viewport. |
| `screen_rect()` | A `Rect` in screen coordinates, for a desktop tool. Exact for a normal window; approximate when a docked panel shares the toolbar. |
| `urls()` | Absolute `http`, `https`, `ftp` and `file` URLs from `href` and `src` on the match and inside it, in page order, without repeats. |
| `exists()` | Whether any match is in the DOM. |
| `is_visible()` | Whether a match is visible. |
| `is_checked()` | Whether a checkbox or radio is checked. |
| `is_selected()` | Whether an option is selected. |
| `is_enabled()` | Whether the control is enabled. |
| `screenshot()` | PNG bytes of the element. |

## Acting

Actions wait until the element can take the action, then run. `click_if_visible` is the exception that returns `false` instead of failing when the element never becomes visible.

| Method | Description |
|--------|-------------|
| `click()` | Clicks the first match. |
| `double_click()` | Double-clicks it. |
| `right_click()` | Right-clicks it. |
| `hover()` | Moves the pointer onto it. |
| `click_and_hold(hold)` | Presses and holds for a `Duration`. |
| `click_with_offset(x, y, center)` | Clicks at an offset. `center` measures from the centre when true, otherwise from the top left. |
| `click_if_visible()` | Clicks if the element becomes visible. Returns `bool`. |
| `click_if_visible_with_timeout(timeout)` | The same, with this wait. |
| `drag_to(target)` | Drags this match onto another locator. |
| `focus()` | Gives the match keyboard focus. |
| `clear()` | Empties the field. |
| `fill(text)` | Clears the field and inserts `text` in one step. The page gets `input` events, not one event per key. |
| `type_text(text)` | Clears the field, then types `text` one key at a time. |
| `send_keys(text)` | Types `text` one key at a time after what is already there. |
| `set_value(value)` | Sets the value and fires `input` and `change`. For controls that ignore keys, such as a range slider. |
| `press(key)` | Focuses the match and presses a `Key`. See [Input](/api/cdp-input). |
| `select_option(by)` | Chooses an option. `SelectBy::Text`, `SelectBy::Value` or `SelectBy::Index`. |
| `check()` | Checks a checkbox or radio. |
| `uncheck()` | Clears a checkbox. |
| `set_checked(checked)` | Sets the checked state. |
| `submit()` | Submits the form. |
| `scroll_into_view()` | Scrolls the match into view. |
| `set_attribute(name, value)` | Sets an attribute. |
| `remove()` | Removes the element from the DOM. |
| `flash()` | Flashes the element so a person can see it. |
| `flash_with(color, duration)` | Flashes it in `color` for a `Duration`. |
| `expect()` | A `LocatorExpect` for this locator. |

## LocatorExpect

`locator.expect()` retries until the check holds or the timeout runs out. `not()` flips the check. `timeout(duration)` sets the wait. Both return the expectation.

| Method | Description |
|--------|-------------|
| `to_be_visible()` | A match is visible. |
| `to_be_hidden()` | No match is visible. |
| `to_exist()` | A match is in the DOM. |
| `to_have_text(expected)` | The text is exactly `expected`. |
| `to_contain_text(fragment)` | The text contains `fragment`. |
| `to_have_attribute(name, value)` | The attribute equals `value`. |
| `to_be_checked()` | The control is checked. |
| `to_have_count(expected)` | Exactly `expected` elements match. |

```rust
use seleniumbase_rs::sb_cdp::Page;

async fn signed_in(page: &Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    page.locator("h1").expect().to_contain_text("Welcome").await?;
    page.locator("#checkout").strict().expect().to_have_count(1).await?;
    Ok(())
}
```

## PageExpect

`page.expect()` retries a check of the tab. `not()` and `timeout(duration)` work as they do on a locator.

| Method | Description |
|--------|-------------|
| `to_have_title(title)` | The title is exactly `title`. |
| `to_contain_title(fragment)` | The title contains `fragment`. |
| `to_have_url(url)` | The URL is exactly `url`. |
| `to_contain_url(fragment)` | The URL contains `fragment`. |
