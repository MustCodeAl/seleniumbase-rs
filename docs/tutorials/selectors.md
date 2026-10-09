# Selectors Guide

Selectors are the addresses your tests use to find elements on the page.
`seleniumbase-rs` accepts several formats and converts them into `thirtyfour::By`
locators automatically. Choosing the right selector is one of the most important
things you can do for test maintainability.

## What you will learn

- The selector formats supported by `BaseCase`.
- How to pierce open Shadow DOM trees.
- How to convert between CSS and XPath.
- Best practices for stable, readable selectors.

## Supported formats

| Prefix | Meaning | Example |
|--------|---------|---------|
| `#` | ID | `#username` |
| `.` | Class (first match) | `.btn-primary` |
| `/`, `//`, `./`, `(` | XPath | `//div[@class='hero']` |
| an XPath axis | XPath | `parent::div`, `following-sibling::td` |
| `link=`, `link_text=`, `text=` | Exact link text | `link=Log in` |
| `partial_link=`, `partial_link_text=`, `partial_text=` | Partial link text | `partial_text=Privacy` |
| `p_link=`, `p_link_text=`, `p_text=` | Partial link text (short forms) | `p_text=Privacy` |
| (none) | CSS selector | `input[name='email']` |

Most `BaseCase` methods accept these strings directly. The library inspects the
selector to decide which `By` variant to use, so the same method takes CSS,
XPath, or a prefixed link-text selector without you having to say which it is.

Any of the thirteen XPath axes may lead an expression, so `parent::div` and
`ancestor-or-self::form` are recognized as XPath. CSS pseudo-elements are not
confused with them, because a pseudo-element never begins with an axis name:
`div::before` and `::after` stay CSS.

If you need to force a particular strategy, construct the `Selector` variant
yourself instead of relying on detection:

```rust
use seleniumbase_rs::Selector;

// Detected automatically.
let by = Selector::auto_by("parent::div")?;

// Or stated explicitly.
let by = Selector::Css("div").to_by()?;
```

## Examples

```rust
// ID
sb.type_text("#username", "alice").await?;

// CSS attribute selector
sb.type_text("input[name='q']", "seleniumbase").await?;

// Exact link text
sb.click("link=Sign up").await?;

// Partial link text
sb.click("partial link=Privacy Policy").await?;

// XPath
sb.hover("//button[contains(text(), 'Menu')]").await?;
```

## Using `Selector` directly

If you prefer an explicit type, use the `Selector` enum or the `selector!` macro:

```rust
use seleniumbase_rs::{selector, Selector};

let s1 = selector!(css, "#submit");
let s2 = selector!(id, "username");
let s3 = selector!(link, "Home");
let s4 = selector!(partial_link, "Terms");
let s5 = selector!(xpath, "//button[text()='Go']");

assert_eq!(s1, Selector::Css("#submit"));
```

## Shadow DOM

Web Components often hide markup inside Shadow DOM trees. `seleniumbase-rs` can
pierce through **open** shadow roots with the `::shadow` combinator.

```rust
sb.shadow_click("my-app ::shadow button").await?;
sb.shadow_type("my-app ::shadow input", "hello").await?;
let text = sb.shadow_get_text("my-app ::shadow div.result").await?;
```

Each `::shadow` fragment moves the search context into the shadow root of the
preceding element. You can chain them for nested Web Components:

```rust
sb.shadow_click("app-shell ::shadow nav-menu ::shadow a[href='/settings']").await?;
```

### Limitations

- Only **open** shadow roots are accessible. Closed shadow roots cannot be
  pierced by WebDriver or CDP.
- CSS selectors inside a shadow root must match the local DOM; they do not leak
  out to the parent tree.
- For full control, fetch a shadow root as a `WebElement` and search inside it:

```rust
let root = sb.get_shadow_root("my-app").await?;
let button = root.find(thirtyfour::prelude::By::Css("button")).await?;
button.click().await?;
```

## XPath conversion

Some helpers accept either XPath or CSS. You can convert between them with the
selector utility:

```rust
use seleniumbase_rs::utils::selectors;

let xpath = selectors::css_to_xpath("div.content > p")?;
```

This is useful when you have a CSS selector but need an XPath for a tool or
assertion that only accepts XPath.

## Best practices

- **Prefer stable attributes**: `data-testid`, `name`, `id`, or ARIA roles are
  usually more stable than positional XPath or generated class names.
- **Avoid generated class names**: frameworks such as styled-components or CSS
  modules generate class names that change between builds.
- **Use link text for navigation**: `link=` and `partial link=` match the text
  users see, which makes tests readable and resilient to markup changes.
- **Keep selectors short**: a selector with many descendants is brittle. Add
  stable attributes to the application under test when needed.
- **Document unusual selectors**: if a selector is non-obvious, add a comment
  explaining what it targets.

## Selector troubleshooting

| Problem | Likely cause | Fix |
|---------|--------------|-----|
| `ElementNotFound` | Selector is wrong or element not yet rendered. | Check the selector; add an explicit wait. |
| `InvalidSelector` | Malformed XPath or CSS. | Validate syntax; escape quotes properly. |
| Stale element | DOM was replaced after the element was found. | Re-find the element before each action. |
| Shadow DOM not pierced | Shadow root is closed or selector format is wrong. | Verify the component uses open shadow DOM; use `::shadow`. |

## Related reading

- [Waits and Assertions](./waits_assertions.md) — how to wait for elements to be ready.
- [Shadow DOM](./shadow_dom.md) — deeper coverage of Web Components.
- [API Reference](./api_reference.md) — all methods that accept selectors.
