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
| `save_cookies(name)` | Saves the cookies, as JSON, to `saved_cookies/<name>.txt` in the working directory (the last path part of `name`, `.txt` added) and returns the path. The file holds the W3C cookie objects that Python SeleniumBase writes and reads, so the two can share it. |
| `load_cookies(name)` | Adds the cookies in `saved_cookies/<name>.txt`; a cookie of another domain gets the page's and the expiry is dropped. `load_cookies_with(name, CookieExpiry)` says what to do with the expiry. |
| `get_saved_cookies(name)` | Reads `saved_cookies/<name>.txt` without adding the cookies. |
| `delete_saved_cookies(name)` | Deletes `saved_cookies/<name>.txt` (not the cookies of the browser). |
| `save_cookies_to_path(path)` / `load_cookies_from_path(path, expiry)` | The same for a file at any path. |
| `add_cookie(&cookie)` | Adds a browser cookie, given as a `seleniumbase_rs::Cookie`: `Cookie::new(name, value)` and the builders `domain`, `path`, `expires`, `secure`, `http_only` and `same_site`, or a cookie that `get_cookies()` returned. A W3C cookie object, as Python takes it, becomes one with `seleniumbase_rs::json::from_value::<Cookie>(object)` (`expiry` is read as `expires`; a missing `path`, `domain`, `secure` or `httpOnly` takes its default). By default the cookie's own expiry is dropped, so it lasts for the browser session, and a `domain` outside the page's origin becomes the origin's host. Returns a call builder with `.expiry(impl Into<CookieExpiry>)`: `true` expires the cookie a day from now, as Python's `expiry=True`, or pass a `CookieExpiry`. |
| `get_cookie(name)` | Returns the cookie of the current page called `name` as `Option<Cookie>`: `None` when the page has no such cookie, as in Python. Any other failure is an error. |
| `get_cookies()` | Returns the cookies of the current page as `Vec<Cookie>`, the same in every `DriverMode` (the DevTools Protocol in `cdp` and `uc`, WebDriver otherwise). These are the cookies the browser would send to the current URL, not every cookie it holds. A session cookie has no `expires`. |
| `delete_all_cookies()` | Deletes all cookies. |
| `set_local_storage_item(key, value)` | Sets localStorage item. |
| `get_local_storage_item(key)` | Gets localStorage item. |
| `remove_local_storage_item(key)` | Removes localStorage item. |
| `clear_local_storage()` | Clears localStorage. |
| `set_session_storage_item(key, value)` | Sets sessionStorage item. |
| `get_session_storage_item(key)` | Gets sessionStorage item. |
| `clear_session_storage()` | Clears sessionStorage. |
