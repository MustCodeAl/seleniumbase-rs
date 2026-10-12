//! Selectors: Playwright's selector engines and Puppeteer's query handlers,
//! turned into the steps of a pure CDP [`Locator`](crate::sb_cdp::Locator).
//!
//! CSS and XPath pass through. Text, role and label selectors become the
//! [`SelectorBuf`](crate::utils::selectors::SelectorBuf) helpers that
//! approximate them in XPath. What has no counterpart (shadow-root piercing,
//! layout selectors such as `:near()`, regular expressions) is refused with a
//! reason, which the translator turns into a `TODO`.
//!
//! Every loop here is bounded by the length of the selector; nothing recurses
//! on the input.

use crate::utils::selectors::css_quoted;

/// One step of a locator chain.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Step {
    /// A CSS selector.
    Css(String),
    /// An XPath expression.
    XPath(String),
    /// `SelectorBuf::role(role, name, exact)`.
    Role {
        role: String,
        name: Option<String>,
        exact: bool,
    },
    /// `SelectorBuf::text(text, exact)`.
    Text { text: String, exact: bool },
    /// `SelectorBuf::label(text, exact)`.
    Label { text: String, exact: bool },
    /// `SelectorBuf::has_text(text)`: keep the matches whose text contains it.
    HasText(String),
    /// `.nth(n)`.
    Nth(usize),
    /// `.last()`.
    Last,
    /// `.visible()`.
    Visible,
}

/// The Playwright pseudo-classes that are not CSS, and what they would need.
const PLAYWRIGHT_ONLY_PSEUDO: &[&str] = &[
    ":has-text(",
    ":text(",
    ":text-is(",
    ":text-matches(",
    ":nth-match(",
    ":near(",
    ":left-of(",
    ":right-of(",
    ":above(",
    ":below(",
    ":light",
    ":visible",
];

/// A CSS attribute test that `value` is (`exact`) or contains, ignoring case,
/// the attribute's value.
pub(super) fn attribute_css(attribute: &str, value: &str, exact: bool) -> String {
    let quoted = css_quoted(value, '"');
    if exact {
        format!("[{attribute}={quoted}]")
    } else {
        format!("[{attribute}*={quoted} i]")
    }
}

/// The steps for a Playwright selector string (`locator()`, `waitForSelector()`
/// and the selector argument of the page-level actions).
pub(super) fn playwright(selector: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for part in split_chain(selector)? {
        steps.extend(playwright_part(&part)?);
    }
    if steps.is_empty() {
        return Err("an empty selector".to_owned());
    }
    Ok(steps)
}

/// Splits on Playwright's `>>` outside quotes and brackets.
fn split_chain(selector: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0_usize;
    let mut chars = selector.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if c == q {
                    quote = None;
                }
                continue;
            }
            None => {}
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                current.push(c);
            }
            '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            '>' if depth == 0 && chars.peek() == Some(&'>') => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    return Err("the `>>>` combinator".to_owned());
                }
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if quote.is_some() {
        return Err("a selector with an unclosed quote".to_owned());
    }
    parts.push(current);
    Ok(parts)
}

/// `engine=body` when the selector starts with a selector engine name.
fn engine_prefix(part: &str) -> Option<(&str, &str)> {
    let (engine, body) = part.split_once('=')?;
    let valid = engine
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && engine
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'));
    valid.then_some((engine, body))
}

