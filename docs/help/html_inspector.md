# HTML Inspector

The HTML Inspector scans a page for common accessibility and markup issues. It
is a fast static check that can catch missing attributes, invalid references,
and structural problems before they reach production.

## What you will learn

- How to run an inspection.
- Which checks are performed.
- How to use the inspector in CI.
- The scope and limitations of static analysis.

## Run an inspection

```rust
let inspection = sb.inspect_html().await?;
assert!(inspection.is_clean(), "{inspection:?}");
```

## Checks performed

- Missing document language or title.
- Missing viewport meta tag for responsive layout.
- Missing skip link to main content.
- Missing `alt` attributes on images.
- Links and buttons without a static accessible name.
- Generic link text such as "click here" or "read more".
- Nested interactive elements (for example, a button inside a link).
- Form controls without a label, `aria-label`, or valid
  `aria-labelledby` reference.
- Radio button groups not grouped in a `fieldset` with a `legend`.
- Duplicate element IDs.
- Invalid `aria-labelledby` or `aria-describedby` references.
- Empty headings or skipped heading levels after the first heading.
- Tables without header cells.
- Missing `main` landmark.

## Access results

```rust
let issues = inspection.issues;
for issue in issues {
    println!(
        "{:?} {} at {:?}: {}",
        issue.severity, issue.rule, issue.selector, issue.message
    );
}
```

## Use in CI

Fail a build when new markup issues appear:

```rust
if !inspection.is_clean() {
    std::process::exit(1);
}
```

## Why it matters

Clean HTML improves accessibility, SEO, and test reliability.

## Scope and limitations

The inspector uses an HTML5 parser and deterministic static rules. Stable rule
IDs and best-effort element selectors make findings suitable for CI allowlists.
It does not execute JavaScript, compute styles or contrast, test keyboard
behavior, or inspect the browser accessibility tree.

Use it as an early check alongside browser-based accessibility testing such as
axe-core and manual keyboard and assistive-technology review. A clean result is
not a claim of WCAG conformance.
