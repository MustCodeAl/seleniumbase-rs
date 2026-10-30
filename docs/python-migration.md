---
title: "Migrate Python Tests"
sidebarTitle: "From Python"
---

Moving an existing Python SeleniumBase or Selenium WebDriver test suite to Rust
is rarely a pure translation exercise. Tests often contain dynamic expressions,
custom helpers, fixtures, and framework plugins that cannot be converted
automatically. The `sbrs import-python` command is designed to handle the
mechanical parts and surface the parts that need human review.

This page explains how to use the importer, what it can and cannot translate,
and how to validate the generated Rust.

<Visibility for="agents">

Summary for AI exports: `sbrs import-python FILE.py --output FILE.rs` converts a Python SeleniumBase or Selenium WebDriver test into Rust tests in the form `sbrs mkfile` writes: `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { .. }).await }` (feature `testing`). Each `def test_*` method, `test_*` function, `with SB(...)` block or script is its own Rust test; `@parameterized.expand` and `@pytest.mark.parametrize` become `#[case]`s, `setUp` and `tearDown` run at the start and end of each test. A script that uses `sb_cdp.Chrome()`, `sb.cdp.*` or `activate_cdp_mode()` becomes a pure CDP test (`sb_cdp: SbCdp`, `Page` and `Locator` calls). Python variables, f-strings, `%` formatting, `for`, `if`/`elif`/`else`, `while`, `try`/`except` and `with self.frame_switch(..)` are translated; a text assertion without a selector looks at `html`, as in Python. Options: `--source` (`auto`, `selenium-base`, `selenium`), `--test-name` and `--output`. The output is a first draft: what it cannot translate becomes a `TODO` comment and a diagnostic. Review it, run `cargo fmt` and compile it before trusting it.

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
plain Selenium WebDriver, and writes Rust tests in the form `sbrs mkfile` writes
by default: an `#[rstest]` test that takes the `sb` fixture and runs its steps in
`sb.run(async |sb| { .. })`. The fixture reads the run configuration
(`SB_HEADLESS`, the settings file, reruns, the evidence of a failure), so a
generated test opens no browser of its own and runs headless when the settings
say so. The module docstring of the Python file becomes the module comment;
`BaseCase.main(__name__, __file__)` and `if __name__ == "__main__":` are dropped,
because `cargo test` runs the tests.

Diagnostics are printed to standard error. If you omit `--output`, the generated
Rust is written to standard output so you can inspect it before saving. The file
is written even when there are problems, but the command then exits with an
error if any diagnostic has the `Error` severity.

A Python file with several tests becomes several Rust tests: each `def test_*`
method of a class, each `test_*` function, each `with SB(...) as sb:` block and
a loose script is one function, named after the Python test. A file with one
test gets the name you pass with `--test-name`, or the Python file's name
(`login_test` here). `setUp` runs at the start of each test of its class and
`tearDown` at the end (after the steps, so not when a step fails).
`@parameterized.expand([...])` and `@pytest.mark.parametrize` become one
`#[case(..)]` per row, with the parameters as `#[case]` arguments.

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

The SeleniumBase reading parses the file as Python and translates the
statements of each test. Arguments are matched with Python's parameters, so
`selector="#a"` and `timeout=5` work as well as positional ones, and a text
assertion without a selector looks at `html`, as in Python.

<Tabs>

<Tab title="SeleniumBase" icon="flask">

SeleniumBase, as `self.` or `sb.` calls. Every `implemented` row of
`parity/api.toml` whose Rust call takes strings, numbers and booleans is
mapped (about 330 methods), among them:

- Navigation and windows: `open`, `visit`, `goto`, `get`, `refresh`, `go_back`,
  `go_forward`, `maximize_window`, `open_new_window`, `switch_to_window(0)`,
  `get_new_driver`.
- Frames: `switch_to_frame`, `switch_to_default_content`,
  `switch_to_parent_frame`, `switch_to_frame_of_element`, `set_content_to_frame`
  and `with self.frame_switch(..):`, which becomes
  `sb.frame_switch(..).run(async |sb| { .. }).await?`, so the frame is left
  again when the block ends.
- Interactions: `click`, `js_click`, `click_link`, `double_click`,
  `right_click`, `hover`, `hover_and_click`, `hover_and_js_click`,
  `drag_and_drop`, `type` and `update_text` (which clear the field), `add_text`
  and `send_keys` (which add to it, as in Python), `set_value`,
  `select_option_by_text`, `select_option_by_value`, `select_option_by_index`,
  `check_if_unchecked`, `uncheck_if_checked`, `click_visible_elements`,
  `highlight`, `highlight_elements(.., loops=1, limit=3)`, `post_message`,
  `activate_demo_mode`, `clear_local_storage` and `skip`.
- Assertions: `assert_element`, `assert_text`, `assert_exact_text`,
  `assert_text_not_visible`, `assert_title`, `assert_url`, `assert_link_text`,
  `assert_attribute`, `assert_elements(*selectors)`, `assert_true`,
  `assert_false`, `assert_equal`, `assert_in` and the deferred assertions. A
  `timeout` argument of an assertion is refused (the assertion waits for the
  default timeout), and so is a `by` that is not the CSS strategy.
