# ADR 002: Multi-Driver and Playwright Feature Design

## Status
Accepted

## Context
The port needs to support both WebDriver (thirtyfour) and a Playwright-compatible backend (the `rustwright` crate), plus multiple simultaneous browser sessions.

## Decision
- `BaseCase` owns one active `BrowserSession` (a WebDriver handle plus an optional CDP client) and a `Vec<BrowserSession>` of other sessions.
- `BaseCase::get_new_driver(Option<BrowserConfig>)` connects a new session and makes it the active one; passing `None` reuses the current `BrowserConfig`. The session that was active is moved into the vector, and the returned `usize` is its slot there. `switch_to_driver(index)` moves the session in that slot into the active position, so element and assertion helpers act on it. `quit_extra_driver(index)` quits a session in the vector, and `driver_count()` reports its length.
- Playwright support is gated behind the `playwright` feature flag, which pulls in `rustwright` and `rustwright-core`. When enabled, a separate `PlaywrightSession` (`src/browser/playwright.rs`, with thin helpers in `src/api/playwright.rs`) drives a Chromium launched by `rustwright`. `BaseCase` holds an optional one, started with `BaseCase::activate_playwright_mode()` and read back with `BaseCase::playwright_session()`; both exist only with the feature, and none of the WebDriver helpers act on it.

## Consequences
- Positive: Feature flags keep the default dependency graph small. No feature is on by default.
- Positive: Driver switching is explicit and test scripts can reason about which session is active.
- Negative: Sessions are addressed by position in the vector, not by name, and `switch_to_driver` moves a session out of its slot without putting the previously active one back: that session is dropped, and dropping a `BrowserSession` closes its browser in the background. Do not rely on being able to return to the session you left.
- Negative: Helpers that depend on Playwright-specific behavior live on `PlaywrightSession`, requiring users to learn two APIs.

## Later addition: the pure CDP engine
`src/sb_cdp` is a third, independent backend: it launches Chrome and speaks the DevTools Protocol directly (`Browser`, `Page`, `Locator`, with `BrowserContext`, `BrowserPool` and `SessionStore` on top). It has no chromedriver and no `thirtyfour` dependency, is always compiled, and is not reached through `BaseCase`. See the [Pure CDP guide](../tutorials/pure_cdp.md).
