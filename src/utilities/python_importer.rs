//! Static migration of common SeleniumBase and Selenium Python tests to Rust.

use std::collections::HashMap;

use regex::Regex;

use crate::utilities::importer::pyconv;
use crate::utils::selectors::is_xpath_selector;

/// Python API family used by the input file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PythonSource {
    /// Detect the API family from imports and calls.
    #[default]
    Auto,
    /// Python SeleniumBase (`BaseCase`, `SB`, or `self` calls).
    SeleniumBase,
    /// Selenium WebDriver Python.
    Selenium,
}

/// Import configuration.
#[derive(Clone, Debug)]
pub struct ImportOptions {
    pub source: PythonSource,
    pub test_name: String,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            source: PythonSource::Auto,
            test_name: "imported_python_test".to_owned(),
        }
    }
}

/// Diagnostic severity emitted during conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportSeverity {
    Warning,
    Error,
}

/// A source-located migration diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDiagnostic {
    pub line: usize,
    pub severity: ImportSeverity,
    pub message: String,
}

/// Generated Rust and diagnostics from one Python source file.
#[derive(Clone, Debug)]
pub struct ImportResult {
    pub rust: String,
    pub diagnostics: Vec<ImportDiagnostic>,
    pub detected_source: PythonSource,
}

impl ImportResult {
    /// Returns true when no error-level diagnostics were emitted.
    pub fn is_complete(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ImportSeverity::Error)
    }
}

#[derive(Clone, Debug)]
enum Action {
    Open(String),
    Click(String),
    ClickLink(String),
    ClickPartialLink(String),
    Clear(String),
    Submit(String),
    AddText(String, String),
    AssertTitle(String),
    AssertTitleContains(String),
    GetText(String),
    GetAttribute(String, String),
    Maximize,
    Back,
    Forward,
    Refresh,
    Todo(usize, String),
}

#[derive(Clone, Debug)]
enum Locator {
    /// A selector `BaseCase` classifies itself: CSS or XPath.
    Selector(String),
    LinkText(String),
    PartialLinkText(String),
}

/// Converts common SeleniumBase or Selenium Python statements into a Rust test.
///
/// Dynamic Python, custom helpers, and unsupported locators remain visible as
/// source-located `TODO` comments and diagnostics.
pub fn import_python(source: &str, options: &ImportOptions) -> ImportResult {
    let detected_source = detect_source(source, options.source);
    if detected_source == PythonSource::SeleniumBase {
        let converted = pyconv::convert(source, &options.test_name);
        return ImportResult {
            rust: converted.rust,
            diagnostics: converted.diagnostics,
            detected_source,
        };
    }
    let mut diagnostics = Vec::new();
    let mut elements = HashMap::new();
    let mut actions = Vec::new();

    for (line, statement) in statements(source) {
        let trimmed = statement.trim();
        if should_ignore(trimmed) {
            continue;
        }

        match parse_selenium(trimmed, line, &mut elements) {
            ParseOutcome::Action(action) => actions.push(action),
            ParseOutcome::Element(name, locator) => {
                elements.insert(name, locator);
            }
            ParseOutcome::Ignored => {}
            ParseOutcome::Unsupported(message) => {
                diagnostics.push(ImportDiagnostic {
                    line,
                    severity: ImportSeverity::Warning,
                    message: message.clone(),
                });
                actions.push(Action::Todo(line, message));
            }
            ParseOutcome::Invalid(message) => {
                diagnostics.push(ImportDiagnostic {
                    line,
                    severity: ImportSeverity::Error,
                    message: message.clone(),
                });
                actions.push(Action::Todo(line, message));
            }
        }
    }

    ImportResult {
        rust: render_test(&options.test_name, &actions),
        diagnostics,
        detected_source,
    }
}

enum ParseOutcome {
    Action(Action),
    Element(String, Locator),
    Ignored,
    Unsupported(String),
    Invalid(String),
}

fn detect_source(source: &str, requested: PythonSource) -> PythonSource {
    if requested != PythonSource::Auto {
        return requested;
    }
    // The import line, not the word: a Selenium script can mention
    // `seleniumbase.io` in a URL.
    if source.contains("from seleniumbase")
        || source.contains("import seleniumbase")
        || source.contains("BaseCase")
        || source.contains("with SB(")
    {
        PythonSource::SeleniumBase
    } else {
        PythonSource::Selenium
    }
}

fn should_ignore(statement: &str) -> bool {
    statement.is_empty()
        || statement.starts_with('#')
        || statement.starts_with("import ")
        || statement.starts_with("from ")
        || statement.starts_with('@')
        || statement.starts_with("def ")
        || statement.starts_with("class ")
        || statement.starts_with("with SB(")
        || statement == "pass"
}

