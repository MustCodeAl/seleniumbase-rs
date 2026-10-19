---
title: "Best practices"
sidebarTitle: "Best practices"
description: "Habits that keep seleniumbase-rs tests fast, stable and safe."
---

<Info>

**In short:** pick stable selectors, let the methods wait instead of sleeping,
always close the browser, run headless in CI, keep secrets out of the
repository, and keep a browser identity consistent. Each habit below links to
the page that explains it.

</Info>

## Writing tests

<AccordionGroup>

<Accordion title="Pick selectors that survive a redesign" icon="crosshairs" defaultOpen>

Prefer an `id` or a test attribute (`#checkout`, `[data-testid="total"]`) over a
long CSS path or a position (`div > div:nth-child(3)`). Use link text
(`link=Sign in`) for links whose text is the contract. See
[Selectors](/tutorials/selectors).

</Accordion>

<Accordion title="Wait for the page, never sleep" icon="hourglass-half">

Actions and assertions wait for their element, so a fixed `sleep` only makes a
test slower, or flaky when the page is slower than the sleep. When you need to
wait for something an action does not cover, use a wait method with a timeout
(`wait_for_element_visible(selector, timeout_secs)`). See
[Waits and assertions](/tutorials/waits_assertions).

</Accordion>

<Accordion title="Assert what the user sees" icon="eye">

Check visible text and elements (`assert_text_visible("Welcome!", "body")`)
rather than internals. Note the order: the expected text comes first, then the
selector.

</Accordion>

<Accordion title="End every call with ?" icon="circle-exclamation">

Every call returns a `Result`. `?` stops the test at the first failure with an
error that names the selector; ignoring the result hides failures. See
[Taking advantage of Rust](/rust-advantages).

</Accordion>

<Accordion title="Always close the browser" icon="power-off">

A test that fails before `sb.quit()` leaves a Chrome running. `sb_test!` and
`run_browser_test` always quit, even on failure. See [Macros](/tutorials/macros).

</Accordion>

<Accordion title="Write helpers against the traits" icon="layer-group">

A helper written for `BrowserApi` and `ElementApi` runs on both engines, so you
can switch a suite from WebDriver to pure CDP without rewriting it. See
[Capability traits](/api/traits).

</Accordion>

</AccordionGroup>

## Running tests

<AccordionGroup>

<Accordion title="Run headless in CI and containers" icon="eye-slash" defaultOpen>

There is no screen there. Set `SB_HEADLESS=true` for `sbrs` and code that loads
`Settings`; `BrowserConfig::default()` is already headless. Debug headed on
your own machine. See [Headless and headed browsers](/tutorials/headless).

</Accordion>

<Accordion title="Keep the driver matched to the browser" icon="plug">

chromedriver must have the same major version as Chrome. `sbrs doctor` shows
both; `sbrs install` fetches the matching driver. Or use the pure CDP engine,
which needs no driver. See [WebDriver and drivers](/concepts/webdriver).

</Accordion>

<Accordion title="Reuse browsers instead of starting one per task" icon="recycle">

Starting Chrome is the slowest step. Share browsers between tests with
`--reuse-session`, or hand out pages from a `BrowserPool` in the pure CDP
engine. See [Pure CDP](/tutorials/pure_cdp).

</Accordion>

<Accordion title="Keep evidence of a failure" icon="camera">

Save a screenshot and the page source when a test fails
(`save_screenshot`, `save_page_source_to_logs`), and turn on logs with
`SB_LOG_LEVEL=debug` when you investigate. See
[Tracing and logging](/tutorials/tracing).

</Accordion>

</AccordionGroup>

## Staying safe and polite

<AccordionGroup>

<Accordion title="Keep credentials out of the repository" icon="key" defaultOpen>

Read passwords and TOTP secrets from environment variables, and never commit
saved cookie files: anyone with them is signed in as you. See
[Sessions, cookies and storage](/concepts/sessions-and-state) and
[MFA and TOTP](/tutorials/mfa_totp).

</Accordion>

<Accordion title="Do not overload a site" icon="gauge">

Space out requests and retry with backoff instead of hammering a page; the crate
has `RetryPolicy` and `RateLimiter` for this. Respect a site's terms of use.

</Accordion>

<Accordion title="Keep one identity consistent" icon="fingerprint">

If you change the browser's identity, change it as a whole: a `Fingerprint`
draws random values together (operating system, region, locale), and `.seed(..)`
makes a run repeatable. A proxy in one country with a time zone in another is a
giveaway. See [Detection and spoofing](/concepts/detection).

</Accordion>

</AccordionGroup>

## Where to go next

<Columns cols={2}>

<Card title="Common problems" icon="triangle-exclamation" color="#d52c36" href="/help/common_problems">
  What to do when a test will not start or fails oddly.
</Card>

<Card title="Writing browser tests" icon="flask" color="#1a85d4" href="/rust-test-tooling">
  Test layout, fixtures and running the suite.
</Card>

</Columns>
