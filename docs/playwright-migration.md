---
title: "Migrate Playwright Tests"
sidebarTitle: "From Playwright"
---

`sbrs import-playwright` converts Playwright tests and scripts into Rust tests
for the [pure CDP engine](/tutorials/pure_cdp). It reads:

- `@playwright/test` files in JavaScript or TypeScript: `test()`,
  `test.describe()`, the `beforeEach`/`beforeAll`/`afterEach` hooks,
  `test.skip`/`test.only`/`test.fixme`, `test.step()`, several tests per file;
- `npx playwright codegen` output for the `javascript`, `playwright-test`,
  `python` and `python-pytest` targets, and the async Python API;
- pytest-playwright tests (`def test_x(page: Page)`) and plain sync or async
  Python scripts (`with sync_playwright() as p:`, `asyncio.run(main())`).

The output is a Rust file with one `#[tokio::test]` per Playwright test. Each
launches a browser, drives its page with `Page` and `Locator` calls, and closes
it, the way the recorder's `--rec-sb-cdp` output does. Anything the importer
does not convert stays in place as a `// TODO(playwright): <statement>` comment
and is reported with its line number, so nothing is dropped silently.

<Warning>

Treat the output as a first draft and review every `TODO`.

</Warning>

<Visibility for="agents">

Summary for AI exports: `sbrs import-playwright FILE --output tests/FILE.rs` converts Playwright tests and scripts (JavaScript, TypeScript and Python) into Rust tests for the pure CDP engine, with one `#[tokio::test]` per Playwright test. A statement it does not convert stays as a `// TODO(playwright): <statement>` comment and is reported with its line. Exit status: 0 when everything was read, 1 when a statement could not be read, 6 when the file cannot be read, 2 for a source it refuses.

</Visibility>

## Convert a file

```bash
cargo run --bin sbrs -- import-playwright e2e/login.spec.ts --output tests/login.rs
cargo run --bin sbrs -- import-pw tests/test_login.py   # prints to stdout
```

<ParamField body="--output, -o" type="file path">

Write the Rust there instead of to standard output.

</ParamField>

<ParamField body="--language" type="js | ts | python">

Override the language, which otherwise comes from the extension (`.js`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.py`) or the content.

</ParamField>

<ParamField body="--test-name" type="any text">

Name the test made from a script that has no test titles (codegen output); the default is the file name. Titled tests keep their titles.

</ParamField>

Diagnostics go to standard error, one line per `TODO`. The exit status is the
one `sbrs import-python` uses:

| Status | Meaning |
|---|---|
| 0 | Everything was read (gaps are warnings). |
| 1 | A statement could not be read at all. |
| 6 | The file cannot be read. |
| 2 | A source the importer refuses: larger than 2 MB, or a Chrome Recorder export, which is for `sbrs import-puppeteer`. |

From Rust, call `seleniumbase_rs::utilities::importer::import_playwright`;
`ScriptImport::is_complete()` is `false` when anything became a `TODO`.

## What a converted test looks like

A Playwright test becomes a Rust test:

<CodeGroup>

```typescript Playwright
test.describe('shop', () => {
  test.beforeEach(async ({ page }) => { await page.goto('https://shop.test/'); });
  test('orders', async ({ page }) => {
    await page.getByLabel('Email').fill('a@b.test');
    await page.getByRole('button', { name: 'Place order' }).click();
    await expect(page.locator('#status')).toHaveText('Ordered');
  });
});
```

```rust,ignore Rust
#[tokio::test]
async fn shop_orders() -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::launch(support::launch_options()?).await?;
    let page = browser.default_page().await?;
    page.goto("https://shop.test/").await?;
    page.locator(SelectorBuf::label("Email", false)).fill("a@b.test").await?;
    page.locator(SelectorBuf::role("button", Some("Place order"), false)).click().await?;
    page.locator("#status").expect().to_have_text("Ordered").await?;
    browser.close().await?;
    Ok(())
}
```

</CodeGroup>

Group titles become a prefix of the test name; two tests with the same name
get `_2`, `_3`. Hooks are inlined into every test of their group, in order. A
skipped test gets `#[ignore = "..."]`. The file ends with a `support` module:
`launch_options()`, which runs headless when `SB_HEADLESS` says so (as the rest
of this crate does), and the few helpers the tests use. The generated tests
need `seleniumbase-rs` and `tokio` with the `macros`, `rt-multi-thread` and
`time` features. Run them headless with `SB_HEADLESS=true cargo test`.

