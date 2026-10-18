---
title: "ADR 002: Multi-Driver and Playwright Feature Design"
sidebarTitle: "002: Multi-driver and Playwright"
---

## Status
Accepted

## Context
The port needs to support both WebDriver (thirtyfour) and a Playwright-compatible backend (the `rustwright` crate), plus multiple simultaneous browser sessions.

## Decision
- `BaseCase` owns one active `BrowserSession` (a WebDriver handle plus an optional CDP client) and a table of the other browsers it opened, each named by a `DriverId`.
- `BaseCase::get_new_driver()` (or `get_new_driver_with(config)`) connects a new session, makes it the active one and returns its `DriverId`. The browser the case started with is `DriverId::DEFAULT`; the others are numbered in the order they were opened, and an id is never given to another browser. The session that was active is parked in the table. `switch_to_driver(id)` makes that browser the active one, so element and assertion helpers act on it, and parks the one you left; it does nothing when `id` is the active browser, so it can be repeated. An id the case never gave out, or one whose browser was quit, returns `SeleniumBaseError::InvalidConfig` and changes nothing.
- `switch_to_default_driver()` is `switch_to_driver(DriverId::DEFAULT)`: it always reaches the browser the case started with, as Python's `switch_to_default_driver` does with its fixed default driver.
- `quit_extra_driver(id)` quits a browser other than the default one. When it is the active browser, the case falls back to the default one, as in Python. `driver_count()` is the number of browsers still open, the active one included.
- `BaseCase::quit()` quits every browser the case opened, the parked ones too.
- Playwright support is gated behind the `playwright` feature flag, which pulls in `rustwright` and `rustwright-core`. When enabled, a separate `PlaywrightSession` (`src/browser/playwright.rs`, with thin helpers in `src/api/playwright.rs`) drives a Chromium launched by `rustwright`. `BaseCase` holds an optional one, started with `BaseCase::activate_playwright_mode()` and read back with `BaseCase::playwright_session()`; both exist only with the feature, and none of the WebDriver helpers act on it. `activate_playwright_mode` starts the second Chromium as the case's `BrowserConfig` says (without its profile folder, which the WebDriver browser holds), and `BaseCase::quit` closes it.

## Consequences
- Positive: Feature flags keep the default dependency graph small. No feature is on by default.
- Positive: Driver switching is explicit and test scripts can reason about which session is active.
- Positive: A session is never lost by switching. `DriverTable::switch` (in `src/api/base_case_impls/base_case_impl_drivers.rs`) parks the session you leave and brings the other in, so you can always return to it by its id. This matters because dropping a `BrowserSession` closes its browser in the background.
- Positive: A `DriverId` names one browser for as long as the case lives. It does not move when you switch, and a quit browser leaves a marker in the table, so switching to it is an error rather than a panic on a disconnected session.
- Negative: The ids are numbers in the order the browsers were opened, not names you choose; keep the id `get_new_driver` returns.
- Negative: Helpers that depend on Playwright-specific behavior live on `PlaywrightSession`, requiring users to learn two APIs.

## Later addition: the pure CDP engine
`src/sb_cdp` is a third, independent backend: it launches Chrome and speaks the DevTools Protocol directly (`Browser`, `Page`, `Locator`, with `BrowserContext`, `BrowserPool` and `SessionStore` on top). It has no chromedriver and no `thirtyfour` dependency, is always compiled, and is not reached through `BaseCase`. See the [Pure CDP guide](/tutorials/pure_cdp).

---

Verified against commit 8e9180f (multi-driver behaviour: `src/api/base_case_impls/base_case_impl_drivers.rs`; Playwright gating: `Cargo.toml`, `src/api/base_case.rs`).
