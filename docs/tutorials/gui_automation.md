# GUI Automation Guide

Some scenarios require controlling the operating system's mouse and keyboard
instead of the browser. SeleniumBase for Rust exposes GUI automation helpers via
`enigo`. Use these helpers only when WebDriver cannot reach the target, because
they move the real cursor and depend on screen state.

## What you will learn

- Which GUI helpers are available.
- How to perform keyboard shortcuts and text entry.
- Common use cases and limitations.

## Available helpers

| Method | Description |
|---|---|
| `gui_click_x_y(x, y)` | Click an absolute screen coordinate. |
| `gui_write(text)` | Type a string of characters. |
| `gui_key_sequence(keys)` | Press and release a sequence of keys. |
| `gui_press_keys(keys)` | Hold modifier keys. |
| `gui_release_keys(keys)` | Release modifier keys. |

```rust
sb.gui_click_x_y(100, 200)?;
sb.gui_write("hello world")?;
sb.gui_key_sequence(&["command", "a", "delete"])?;
sb.gui_press_keys(&["command", "c"])?;
sb.gui_release_keys(&["command"])?;
```

## Coordinate systems

Screen coordinates are absolute pixels. They depend on screen resolution and
window position, so GUI scripts are usually machine-specific.

## Typical use cases

- Interacting with native file dialogs triggered by `<input type="file">`.
- Dismissing OS-level notifications.
- Automating multi-window workflows where the browser is not in focus.

## Example

```rust
// Focus the browser address bar with Ctrl+L and type a URL
sb.gui_press_keys(&["control", "l"])?;
sb.gui_release_keys(&["control"])?;
sb.gui_write("https://seleniumbase.io")?;
sb.gui_key_sequence(&["return"])?;
```

## Limitations

- GUI automation moves the real cursor and keyboard focus.
- Coordinates are platform-dependent.
- It is slower and less reliable than WebDriver interactions; prefer WebDriver when possible.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Click misses target | Wrong screen coordinates | Verify resolution and window position. |
| Key sequence not applied | Modifier still held | Call `gui_release_keys` after the shortcut. |
| Works locally but fails in CI | No display server | Use a virtual framebuffer such as Xvfb or prefer WebDriver. |
