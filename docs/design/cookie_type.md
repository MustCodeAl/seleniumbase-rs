---
title: "Design: one cookie type for both engines"
sidebarTitle: "One cookie type"
---

Status: proposal (2026-11-05). It is the first lane after the `Element` lane, as
[the element note](/design/element_type) recommends in open question 9: one lane for each
thirtyfour type left in the public API, `Cookie` first, because `get_cookie` hands it
to users.

## The problem

The WebDriver engine gives users thirtyfour's cookie, and the pure CDP engine gives
them ours. The two disagree on almost everything:

| | thirtyfour's `Cookie` (`drivers::Cookie`) | ours (`sb_cdp::Cookie`) |
|---|---|---|
| Where | `src/drivers.rs:34` re-exports `thirtyfour::cookie::Cookie` | `src/sb_cdp/state.rs:33-187` |
| Fields | `path`, `domain`, `secure`, `http_only` are all `Option` | plain `String` and `bool`, with `/`, `""` and `false` as defaults |
| Expiry | `expiry: Option<i64>` | `expires: Option<f64>` (CDP sends fractions; `None` is a session cookie) |
| `SameSite` | `Strict`, `Lax`, `None`, `Default` (BiDi only) | `Strict`, `Lax`, `None` |
| `Debug` | derived, so `{:?}` prints the value, which is often a login | hand-written; the value shows as `<redacted>` |
| Builders | `set_path(&mut self, ..)` and so on | `Cookie::new(..).domain(..).secure(true)` |

The thirtyfour type is checked with Context7 (`/websites/rs_thirtyfour`, thirtyfour 0.38,
the version `Cargo.lock` resolves).

Three behaviours are wrong today, beside the type:

1. **`get_cookie` fails for a missing cookie.** `BrowserSession::get_cookie`
   (`src/browser/session/page.rs:86-95`) calls thirtyfour's `get_named_cookie`, which
   returns the driver's "no such cookie" error. Python's `get_cookie` returns
   `self.driver.get_cookie(name)` (`base_case.py:4912-4915`), and Selenium returns `None`
   when the cookie is missing. `examples/cookies_storage.rs:52` even asserts `.is_err()`.
2. **`get_cookies` returns two JSON shapes behind one signature.**
   `BrowserSession::get_cookies` (`src/browser/session/cdp.rs:173-187`) returns
   `{"cookies": [...]}`. With DevTools connected, the entries are CDP cookie objects:
   `expires` is a float, `-1` for a session cookie, and the objects carry `size`, `session`
   and `priority`. In plain WebDriver mode they are thirtyfour's: `expiry` is an integer
   and is missing for a session cookie. Code that reads `expires` works in one mode only.
   `docs/parity.md:210` lists the method as "Differs: Returns a serde_json::Value".
3. **The doc comments say "all cookies".** `BaseCase::get_cookies`
   (`src/api/base_case.rs:1289-1292`) and `CdpClient::get_cookies`
   (`src/evasion/cdp.rs:172-178`) say so, but `Network.getCookies` without `urls` returns
   the cookies for the current URL (CDP docs on Context7,
   `/websites/chromedevtools_github_io_devtools-protocol_tot`). That is the same scope as
   WebDriver's `get_all_cookies` and as Python's `get_cookies`. Only the comments are
   wrong; the scope is right.

What already works and must keep working:

- `BaseCase::add_cookie(&cookie_dict)` (`src/api/base_case.rs:2108-2129`,
  `AddCookie` in `src/api/misc_calls.rs:666`). It takes the W3C cookie object as a
  `json::Value`, as Python takes a dict, and builds a thirtyfour cookie inside
  (`:2166`). It applies the expiry rule (`CookieExpiry`) and moves a foreign domain to
  the page's host.
- `save_cookies` and `save_cookies_to_path` (`src/api/base_case.rs:579-610`) write the
  driver's cookies as W3C objects, and `load_cookies*` (`:626-680`) read them back; it is
  the format Python SeleniumBase reads and writes. The file format must not change.
- Stealth builds `sb_cdp::Cookie` with a struct literal, and checks that it is
  `Debug + Clone + PartialEq` (`tests/contract_stealth.rs:117-120`, `:236`, `:256`,
  `:451`, `:490-491`). So the fields and the `sb_cdp` path stay as they are.
- No other repository names `drivers::Cookie` (contracts, and a search of stealth,
  plugins, the Tauri plugin and the companion).

## The proposal

1. **One type.** Move `Cookie` and `SameSite` out of `sb_cdp/state.rs` into an
   engine-neutral `src/cookie.rs`. Re-export them at the crate root
   (`seleniumbase_rs::{Cookie, SameSite}`, next to `Element`), and keep
   `sb_cdp::{Cookie, SameSite}` as paths to the same items, because `Cookies::all`
   returns them and stealth imports them from there. The fields, the builders, the serde
   names and the redacted `Debug` stay unchanged. `from_protocol` and `to_protocol`
   become `pub(crate)`.
