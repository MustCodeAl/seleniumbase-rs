---
title: "Windows, frames and drivers"
sidebarTitle: "Windows and frames"
description: "Switch windows, tabs and frames, and drive several browsers."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

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
