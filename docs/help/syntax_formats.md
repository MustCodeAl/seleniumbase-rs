---
title: "Syntax Formats"
sidebarTitle: "Syntax formats"
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

## The formats

<Tabs>

<Tab title="Direct API" icon="code">

The most flexible format. Use it for production tests and complex logic.

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.click("#my-button").await?;
let text = sb.get_text("h1").await?;
# let _ = text;
# Ok(())
# }
```

The [pure CDP engine](/tutorials/pure_cdp) has its own API, in which the
selector lives in a `Locator` instead of being passed to every call:

```rust,no_run
# async fn demo(page: &seleniumbase_rs::sb_cdp::Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.locator("#my-button").click().await?;
let text = page.locator("h1").text().await?;
# let _ = text;
# Ok(())
# }
```

</Tab>

<Tab title="CLI commands" icon="terminal">

Useful for quick checks, CI one-liners, and ad-hoc debugging. Every `sbrs`
command starts its own browser and closes it when it finishes, so the one-liners
that make sense are the ones that take a URL:

```bash Terminal
cargo run --bin sbrs -- open https://seleniumbase.io
cargo run --bin sbrs -- smoke https://seleniumbase.io --title-contains SeleniumBase
cargo run --bin sbrs -- deferred --url https://seleniumbase.io \
    --assert element=h1 --assert title=SeleniumBase
```

For a flow with several steps, use a scenario or a Rust test.

</Tab>

<Tab title="JSON scenario" icon="file-code">

Scenarios are portable and do not require recompiling a test binary. A scenario
is an object with a `name` and a list of `steps`; each step has an `action` and
the fields that action takes (`url`, `css`, `text`, `contains`, ...):

```json scenario.json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "click", "css": "#my-button"},
    {"action": "assert_text", "css": "h1", "contains": "SeleniumBase"}
  ]
}
```

Run the scenario:

```bash Terminal
cargo run --bin sbrs -- run-scenario --file scenario.json
```

The list of actions is in [Recorder CLI](/help/recorder_cli#run-a-json-scenario).
`sbrs record` writes a different file, an array of recorded actions, which you
replay with `sbrs replay`.

</Tab>

</Tabs>

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

<Card title="Getting started" icon="rocket" href="/tutorials/getting_started">

Your first test with `BaseCase`.

</Card>

<Card title="Selectors" icon="crosshairs" href="/tutorials/selectors">

CSS, XPath, link text and shadow DOM.

</Card>

<Card title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

The driverless engine and its `Locator` API.

</Card>

<Card title="Recorder CLI" icon="circle-dot" href="/help/recorder_cli">

The scenario file and the recorded action file.

</Card>

</Columns>
