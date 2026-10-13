//! The exact Rust each mapping writes, in the JavaScript and the Python
//! spelling, and what becomes a `TODO`.

use super::*;

/// A Playwright test in JavaScript with `body` inside it.
fn js(body: &str) -> ScriptImport {
    let source = format!(
        "import {{ test, expect }} from '@playwright/test';\n\
         test('t', async ({{ page }}) => {{\n{body}\n}});\n"
    );
    import_playwright(&source, &ScriptImportOptions::default()).expect("imports")
}

/// A pytest-playwright test with `body` inside it.
fn py(body: &str) -> ScriptImport {
    let mut source =
        String::from("import re\nfrom playwright.sync_api import Page, expect\n\n\ndef test_t(page: Page) -> None:\n");
    for line in body.lines() {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    import_playwright(&source, &ScriptImportOptions::default()).expect("imports")
}

/// A jest-puppeteer test (global `page`) with `body` inside it.
fn pptr(body: &str) -> ScriptImport {
    let source =
        format!("describe('shop', () => {{\n  it('t', async () => {{\n{body}\n  }});\n}});\n");
    import_puppeteer(&source, &ScriptImportOptions::default()).expect("imports")
}

/// Asserts that one line of the generated Rust is exactly `expected`.
#[track_caller]
fn has(import: &ScriptImport, expected: &str) {
    assert!(
        import.rust.lines().any(|line| line.trim() == expected),
        "missing line:\n{expected}\n--- in ---\n{}\n--- diagnostics ---\n{:?}",
        import.rust,
        import.diagnostics
    );
}

/// Asserts that everything was converted and the file parses as Rust.
#[track_caller]
fn complete(import: &ScriptImport) {
    assert!(
        import.is_complete(),
        "{:?}\n{}",
        import.diagnostics,
        import.rust
    );
    syn::parse_file(&import.rust).unwrap_or_else(|e| panic!("{e}\n{}", import.rust));
}

#[track_caller]
fn both(js_body: &str, py_body: &str, expected: &str) {
    for import in [js(js_body), py(py_body)] {
        complete(&import);
        has(&import, expected);
    }
}

// ----------------------------------------------------------------------
// The page
// ----------------------------------------------------------------------

#[test]
fn navigation() {
    both(
        "await page.goto('https://x.test/');",
        "page.goto(\"https://x.test/\")",
        "page.goto(\"https://x.test/\").await?;",
    );
    both(
        "await page.reload();",
        "page.reload()",
        "page.reload().await?;",
    );
    both(
        "await page.goBack();",
        "page.go_back()",
        "page.back().await?;",
    );
    both(
        "await page.goForward();",
        "page.go_forward()",
        "page.forward().await?;",
    );
}

#[test]
fn waits() {
    both(
        "await page.waitForURL('**/done');",
        "page.wait_for_url(\"**/done\")",
        "page.expect().to_contain_url(\"/done\").await?;",
    );
    both(
        "await page.waitForURL('https://x.test/done');",
        "page.wait_for_url(\"https://x.test/done\")",
        "page.expect().to_have_url(\"https://x.test/done\").await?;",
    );
    both(
        "await page.waitForURL(/\\/done$/);",
        "page.wait_for_url(re.compile(r\"\\/done$\"))",
        "page.expect().to_contain_url(\"/done\").await?;",
    );
    both(
        "await page.waitForLoadState('networkidle');",
        "page.wait_for_load_state()",
        "page.wait_for_load().await?;",
    );
    both(
        "await page.waitForTimeout(500);",
        "page.wait_for_timeout(500)",
        "tokio::time::sleep(std::time::Duration::from_millis(500)).await;",
    );
    both(
        "await page.waitForSelector('#x');",
        "page.wait_for_selector(\"#x\")",
        "page.locator(\"#x\").wait_for(State::Visible).await?;",
    );
    both(
        "await page.waitForSelector('#x', { state: 'hidden' });",
        "page.wait_for_selector(\"#x\", state=\"hidden\")",
        "page.locator(\"#x\").wait_for(State::Hidden).await?;",
    );
    has(
        &js("await page.waitForSelector('#x');"),
        "use seleniumbase_rs::sb_cdp::{Browser, State};",
    );
}

#[test]
fn viewport_and_screenshot() {
    both(
        "await page.setViewportSize({ width: 800, height: 600 });",
        "page.set_viewport_size({\"width\": 800, \"height\": 600})",
        "page.window().set_bounds(0, 0, 800, 600).await?;",
    );
    both(
        "await page.screenshot({ path: 'shot.png' });",
        "page.screenshot(path=\"shot.png\")",
        "std::fs::write(\"shot.png\", page.screenshot().await?)?;",
    );
}

#[test]
fn title_and_url_are_read_into_values() {
    let import = js("const title = await page.title();\nexpect(title).toBe('Shop');");
    complete(&import);
    has(&import, "let title = page.title().await?;");
    has(&import, "assert_eq!(title, \"Shop\");");
    let import = py("title = page.title()\nassert title == \"Shop\"");
    complete(&import);
    has(&import, "let title = page.title().await?;");
    has(&import, "assert_eq!(title, \"Shop\");");

    let import = js("const url = page.url();\nexpect(url).toContain('/shop');");
    complete(&import);
    has(&import, "let url = page.url().await?;");
    has(&import, "assert!(url.contains(\"/shop\"));");
    // In Python `url` is a property.
    let import = py("assert \"/shop\" in page.url");
    complete(&import);
    has(&import, "assert!(page.url().await?.contains(\"/shop\"));");
}

#[test]
fn evaluate_takes_a_string_or_a_function_that_uses_nothing_from_the_test() {
    let import =
        js("const t = await page.evaluate(() => document.title);\nexpect(t).toBe('Shop');");
    complete(&import);
    has(
        &import,
        "let t = page.evaluate(\"(() => document.title)()\").await?;",
    );
    has(&import, "assert_eq!(t, \"Shop\");");
    let import = py("t = page.evaluate(\"() => document.title\")\nassert t == \"Shop\"");
    complete(&import);
    has(
        &import,
        "let t = page.evaluate(\"(() => document.title)()\").await?;",
    );
    let import = py("t = page.evaluate(\"document.title\")\nassert t == \"Shop\"");
    has(&import, "let t = page.evaluate(\"document.title\").await?;");

    let captured =
        js("const sel = '#a';\nawait page.evaluate(() => document.querySelector(sel).click());");
    assert!(!captured.is_complete());
    assert!(
        captured.diagnostics[0]
            .message
            .contains("uses `sel` from the test"),
        "{:?}",
        captured.diagnostics
    );
    let typed = js("await page.evaluate(() => (document.body as HTMLElement).id);");
    assert!(typed.diagnostics[0].message.contains("TypeScript syntax"));
}

#[test]
fn keyboard_and_mouse() {
    both(
        "await page.keyboard.press('Enter');",
        "page.keyboard.press(\"Enter\")",
        "page.keyboard().press(Key::Enter).await?;",
    );
    both(
        "await page.keyboard.type('abc');",
        "page.keyboard.type(\"abc\")",
        "page.keyboard().type_text(\"abc\").await?;",
    );
    both(
        "await page.mouse.click(10, 20);",
        "page.mouse.click(10, 20)",
        "page.mouse().click(Point::new(10.0, 20.0)).await?;",
    );
    let combination = js("await page.keyboard.press('Control+A');");
    assert!(combination.diagnostics[0]
        .message
        .contains("key combination"));
}

// ----------------------------------------------------------------------
// Locators
// ----------------------------------------------------------------------

#[test]
fn selector_engines() {
    for (selector, expected) in [
        ("#a", "page.locator(\"#a\").click().await?;"),
        ("css=#a", "page.locator(\"#a\").click().await?;"),
        ("xpath=//a", "page.locator(\"//a\").click().await?;"),
        ("//a", "page.locator(\"//a\").click().await?;"),
        (
            "text=Go",
            "page.locator(SelectorBuf::text(\"Go\", false)).click().await?;",
        ),
        ("id=x", r#"page.locator("[id=\"x\"]").click().await?;"#),
        (
            "data-testid=x",
            r#"page.locator("[data-testid=\"x\"]").click().await?;"#,
        ),
        (
            "button:has-text(\"Go\")",
            "page.locator(\"button\").locator(SelectorBuf::has_text(\"Go\")).click().await?;",
        ),
        (
            "form >> button",
            "page.locator(\"form\").locator(\"button\").click().await?;",
        ),
    ] {
        let import = js(&format!(
            "await page.locator('{}').click();",
            selector.replace('\'', "\\'")
        ));
        complete(&import);
        has(&import, expected);
        let import = py(&format!(
            "page.locator('{}').click()",
            selector.replace('\'', "\\'")
        ));
        complete(&import);
        has(&import, expected);
    }
    both(
        "await page.locator('form').locator('button').click();",
        "page.locator(\"form\").locator(\"button\").click()",
        "page.locator(\"form\").locator(\"button\").click().await?;",
    );
    both(
        "await page.frameLocator('#f').locator('button').click();",
        "page.frame_locator(\"#f\").locator(\"button\").click()",
        "page.locator(\"#f\").locator(\"button\").click().await?;",
    );
    has(
        &js("await page.getByText('Go').click();"),
        "use seleniumbase_rs::utils::selectors::SelectorBuf;",
    );
}

#[test]
fn get_by_locators() {
    both(
        "await page.getByRole('button', { name: 'Sign in', exact: true }).click();",
        "page.get_by_role(\"button\", name=\"Sign in\", exact=True).click()",
        "page.locator(SelectorBuf::role(\"button\", Some(\"Sign in\"), true)).click().await?;",
    );
    both(
        "await page.getByRole('link').click();",
        "page.get_by_role(\"link\").click()",
        "page.locator(SelectorBuf::role(\"link\", None, false)).click().await?;",
    );
    both(
        "await page.getByText('Welcome').click();",
        "page.get_by_text(\"Welcome\").click()",
        "page.locator(SelectorBuf::text(\"Welcome\", false)).click().await?;",
    );
    both(
        "await page.getByText('Welcome', { exact: true }).click();",
        "page.get_by_text(\"Welcome\", exact=True).click()",
        "page.locator(SelectorBuf::text(\"Welcome\", true)).click().await?;",
    );
    both(
        "await page.getByLabel('Email').fill('a@b.c');",
        "page.get_by_label(\"Email\").fill(\"a@b.c\")",
        "page.locator(SelectorBuf::label(\"Email\", false)).fill(\"a@b.c\").await?;",
    );
    both(
        "await page.getByPlaceholder('Search').fill('x');",
        "page.get_by_placeholder(\"Search\").fill(\"x\")",
        r#"page.locator("[placeholder*=\"Search\" i]").fill("x").await?;"#,
    );
    both(
        "await page.getByTestId('submit').click();",
        "page.get_by_test_id(\"submit\").click()",
        r#"page.locator("[data-testid=\"submit\"]").click().await?;"#,
    );
    both(
        "await page.getByAltText('Logo').click();",
        "page.get_by_alt_text(\"Logo\").click()",
        r#"page.locator("[alt*=\"Logo\" i]").click().await?;"#,
    );
    both(
        "await page.getByTitle('Help').hover();",
        "page.get_by_title(\"Help\").hover()",
        r#"page.locator("[title*=\"Help\" i]").hover().await?;"#,
    );
}

#[test]
fn positions_and_filters() {
    both(
        "await page.locator('li').first().click();",
        "page.locator(\"li\").first.click()",
        "page.locator(\"li\").first().click().await?;",
    );
    both(
        "await page.locator('li').last().click();",
        "page.locator(\"li\").last.click()",
        "page.locator(\"li\").last().click().await?;",
    );
    both(
        "await page.locator('li').nth(2).click();",
        "page.locator(\"li\").nth(2).click()",
        "page.locator(\"li\").nth(2).click().await?;",
    );
    both(
        "await page.locator('li').filter({ hasText: 'Shoes' }).click();",
        "page.locator(\"li\").filter(has_text=\"Shoes\").click()",
        "page.locator(\"li\").locator(SelectorBuf::has_text(\"Shoes\")).click().await?;",
    );
}

// ----------------------------------------------------------------------
// Actions
// ----------------------------------------------------------------------

#[test]
fn clicks_and_typing() {
    both(
        "await page.locator('#a').click();",
        "page.locator(\"#a\").click()",
        "page.locator(\"#a\").click().await?;",
    );
    both(
        "await page.locator('#a').dblclick();",
        "page.locator(\"#a\").dblclick()",
        "page.locator(\"#a\").double_click().await?;",
    );
    both(
        "await page.locator('#a').click({ button: 'right' });",
        "page.locator(\"#a\").click(button=\"right\")",
        "page.locator(\"#a\").right_click().await?;",
    );
    both(
        "await page.locator('#a').fill('x');",
        "page.locator(\"#a\").fill(\"x\")",
        "page.locator(\"#a\").fill(\"x\").await?;",
    );
    both(
        "await page.locator('#a').type('x');",
        "page.locator(\"#a\").type(\"x\")",
        "page.locator(\"#a\").type_text(\"x\").await?;",
    );
    both(
        "await page.locator('#a').pressSequentially('x');",
        "page.locator(\"#a\").press_sequentially(\"x\")",
        "page.locator(\"#a\").type_text(\"x\").await?;",
    );
    both(
        "await page.locator('#a').press('Enter');",
        "page.locator(\"#a\").press(\"Enter\")",
        "page.locator(\"#a\").press(Key::Enter).await?;",
    );
    both(
        "await page.locator('#a').press('a');",
        "page.locator(\"#a\").press(\"a\")",
        "page.locator(\"#a\").type_text(\"a\").await?;",
    );
    // The page-level forms take the selector first.
    both(
        "await page.click('#a');",
        "page.click(\"#a\")",
        "page.locator(\"#a\").click().await?;",
    );
    both(
        "await page.fill('#a', 'x');",
        "page.fill(\"#a\", \"x\")",
        "page.locator(\"#a\").fill(\"x\").await?;",
    );
}

#[test]
fn form_controls() {
    both(
        "await page.locator('#c').check();",
        "page.locator(\"#c\").check()",
        "page.locator(\"#c\").check().await?;",
    );
    both(
        "await page.locator('#c').uncheck();",
        "page.locator(\"#c\").uncheck()",
        "page.locator(\"#c\").uncheck().await?;",
    );
    both(
        "await page.locator('#c').setChecked(true);",
        "page.locator(\"#c\").set_checked(True)",
        "page.locator(\"#c\").set_checked(true).await?;",
    );
    both(
        "await page.locator('#s').selectOption('blue');",
        "page.locator(\"#s\").select_option(\"blue\")",
        "support::select_option(&page.locator(\"#s\"), \"blue\").await?;",
    );
    both(
        "await page.locator('#s').selectOption({ label: 'Blue' });",
        "page.locator(\"#s\").select_option(label=\"Blue\")",
        "page.locator(\"#s\").select_option(SelectBy::Text(\"Blue\")).await?;",
    );
    both(
        "await page.locator('#s').selectOption({ index: 2 });",
        "page.locator(\"#s\").select_option(index=2)",
        "page.locator(\"#s\").select_option(SelectBy::Index(2)).await?;",
    );
    both(
        "await page.locator('#s').selectOption({ value: 'b' });",
        "page.locator(\"#s\").select_option(value=\"b\")",
        "page.locator(\"#s\").select_option(SelectBy::Value(\"b\")).await?;",
    );
    both(
        "await page.locator('#a').hover();",
        "page.locator(\"#a\").hover()",
        "page.locator(\"#a\").hover().await?;",
    );
    both(
        "await page.locator('#a').focus();",
        "page.locator(\"#a\").focus()",
        "page.locator(\"#a\").focus().await?;",
    );
    both(
        "await page.locator('#a').clear();",
        "page.locator(\"#a\").clear()",
        "page.locator(\"#a\").clear().await?;",
    );
    both(
        "await page.locator('#a').blur();",
        "page.locator(\"#a\").blur()",
        "page.locator(\"#a\").page().evaluate(\"document.activeElement && document.activeElement.blur()\").await?;",
    );
    both(
        "await page.locator('#f').setInputFiles('a.txt');",
        "page.locator(\"#f\").set_input_files(\"a.txt\")",
        "support::set_input_files(&page.locator(\"#f\"), \"a.txt\").await?;",
    );
    let import = js("await page.locator('#s').selectOption('blue');");
    assert!(
        import.rust.contains("pub async fn select_option("),
        "{}",
        import.rust
    );
}

#[test]
fn values_read_from_the_page_and_used_later() {
    let import = js("const a = await page.locator('#a').textContent();\n\
         const b = await page.locator('#b').innerText();\n\
         const v = await page.locator('#c').inputValue();\n\
         const h = await page.locator('#d').getAttribute('href');\n\
         const shown = await page.locator('#e').isVisible();\n\
         const on = await page.locator('#f').isChecked();\n\
         const n = await page.locator('li').count();\n\
         expect(a).toBe('x');\n\
         expect(b).toContain('y');\n\
         await page.locator('#m').fill(v);\n\
         expect(h).toBe('/about');\n\
         expect(shown).toBe(true);\n\
         expect(on).toBeFalsy();\n\
         expect(n).toBe(3);");
    complete(&import);
    for line in [
        "let a = page.locator(\"#a\").text().await?;",
        "let b = page.locator(\"#b\").text().await?;",
        "let v = support::field_value(&page.locator(\"#c\")).await?;",
        "let h = page.locator(\"#d\").attribute(\"href\").await?;",
        "let shown = page.locator(\"#e\").is_visible().await?;",
        "let on = page.locator(\"#f\").is_checked().await?;",
        "let n = page.locator(\"li\").count().await?;",
        "assert_eq!(a, \"x\");",
        "assert!(b.contains(\"y\"));",
        "page.locator(\"#m\").fill(&v).await?;",
        "assert_eq!(h.as_deref(), Some(\"/about\"));",
        "assert!(shown);",
        "assert!(!(on));",
        "assert_eq!(n, 3);",
    ] {
        has(&import, line);
    }
    let import = py("a = page.locator(\"#a\").text_content()\n\
         v = page.locator(\"#c\").input_value()\n\
         h = page.locator(\"#d\").get_attribute(\"href\")\n\
         shown = page.locator(\"#e\").is_visible()\n\
         n = page.locator(\"li\").count()\n\
         assert a == \"x\"\n\
         assert \"y\" in a\n\
         page.locator(\"#m\").fill(v)\n\
         assert h == \"/about\"\n\
         assert not shown\n\
         assert n == 3");
    complete(&import);
    for line in [
        "let a = page.locator(\"#a\").text().await?;",
        "let v = support::field_value(&page.locator(\"#c\")).await?;",
        "assert_eq!(a, \"x\");",
        "assert!(a.contains(\"y\"));",
        "page.locator(\"#m\").fill(&v).await?;",
        "assert_eq!(h.as_deref(), Some(\"/about\"));",
        "assert!(!(shown));",
        "assert_eq!(n, 3);",
    ] {
        has(&import, line);
    }
}

#[test]
fn a_value_that_is_never_used_is_named_with_an_underscore() {
    let import = js("const unused = await page.title();");
    has(&import, "let _unused = page.title().await?;");
}

// ----------------------------------------------------------------------
// Assertions
// ----------------------------------------------------------------------

#[test]
fn locator_expectations() {
    for (js_line, py_line, expected) in [
        ("toBeVisible()", "to_be_visible()", "page.locator(\"#a\").expect().to_be_visible().await?;"),
        ("toBeHidden()", "to_be_hidden()", "page.locator(\"#a\").expect().to_be_hidden().await?;"),
        ("not.toBeVisible()", "not_to_be_visible()", "page.locator(\"#a\").expect().not().to_be_visible().await?;"),
        ("toHaveText('Hi')", "to_have_text(\"Hi\")", "page.locator(\"#a\").expect().to_have_text(\"Hi\").await?;"),
        ("toContainText('Hi')", "to_contain_text(\"Hi\")", "page.locator(\"#a\").expect().to_contain_text(\"Hi\").await?;"),
        ("toHaveText(/Hi/)", "to_have_text(re.compile(\"Hi\"))", "page.locator(\"#a\").expect().to_contain_text(\"Hi\").await?;"),
        ("toHaveValue('x')", "to_have_value(\"x\")", "support::expect_field_value(&page.locator(\"#a\"), \"x\", false).await?;"),
        ("not.toHaveValue('x')", "not_to_have_value(\"x\")", "support::expect_field_value(&page.locator(\"#a\"), \"x\", true).await?;"),
        (
            "toHaveAttribute('href', '/about')",
            "to_have_attribute(\"href\", \"/about\")",
            "page.locator(\"#a\").expect().to_have_attribute(\"href\", \"/about\").await?;",
        ),
        ("toBeChecked()", "to_be_checked()", "page.locator(\"#a\").expect().to_be_checked().await?;"),
        (
            "toBeChecked({ checked: false })",
            "to_be_checked(checked=False)",
            "page.locator(\"#a\").expect().not().to_be_checked().await?;",
        ),
        ("toBeEnabled()", "to_be_enabled()", "support::expect_enabled(&page.locator(\"#a\"), true).await?;"),
        ("toBeDisabled()", "to_be_disabled()", "support::expect_enabled(&page.locator(\"#a\"), false).await?;"),
        ("not.toBeEnabled()", "not_to_be_enabled()", "support::expect_enabled(&page.locator(\"#a\"), false).await?;"),
        ("toHaveCount(3)", "to_have_count(3)", "page.locator(\"#a\").expect().to_have_count(3).await?;"),
        (
            "toBeVisible({ timeout: 5000 })",
            "to_be_visible(timeout=5000)",
            "page.locator(\"#a\").expect().timeout(std::time::Duration::from_millis(5000)).to_be_visible().await?;",
        ),
    ] {
        both(
            &format!("await expect(page.locator('#a')).{js_line};"),
            &format!("expect(page.locator(\"#a\")).{py_line}"),
            expected,
        );
    }
}

#[test]
fn page_expectations() {
    both(
        "await expect(page).toHaveTitle('Shop');",
        "expect(page).to_have_title(\"Shop\")",
        "page.expect().to_have_title(\"Shop\").await?;",
    );
    both(
        "await expect(page).toHaveTitle(/Shop/);",
        "expect(page).to_have_title(re.compile(\"Shop\"))",
        "page.expect().to_contain_title(\"Shop\").await?;",
    );
    both(
        "await expect(page).toHaveTitle(/^Shop$/);",
        "expect(page).to_have_title(re.compile(r\"^Shop$\"))",
        "page.expect().to_have_title(\"Shop\").await?;",
    );
    both(
        "await expect(page).toHaveURL('https://x.test/done');",
        "expect(page).to_have_url(\"https://x.test/done\")",
        "page.expect().to_have_url(\"https://x.test/done\").await?;",
    );
    both(
        "await expect(page).toHaveURL(/done/);",
        "expect(page).to_have_url(re.compile(\"done\"))",
        "page.expect().to_contain_url(\"done\").await?;",
    );
    both(
        "await expect(page).not.toHaveTitle('Old');",
        "expect(page).not_to_have_title(\"Old\")",
        "page.expect().not().to_have_title(\"Old\").await?;",
    );
    let regex = js("await expect(page).toHaveTitle(/Sh.p/);");
    assert!(regex.diagnostics[0].message.contains("not plain text"));
}

#[test]
fn python_asserts() {
    let import = py("assert page.title() == \"Shop\"\nassert page.title() != \"Old\"\nassert \"Shop\" == page.title()");
    complete(&import);
    has(&import, "assert_eq!(page.title().await?, \"Shop\");");
    has(&import, "assert_ne!(page.title().await?, \"Old\");");
}

// ----------------------------------------------------------------------
// What becomes a TODO
// ----------------------------------------------------------------------

#[test]
fn an_unconverted_statement_is_a_todo_with_its_line() {
    let import = js("await page.goto('https://x.test/');\nawait page.route('**/api', route => route.abort());\nawait page.reload();");
    assert!(!import.is_complete());
    has(
        &import,
        "// TODO(playwright): await page.route('**/api', route => route.abort());",
    );
    has(
        &import,
        "//     line 4: request interception (`route()`) is not converted",
    );
    assert_eq!(import.diagnostics.len(), 1);
    assert_eq!(import.diagnostics[0].line, 4);
    assert_eq!(import.diagnostics[0].severity, ImportSeverity::Warning);
    assert!(import.diagnostics[0].message.contains("page.route"));
    // The statements around it are still converted.
    has(&import, "page.reload().await?;");
    syn::parse_file(&import.rust).unwrap();
}

#[test]
fn loops_conditionals_helpers_and_popups_are_todos() {
    for (body, reason) in [
        (
            "for (const x of xs) { await page.click(x); }",
            "a for loop is not converted",
        ),
        (
            "if (await page.isVisible('#a')) { await page.click('#a'); }",
            "an if statement is not converted",
        ),
        (
            "await login(page);",
            "calling `login()` (a helper function) is not converted",
        ),
        (
            "const popup = await page.waitForEvent('popup');",
            "event handlers and waits for events",
        ),
        ("page.on('dialog', d => d.accept());", "event handlers"),
        (
            "await context.newPage();",
            "is not something the importer knows",
        ),
    ] {
        let import = js(body);
        assert!(!import.is_complete(), "{body}");
        assert!(
            import
                .diagnostics
                .iter()
                .any(|d| d.message.contains(reason)),
            "{body}: {:?}",
            import.diagnostics
        );
        syn::parse_file(&import.rust).unwrap_or_else(|e| panic!("{e}\n{}", import.rust));
    }
    let import = py("for x in xs:\n    page.click(x)\npage.reload()");
    assert!(import.diagnostics[0]
        .message
        .contains("a for loop is not converted"));
    has(&import, "page.reload().await?;");
}

#[test]
fn comments_are_kept_and_blank_lines_dropped() {
    let import = js("// open the shop\nawait page.goto('https://x.test/');\n\n/* then reload */\nawait page.reload();");
    has(&import, "// open the shop");
    has(&import, "// then reload");
    let import = py("# open the shop\npage.goto(\"https://x.test/\")");
    has(&import, "// open the shop");
}

// ----------------------------------------------------------------------
// Files: tests, groups, hooks, scripts
// ----------------------------------------------------------------------

#[test]
fn describe_titles_hooks_and_skips() {
    let source = "import { test, expect } from '@playwright/test';\n\
         const BASE = 'https://x.test/';\n\
         test.describe('shop', () => {\n\
           test.beforeEach(async ({ page }) => {\n\
             await page.goto(BASE);\n\
           });\n\
           test('adds to cart', async ({ page }) => {\n\
             await page.getByRole('button', { name: 'Add' }).click();\n\
           });\n\
           test.skip('checks out', async ({ page }) => {\n\
             await page.reload();\n\
           });\n\
           test('adds to cart', async ({ page }) => {\n\
             await page.reload();\n\
           });\n\
         });\n";
    let import = import_playwright(source, &ScriptImportOptions::default()).unwrap();
    complete(&import);
    assert_eq!(
        import.tests,
        [
            "shop_adds_to_cart",
            "shop_checks_out",
            "shop_adds_to_cart_2"
        ]
    );
    assert!(import.rust.contains(
        "#[tokio::test]\nasync fn shop_adds_to_cart() -> Result<(), Box<dyn std::error::Error>> {\n    \
         let browser = Browser::launch(support::launch_options()?).await?;\n    \
         let page = browser.default_page().await?;\n    \
         page.goto(\"https://x.test/\").await?;\n    \
         page.locator(SelectorBuf::role(\"button\", Some(\"Add\"), false)).click().await?;\n    \
         browser.close().await?;\n    Ok(())\n}"
    ), "{}", import.rust);
    assert!(import.rust.contains(
        "#[tokio::test]\n#[ignore = \"skipped with test.skip in the source\"]\nasync fn shop_checks_out()"
    ), "{}", import.rust);
}

#[test]
fn a_test_step_is_inlined() {
    let import =
        js("await test.step('log in', async () => {\n  await page.goto('https://x.test/');\n});");
    complete(&import);
    has(&import, "// step: log in");
    has(&import, "page.goto(\"https://x.test/\").await?;");
}

#[test]
fn javascript_codegen_output() {
    let source = "const { chromium } = require('playwright');\n\n\
         (async () => {\n  const browser = await chromium.launch({\n    headless: false\n  });\n  \
         const context = await browser.newContext();\n  const page = await context.newPage();\n  \
         await page.goto('https://x.test/');\n  \
         await page.getByRole('link', { name: 'About' }).click();\n\n  \
         // ---------------------\n  await context.close();\n  await browser.close();\n})();\n";
    let options = ScriptImportOptions {
        test_name: "codegen".to_owned(),
        ..ScriptImportOptions::default()
    };
    let import = import_playwright(source, &options).unwrap();
    complete(&import);
    assert_eq!(import.tests, ["codegen"]);
    assert_eq!(import.language, ScriptLanguage::JavaScript);
    has(&import, "page.goto(\"https://x.test/\").await?;");
    has(
        &import,
        "page.locator(SelectorBuf::role(\"link\", Some(\"About\"), false)).click().await?;",
    );
    has(&import, "// ---------------------");
}

#[test]
fn python_codegen_sync_and_async() {
    let sync =
        "import re\nfrom playwright.sync_api import Playwright, sync_playwright, expect\n\n\n\
         def run(playwright: Playwright) -> None:\n    \
         browser = playwright.chromium.launch(headless=False)\n    \
         context = browser.new_context()\n    page = context.new_page()\n    \
         page.goto(\"https://x.test/\")\n    \
         page.get_by_role(\"link\", name=\"About\").click()\n\n    \
         # ---------------------\n    context.close()\n    browser.close()\n\n\n\
         with sync_playwright() as playwright:\n    run(playwright)\n";
    let asynchronous = "import asyncio\nimport re\nfrom playwright.async_api import Playwright, async_playwright, expect\n\n\n\
         async def run(playwright: Playwright) -> None:\n    \
         browser = await playwright.chromium.launch(headless=False)\n    \
         context = await browser.new_context()\n    page = await context.new_page()\n    \
         await page.goto(\"https://x.test/\")\n    \
         await page.get_by_role(\"link\", name=\"About\").click()\n    \
         await context.close()\n    await browser.close()\n\n\n\
         async def main() -> None:\n    async with async_playwright() as playwright:\n        \
         await run(playwright)\n\n\nasyncio.run(main())\n";
    for source in [sync, asynchronous] {
        let import = import_playwright(source, &ScriptImportOptions::default()).unwrap();
        complete(&import);
        assert_eq!(import.language, ScriptLanguage::Python);
        assert_eq!(import.tests, ["imported_test"]);
        has(&import, "page.goto(\"https://x.test/\").await?;");
        has(
            &import,
            "page.locator(SelectorBuf::role(\"link\", Some(\"About\"), false)).click().await?;",
        );
    }
}

#[test]
fn python_script_with_its_body_inside_the_with_block() {
    let source = "from playwright.sync_api import sync_playwright\n\n\
         with sync_playwright() as p:\n    browser = p.chromium.launch()\n    \
         page = browser.new_page()\n    page.goto(\"https://x.test/\")\n    \
         print(page.title())\n    browser.close()\n";
    let import = import_playwright(source, &ScriptImportOptions::default()).unwrap();
    complete(&import);
    has(&import, "page.goto(\"https://x.test/\").await?;");
    has(&import, "println!(\"{}\", page.title().await?);");
}

#[test]
fn pytest_skip_marks_and_fixtures() {
    let source = "import pytest\nfrom playwright.sync_api import Page\n\n\
         @pytest.fixture\ndef user():\n    return \"alice\"\n\n\
         @pytest.mark.skip(reason=\"flaky\")\ndef test_a(page: Page):\n    page.reload()\n";
    let import = import_playwright(source, &ScriptImportOptions::default()).unwrap();
    assert!(
        import.rust.contains("#[ignore = \"flaky\"]"),
        "{}",
        import.rust
    );
    assert!(
        import
            .diagnostics
            .iter()
            .any(|d| d.message.contains("pytest fixture `user`")),
        "{:?}",
        import.diagnostics
    );
}

#[test]
fn the_language_comes_from_the_extension_or_the_content() {
    assert_eq!(
        ScriptLanguage::detect("def test_x(page):\n    pass"),
        ScriptLanguage::Python
    );
    assert_eq!(
        ScriptLanguage::detect("test('x', async ({ page }: { page: Page }) => {})"),
        ScriptLanguage::TypeScript
    );
    assert_eq!(
        ScriptLanguage::detect("const x = 1;"),
        ScriptLanguage::JavaScript
    );
    assert_eq!(
        ScriptLanguage::detect("{\"title\": \"r\", \"steps\": []}"),
        ScriptLanguage::RecorderJson
    );
    assert_eq!(
        ScriptLanguage::from_path(Path::new("a.mjs")),
        Some(ScriptLanguage::JavaScript)
    );
}

// ----------------------------------------------------------------------
// Puppeteer
// ----------------------------------------------------------------------

#[test]
fn a_puppeteer_script() {
    let source = "const puppeteer = require('puppeteer');\n\n\
         (async () => {\n  const browser = await puppeteer.launch({ headless: 'new' });\n  \
         const page = await browser.newPage();\n  \
         await page.setViewport({ width: 1280, height: 720 });\n  \
         await page.goto('https://x.test/');\n  \
         await page.type('#email', 'a@b.c');\n  \
         await page.click('#go');\n  \
         await page.select('#color', 'blue');\n  \
         const heading = await page.$eval('h1', el => el.textContent);\n  \
         const items = await page.$$eval('li', els => els.length);\n  \
         console.log(heading, items);\n  \
         await browser.close();\n})();\n";
    let import = import_puppeteer(source, &ScriptImportOptions::default()).unwrap();
    complete(&import);
    for line in [
        "page.window().set_bounds(0, 0, 1280, 720).await?;",
        "page.goto(\"https://x.test/\").await?;",
        "page.locator(\"#email\").type_text(\"a@b.c\").await?;",
        "page.locator(\"#go\").click().await?;",
        "page.locator(\"#color\").select_option(SelectBy::Value(\"blue\")).await?;",
        "let heading = page.locator(\"h1\").text().await?;",
        "let items = page.locator(\"li\").count().await?;",
        "println!(\"{} {}\", heading, items);",
    ] {
        has(&import, line);
    }
    assert!(import
        .rust
        .contains("// Imported from Puppeteer code by `sbrs import-puppeteer`."));
}

#[test]
fn puppeteer_page_methods() {
    for (body, expected) in [
        (
            "await page.waitForSelector('#x');",
            "page.locator(\"#x\").wait_for(State::Present).await?;",
        ),
        (
            "await page.waitForSelector('#x', { visible: true });",
            "page.locator(\"#x\").wait_for(State::Visible).await?;",
        ),
        (
            "await page.waitForSelector('#x', { hidden: true });",
            "page.locator(\"#x\").wait_for(State::Hidden).await?;",
        ),
        (
            "await page.waitForNavigation();",
            "page.wait_for_load().await?;",
        ),
        (
            "await page.waitForNetworkIdle();",
            "page.wait_for_load().await?;",
        ),
        (
            "await page.waitForTimeout(250);",
            "tokio::time::sleep(std::time::Duration::from_millis(250)).await;",
        ),
        (
            "await new Promise(r => setTimeout(r, 300));",
            "tokio::time::sleep(std::time::Duration::from_millis(300)).await;",
        ),
        (
            "await new Promise((resolve) => { setTimeout(resolve, 40); });",
            "tokio::time::sleep(std::time::Duration::from_millis(40)).await;",
        ),
        (
            "await page.setUserAgent('UA');",
            "page.emulation().user_agent(\"UA\", None).await?;",
        ),
        (
            "const html = await page.content();\nconsole.log(html);",
            "let html = page.content().await?;",
        ),
        (
            "await page.focus('#a');",
            "page.locator(\"#a\").focus().await?;",
        ),
        (
            "await page.hover('#a');",
            "page.locator(\"#a\").hover().await?;",
        ),
        (
            "await page.keyboard.press('Tab');",
            "page.keyboard().press(Key::Tab).await?;",
        ),
        (
            "await page.keyboard.type('hi');",
            "page.keyboard().type_text(\"hi\").await?;",
        ),
        (
            "await page.mouse.click(5, 6);",
            "page.mouse().click(Point::new(5.0, 6.0)).await?;",
        ),
        ("await page.reload();", "page.reload().await?;"),
        ("await page.goBack();", "page.back().await?;"),
        ("await page.goForward();", "page.forward().await?;"),
        (
            "await page.screenshot({ path: 'p.png' });",
            "std::fs::write(\"p.png\", page.screenshot().await?)?;",
        ),
        (
            "await page.setCookie({ name: 'a', value: 'b', domain: 'x.test' });",
            "page.cookies().set(&[Cookie::new(\"a\", \"b\").domain(\"x.test\")]).await?;",
        ),
        (
            "const t = await page.evaluate(() => document.title);\nconsole.log(t);",
            "let t = page.evaluate(\"(() => document.title)()\").await?;",
        ),
    ] {
        let import = pptr(body);
        complete(&import);
        has(&import, expected);
    }
}

#[test]
fn promise_all_waits_after_the_action_that_navigates() {
    let import = pptr("await Promise.all([page.waitForNavigation(), page.click('a')]);");
    complete(&import);
    let click = import
        .rust
        .find("page.locator(\"a\").click().await?;")
        .unwrap();
    let wait = import.rust.find("page.wait_for_load().await?;").unwrap();
    assert!(click < wait, "{}", import.rust);
}

#[test]
fn element_handles_and_the_locator_api() {
    let import = pptr("const el = await page.$('#go');\nawait el.click();\nawait el.type('x');\n\
         await el.uploadFile('a.txt');\nconst items = await page.$$('li');\nexpect(items.length).toBe(2);\n\
         await page.locator('#q').fill('x');\nawait page.locator('#q').hover();\nawait page.locator('#q').wait();\n\
         await page.locator('#q').click();");
    complete(&import);
    for line in [
        "let el = page.locator(\"#go\");",
        "el.click().await?;",
        "el.type_text(\"x\").await?;",
        "support::set_input_files(&el, \"a.txt\").await?;",
        "let items = page.locator(\"li\");",
        "assert_eq!(items.count().await?, 2);",
        "page.locator(\"#q\").fill(\"x\").await?;",
        "page.locator(\"#q\").hover().await?;",
        "page.locator(\"#q\").wait_for(State::Visible).await?;",
        "page.locator(\"#q\").click().await?;",
    ] {
        has(&import, line);
    }
}

#[test]
fn puppeteer_selectors() {
    for (selector, expected) in [
        (
            "xpath///button",
            "page.locator(\"//button\").click().await?;",
        ),
        (
            "text/Go",
            "page.locator(SelectorBuf::text(\"Go\", false)).click().await?;",
        ),
        (
            "aria/Sign in[role=\"button\"]",
            "page.locator(SelectorBuf::role(\"button\", Some(\"Sign in\"), true)).click().await?;",
        ),
        (
            "::-p-text(Go)",
            "page.locator(SelectorBuf::text(\"Go\", false)).click().await?;",
        ),
        (
            "::-p-xpath(//button)",
            "page.locator(\"//button\").click().await?;",
        ),
        (
            "::-p-aria(Sign in)",
            "page.locator(SelectorBuf::role(\"\", Some(\"Sign in\"), true)).click().await?;",
        ),
        (
            "button::-p-text(Go)",
            "page.locator(\"button\").locator(SelectorBuf::has_text(\"Go\")).click().await?;",
        ),
    ] {
        let import = pptr(&format!("await page.click('{selector}');"));
        complete(&import);
        has(&import, expected);
    }
    let pierce = pptr("await page.click('pierce/#x');");
    assert!(pierce.diagnostics[0].message.contains("shadow"));
}

#[test]
fn jest_chai_and_node_assertions() {
    let import = pptr("const heading = await page.$eval('h1', el => el.textContent);\n\
         const shown = await page.$eval('#a', el => el.checked);\n\
         const n = await page.$$eval('li', els => els.length);\n\
         expect(heading).toBe('Shop');\nexpect(heading).toEqual('Shop');\nexpect(heading).toContain('Sh');\n\
         expect(heading).toMatch(/Sh/);\nexpect(heading).not.toBe('Old');\n\
         assert.strictEqual(heading, 'Shop');\nassert.equal(n, 3);\nassert.ok(shown);\nassert(shown);\n\
         expect(heading).to.equal('Shop');\nexpect(heading).to.include('Sh');\nexpect(shown).to.be.true;\n\
         expect(heading).to.not.equal('Old');\nexpect(n).to.be.above(1);");
    complete(&import);
    for line in [
        "assert_eq!(heading, \"Shop\");",
        "assert!(heading.contains(\"Sh\"));",
        "assert_ne!(heading, \"Old\");",
        "assert_eq!(n, 3);",
        "assert!(shown);",
        "assert!(n > 1);",
    ] {
        has(&import, line);
    }
}

#[test]
fn jest_puppeteer_matchers() {
    for (body, expected) in [
        (
            "await expect(page).toMatchElement('h1', { text: 'Shop' });",
            "page.locator(\"h1\").locator(SelectorBuf::has_text(\"Shop\")).expect().to_exist().await?;",
        ),
        (
            "await expect(page).toClick('button', { text: 'Go' });",
            "page.locator(\"button\").locator(SelectorBuf::has_text(\"Go\")).click().await?;",
        ),
        ("await expect(page).toFill('#q', 'x');", "page.locator(\"#q\").fill(\"x\").await?;"),
        (
            "await expect(page).toMatchTextContent('Shop');",
            "page.locator(\"body\").expect().to_contain_text(\"Shop\").await?;",
        ),
    ] {
        let import = pptr(body);
        complete(&import);
        has(&import, expected);
    }
}

#[test]
fn mocha_hooks_assign_the_page() {
    let source = "const puppeteer = require('puppeteer');\nconst { expect } = require('chai');\n\n\
         describe('shop', function () {\n  this.timeout(10000);\n  let browser, page;\n\n  \
         before(async () => {\n    browser = await puppeteer.launch();\n    page = await browser.newPage();\n  });\n\n  \
         after(async () => {\n    await browser.close();\n  });\n\n  \
         it('has a title', async () => {\n    await page.goto('https://x.test/');\n    \
         const title = await page.title();\n    expect(title).to.equal('Shop');\n  });\n});\n";
    let import = import_puppeteer(source, &ScriptImportOptions::default()).unwrap();
    complete(&import);
    assert_eq!(import.tests, ["shop_has_a_title"]);
    has(&import, "page.goto(\"https://x.test/\").await?;");
    has(&import, "assert_eq!(title, \"Shop\");");
}

#[test]
fn puppeteer_extra_stealth_is_a_todo() {
    let source = "const puppeteer = require('puppeteer-extra');\n\
         const StealthPlugin = require('puppeteer-extra-plugin-stealth');\n\
         puppeteer.use(StealthPlugin());\n\
         (async () => {\n  const browser = await puppeteer.launch();\n  const page = await browser.newPage();\n  \
         await page.goto('https://x.test/');\n  await browser.close();\n})();\n";
    let import = import_puppeteer(source, &ScriptImportOptions::default()).unwrap();
    assert!(!import.is_complete());
    assert!(
        import.diagnostics[0]
            .message
            .contains("puppeteer-extra plugins"),
        "{:?}",
        import.diagnostics
    );
    has(&import, "page.goto(\"https://x.test/\").await?;");
}

#[test]
fn a_recorder_json_export() {
    let source = r##"{
  "title": "Checkout flow",
  "steps": [
    { "type": "setViewport", "width": 800, "height": 600, "deviceScaleFactor": 1, "isMobile": false, "hasTouch": false, "isLandscape": false },
    { "type": "navigate", "url": "https://x.test/", "assertedEvents": [{ "type": "navigation", "url": "https://x.test/", "title": "Shop" }] },
    { "type": "click", "target": "main", "selectors": [["aria/Sign in"], ["#go"], ["xpath///*[@id=\"go\"]"], ["pierce/#go"], ["text/Sign in"]], "offsetX": 5, "offsetY": 5, "assertedEvents": [{ "type": "navigation" }] },
    { "type": "change", "value": "alice", "selectors": [["#user"]], "target": "main" },
    { "type": "keyDown", "target": "main", "key": "Enter" },
    { "type": "keyUp", "target": "main", "key": "Enter" },
    { "type": "waitForElement", "selectors": [["#done"]] },
    { "type": "customStep", "name": "x", "parameters": {} }
  ]
}"##;
    let import = import_puppeteer(source, &ScriptImportOptions::default()).unwrap();
    assert_eq!(import.language, ScriptLanguage::RecorderJson);
    assert_eq!(import.tests, ["checkout_flow"]);
    for line in [
        "page.window().set_bounds(0, 0, 800, 600).await?;",
        "page.goto(\"https://x.test/\").await?;",
        "page.locator(\"#go\").click().await?;",
        "page.wait_for_load().await?;",
        "support::change(&page.locator(\"#user\"), \"alice\").await?;",
        "page.keyboard().press(Key::Enter).await?;",
        "page.locator(\"#done\").wait_for(State::Visible).await?;",
    ] {
        has(&import, line);
    }
    assert_eq!(import.diagnostics.len(), 1, "{:?}", import.diagnostics);
    assert_eq!(import.diagnostics[0].line, 11);
    assert!(import.diagnostics[0].message.contains("customStep"));
    syn::parse_file(&import.rust).unwrap();
}

#[test]
fn the_wrong_importer_says_which_one_to_use() {
    let json = "{\"title\": \"r\", \"steps\": []}";
    let error = import_playwright(json, &ScriptImportOptions::default()).unwrap_err();
    assert!(error.to_string().contains("import-puppeteer"), "{error}");
    let python = ScriptImportOptions {
        language: Some(ScriptLanguage::Python),
        ..ScriptImportOptions::default()
    };
    let error = import_puppeteer("page.goto('x')", &python).unwrap_err();
    assert!(error.to_string().contains("import-playwright"), "{error}");
}

// ----------------------------------------------------------------------
// Hostile input
// ----------------------------------------------------------------------

/// The generated code with string literals and comments removed.
fn code_outside_literals(rust: &str) -> String {
    let mut out = String::new();
    let mut chars = rust.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '"' => {
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
                out.push_str("\"\"");
            }
            c => out.push(c),
        }
    }
    out
}

