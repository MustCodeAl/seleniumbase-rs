---
title: "HTML Inspector"
sidebarTitle: "HTML inspector"
---

The HTML Inspector scans a page for common accessibility and markup issues. It
is a fast static check that can catch missing attributes, invalid references,
and structural problems before they reach production.

<Info>

**What you will learn**

- How to run an inspection.
- Which checks are performed.
- How to use the inspector in CI.
- The scope and limitations of static analysis.

</Info>

<Visibility for="agents">

Summary for AI exports: `BaseCase::inspect_html` inspects the current page source and `HtmlInspector::inspect(html)` inspects any HTML string without a browser. Each finding has a rule ID (such as `image-alt`), an error or warning severity and an optional selector. `is_clean()` is true only when there are no findings; `has_errors()` is true when at least one finding is an error. It is a static check and no claim of WCAG conformance.

</Visibility>

## Run an inspection

<Tabs>

<Tab title="From a page" icon="globe">

`BaseCase::inspect_html` takes the current page source and inspects it:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let inspection = sb.inspect_html().await?;
assert!(inspection.is_clean(), "{inspection:?}");
# Ok(())
# }
```

`BaseCase::assert_no_html_issues` does the same and fails with an
`AssertionFailed` error that lists every `[rule] message`.

</Tab>

<Tab title="From an HTML string" icon="code">

The inspector needs no browser. `HtmlInspector::inspect` takes any HTML string:

```rust
use seleniumbase_rs::api::html_inspector::HtmlInspector;

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let html = r#"<html lang="en"><head><title>T</title></head>
              <body><img src="logo.png"></body></html>"#;
let inspection = HtmlInspector::inspect(html)?;
assert!(inspection.issues.iter().any(|issue| issue.rule == "image-alt"));
# Ok(())
# }
```

</Tab>

</Tabs>

## Checks performed

Every finding has a stable rule ID and a severity. `is_clean()` is true only
when there are no findings at all, warnings included; `has_errors()` is true
when at least one finding has `InspectionSeverity::Error`.

| Rule ID | Severity | What it flags |
|---|---|---|
| `document-lang` | <Badge color="red">error</Badge> | The `html` element has no non-empty `lang`. |
| `document-title` | <Badge color="red">error</Badge> | The document has no non-empty `title`. |
| `meta-viewport` | <Badge color="orange">warning</Badge> | No viewport meta tag whose content contains `width=device-width`. |
| `skip-link` | <Badge color="orange">warning</Badge> | No link to `#maincontent` or `#content`. |
| `image-alt` | <Badge color="red">error</Badge> | An `img` without an `alt` attribute (use `alt=""` for decoration). |
| `interactive-name` | <Badge color="red">error</Badge> | A link (`a[href]`) or button without a static accessible name (text, `aria-label`, `title`, `alt`, or a valid `aria-labelledby`). |
| `link-text` | <Badge color="orange">warning</Badge> | A link with no visible text, or with generic text: "click here", "here", "read more", "more", "link" or "details". |
| `nested-interactive` | <Badge color="red">error</Badge> | A button inside a link. |
| `form-control-label` | <Badge color="red">error</Badge> | A form control without a `label`, wrapping `label`, `aria-label`, or valid `aria-labelledby` reference. |
| `radio-group-legend` | <Badge color="orange">warning</Badge> | A named radio group of two or more not inside a `fieldset` with a `legend`. |
| `duplicate-id` | <Badge color="red">error</Badge> | An ID used more than once. |
| `invalid-aria-reference` | <Badge color="red">error</Badge> | `aria-labelledby` or `aria-describedby` naming an ID that does not exist. |
| `empty-heading` | <Badge color="orange">warning</Badge> | A heading with no text. |
| `heading-order` | <Badge color="orange">warning</Badge> | A heading level that jumps by more than one. |
| `page-h1` | <Badge color="orange">warning</Badge> | No `h1`, or more than one. |
| `table-headers` | <Badge color="orange">warning</Badge> | A table with no `th` cell. |
| `main-landmark` | <Badge color="orange">warning</Badge> | No `main` element or `role="main"`. |

## Access results

```rust
# use seleniumbase_rs::api::html_inspector::InspectionResult;
# fn print_issues(inspection: InspectionResult) {
let issues = inspection.issues;
for issue in issues {
    println!(
        "{:?} {} at {:?}: {}",
        issue.severity, issue.rule, issue.selector, issue.message
    );
}
# }
```

`selector` is an `Option<String>`: a best-effort selector for the element, or
`None` for document-level findings.

## Use in CI

Fail a build when new markup issues appear:

```rust
# use seleniumbase_rs::api::html_inspector::InspectionResult;
# fn gate(inspection: InspectionResult) {
if !inspection.is_clean() {
    std::process::exit(1);
}
# }
```

<Tip>

To fail only on errors and let warnings through, test `inspection.has_errors()`
instead.

</Tip>

## Why it matters

Clean HTML improves accessibility, SEO, and test reliability.

## Scope and limitations

The inspector uses an HTML5 parser and deterministic static rules. Stable rule
IDs and best-effort element selectors make findings suitable for CI allowlists.
It does not execute JavaScript, compute styles or contrast, test keyboard
behavior, or inspect the browser accessibility tree.

<Warning>

Use it as an early check alongside browser-based accessibility testing such as
axe-core and manual keyboard and assistive-technology review. A clean result is
not a claim of WCAG conformance.

</Warning>