fn playwright_part(part: &str) -> Result<Vec<Step>, String> {
    let part = part.trim();
    if part.is_empty() {
        return Err("an empty part in a `>>` selector".to_owned());
    }
    if let Some((engine, body)) = engine_prefix(part) {
        let body = body.trim();
        return match engine {
            "css" => css_steps(body),
            "xpath" => Ok(vec![Step::XPath(body.to_owned())]),
            "text" => text_selector(body).map(|step| vec![step]),
            "id" => Ok(vec![Step::Css(format!(
                "[id={}]",
                css_quoted(&unquote(body), '"')
            ))]),
            "data-testid" | "data-test-id" | "data-test" => Ok(vec![Step::Css(format!(
                "[{engine}={}]",
                css_quoted(&unquote(body), '"')
            ))]),
            "nth" => match body.parse::<i64>() {
                Ok(-1) => Ok(vec![Step::Last]),
                Ok(n) if n >= 0 => Ok(vec![Step::Nth(usize::try_from(n).unwrap_or(usize::MAX))]),
                _ => Err(format!("the selector part `nth={body}`")),
            },
            "visible" if body == "true" => Ok(vec![Step::Visible]),
            "role" => role_selector(body).map(|step| vec![step]),
            _ => Err(format!("the `{engine}=` selector engine")),
        };
    }
    if part.starts_with("//") || part.starts_with("..") {
        return Ok(vec![Step::XPath(part.to_owned())]);
    }
    if is_quoted(part) {
        return Ok(vec![Step::Text {
            text: unquote(part),
            exact: true,
        }]);
    }
    css_steps(part)
}

/// A CSS selector, with Playwright's `:has-text()`, `:text()` and `:visible`
/// at its end peeled off into steps.
fn css_steps(css: &str) -> Result<Vec<Step>, String> {
    let mut base = css.trim().to_owned();
    let mut filters = Vec::new();
    let mut visible = false;
    loop {
        if let Some(rest) = base.strip_suffix(":visible") {
            visible = true;
            base = rest.trim_end().to_owned();
            continue;
        }
        let Some(text) = trailing_pseudo(&base, &[":has-text(", ":text("]) else {
            break;
        };
        let (rest, argument) = text;
        filters.push(argument);
        base = rest;
    }
    if let Some(pseudo) = PLAYWRIGHT_ONLY_PSEUDO.iter().find(|p| base.contains(**p)) {
        return Err(format!(
            "the Playwright pseudo-class `{}` here",
            pseudo.trim_end_matches('(')
        ));
    }
    if base.is_empty() {
        base = "*".to_owned();
    }
    if !filters.is_empty() && base.contains(',') {
        return Err("`:has-text()` on a selector list".to_owned());
    }
    let mut steps = vec![Step::Css(base)];
    // Peeled from the end, so the last one found was written first.
    steps.extend(filters.into_iter().rev().map(Step::HasText));
    if visible {
        steps.push(Step::Visible);
    }
    Ok(steps)
}

/// When `css` ends with one of `pseudos` (`name(argument)`), the selector
/// before it and the argument, unquoted.
fn trailing_pseudo(css: &str, pseudos: &[&str]) -> Option<(String, String)> {
    if !css.ends_with(')') {
        return None;
    }
    let (at, pseudo) = pseudos
        .iter()
        .filter_map(|p| css.rfind(*p).map(|at| (at, *p)))
        .max_by_key(|(at, _)| *at)?;
    let argument = &css[at + pseudo.len()..css.len() - 1];
    // The argument must be the whole of the parentheses: one quoted string
    // or text without parentheses.
    let plain = !argument.contains('(') && !argument.contains(')');
    if !(is_quoted(argument.trim()) || plain) {
        return None;
    }
    Some((css[..at].trim_end().to_owned(), unquote(argument.trim())))
}

/// `text=` bodies: quoted means the exact text, otherwise containing it.
fn text_selector(body: &str) -> Result<Step, String> {
    if body.starts_with('/') {
        return Err("a text selector with a regular expression".to_owned());
    }
    if is_quoted(body) {
        Ok(Step::Text {
            text: unquote(body),
            exact: true,
        })
    } else {
        Ok(Step::Text {
            text: body.trim().to_owned(),
            exact: false,
        })
    }
}

