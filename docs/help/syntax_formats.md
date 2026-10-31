---
title: "Syntax Formats"
sidebarTitle: "Syntax formats"
description: "One check written three ways in seleniumbase-rs: Rust with BaseCase or the pure CDP engine, sbrs command lines, and a JSON scenario file."
keywords: ["syntax", "formats", "basecase", "sbrs", "scenario", "selectors"]
---

SeleniumBase for Rust supports three primary ways to express automation:
direct API calls in Rust, CLI commands, and JSON-based scenarios. This page
compares the formats and links to deeper guides.

<Info>

**What you will learn**

- How the same action looks in each format.
- How selectors are represented across formats.
- When to choose one format over another.

</Info>

<Visibility for="agents">

Summary for AI exports: there are three formats. Direct API calls in Rust (`BaseCase`, or `Locator` on the pure CDP engine), `sbrs` CLI commands such as `open`, `smoke` and `deferred`, and JSON scenarios run with `sbrs run-scenario --file`. Use the direct API for production tests, the CLI or JSON scenarios for CI smoke checks, and JSON scenarios for non-developer authors.

</Visibility>

<RequestExample>

```rust BaseCase
use seleniumbase_rs::{run_browser_test, tokio, BrowserConfig};

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let config = BrowserConfig::from_env()?.with_headless(true);

    run_browser_test(config, async |sb| {
        sb.open("https://example.com").await?;
        let heading = sb.get_text("h1").await?;
        println!("{heading}");
        sb.assert_text("Example", "h1").await
    })
    .await
}
```

```rust Pure CDP
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;

#[tokio::main]
async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("https://example.com").await?;
    let heading = page.locator("h1").text().await?;
    println!("{heading}");
    page.locator("h1").expect().to_contain_text("Example").await?;

    browser.close().await?;
    Ok(())
}
```

```bash sbrs
sbrs open https://example.com
sbrs smoke https://example.com --title-contains Example
```

```bash sbrs deferred
sbrs --url https://example.com deferred \
    --assert element=h1 --assert text=h1:Example
```

```json scenario.json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://example.com"},
    {"action": "assert_text", "css": "h1", "contains": "Example"}
  ]
}
```

```bash run-scenario
sbrs run-scenario --file scenario.json
```

</RequestExample>

<ResponseExample>

```text BaseCase
Example Domain
```

```text Pure CDP
Example Domain
```

```text sbrs
Title: Example Domain
Assertion passed: title contains 'Example'
```

```text sbrs deferred
Deferred assertion summary
--------------------------
PASS  element exists: h1
PASS  text "Example" in h1
--------------------------
total=2 passed=2 failed=0
```

```text run-scenario
Dashboard written: <path>
Scenario 'basic_flow' steps: total=2, passed=2, failed=0
Recording json: <path>
Recording script: <path>
```

</ResponseExample>

Every tab checks the same thing: the page at `https://example.com` has an `h1`
that contains `Example`. The program tabs print the heading; the `sbrs` tabs
print what the command prints (the lines come from the command's code, and
`<path>` stands for a file under `latest_logs/`). A failed check ends a Rust
program with an error and `sbrs` with a non-zero exit status.

## The formats

### Direct API

The most flexible format. Use it for production tests and complex logic. With
`BaseCase`, the selector is an argument of every call (`sb.get_text("h1")`).
`run_browser_test` starts the browser, runs your closure and always quits the
browser, even when an assertion fails.

The [pure CDP engine](/tutorials/pure_cdp) has its own API, in which the
selector lives in a `Locator` instead of being passed to every call
(`page.locator("h1").text()`), and it needs no chromedriver.

### CLI commands

Useful for quick checks, CI one-liners, and ad-hoc debugging. Every `sbrs`
command starts its own browser and closes it when it finishes, so the one-liners
that make sense are the ones that take a URL: `open` prints the page title,
`smoke` asserts on it, and `deferred` runs several assertions and reports
every failure at once. The global `--url` flag names the page `deferred`
runs on.

For a flow with several steps, use a scenario or a Rust test.

### JSON scenario

Scenarios are portable and do not require recompiling a test binary. A scenario
is an object with a `name` and a list of `steps`; each step has an `action` and
the fields that action takes (`url`, `css`, `text`, `contains`, ...). The steps
run in order, a failed step is collected instead of stopping the run, and
`sbrs run-scenario` exits non-zero when any step failed.

The list of actions is in [Recorder CLI](/help/recorder_cli#run-a-json-scenario).
`sbrs mkrec` writes a different file, an array of recorded actions
(`recordings/NAME_rec.json`), which you replay with `sbrs replay`.

## Selector syntax

SeleniumBase accepts multiple selector formats. See the [Selectors Guide](/tutorials/selectors) for details.

| Format | Example |
|--------|---------|
| CSS | `#my-button` |
| XPath | `//button[@id='my-button']` |
| Link text | `link=Sign in` |
| Partial link text | `partial_link=Privacy` |
| Shadow DOM | `my-app ::shadow button` |

A selector beginning with `/`, `./` or `(` (or an XPath axis such as `parent::`)
is XPath, the `link=`, `link_text=` and `text=` prefixes mean link text, and the
`partial_link=`, `partial_link_text=` and `partial_text=` prefixes mean partial
link text; anything else is CSS.

## Choosing a format

| Use case | Recommended format |
|---|---|
| Production regression tests | Direct API |
| CI smoke checks | CLI or JSON scenarios |
| Prototyping / demos | Recorder → Rust or JSON |
| Non-developer authors | JSON scenarios |

## Next steps

<Columns cols={2}>

<Card color="#1a85d4" title="Working with seleniumbase-rs" icon="rocket" href="/tutorials/getting_started">

Your first test with `BaseCase`.

</Card>

<Card color="#199f43" title="Selectors" icon="crosshairs" href="/tutorials/selectors">

CSS, XPath, link text and shadow DOM.

</Card>

<Card color="#d47628" title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

The driverless engine and its `Locator` API.

</Card>

<Card color="#a631be" title="Recorder CLI" icon="circle-dot" href="/help/recorder_cli">

The scenario file and the recorded action file.

</Card>

</Columns>
