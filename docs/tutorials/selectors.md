---
title: "Selectors Guide"
sidebarTitle: "Selectors"
---

Selectors are the addresses your tests use to find elements on the page.
`seleniumbase-rs` accepts several formats and converts them into `thirtyfour::By`
locators automatically. Choosing the right selector is one of the most important
things you can do for test maintainability.

<Info>

**What you will learn**

- The selector formats supported by `BaseCase`.
- How to pierce open Shadow DOM trees.
- How to convert between CSS and XPath.
- Best practices for stable, readable selectors.

</Info>

<Visibility for="agents">

Summary for AI exports: `BaseCase` reads a selector string by its prefix. `#` is an ID and `.` a class, a string that starts with `/`, `//`, `./`, `../`, `(` or an XPath axis is XPath, `link=` and `partial_link=` (and their short forms) match link text, and anything else is CSS. Prefixes are matched exactly. `::shadow` pierces open shadow roots. `Selector::auto_by` detects the strategy and `Selector::Css(..)` states it.

</Visibility>

## Supported formats

| Prefix | Meaning | Example |
|--------|---------|---------|
| `#` | ID | `#username` |
| `.` | Class (first match) | `.btn-primary` |
| `/`, `//`, `./`, `../`, `(` | XPath | `//div[@class='hero']` |
| an <Tooltip tip="The direction an XPath step walks from the current element, as in parent:: or following-sibling::.">XPath axis</Tooltip> | XPath | `parent::div`, `following-sibling::td` |
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

<Warning>

The prefixes are matched exactly and everything after one is the text, so
`LINK=Home` (wrong case) and ` link=Home` (leading space) are CSS, and
`text=//div` is a link whose text is `//div`. An attribute selector such as
`[name=q]` or `input[name='q']` is CSS: there is no `name=` prefix.

</Warning>

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
sb.click("partial_link=Privacy Policy").await?;

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

<Warning>

- Only **open** shadow roots are accessible. Closed shadow roots cannot be
  pierced by WebDriver or CDP.
- CSS selectors inside a shadow root must match the local DOM; they do not leak
  out to the parent tree.

</Warning>

For full control, fetch a shadow root as a `WebElement` and search inside it:

```rust
let root = sb.get_shadow_root("my-app").await?;
let button = root.find(thirtyfour::prelude::By::Css("button")).await?;
button.click().await?;
```

## XPath to CSS

A simple XPath expression can be turned into the equivalent CSS selector:

```rust
use seleniumbase_rs::utils::selectors::xpath_to_css;

assert_eq!(xpath_to_css("//input[@type='text' and @name='q']")?, "input[type='text'][name='q']");
```

Only expressions with an exact CSS equivalent are converted: a tag (or `*`)
anywhere in the document, narrowed by attribute tests (`@id='x'`, `@href`)
joined with `and`. Anything else (`or`, `contains()`, `text()`, positions,
paths with several steps) is an `InvalidSelector` error, because a CSS selector
that quietly left part of the XPath out would match different elements. The
other direction is `BaseCase::convert_css_to_xpath`.

## Best practices

- **Prefer stable attributes**: `data-testid`, `name`, `id`, or ARIA roles are
  usually more stable than positional XPath or generated class names.
- **Avoid generated class names**: frameworks such as styled-components or CSS
  modules generate class names that change between builds.
- **Use link text for navigation**: `link=` and `partial_link=` match the text
  users see, which makes tests readable and resilient to markup changes.
- **Keep selectors short**: a selector with many descendants is brittle. Add
  stable attributes to the application under test when needed.
- **Document unusual selectors**: if a selector is non-obvious, add a comment
  explaining what it targets.

## Selector troubleshooting

<AccordionGroup>

<Accordion title="ElementNotFound" icon="magnifying-glass">

**Likely cause:** Selector is wrong or element not yet rendered.

**Fix:** Check the selector; add an explicit wait.

</Accordion>

<Accordion title="InvalidSelector" icon="triangle-exclamation">

**Likely cause:** Malformed XPath or CSS.

**Fix:** Validate syntax; escape quotes properly.

</Accordion>

<Accordion title="Stale element" icon="clock-rotate-left">

**Likely cause:** DOM was replaced after the element was found.

**Fix:** Re-find the element before each action.

</Accordion>

<Accordion title="Shadow DOM not pierced" icon="layer-group">

**Likely cause:** Shadow root is closed or selector format is wrong.

**Fix:** Verify the component uses open shadow DOM; use `::shadow`.

</Accordion>

</AccordionGroup>

## Related reading

<Columns cols={3}>

<Card color="#1a85d4" title="Waits and Assertions" icon="clock" horizontal href="/tutorials/waits_assertions">

How to wait for elements to be ready.

</Card>

<Card color="#199f43" title="Shadow DOM" icon="layer-group" horizontal href="/tutorials/shadow_dom">

Deeper coverage of Web Components.

</Card>

<Card color="#d47628" title="API Reference" icon="code" horizontal href="/tutorials/api_reference">

All methods that accept selectors.

</Card>

</Columns>
