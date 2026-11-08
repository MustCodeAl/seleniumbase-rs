---
title: "Design: the session escape hatch, and what drivers keeps"
sidebarTitle: "What drivers keeps"
---

Status: proposal (2026-11-05). The last of the lanes that
[the element note](/design/element_type) recommends in open question 9: after
[the cookie note](/design/cookie_type), [the error note](/design/driver_error),
[the network throttle note](/design/network_throttle),
[the DevTools client note](/design/devtools_client) and
[the capabilities note](/design/capability_types), `seleniumbase_rs::drivers` shrinks to the
escape-hatch types. This note also settles the session's own escape hatch, which has two names.

## The problem

### Two names for one driver

`BrowserSession` hands out its WebDriver twice (`src/browser/session/mod.rs:84-126`):

- `driver(&self) -> &WebDriver` panics (an `expect`) when the session has none: a direct CDP
  attachment, after `quit`, or after a `reconnect` that could not start a browser;
- `webdriver(&self) -> Result<&WebDriver, SeleniumBaseError>` answers those cases with
  `Unsupported` (naming the CDP attachment) or `SessionNotStarted`.

Callers: `.webdriver()` is on 104 lines, all in `src/`; as its doc comment says, every
WebDriver-backed method of the session and of `BaseCase` goes through it. `.driver()` is on
3 lines, all in the tests that need a Firefox or an Edge driver
(`tests/firefox_geckodriver.rs:88`, `:90`, `tests/edge_msedgedriver.rs:67`). No consumer calls
either (stealth, the plugins, the Tauri plugin and the companion), and no contract file names
them. The crate's rule is that errors are `SeleniumBaseError` and `unwrap()` is for tests
(`CLAUDE.md`, Conventions); a public `expect` is the same panic.

Python's raw driver is `self.driver` (`base_case.py:118` and on), Selenium's `WebDriver`. This
crate's `BaseCase` has no accessor for it: no public method outside `BrowserSession` returns a
`&WebDriver` (a literal search of `&WebDriver` in `src/`). The `BaseCase` escape hatch is the
element's (`Element::as_webdriver_element`).

### What `drivers` holds

`src/drivers.rs:32-48` re-exports 16 thirtyfour items. Each one, what still needs it, and the
lane that removes it:

| Item | Needed by | Removed by | After |
|---|---|---|---|
| `Cookie` | `BaseCase::get_cookie`, `BrowserSession::get_cookie` | the cookie lane | goes |
| `WebDriverError` | the escape hatches: thirtyfour's own methods on `WebDriver` and `WebElement` fail with it, and `From` turns it into ours | | stays |
| `WebDriverErrorInner` | reading the cases of a `WebDriverError` | the error lane | goes |
| `ConnectionType`, `NetworkConditions` | `set_network_conditions` | the network throttle lane | go |
| `ChromiumLikeCapabilities`, `BrowserCapabilitiesHelper`, `Capabilities`, `CapabilitiesHelper`, `DesiredCapabilities`, `LoggingPrefsLogLevel` | `apply_to`, `apply_undetected_args` | the capabilities lane | go |
| `Key` | nothing: `Element::press` takes `seleniumbase_rs::Key`, which is `sb_cdp::Key` (`src/lib.rs:218`); thirtyfour's `Key` is used privately (`src/api/base_case.rs:47`, `src/browser/element.rs:17`) | this lane | goes |
| `By` | nothing since the element lane: a literal search of `: By` in `src/` finds only crate-private functions (`shadow_find.rs:182`, `:204`, `utils/selectors.rs:980`), and `tests/offline/docs_current.rs:172-177` already keeps it out of the pages | this lane | goes |
| `WebDriverResult` | nothing public: its one use is the return type of thirtyfour's `HttpClient` trait, which `src/browser/reuse.rs:794-799` implements | this lane | goes |
| `WebDriver` | `BrowserSession::webdriver` | | stays |
| `WebElement` | `Element::as_webdriver_element`, `into_webdriver_element` | | stays |

Two of these are worse than unused:

- **Two types named `Key`.** `seleniumbase_rs::Key` is ours, `seleniumbase_rs::drivers::Key`
  is thirtyfour's, and `docs/tutorials/getting_started.md` teaches both: `Key` "at the crate
  root" (`:372`) and `drivers::{DesiredCapabilities, Key, Cookie, NetworkConditions, ...}`
  (`:374`). One name, two types.