/// `role=button[name="Sign in"]`: the name matches as a substring, or exactly
/// with an `s` flag (`[name="Sign in" s]`).
fn role_selector(body: &str) -> Result<Step, String> {
    let role_end = body
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(body.len());
    let (role, mut rest) = body.split_at(role_end);
    if role.is_empty() {
        return Err(format!("the role selector `role={body}`"));
    }
    let mut name = None;
    let mut exact = false;
    while let Some(after) = rest.trim_start().strip_prefix('[') {
        let close =
            attribute_end(after).ok_or_else(|| format!("the role selector `role={body}`"))?;
        let attribute = &after[..close];
        rest = &after[close + 1..];
        let (key, value) = attribute
            .split_once('=')
            .ok_or_else(|| format!("the role selector option `[{attribute}]`"))?;
        if key.trim() != "name" {
            return Err(format!("the role selector option `[{}]`", key.trim()));
        }
        let value = value.trim();
        let (value, flag) = match value.rsplit_once(char::is_whitespace) {
            Some((quoted, flag)) if is_quoted(quoted.trim()) => (quoted.trim(), Some(flag)),
            _ => (value, None),
        };
        exact = flag.is_some_and(|f| f.eq_ignore_ascii_case("s"));
        name = Some(unquote(value));
    }
    if !rest.trim().is_empty() {
        return Err(format!("the role selector `role={body}`"));
    }
    Ok(Step::Role {
        role: role.to_owned(),
        name,
        exact,
    })
}

/// The index of the `]` that closes an attribute test, quotes respected.
fn attribute_end(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (at, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, c) {
            (_, '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, ']') => return Some(at),
            (None, _) => {}
        }
    }
    None
}

/// The steps for a Puppeteer selector string: CSS, the `xpath/`, `text/`,
/// `aria/` and `pierce/` prefixes, and the `::-p-text()`, `::-p-xpath()` and
/// `::-p-aria()` pseudo-elements.
pub(super) fn puppeteer(selector: &str) -> Result<Vec<Step>, String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err("an empty selector".to_owned());
    }
    if let Some(rest) = selector.strip_prefix("xpath/") {
        return Ok(vec![Step::XPath(rest.to_owned())]);
    }
    if let Some(rest) = selector.strip_prefix("text/") {
        return Ok(vec![Step::Text {
            text: rest.to_owned(),
            exact: false,
        }]);
    }
    if let Some(rest) = selector.strip_prefix("aria/") {
        return aria(rest).map(|step| vec![step]);
    }
    if selector.starts_with("pierce/") || selector.contains(">>>") {
        return Err(
            "a shadow-DOM piercing selector (`pierce/`, `>>>`): the pure CDP engine's \
             selectors do not enter shadow roots"
                .to_owned(),
        );
    }
    if let Some(at) = selector.find("::-p-") {
        return p_selector(selector, at);
    }
    Ok(vec![Step::Css(selector.to_owned())])
}

/// `prefix::-p-name(argument)`.
fn p_selector(selector: &str, at: usize) -> Result<Vec<Step>, String> {
    let prefix = &selector[..at];
    let rest = &selector[at + "::-p-".len()..];
    let open = rest
        .find('(')
        .ok_or_else(|| format!("the selector `{}`", short(selector)))?;
    let name = &rest[..open];
    let argument = rest[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| "a `::-p-` selector followed by more selector".to_owned())?;
    if argument.contains("::-p-") {
        return Err("several `::-p-` selectors in one".to_owned());
    }
    let argument = unescape_css(&unquote_raw(argument.trim()));
    let step = match name {
        "text" => Step::Text {
            text: argument.clone(),
            exact: false,
        },
        "xpath" => Step::XPath(argument.clone()),
        "aria" => aria(&argument)?,
        _ => return Err(format!("the `::-p-{name}()` selector")),
    };
    if prefix.is_empty() {
        return Ok(vec![step]);
    }
    if prefix.ends_with(char::is_whitespace) {
        // A descendant combinator: the step is searched inside the CSS matches.
        return Ok(vec![Step::Css(prefix.trim().to_owned()), step]);
    }
    if name == "text" {
        // `button::-p-text(Go)`: buttons whose text contains it.
        return Ok(vec![Step::Css(prefix.to_owned()), Step::HasText(argument)]);
    }
    Err(format!("the selector `{}`", short(selector)))
}

