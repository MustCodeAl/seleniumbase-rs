---
title: "Design: our own error for a failed WebDriver command"
sidebarTitle: "Our own WebDriver error"
---

Status: implemented on `lane/thirtyfour`, with every recommendation of the open questions accepted (2026-11-06). One of the lanes that
[the element note](/design/element_type) recommends in open question 9 (one lane for each
thirtyfour type left in the public API), after [the cookie note](/design/cookie_type). This one
is `SeleniumBaseError::WebDriver`, which any call of the WebDriver engine can return.

The note was written before the code and is kept as it was; a few things turned out differently.
`FatalError` is a case of thirtyfour 0.38.1 (the crate's own test builds one), and it is
`Other`, as the table says. `DriverError::new` serves the tests and `explain_error`
(`src/mcp/dev/diagnose.rs`), and there the kind comes from the words of the message
(`driver_kind_of`), the one place that still reads words: the error the text came from is gone.
The reason of the two variants the conversion makes (`BrowserDisconnected`, `InvalidSelector`)
and of the `ElementNotInteractable` that `Element::failed` makes is the driver's message on one
line, without the page data and the stack trace (`summary`), where the full text stays the
`message()`. `Element::find` names the chain it looked for (`#cart >> .row`), not the element it
searched in, and `BrowserSession::get_cookie` reads the kind through the crate-private
`driver_kind`, as `shadow_find` does, because both hold thirtyfour's error and not ours.
`SeleniumBaseError::WebDriver` has no `#[source]`, as the snippet shows: the thirtyfour error is
the source of the `DriverError`, one step down, and it is not boxed a second time (`WebDriverError`
is a box already), so `source().downcast_ref::<WebDriverError>()` finds it. Problem 2 holds for
the waits that poll through `poll_until_found`; `BaseCase::wait_for_element_present` has a loop
of its own that ignores every error until its timeout, and is not changed here.

## The problem

The variant holds thirtyfour's error (`src/error.rs:74-76`):

```rust
/// The WebDriver backend returned an error.
#[error("webdriver command failed: {0}")]
WebDriver(#[from] thirtyfour::error::WebDriverError),
```

A test that wants to know what the driver said has to name thirtyfour: `as_inner()` gives
`drivers::WebDriverErrorInner` (`src/drivers.rs:35`), thirtyfour's own `#[non_exhaustive]`
enum. Context7 (`/websites/rs_thirtyfour`, `error.rs`) lists 31 cases on docs.rs's latest
page; `Cargo.lock:6950-6951` resolves thirtyfour 0.38.1, and the crate's tests also build a
`FatalError` (`error.rs:1156`) that the page does not list, so the lane checks the cases
against 0.38.1 with rust-analyzer before writing the table below into code.

Who builds and reads it. A literal search for `SeleniumBaseError::WebDriver` finds 57 lines:
3 in the element note and 54 in code.

- **39 build it in library code**, as `.map_err(SeleniumBaseError::WebDriver)` or
  `SeleniumBaseError::WebDriver(error)`: `src/browser/session/page.rs` 9,
  `session/navigation.rs` 6, `base_case_impl_extra.rs` 7, `base_case_impl_links.rs` 4,
  `src/api/switching.rs` 4 (the alert methods, `:404-453`), `base_case_impl_window.rs` 3,
  `base_case_impl_switching.rs` 2 (`:30`, `:55`), `session/cdp.rs` 2,
  `base_case_impl_media.rs` 1 and `base_case_impl_storage.rs` 1. One more builds it under an
  alias from a message: `src/mcp/dev/diagnose.rs:334-336` (`UnknownResponse(0, text)`).
- **2 read it**: `Element::failed` (`src/browser/element.rs:103-121`) turns thirtyfour's
  `StaleElementReference` into `StaleElement` with the selector, and `concerns_page`
  (`src/mcp/failure.rs:120-136`) counts it as a page problem. `error.rs` itself matches it in
  `category` (`:511`), `exit_code` (`:608`) and `hint` (`:709`).
- **9 are tests** (among them `error.rs:1156` and
  `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:57-61`), **4 are doc comments**
  (`base_case_impl_addon.rs:98`, `:130`, `base_case_impl_fingerprint.rs:23`,
  `base_case_impl_mouse.rs:246`).
- Beside them, `#[from]` turns every `?` on a thirtyfour result into this variant. Those sites
  cannot be counted by text, and the proposal keeps them compiling.

Two more places look into thirtyfour's enum: `shadow_find::is_missing` checks `NoSuchElement`
(`src/browser/shadow_find.rs:85-87`), and `webdriver_hint` guesses the case from the words
of the message (`error.rs:967-991`: "no such element", "stale element", "click intercepted",
"session not created", "invalid session", "timeout").

Three behaviours are wrong because the variant is opaque:

1. **`BrowserSession::find` of a missing element is `WebDriver`, not `ElementNotFound`.**
   `find` (`src/browser/session/elements.rs:158-167`) passes the driver's answer on through
   `?` (`shadow_find.rs:184`, `:187`), so `is_element_error()` is false and `exit_code()` is
   4, "the browser or its driver failed", instead of 3 (`error.rs:567-625`). Only the
   `BaseCase` waits, which poll, end in their own `WaitTimeout`.
2. **A wait goes on after the browser is gone.** `poll_until_found` stops early only for
   `InvalidSelector`, `InvalidConfig`, `Unsupported`, `SessionNotStarted` and
   `BrowserDisconnected` (`base_case_impl_common.rs:32-41`, `:62-91`). The driver's "invalid
   session id" arrives as `WebDriver`, so a wait polls a dead session until its timeout; a
   selector the browser itself refuses ("invalid selector") is retried the same way. This is
   read from the code; the lane's first test shows it.
3. **The hint guesses from words.** A driver that words a message differently loses the
   advice, although thirtyfour already says which case it is.

### Python parity

Python SeleniumBase raises two families of exceptions:

- **Selenium's**, which is what the driver's answer becomes: `base_case.py:53-63` imports
  `ElementClickInterceptedException`, `ElementNotInteractableException`,
  `InvalidArgumentException`, `MoveTargetOutOfBoundsException`, `NoSuchElementException`,
  `NoSuchWindowException`, `StaleElementReferenceException`, `TimeoutException` and
  `WebDriverException`; `page_actions.py:24-30` adds `ElementNotVisibleException`,
  `NoAlertPresentException` and `NoSuchAttributeException`; `browser_launcher.py:17-19` adds
  `InvalidSessionIdException` and `SessionNotCreatedException`. `common/exceptions.py:65-117`
  keeps simplified copies, with `NoSuchFrameException` and `InvalidElementStateException`, all
  under `WebDriverException`.
- **Its own** (`common/exceptions.py:1-14`: `LinkTextNotFoundException`,
  `NoSuchFileException`, `NoSuchOptionException`, `TextNotVisibleException`,
  `TimeLimitExceededException`, `NotUsingChromeException` and the others), raised by
  SeleniumBase and not by the driver. They are this crate's other variants and not this
  note's subject; one is already recorded (`NoSuchFileException` is `WaitTimeout`,
  `parity/api.toml:1963`).

