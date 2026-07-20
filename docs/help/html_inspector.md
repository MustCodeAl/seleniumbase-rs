# HTML Inspector

The HTML Inspector scans a page for common accessibility and markup issues.

## Run an inspection

```rust
let inspection = sb.inspect_html().await?;
assert!(inspection.is_clean(), "{inspection:?}");
```

## Checks performed

- Missing `alt` attributes on images.
- Empty links (`<a></a>` or `<a href="#"></a>`).
- Duplicate element IDs.
- Skipped heading levels.
- Missing landmark regions (`main`, `nav`, `header`, `footer`).

## Access results

```rust
let issues = inspection.issues;
for issue in issues {
    println!("{issue:?}");
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
