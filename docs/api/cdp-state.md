---
title: "Pure CDP: State"
sidebarTitle: "State"
description: "Cookies, storage, the window, emulation and a fingerprint plan on a pure CDP page."
---

<Note>

Methods are listed as `name(arguments)`. End an async call with `.await?`. Get the handles from a `Page`: `cookies()`, `local_storage()`, `session_storage()`, `window()` and `emulation()`. Argument names are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

Browser-wide permissions are on [Browser](/api/cdp-browser). This page is the state of one tab.

## Cookies

`page.cookies()` is the cookie jar.

Build a cookie with `Cookie::new(name, value)`, then `domain`, `path`, `expires` (seconds, `f64`), `http_only`, `secure` and `same_site`. Those methods return the cookie. The fields are public: `name`, `value`, `domain`, `path`, `expires` (`Option<f64>`), `http_only`, `secure`, `same_site` (`Option<SameSite>`). `SameSite` is `Strict`, `Lax` or `None`. `Cookie` and `SameSite` are the types of the whole crate: `seleniumbase_rs::{Cookie, SameSite}` is the canonical path, `sb_cdp::{Cookie, SameSite}` the same pair, and the WebDriver engine's `get_cookie`, `get_cookies` and `add_cookie` use them too.

| Method | Description |
|--------|-------------|
| `all()` | Every cookie, as `Vec<Cookie>`. |
| `set(cookies)` | Sets the cookies in the slice. |
| `clear()` | Deletes them. |
| `header()` | The cookies as one `Cookie` header value. |
| `save(path, filter)` | Writes cookies to `path`. `filter` is an optional substring. Returns how many were written. |
| `load(path, filter)` | Reads cookies from `path` and sets them. `filter` is an optional substring. Returns how many were set. |

## Storage

`page.local_storage()` is `localStorage` for the page's origin. `page.session_storage()` is `sessionStorage`. Both are a `Storage`. `StorageArea` is `Local` or `Session`.

| Method | Description |
|--------|-------------|
| `get(key)` | The value, or `None` when the key is absent. |
| `set(key, value)` | Sets one entry. |
| `remove(key)` | Removes one key. |
| `clear()` | Removes every entry. |
| `entries()` | Every entry, as a `BTreeMap<String, String>`. |

## Window

`page.window()` is the window that holds the tab, not the monitor. The monitor is `page.screen_rect()` on [Page](/api/cdp-page).

`WindowBounds` has public fields `x`, `y`, `width`, `height` (`i64`) and `state`. `WindowState` is `Normal`, `Minimized`, `Maximized` or `Fullscreen`.

| Method | Description |
|--------|-------------|
| `bounds()` | The current `WindowBounds`. |
| `set_bounds(x, y, width, height)` | Moves and resizes the window. |
| `maximize()` | Maximizes it. |
| `minimize()` | Minimizes it. |
| `fullscreen()` | Full screen. |
| `restore()` | Restores a normal window. |

## Emulation

`page.emulation()` overrides what the tab reports to pages.

| Method | Description |
|--------|-------------|
| `timezone(id)` | Time zone id, such as `"America/New_York"`. |
| `locale(locale)` | Locale, such as `"en-US"`. |
| `geolocation(latitude, longitude, accuracy)` | Geolocation, each an `f64`. |
| `user_agent(user_agent, platform)` | User-agent string. `platform` is `Option<&str>`. |
| `offline(offline)` | `true` simulates a lost network. `false` restores it. The same as `network_throttle` with a throttle that only sets `offline`. |
| `network_throttle(&throttle)` | Slows the tab's network down, cuts it off, or restores it, as the `NetworkThrottle` says. It sends `Network.enable`, then `Network.emulateNetworkConditions`. The throttle belongs to this tab. |
| `device_metrics(metrics, mobile)` | A screen of `metrics` CSS pixels. With `mobile`, the page is laid out as on a phone. Touch and the phone user agent are separate: `touch` and `user_agent`, or `LaunchOptionsBuilder::mobile` at launch. |
| `clear_device_metrics()` | Drops that screen. The tab follows its window again. |
| `touch(enabled)` | Touch emulation. On means five touch points, so `navigator.maxTouchPoints` and touch events follow a phone. |
| `color_scheme(scheme)` | What `prefers-color-scheme` reports. |

`ColorScheme` is `Light`, `Dark` or `NoPreference`. `NoPreference` removes the override so the browser's own setting decides. This is not `LaunchOptionsBuilder::dark_mode`, which forces Chrome's dark theme on web pages.

`DeviceMetrics` is `seleniumbase_rs::browser::options::DeviceMetrics`. `DeviceMetrics::new(width, height, pixel_ratio)` is a screen of `width` by `height` CSS pixels (`u32`) at `pixel_ratio` device pixels per CSS pixel (`f64`).

`NetworkThrottle` is `seleniumbase_rs::NetworkThrottle`, the same type `BaseCase::set_network_throttle` takes. Its fields are `offline` (`bool`, every request fails at once), `latency` (a `Duration` added before each response starts to arrive, in whole milliseconds), and `download` and `upload` (`Option<u32>`: the most bytes per second the page may receive and send, `None` for no limit). `NetworkThrottle::default()` changes nothing, so it puts the connection back:

```rust
use std::time::Duration;

use seleniumbase_rs::NetworkThrottle;

page.emulation()
    .network_throttle(&NetworkThrottle {
        latency: Duration::from_millis(400),
        download: Some(200_000),
        ..NetworkThrottle::default()
    })
    .await?;
// ... load the page, then put the connection back.
page.emulation().network_throttle(&NetworkThrottle::default()).await?;
```

## StealthPlan

`StealthPlan` is the pure CDP form of a `Fingerprint`: the init script and the DevTools commands to send, not the fingerprint type itself. `LaunchOptionsBuilder::fingerprint` builds one and applies it to every tab. `Page::apply_fingerprint` applies one to the tab you have.

| Method | Description |
|--------|-------------|
| `new(fingerprint)` | The plan for `fingerprint`. |
| `init_script()` | The script that makes the page's own scripts read the fingerprint. Run it in every document, including frames, before the page's scripts. |
| `overrides()` | The DevTools overrides `(method, params)`, after the init script. |
| `commands()` | Every command in send order: the init script as `Page.addScriptToEvaluateOnNewDocument`, then the overrides. |
| `permission_grant(context)` | The permission grant for a browser context id, or `None`. `context` is `Option<&str>`. |

`Page::capture_fingerprint` and `Page::compare_fingerprint` read what the tab shows. They are on [Page](/api/cdp-page).
