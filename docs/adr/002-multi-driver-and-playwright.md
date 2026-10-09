# ADR 002: Multi-Driver and Playwright Feature Design

## Status
Accepted

## Context
The port needs to support both WebDriver (thirtyfour) and Playwright (padamson/playwright-rs) backends, plus multiple simultaneous browser sessions.

## Decision
- `BaseCase` owns an optional primary WebDriver handle and a map of extra named drivers (`HashMap<String, WebDriver>`).
- Switching drivers updates the active handle used by element and assertion helpers.
- Playwright support is gated behind the `playwright` feature flag. When enabled, a separate `PlaywrightManager` is available but is not mixed into the core `BaseCase` API to keep compile times reasonable.
- New drivers are created via `BaseCase::get_new_driver`, which accepts the same `BrowserConfig` used for the primary session.

## Consequences
- Positive: Feature flags keep the default dependency graph small.
- Positive: Driver switching is explicit and test scripts can reason about which session is active.
- Negative: Helpers that depend on Playwright-specific behavior live outside `BaseCase`, requiring users to learn two APIs.