- Reads, whose results can be used: `get_text`, `get_attribute`, `get_title`,
  `get_select_options(selector, attribute="value")`, `is_selected`,
  `is_element_visible`, `is_text_visible`, `is_alert_present` and
  `switch_to_alert()` with `alert.text`.
- Waits: `wait_for_element(selector, timeout=3)` (the timeout becomes
  `.timeout(Duration::from_secs(3))`), `wait_for_text` and the other waits with
  their timeout (10 seconds when left out), and `sleep`.

Values and statements are translated too:

- A variable is a `let`, and `x = ...` in an inner block assigns the outer
  variable (which becomes `mut`). Strings, f-strings, `%` formatting and `+`
  of strings become `format!`; lists of strings, `len`, `str`, `.lower()`,
  `.upper()`, `.strip()`, `.replace()`, `.startswith()`, `.endswith()`, `in`,
  `not in`, comparisons, `and`, `or` and `not` are carried over. A call that
  Rust awaits and Python uses as a value (`assert_true(self.is_selected(x))`)
  is first written to a `let`.
- `for x in items`, `for i, x in enumerate(items)` and `for i in range(n)`;
  `if`, `elif`, `else`; `while`; `assert`; `print` as `println!`;
  `time.sleep`; and `try`/`except`: the body runs in an async block and the
  first `except` body when it failed (the exception type is not looked at).
- A condition the importer cannot translate (`self.headed`, `self.browser`)
  keeps its body under `if false`, with a `TODO`, so the body does not run until
  you decide.

A script that drives CDP Mode (`sb = sb_cdp.Chrome()`, `with SB(uc=True) as sb:`
with `sb.activate_cdp_mode()`, or `sb.cdp.*` calls) becomes a pure CDP test:
`sb_cdp: SbCdp` and `Page` and `Locator` calls (`page.locator("#a").click()`,
`page.locator("h1").expect().to_contain_text(..)`,
`page.locator(a).drag_to(&page.locator(b))`), with the `SelectBy`, `State` and
`Scroll` imports it needs. `sb.quit()` is dropped, `open_new_tab` and
`switch_to_tab(n)` rebind `page`, and `find_element(..).query_selector_all(..)`
becomes `locator(..).all_waiting()`. The options of `SB(uc=True, ...)` are kept in
a comment: set the matching settings (`sbase_config.toml` or `SB_*`).

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

- Helper methods and functions, page-object classes and class attributes
  (the helper is listed at the top of the file as a `TODO`, and a call of it is
  a warning).
- Conditions on the run (`self.headed`, `self.headless`, `self.browser`,
  `self.recorder_mode`) and on the driver (`self.driver`), which have no Rust
  equal.
- Python modules and objects (`os`, `json`, `requests`), dicts, comprehensions,
  lambdas and slices.
- Pytest fixtures other than `sb`, plugins, and decorators other than
  `parameterized.expand` and `pytest.mark.parametrize`.
- Arguments Rust has no equal for: a `timeout` of an assertion, `browser=True` of
  the download checks, the `scroll` and `delay` of a click.
- The GUI calls of CDP Mode (`gui_drag_drop_points`), `save_data_to_logs` and
  `switch_to_frame` on a pure CDP page.
- Selenium `WebDriverWait`, `Select(...).select_by_visible_text`, `ActionChains`
  and keyboard chords for `send_keys`.

A statement the importer does not recognise (a method it has no mapping for, an
expression it does not translate, a statement it cannot parse) becomes a `TODO`
comment and a `Warning` diagnostic. A call it recognises but cannot write as the
Python wrote it (an argument Rust has no equal for, a value of the wrong kind,
a name that is not defined, an unknown `By` strategy) becomes a `TODO` comment
and an `Error` diagnostic, which makes the CLI exit with an error. A name whose
assignment was not converted is a warning where it is used, not a second error.
A `tearDown` runs after the steps of the test, so it does not run when a step
fails; use a fixture for cleanup that must always happen.

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
        user = "alice"
        self.open("https://example.com/login")
        self.type("#username", user)
        self.type("#password", "secret")
        self.click("#submit")
        self.assert_text(f"Welcome, {user}")
```

```rust tests/login_test.rs
//! Converted from Python by `sbrs import-python`. Run it with `cargo test --test login_test`
//! when this file is in your crate's `tests/` folder; `SB_HEADLESS=true` runs it
//! without a window. Search for `TODO` to find what was not converted.

use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn login_test(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        let user = "alice";
        sb.open("https://example.com/login").await?;
        sb.type_text("#username", user).await?;
        sb.type_text("#password", "secret").await?;
        sb.click("#submit").await?;
        sb.assert_text(&format!("Welcome, {}", user), "html").await?;
        Ok(())
    })
    .await
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

<Card color="#1a85d4" title="Writing Browser Tests" icon="flask" horizontal href="/rust-test-tooling">

How to structure Rust tests.

</Card>

<Card color="#199f43" title="Selectors" icon="crosshairs" horizontal href="/tutorials/selectors">

Selector syntax differences.

</Card>

<Card color="#d47628" title="Waits and Assertions" icon="clock" horizontal href="/tutorials/waits_assertions">

Equivalent Rust methods.

</Card>

</Columns>

