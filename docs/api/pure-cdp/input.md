---
title: "Pure CDP: Input"
sidebarTitle: "Input"
description: "Trusted mouse and keyboard input, and a human-paced pointer, on a pure CDP page."
---

<Note>

Methods are listed as `name(arguments)`. End an async call with `.await?`. `Mouse` and `Keyboard` come from `page.mouse()` and `page.keyboard()`. Argument names are explained in [Reading the API reference](/api/api_reference#reading-this-page).

</Note>

`Mouse` and `Keyboard` send trusted input events (`isTrusted`). A `Point` is viewport coordinates: `Point::new(x, y)` with `x` and `y` as `f64`. Positions are CSS pixels.

Prefer a [Locator](/api/pure-cdp/locator) (`click`, `fill`, `type_text`, `press`) when the target is an element. Use the pointer and keyboard when you already have a point, or when focus is already in the right field.

## Mouse

| Method | Description |
|--------|-------------|
| `move_to(at)` | Moves the pointer to `at`. |
| `down(at, button)` | Presses `button` at `at`. |
| `up(at, button)` | Releases `button` at `at`. |
| `click(at)` | Clicks the left button at `at`. |
| `double_click(at)` | Double-clicks at `at`. |
| `right_click(at)` | Right-clicks at `at`. |
| `drag(from, to)` | Drags from one point to the other. |
| `wheel(at, dx, dy)` | Scrolls the wheel at `at` by `dx` and `dy` (`f64`). |

`Button` is `Left`, `Right` or `Middle`.

## Keyboard

`press` presses and releases one key. `type_text` types one character at a time, so the page sees a key press for each; a newline presses Enter and a tab presses Tab. `type_char` is one character, with the same newline and tab rule. For uppercase ASCII characters, `type_char` and `type_text` emulate real human Shift typing (SeleniumBase super stealth typing parity): they dispatch `ShiftLeft` (`rawKeyDown`, virtual keycode 16, modifier 8) before the character, emit the character key event with `modifiers: 8` and `unmodifiedText` set to lowercase, and dispatch `ShiftLeft` (`keyUp`) immediately after.

`insert_text` inserts the whole string in one step, as a paste would, and produces no per-character key events. `Locator::fill` uses `insert_text`; `Locator::type_text` and `Locator::send_keys` use `type_text`.

| Method | Description |
|--------|-------------|
| `press(key)` | Presses and releases `key`. |
| `type_text(text)` | Types `text` one character at a time, emulating Shift keydown/keyup for uppercase characters. |
| `type_char(ch)` | Presses and releases the key that produces `ch`, with Shift modifiers for uppercase. |
| `insert_text(text)` | Inserts `text` in one step, with no per-key events. |

`Key` is `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp` or `PageDown`.

```rust
use seleniumbase_rs::sb_cdp::{Key, Page};

async fn search(page: &Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    page.locator("#search").focus().await?;
    page.keyboard().type_text("rust").await?;
    page.keyboard().press(Key::Enter).await?;
    Ok(())
}
```

## Human

`page.human(behavior)` returns a `Human` that moves and types at a person's pace. `behavior` is a `seleniumbase_rs::sb_cdp::human::behavior::Behavior`. Cloning a `Human` shares the pointer position, so two tasks do not both start from where the pointer used to be.

| Method | Description |
|--------|-------------|
| `move_to(to, target_width)` | Moves along a human path to `to`. `target_width` is the target size in pixels; a smaller target takes longer. |
| `click(target)` | Clicks a locator with that pace. |
| `click_at(aim, tolerance)` | Clicks near `aim`, within `tolerance` pixels. Returns the `Point` that was hit. |
| `drag(press, release, tolerance)` | Drags from `press` to `release`, each within `tolerance`. |
| `type_text(target, text)` | Types `text` into a locator at a person's pace. |
| `type_focused(text)` | Types `text` into whatever has focus. |
| `scroll_by(dy)` | Scrolls by `dy` pixels (`f64`). |