So a Python test can tell "no such window" from "no alert" from "stale element" by the class.
A Rust test can only by naming thirtyfour.

### Consumers

- **Stealth** turns `SeleniumBaseError::Navigation` into its own error and passes every other
  variant on unchanged (`crates/scrape/src/error.rs:82-94` in seleniumbase-rs-stealth); it never
  names `WebDriver`.
- **The Tauri plugin** has a `WebDriverError` of its own (`src/webdriver.rs`, `src/lib.rs:52`):
  the error its W3C endpoint answers with, a type of that crate. It imports nothing of ours by
  that name; its contract (`tests/contract_tauri.rs:26`) imports `SeleniumBaseError` and matches
  `BrowserDisconnected` only (`:247`).
- **The plugins and the companion**: no hit. No contract file names the variant, `drivers::`
  or `WebDriverErrorInner`.

## The proposal

1. **Our type in the variant.** The message and the `#[error]` text stay:

   ```rust
   /// The WebDriver engine's driver failed a command, or could not be reached.
   #[error("webdriver command failed: {0}")]
   WebDriver(DriverError),
   ```

   `DriverError` lives in `src/error.rs` and is re-exported at the crate root beside
   `SeleniumBaseError`:

   ```rust
   /// What the driver (chromedriver, geckodriver, msedgedriver, ...) answered
   /// when it failed a command.
   #[derive(Debug)]
   pub struct DriverError {
       kind: DriverErrorKind,
       message: String,
       source: Option<Box<thirtyfour::error::WebDriverError>>,
   }

   impl DriverError {
       pub fn kind(&self) -> DriverErrorKind;
       pub fn message(&self) -> &str;
   }
   ```

   `Display` prints the message, and `std::error::Error::source` gives the thirtyfour error as
   a `&dyn Error`: it is kept, but only as the private source, so no public field or method
   names its type. A crate-private `DriverError::new(kind, message)` serves
   `diagnose.rs:334-336` and the tests.

