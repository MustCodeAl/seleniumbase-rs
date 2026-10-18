---
title: "Migrate Puppeteer Tests"
sidebarTitle: "From Puppeteer"
---

`sbrs import-puppeteer` converts Puppeteer code into Rust tests for the
[pure CDP engine](/tutorials/pure_cdp). It reads JavaScript and TypeScript:

- plain scripts on `puppeteer`, `puppeteer-core` and `puppeteer-extra`
  (`const browser = await puppeteer.launch(); const page = await browser.newPage(); ...`),
  in an async IIFE, a `main()` the file calls, or top-level `await`;
- jest-puppeteer tests: the global `page`, `describe` and `it`/`test`, the
  `beforeAll`/`beforeEach`/`afterEach` hooks, and the expect-puppeteer matchers;
- Mocha tests with `describe`/`it`, `before`/`beforeEach`/`after` hooks that
  launch the browser and assign `page`, and node's `assert` or chai's `expect`;
- the Chrome DevTools Recorder's JSON export (the `@puppeteer/replay` format).

It shares its tokenizer, parser, escaping and Rust writer with
[`sbrs import-playwright`](/playwright-migration), so the generated file
has the same shape: one `#[tokio::test]` per test (a script or a recording is
one test), each launching a browser and driving its page with `Page` and
`Locator` calls. What is not converted stays in place as a
`// TODO(puppeteer): <statement>` comment and a diagnostic with its line.

<Visibility for="agents">

Summary for AI exports: `sbrs import-puppeteer FILE --output tests/FILE.rs` converts Puppeteer code (plain scripts, jest-puppeteer and Mocha tests, and a Chrome DevTools Recorder JSON export) into Rust tests for the pure CDP engine. A statement it does not convert stays as a `// TODO(puppeteer): <statement>` comment and is reported with its line. Exit status: 0 when everything was read, 1 when a statement could not be read, 6 when the file cannot be read, 2 for a source it refuses.

</Visibility>

## Convert a file

```bash
cargo run --bin sbrs -- import-puppeteer scripts/checkout.js --output tests/checkout.rs
cargo run --bin sbrs -- import-pptr __tests__/shop.test.ts
cargo run --bin sbrs -- import-puppeteer recordings/checkout.json   # a Recorder export
```

<ParamField body="--output, -o" type="file path">

Write the Rust there instead of to standard output.

</ParamField>

<ParamField body="--language" type="js | ts | json">

Override the language, which otherwise comes from the extension (`.json` is a Recorder export) or the content.

</ParamField>

<ParamField body="--test-name" type="any text">

Name the test made from a script; the default is the file name. Tests keep their `describe`/`it` titles and a recording its `title`.

</ParamField>

The diagnostics and exit statuses are those of `import-playwright`:

| Status | Meaning |
|---|---|
| 0 | Everything was read, with gaps as warnings. |
| 1 | A statement could not be read. |
| 6 | A file that cannot be read. |
| 2 | A source the importer refuses: over 2 MB, or Python, which Puppeteer has no API for. |

From Rust, call `seleniumbase_rs::utilities::importer::import_puppeteer`.

<Tip>

Run the generated tests headless with `SB_HEADLESS=true cargo test`. They need
`seleniumbase-rs` and `tokio` with the `macros`, `rt-multi-thread` and `time`
features.

</Tip>

## The mappings

### The page

| Puppeteer | Rust |
|---|---|
| `goto(url, { waitUntil })` | `page.goto(url)` (it waits for the load event) |
| `reload()`, `goBack()`, `goForward()` | `page.reload()`, `page.back()`, `page.forward()` |
| `waitForNavigation()`, `waitForNetworkIdle()` | `page.wait_for_load()` |
| `Promise.all([page.waitForNavigation(), page.click(sel)])` | the click, then `page.wait_for_load()` |
| `waitForSelector(sel)`, `{ visible: true }`, `{ hidden: true }` | `.wait_for(State::Present)`, `State::Visible`, `State::Hidden`; as a value it is the locator |
| `waitForTimeout(ms)`, `new Promise(r => setTimeout(r, ms))` | `tokio::time::sleep(Duration::from_millis(ms))` |
| `waitForFunction(fn or string)` | `page.wait_for_function(...)`, under the `evaluate` rule below |
| `setViewport({ width, height })` | `page.window().set_bounds(0, 0, width, height)` (a mobile or scaled viewport is a `TODO`) |
| `setUserAgent(ua)` | `page.emulation().user_agent(ua, None)` |
| `screenshot({ path })` | `std::fs::write(path, page.screenshot().await?)` |
| `title()`, `url()`, `content()` | `page.title()`, `page.url()`, `page.content()` |
| `evaluate(fn or string)` | `page.evaluate(...)`: a function with no parameters that uses nothing from the test is copied as JavaScript text |
| `setCookie({ name, value, domain or url, path, secure, httpOnly, expires })` with literal values | `page.cookies().set(&[Cookie::new(name, value).domain(...)...])` |
| `setDefaultTimeout(ms)` | `let page = page.with_timeout(...)` |
| `keyboard.press(key)`, `keyboard.type(text)`, `keyboard.sendCharacter(text)` | `page.keyboard().press(Key::...)`, `.type_text(text)`, `.insert_text(text)` |
| `mouse.click(x, y)`, `mouse.dblclick(x, y)`, `mouse.move(x, y)` (literal numbers) | `page.mouse().click(Point::new(x, y))`, `double_click`, `move_to` |

