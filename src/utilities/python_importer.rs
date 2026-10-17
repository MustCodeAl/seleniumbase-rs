//! Static migration of common SeleniumBase and Selenium Python tests to Rust.

use std::collections::HashMap;

use regex::Regex;

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
    TypeText(String, String),
    Clear(String),
    Submit(String),
    Hover(String),
    AssertElement(String),
    AssertText(String, String),
    AssertExactText(String, String),
    AssertTextNotVisible(String, String),
    AssertExactTextNotVisible(String, String),
    WaitText(String, String, u64),
    WaitExactText(String, String, u64),
    DeferredElement(String),
    DeferredText(String, String),
    DeferredExactText(String, String),
    ProcessDeferred,
    EnterMfaCode(String, String),
    AddText(String, String),
    AssertTitle(String),
    AssertTitleContains(String),
    AssertUrl(String),
    AssertUrlContains(String),
    GetText(String),
    GetAttribute(String, String),
    Check(String),
    Uncheck(String),
    DoubleClick(String),
    RightClick(String),
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

fn parse_seleniumbase(statement: &str, line: usize) -> ParseOutcome {
    let Some((method, args)) = method_call(statement, &["self", "sb"]) else {
        return ParseOutcome::Unsupported(format!(
            "unsupported SeleniumBase statement: {}",
            one_line(statement)
        ));
    };
    let args = split_args(&args);

    match method.as_str() {
        "open" | "visit" | "get" => one_string(&args, Action::Open),
        "click" => one_string(&args, Action::Click),
        "click_link" | "click_link_text" => one_string(&args, Action::ClickLink),
        "click_partial_link_text" => one_string(&args, Action::ClickPartialLink),
        "double_click" | "double_click_element" => one_string(&args, Action::DoubleClick),
        "right_click" | "right_click_element" => one_string(&args, Action::RightClick),
        // Python's `send_keys` is `add_text`: it does not clear the field.
        "type" | "type_text" | "update_text" => selector_and_text(
            &args,
            &["selector", "text", "by", "timeout", "retry"],
            Action::TypeText,
        ),
        "add_text" | "send_keys" => selector_and_text(
            &args,
            &["selector", "text", "by", "timeout"],
            Action::AddText,
        ),
        "enter_mfa_code" => selector_and_text(
            &args,
            &["selector", "totp_key", "by", "timeout"],
            Action::EnterMfaCode,
        ),
        "clear" => one_string(&args, Action::Clear),
        "submit" => one_string(&args, Action::Submit),
        "hover" | "hover_on_element" => one_string(&args, Action::Hover),
        "check_if_unchecked" | "check" => one_string(&args, Action::Check),
        "uncheck_if_checked" | "uncheck" => one_string(&args, Action::Uncheck),
        "assert_element" | "assert_element_visible" => {
            selector_only(&args, &["selector", "by", "timeout"], Action::AssertElement)
        }
        // The selector is `body` when the Python call leaves it out.
        "assert_text" | "assert_text_visible" => {
            text_call(&args, false, |text, css, _| Action::AssertText(text, css))
        }
        "assert_exact_text" => text_call(&args, false, |text, css, _| {
            Action::AssertExactText(text, css)
        }),
        "assert_text_not_visible" => text_call(&args, false, |text, css, _| {
            Action::AssertTextNotVisible(text, css)
        }),
        "assert_exact_text_not_visible" => text_call(&args, false, |text, css, _| {
            Action::AssertExactTextNotVisible(text, css)
        }),
        "wait_for_text" | "wait_for_text_visible" => {
            text_call(&args, true, |text, css, timeout| {
                Action::WaitText(text, css, timeout.unwrap_or(DEFAULT_WAIT_SECONDS))
            })
        }
        "wait_for_exact_text" | "wait_for_exact_text_visible" => {
            text_call(&args, true, |text, css, timeout| {
                Action::WaitExactText(text, css, timeout.unwrap_or(DEFAULT_WAIT_SECONDS))
            })
        }
        "deferred_assert_element" | "delayed_assert_element" => selector_only(
            &args,
            &["selector", "by", "timeout", "fs"],
            Action::DeferredElement,
        ),
        "deferred_assert_text" | "delayed_assert_text" => text_call_with(
            &args,
            &["text", "selector", "by", "timeout", "fs"],
            Action::DeferredText,
        ),
        "deferred_assert_exact_text" | "delayed_assert_exact_text" => text_call_with(
            &args,
            &["text", "selector", "by", "timeout", "fs"],
            Action::DeferredExactText,
        ),
        "process_deferred_asserts" | "process_delayed_asserts" => {
            ParseOutcome::Action(Action::ProcessDeferred)
        }
        "assert_title" => one_string(&args, Action::AssertTitle),
        "assert_title_contains" => one_string(&args, Action::AssertTitleContains),
        "assert_url" => one_string(&args, Action::AssertUrl),
        "assert_url_contains" => one_string(&args, Action::AssertUrlContains),
        "get_text" => one_string(&args, Action::GetText),
        "get_attribute" => two_strings(&args, Action::GetAttribute),
        "wait_for_element_visible" => wait_action(&args, true),
        "wait_for_element_present" => wait_action(&args, false),
        "select_option_by_text" => two_strings(&args, Action::SelectText),
        "maximize_window" => ParseOutcome::Action(Action::Maximize),
        "go_back" => ParseOutcome::Action(Action::Back),
        "go_forward" => ParseOutcome::Action(Action::Forward),
        "refresh_page" | "refresh" => ParseOutcome::Action(Action::Refresh),
        // `Duration::from_secs_f64` panics on a negative or non-finite number.
        "sleep" => match args
            .first()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        {
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

/// How long a wait lasts when the Python call gives no timeout, in seconds.
const DEFAULT_WAIT_SECONDS: u64 = 10;

/// The text of `value` if it is a Python string literal.
fn literal(value: Option<&str>, name: &str) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("the '{name}' argument is missing"))?;
    python_string(value).ok_or_else(|| format!("'{name}' must be a string literal"))
}

/// Whether `by` is the CSS strategy, the one `BaseCase` assumes for a
/// selector that does not start like XPath.
fn check_by(by: Option<&str>) -> Result<(), String> {
    match by {
        None => Ok(()),
        Some(by)
            if python_string(by).as_deref() == Some("css selector")
                || by.trim() == "By.CSS_SELECTOR" =>
        {
            Ok(())
        }
        Some(by) => Err(format!(
            "by={by} needs manual migration: BaseCase reads a selector that starts with '/' or '(' as XPath"
        )),
    }
}

/// `name=value` as a keyword argument of a call, if `arg` is one.
fn keyword(arg: &str) -> Option<(&str, &str)> {
    let (name, value) = arg.split_once('=')?;
    let name = name.trim();
    let is_name = !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (is_name && !value.starts_with('=')).then_some((name, value.trim()))
}

/// The arguments of a call matched with the names of Python's parameters:
/// positional ones in order, keyword ones (`name=value`) by name.
fn bind<'a>(args: &'a [String], names: &[&str]) -> Result<Vec<Option<&'a str>>, String> {
    let mut bound: Vec<Option<&str>> = vec![None; names.len()];
    let mut next = 0;
    for arg in args {
        let index = match keyword(arg) {
            Some((key, _)) => names
                .iter()
                .position(|name| *name == key)
                .ok_or_else(|| format!("the argument '{key}' is not supported"))?,
            None => {
                next += 1;
                next - 1
            }
        };
        if index >= names.len() {
            return Err("too many arguments".to_owned());
        }
        if bound[index].is_some() {
            return Err(format!("'{}' is given twice", names[index]));
        }
        bound[index] = Some(keyword(arg).map_or(arg.as_str(), |(_, value)| value));
    }
    Ok(bound)
}