fn parse_selenium(
    statement: &str,
    line: usize,
    elements: &mut HashMap<String, Locator>,
) -> ParseOutcome {
    if let Some(captures) = regex(
        r"^(?P<name>[A-Za-z_]\w*)\s*=\s*(?:driver|self\.driver)\.find_element\((?P<args>.+)\)$",
    )
    .captures(statement)
    {
        let name = captures["name"].to_owned();
        return match parse_locator(&captures["args"]) {
            Ok(locator) => ParseOutcome::Element(name, locator),
            Err(message) => ParseOutcome::Invalid(message),
        };
    }

    if let Some(captures) =
        regex(r"^(?:driver|self\.driver)\.get\((?P<url>.+)\)$").captures(statement)
    {
        return match python_string(&captures["url"]) {
            Some(url) => ParseOutcome::Action(Action::Open(url)),
            None => ParseOutcome::Invalid("driver.get() requires a string literal URL".to_owned()),
        };
    }

    if statement == "driver.maximize_window()" || statement == "self.driver.maximize_window()" {
        return ParseOutcome::Action(Action::Maximize);
    }
    if statement == "driver.back()" || statement == "self.driver.back()" {
        return ParseOutcome::Action(Action::Back);
    }
    if statement == "driver.forward()" || statement == "self.driver.forward()" {
        return ParseOutcome::Action(Action::Forward);
    }
    if statement == "driver.refresh()" || statement == "self.driver.refresh()" {
        return ParseOutcome::Action(Action::Refresh);
    }

    if let Some(captures) =
        regex(r#"^assert\s+(?P<value>.+?)\s+in\s+(?:driver|self\.driver)\.title$"#)
            .captures(statement)
    {
        return python_string(&captures["value"]).map_or_else(
            || ParseOutcome::Invalid("title assertion must use a string literal".to_owned()),
            |value| ParseOutcome::Action(Action::AssertTitleContains(value)),
        );
    }
    if let Some(captures) =
        regex(r#"^assert\s+(?:driver|self\.driver)\.title\s*==\s*(?P<value>.+)$"#)
            .captures(statement)
    {
        return python_string(&captures["value"]).map_or_else(
            || ParseOutcome::Invalid("title assertion must use a string literal".to_owned()),
            |value| ParseOutcome::Action(Action::AssertTitle(value)),
        );
    }

    if let Some(captures) = regex(
        r"^(?:driver|self\.driver)\.find_element\((?P<locator>.+)\)\.(?P<method>click|clear|submit|send_keys)\((?P<args>.*)\)$",
    )
    .captures(statement)
    {
        return selenium_element_action(
            parse_locator(&captures["locator"]),
            &captures["method"],
            &captures["args"],
        );
    }

    if let Some(captures) =
        regex(r"^(?P<name>[A-Za-z_]\w*)\.(?P<method>click|clear|submit|send_keys)\((?P<args>.*)\)$")
            .captures(statement)
    {
        let Some(locator) = elements.get(&captures["name"]).cloned() else {
            return ParseOutcome::Unsupported(format!(
                "element variable '{}' was not created by a supported find_element call",
                &captures["name"]
            ));
        };
        return selenium_element_action(Ok(locator), &captures["method"], &captures["args"]);
    }

    if let Some(captures) = regex(
        r"^(?:[A-Za-z_]\w*\s*=\s*)?(?P<name>[A-Za-z_]\w*)\.(?P<method>text|get_attribute)\s*(?:\((?P<args>.*)\))?$",
    )
    .captures(statement)
    {
        let Some(locator) = elements.get(&captures["name"]).cloned() else {
            return ParseOutcome::Unsupported(format!(
                "element variable '{}' was not created by a supported find_element call",
                &captures["name"]
            ));
        };
        return match (locator, &captures["method"]) {
            (Locator::Selector(css), "text") => ParseOutcome::Action(Action::GetText(css)),
            (Locator::Selector(css), "get_attribute") => match python_string(&captures["args"]) {
                Some(attr) => ParseOutcome::Action(Action::GetAttribute(css, attr)),
                None => ParseOutcome::Invalid(
                    "get_attribute() requires a string literal attribute name".to_owned(),
                ),
            },
            _ => ParseOutcome::Unsupported(
                "this read operation is only supported for element variables with a CSS or XPath locator".to_owned(),
            ),
        };
    }

    if statement.starts_with("driver.quit(")
        || statement.starts_with("driver.close(")
        || statement.starts_with("self.driver.quit(")
    {
        return ParseOutcome::Ignored;
    }

    ParseOutcome::Unsupported(format!(
        "unsupported Selenium statement at line {line}: {}",
        one_line(statement)
    ))
}

fn selenium_element_action(
    locator: Result<Locator, String>,
    method: &str,
    args: &str,
) -> ParseOutcome {
    let locator = match locator {
        Ok(locator) => locator,
        Err(message) => return ParseOutcome::Invalid(message),
    };
    match (locator, method) {
        (Locator::Selector(css), "click") => ParseOutcome::Action(Action::Click(css)),
        (Locator::LinkText(text), "click") => ParseOutcome::Action(Action::ClickLink(text)),
        (Locator::PartialLinkText(text), "click") => {
            ParseOutcome::Action(Action::ClickPartialLink(text))
        }
        (Locator::Selector(css), "clear") => ParseOutcome::Action(Action::Clear(css)),
        (Locator::Selector(css), "submit") => ParseOutcome::Action(Action::Submit(css)),
        // Selenium's `send_keys` adds to what is in the field.
        (Locator::Selector(css), "send_keys") => match python_string(args) {
            Some(text) => ParseOutcome::Action(Action::AddText(css, text)),
            None => ParseOutcome::Invalid(
                "send_keys() requires a string literal; key chords need manual migration"
                    .to_owned(),
            ),
        },
        (_, action) => ParseOutcome::Unsupported(format!(
            "locator is not supported for Selenium action '{action}'"
        )),
    }
}

fn parse_locator(input: &str) -> Result<Locator, String> {
    let args = split_args(input);
    if args.len() != 2 {
        return Err("find_element() must contain a By strategy and string literal".to_owned());
    }
    let value = python_string(&args[1])
        .ok_or_else(|| "dynamic Selenium locators need manual migration".to_owned())?;
    let strategy = args[0].trim().rsplit('.').next().unwrap_or_default();
    match strategy {
        "CSS_SELECTOR" => Ok(Locator::Selector(value)),
        "ID" => Ok(Locator::Selector(format!(r#"[id="{}"]"#, css_escape(&value)))),
        "NAME" => Ok(Locator::Selector(format!(r#"[name="{}"]"#, css_escape(&value)))),
        "CLASS_NAME" if !value.chars().any(char::is_whitespace) => {
            Ok(Locator::Selector(format!(".{}", css_escape(&value))))
        }
        "TAG_NAME" => Ok(Locator::Selector(value)),
        // `BaseCase` takes XPath as it is; converting it to CSS would refuse
        // every XPath CSS cannot say.
        "XPATH" if is_xpath_selector(&value) => Ok(Locator::Selector(value)),
        "XPATH" => Err(format!(
            "XPath '{value}' does not start with '/', './', '../' or '(', so BaseCase would read it as CSS"
        )),
        "LINK_TEXT" => Ok(Locator::LinkText(value)),
        "PARTIAL_LINK_TEXT" => Ok(Locator::PartialLinkText(value)),
        _ => Err(format!(
            "unsupported Selenium locator strategy '{}'",
            args[0]
        )),
    }
}

fn statements(source: &str) -> Vec<(usize, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;
    let mut depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;

    for (index, line) in source.lines().enumerate() {
        // A `#` outside a string starts a comment. What follows it is prose,
        // so an apostrophe there must not open a string that swallows the
        // lines after it.
        let mut code_end = line.len();
        for (at, character) in line.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' && quote.is_some() {
                escaped = true;
                continue;
            }
            if matches!(character, '\'' | '"') {
                if quote == Some(character) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(character);
                }
                continue;
            }
            if quote.is_none() {
                match character {
                    '#' => {
                        code_end = at;
                        break;
                    }
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
            }
        }
        if current.is_empty() {
            start_line = index + 1;
        } else {
            current.push(' ');
        }
        current.push_str(line[..code_end].trim());
        if depth <= 0 && quote.is_none() {
            result.push((start_line, std::mem::take(&mut current)));
            depth = 0;
        }
    }
    if !current.trim().is_empty() {
        result.push((start_line, current));
    }
    result
}

fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            current.push(character);
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            current.push(character);
            continue;
        }
        if quote.is_none() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(current.trim().to_owned());
                    current.clear();
                    continue;
                }
                _ => {}
            }
        }
        current.push(character);
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_owned());
    }
    args
}