### Actions

| Puppeteer | Rust |
|---|---|
| `page.click(sel)`, `{ button: 'right' }`, `{ clickCount: 2 }`, `page.tap(sel)` | `page.locator(sel).click()`, `.right_click()`, `.double_click()` |
| `page.type(sel, text)` | `page.locator(sel).send_keys(text)` (Puppeteer types after what is there, so does this; `type_text` empties the field first) |
| `page.focus(sel)`, `page.hover(sel)` | `.focus()`, `.hover()` |
| `page.select(sel, value)` (one value) | `.select_option(SelectBy::Value(value))` |
| `page.locator(sel).click()`, `.fill(text)`, `.hover()`, `.wait()`, `.setTimeout(ms)` | `.click()`, `.fill(text)`, `.hover()`, `.wait_for(State::Visible)`, `.with_timeout(...)` |

### Element handles

`const el = await page.$(sel)` becomes `let el = page.locator(sel);`: a lazy
locator, which looks the element up again each time it is used.

| Puppeteer | Rust |
|---|---|
| `el.click()`, `el.type(text)`, `el.focus()`, `el.hover()`, `el.select(value)` | the same calls on the locator |
| `el.uploadFile(path)` (one literal path) | `support::set_input_files(&el, path)` |
| `el.evaluate(e => e.textContent)` | as `$eval` below |
| `const els = await page.$$(sel)`, `els.length`, `els[0]` | `let els = page.locator(sel);`, `els.count().await?`, `els.nth(0)` |
| `page.$eval(sel, el => el.textContent)` (or `innerText`, `textContent.trim()`) | `page.locator(sel).text().await?` |
| `page.$eval(sel, el => el.value)` | `support::field_value(&page.locator(sel)).await?` |
| `page.$eval(sel, el => el.checked)` | `page.locator(sel).is_checked().await?` |
| `page.$eval(sel, el => el.getAttribute('x'))` | `page.locator(sel).attribute("x").await?` |
| `page.$$eval(sel, els => els.length)` | `page.locator(sel).count().await?` |

### Selectors