#[test]
fn hostile_text_stays_inside_string_literals_and_comments() {
    let hostile = [
        r#"'\"); evil(); (\"'"#,
        r##"'\"#; evil(); r#\"'"##,
        "'*/ evil(); /*'",
        r"'line\nbreak evil();'",
        r"'\u{202e} evil(); \u2028 evil();'",
        r"'nul\0 evil();'",
        r"'back\\slash\\\\ evil();'",
    ];
    for text in hostile {
        let body = format!(
            "await page.goto({text});\n\
             await page.locator({text}).fill({text});\n\
             await expect(page).toHaveTitle({text});\n\
             await page.route({text}, r => r.abort()); // {text} evil();\n\
             /* {text} */"
        );
        for import in [js(&body), pptr(&body)] {
            syn::parse_file(&import.rust)
                .unwrap_or_else(|e| panic!("{text}: {e}\n{}", import.rust));
            let code = code_outside_literals(&import.rust);
            assert!(!code.contains("evil"), "{text}:\n{}", import.rust);
        }
    }
    // Python string forms, and a test title.
    let source = "from playwright.sync_api import Page\n\n\
         def test_evil(page: Page):\n    page.goto(r'\\\"); evil(); (\"')\n    page.goto('''a\n\"\"\" evil();''')\n";
    let import = import_playwright(source, &ScriptImportOptions::default()).unwrap();
    syn::parse_file(&import.rust).unwrap_or_else(|e| panic!("{e}\n{}", import.rust));
    assert!(
        !code_outside_literals(&import.rust).contains("evil();"),
        "{}",
        import.rust
    );
    let titled = import_playwright(
        "test('x\"); evil(); //\\n\\u202e', async ({ page }) => { await page.reload(); });",
        &ScriptImportOptions::default(),
    )
    .unwrap();
    // The title's escapes are decoded first; what is left is letters only.
    assert_eq!(titled.tests, ["x_evil"]);
    syn::parse_file(&titled.rust).unwrap();
}