fn python_string(input: &str) -> Option<String> {
    let input = input.trim();
    let input = input
        .strip_prefix('r')
        .or_else(|| input.strip_prefix('u'))
        .unwrap_or(input);
    let quote = input.chars().next()?;
    if !matches!(quote, '\'' | '"') || !input.ends_with(quote) || input.len() < 2 {
        return None;
    }
    let mut output = String::new();
    let mut chars = input[1..input.len() - 1].chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = chars.next()?;
        output.push(match escaped {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            other => other,
        });
    }
    Some(output)
}

fn render_test(test_name: &str, actions: &[Action]) -> String {
    let lines: Vec<String> = actions.iter().map(render_action).collect();
    pyconv::render_lines(test_name, &lines)
}

fn render_action(action: &Action) -> String {
    match action {
        Action::Open(url) => format!("sb.open({url:?}).await?;"),
        Action::Click(css) => format!("sb.click({css:?}).await?;"),
        Action::ClickLink(text) => format!("sb.click_link_text({text:?}).await?;"),
        Action::ClickPartialLink(text) => {
            format!("sb.click_partial_link_text({text:?}).await?;")
        }
        Action::Clear(css) => format!("sb.clear({css:?}).await?;"),
        Action::Submit(css) => format!("sb.submit({css:?}).await?;"),
        Action::AddText(css, text) => format!("sb.add_text({css:?}, {text:?}).await?;"),
        Action::AssertTitle(title) => format!("sb.assert_title({title:?}).await?;"),
        Action::AssertTitleContains(title) => {
            format!("sb.assert_title_contains({title:?}).await?;")
        }
        Action::GetText(css) => format!("let _text = sb.get_text({css:?}).await?;"),
        Action::GetAttribute(css, attr) => {
            format!("let _value = sb.get_attribute({css:?}, {attr:?}).await?;")
        }
        Action::Maximize => "sb.maximize_window().await?;".to_owned(),
        Action::Back => "sb.go_back().await?;".to_owned(),
        Action::Forward => "sb.go_forward().await?;".to_owned(),
        Action::Refresh => "sb.refresh().await?;".to_owned(),
        Action::Todo(line, message) => {
            format!("// TODO(source line {line}): {}", one_line(message))
        }
    }
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("importer regex must compile")
}