- **The module doc is out of date.** It says thirtyfour 0.37 (`src/drivers.rs:27-28`), where
  `Cargo.toml:82` asks for 0.38 and `Cargo.lock:6950-6951` resolves 0.38.1, and it gives
  `get_cookie` and `DesiredCapabilities` as the reasons the module exists (`:4-8`), which the
  cookie and capabilities lanes remove.

## The proposal

1. **One accessor: `BrowserSession::webdriver()`**, unchanged. `driver()` goes; the three test
   lines call `webdriver()` and `expect` there, in the test. The name also matches the element's
   `as_webdriver_element`: what comes out is the WebDriver engine's raw type.
2. **`drivers` keeps three items**, the escape-hatch types:

   ```rust
   pub use thirtyfour::error::WebDriverError;
   pub use thirtyfour::{WebDriver, WebElement};
   ```

   This lane removes `Key`, `By` and `WebDriverResult`; the other lanes remove theirs, and this
   lane checks that nothing else is left.
3. **The module doc says what the module is now**: the raw types of the WebDriver engine, for
   a call that `Element` and `BrowserSession` have no method for, with the thirtyfour version
   the crate builds against (0.38) and the rule that a program needing more of thirtyfour lists
   it in its own manifest. The example (`src/drivers.rs:10-25`) stays: it shows the element's
   escape hatch.
4. **The pages stop teaching what went**: `docs/tutorials/getting_started.md:26`, `:372-383`,
   `docs/ABI_API.md:29` and `context7.json:34` name only the three items, and `Key` at the
   crate root. Each removed path goes into the `GONE` list of
   `tests/offline/docs_current.rs`, as the element lane did for `By` (`:173-179`), so that no
   page teaches it again: `drivers::Key` (use `seleniumbase_rs::Key`), `drivers::Cookie`,
   `drivers::NetworkConditions`, `drivers::DesiredCapabilities`, `drivers::WebDriverErrorInner`,
   each lane adding its own.

### Order

The cookie, error, network throttle, DevTools client and capabilities lanes come first, in any
order: none of them touches another's items. This lane comes last, because it rewrites the
module doc and checks the final list. Removing `Key`, `By`, `WebDriverResult` and `driver()` does
not depend on the others and can move earlier if a lane is free.

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/drivers.rs` (the re-exports and the module doc); `src/browser/session/mod.rs:84-99` (`driver` goes, and the `webdriver` doc no longer points to it) |
| Tests | `tests/firefox_geckodriver.rs:88`, `:90`, `tests/edge_msedgedriver.rs:67` (real-driver binaries, compile-checked here); `crates/own-imports/tests/drivers_and_json_from_our_crate.rs` (imports only what `drivers` keeps); `tests/offline/docs_current.rs` (the `GONE` entries) |
| Docs | `docs/tutorials/getting_started.md:26`, `:372-383`; `docs/ABI_API.md:29`; `context7.json:34`; `docs/api/crate-root.md` if it lists the `drivers` items; CHANGELOG under Breaking |
| Generated | none |
| Contracts | unchanged; check with `mise run contracts` |

## Tests

- The own-imports test imports `drivers::{WebDriver, WebElement, WebDriverError}` and nothing
  else from `drivers`, and still compiles with no `thirtyfour` line in its manifest.
- `docs_current` fails for a page that names a removed path.
- No new behaviour test: `webdriver()` is unchanged and already covered by the `BaseCase` and
  session tests that go through it.

## Open questions

1. **Keep `webdriver()` and drop `driver()`.** Recommendation: yes; it is the one that does not
   panic, and 104 call sites already use it. Alternative: keep the name `driver()` with the
   `Result` (Python's `self.driver`), which renames 104 sites for no gain.
2. **A `BaseCase` accessor for the WebDriver**, as Python's `self.driver`. Recommendation: not
   in this lane; there is no parity row for it, and `Element` and the `execute_*` methods cover
   what the tests do. If one comes, it is `BaseCase::webdriver()`, the same name.
3. **Rename the module** (`webdriver` instead of `drivers`). Recommendation: no; the pages and
   `docs_current.rs` teach `drivers::WebElement`, and with three items left the name no longer
   misleads anyone into looking for capability or cookie types there.
4. **Keep `WebDriverError` in `drivers`.** Recommendation: yes, with the error lane's public
   `From`; a user writing a helper over the raw types can name the error they return.