| Puppeteer | Rust |
|---|---|
| CSS | `page.locator("css")` |
| `xpath/...`, `::-p-xpath(...)` | `page.locator("//xpath")` |
| `text/...`, `::-p-text(...)` | `SelectorBuf::text(text, false)`: the innermost elements containing the text (case-insensitive here, case-sensitive in Puppeteer) |
| `css ::-p-text(...)` (descendant) | `.locator(css).locator(SelectorBuf::text(...))` |
| `css::-p-text(...)` (the element itself) | `.locator(css).locator(SelectorBuf::has_text(...))` |
| `aria/Name`, `aria/Name[role="button"]`, `::-p-aria(...)` | `SelectorBuf::role(role, Some(name), true)`, the role approximation described in [Migrate Playwright Tests](/playwright-migration#the-getbyrole-approximation) |
| `pierce/...`, `>>>` | a `TODO`: the pure CDP engine's selectors do not enter shadow roots |

### Assertions

| Source | Rust |
|---|---|
| jest `expect(x).toBe(y)`, `toEqual`, `toStrictEqual`, `.not.toBe` | `assert_eq!(x, y)`, `assert_ne!` |
| jest `toContain`, `toMatch(/plain/ or 'text')`, `toBeTruthy`, `toBeFalsy`, `toBeNull`, `toBeGreaterThan`, `toHaveLength` | `assert!(x.contains(...))`, `assert!(...)` |
| expect-puppeteer `expect(page).toMatchElement(sel, { text })` | `page.locator(sel).locator(SelectorBuf::has_text(text)).expect().to_exist()` (`{ visible: true }`: `to_be_visible`) |
| `expect(page).toClick(sel, { text })`, `toFill(sel, value)`, `toSelect(sel, value)`, `toUploadFile(sel, path)` | the click, `fill`, `support::select_option`, `support::set_input_files` |
| `expect(page).toMatchTextContent(text)` (and `toMatch`) | `page.locator("body").expect().to_contain_text(text)` |
| node `assert(x)`, `assert.ok(x)`, `assert.equal`, `assert.strictEqual`, `assert.deepStrictEqual`, `assert.notEqual`, `assert.match` | `assert!`, `assert_eq!`, `assert_ne!` |
| chai `expect(x).to.equal(y)`, `.to.eql`, `.to.include`, `.to.contain`, `.to.match`, `.to.be.true`, `.to.be.false`, `.to.be.ok`, `.to.be.null`, `.to.exist`, `.to.be.empty`, `.to.be.above`, `.to.be.below`, `.to.have.lengthOf`, with `.not` | the same |

A regular expression is converted only when it is plain text, as for
Playwright: `/Shop/` is a contains check and `/^Shop$/` an exact one.

### The Chrome Recorder export

The steps of a recording become the statements of one test named after its
`title`. Each step that targets an element lists several selectors; the first
one this crate can use is taken: CSS first, then `xpath/`, then `aria/`, then
`text/`. Selectors through frames or shadow roots (more than one part) and
`pierce/` are passed over.

| Step | Rust |
|---|---|
| `setViewport` | `page.window().set_bounds(...)` (a mobile viewport is a `TODO`) |
| `navigate` | `page.goto(url)` |
| `click`, `doubleClick` (`button: "secondary"`: a right click) | `.click()`, `.double_click()`, `.right_click()`; the offsets are not kept |
| `hover` | `.hover()` |
| `change` | `support::change(&locator, value)`: a `<select>` chooses the option with that value, anything else is filled |
| `keyDown` (`keyUp` is part of the same press) | `page.keyboard().press(Key::...)` or `type_text` |
| `scroll` | `page.scroll(Scroll::To(y))`, or `.scroll_into_view()` on an element |
| `waitForElement` (one or more matches, `visible`) | `.wait_for(State::Visible)` or `State::Present`; other counts are a `TODO` |
| `waitForExpression` | `page.wait_for_function(expression)` |
| `assertedEvents` with a navigation | `page.wait_for_load()` after the step |
| `close` | nothing: the test closes its browser |
| other steps (`emulateNetworkConditions`, `customStep`, steps in another tab or a frame) | a `TODO` |

## What becomes a TODO

- request interception (`setRequestInterception`, `on('request')`), event
  handlers (`page.on`, `once`), `waitForRequest`, `waitForResponse`,
  `waitForFileChooser`;
- `puppeteer.use(...)` plugins, such as the puppeteer-extra stealth plugin
  (the pure CDP engine has its own stealth options: `LaunchOptions` and
  `Identity`), and `puppeteer.connect()`;
- frames (`page.frames()`, `mainFrame()`), CDP sessions, a second page,
  `browser.pages()`, cookies other than `setCookie` with literal values;
- shadow-DOM piercing selectors (`pierce/`, `>>>`);
- loops, conditionals, `try`, helper functions, page objects, classes;
- template literals with `${}`, selectors built at run time, `evaluate` with
  arguments or with a function that uses the test's variables;
- element callbacks other than the ones in the table above, and options the
  importer does not know.

## Dropped on purpose

<Info>

`require`/`import` lines, `puppeteer.launch()` and its options (the test
launches with `support::launch_options()`, headless when `SB_HEADLESS` is
set), `browser.close()` and `page.close()`, `this.timeout()` and
`jest.setTimeout()`, and the `delay` option of typing are left out without a
`TODO`. Comments inside a test
are kept; blank lines and comments outside tests are not.

</Info>

## Safety

<Warning>

The input is read as untrusted text by the importer's own tokenizer and
parser, with a 2 MB limit and a nesting limit; text from it reaches the Rust
only in escaped string literals and neutralised comments, and names keep only
`a-z`, `0-9` and `_`. See [Migrate Playwright Tests](/playwright-migration#safety).

</Warning>