2. **`get_cookie` returns `Result<Option<Cookie>>`** on `BaseCase` and `BrowserSession`.
   It is `Ok(None)` when the driver says "no such cookie", and every other error stays
   an error. The lane checks thirtyfour's error variant for that case on Context7.
3. **`get_cookies` returns `Result<Vec<Cookie>>`** on `BaseCase` and `BrowserSession`,
   with one shape in both modes. The DevTools path converts each entry with
   `Cookie::from_protocol`, which already turns `expires: -1` into `None`. The plain
   WebDriver path converts each thirtyfour cookie. The scope stays the current page, and
   the doc comments say so.
4. **`add_cookie` takes `&Cookie`** on `BaseCase` (same `AddCookie` builder, same expiry
   and domain rules) and on `BrowserSession` (today `(name, value)`, which cannot set a
   path, domain or flag). Then `get_cookies` and `add_cookie` round-trip without JSON. A
   W3C object from elsewhere still works, through `serde_json::from_value::<Cookie>`:
   `Cookie` deserialization learns `expiry` as an alias of `expires`, and defaults for a
   missing `path`, `domain`, `secure` and `httpOnly`. Both are additive, so stealth's
   contract does not change.
5. **Two crate-private conversions** at the WebDriver boundary, in `src/cookie.rs`:
   - thirtyfour to ours: `path` `None` becomes `"/"`, `domain` `None` becomes `""`, a
     flag `None` becomes `false`, `expiry` becomes `expires as f64`, and
     `SameSite::Default` becomes `None`;
   - ours to thirtyfour: empty `domain` is left out, and `expires` is truncated to whole
     seconds.
   No public impl names a thirtyfour type.
6. **The saved-cookie files keep the W3C format** that Python SeleniumBase reads and
   writes. `load_cookies*` and `save_cookies` convert at the file boundary.
7. **`drivers::Cookie` goes.** `drivers` keeps only the escape-hatch types
   (`WebDriver`, `WebElement`).

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/cookie.rs` (new); `src/sb_cdp/state.rs` (moves out, re-exports); `src/lib.rs`; `src/drivers.rs:34`; `src/browser/session/page.rs:76-95`; `src/browser/session/cdp.rs:173-187`; `src/evasion/cdp.rs:172-178` (comment); `src/api/base_case.rs` (`get_cookie`, `get_cookies`, `add_cookie`, `load_cookies*`); `src/api/misc_calls.rs` (`AddCookie`); the MCP `get_cookies` tool (`src/mcp/webdriver.rs:746`): its JSON becomes the serialized `Vec<Cookie>`, which the CHANGELOG notes |
| Examples | `examples/cookies_storage.rs:42-60` (`.is_err()` becomes `is_none()`), `examples/basic_snippets.rs:91` |
| Docs | `docs/api/cookies-storage.md:22-23`, `docs/api/cdp-uc.md:39`, `docs/tutorials/cdp_mode.md:102`, `docs/concepts/sessions-and-state.md:420`, `docs/api/crate-root.md` (the new root items), the `get_cookie`/`get_cookies`/`add_cookie` rows in `parity/api.toml` (then `docs/parity.md` is regenerated), CHANGELOG under Breaking |
| Generated | `src/mcp/dev/api_index.json` (`SB_UPDATE_API_INDEX=1`), `tools/docs_layout_tree.py` (the new `cookie.rs`) |
| Contracts | `tests/contract_stealth.rs` is unchanged; check it with `mise run contracts` |

## Tests

- Unit tests for both conversions: every field, a session cookie, `SameSite::Default`,
  a missing `path` and `domain`.
- A test that deserializes a W3C object (with `expiry`, without `path`) into `Cookie`,
  and one that reads a saved file written by Python SeleniumBase (a fixture).
- A test against `MockWebDriver` (`src/api/base_case.rs`, module `webdriver_support`, as
  the element lane's session tests use it): `get_cookie` of a missing name is `Ok(None)`,
  and another driver error is still `Err`. Written first; it must fail before the change.
- A test that the DevTools path and the plain WebDriver path give equal `Vec<Cookie>`
  for the same cookies.
- An `#[ignore]`d Chrome test in `tests/chrome` for `get_cookie`, `get_cookies` and
  `add_cookie` in both modes. It runs only when the owner says the Mac is free.

## Open questions

1. **Where the type lives.** Recommendation: the new `src/cookie.rs`, with
   `seleniumbase_rs::Cookie` as the path the docs teach and `sb_cdp::Cookie` kept for the
   CDP engine and stealth.
2. **`get_cookie` returns `Option`.** Recommendation: yes; it is Python's `None`.
3. **`get_cookies` returns `Vec<Cookie>`.** Recommendation: yes; one shape in both modes,
   and the parity row loses its "Differs".
4. **`add_cookie` takes `&Cookie` instead of a JSON value.** Recommendation: yes, with
   W3C JSON still accepted through serde. Alternative: keep `&Value` for a dict-like
   Python feel, and add nothing.
5. **`drivers::Cookie` is removed.** Recommendation: yes; no consumer uses it.