/// The outcome of a call whose arguments were read into an action, or the
/// reason they could not be.
fn outcome(result: Result<Action, String>) -> ParseOutcome {
    result.map_or_else(ParseOutcome::Invalid, ParseOutcome::Action)
}

/// The selector argument of a call, `body` when it is left out.
fn selector_or_body(value: Option<&str>) -> Result<String, String> {
    value.map_or_else(
        || Ok("body".to_owned()),
        |selector| literal(Some(selector), "selector"),
    )
}

/// A text call like Python's `assert_text(text, selector="body", by, timeout)`:
/// the text, the selector (`body` when it is left out) and, when
/// `timeout_allowed`, the timeout in seconds.
fn text_call(
    args: &[String],
    timeout_allowed: bool,
    action: impl FnOnce(String, String, Option<u64>) -> Action,
) -> ParseOutcome {
    outcome(
        text_arguments(args, timeout_allowed)
            .map(|(text, css, timeout)| action(text, css, timeout)),
    )
}

fn text_arguments(
    args: &[String],
    timeout_allowed: bool,
) -> Result<(String, String, Option<u64>), String> {
    let bound = bind(args, &["text", "selector", "by", "timeout"])?;
    let text = literal(bound[0], "text")?;
    let selector = selector_or_body(bound[1])?;
    check_by(bound[2])?;
    let timeout = match bound[3] {
        None => None,
        Some(timeout) if timeout_allowed => Some(
            timeout
                .parse::<u64>()
                .map_err(|_| "the timeout must be a whole number of seconds".to_owned())?,
        ),
        Some(_) => {
            return Err(
                "a timeout needs manual migration: the assertion waits for the \
                        default timeout (set_timeout)"
                    .to_owned(),
            )
        }
    };
    Ok((text, selector, timeout))
}

