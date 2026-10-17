---
title: "Migrate Python Tests"
---

# Migrate Python Tests

Moving an existing Python SeleniumBase or Selenium WebDriver test suite to Rust
is rarely a pure translation exercise. Tests often contain dynamic expressions,
custom helpers, fixtures, and framework plugins that cannot be converted
automatically. The `sbrs import-python` command is designed to handle the
mechanical parts and surface the parts that need human review.

This page explains how to use the importer, what it can and cannot translate,
and how to validate the generated Rust.

## When to use the importer

Use `sbrs import-python` when:

- You have many simple, linear tests that mostly navigate, click, type, and
  assert.
- You want a starting point for a manual migration.
- You need to compare the original Python with generated Rust side by side.

<Warning>

Do not expect the importer to produce production-ready Rust for complex suites
without review. Treat its output as a first draft.

</Warning>

## Convert a file from the CLI

```bash
cargo run --bin sbrs -- import-python tests/login_test.py --output tests/login_test.rs
```

The tool reads the Python file, detects whether it looks like SeleniumBase or
plain Selenium WebDriver, and writes a Rust test that uses `#[tokio::test]` and
`run_browser_test`.

Diagnostics are printed to standard error. If you omit `--output`, the generated
Rust is written to standard output so you can inspect it before saving. Unless
you pass `--test-name`, the test function is named after the Python file
(`login_test` here). The file is written even when there are problems, but the
command then exits with an error if any diagnostic has the `Error` severity.

The importer skips `import`, `from`, `def`, `class`, decorator and `with SB(`
lines and reads the statements that remain, so every statement in the file ends
up in one generated test, even when the Python file has several test methods.

<Note>

Split the test methods by hand: every statement of the file lands in one generated test.

</Note>

## Override detection

The importer tries to detect the source API automatically, but you can override
it:

```bash
cargo run --bin sbrs -- import-python tests/login_test.py \
  --source selenium \
  --test-name user_can_log_in \
  --output tests/login_test.rs
```

| CLI option | Values | Purpose |
|------------|--------|---------|
| `--source` | `auto`, `selenium-base`, `selenium` | Force the source API interpretation. |
| `--test-name` | any valid Rust identifier | Name the generated test function. |
| `--output` | file path | Write generated Rust to a file instead of stdout. |

## What the importer converts

The importer reads one statement at a time and understands these. In all cases
the arguments must be string (or, for waits and `sleep`, number) literals.

SeleniumBase, as `self.` or `sb.` calls:

- Navigation: `open`, `visit`, `get`, `refresh`, `refresh_page`, `go_back`,
  `go_forward`, `maximize_window`.
- Interactions: `click`, `click_link`, `click_link_text`,
  `click_partial_link_text`, `double_click`, `right_click`, `type`, `type_text`,
  `update_text`, `send_keys`, `clear`, `submit`, `hover`, `check_if_unchecked`,
  `uncheck_if_checked`, `select_option_by_text`.
- Assertions: `assert_element`, `assert_element_visible`, `assert_text`,
  `assert_exact_text`, `assert_title`, `assert_title_contains`, `assert_url`,
  `assert_url_contains`. For `assert_text` and `assert_exact_text` the importer
  swaps the arguments, because Python takes the text first and the Rust methods
  take the selector first.
- Reads: `get_text`, `get_attribute` (the value is bound to an unused variable).
- Waits: `wait_for_element_visible`, `wait_for_element_present` (the timeout
  defaults to 10 seconds), and `sleep`.

Selenium WebDriver, as `driver.` or `self.driver.` calls:

- Navigation: `driver.get(url)`, `maximize_window()`, `back()`, `forward()`,
  `refresh()`. `driver.quit()` and `driver.close()` are dropped, because the
  generated test closes the browser itself.
- Saved element variables assigned from `find_element` calls, and
  `driver.find_element(...)` used directly, followed by `click`, `clear`,
  `submit` or `send_keys` with a string literal; `.text` and `.get_attribute()`
  on CSS-based variables.
- Locators: `By.CSS_SELECTOR`, `By.ID`, `By.NAME`, `By.CLASS_NAME` (one class),
  `By.TAG_NAME`, `By.XPATH`, `By.LINK_TEXT`, `By.PARTIAL_LINK_TEXT`. Link-text
  locators only support `click`.