/// An ARIA query: `Name[role="button"]`, or `[name="Name"][role="button"]`.
fn aria(query: &str) -> Result<Step, String> {
    let query = query.trim();
    let name_end = if query.starts_with('[') {
        0
    } else {
        query.find('[').unwrap_or(query.len())
    };
    let mut name = query[..name_end].trim().to_owned();
    let mut role = String::new();
    let mut rest = &query[name_end..];
    while let Some(after) = rest.trim_start().strip_prefix('[') {
        let close = attribute_end(after).ok_or_else(|| format!("the ARIA selector `{query}`"))?;
        let attribute = &after[..close];
        rest = &after[close + 1..];
        let (key, value) = attribute
            .split_once('=')
            .ok_or_else(|| format!("the ARIA selector option `[{attribute}]`"))?;
        let value = unquote(value.trim());
        match key.trim() {
            "role" => role = value,
            "name" => name = value,
            other => return Err(format!("the ARIA selector option `[{other}]`")),
        }
    }
    if !rest.trim().is_empty() {
        return Err(format!("the ARIA selector `{query}`"));
    }
    if name.is_empty() && role.is_empty() {
        return Err("an empty ARIA selector".to_owned());
    }
    Ok(Step::Role {
        role,
        name: (!name.is_empty()).then_some(name),
        exact: true,
    })
}

fn is_quoted(text: &str) -> bool {
    text.len() >= 2
        && ((text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\'')))
}

/// The value of a quoted CSS string (backslash escapes read), or the text
/// itself, trimmed, when it is not quoted.
fn unquote(text: &str) -> String {
    let text = text.trim();
    if is_quoted(text) {
        unescape_css(&text[1..text.len() - 1])
    } else {
        text.to_owned()
    }
}

/// The text inside quotes, escapes kept, or the text itself.
fn unquote_raw(text: &str) -> String {
    if is_quoted(text) {
        text[1..text.len() - 1].to_owned()
    } else {
        text.to_owned()
    }
}

/// Reads CSS backslash escapes: `\"` is `"`, `\(` is `(`, and `\41 ` is `A`.
fn unescape_css(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 && chars.peek().is_some_and(char::is_ascii_hexdigit) {
            hex.extend(chars.next());
        }
        if hex.is_empty() {
            out.extend(chars.next());
        } else {
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            let code = u32::from_str_radix(&hex, 16).unwrap_or(0xfffd);
            out.push(
                char::from_u32(code)
                    .filter(|c| *c != '\0')
                    .unwrap_or('\u{fffd}'),
            );
        }
    }
    out
}