## The mappings

Python's snake_case names and keyword arguments map exactly like the
JavaScript ones (`get_by_role("button", name="Go")` is
`getByRole('button', { name: 'Go' })`, `not_to_be_visible()` is
`.not.toBeVisible()`, `locator.first` is `locator.first()`).

### The page

| Playwright | Rust |
|---|---|
| `goto(url)` | `page.goto(url)` (the `waitUntil` and `timeout` options are not carried over) |
| `reload()`, `goBack()`, `goForward()` | `page.reload()`, `page.back()`, `page.forward()` |
| `waitForURL(url)` | `page.expect().to_have_url(url)`; a glob with `**` at its ends becomes `to_contain_url` of the rest; a regular expression as below |
| `waitForLoadState(state?)` | `page.wait_for_load()` (also for `networkidle`, which it approximates with the load event) |
| `waitForTimeout(ms)` | `tokio::time::sleep(Duration::from_millis(ms))` |
| `waitForSelector(sel, { state })` | `page.locator(sel).wait_for(State::Visible)` (`attached` is `Present`, `detached` `Absent`, `hidden` `Hidden`) |
| `setViewportSize({ width, height })` | `page.window().set_bounds(0, 0, width, height)`: the window size, which in headless Chrome is the viewport |
| `screenshot({ path })` | `std::fs::write(path, page.screenshot().await?)` (not `fullPage`) |
| `title()`, `url()` (Python: `page.url`) | `page.title()`, `page.url()` |
| `evaluate(fn)` | `page.evaluate("(fn)()")`: a function with no parameters that uses nothing from the test is copied as JavaScript text; a string is used as it is (a Python string that is a function is called) |
| `keyboard.press(key)`, `keyboard.type(text)` | `page.keyboard().press(Key::Enter)` for named keys, `type_text` for one character; `page.keyboard().type_text(text)` |
| `mouse.click(x, y)` (literal numbers) | `page.mouse().click(Point::new(x, y))`; `{ button: 'right' }` is `right_click`, `{ clickCount: 2 }` `double_click` |
| `setDefaultTimeout(ms)` | `let page = page.with_timeout(...)` |

### Locators and selectors

| Playwright | Rust |
|---|---|
| `locator(css)`, `css=`, `xpath=`, `//...` | `page.locator("css")`, `page.locator("//xpath")` |
| `text=Foo` / `text="Foo"`, `"Foo"` | `SelectorBuf::text("Foo", false)` / `SelectorBuf::text("Foo", true)` |
| `id=x`, `data-testid=x` | `[id="x"]`, `[data-testid="x"]` |
| `css:has-text("x")`, `:text("x")` (at the end of a selector), `:visible` | `.locator(css).locator(SelectorBuf::has_text("x"))`, `.visible()` |
| `a >> b`, `nth=N`, `locator(a).locator(b)` | `page.locator(a).locator(b)`, `.nth(N)` (`nth=-1` is `.last()`) |
| `getByRole(role, { name, exact })` | `SelectorBuf::role(role, Some(name), exact)` (see below) |
| `getByText(text, { exact })` | `SelectorBuf::text(text, exact)` |
| `getByLabel(text, { exact })` | `SelectorBuf::label(text, exact)` |
| `getByPlaceholder`, `getByAltText`, `getByTitle` | `[placeholder*="text" i]`, `[alt*=...]`, `[title*=...]` (`exact` makes it `[attr="text"]`) |
| `getByTestId(id)` | `[data-testid="id"]` |
| `.first()`, `.last()`, `.nth(i)` | `.first()`, `.last()`, `.nth(i)` |
| `.filter({ hasText })` | `.locator(SelectorBuf::has_text(text))` |
| `frameLocator(css).locator(css)` | `page.locator(frame).locator(css)`: the engine searches inside same-origin frames |

The text matches follow Playwright: whitespace collapsed, and without `exact`
a substring match that ignores case (ASCII letters only, since it is done in
XPath 1.0).