2. **`DriverErrorKind`**, `#[non_exhaustive]`, `Copy`, `Eq`, in this crate's words. Where a
   variant of `SeleniumBaseError` already means the same thing, the kind has its name:

   | Kind | thirtyfour 0.38 (`WebDriverErrorInner`) | Python exception |
   |---|---|---|
   | `ElementNotFound` | `NoSuchElement` | `NoSuchElementException` |
   | `StaleElement` | `StaleElementReference` | `StaleElementReferenceException` |
   | `ElementNotInteractable` | `ElementNotInteractable`, `ElementClickIntercepted`, `InvalidElementState`, `MoveTargetOutOfBounds` | `ElementNotInteractableException`, `ElementClickInterceptedException`, `InvalidElementStateException`, `ElementNotVisibleException`, `MoveTargetOutOfBoundsException` |
   | `FrameNotFound` | `NoSuchFrame` | `NoSuchFrameException` |
   | `WindowNotFound` | `NoSuchWindow` | `NoSuchWindowException` |
   | `AlertNotFound` | `NoSuchAlert` | `NoAlertPresentException` |
   | `CookieNotFound` | `NoSuchCookie` | none: Python's `get_cookie` returns `None`, and the cookie lane turns this kind into `Ok(None)` |
   | `ScriptFailed` | `JavascriptError` | none among the classes SeleniumBase imports |
   | `TimedOut` | `Timeout`, `WebDriverTimeout`, `ScriptTimeout` | `TimeoutException` |
   | `BrowserLaunch` | `SessionNotCreated` | `SessionNotCreatedException` |
   | `InvalidArgument` | `InvalidArgument`, `InvalidUrl`, `InvalidCookieDomain`, `UnableToSetCookie` | `InvalidArgumentException` |
   | `Unreachable` | `RequestFailed`, `HttpError`, `IoError` | `WebDriverException` |
   | `Other` | the rest (`UnknownResponse`, `NotInSpec`, `ParseError`, `Json`, `DecodeError`, `InsecureCertificate`, `NotFound`, and `FatalError` if 0.38.1 has it as a case) and any case thirtyfour adds | `WebDriverException` |

   `TimedOut` is not `WaitTimeout`: that one is this crate's own wait running out
   (`error.rs:48-50`), this one is the driver's command or script running out of time.
   Python's `NoSuchAttributeException` has no driver case: `Element::attribute` answers `None`
   for a missing attribute (`src/browser/element.rs:197-220`).

3. **Two answers need no context and already have a variant**, so the conversion gives the
   variant instead of a kind:
   - `InvalidSessionId` becomes `BrowserDisconnected { reason }`. It is what `webdriver_hint`
     already tells the user ("The browser behind this session has closed or crashed",
     `error.rs:982-983`); a wait then stops at once (problem 2), and `is_transient()` answers
     as for any other disconnection (`error.rs:470-479`).
   - `InvalidSelector` becomes `InvalidSelector(message)`, which a wait already treats as
     final.

4. **The conversion stays a public `From`**: `impl From<drivers::WebDriverError> for
   SeleniumBaseError`. It is the one public item here that names thirtyfour, on purpose: the
   escape hatches (`Element::as_webdriver_element`, `BrowserSession::webdriver`) hand out
   thirtyfour's own types, whose methods return thirtyfour results, and their `?` must still
   become ours (open question 1). Every `?` in the crate keeps working. The 39
   `.map_err(SeleniumBaseError::WebDriver)` sites become `?` or
   `.map_err(SeleniumBaseError::from)`, in one `ast_grep_rewrite`.