#[test]
fn bidirectional_and_invisible_characters_are_escaped() {
    let import = js("await page.goto('a\u{202e}b\u{2066}c\u{200b}d');\n// note \u{202e} here");
    assert!(
        import
            .rust
            .contains("page.goto(\"a\\u{202e}b\\u{2066}c\\u{200b}d\")"),
        "{}",
        import.rust
    );
    assert!(
        import.rust.contains("// note \\u{202e} here"),
        "{}",
        import.rust
    );
    assert!(!import.rust.contains('\u{202e}'));
}

#[test]
fn very_long_lines_and_deep_nesting() {
    let long = "x".repeat(100_000);
    let import = js(&format!(
        "await page.goto('{long}');\nawait page.route('{long}', r => r);"
    ));
    has(&import, &format!("page.goto(\"{long}\").await?;"));
    let todo = import
        .rust
        .lines()
        .find(|l| l.contains("TODO(playwright)"))
        .unwrap();
    assert!(
        todo.len() < 400,
        "a TODO quotes at most 240 characters: {}",
        todo.len()
    );

    let deep = format!(
        "await page.goto({}'x'{});\nawait page.reload();",
        "(".repeat(10_000),
        ")".repeat(10_000)
    );
    let import = js(&deep);
    assert!(
        import
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nested more than")),
        "{:?}",
        import.diagnostics
    );
    has(&import, "page.reload().await?;");
    let deep_py = format!(
        "page.goto({}'x'{})\npage.reload()",
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    let import = py(&deep_py);
    has(&import, "page.reload().await?;");
}

#[test]
fn a_file_over_2_mb_is_refused() {
    let big = format!("// {}\n", "x".repeat(MAX_SOURCE_BYTES));
    let error = import_playwright(&big, &ScriptImportOptions::default()).unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::Unsupported(_)),
        "{error:?}"
    );
    assert!(error.to_string().contains("2 MB"), "{error}");
}

#[test]
fn an_unclosed_string_names_its_line() {
    let error = import_playwright(
        "test('x', async () => {\n  await page.goto('open\n});",
        &ScriptImportOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::Parse { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("line 2"), "{error}");
}