/// A deferred text call: `names` are the Python parameters, the text first
/// and the selector second (`body` when it is left out). The timeout and the
/// `fs` flag cannot be carried over.
fn text_call_with(
    args: &[String],
    names: &[&str],
    action: impl FnOnce(String, String) -> Action,
) -> ParseOutcome {
    outcome(deferred_text_arguments(args, names).map(|(text, css)| action(text, css)))
}

fn deferred_text_arguments(args: &[String], names: &[&str]) -> Result<(String, String), String> {
    let bound = bind(args, names)?;
    let text = literal(bound[0], "text")?;
    let selector = selector_or_body(bound[1])?;
    check_by(bound[2])?;
    if bound[3..].iter().any(Option::is_some) {
        return Err("the timeout and fs arguments need manual migration".to_owned());
    }
    Ok((text, selector))
}

/// A call that takes a selector (its first parameter) and nothing that Rust
/// can carry over: `by` must be CSS and there must be no timeout.
fn selector_only(
    args: &[String],
    names: &[&str],
    action: impl FnOnce(String) -> Action,
) -> ParseOutcome {
    outcome(selector_arguments(args, names).map(action))
}

fn selector_arguments(args: &[String], names: &[&str]) -> Result<String, String> {
    let bound = bind(args, names)?;
    let selector = literal(bound[0], "selector")?;
    check_by(bound[1])?;
    if bound[2..].iter().any(Option::is_some) {
        return Err("the timeout and fs arguments need manual migration".to_owned());
    }
    Ok(selector)
}

/// A call like Python's `type(selector, text, by, timeout)`: a selector and a
/// string, then arguments Rust cannot carry over.
fn selector_and_text(
    args: &[String],
    names: &[&str],
    action: impl FnOnce(String, String) -> Action,
) -> ParseOutcome {
    outcome(selector_text_arguments(args, names).map(|(selector, text)| action(selector, text)))
}

