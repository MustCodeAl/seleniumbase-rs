# Migrate Python Tests

Moving an existing Python SeleniumBase or Selenium WebDriver test suite to Rust
is rarely a pure translation exercise. Tests often contain dynamic expressions,
custom helpers, fixtures, and framework plugins that cannot be converted
automatically. The `sbase import-python` command is designed to handle the
mechanical parts and surface the parts that need human review.

This page explains how to use the importer, what it can and cannot translate,
and how to validate the generated Rust.

## When to use the importer

Use `sbase import-python` when:

- You have many simple, linear tests that mostly navigate, click, type, and
  assert.
- You want a starting point for a manual migration.
- You need to compare the original Python with generated Rust side by side.

Do not expect the importer to produce production-ready Rust for complex suites
without review. Treat its output as a first draft.

## Convert a file from the CLI

```bash
cargo run --bin sbase -- import-python tests/login_test.py --output tests/login_test.rs
```

The tool reads the Python file, detects whether it looks like SeleniumBase or
plain Selenium WebDriver, and writes a Rust test that uses `#[tokio::test]` and
`run_browser_test`.

Diagnostics are printed to standard error. If you omit `--output`, the generated
Rust is written to standard output so you can inspect it before saving.

## Override detection

The importer tries to detect the source API automatically, but you can override
it:

```bash
cargo run --bin sbase -- import-python tests/login_test.py \
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

The importer understands the most common SeleniumBase and Selenium statements:

- Navigation: `open`, `get`, `refresh`, `go_back`, `go_forward`.
- Interactions: `click`, `type`, `send_keys`, `clear`, `submit`, `hover`, `select_by_visible_text`.
- Assertions: `assert_element`, `assert_text`, `assert_exact_text`, `assert_title`.
- Locators: `By.ID`, `By.CSS_SELECTOR`, `By.XPATH`, `By.LINK_TEXT`, `By.PARTIAL_LINK_TEXT`.
- Saved element variables assigned from `find_element` calls.
- Simple XPath expressions that can be safely approximated as CSS selectors.
- Explicit `WebDriverWait` for visibility and presence.

## What requires manual review

The importer is conservative. Anything it does not understand is emitted as a
`TODO` comment or a diagnostic. Common cases that need manual work include:

- Dynamic expressions and string formatting for selectors or URLs.
- Control flow such as loops, conditionals, and try/except blocks.
- Complex XPath that has no CSS equivalent.
- Custom helpers and page-object classes.
- Pytest fixtures, parametrization, and plugins.
- File uploads, downloads, and non-standard API calls.

Always review the generated file before running it.

## Example conversion

Given this Python file:

```python
# tests/login_test.py
from seleniumbase import BaseCase

class LoginTest(BaseCase):
    def test_login(self):
        self.open("https://example.com/login")
        self.type("#username", "alice")
        self.type("#password", "secret")
        self.click("#submit")
        self.assert_text("Welcome", "body")
```

The importer produces something like:

```rust
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

#[tokio::test]
async fn login() -> Result<()> {
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("https://example.com/login").await?;
            sb.type_text("#username", "alice").await?;
            sb.type_text("#password", "secret").await?;
            sb.click("#submit").await?;
            sb.assert_text("body", "Welcome").await
        })
    })
    .await
}
```

## Validate the generated Rust

After conversion, run these steps before trusting the test:

1. Read the file and resolve every `TODO` comment.
2. Run `cargo fmt` to normalize formatting.
3. Compile with `cargo check` or `cargo build`.
4. Run the test against a safe environment first.
5. Review diagnostics printed by the importer.

## Programmatic API

The same importer is available from Rust code through `import_python`,
`ImportOptions`, and `PythonSource`:

```rust
use seleniumbase_rs::{
    import_python, ImportOptions, ImportResult, PythonSource,
};

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
```

## Migration strategy

For large suites, a full automated migration is usually not practical. A more
realistic approach is:

1. Pick a small, stable subset of tests to migrate first.
2. Use `sbase import-python` to generate drafts.
3. Refactor the generated code into helpers and page objects in Rust.
4. Run both Python and Rust suites in parallel until confidence is high.
5. Gradually migrate the remaining tests, prioritizing the most valuable ones.

## Related reading

- [Writing Browser Tests](rust-test-tooling.md) — how to structure Rust tests.
- [Selectors](tutorials/selectors.md) — selector syntax differences.
- [Waits and Assertions](tutorials/waits_assertions.md) — equivalent Rust methods.

