# Syntax Formats

SeleniumBase for Rust supports direct API calls, CLI commands, and JSON-based scenarios.

## Direct API

```rust
sb.click("#my-button").await?;
let text = sb.get_text("h1").await?;
```

## CLI commands

```bash
cargo run --bin sbase -- open https://seleniumbase.io
cargo run --bin sbase -- click --css "#my-button"
```

## JSON scenario

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