- Simple XPath expressions that can be safely approximated as CSS selectors.
- Title checks: `assert "text" in driver.title` and
  `assert driver.title == "text"`.

## What requires manual review

The importer is conservative. Anything it does not understand is emitted as a
`TODO` comment or a diagnostic. Common cases that need manual work include:

- Dynamic expressions and string formatting for selectors or URLs.
- Control flow such as loops, conditionals, and try/except blocks.
- Complex XPath that has no CSS equivalent.
- Custom helpers and page-object classes.
- Pytest fixtures, parametrization, and plugins.
- File uploads, downloads, and non-standard API calls.
- Selenium `WebDriverWait`, `Select(...).select_by_visible_text`, `ActionChains`
  and keyboard chords for `send_keys`.

A statement the importer does not recognise becomes a `TODO` comment and a
`Warning` diagnostic. One it recognises but cannot translate safely (a
non-literal argument, an XPath with no CSS equivalent, an unknown `By` strategy)
becomes a `TODO` comment and an `Error` diagnostic, which makes the CLI exit
with an error.

<Danger>

Always review the generated file before running it.

</Danger>

## Example conversion

Given this Python file, the importer produces the Rust file next to it (the layout
is rustfmt-able but not formatted):

<CodeGroup>

```python tests/login_test.py
from seleniumbase import BaseCase

class LoginTest(BaseCase):
    def test_login(self):
        self.open("https://example.com/login")
        self.type("#username", "alice")
        self.type("#password", "secret")
        self.click("#submit")
        self.assert_text("Welcome", "body")
```

```rust tests/login_test.rs
use seleniumbase_rs::{BrowserConfig, run_browser_test};

#[tokio::test]
async fn login_test() -> seleniumbase_rs::Result<()> {
    run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open("https://example.com/login").await?;
        sb.type_text("#username", "alice").await?;
        sb.type_text("#password", "secret").await?;
        sb.click("#submit").await?;
        sb.assert_text("body", "Welcome").await?;
        Ok(())
    }).await
}
```

</CodeGroup>

## Validate the generated Rust

After conversion, run these steps before trusting the test:

<Steps>

<Step title="Resolve the TODOs">

Read the file and resolve every `TODO` comment.

</Step>

<Step title="Format">

Run `cargo fmt` to normalize formatting.

</Step>

<Step title="Compile">

Compile with `cargo check` or `cargo build`.

</Step>

<Step title="Run it safely">

Run the test against a safe environment first.

</Step>

<Step title="Review the diagnostics">

Review diagnostics printed by the importer.

</Step>

</Steps>

## Programmatic API

The same importer is available from Rust code through `import_python`,
`ImportOptions`, and `PythonSource`:

```rust,no_run
use seleniumbase_rs::{
    import_python, ImportOptions, ImportResult, PythonSource,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let python = std::fs::read_to_string("tests/login_test.py")?;
let result: ImportResult = import_python(
    &python,
    &ImportOptions {
        source: PythonSource::Auto,
        test_name: "login".to_owned(),
    },
);

println!("{}", result.rust);
for diagnostic in &result.diagnostics {
    eprintln!("{:?} at line {}: {}", diagnostic.severity, diagnostic.line, diagnostic.message);
}
if !result.is_complete() {
    eprintln!("error-level diagnostics: review the TODO comments");
}
# Ok(())
# }
```

`result.detected_source` says which API family the importer assumed, and
`result.is_complete()` is true when no diagnostic has the `Error` severity.

## Migration strategy

For large suites, a full automated migration is usually not practical. A more
realistic approach is:

<Steps>

<Step title="Start small">

Pick a small, stable subset of tests to migrate first.

</Step>

<Step title="Generate drafts">

Use `sbrs import-python` to generate drafts.

</Step>

<Step title="Refactor">

Refactor the generated code into helpers and page objects in Rust.

</Step>

<Step title="Run both suites">

Run both Python and Rust suites in parallel until confidence is high.

</Step>

<Step title="Migrate the rest">

Gradually migrate the remaining tests, prioritizing the most valuable ones.

</Step>

</Steps>

## Related reading

- [Writing Browser Tests](rust-test-tooling.md) — how to structure Rust tests.
- [Selectors](tutorials/selectors.md) — selector syntax differences.
- [Waits and Assertions](tutorials/waits_assertions.md) — equivalent Rust methods.

