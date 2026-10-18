---
title: "Migrate Python Tests"
---

Moving an existing Python SeleniumBase or Selenium WebDriver test suite to Rust
is rarely a pure translation exercise. Tests often contain dynamic expressions,
custom helpers, fixtures, and framework plugins that cannot be converted
automatically. The `sbrs import-python` command is designed to handle the
mechanical parts and surface the parts that need human review.

This page explains how to use the importer, what it can and cannot translate,
and how to validate the generated Rust.

<Visibility for="agents">

Summary for AI exports: `sbrs import-python FILE.py --output FILE.rs` converts a Python SeleniumBase or Selenium WebDriver test into a Rust test that uses `#[tokio::test]` and `run_browser_test`. Options: `--source` (`auto`, `selenium-base`, `selenium`), `--test-name` and `--output`. The output is a first draft: statements it cannot translate become `TODO` comments and diagnostics, and every statement of the file lands in one generated test. Review it, run `cargo fmt` and compile it before trusting it.

</Visibility>

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

```bash Terminal
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

```bash Terminal
cargo run --bin sbrs -- import-python tests/login_test.py \
  --source selenium \
  --test-name user_can_log_in \
  --output tests/login_test.rs
```

<ParamField query="--source" type="auto | selenium-base | selenium">

Force the source API interpretation.

</ParamField>

<ParamField query="--test-name" type="Rust identifier">

Name the generated test function. Any valid Rust identifier.

</ParamField>

<ParamField query="--output" type="file path">

Write generated Rust to a file instead of stdout.

</ParamField>

## What the importer converts

The importer reads one statement at a time and understands these. In all cases
the arguments must be string (or, for waits and `sleep`, number) literals.
Arguments are matched with Python's parameters, so `selector="#a"` and
`timeout=5` work as well as positional ones, and a left-out selector is `body`,
as in Python.

<Tabs>

<Tab title="SeleniumBase" icon="flask">

SeleniumBase, as `self.` or `sb.` calls:

- Navigation: `open`, `visit`, `get`, `refresh`, `refresh_page`, `go_back`,
  `go_forward`, `maximize_window`.
- Interactions: `click`, `click_link`, `click_link_text`,
  `click_partial_link_text`, `double_click`, `right_click`, `type`, `type_text`,
  `update_text` (which clear the field), `add_text` and `send_keys` (which add
  to it, as in Python), `enter_mfa_code`, `clear`, `submit`, `hover`,
  `check_if_unchecked`, `uncheck_if_checked`, `select_option_by_text`.
- Assertions: `assert_element`, `assert_element_visible`, `assert_text`,
  `assert_text_visible`, `assert_exact_text`, `assert_text_not_visible`,
  `assert_exact_text_not_visible`, `assert_title`, `assert_title_contains`,
  `assert_url`, `assert_url_contains`. The text assertions take the text first
  and the selector second in Rust too, so the arguments are copied as they
  are. A `timeout` argument is refused (the assertion waits for the default
  timeout), and so is a `by` that is not the CSS strategy.
- Soft assertions: `deferred_assert_element`, `deferred_assert_text`,
  `deferred_assert_exact_text` (and their `delayed_` names) and
  `process_deferred_asserts`.
- Reads: `get_text`, `get_attribute` (the value is bound to an unused variable).
- Waits: `wait_for_element_visible`, `wait_for_element_present`,
  `wait_for_text` and `wait_for_exact_text` (and their `_visible` names). The
  timeout is the `timeout=` argument and defaults to 10 seconds. And `sleep`.

</Tab>

<Tab title="Selenium WebDriver" icon="globe">

Selenium WebDriver, as `driver.` or `self.driver.` calls:

- Navigation: `driver.get(url)`, `maximize_window()`, `back()`, `forward()`,
  `refresh()`. `driver.quit()` and `driver.close()` are dropped, because the
  generated test closes the browser itself.
- Saved element variables assigned from `find_element` calls, and
  `driver.find_element(...)` used directly, followed by `click`, `clear`,
  `submit` or `send_keys` with a string literal (`send_keys` becomes
  `add_text`, since Selenium's does not clear the field); `.text` and
  `.get_attribute()`
  on CSS-based variables.
- Locators: `By.CSS_SELECTOR`, `By.ID`, `By.NAME`, `By.CLASS_NAME` (one class),
  `By.TAG_NAME`, `By.XPATH`, `By.LINK_TEXT`, `By.PARTIAL_LINK_TEXT`. Link-text
  locators only support `click`.
- Simple XPath expressions that can be safely approximated as CSS selectors.
- Title checks: `assert "text" in driver.title` and
  `assert driver.title == "text"`.

</Tab>

</Tabs>

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
        sb.assert_text("Welcome", "body").await?;
        Ok(())
    }).await
}
```

</CodeGroup>

## Validate the generated Rust

After conversion, run these steps before trusting the test:

<Steps>

<Step title="Resolve the TODOs" icon="list-check">

Read the file and resolve every `TODO` comment.

</Step>

<Step title="Format" icon="broom">

Run `cargo fmt` to normalize formatting.

</Step>

<Step title="Compile" icon="hammer">

Compile with `cargo check` or `cargo build`.

</Step>

<Step title="Run it safely" icon="shield">

Run the test against a safe environment first.

</Step>

<Step title="Review the diagnostics" icon="magnifying-glass">

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

<Step title="Start small" icon="seedling">

Pick a small, stable subset of tests to migrate first.

</Step>

<Step title="Generate drafts" icon="wand-magic-sparkles">

Use `sbrs import-python` to generate drafts.

</Step>

<Step title="Refactor" icon="recycle">

Refactor the generated code into helpers and page objects in Rust.

</Step>

<Step title="Run both suites" icon="code-compare">

Run both Python and Rust suites in parallel until confidence is high.

</Step>

<Step title="Migrate the rest" icon="forward">

Gradually migrate the remaining tests, prioritizing the most valuable ones.

</Step>

</Steps>

## Related reading

<Columns cols={3}>

<Card title="Writing Browser Tests" icon="flask" horizontal href="/rust-test-tooling">

How to structure Rust tests.

</Card>

<Card title="Selectors" icon="crosshairs" horizontal href="/tutorials/selectors">

Selector syntax differences.

</Card>

<Card title="Waits and Assertions" icon="clock" horizontal href="/tutorials/waits_assertions">

Equivalent Rust methods.

</Card>

</Columns>

