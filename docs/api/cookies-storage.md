---
title: "Cookies and storage"
sidebarTitle: "Cookies and storage"
description: "Read and write cookies, local storage and session storage."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Cookies and storage

| Method | Description |
|--------|-------------|
| `save_cookies(name)` | Saves the cookies, as JSON, to `saved_cookies/<name>.txt` in the working directory (the last path part of `name`, `.txt` added) and returns the path. |
| `load_cookies(name)` | Adds the cookies in `saved_cookies/<name>.txt`; a cookie of another domain gets the page's and the expiry is dropped. `load_cookies_with(name, CookieExpiry)` says what to do with the expiry. |
| `get_saved_cookies(name)` | Reads `saved_cookies/<name>.txt` without adding the cookies. |
| `delete_saved_cookies(name)` | Deletes `saved_cookies/<name>.txt` (not the cookies of the browser). |
| `save_cookies_to_path(path)` / `load_cookies_from_path(path, expiry)` | The same for a file at any path. |
| `add_cookie(&cookie_dict)` | Adds a browser cookie, given as the W3C cookie object (a `seleniumbase_rs::json::Value` with a string `name` and `value`, and optionally `path`, `domain`, `secure`, `httpOnly`, `expiry` and `sameSite`) that `get_cookies()` returns; anything else fails with `InvalidConfig`. By default the cookie's own `expiry` is dropped, so it lasts for the browser session, and a `domain` outside the page's origin becomes the origin's host. Returns a call builder with `.expiry(impl Into<CookieExpiry>)`: `true` expires the cookie a day from now, as Python's `expiry=True`, or pass a `CookieExpiry`. |
| `get_cookie(name)` | Returns a cookie by name. |
| `delete_all_cookies()` | Deletes all cookies. |
| `set_local_storage_item(key, value)` | Sets localStorage item. |
| `get_local_storage_item(key)` | Gets localStorage item. |
| `remove_local_storage_item(key)` | Removes localStorage item. |
| `clear_local_storage()` | Clears localStorage. |
| `set_session_storage_item(key, value)` | Sets sessionStorage item. |
| `get_session_storage_item(key)` | Gets sessionStorage item. |
| `clear_session_storage()` | Clears sessionStorage. |
