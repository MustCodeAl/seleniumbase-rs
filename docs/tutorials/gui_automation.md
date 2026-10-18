---
title: "GUI Automation Guide"
sidebarTitle: "GUI automation"
---

Some scenarios require controlling the operating system's mouse and keyboard
instead of the browser. SeleniumBase for Rust exposes GUI automation helpers via
`enigo`. Use these helpers only when WebDriver cannot reach the target, because
they move the real cursor and depend on screen state.

<Badge color="purple">Feature `gui`</Badge>

<Info>

**What you will learn**

- Which GUI helpers are available.
- How to perform keyboard shortcuts and text entry.
- Common use cases and limitations.

</Info>

<Visibility for="agents">

Summary for AI exports: the GUI helpers need the `gui` Cargo feature (off by default). `BaseCase::gui_click_x_y`, `gui_write`, `gui_press_key`, `gui_press_keys` and the element helpers such as `gui_click_element` drive the operating system's real mouse and keyboard through `enigo`. Use them only when WebDriver cannot reach the target.

</Visibility>

## Enable the `gui` feature

The helpers exist only when the `gui` Cargo feature is on. It is off by default
because it pulls in `enigo` (and `rfd` for native dialogs), which link GTK and
X11 on Linux:

<CodeGroup>

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["gui"] }
```

```bash Terminal
cargo run --example gui_automation --features gui
```

</CodeGroup>

The example serves a one-field page from `127.0.0.1`, opens it in a visible
browser window (headless Chrome has no window for native keys to land in), gives
the field focus, types into it with the operating system's keyboard, and checks
what arrived.

<Warning>

It takes over your keyboard for a moment, so leave the browser window in front
and do not type while it runs. On macOS your terminal needs the Accessibility
permission (System Settings > Privacy & Security > Accessibility).

</Warning>

Without the feature, `BaseCase::gui_*`, `seleniumbase_rs::Gui` and the native
dialog helpers (`show_message`, `show_confirm`, `show_prompt`,
`choose_file_dialog`, `save_file_dialog`) are not compiled.

## Available helpers

These `BaseCase` methods are synchronous (no `.await`) unless the table says
otherwise.

| Method | Description |
|---|---|
| `gui_click_x_y(x, y)` | Move to an absolute screen coordinate and click. |
| `gui_hover_x_y(x, y)` | Move to an absolute screen coordinate. |
| `gui_write(text)` | Type a string of characters. |
| `gui_press_key(key)` | Press and release one key by name. |
| `gui_press_keys(keys)` | Press a chord: modifier keys are held while the other keys are clicked, then released. |
| `gui_drag_and_drop_points(from_x, from_y, to_x, to_y)` | Drag between two screen points. |
| `gui_click_and_hold(x, y)` / `gui_release()` | Press the left button at a point, and release it later. |
| `gui_click_element(css)` (async) | Click the center of an element, in screen coordinates. |
| `gui_click_with_offset(css, x, y)` (async) | Click the element's center plus an offset. |
| `gui_move_to_element(css)`, `gui_hover_element(css)` (async) | Move the cursor to an element's center. |
| `gui_hover_and_click(css)` (async) | Move to an element's center and click. |
| `gui_drag_and_drop(source_css, target_css)` (async) | Drag one element's center onto another's. |
| `uc_gui_click_x_y`, `uc_gui_write`, `uc_gui_press_key`, `uc_gui_handle_captcha(css)` | Aliases under the Python `uc_gui_*` names; the last clicks an element with native input (it does not solve CAPTCHAs). |

Key names are case-insensitive: `enter` or `return`, `tab`, `esc` or `escape`,
`backspace`, `delete` or `del`, `space`, `ctrl` or `control`, `alt`, `shift`,
`meta` / `command` / `cmd` / `win` / `windows` / `super` (all one key), `home`,
`end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right` (or with an `arrow`
suffix), `capslock`, `f1` to `f12`, or any single character. An unknown name
returns a `SeleniumBaseError::Gui` error.

```rust,no_run
# #[cfg(feature = "gui")]
# fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.gui_click_x_y(100, 200)?;
sb.gui_write("hello world")?;
sb.gui_press_keys(&["command", "a"])?; // select all; use "ctrl" instead of "command" off macOS
sb.gui_press_key("delete")?;
# Ok(())
# }
```

`seleniumbase_rs::Gui` is the same wrapper without a browser: `Gui::new()?`
gives `move_mouse`, `click`, `right_click`, `double_click`, `scroll`,
`press_key`, `press_keys`, `write`, `drag`, `mouse_down`, `mouse_up` and
`screen_size`.

## Coordinate systems

Screen coordinates are absolute pixels. They depend on screen resolution and
window position, so GUI scripts are usually machine-specific. The element
helpers compute an element's center from the page (`getBoundingClientRect` plus
the window's screen position), which ignores the browser's toolbar height, so
they can land a few dozen pixels off in a window with a toolbar. They find the
element with `document.querySelector`, so they take a CSS selector only.

## Typical use cases

- Interacting with native file dialogs triggered by `<input type="file">`.
- Dismissing OS-level notifications.
- Automating multi-window workflows where the browser is not in focus.

## Example

```rust,no_run
# #[cfg(feature = "gui")]
# fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
// Focus the browser address bar with Ctrl+L and type a URL
sb.gui_press_keys(&["control", "l"])?;
sb.gui_write("https://seleniumbase.io")?;
sb.gui_press_key("return")?;
# Ok(())
# }
```

## Limitations

<Warning>

- GUI automation moves the real cursor and keyboard focus.
- Coordinates are platform-dependent.
- It is slower and less reliable than WebDriver interactions; prefer WebDriver when possible.

</Warning>

## Troubleshooting

<AccordionGroup>

<Accordion title="gui_* methods not found" icon="magnifying-glass">

**Likely cause:** The `gui` feature is off.

**Fix:** Build with `--features gui`.

</Accordion>

<Accordion title="Click misses target" icon="crosshairs">

**Likely cause:** Wrong screen coordinates.

**Fix:** Verify resolution and window position; the element helpers ignore the toolbar height.

</Accordion>

<Accordion title="Shortcut does nothing" icon="keyboard">

**Likely cause:** Wrong modifier for the OS.

**Fix:** `command` and `meta` are the Cmd key on macOS and the Windows key on Windows; use `ctrl` for Ctrl shortcuts.

</Accordion>

<Accordion title="unknown key error" icon="key">

**Likely cause:** A key name that is not in the list above.

**Fix:** Use one of the listed names or a single character.

</Accordion>

<Accordion title="Works locally but fails in CI" icon="display">

**Likely cause:** No display server.

**Fix:** Use a virtual framebuffer such as Xvfb or prefer WebDriver.

</Accordion>

</AccordionGroup>