fn css_escape(value: &str) -> String {
    value.replace('\\', r"\\").replace('"', r#"\""#)
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Compile check. `cargo test --doc --features testing` builds what the importer
// writes for the Python examples exactly as a user's crate would, so a mapping
// that drifts from the API fails here. Each golden is a module of its own, as
// every file is a test target. The Python files are beside them, and
// `pyconv::tests` checks that the importer still writes these goldens.
#[cfg(all(doctest, feature = "testing"))]
#[doc = concat!(
    "```no_run,test_harness\n",
    "mod handle_alert_test {\n",
    include_str!("../../tests/golden/import_python/handle_alert_test.rs.golden"),
    "}\n",
    "mod iframe_tests {\n",
    include_str!("../../tests/golden/import_python/iframe_tests.rs.golden"),
    "}\n",
    "mod my_first_test {\n",
    include_str!("../../tests/golden/import_python/my_first_test.rs.golden"),
    "}\n",
    "mod raw_basic_cdp {\n",
    include_str!("../../tests/golden/import_python/raw_basic_cdp.rs.golden"),
    "}\n",
    "mod raw_cdp_swag {\n",
    include_str!("../../tests/golden/import_python/raw_cdp_swag.rs.golden"),
    "}\n",
    "mod raw_cdp_tabs {\n",
    include_str!("../../tests/golden/import_python/raw_cdp_tabs.rs.golden"),
    "}\n",
    "mod raw_cdp_xpath {\n",
    include_str!("../../tests/golden/import_python/raw_cdp_xpath.rs.golden"),
    "}\n",
    "mod raw_demo_site {\n",
    include_str!("../../tests/golden/import_python/raw_demo_site.rs.golden"),
    "}\n",
    "mod raw_drag_and_drop {\n",
    include_str!("../../tests/golden/import_python/raw_drag_and_drop.rs.golden"),
    "}\n",
    "mod shadow_root_test {\n",
    include_str!("../../tests/golden/import_python/shadow_root_test.rs.golden"),
    "}\n",
    "mod test_assert_elements {\n",
    include_str!("../../tests/golden/import_python/test_assert_elements.rs.golden"),
    "}\n",
    "mod test_checkboxes {\n",
    include_str!("../../tests/golden/import_python/test_checkboxes.rs.golden"),
    "}\n",
    "mod test_coffee_cart {\n",
    include_str!("../../tests/golden/import_python/test_coffee_cart.rs.golden"),
    "}\n",
    "mod test_contains_selector {\n",
    include_str!("../../tests/golden/import_python/test_contains_selector.rs.golden"),
    "}\n",
    "mod test_cycle_elements {\n",
    include_str!("../../tests/golden/import_python/test_cycle_elements.rs.golden"),
    "}\n",
    "mod test_deferred_asserts {\n",
    include_str!("../../tests/golden/import_python/test_deferred_asserts.rs.golden"),
    "}\n",
    "mod test_demo_site {\n",
    include_str!("../../tests/golden/import_python/test_demo_site.rs.golden"),
    "}\n",
    "mod test_double_click {\n",
    include_str!("../../tests/golden/import_python/test_double_click.rs.golden"),
    "}\n",
    "mod test_drag_and_drop {\n",
    include_str!("../../tests/golden/import_python/test_drag_and_drop.rs.golden"),
    "}\n",
    "mod test_highlight_elements {\n",
    include_str!("../../tests/golden/import_python/test_highlight_elements.rs.golden"),
    "}\n",
    "mod test_select_options {\n",
    include_str!("../../tests/golden/import_python/test_select_options.rs.golden"),
    "}\n",
    "mod test_shadow_dom {\n",
    include_str!("../../tests/golden/import_python/test_shadow_dom.rs.golden"),
    "}\n",
    "mod test_simple_login {\n",
    include_str!("../../tests/golden/import_python/test_simple_login.rs.golden"),
    "}\n",
    "mod test_tinymce {\n",
    include_str!("../../tests/golden/import_python/test_tinymce.rs.golden"),
    "}\n",
    "mod test_todomvc {\n",
    include_str!("../../tests/golden/import_python/test_todomvc.rs.golden"),
    "}\n",
    "mod test_url_asserts {\n",
    include_str!("../../tests/golden/import_python/test_url_asserts.rs.golden"),
    "}\n",
    "mod test_window_switching {\n",
    include_str!("../../tests/golden/import_python/test_window_switching.rs.golden"),
    "}\n",
    "mod xpath_test {\n",
    include_str!("../../tests/golden/import_python/xpath_test.rs.golden"),
    "}\n",
    "fn main() {}\n```"
)]
struct ImportGoldensCompile;

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust for the statements `body` inside one test method of a
    /// `BaseCase` class.
    fn in_class(body: &str) -> ImportResult {
        let mut source = String::from(
            "from seleniumbase import BaseCase\n\n\nclass T(BaseCase):\n    def test_x(self):\n",
        );
        for line in body.lines() {
            source.push_str("        ");
            source.push_str(line);
            source.push('\n');
        }
        import_python(&source, &ImportOptions::default())
    }

    #[test]
    fn print_of_an_f_string_is_one_println_and_a_text_index_is_parsed() {
        let result = in_class(
            "name = \"x\"\n\
             print(f\"Hello {name}\")\n\
             indexes = [\"0\", \"1\"]\n\
             for i in indexes:\n\
             \x20   self.select_option_by_index(\"#s\", i)",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "println!(\"Hello {}\", name);",
                "sb.select_option_by_index(\"#s\", i.parse().expect(\"a whole number\")).await?;",
            ],
        );
    }

    #[test]
    fn assert_in_follows_the_membership_trait() {
        let result = in_class(
            "title = self.get_title()\n\
             self.assert_in(\"jq\", title.lower())\n\
             self.assert_in(\"a\", [\"a\", \"b\"])",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "sb.assert_in(\"jq\", &title.to_lowercase())?;",
                "sb.assert_in(\"a\".to_owned(), &[\"a\", \"b\"].map(String::from).to_vec())?;",
            ],
        );
    }

    #[test]
    fn a_timeout_of_wait_for_element_and_an_attribute_of_the_options_are_builder_methods() {
        let result = in_class(
            "self.wait_for_element(\"#a\", timeout=3)\n\
             self.get_select_options(\"#s\", attribute=\"value\")",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "sb.wait_for_element(\"#a\").timeout(std::time::Duration::from_secs(3)).await?;",
                "sb.get_select_options(\"#s\").attribute(\"value\").await?;",
            ],
        );
    }

    #[test]
    fn pure_cdp_assertions_are_rust_assertions() {
        let result = import_python(
            "from seleniumbase import sb_cdp\n\
             sb = sb_cdp.Chrome()\n\
             sb.assert_true(sb.is_selected(\"#a\"))\n\
             sb.assert_false(sb.is_selected(\"#a\"))\n\
             sb.assert_equal(sb.get_text(\"h1\"), \"Hi\")",
            &ImportOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "let selected = page.locator(\"#a\").is_selected().await?;",
                "assert!(selected);",
                "assert!(!selected_2);",
                "assert_eq!(text.as_str(), \"Hi\");",
            ],
        );
    }

    #[test]
    fn a_name_whose_assignment_was_not_converted_is_a_warning_when_it_is_used() {
        let result = in_class("name = self.no_such_method()\nself.click(name)");
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        assert_eq!(result.diagnostics.len(), 2, "{:?}", result.diagnostics);
    }

    #[test]
    fn while_loops_are_translated_and_an_unknown_with_block_keeps_its_body() {
        let result = in_class(
            "n = 0\n\
             while n < 3:\n\
             \x20   self.click(\"#a\")\n\
             \x20   n += 1\n\
             with self.assert_raises(Exception):\n\
             \x20   self.click(\"#b\")",
        );
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "let mut n = 0;",
                "while n < 3 {",
                "n += 1;",
                "sb.click(\"#b\").await?;",
            ],
        );
    }

    #[test]
    fn several_tests_keep_their_python_names() {
        let result = import_python(
            "from seleniumbase import BaseCase\n\n\n\
             class T(BaseCase):\n\
             \x20   def test_a(self):\n\
             \x20       self.open(\"https://a.test\")\n\n\
             \x20   def test_b(self):\n\
             \x20       self.open(\"https://b.test\")\n",
            &ImportOptions::default(),
        );
        assert_has(
            &result,
            &["async fn test_a(sb: Sb)", "async fn test_b(sb: Sb)"],
        );
    }

    #[test]
    fn a_file_the_lexer_cannot_read_is_one_error_and_a_valid_shell() {
        let result = import_python(
            "self.open(\"https://a.test)\n",
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        );
        assert!(!result.is_complete());
        assert_has(&result, &["TODO(source line 1): could not read the file"]);
        syn::parse_file(&result.rust).unwrap();
    }

    /// The example of `docs/python-migration.md`.
    #[test]
    fn the_docs_example_converts_as_shown() {
        let python = r##"from seleniumbase import BaseCase

class LoginTest(BaseCase):
    def test_login(self):
        user = "alice"
        self.open("https://example.com/login")
        self.type("#username", user)
        self.type("#password", "secret")
        self.click("#submit")
        self.assert_text(f"Welcome, {user}")
"##;
        let result = import_python(
            python,
            &ImportOptions {
                source: PythonSource::Auto,
                test_name: "login_test".to_owned(),
            },
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.rust,
            r##"//! Converted from Python by `sbrs import-python`. Run it with `cargo test --test login_test`
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
"##
        );
    }

    /// Fails with the whole output when a line is missing.
    fn assert_has(result: &ImportResult, lines: &[&str]) {
        for line in lines {
            assert!(
                result.rust.contains(line),
                "missing `{line}` in:\n{}\n{:?}",
                result.rust,
                result.diagnostics
            );
        }
    }

    #[test]
    fn gap6_the_test_is_the_rstest_sb_fixture_form_mkfile_writes() {
        let result = in_class("self.open(\"https://example.test\")");
        assert_has(
            &result,
            &[
                "use seleniumbase_rs::testing::{rstest, sb, Sb};",
                "use seleniumbase_rs::{tokio, Result};",
                "#[rstest]\n#[tokio::test]\nasync fn imported_python_test(sb: Sb) -> Result<()> {",
                "    sb.run(async |sb| {",
                "        sb.open(\"https://example.test\").await?;",
                "        Ok(())\n    })\n    .await\n}",
            ],
        );
        assert!(!result.rust.contains("BrowserConfig"), "{}", result.rust);
        assert!(!result.rust.contains("run_browser_test"), "{}", result.rust);
    }

    #[test]
    fn gap6_a_docstring_becomes_a_module_comment_and_main_is_dropped_silently() {
        let source = "\"\"\"A complete end-to-end test.\"\"\"\n\
                      from seleniumbase import BaseCase\n\
                      BaseCase.main(__name__, __file__)\n\n\n\
                      class T(BaseCase):\n\
                      \x20   def test_x(self):\n\
                      \x20       self.open(\"https://example.test\")\n\n\n\
                      if __name__ == \"__main__\":\n\
                      \x20   print(\"run me with pytest\")\n";
        let result = import_python(source, &ImportOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            result.rust.starts_with("//! A complete end-to-end test."),
            "{}",
            result.rust
        );
        assert!(!result.rust.contains("TODO"), "{}", result.rust);
    }

    #[test]
    fn gap1_methods_with_a_rust_equal_are_mapped_and_not_left_as_todos() {
        let result = in_class(
            "self.goto(\"https://example.test\")\n\
             self.switch_to_frame(\"#f\")\n\
             self.switch_to_default_content()\n\
             self.switch_to_parent_frame()\n\
             self.switch_to_window(0)\n\
             self.open_new_window()\n\
             self.highlight(\"h2\")\n\
             self.js_click(\"a#go\")\n\
             self.set_value(\"input#s\", \"100\")\n\
             self.drag_and_drop(\"#a\", \"#b\")\n\
             self.hover_and_click(\"#a\", \"#b\")\n\
             self.assert_link_text(\"Home\")\n\
             self.assert_element_not_visible(\"img\")\n\
             self.click_visible_elements(\"input.c\")\n\
             self.select_option_by_value(\"#s\", \"x\")\n\
             self.select_option_by_index(\"#s\", 2)\n\
             self.post_message(\"hi\")\n\
             self.wait_for_ready_state_complete()\n\
             self.clear_local_storage()\n\
             self.activate_demo_mode()",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "sb.goto(\"https://example.test\").await?;",
                "sb.switch_to_frame(\"#f\").await?;",
                "sb.switch_to_default_content().await?;",
                "sb.switch_to_parent_frame().await?;",
                "sb.switch_to_window(0).await?;",
                "sb.open_new_window().await?;",
                "sb.highlight(\"h2\").await?;",
                "sb.js_click(\"a#go\").await?;",
                "sb.set_value(\"input#s\", \"100\").await?;",
                "sb.drag_and_drop(\"#a\", \"#b\").await?;",
                "sb.hover_and_click(\"#a\", \"#b\").await?;",
                "sb.assert_link_text(\"Home\").await?;",
                "sb.assert_element_not_visible(\"img\").await?;",
                "sb.click_visible_elements(\"input.c\").await?;",
                "sb.select_option_by_value(\"#s\", \"x\").await?;",
                "sb.select_option_by_index(\"#s\", 2).await?;",
                "sb.post_message(\"hi\").await?;",
                "sb.wait_for_ready_state_complete().await?;",
                "sb.clear_local_storage().await?;",
                "sb.activate_demo_mode().await?;",
            ],
        );
    }

    #[test]
    fn gap1_waits_and_asserts_with_defaults_are_filled_in() {
        let result = in_class(
            "self.wait_for_element(\"#c\")\n\
             self.assert_true(self.is_element_visible(\"img\"))\n\
             self.assert_false(self.is_text_visible(\"gone\"))\n\
             self.assert_elements(\"h1\", \"h2\")\n\
             self.sleep(1)",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "sb.wait_for_element(\"#c\").await?;",
                "let element_visible = sb.is_element_visible(\"img\").await?;",
                "sb.assert_true(element_visible)?;",
                "let text_visible = sb.is_text_visible(\"gone\", \"html\").await?;",
                "sb.assert_false(text_visible)?;",
                "sb.assert_elements(&[\"h1\", \"h2\"]).await?;",
                "sb.sleep(1.0).await;",
            ],
        );
    }

    #[test]
    fn gap2_variables_f_strings_and_concatenations_pass_through() {
        let result = in_class(
            "checkbox = \"input#vehicle2\"\n\
             self.click(checkbox)\n\
             base = \"https://example.test\"\n\
             self.open(base + \"/login\")\n\
             self.open(f\"{base}/home\")\n\
             self.click('a[href*=\"%s\"]' % checkbox)\n\
             title = self.get_title()\n\
             self.assert_true(\"login\" in title.lower())",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "let checkbox = \"input#vehicle2\";",
                "sb.click(checkbox).await?;",
                "let base = \"https://example.test\";",
                "sb.open(&format!(\"{}/login\", base)).await?;",
                "sb.open(&format!(\"{}/home\", base)).await?;",
                "sb.click(&format!(\"a[href*=\\\"{}\\\"]\", checkbox)).await?;",
                "let title = sb.get_title().await?;",
                "title.to_lowercase().contains(\"login\")",
            ],
        );
    }

    #[test]
    fn gap2_an_expression_that_cannot_be_translated_is_a_todo_not_an_error() {
        let result = in_class("self.click(some_module.selector_for(3))");
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_has(&result, &["TODO(source line 6)"]);
    }

    #[test]
    fn gap3_for_if_try_with_and_assignments_are_translated() {
        let result = in_class(
            "texts = [\"a\", \"b\"]\n\
             for index, text in enumerate(texts):\n\
             \x20   self.select_option_by_text(\"#s\", text)\n\
             \x20   self.assert_text(text, \"#out\")\n\
             if self.is_element_visible(\"#x\"):\n\
             \x20   self.click(\"#x\")\n\
             elif self.is_element_visible(\"#y\"):\n\
             \x20   self.click(\"#y\")\n\
             else:\n\
             \x20   self.click(\"#z\")\n\
             try:\n\
             \x20   self.hover_and_click(\"#a\", \"#b\")\n\
             except Exception:\n\
             \x20   self.hover_and_js_click(\"#a\", \"#b\")\n\
             with self.frame_switch(\"iframe\"):\n\
             \x20   self.click(\"h2\")\n\
             \x20   self.assert_text(\"Hi\", \"h2\")",
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "for (index, text) in texts.iter().enumerate() {",
                "sb.select_option_by_text(\"#s\", text).await?;",
                "let element_visible = sb.is_element_visible(\"#x\").await?;",
                "if element_visible {",
                "} else {",
                "let attempt: Result<()> = async {",
                "sb.hover_and_click(\"#a\", \"#b\").await?;",
                "if attempt.is_err() {",
                "sb.hover_and_js_click(\"#a\", \"#b\").await?;",
                "sb.frame_switch(\"iframe\")",
                ".run(async |sb| {",
                "sb.assert_text(\"Hi\", \"h2\").await?;",
            ],
        );
    }

    #[test]
    fn gap3_parameterized_tests_become_rstest_cases_and_set_up_runs_first() {
        let source = "from parameterized import parameterized\n\
                      from seleniumbase import BaseCase\n\n\n\
                      class TodoMVC(BaseCase):\n\
                      \x20   def setUp(self):\n\
                      \x20       super().setUp()\n\
                      \x20       self.open_new_window()\n\n\
                      \x20   def tearDown(self):\n\
                      \x20       self.save_teardown_screenshot()\n\
                      \x20       super().tearDown()\n\n\
                      \x20   @parameterized.expand([[\"jquery\"], [\"react\"]])\n\
                      \x20   def test_todomvc(self, framework):\n\
                      \x20       self.goto(\"https://todomvc.com/\")\n\
                      \x20       self.click('a[href*=\"examples/%s/dist\"]' % framework)\n";
        let result = import_python(source, &ImportOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "#[rstest]\n#[case(\"jquery\")]\n#[case(\"react\")]\n#[tokio::test]",
                "async fn imported_python_test(sb: Sb, #[case] framework: &str) -> Result<()> {",
                "sb.open_new_window().await?;\n        sb.goto(\"https://todomvc.com/\").await?;",
                "sb.save_teardown_screenshot().await?;",
            ],
        );
    }

    #[test]
    fn gap3_an_if_whose_condition_cannot_be_translated_does_not_run_its_body() {
        let result = in_class("if self.headed:\n    self.activate_demo_mode()");
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_has(&result, &["TODO(source line 6)", "if false {"]);
    }

    #[test]
    fn gap4_a_text_assertion_without_a_selector_looks_at_html() {
        let result = in_class("self.assert_text(\"Checkout: Overview\")");
        assert_has(
            &result,
            &["sb.assert_text(\"Checkout: Overview\", \"html\").await?;"],
        );
    }

    #[test]
    fn gap5_with_sb_and_sb_cdp_scripts_become_fixture_tests() {
        let with_sb = import_python(
            "from seleniumbase import SB\n\nwith SB(uc=True, test=True) as sb:\n    sb.open(\"https://example.test\")\n    sb.click(\"#go\")\n",
            &ImportOptions::default(),
        );
        assert!(with_sb.diagnostics.is_empty(), "{:?}", with_sb.diagnostics);
        assert_has(
            &with_sb,
            &[
                "async fn imported_python_test(sb: Sb) -> Result<()> {",
                "sb.click(\"#go\").await?;",
                "// SB(uc=True, test=True)",
            ],
        );

        let cdp = import_python(
            "from seleniumbase import sb_cdp\n\nsb = sb_cdp.Chrome()\nsb.goto(\"https://example.test\")\nsb.type(\"#q\", \"rust\")\nsb.assert_text(\"Results\", \"h1\")\nsb.assert_element(\"img\")\nsb.highlight(\"#q\")\nsb.quit()\n",
            &ImportOptions::default(),
        );
        assert!(cdp.diagnostics.is_empty(), "{:?}", cdp.diagnostics);
        assert_has(
            &cdp,
            &[
                "use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};",
                "async fn imported_python_test(sb_cdp: SbCdp) -> Result<()> {",
                "sb_cdp\n        .run(async |page| {",
                "page.goto(\"https://example.test\").await?;",
                "page.locator(\"#q\").type_text(\"rust\").await?;",
                "page.locator(\"h1\").expect().to_contain_text(\"Results\").await?;",
                "page.locator(\"img\").expect().to_be_visible().await?;",
                "page.locator(\"#q\").flash().await?;",
            ],
        );
        assert!(!cdp.rust.contains("quit"), "{}", cdp.rust);
    }

    #[test]
    fn gap5_activate_cdp_mode_in_a_with_sb_block_is_a_pure_cdp_test() {
        let result = import_python(
            "from seleniumbase import SB\n\nwith SB(uc=True) as sb:\n    sb.activate_cdp_mode()\n    sb.goto(\"https://example.test\")\n    sb.click(\"#go\")\n",
            &ImportOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_has(
            &result,
            &[
                "sb_cdp: SbCdp",
                "page.goto(\"https://example.test\").await?;",
                "page.locator(\"#go\").click().await?;",
            ],
        );
    }

    #[test]
    fn imports_seleniumbase_test() {
        let source = r##"
from seleniumbase import BaseCase

class LoginTests(BaseCase):
    def test_login(self):
        self.open("https://example.test/login")
        self.type("#username", "alice")
        self.click("button[type='submit']")
        self.assert_text("Welcome", "main")
"##;
        let result = import_python(source, &ImportOptions::default());
        assert_eq!(result.detected_source, PythonSource::SeleniumBase);
        assert!(result.is_complete());
        assert!(result
            .rust
            .contains("sb.open(\"https://example.test/login\")"));
        assert!(result
            .rust
            .contains("sb.type_text(\"#username\", \"alice\")"));
        assert!(result
            .rust
            .contains("sb.assert_text(\"Welcome\", \"main\")"));
        assert!(result.rust.contains("sb.run(async |sb| {"));
    }

    #[test]
    fn imports_selenium_with_element_variables() {
        let source = r#"
from selenium.webdriver.common.by import By

driver.get("https://example.test/login")
username = driver.find_element(By.ID, "username")
username.send_keys("alice")
driver.find_element(By.CSS_SELECTOR, "button").click()
assert "Dashboard" in driver.title
driver.quit()
"#;
        let result = import_python(source, &ImportOptions::default());
        assert_eq!(result.detected_source, PythonSource::Selenium);
        assert!(result.is_complete());
        assert!(result
            .rust
            .contains(r#"sb.add_text("[id=\"username\"]", "alice")"#));
        assert!(result.rust.contains("sb.click(\"button\")"));
        assert!(result
            .rust
            .contains("sb.assert_title_contains(\"Dashboard\")"));
    }

    #[test]
    fn an_xpath_locator_is_passed_through_unchanged() {
        let source = r#"
from selenium.webdriver.common.by import By
driver.find_element(By.XPATH, "//div[@class='a']/span[2]").click()
driver.find_element(By.XPATH, "//a[text()='Home']").click()
"#;
        let result = import_python(source, &ImportOptions::default());
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        assert!(result
            .rust
            .contains(r#"sb.click("//div[@class='a']/span[2]")"#));
        assert!(result.rust.contains(r#"sb.click("//a[text()='Home']")"#));
    }

    #[test]
    fn a_relative_xpath_that_would_be_read_as_css_is_reported() {
        let result = import_python(
            "driver.find_element(By.XPATH, \"div/a\").click()",
            &ImportOptions::default(),
        );
        assert!(!result.is_complete());
    }

    #[test]
    fn preserves_unsupported_python_as_diagnostic() {
        let result = import_python(
            "driver.execute_script(dynamic_script)",
            &ImportOptions::default(),
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.rust.contains("TODO(source line 1)"));
    }

    #[test]
    fn combines_multiline_calls() {
        let source = "self.type(\n    \"#name\",\n    \"Ada\",\n)";
        let result = import_python(
            source,
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        );
        assert!(result.is_complete());
        assert!(result.rust.contains("sb.type_text(\"#name\", \"Ada\")"));
    }

    #[test]
    fn imports_seleniumbase_extra_methods() {
        let source = r##"
self.open("https://example.test")
self.assert_url_contains("example")
self.check_if_unchecked("#agree")
self.double_click("#submit")
"##;
        let result = import_python(
            source,
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                test_name: "extra".to_owned(),
            },
        );
        assert!(result.is_complete());
        assert!(result.rust.contains("sb.assert_url_contains(\"example\")"));
        assert!(result.rust.contains("sb.check_if_unchecked(\"#agree\")"));
        assert!(result.rust.contains("sb.double_click(\"#submit\")"));
    }

    #[test]
    fn a_sleep_becomes_a_float_literal_that_compiles() {
        let result = import_python(
            "self.sleep(1)\nself.sleep(0.5)\nself.sleep(2.0)",
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        for seconds in ["1.0", "0.5", "2.0"] {
            assert!(
                result.rust.contains(&format!("sb.sleep({seconds}).await;")),
                "{seconds}: {}",
                result.rust
            );
        }
    }

    #[test]
    fn a_sleep_that_would_panic_at_run_time_is_reported() {
        for bad in ["-1", "nan", "inf"] {
            let result = import_python(
                &format!("self.sleep({bad})"),
                &ImportOptions {
                    source: PythonSource::SeleniumBase,
                    ..Default::default()
                },
            );
            assert!(!result.is_complete(), "{bad} was accepted");
        }
    }

    #[test]
    fn an_apostrophe_in_a_comment_does_not_swallow_the_next_lines() {
        let source = "# it's the login page\nself.open(\"https://example.test\")  # don't wait\nself.click(\"#go\")";
        let result = import_python(
            source,
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        assert!(result.rust.contains("sb.open(\"https://example.test\")"));
        assert!(result.rust.contains("sb.click(\"#go\")"));
    }

    #[test]
    fn a_hash_inside_a_string_is_not_a_comment() {
        let result = import_python(
            "self.click(\"#submit\")",
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        );
        assert!(result.is_complete());
        assert!(result.rust.contains("sb.click(\"#submit\")"));
    }

    #[test]
    fn a_url_that_mentions_seleniumbase_does_not_make_it_a_seleniumbase_script() {
        let source = "driver.get(\"https://seleniumbase.io/demo_page\")";
        assert_eq!(
            detect_source(source, PythonSource::Auto),
            PythonSource::Selenium
        );
        assert_eq!(
            detect_source("from seleniumbase import BaseCase", PythonSource::Auto),
            PythonSource::SeleniumBase
        );
    }

    #[test]
    fn imports_selenium_element_reads() {
        let source = r##"
from selenium.webdriver.common.by import By
header = driver.find_element(By.CSS_SELECTOR, "h1")
_ = header.text
_ = header.get_attribute("data-id")
"##;
        let result = import_python(source, &ImportOptions::default());
        assert!(result.is_complete());
        assert!(result.rust.contains("sb.get_text(\"h1\")"));
        assert!(result
            .rust
            .contains("sb.get_attribute(\"h1\", \"data-id\")"));
    }

    /// The Rust the importer writes for the SeleniumBase statements `source`.
    fn seleniumbase(source: &str) -> ImportResult {
        import_python(
            source,
            &ImportOptions {
                source: PythonSource::SeleniumBase,
                ..Default::default()
            },
        )
    }

    #[test]
    fn a_text_call_without_a_selector_looks_at_the_body() {
        // Python's `assert_text(text, selector="body")`; the importer used to
        // refuse the one-argument form.
        let result = seleniumbase(
            "self.assert_text(\"Welcome\")\n\
             self.assert_exact_text(\"Hi\")\n\
             self.assert_text_visible(\"Shown\")\n\
             self.assert_text_not_visible(\"Gone\")\n\
             self.deferred_assert_text(\"Terms\")",
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        for line in [
            "sb.assert_text(\"Welcome\", \"html\").await?;",
            "sb.assert_exact_text(\"Hi\", \"html\").await?;",
            "sb.assert_text(\"Shown\", \"html\").await?;",
            "sb.assert_text_not_visible(\"Gone\", \"html\").await?;",
            "sb.deferred_assert_text(\"Terms\", \"html\").await?;",
        ] {
            assert!(result.rust.contains(line), "{line}\n{}", result.rust);
        }
    }

    #[test]
    fn keyword_arguments_are_matched_with_python_parameters() {
        let result = seleniumbase(
            "self.assert_text(\"Welcome\", selector=\"h1\")\n\
             self.assert_exact_text(selector=\"h2\", text=\"Hi\")\n\
             self.wait_for_text(\"Done\", \"#status\", timeout=3)\n\
             self.type(selector=\"#user\", text=\"alice\")",
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        for line in [
            "sb.assert_text(\"Welcome\", \"h1\").await?;",
            "sb.assert_exact_text(\"Hi\", \"h2\").await?;",
            "sb.wait_for_text(\"Done\", \"#status\", 3).await?;",
            "sb.type_text(\"#user\", \"alice\").await?;",
        ] {
            assert!(result.rust.contains(line), "{line}\n{}", result.rust);
        }
    }

    #[test]
    fn the_waits_and_deferred_checks_are_imported() {
        let result = seleniumbase(
            "self.wait_for_text(\"Ready\", \"#s\")\n\
             self.wait_for_text_visible(\"Ready\")\n\
             self.wait_for_exact_text(\"Ready\", \"#s\", timeout=4)\n\
             self.wait_for_element_visible(\"#w\", timeout=7)\n\
             self.wait_for_element_present(\"#p\")\n\
             self.deferred_assert_element(\"#h\")\n\
             self.deferred_assert_exact_text(\"Ok\", \"#r\")\n\
             self.process_deferred_asserts()\n\
             self.enter_mfa_code(\"#otp\", \"JBSWY3DPEHPK3PXP\")",
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        for line in [
            "sb.wait_for_text(\"Ready\", \"#s\", 10).await?;",
            "sb.wait_for_text(\"Ready\", \"html\", 10).await?;",
            "sb.wait_for_exact_text(\"Ready\", \"#s\", 4).await?;",
            "sb.wait_for_element_visible(\"#w\", 7).await?;",
            "sb.wait_for_element_present(\"#p\", 10).await?;",
            "sb.deferred_assert_element(\"#h\").await?;",
            "sb.deferred_assert_exact_text(\"Ok\", \"#r\").await?;",
            "sb.process_deferred_asserts().await?;",
            "sb.enter_mfa_code(\"#otp\", \"JBSWY3DPEHPK3PXP\").await?;",
        ] {
            assert!(result.rust.contains(line), "{line}\n{}", result.rust);
        }
    }

    #[test]
    fn send_keys_adds_to_the_field_and_type_replaces_it() {
        // Python's `send_keys` is `add_text`; the importer used to clear the
        // field first.
        let result = seleniumbase(
            "self.send_keys(\"#a\", \"x\")\n\
             self.add_text(\"#a\", \"y\")\n\
             self.type(\"#a\", \"z\")\n\
             self.update_text(\"#a\", \"w\")",
        );
        assert!(result.is_complete(), "{:?}", result.diagnostics);
        assert!(result.rust.contains("sb.add_text(\"#a\", \"x\").await?;"));
        assert!(result.rust.contains("sb.add_text(\"#a\", \"y\").await?;"));
        assert!(result.rust.contains("sb.type_text(\"#a\", \"z\").await?;"));
        assert!(result.rust.contains("sb.type_text(\"#a\", \"w\").await?;"));

        let selenium = import_python(
            "el = driver.find_element(By.ID, \"a\")\nel.send_keys(\"x\")",
            &ImportOptions {
                source: PythonSource::Selenium,
                ..Default::default()
            },
        );
        assert!(selenium.is_complete(), "{:?}", selenium.diagnostics);
        assert!(selenium.rust.contains("sb.add_text("), "{}", selenium.rust);
    }

    #[test]
    fn arguments_rust_cannot_carry_over_are_errors_not_silently_dropped() {
        for source in [
            "self.assert_text(\"x\", timeout=5)",
            "self.assert_text(\"x\", \"#a\", by=\"xpath\")",
            "self.assert_text(text)",
            "self.assert_text(\"x\", \"#a\", \"css selector\", 5, 6)",
            "self.assert_text(\"x\", flavour=\"y\")",
            "self.assert_text(\"x\", text=\"y\")",
            "self.enter_mfa_code(\"#otp\", totp_key=None)",
            "self.wait_for_text(\"x\", timeout=soon)",
            "self.deferred_assert_text(\"x\", fs=True)",
        ] {
            let result = seleniumbase(source);
            assert!(!result.is_complete(), "{source}\n{}", result.rust);
            assert!(result.rust.contains("TODO"), "{source}\n{}", result.rust);
        }
    }
}