### Actions

| Playwright | Rust |
|---|---|
| `click()`, `dblclick()`, `click({ button: 'right' })` | `.click()`, `.double_click()`, `.right_click()` |
| `fill(text)` | `.fill(text)` |
| `type(text)`, `pressSequentially(text)` | `.send_keys(text)` types after what the field holds, as these do; `.type_text(text)` empties it first. The `delay` option is not kept |
| `press(key)` | `.press(Key::Enter)` for named keys, `.send_keys("a")` for one character; key combinations are a `TODO` |
| `check()`, `uncheck()`, `setChecked(b)` | `.check()`, `.uncheck()`, `.set_checked(b)` |
| `selectOption('v')` | `support::select_option(&locator, "v")`: the option with that value, else that text |
| `selectOption({ value })`, `{ label }`, `{ index }` | `.select_option(SelectBy::Value / Text / Index)` |
| `hover()`, `focus()`, `clear()` | `.hover()`, `.focus()`, `.clear()` |
| `blur()` | blurs the focused element through `evaluate` |
| `setInputFiles('path')` (one literal path) | `support::set_input_files(&locator, "path")`, which hands the focused input to `DOM.setFileInputFiles` |
| `waitFor({ state })` | `.wait_for(State::...)` |
| `{ timeout }` on an action | `.with_timeout(Duration::from_millis(...))` |

The page-level forms (`page.click(selector)`, `page.fill(selector, text)` and
so on) map to the same calls on `page.locator(selector)`.

### Values read from the page

`const x = await ...` (Python `x = ...`) becomes `let x = ...;`, and a later use
of `x` as a value becomes `&x`. A value that is never used is named `_x`.

| Playwright | Rust | Type |
|---|---|---|
| `textContent()`, `innerText()` | `.text()` (the rendered text) | `String` |
| `inputValue()` | `support::field_value(&locator)`: the field is focused and its value read | `String` |
| `getAttribute(name)` | `.attribute(name)` | `Option<String>` |
| `isVisible()`, `isHidden()`, `isChecked()`, `isEnabled()`, `isDisabled()` | `.is_visible()`, `!...is_visible()`, ... | `bool` |
| `count()` | `.count()` | `usize` |
| `title()`, `url()` | `page.title()`, `page.url()` | `String` |
| `evaluate(...)` | `page.evaluate(...)` | `serde_json::Value` |

### Expectations

| Playwright | Rust |
|---|---|
| `toBeVisible()`, `toBeHidden()`, `toBeAttached()` | `.expect().to_be_visible()`, `to_be_hidden()`, `to_exist()` |
| `toHaveText(t)`, `toContainText(t)` | `.expect().to_have_text(t)`, `to_contain_text(t)` |
| `toHaveValue(v)` | `support::expect_field_value(&locator, v, negated)` |
| `toHaveAttribute(name, value)`, `toHaveId(id)` | `.expect().to_have_attribute(name, value)` |
| `toBeChecked()` | `.expect().to_be_checked()` (`{ checked: false }` negates it) |
| `toBeEnabled()`, `toBeDisabled()` | `support::expect_enabled(&locator, true / false)` |
| `toHaveCount(n)` | `.expect().to_have_count(n)` |
| `expect(page).toHaveTitle(t)`, `toHaveURL(u)` | `page.expect().to_have_title(t)`, `to_have_url(u)` |
| `.not.` (Python `not_to_...`) | `.not()` |
| `{ timeout }` | `.expect().timeout(Duration::from_millis(...))` |
| `expect(x).toBe(y)`, `toEqual`, `toContain`, `toMatch`, `toBeTruthy`, `toBeFalsy`, `toBeNull`, `toBeGreaterThan`, ... on a value read before | `assert_eq!`, `assert_ne!`, `assert!` |
| Python `assert x == y`, `assert "a" in x`, `assert not x`, `assert x > 1` | the same |

Every expectation re-checks until it holds or the locator's timeout passes, as
Playwright's do. A regular expression is converted only when it is plain text:
`/Shop/` becomes a contains check (`to_contain_title`, `to_contain_text`,
`contains`), `/^Shop$/` an exact one; leading and trailing `.*` are dropped.
Anything else in a regular expression (classes, alternatives, the `i` flag) is
a `TODO`. `expect.soft` is converted as a normal expectation.

