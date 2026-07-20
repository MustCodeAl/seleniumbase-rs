//! Static migration of common SeleniumBase and Selenium Python tests to Rust.

use std::collections::HashMap;

use regex::Regex;

use crate::utils::selectors::xpath_to_css;

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
    TypeText(String, String),
    Clear(String),
    Submit(String),
    Hover(String),
    AssertElement(String),
    AssertText(String, String),
    AssertExactText(String, String),
    AssertTitle(String),
    AssertTitleContains(String),
    WaitVisible(String, u64),
    WaitPresent(String, u64),
    SelectText(String, String),
    Maximize,
    Back,
    Forward,
    Refresh,
    Sleep(f64),
    Todo(usize, String),
}

#[derive(Clone, Debug)]
enum Locator {
    Css(String),
    LinkText(String),
    PartialLinkText(String),
}

/// Converts common SeleniumBase or Selenium Python statements into a Rust test.
///
/// Dynamic Python, custom helpers, and unsupported locators remain visible as
/// source-located `TODO` comments and diagnostics.
pub fn import_python(source: &str, options: &ImportOptions) -> ImportResult {
    let detected_source = detect_source(source, options.source);
    let mut diagnostics = Vec::new();
    let mut elements = HashMap::new();
    let mut actions = Vec::new();

    for (line, statement) in statements(source) {
        let trimmed = statement.trim();
        if should_ignore(trimmed) {
            continue;
        }

        let action = match detected_source {
            PythonSource::SeleniumBase => parse_seleniumbase(trimmed, line),
            PythonSource::Selenium => parse_selenium(trimmed, line, &mut elements),
            PythonSource::Auto => unreachable!("auto source is resolved before parsing"),
        };

        match action {
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
    if source.contains("seleniumbase") || source.contains("BaseCase") || source.contains("with SB(")
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

fn parse_seleniumbase(statement: &str, line: usize) -> ParseOutcome {
    let Some((method, args)) = method_call(statement, &["self", "sb"]) else {
        return ParseOutcome::Unsupported(format!(
            "unsupported SeleniumBase statement: {}",
            one_line(statement)
        ));
    };
    let args = split_args(&args);

    match method.as_str() {
        "open" | "visit" => one_string(&args, Action::Open),
        "click" => one_string(&args, Action::Click),
        "click_link" | "click_link_text" => one_string(&args, Action::ClickLink),
        "click_partial_link_text" => one_string(&args, Action::ClickPartialLink),
        "type" | "type_text" | "update_text" | "send_keys" => two_strings(&args, Action::TypeText),
        "clear" => one_string(&args, Action::Clear),
        "submit" => one_string(&args, Action::Submit),
        "hover" | "hover_on_element" => one_string(&args, Action::Hover),
        "assert_element" | "assert_element_visible" => one_string(&args, Action::AssertElement),
        "assert_text" => reverse_two_strings(&args, Action::AssertText),
        "assert_exact_text" => reverse_two_strings(&args, Action::AssertExactText),
        "assert_title" => one_string(&args, Action::AssertTitle),
        "assert_title_contains" => one_string(&args, Action::AssertTitleContains),
        "wait_for_element_visible" => wait_action(&args, true),
        "wait_for_element_present" => wait_action(&args, false),
        "select_option_by_text" => two_strings(&args, Action::SelectText),
        "maximize_window" => ParseOutcome::Action(Action::Maximize),
        "go_back" => ParseOutcome::Action(Action::Back),
        "go_forward" => ParseOutcome::Action(Action::Forward),
        "refresh_page" | "refresh" => ParseOutcome::Action(Action::Refresh),
        "sleep" => match args.first().and_then(|value| value.parse::<f64>().ok()) {
            Some(seconds) => ParseOutcome::Action(Action::Sleep(seconds)),
            None => invalid_arguments(line, &method),
        },
        _ => ParseOutcome::Unsupported(format!(
            "SeleniumBase method '{method}' needs manual migration"
        )),
    }
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
        (Locator::Css(css), "click") => ParseOutcome::Action(Action::Click(css)),
        (Locator::LinkText(text), "click") => ParseOutcome::Action(Action::ClickLink(text)),
        (Locator::PartialLinkText(text), "click") => {
            ParseOutcome::Action(Action::ClickPartialLink(text))
        }
        (Locator::Css(css), "clear") => ParseOutcome::Action(Action::Clear(css)),
        (Locator::Css(css), "submit") => ParseOutcome::Action(Action::Submit(css)),
        (Locator::Css(css), "send_keys") => match python_string(args) {
            Some(text) => ParseOutcome::Action(Action::TypeText(css, text)),
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
        "CSS_SELECTOR" => Ok(Locator::Css(value)),
        "ID" => Ok(Locator::Css(format!(r#"[id="{}"]"#, css_escape(&value)))),
        "NAME" => Ok(Locator::Css(format!(r#"[name="{}"]"#, css_escape(&value)))),
        "CLASS_NAME" if !value.chars().any(char::is_whitespace) => {
            Ok(Locator::Css(format!(".{}", css_escape(&value))))
        }
        "TAG_NAME" => Ok(Locator::Css(value)),
        "XPATH" => xpath_to_css(&value)
            .map(Locator::Css)
            .map_err(|_| format!("XPath '{value}' cannot be represented safely as a CSS selector")),
        "LINK_TEXT" => Ok(Locator::LinkText(value)),
        "PARTIAL_LINK_TEXT" => Ok(Locator::PartialLinkText(value)),
        _ => Err(format!(
            "unsupported Selenium locator strategy '{}'",
            args[0]
        )),
    }
}

fn method_call(statement: &str, receivers: &[&str]) -> Option<(String, String)> {
    for receiver in receivers {
        let prefix = format!("{receiver}.");
        let Some(rest) = statement.strip_prefix(&prefix) else {
            continue;
        };
        let open = rest.find('(')?;
        if !rest.ends_with(')') {
            return None;
        }
        return Some((
            rest[..open].to_owned(),
            rest[open + 1..rest.len() - 1].to_owned(),
        ));
    }
    None
}

fn one_string(args: &[String], action: impl FnOnce(String) -> Action) -> ParseOutcome {
    match args.first().and_then(|arg| python_string(arg)) {
        Some(value) => ParseOutcome::Action(action(value)),
        None => ParseOutcome::Invalid("expected a string literal argument".to_owned()),
    }
}

fn two_strings(args: &[String], action: impl FnOnce(String, String) -> Action) -> ParseOutcome {
    match (
        args.first().and_then(|arg| python_string(arg)),
        args.get(1).and_then(|arg| python_string(arg)),
    ) {
        (Some(first), Some(second)) => ParseOutcome::Action(action(first, second)),
        _ => ParseOutcome::Invalid("expected two string literal arguments".to_owned()),
    }
}

fn reverse_two_strings(
    args: &[String],
    action: impl FnOnce(String, String) -> Action,
) -> ParseOutcome {
    match (
        args.first().and_then(|arg| python_string(arg)),
        args.get(1).and_then(|arg| python_string(arg)),
    ) {
        (Some(first), Some(second)) => ParseOutcome::Action(action(second, first)),
        _ => ParseOutcome::Invalid("expected two string literal arguments".to_owned()),
    }
}

fn wait_action(args: &[String], visible: bool) -> ParseOutcome {
    let Some(selector) = args.first().and_then(|arg| python_string(arg)) else {
        return ParseOutcome::Invalid("wait requires a string literal selector".to_owned());
    };
    let timeout = args
        .get(1)
        .and_then(|arg| arg.parse::<u64>().ok())
        .unwrap_or(10);
    if visible {
        ParseOutcome::Action(Action::WaitVisible(selector, timeout))
    } else {
        ParseOutcome::Action(Action::WaitPresent(selector, timeout))
    }
}

fn invalid_arguments(line: usize, method: &str) -> ParseOutcome {
    ParseOutcome::Invalid(format!(
        "invalid literal arguments for '{method}' at line {line}"
    ))
}

fn statements(source: &str) -> Vec<(usize, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;
    let mut depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;

    for (index, line) in source.lines().enumerate() {
        if current.is_empty() {
            start_line = index + 1;
        } else {
            current.push(' ');
        }
        current.push_str(line.trim());

        for character in line.chars() {
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
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
            }
        }
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
    let mut output =
        String::from("use seleniumbase_rs::{BrowserConfig, run_browser_test};\n\n#[tokio::test]\n");
    output.push_str(&format!(
        "async fn {}() -> seleniumbase_rs::Result<()> {{\n",
        rust_identifier(test_name)
    ));
    output.push_str("    run_browser_test(BrowserConfig::default(), |sb| Box::pin(async move {\n");
    for action in actions {
        output.push_str("        ");
        output.push_str(&render_action(action));
        output.push('\n');
    }
    output.push_str("        Ok(())\n    })).await\n}\n");
    output
}

fn render_action(action: &Action) -> String {
    match action {
        Action::Open(url) => format!("sb.open({url:?}).await?;"),
        Action::Click(css) => format!("sb.click({css:?}).await?;"),
        Action::ClickLink(text) => format!("sb.click_link_text({text:?}).await?;"),
        Action::ClickPartialLink(text) => {
            format!("sb.click_partial_link_text({text:?}).await?;")
        }
        Action::TypeText(css, text) => format!("sb.type_text({css:?}, {text:?}).await?;"),
        Action::Clear(css) => format!("sb.clear({css:?}).await?;"),
        Action::Submit(css) => format!("sb.submit({css:?}).await?;"),
        Action::Hover(css) => format!("sb.hover({css:?}).await?;"),
        Action::AssertElement(css) => format!("sb.assert_element({css:?}).await?;"),
        Action::AssertText(css, text) => format!("sb.assert_text({css:?}, {text:?}).await?;"),
        Action::AssertExactText(css, text) => {
            format!("sb.assert_exact_text({css:?}, {text:?}).await?;")
        }
        Action::AssertTitle(title) => format!("sb.assert_title({title:?}).await?;"),
        Action::AssertTitleContains(title) => {
            format!("sb.assert_title_contains({title:?}).await?;")
        }
        Action::WaitVisible(css, timeout) => {
            format!("sb.wait_for_element_visible({css:?}, {timeout}).await?;")
        }
        Action::WaitPresent(css, timeout) => {
            format!("sb.wait_for_element_present({css:?}, {timeout}).await?;")
        }
        Action::SelectText(css, text) => {
            format!("sb.select_option_by_text({css:?}, {text:?}).await?;")
        }
        Action::Maximize => "sb.maximize_window().await?;".to_owned(),
        Action::Back => "sb.go_back().await?;".to_owned(),
        Action::Forward => "sb.go_forward().await?;".to_owned(),
        Action::Refresh => "sb.refresh().await?;".to_owned(),
        Action::Sleep(seconds) => {
            format!("tokio::time::sleep(std::time::Duration::from_secs_f64({seconds})).await;")
        }
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

fn rust_identifier(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        let valid = character == '_' || character.is_ascii_alphanumeric();
        let character = if valid { character } else { '_' };
        if index == 0 && character.is_ascii_digit() {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    if output.is_empty() {
        "imported_python_test".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .contains("sb.assert_text(\"main\", \"Welcome\")"));
        assert!(result.rust.contains("run_browser_test"));
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
            .contains(r#"sb.type_text("[id=\"username\"]", "alice")"#));
        assert!(result.rust.contains("sb.click(\"button\")"));
        assert!(result
            .rust
            .contains("sb.assert_title_contains(\"Dashboard\")"));
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
}
