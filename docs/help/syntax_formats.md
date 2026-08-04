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

```rust
sb.click("#my-button").await?;
let text = sb.get_text("h1").await?;
```

## CLI commands

Useful for quick checks, CI one-liners, and ad-hoc debugging.

```bash
cargo run --bin sbase -- open https://seleniumbase.io
cargo run --bin sbase -- click --css "#my-button"
```

## JSON scenario

Scenarios are portable and do not require recompiling a test binary.

```json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "click", "target": "#my-button"},
    {"action": "assert_text", "target": "h1", "text": "SeleniumBase"}
  ]
}
```

Run the scenario:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json
```

## Selector syntax

SeleniumBase accepts multiple selector formats. See the [Selectors Guide](../tutorials/selectors.md) for details.

| Format | Example |
|--------|---------|
| CSS | `#my-button` |
| XPath | `//button[@id='my-button']` |
| Link text | `link=Sign in` |
| Partial link text | `partial link=Privacy` |
| Shadow DOM | `my-app ::shadow button` |

## Choosing a format

| Use case | Recommended format |
|---|---|
| Production regression tests | Direct API |
| CI smoke checks | CLI or JSON scenarios |
| Prototyping / demos | Recorder → Rust or JSON |
| Non-developer authors | JSON scenarios |