### The getByRole approximation

The pure CDP engine has no accessibility-tree query, so `getByRole` becomes an
XPath built by `SelectorBuf::role`. An element has the role when its `role`
attribute says so, or when it has no `role` attribute and is the native
element for it:

- `button`: `<button>`, `<summary>` and `<input type="submit|button|reset|image">`
- `link`: `<a href>` and `<area href>`
- `checkbox`: `<input type="checkbox">`
- `radio`: `<input type="radio">`
- `heading`: `<h1>` to `<h6>`
- `textbox`: `<input>` (text, email, tel, url, password) and `<textarea>`
- `combobox` or `listbox`: `<select>`
- landmarks and structure: `<li>`, `<ul>`, `<ol>`, `<nav>`, `<main>`, `<table>`,
  `<tr>`, `<td>`, `<th>`, `<img alt>`, `<dialog>`, `<form>`, `<section>` and the
  other common landmarks

The name matches the element's `aria-label`, text, `value`, `title`, `alt`, or
the text of its `<label>` (by `for` or by wrapping).

<Warning>

Not covered: `aria-labelledby` and `aria-describedby`, roles that depend on
context (a `<header>` inside an `<article>` is not a `banner`), hidden elements
being left out (the engine's actions wait for a visible match anyway), the
precedence of `aria-label` over the text, and the `level`, `checked`,
`pressed`, `expanded`, `selected` and `includeHidden` options, which become a
`TODO`.

</Warning>

## What becomes a TODO

Each of these is left as a `// TODO(playwright)` comment with the statement and
the reason, plus a diagnostic with its line:

- request interception and mocking: `route`, `unroute`, `routeFromHAR`;
- event handlers and waits for events: `page.on`, `waitForEvent`,
  `waitForRequest`, `waitForResponse`, Python's `with page.expect_popup()` and
  the other `expect_*` context managers (`expect_navigation` is converted);
- popups, a second page, and `browser.pages()`;
- frames beyond `frameLocator(css).locator(...)`: `page.frames()`, `frame()`;
- dialogs, downloads, file choosers, tracing, CDP sessions, `page.pause()`;
- loops, conditionals, `try`, helper functions, page objects, classes;
- fixtures other than `page`, `context`, `browser` and `playwright`, and the
  `pytest.mark.parametrize`, `skipif` and `xfail` markers; `test.use()`;
- template literals with `${}`, f-strings with placeholders, and selectors
  built at run time;
- `evaluate` with arguments, or with a function that uses the test's variables
  or has TypeScript syntax in it;
- options the importer does not know (`force`, `position`, `modifiers`, ...),
  key combinations such as `Control+A`, several files or options at once;
- matchers it does not know: `toHaveClass`, `toHaveCSS`, `toBeFocused`,
  `toBeEditable`, `toBeEmpty`, `toHaveScreenshot`, `expect.poll`, ...

Statements the importer cannot read at all (a syntax error) are a `TODO` too,
with the `Error` severity, and make the command exit with status 1.

## Dropped on purpose

<Info>

These have no effect on the generated test and are left out without a `TODO`:
`import`/`require` lines, the launch call and its options (the test launches
with `support::launch_options()`), `newContext()` without options (with
options, a `TODO` says they are not carried over), `context.close()` and
`browser.close()` (the test closes its browser), `test.describe.configure()`,
`test.slow()`, `test.setTimeout()`, and the `delay` option of typing.
Comments inside a test are kept; blank lines and comments outside tests are
not.

</Info>

## Safety

<Warning>

The input is untrusted text and the output is code you compile.

</Warning>

The importer
reads it with its own small tokenizer and parser (no regular expressions, no
evaluation), refuses files over 2 MB, and stops a statement that nests deeper
than 100 levels. Text from the input reaches the Rust only inside string
literals, escaped (quotes, backslashes, control, bidirectional and invisible
characters), or inside `//` comments with line breaks removed; test and
variable names keep only `a-z`, `0-9` and `_`. Review the generated file before
running it all the same, as you would any code you did not write.