fn selector_text_arguments(args: &[String], names: &[&str]) -> Result<(String, String), String> {
    let bound = bind(args, names)?;
    let selector = literal(bound[0], "selector")?;
    let text = literal(bound[1], names[1])?;
    check_by(bound[2])?;
    if bound[3..].iter().any(Option::is_some) {
        return Err("the timeout and retry arguments need manual migration".to_owned());
    }
    Ok((selector, text))
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

fn wait_action(args: &[String], visible: bool) -> ParseOutcome {
    outcome(wait_arguments(args).map(|(selector, timeout)| {
        if visible {
            Action::WaitVisible(selector, timeout)
        } else {
            Action::WaitPresent(selector, timeout)
        }
    }))
}

/// Python's `wait_for_element_visible(selector, by="css selector", timeout)`:
/// the timeout is the third parameter, so it is given by keyword.
fn wait_arguments(args: &[String]) -> Result<(String, u64), String> {
    let bound = bind(args, &["selector", "by", "timeout"])?;
    let selector = literal(bound[0], "selector")?;
    check_by(bound[1])?;
    let timeout = match bound[2] {
        None => DEFAULT_WAIT_SECONDS,
        Some(timeout) => timeout
            .parse::<u64>()
            .map_err(|_| "the timeout must be a whole number of seconds".to_owned())?,
    };
    Ok((selector, timeout))
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
    let mut output =
        String::from("use seleniumbase_rs::{BrowserConfig, run_browser_test};\n\n#[tokio::test]\n");
    output.push_str(&format!(
        "async fn {}() -> seleniumbase_rs::Result<()> {{\n",
        rust_identifier(test_name)
    ));
    output.push_str("    run_browser_test(BrowserConfig::default(), async |sb| {\n");
    for action in actions {
        output.push_str("        ");
        output.push_str(&render_action(action));
        output.push('\n');
    }
    output.push_str("        Ok(())\n    }).await\n}\n");
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
        Action::AssertText(text, css) => format!("sb.assert_text({text:?}, {css:?}).await?;"),
        Action::AssertExactText(text, css) => {
            format!("sb.assert_exact_text({text:?}, {css:?}).await?;")
        }
        Action::AssertTextNotVisible(text, css) => {
            format!("sb.assert_text_not_visible({text:?}, {css:?}).await?;")
        }
        Action::AssertExactTextNotVisible(text, css) => {
            format!("sb.assert_exact_text_not_visible({text:?}, {css:?}).await?;")
        }
        Action::WaitText(text, css, timeout) => {
            format!("sb.wait_for_text({text:?}, {css:?}, {timeout}).await?;")
        }
        Action::WaitExactText(text, css, timeout) => {
            format!("sb.wait_for_exact_text({text:?}, {css:?}, {timeout}).await?;")
        }
        Action::DeferredElement(css) => format!("sb.deferred_assert_element({css:?}).await?;"),
        Action::DeferredText(text, css) => {
            format!("sb.deferred_assert_text({text:?}, {css:?}).await?;")
        }
        Action::DeferredExactText(text, css) => {
            format!("sb.deferred_assert_exact_text({text:?}, {css:?}).await?;")
        }
        Action::ProcessDeferred => "sb.process_deferred_asserts().await?;".to_owned(),
        Action::EnterMfaCode(css, secret) => {
            format!("sb.enter_mfa_code({css:?}, {secret:?}).await?;")
        }
        Action::AddText(css, text) => format!("sb.add_text({css:?}, {text:?}).await?;"),
        Action::AssertTitle(title) => format!("sb.assert_title({title:?}).await?;"),
        Action::AssertTitleContains(title) => {
            format!("sb.assert_title_contains({title:?}).await?;")
        }
        Action::AssertUrl(url) => format!("sb.assert_url({url:?}).await?;"),
        Action::AssertUrlContains(url) => format!("sb.assert_url_contains({url:?}).await?;"),
        Action::GetText(css) => format!("let _text = sb.get_text({css:?}).await?;"),
        Action::GetAttribute(css, attr) => {
            format!("let _value = sb.get_attribute({css:?}, {attr:?}).await?;")
        }
        Action::Check(css) => format!("sb.check_if_unchecked({css:?}).await?;"),
        Action::Uncheck(css) => format!("sb.uncheck_if_checked({css:?}).await?;"),
        Action::DoubleClick(css) => format!("sb.double_click({css:?}).await?;"),
        Action::RightClick(css) => format!("sb.right_click({css:?}).await?;"),
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
            // `{:?}` keeps the decimal point (`1.0`), which a float literal needs.
            format!("tokio::time::sleep(std::time::Duration::from_secs_f64({seconds:?})).await;")
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
            .contains("sb.assert_text(\"Welcome\", \"main\")"));
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
self.check("#agree")
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
                result.rust.contains(&format!("from_secs_f64({seconds})")),
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
            "sb.assert_text(\"Welcome\", \"body\").await?;",
            "sb.assert_exact_text(\"Hi\", \"body\").await?;",
            "sb.assert_text(\"Shown\", \"body\").await?;",
            "sb.assert_text_not_visible(\"Gone\", \"body\").await?;",
            "sb.deferred_assert_text(\"Terms\", \"body\").await?;",
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
            "sb.wait_for_text(\"Ready\", \"body\", 10).await?;",
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