fn short(text: &str) -> String {
    let mut out: String = text.chars().take(60).collect();
    if text.chars().count() > 60 {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css(text: &str) -> Step {
        Step::Css(text.to_owned())
    }

    #[test]
    fn playwright_engines() {
        assert_eq!(playwright("#login").unwrap(), [css("#login")]);
        assert_eq!(playwright("css=div > a").unwrap(), [css("div > a")]);
        assert_eq!(
            playwright("xpath=//button").unwrap(),
            [Step::XPath("//button".into())]
        );
        assert_eq!(
            playwright("//form//input").unwrap(),
            [Step::XPath("//form//input".into())]
        );
        assert_eq!(
            playwright("text=Sign in").unwrap(),
            [Step::Text {
                text: "Sign in".into(),
                exact: false
            }]
        );
        assert_eq!(
            playwright("text=\"Sign in\"").unwrap(),
            [Step::Text {
                text: "Sign in".into(),
                exact: true
            }]
        );
        assert_eq!(
            playwright("'Sign in'").unwrap(),
            [Step::Text {
                text: "Sign in".into(),
                exact: true
            }]
        );
        assert_eq!(playwright("id=main").unwrap(), [css("[id=\"main\"]")]);
        assert_eq!(
            playwright("data-testid=place-order").unwrap(),
            [css("[data-testid=\"place-order\"]")]
        );
    }

    #[test]
    fn playwright_chains_and_positions() {
        assert_eq!(
            playwright("form >> text=Go >> nth=1").unwrap(),
            [
                css("form"),
                Step::Text {
                    text: "Go".into(),
                    exact: false
                },
                Step::Nth(1)
            ]
        );
        assert_eq!(playwright("li >> nth=-1").unwrap(), [css("li"), Step::Last]);
        // `>>` inside quotes is text.
        assert_eq!(
            playwright("text=\"a >> b\"").unwrap(),
            [Step::Text {
                text: "a >> b".into(),
                exact: true
            }]
        );
    }

    #[test]
    fn has_text_and_visible_become_steps() {
        assert_eq!(
            playwright("button:has-text(\"Sign in\")").unwrap(),
            [css("button"), Step::HasText("Sign in".into())]
        );
        assert_eq!(
            playwright("article:has-text('a'):has-text(b):visible").unwrap(),
            [
                css("article"),
                Step::HasText("a".into()),
                Step::HasText("b".into()),
                Step::Visible
            ]
        );
        assert_eq!(
            playwright(":has-text(\"x\")").unwrap(),
            [css("*"), Step::HasText("x".into())]
        );
    }

    #[test]
    fn playwright_only_selectors_are_refused_with_a_reason() {
        for selector in [
            "button:near(#x)",
            "li:nth-match(a, 2)",
            "text=/sign/i",
            "internal:role=button",
            "a:has-text(\"x\") > b",
            "a, b:has-text(x)",
        ] {
            assert!(playwright(selector).is_err(), "{selector}");
        }
    }

    #[test]
    fn role_selectors() {
        assert_eq!(
            playwright("role=button[name=\"Sign in\"]").unwrap(),
            [Step::Role {
                role: "button".into(),
                name: Some("Sign in".into()),
                exact: false
            }]
        );
        assert_eq!(
            playwright("role=link[name='About' s]").unwrap(),
            [Step::Role {
                role: "link".into(),
                name: Some("About".into()),
                exact: true
            }]
        );
        assert!(playwright("role=button[pressed]").is_err());
    }

    #[test]
    fn puppeteer_prefixes() {
        assert_eq!(
            puppeteer("xpath///h2").unwrap(),
            [Step::XPath("//h2".into())]
        );
        assert_eq!(
            puppeteer("text/Checkout").unwrap(),
            [Step::Text {
                text: "Checkout".into(),
                exact: false
            }]
        );
        assert_eq!(
            puppeteer("aria/Sign in[role=\"button\"]").unwrap(),
            [Step::Role {
                role: "button".into(),
                name: Some("Sign in".into()),
                exact: true
            }]
        );
        assert_eq!(
            puppeteer("aria/Sign in").unwrap(),
            [Step::Role {
                role: String::new(),
                name: Some("Sign in".into()),
                exact: true
            }]
        );
        assert!(puppeteer("pierce/#shadowed").is_err());
        assert!(puppeteer("div >>> span").is_err());
    }

    #[test]
    fn puppeteer_p_selectors() {
        assert_eq!(
            puppeteer("::-p-text(Checkout)").unwrap(),
            [Step::Text {
                text: "Checkout".into(),
                exact: false
            }]
        );
        assert_eq!(
            puppeteer("div ::-p-text(\"He said: \\\"Hi\\\"\")").unwrap(),
            [
                css("div"),
                Step::Text {
                    text: "He said: \"Hi\"".into(),
                    exact: false
                }
            ]
        );
        assert_eq!(
            puppeteer("button::-p-text(Checkout \\(2 items\\))").unwrap(),
            [css("button"), Step::HasText("Checkout (2 items)".into())]
        );
        assert_eq!(
            puppeteer("::-p-xpath(//h2)").unwrap(),
            [Step::XPath("//h2".into())]
        );
        assert_eq!(
            puppeteer("::-p-aria([name=\"Click me\"][role=\"button\"])").unwrap(),
            [Step::Role {
                role: "button".into(),
                name: Some("Click me".into()),
                exact: true
            }]
        );
        assert!(puppeteer("::-p-text(a) > b").is_err());
    }

    #[test]
    fn attribute_css_quotes_the_value() {
        assert_eq!(
            attribute_css("placeholder", "E\"mail", false),
            "[placeholder*=\"E\\\"mail\" i]"
        );
        assert_eq!(attribute_css("alt", "Logo", true), "[alt=\"Logo\"]");
    }
}
