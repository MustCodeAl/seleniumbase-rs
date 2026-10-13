---
title: "ADR 002: Multi-Driver and Playwright Feature Design"
---

# ADR 002: Multi-Driver and Playwright Feature Design

## Status
Accepted

## Context
The port needs to support both WebDriver (thirtyfour) and a Playwright-compatible backend (the `rustwright` crate), plus multiple simultaneous browser sessions.

## Decision
- `BaseCase` owns one active `BrowserSession` (a WebDriver handle plus an optional CDP client) and a `Vec<BrowserSession>` of other sessions.
- `BaseCase::get_new_driver(Option<BrowserConfig>)` connects a new session and makes it the active one; passing `None` reuses the current `BrowserConfig`. The session that was active is parked in the vector, and the returned `usize` is its slot there. `switch_to_driver(index)` swaps the active session with the one parked in that slot, so element and assertion helpers act on the newly active one and the session you left stays open, parked in the same slot. Calling `switch_to_driver` again with the same index switches back. An `index` outside the vector returns `SeleniumBaseError::InvalidConfig` and changes nothing. `quit_extra_driver(index)` quits the session parked in a slot, and `driver_count()` reports the length of the vector.
- `switch_to_default_driver()` is `switch_to_driver(0)`, a swap with slot 0. Slot 0 holds the original session only until the first switch: after that it holds whichever session was left most recently.
- `BaseCase::quit()` quits only the active session. Parked sessions are not quit by it; they are closed when the `BaseCase` is dropped (see Consequences), or earlier by `quit_extra_driver`.
- Playwright support is gated behind the `playwright` feature flag, which pulls in `rustwright` and `rustwright-core`. When enabled, a separate `PlaywrightSession` (`src/browser/playwright.rs`, with thin helpers in `src/api/playwright.rs`) drives a Chromium launched by `rustwright`. `BaseCase` holds an optional one, started with `BaseCase::activate_playwright_mode()` and read back with `BaseCase::playwright_session()`; both exist only with the feature, and none of the WebDriver helpers act on it.

## Consequences
- Positive: Feature flags keep the default dependency graph small. No feature is on by default.
- Positive: Driver switching is explicit and test scripts can reason about which session is active.
- Positive: A session is never lost by switching. `switch_to_driver` parks the session you leave in the slot the other came from (the swap is `swap_with_parked` in `src/api/base_case_impls/base_case_impl_drivers.rs`), so you can always return to it with the same index. This matters because dropping a `BrowserSession` closes its browser in the background.
- Negative: Sessions are addressed by position in the vector, not by name, and the position of a given session changes as you switch: after `switch_to_driver(i)` the session you left is at `i`, and the one you entered is no longer parked. Keep track of which index holds which session.
- Negative: `quit_extra_driver(i)` leaves a disconnected placeholder in slot `i` so the other slots keep their indices. `driver_count()` therefore does not shrink after a quit, and switching to a quit slot makes a disconnected session active, whose WebDriver calls panic. Do not switch to a slot you have quit.
- Negative: Parked sessions are closed only by `quit_extra_driver` or by `Drop` when the `BaseCase` goes away. `BaseCase::quit()` does not quit them.
- Negative: Helpers that depend on Playwright-specific behavior live on `PlaywrightSession`, requiring users to learn two APIs.

## Later addition: the pure CDP engine
`src/sb_cdp` is a third, independent backend: it launches Chrome and speaks the DevTools Protocol directly (`Browser`, `Page`, `Locator`, with `BrowserContext`, `BrowserPool` and `SessionStore` on top). It has no chromedriver and no `thirtyfour` dependency, is always compiled, and is not reached through `BaseCase`. See the [Pure CDP guide](../tutorials/pure_cdp.md).

---

Verified against commit 8e9180f (multi-driver behaviour: `src/api/base_case_impls/base_case_impl_drivers.rs`; Playwright gating: `Cargo.toml`, `src/api/base_case.rs`).
