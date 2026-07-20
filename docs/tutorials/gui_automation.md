# GUI Automation Guide

Some scenarios require controlling the operating system's mouse and keyboard instead of the browser. SeleniumBase for Rust exposes GUI automation helpers via `enigo`.

## Available helpers

```rust
sb.gui_click_x_y(100, 200)?;
sb.gui_write("hello world")?;
sb.gui_key_sequence(&["command", "a", "delete"])?;
sb.gui_press_keys(&["command", "c"])?;
sb.gui_release_keys(&["command"])?;
```

## Coordinate systems

Screen coordinates are absolute pixels. Use with caution because they depend on screen resolution and window position.

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