5. **The sites that know the selector use it**, as `Element::failed` does for a stale element
   today: `Element::failed` also for `ElementNotFound` (a `find` inside an element) and
   `ElementNotInteractable`, and `BrowserSession::find` and `find_all`
   (`session/elements.rs`) for `ElementNotFound`, so the error names the selector.
   `shadow_find::is_missing` checks the kind.

6. **The hint reads the kind** (`error.rs:709`), and `webdriver_hint`'s word list goes.
   `category()` stays `"webdriver"` and `exit_code()` stays 4.

7. **`drivers::WebDriverErrorInner` goes.** `drivers::WebDriverError` stays, as the error of
   the escape-hatch types.

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/error.rs` (the variant, `DriverError`, `DriverErrorKind`, the `From`, `hint`; `webdriver_hint` goes); `src/lib.rs` (the root re-exports); `src/drivers.rs:35`; the 39 `map_err` sites; `src/browser/element.rs:103-121`; `src/browser/shadow_find.rs:85-87`; `src/browser/session/elements.rs` (`find`, `find_all`); `src/mcp/dev/diagnose.rs:334-336` |
| Tests | `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:13-16`, `:57-61` (builds the error through `From`); `error.rs:1145-1170` (`one_of_each`); the tests that match `WebDriver(_)` keep compiling, the pattern is the same |
| Docs | `docs/api/crate-root.md:50` (rows for `DriverError` and `DriverErrorKind`); the four doc comments above, each checked against what the method now returns; CHANGELOG under Breaking |
| Generated | `src/mcp/dev/api_index.json` only if a `BaseCase` signature changes (none does) |
| Contracts | unchanged; check with `mise run contracts` |

## Tests

- Unit tests of the conversion: one for each row of the table, from an error built with
  thirtyfour's `WebDriverError::from_inner` (Context7); `InvalidSessionId` gives
  `BrowserDisconnected` and `InvalidSelector` gives `InvalidSelector`; `source()` is the
  thirtyfour error.
- A test against `MockWebDriver`, whose `Reply::Error(status, code)`
  (`base_case_impl_common.rs:767`) the element tests already use for "no such element":
  `BrowserSession::find` of a missing element is `ElementNotFound` naming the selector.
  Written first; it must fail before the change.
- A `MockWebDriver` test: a `BaseCase` wait whose driver answers "invalid session id" returns
  `BrowserDisconnected` long before its timeout. Written first.
- `hint()` for each kind, and `exit_code()` unchanged for `WebDriver`.
- The own-imports test proves that the `From` compiles for a crate that imports only
  `seleniumbase_rs`.

## Open questions

1. **Keep the public `From<drivers::WebDriverError>`.** Recommendation: yes; the escape
   hatches return thirtyfour results, and `?` on them is the reason they are usable.
   Alternative: a crate-private conversion and no `?` for escape-hatch calls.
2. **Kinds that repeat a variant's name** (`ElementNotFound`, `StaleElement`,
   `ElementNotInteractable`, `BrowserLaunch`). Recommendation: yes, one name for one meaning;
   the variant is what this crate found out (with the selector or the browser), the kind is
   what the driver said.
3. **Should the conversion also turn the element kinds into the element variants**, with an
   empty selector? Recommendation: no; only the sites that know the selector do it
   (proposal 5), so `ElementNotFound` always names one.
4. **`CdpDriver`** (`error.rs:78-80`) says "driver" for the pure CDP engine, which has none,
   next to a `DriverError` that is the WebDriver driver's answer. Recommendation: rename it
   in a lane of its own; stealth builds and matches it (`crates/scrape/src/error.rs:141-153`,
   `crates/wire/src/stealth_session/error.rs:99-126`,
   `crates/wire/src/http_session/page.rs:361` in seleniumbase-rs-stealth), so that lane updates
   the stealth contract.
5. **A public constructor for `DriverError`**, for a user's own mock. Recommendation: no;
   no consumer builds one, and the crate-private one serves `diagnose.rs` and the tests.
