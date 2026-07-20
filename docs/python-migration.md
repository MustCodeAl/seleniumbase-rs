# Migrate Python Tests

The `sbase import-python` command converts common SeleniumBase Python and
Selenium WebDriver Python statements into an idiomatic Tokio test.

## Convert a file

```bash
sbase import-python tests/login_test.py --output tests/login_test.rs
```

Detection is automatic. Override it when a file uses unusual imports:

```bash
sbase import-python tests/login_test.py \
  --source selenium-base \
  --test-name user_can_log_in \
  --output tests/login_test.rs
```

Diagnostics are printed to standard error. Generated Rust is written to
standard output unless `--output` is provided.

## What the importer converts

- Navigation, refresh, back, and forward.
- Click, type, clear, submit, hover, and select-by-text.
- Element, text, exact-text, and title assertions.
- Common Selenium `By` locators and saved element variables.
- Simple XPath expressions that can be represented safely as CSS.
- Explicit visibility and presence waits.

## Review required

The importer is a conservative migration aid, not a Python compiler. Dynamic
expressions, custom helpers, control flow, complex XPath, fixtures, and
framework plugins become source-located `TODO` comments and diagnostics.

Review the generated file, resolve every `TODO`, format it with `cargo fmt`, and
compile it before running against credentials or production-like data.

The same API is available from Rust through `import_python`, `ImportOptions`,
and `PythonSource`.

