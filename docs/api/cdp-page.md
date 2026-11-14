---
title: "Pure CDP: Page"
sidebarTitle: "Page"
description: "Navigate a tab, run JavaScript, and read the page. Locators cover the elements."
---

<Note>

Methods are listed as `name(arguments)`, called on a `Page`. End an async call with `.await?`. Almost every method returns `Result`. Selectors and the reading guide are in [Reading the API reference](/tutorials/api_reference#reading-this-page). Elements are a [Locator](/api/cdp-locator), not methods on the page.

</Note>

A `Page` is one tab: a cheap handle you clone. Get one from [Browser](/api/cdp-browser). It waits **7 seconds** unless you call `with_timeout(timeout)`, which returns a handle to the same tab. Locators created afterwards inherit that timeout.

There is no public method that switches into a frame, and none that accepts or dismisses a JavaScript dialog (`alert`, `confirm`, `prompt`). A locator searches into frames on its own, including frames of another origin (a plain locator stops at the first frame that holds a match; a strict one counts every frame). A raw DevTools command goes through `execute`.

## The handle

| Method | Description |
|--------|-------------|
| `id()` | DevTools target id of the tab. |
| `browser()` | The `Browser` that owns it. |
| `timeout()` | How long this handle waits, as a `Duration`. |
| `with_timeout(timeout)` | A handle to the same tab with a different wait. |
| `locator(selector)` | A lazy `Locator`. Nothing is looked up until it is used. |
| `expect()` | A `PageExpect` for the title and URL. See [Locator](/api/cdp-locator). |
| `mouse()` | The tab's `Mouse`. See [Input](/api/cdp-input). |
| `keyboard()` | The tab's `Keyboard`. |
| `human(behavior)` | A `Human` pointer that follows `behavior`. |
| `bring_to_front()` | Brings the tab to the foreground. |
| `close()` | Closes the tab and waits until the browser no longer lists it. |
| `events()` | An `Events` handle for this tab's DevTools events. |
| `execute(method, params)` | Sends one DevTools command. `params` and the result are JSON `Value`s. |

## Navigation

`goto` navigates and waits for the load. A URL with no scheme is opened over HTTPS. A navigation that the browser reports as aborted because a download started is not an error. A same-document change (a `#fragment`) does not wait for a load event. With the `plugins` feature, each plugin's `before_navigate` runs first and may cancel the navigation or send it elsewhere.

| Method | Description |
|--------|-------------|
| `goto(url)` | Opens `url` and waits for the load. |
| `goto_if_not_url(url)` | `goto`, unless the tab is already on that address. `https://example.com` and `https://example.com/` count as the same page. |
| `reload()` | Reloads and waits for the load. |
| `hard_reload()` | Reloads, bypassing the cache, and waits for the load. |
| `back()` | Goes back one step. Does nothing at the start of the history. |
| `forward()` | Goes forward one step. Does nothing at the end of the history. |
| `wait_for_load()` | Waits until the page has loaded. |
| `wait_for_function(expression)` | Waits until the JavaScript expression is truthy, within the page timeout. |
| `wait_for_any(locators, state)` | Waits until one locator reaches a `State`. Returns that locator's index (`usize`). |

`State` is `Present` (in the DOM), `Visible` (rendered with a non-zero size), `Hidden` (nothing visible; hidden matches may still exist) or `Absent` (gone from the DOM).

## What the tab shows

| Method | Description |
|--------|-------------|
| `url()` | Current URL (`window.location.href`). |
| `title()` | Document title. |
| `content()` | Current HTML, including changes scripts have made (`document.documentElement.outerHTML`). |
| `origin()` | Document origin, such as `https://example.com`. |
| `user_agent()` | `navigator.userAgent`. |
| `locale_code()` | `navigator.language`, or the first of `navigator.languages` when that is empty. |
| `screen_rect()` | The monitor (`window.screen`) as a `ScreenRect`: width, height, available size and origin, colour depth and pixel depth. Not the browser window. |
| `info()` | `PageInfo`: target id, URL and title. |
| `is_online()` | Whether the browser reports network access (`navigator.onLine`). |
| `active_element_css()` | A short CSS selector for the focused element, or `body` when nothing is focused. |
| `active_element()` | A `Locator` for the element that was focused when this was called. It does not follow focus afterwards. |
| `click_active_element()` | Calls `click()` on `document.activeElement`. The page sees a click, not a mouse move. |
| `js_errors()` | Uncaught script errors and unhandled rejections in the current document, oldest first. Cleared on navigation. A failed image load is not included. |
| `assert_no_js_errors()` | Fails with `AssertionFailed` if `js_errors` is not empty. |
| `scroll(to)` | Scrolls the page. `to` is a `Scroll`: `Top`, `Bottom`, `To(pixels)`, `By(pixels)`, `PageDown(fraction)` or `PageUp(fraction)`. |
| `screenshot()` | Viewport PNG bytes (`Vec<u8>`). |
| `pdf()` | The page printed to PDF bytes, with backgrounds. |

## Scripts and styles

`evaluate` runs JavaScript in the page and returns a JSON `Value`. `evaluate_as` deserializes that value into `T`. A Content Security Policy does not block `evaluate`. It can block a tag.

| Method | Description |
|--------|-------------|
| `evaluate(expression)` | Runs JavaScript. Returns a `Value`. |
| `evaluate_as(expression)` | Same, deserialized into the caller's type. |
| `add_script_tag(source)` | Adds a `<script>` to the current document and waits for it. `TagSource::url(url)` loads a file, up to the page timeout. `TagSource::content(code)` runs inline. |
| `add_style_tag(source)` | Adds a stylesheet the same way: a `<link>` for a URL, a `<style>` for content. |
| `add_init_script(source)` | Runs `source` in every document this tab loads from now on, before the page's own scripts, including frames. Returns an `InitScriptId`. The document already open is left as it is. |
| `remove_init_script(id)` | Stops that init script for documents loaded from now on. |

## Captcha

`solve_captcha` looks for the first supported widget (Cloudflare Turnstile, reCAPTCHA, hCaptcha, Friendly Captcha, DataDome), in the page, its open shadow roots and the frames it can read. A checkbox that is not part of a challenge is not touched. The press uses trusted pointer events. It returns `Some(Captcha)` for the widget it tried, or `None` when the page shows none. It returns `Captcha` (the error) when the widget was pressed the allowed number of times and the page never showed that it cleared.

`solve_captcha_with(options)` is the same and returns `Option<Solved>`: the widget, how many presses, the `Evidence` that it cleared (`Token`, `Marker`, `Cookie`, `Navigated`, `WidgetState`, `WidgetGone`) or `None` when that cannot be seen, and the elapsed `Duration`.

`SolveOptions::new()` is the default (up to three presses in one round). `locate_timeout`, `interstitial_timeout`, `settle`, `verify_timeout`, `attempts`, `backoff`, `retry_policy`, `rounds`, `cooldown`, `rotate_proxy_after`, `rotate_fingerprint_after`, `clearance_cookie`, `success_marker` and `behavior` each return a new options value. `attempts_allowed` and `rounds_allowed` are at least one; `cooldown_between_rounds` is the wait between rounds; `verify_timeout_allowed` is the time allowed for the page to show a result after a press, as `verify_timeout` set it (the layer that waits for a result itself, such as the stealth scraper waiting for the clearance cookie, reads it here); `proxy_rotation_due(failed_rounds)` and `fingerprint_rotation_due(failed_rounds)` say whether a rotation is due after that many rounds in a row without a result. The solver applies `rounds`, `cooldown` and `clearance_cookie` itself; a `Page` has no proxy list and cannot change its fingerprint, so `rotate_proxy_after` and `rotate_fingerprint_after` only carry the setting for the layer that owns them. `Solved::rounds` is how many rounds the call made.

A challenge page that shows no widget (an interstitial such as "Just a moment...") is waited out for `interstitial_timeout` (20 s by default) before the page that is left is looked at for a widget; a reading that fails while the page is being replaced is read again, and any other failed reading is an error at once. A page that is still a challenge page when the time is up (at most `locate_timeout` and `interstitial_timeout` together) fails with `SeleniumBaseError::WaitTimeout` rather than answering `None`.

## Messenger, fingerprint, WebRTC, breakpoint

| Method | Description |
|--------|-------------|
| `activate_messenger()` | Loads the in-page messenger. A navigation drops it. The methods below load it themselves when it is missing. |
| `set_messenger_theme(settings)` | How later messages look and where they sit. `settings` converts into `MessengerSettings` (`MessengerTheme` or `MessengerLocation` change one part). |
| `post_message(message, options)` | Shows `message` as text, not HTML. `options` is `MessageOptions`. When pause is set, the call waits out the message duration. |
| `apply_fingerprint(fingerprint)` | Applies a `Fingerprint` to this tab. What a plan contains is on [State](/api/cdp-state). |
| `capture_fingerprint()` | Reads what this tab shows a page, as a `CapturedFingerprint`. One script evaluation; it changes nothing and asks for no permission. Prefer an `https:` page or `about:blank` for a full reading. |
| `compare_fingerprint(expected)` | Captures, then compares with `expected`. Returns a `FingerprintComparison`. |
| `shield_webrtc()` | Relay-only WebRTC in this tab, now and on later pages. No host, `.local` or public candidate, and no STUN server. |
| `webrtc_report()` | Gathers local ICE candidates (no STUN server) into a `WebRtcReport`. The page must be allowed to use WebRTC. |
| `breakpoint(message)` | Pauses for a person at the terminal: `c` continues, `s` saves a screenshot, `src` saves the page source, `q` stops the test. Skipped when stdin is not a terminal, and when `SB_BREAKPOINTS` is `0`. |

Cookies, storage, the window and emulation are `page.cookies()`, `page.local_storage()`, `page.session_storage()`, `page.window()` and `page.emulation()`, documented on [State](/api/cdp-state). Network recording and downloads are on [Network](/api/cdp-network).
