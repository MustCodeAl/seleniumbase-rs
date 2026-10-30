---
title: "Reliable Browser Tests"
sidebarTitle: "Reliable tests"
description: "Fifteen rules for browser tests that do not flake, leak browsers or hide failures, each with a do and an avoid, written for the sb fixtures, BaseCase and the pure CDP engine. For people and for AI coding assistants."
---

`seleniumbase-rs` waits for elements, closes browsers and keeps the evidence of a
failure for you, as long as the test lets it. Most flaky or leaky tests have the
same few causes: a browser the test started itself, a fixed sleep, a selector
that matches the wrong element, a handle that went stale, one tab used by two
flows. These rules are the short list that prevents them.

They are written for people and for AI coding assistants. The same list is in
[`AGENTS.md`](https://github.com/MustCodeAl/seleniumbase-rs/blob/main/AGENTS.md)
at the root of the repository, ready to paste into the instructions of a project
that uses the crate. It follows the checklist that `thirtyfour` publishes as
[Reliable AI-Generated Tests](https://github.com/stevepryde/thirtyfour/blob/main/docs/src/getting-started/reliable-tests.md),
with every item written in terms of this crate's API.

<Info>

**What you will learn**

- Which test shape closes the browser and keeps evidence whatever happens.
- How to wait, assert and select without sleeps, stale handles or ambiguous matches.
- How to keep evidence bounded, concurrent flows apart and raw protocol code contained.
- What the crate cannot enforce for you yet.

</Info>

<Visibility for="agents">

Summary for AI exports: write each browser test as `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { .. }).await }` (feature `testing`), or `sb_cdp: SbCdp` with `sb_cdp.run(async |page| { .. })` for the pure CDP engine; never start a browser by hand in a test body. Import only from `seleniumbase_rs`. Use stable selectors (`#id`, `[data-testid="x"]`, `link=Text`). Never sleep: actions wait, and `wait_for_element_visible`, `wait_for_text`, `wait_for_element_absent` or `locator.wait_for(State::Absent)` cover the rest. Assert with the retrying assertions (`assert_text(text, selector)` takes the text first, `locator.expect().to_have_text(..)`), not `get_text` plus `assert_eq!`. A selector or `Locator` acts on the first match: scope it, and use `expect().to_have_count(1)` when there must be exactly one. Do not keep a `WebElement` across a page change. Cleanup: the fixture and `run_browser_test` close the browser; a `Browser` you launch is closed on both paths. Evidence is `latest_logs/<test id>/`; add to it with `save_screenshot_to_logs` and `save_page_source_to_logs`, never print page source. One fixture test, one `BrowserPool` lease or one `BrowserContext` per concurrent flow. CI runs `SB_HEADLESS=true cargo test`. Raw protocol calls (`Page::execute`, `Browser::execute`, `BaseCase::execute_cdp_cmd`) live in one helper behind a feature of the test crate. Test code that drives pages without Chrome through `Browser::new_mocked()` (feature `test-util`). Credentials and data come from `sb.account()`, `sb.data()`, `sb.variables()`, never from the file.

</Visibility>

## The rules

| # | Rule |
|---|---|
| 1 | [Let the fixture own the browser](#1-let-the-fixture-own-the-browser) |
| 2 | [Import from `seleniumbase_rs` only](#2-import-from-seleniumbase_rs-only) |
| 3 | [Use selectors the application owns](#3-use-selectors-the-application-owns) |
| 4 | [Wait for a condition, never for time](#4-wait-for-a-condition-never-for-time) |
| 5 | [Assert with the retrying assertions](#5-assert-with-the-retrying-assertions) |
| 6 | [Scope the query and know how many elements match](#6-scope-the-query-and-know-how-many-elements-match) |
| 7 | [Find again instead of holding an element](#7-find-again-instead-of-holding-an-element) |
| 8 | [Make cleanup run on every path](#8-make-cleanup-run-on-every-path) |
| 9 | [Keep the failure evidence bounded](#9-keep-the-failure-evidence-bounded) |
| 10 | [One browser, context or lease per concurrent flow](#10-one-browser-context-or-lease-per-concurrent-flow) |
| 11 | [Run headless in CI, and set it outside the code](#11-run-headless-in-ci-and-set-it-outside-the-code) |
| 12 | [Write a body that is safe to run twice](#12-write-a-body-that-is-safe-to-run-twice) |
| 13 | [Keep raw protocol calls in one helper](#13-keep-raw-protocol-calls-in-one-helper) |
| 14 | [Test page-driving code without Chrome](#14-test-page-driving-code-without-chrome) |
| 15 | [Read credentials and data from the run](#15-read-credentials-and-data-from-the-run) |

### 1. Let the fixture own the browser

A test that asks for `sb: Sb` (or `sb_cdp: SbCdp`) gets a managed browser.
`sb.run` launches it, calls your closure, runs it again when `SB_RERUNS` allows,
writes the evidence of a failure, and closes the browser whether the body passed,
returned an error or panicked. The test never starts or stops a browser itself.

<CodeGroup>

```rust Do (WebDriver)
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn cart_shows_the_total(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://shop.example.com/cart").await?;
        sb.assert_text("$42.00", "#total").await
    })
    .await
}
```

```rust Do (pure CDP)
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn cart_shows_the_total(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://shop.example.com/cart").await?;
            page.locator("#total").expect().to_have_text("$42.00").await
        })
        .await
}
```

```rust Avoid
let mut sb = BaseCase::new(BrowserConfig::default()).await?;
sb.open("https://shop.example.com/cart").await?;
sb.assert_text("$42.00", "#total").await?; // fails: the quit below never runs
sb.quit().await?;
```

</CodeGroup>

Without the feature `testing`, `run_browser_test(BrowserConfig::default(), async |sb| { .. })`
and `run_cdp_test(LaunchOptions::default(), async |page| { .. })` close the browser
on every path too, but they read no configuration (no reruns, no time limit) and
keep no evidence. [Writing Tests with the sb Fixture](/tutorials/writing_tests)
has the setup and the settings.

### 2. Import from `seleniumbase_rs` only

The manifest of a test crate lists `seleniumbase-rs` with the feature `testing`
and nothing else. Every type the API takes or returns, and the macros' runtime,
come from the crate: start a file with `use seleniumbase_rs::tokio;` (or put
`tokio` in the braces of the import you already have) before `#[tokio::main]` or
`#[tokio::test]`. Do not add `thirtyfour`, `tokio`, `rstest` or `serde_json` to
`Cargo.toml` for the sake of a test.

<CodeGroup>

```toml Do
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing"] }
```

```rust Do
use seleniumbase_rs::drivers::By; // a WebDriver type, through the crate
use seleniumbase_rs::json::json; // the json! macro, through the crate
use seleniumbase_rs::tokio;
```

</CodeGroup>

[Imports and dependencies](/imports) lists every re-export and the few lines
that must stay.

### 3. Use selectors the application owns

Prefer an `id` or a test attribute, and link text where the text is the contract.
Avoid positions, generated class names and long paths: they break when the page
is redesigned, not when it is wrong.

<CodeGroup>

```rust Do
sb.click("#checkout").await?;
sb.assert_text("$42.00", r#"[data-testid="total"]"#).await?;
sb.click("link=Sign in").await?;
```

```rust Avoid
sb.click("div.main > div:nth-child(3) > button").await?;
sb.assert_text("$42.00", "body > div > span.price-2").await?;
```

</CodeGroup>

A prefix decides the strategy (`#`, `link=`, `//` for XPath); the full table is
in [Selectors](/tutorials/selectors). Ask the team that owns the page for a
`data-testid` rather than reaching for XPath.

### 4. Wait for a condition, never for time

Actions wait for their element for 10 seconds by default, so a `click` or
`type_text` needs no wait before it. For anything an action does not cover, wait
for the condition. `sb.sleep(..)`, `tokio::time::sleep` and hand-written poll
loops make a test slow when the page is fast and flaky when it is slow.

<CodeGroup>

```rust Do (WebDriver)
sb.click("#save").await?;
sb.wait_for_element_visible("#saved-toast", 10).await?;
sb.wait_for_element_absent(".spinner", 10).await?;
sb.wait_for_text("Saved", "#status", 10).await?;
```

```rust Do (pure CDP)
use seleniumbase_rs::sb_cdp::State;

page.locator("#save").click().await?;
page.locator(".spinner").wait_for(State::Absent).await?;
page.locator("#status").expect().to_have_text("Saved").await?;
```

```rust Avoid
sb.click("#save").await?;
sb.sleep(3.0).await; // too long when fast, too short when slow
```

</CodeGroup>

To change how long everything waits, set the default once (`sb.set_timeout(30).await?`)
or narrow it for one locator (`page.locator(..).with_timeout(Duration::from_secs(30))`).
A wait that runs out fails with `SeleniumBaseError::WaitTimeout`, which names the
selector and the state it waited for. [Waits and Assertions](/tutorials/waits_assertions)
lists every wait.

### 5. Assert with the retrying assertions

An assertion waits until it holds or the timeout ends, then fails with a message
that names the selector. Reading a value once and comparing it does neither, so
it fails on a page that is merely a moment late. The text assertions take the
text first and the selector second. End every call with `?`: a result that is
ignored hides the failure.

<CodeGroup>

```rust Do
sb.assert_text("Welcome", "h1").await?;
sb.assert_exact_text("Saved", "#status").await?;
sb.assert_element("#receipt").await?;
// pure CDP
page.locator("#status").expect().to_have_text("Saved").await?;
```

```rust Avoid
let status = sb.get_text("#status").await?; // reads once, at whatever moment it runs
assert_eq!(status, "Saved");
```

</CodeGroup>

When a page has many independent things to check, `deferred_assert_element` and
`deferred_assert_text` record each miss and `process_deferred_asserts` reports
them all at the end.

### 6. Scope the query and know how many elements match

A selector string or a `Locator` acts on the **first** match; there is no strict
mode that fails on two. So reach the element through its container, and when the
test depends on there being exactly one, say so.

<CodeGroup>

```rust Do (WebDriver)
// The container is part of the selector.
sb.click(r#"#cart [data-sku="B-7"] .remove"#).await?;

// Inside an iframe: the closure runs in the frame, and the case comes back
// out of it when the closure ends, also when it fails.
sb.frame_switch("#payment")
    .run(async |sb| sb.click("button.pay").await)
    .await?;

// Inside an open shadow root.
sb.shadow_click("my-app ::shadow button").await?;
```

```rust Do (pure CDP)
// Narrow step by step; a frame is just a nested locator.
let cart = page.locator("#cart");
cart.locator(r#"[data-sku="B-7"] .remove"#).click().await?;
page.locator("#payment").locator("button.pay").click().await?;

// When exactly one must match.
page.locator("#checkout").expect().to_have_count(1).await?;
```

```rust Avoid
sb.click(".remove").await?; // the first .remove on the page, whichever row it is
```

</CodeGroup>

`.visible()`, `.nth(i)`, `.first()` and `.last()` narrow a `Locator` further.

### 7. Find again instead of holding an element

Every `BaseCase` call that takes a selector finds the element again, and a
`Locator` looks again on every use, so both survive a re-render. A `WebElement`
from `find_element` is a handle to one node: once the page replaces that node,
the handle is stale.

<CodeGroup>

```rust Do
sb.click("#refresh").await?;
sb.click(".row .edit").await?; // finds the row again, after the table was rebuilt
```

```rust Avoid
let row = sb.find_element(".row .edit").await?; // a handle to one node
sb.click("#refresh").await?; // rebuilds the table
row.click().await?; // the node is gone: a stale element
```

</CodeGroup>

Use `find_element` for a short, immediate read of an element handle, not as the
way to keep a reference through a test.

### 8. Make cleanup run on every path

Inside `sb.run` and `run_browser_test` there is nothing to clean up, and no `quit`
to call: both close the browser after success, after an early `?` and after a
panic. If you launch a browser yourself, because a helper needs the `Browser`,
close it before you return the outcome, so a failed step cannot skip the close.

<CodeGroup>

```rust Do
let browser = Browser::launch(options).await?;
let page = browser.default_page().await?;
let outcome = steps(&page).await; // an async fn with the test's steps
browser.close().await?; // whether the steps passed or failed
outcome
```

```rust Do (pool)
let lease = pool.acquire().await?;
let outcome = steps(lease.page()).await;
lease.release().await; // frees the slot at once
outcome
```

```rust Avoid
let page = browser.default_page().await?;
steps(&page).await?; // an error returns here
browser.close().await?; // and this never runs
```

</CodeGroup>

Do not end a test with `std::process::exit`, and do not drive a browser from a
synchronous test: `Drop` cannot wait for the browser to close.

### 9. Keep the failure evidence bounded

The fixture writes the evidence of a failed attempt into one folder per test,
while the browser is still open:

```text
latest_logs/cart.shows_the_total/
  screenshot.png          (not with SB_NO_SCREENSHOT)
  page_source.html
  basic_test_info.txt     test id, last page, duration, browser, time, error
```

That is all the fixture writes for a failure, so a failure message does not need
the page in it. Add a point of interest with `sb.save_screenshot_to_logs()` and
`sb.save_page_source_to_logs()`; each returns the path it wrote. Do not print the
page source, or put it in an assertion message. A HAR keeps no response bodies
unless you ask, and then each body has a limit:

<CodeGroup>

```rust Do
sb.save_screenshot_to_logs().await?;
sb.save_page_source_to_logs().await?;

// pure CDP: bodies off by default; when you want them, cap each one
let recorder = page
    .start_har_with(HarOptions::new().content(true).max_content_bytes(256 * 1024))
    .await?;
```

```rust Avoid
println!("{}", sb.get_page_source().await?); // thousands of lines in a CI log
```

</CodeGroup>

Without a limit, a test that hangs holds the run until CI kills it. Set a time
limit (`SB_TIME_LIMIT=120`, or `settings.test.time_limit = Some(120.0)` through
`sb.configure`): the fixture stops a test that runs longer and fails it. Upload
`latest_logs/` as a CI artifact and keep it out of version control.

### 10. One browser, context or lease per concurrent flow

Every fixture test launches its own browser, so tests that `cargo test` runs side
by side do not touch each other. Inside one test, do not run two flows on one
tab: they would navigate it in turn, and the cookies of one sign-in would be the
other's. Give each flow its own lease from a `BrowserPool`, which is an isolated
context and a tab, or its own `BrowserContext`.

<CodeGroup>

```rust Do
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, Page, PoolOptions};
use seleniumbase_rs::{tokio, Result};

async fn sign_in_as(page: &Page, user: &str) -> Result<()> {
    page.goto("https://shop.example.com/login").await?;
    page.locator("#username").fill(user).await?;
    page.locator("#log-in").click().await?;
    page.locator("#greeting").expect().to_contain_text(user).await
}

async fn flow(pool: &BrowserPool, user: &str) -> Result<()> {
    let lease = pool.acquire().await?; // its own context and tab
    let outcome = sign_in_as(lease.page(), user).await;
    lease.release().await;
    outcome
}

#[tokio::test]
async fn two_users_sign_in_at_once() -> Result<()> {
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(1).contexts_per_browser(2).build()?,
    );
    let (alice, bob) = tokio::join!(flow(&pool, "alice"), flow(&pool, "bob"));
    pool.close().await; // close first, then report either failure
    alice?;
    bob?;
    Ok(())
}
```

```rust Avoid
let page = browser.default_page().await?;
// two sign-ins on one tab
let (alice, bob) = tokio::join!(sign_in_as(&page, "alice"), sign_in_as(&page, "bob"));
```

</CodeGroup>

The pool never holds more than `max_browsers * contexts_per_browser` leases and
makes a waiting flow queue; see [Pure CDP](/tutorials/pure_cdp#many-workers-the-browser-pool).
`browser.new_context()` gives two sessions in one browser when a pool is more
than you need.

### 11. Run headless in CI, and set it outside the code

A server has no screen. The fixtures read the settings file (`sbase_config.toml`)
and the `SB_*` variables, so the choice belongs to the environment:

```bash
SB_HEADLESS=true cargo test
```

Do not write `.headless(false)` or `.with_headless(false)` into a committed
test: a headed browser needs a display, which a CI server does not have.
`BrowserConfig::default()` is already headless and does not read `SB_HEADLESS`;
the pure CDP engine, started from `LaunchOptions` by hand, is headless on Linux
and headed elsewhere until the options say otherwise. [Headless and headed browsers](/tutorials/headless) has the table.

### 12. Write a body that is safe to run twice

With `SB_RERUNS=2` the fixture calls the closure again after a failure, on a new
browser. So the closure must not move out of what it captures (the compiler
refuses one that does), and it must not depend on what an earlier attempt left
behind. Do not turn a red test green by swallowing its error, or by weakening or
deleting an assertion.

<CodeGroup>

```rust Do
let paths = vec!["/", "/cart", "/account"];
sb.run(async |sb| {
    for path in &paths {
        // borrows `paths`, so the closure can run again
        sb.open(&format!("https://shop.example.com{path}")).await?;
        sb.assert_element("body").await?;
    }
    Ok(())
})
.await
```

```rust Avoid (moves out of the closure)
let paths = vec!["/", "/cart", "/account"];
sb.run(async |sb| {
    for path in paths {
        // `paths` is consumed by the first attempt: this closure can run only once
        sb.open(&format!("https://shop.example.com{path}")).await?;
    }
    Ok(())
})
.await
```

```rust Avoid (swallows the error)
sb.run(async |sb| {
    let _ = sb.click("#create").await; // a failed click no longer fails the test
    Ok(())
})
.await
```

</CodeGroup>

A rerun prints `RERUN <test id> (attempt 2 of 3) after: <error>` to standard
error, so it is never silent. Use reruns to ride out a known flaky service, not
to hide a flaky test.

### 13. Keep raw protocol calls in one helper

`Page::execute`, `Browser::execute` and `BaseCase::execute_cdp_cmd` send a raw
DevTools command. They work on Chromium browsers only, and some protocol methods
are experimental. Put them in one helper module with a name that says what they
do, and gate the tests that need them behind a Cargo feature of the test crate or
an `#[ignore = "needs Chrome DevTools"]`, so the rest of the suite runs on any
engine. This crate has no WebDriver BiDi API.

<CodeGroup>

```rust Do
// tests/support/raw.rs: the only file that names a protocol method
use seleniumbase_rs::sb_cdp::Page;
use seleniumbase_rs::Result;

pub async fn clear_cache(page: &Page) -> Result<()> {
    page.execute("Network.clearBrowserCache", seleniumbase_rs::json::json!({}))
        .await
        .map(|_| ())
}
```

```rust Avoid
// a protocol method in the middle of a test, with no note of why
page.execute("Network.clearBrowserCache", seleniumbase_rs::json::json!({})).await?;
```

</CodeGroup>

When a helper should serve both engines, write it over `BrowserApi`, `ElementApi`
and `AssertionApi`, which `BaseCase` and `Page` both implement; see
[One helper for either engine](/tutorials/pure_cdp#one-helper-for-either-engine).

### 14. Test page-driving code without Chrome

For code of your own that drives a page (a helper, a retry, what it sends), do not
start Chrome. With the feature `test-util`, `Browser::new_mocked()` returns a
browser backed by a scripted `MockCtrl`, and `sb_cdp.run_with_browser(..)` runs a
fixture test on it. Keep the real browser for what only a browser can show.

```toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing", "test-util"] }
```

[Testing without Chrome](/tutorials/pure_cdp#testing-without-chrome) shows how to
script the answers.

### 15. Read credentials and data from the run

A password, token or account name is never a literal in a test file. The `Sb`
fixture reads them from the run: `sb.account()`, `sb.data()`, `sb.env()`,
`sb.var1()` to `sb.var3()` and `sb.variables()` (from `SB_ACCOUNT`, `SB_DATA`,
`SB_ENV`, `SB_VAR1` to `SB_VAR3` and `SB_VARIABLES`), and the pure CDP fixture
reads `sb_cdp.data()`. Anything else comes from the environment of the process.

<CodeGroup>

```rust Do
sb.run(async |sb| {
    sb.open("https://shop.example.com/login").await?;
    let account = sb.account().unwrap_or("demo").to_owned();
    sb.type_text("#user", &account).await?;
    sb.click("#log-in").await
})
.await
```

```rust Avoid
sb.type_text("#password", "hunter2").await?; // in the repository for good
```

</CodeGroup>

See [Test data](/tutorials/writing_tests#test-data) and
[MFA and TOTP codes](/tutorials/mfa_totp).

## What the crate cannot enforce yet

A rule is only as strong as what checks it. These are the places where this crate
leaves a rule to the author or the reviewer:

- **No strict single match.** A selector or a `Locator` that matches several
  elements acts on the first. Rule 6 uses `to_have_count(1)` for that; there is
  no `single()` that fails on two.
- **No scoped query on `BaseCase`.** There is no object that stands for a
  container. You scope with the selector string, a frame or a shadow root, or with
  a `WebElement`, which rule 7 warns about. The pure CDP `Locator` does scope.
- **The hard time limit is the fixtures'.** `SB_TIME_LIMIT` stops a fixture test.
  `run_browser_test` has no limit, and `BaseCase::set_time_limit` is checked only
  when the test opens a page or waits, so a test that does neither is not
  interrupted; wrap it in `tokio::time::timeout` for a hard stop.
- **Nothing refuses a sleep.** `BaseCase::sleep` exists, as in Python, and no
  lint or check flags it. Rule 4 is a review item.
- **No WebDriver BiDi.** The WebDriver engine speaks classic WebDriver and, on
  Chrome, raw CDP commands through `execute_cdp_cmd`; there is no BiDi session to
  opt into.

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="Writing Tests with the sb Fixture" icon="flask" href="/tutorials/writing_tests">

The fixtures, the run configuration, evidence and reports.

</Card>

<Card color="#199f43" title="Waits and Assertions" icon="clock" href="/tutorials/waits_assertions">

Every wait and assertion, and the default timeout.

</Card>

<Card color="#d47628" title="Selectors" icon="crosshairs" href="/tutorials/selectors">

The selector formats and how to choose a stable one.

</Card>

<Card color="#693acf" title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

Locators, expectations, contexts, the pool and the mock browser.

</Card>

<Card color="#d52c36" title="Headless and headed browsers" icon="eye-slash" href="/tutorials/headless">

What each engine does by default and how to switch.

</Card>

<Card color="#1ca1c7" title="Best practices" icon="list-check" href="/best-practices">

The shorter list of habits, with the safety and CI notes.

</Card>

</Columns>
