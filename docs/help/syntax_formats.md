# Syntax Formats

SeleniumBase for Rust supports three primary ways to express automation:
direct API calls in Rust, CLI commands, and JSON-based scenarios. This page
compares the formats and links to deeper guides.

## What you will learn

- How the same action looks in each format.
- How selectors are represented across formats.
- When to choose one format over another.

## Direct API

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

The [pure CDP engine](../tutorials/pure_cdp.md) has its own API, in which the
selector lives in a `Locator` instead of being passed to every call:

```rust,no_run
# async fn demo(page: &seleniumbase_rs::sb_cdp::Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.locator("#my-button").click().await?;
let text = page.locator("h1").text().await?;
# let _ = text;
# Ok(())
# }
```

## CLI commands

Useful for quick checks, CI one-liners, and ad-hoc debugging. Every `sbrs`
command starts its own browser and closes it when it finishes, so the one-liners
that make sense are the ones that take a URL:

```bash
cargo run --bin sbrs -- open https://seleniumbase.io
cargo run --bin sbrs -- smoke https://seleniumbase.io --title-contains SeleniumBase
cargo run --bin sbrs -- deferred --url https://seleniumbase.io \
    --assert element=h1 --assert title=SeleniumBase
```

For a flow with several steps, use a scenario or a Rust test.

## JSON scenario

Scenarios are portable and do not require recompiling a test binary. A scenario
is an object with a `name` and a list of `steps`; each step has an `action` and
the fields that action takes (`url`, `css`, `text`, `contains`, ...):

```json
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

```bash
cargo run --bin sbrs -- run-scenario --file scenario.json
```

The list of actions is in [Recorder CLI](./recorder_cli.md#run-a-json-scenario).
`sbrs record` writes a different file, an array of recorded actions, which you
replay with `sbrs replay`.

## Selector syntax

SeleniumBase accepts multiple selector formats. See the [Selectors Guide](../tutorials/selectors.md) for details.

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
