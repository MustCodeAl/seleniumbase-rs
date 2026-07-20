# Selectors Guide

SeleniumBase for Rust accepts several selector formats. The library auto-detects the format and converts it to a `thirtyfour::By` locator.

## Supported formats

| Prefix | Meaning | Example |
|--------|---------|---------|
| `#` | ID | `#username` |
| `.` | Class (first match) | `.btn-primary` |
| `//` | XPath | `//div[@class='hero']` |
| `link=` | Exact link text | `link=Log in` |
| `partial link=` | Partial link text | `partial link=Privacy` |
| (none) | CSS selector | `input[name='email']` |

## Examples

```rust
sb.click("#submit").await?;
sb.type_text("input[name='q']", "seleniumbase").await?;
sb.click("link=Sign up").await?;
sb.hover("//button[contains(text(), 'Menu')]").await?;
```

## Shadow DOM

Append `::shadow` to CSS fragments to pierce through open Shadow DOM trees.

```rust
sb.shadow_click("my-app ::shadow button").await?;
sb.shadow_type("my-app ::shadow input", "hello").await?;
let text = sb.shadow_get_text("my-app ::shadow div.result").await?;
```

Each `::shadow` moves the search context into the matching element's shadow root.

## XPath conversion

Some APIs accept either XPath or CSS. Use the selector utility to convert a CSS selector to XPath when needed:

```rust
use seleniumbase_rs::utils::selectors;
let xpath = selectors::css_to_xpath("div.content > p")?;
```

## Best practices

- Prefer stable attributes such as `data-testid` or `name` over positional XPath.
- Avoid selectors that depend on generated class names.
- Use `link=` and `partial link=` for navigation links because they match user-facing text.
