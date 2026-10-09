//! Element selectors: telling CSS, XPath and link text apart.
//!
//! Every method that takes a selector accepts a plain string, and the kind is
//! worked out from how the string begins, the same way Python SeleniumBase does
//! it:
//!
//! | Begins with | Kind |
//! |---|---|
//! | `/`, `./`, `../`, `(`, or an XPath axis such as `parent::` | XPath |
//! | `link=`, `link_text=`, `text=` | link text, exactly |
//! | `partial_link=`, `partial_link_text=`, `partial_text=`, `p_link=`, `p_link_text=`, `p_text=` | link text, containing |
//! | anything else | CSS |
//!
//! [`Selector`] borrows the string, [`SelectorBuf`] owns it and is what the
//! Pure CDP engine ([`sb_cdp`](crate::sb_cdp)) sends to the page already
//! tagged with its kind. Use the explicit constructors ([`SelectorBuf::css`],
//! [`SelectorBuf::xpath`]) when a string would be misread: the guess never
//! applies to them.

use thirtyfour::By;

use crate::error::SeleniumBaseError;

/// A selector that borrows its text, with its kind decided.
///
/// [`Selector::auto`] decides the kind from the text; build a variant
/// yourself to skip the guess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selector<'a> {
    /// A link whose text is exactly this.
    LinkText(&'a str),
    /// A link whose text contains this.
    PartialLinkText(&'a str),
    /// A CSS selector.
    Css(&'a str),
    /// An XPath expression.
    XPath(&'a str),
    /// An element id. Unlike `Css("#id")` it needs no escaping, whatever
    /// characters the id contains.
    Id(&'a str),
}

/// XPath axis names that may introduce an expression, per the XPath
/// specification. A selector beginning with one of these followed by `::` is
/// an XPath expression rather than CSS. CSS pseudo-elements such as
/// `div::before` never begin with an axis name, so they are not misread.
const XPATH_AXES: [&str; 13] = [
    "ancestor-or-self",
    "ancestor",
    "attribute",
    "child",
    "descendant-or-self",
    "descendant",
    "following-sibling",
    "following",
    "namespace",
    "parent",
    "preceding-sibling",
    "preceding",
    "self",
];

/// Prefixes that mark a selector as link text, matching SeleniumBase.
const LINK_TEXT_PREFIXES: [&str; 3] = ["link=", "link_text=", "text="];

/// Prefixes that mark a selector as partial link text, matching SeleniumBase.
const PARTIAL_LINK_TEXT_PREFIXES: [&str; 6] = [
    "partial_link_text=",
    "partial_link=",
    "partial_text=",
    "p_link_text=",
    "p_link=",
    "p_text=",
];

/// Returns `true` when `selector` is an XPath expression.
///
/// This mirrors SeleniumBase's `page_utils.is_xpath_selector`, which treats a
/// selector starting with `/`, `./`, or `(` as XPath, and additionally accepts
/// a leading XPath axis such as `parent::` or `ancestor::`, and the parent
/// step `..` (`../div`). None of these can begin a valid CSS selector.
/// Whitespace before the first character is ignored.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::selectors::is_xpath_selector;
///
/// assert!(is_xpath_selector("//div[@id='x']"));
/// assert!(is_xpath_selector("(//a)[2]"));
/// assert!(is_xpath_selector("parent::section"));
/// assert!(!is_xpath_selector("div > a"));
/// assert!(!is_xpath_selector("[name=q]"));
/// ```
pub fn is_xpath_selector(selector: &str) -> bool {
    let trimmed = selector.trim_start();
    if trimmed.starts_with('/')
        || trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed == ".."
        || trimmed.starts_with('(')
    {
        return true;
    }
    XPATH_AXES.iter().any(|axis| {
        trimmed
            .strip_prefix(axis)
            .is_some_and(|rest| rest.starts_with("::"))
    })
}

/// Returns `true` when `selector` uses a link-text prefix.
///
/// The prefix is matched exactly: `LINK=Home` and ` link=Home` are CSS.
pub fn is_link_text_selector(selector: &str) -> bool {
    LINK_TEXT_PREFIXES
        .iter()
        .any(|prefix| selector.starts_with(prefix))
}

/// Returns `true` when `selector` uses a partial-link-text prefix.
pub fn is_partial_link_text_selector(selector: &str) -> bool {
    PARTIAL_LINK_TEXT_PREFIXES
        .iter()
        .any(|prefix| selector.starts_with(prefix))
}

impl<'a> Selector<'a> {
    /// Detects the lookup strategy for `selector` the way SeleniumBase does,
    /// so callers can pass CSS, XPath, or a prefixed link-text selector to the
    /// same method.
    ///
    /// Detection order matches the upstream framework: partial link text, then
    /// link text, then XPath, then CSS as the fallback. The prefixes do not
    /// overlap, so the order only matters for the fallback. Everything after
    /// a link-text prefix is the text, even if it looks like another kind of
    /// selector (`text=//div` is a link whose text is `//div`).
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::Selector;
    ///
    /// assert_eq!(Selector::auto("#submit"), Selector::Css("#submit"));
    /// assert_eq!(Selector::auto("[name=q]"), Selector::Css("[name=q]"));
    /// assert_eq!(Selector::auto("//div[@id='x']"), Selector::XPath("//div[@id='x']"));
    /// assert_eq!(Selector::auto("parent::div"), Selector::XPath("parent::div"));
    /// assert_eq!(Selector::auto("link=Home"), Selector::LinkText("Home"));
    /// assert_eq!(Selector::auto("p_text=Hom"), Selector::PartialLinkText("Hom"));
    /// ```
    pub fn auto(selector: &'a str) -> Self {
        for prefix in PARTIAL_LINK_TEXT_PREFIXES {
            if let Some(rest) = selector.strip_prefix(prefix) {
                return Self::PartialLinkText(rest);
            }
        }
        for prefix in LINK_TEXT_PREFIXES {
            if let Some(rest) = selector.strip_prefix(prefix) {
                return Self::LinkText(rest);
            }
        }
        if is_xpath_selector(selector) {
            return Self::XPath(selector);
        }
        Self::Css(selector)
    }

    /// Converts an auto-detected selector straight into a `By` locator.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] when `selector` is empty
    /// or contains only whitespace.
    pub fn auto_by(selector: &'a str) -> Result<By, SeleniumBaseError> {
        Self::auto(selector).to_by()
    }

    /// Converts this selector into a WebDriver locator.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] when the text is empty
    /// or contains only whitespace.
    pub fn to_by(self) -> Result<By, SeleniumBaseError> {
        match self {
            Self::Css(value) if !value.trim().is_empty() => Ok(By::Css(value.to_owned())),
            Self::XPath(value) if !value.trim().is_empty() => Ok(By::XPath(value.to_owned())),
            Self::Id(value) if !value.trim().is_empty() => Ok(By::Id(value.to_owned())),
            Self::LinkText(value) if !value.trim().is_empty() => Ok(By::LinkText(value.to_owned())),
            Self::PartialLinkText(value) if !value.trim().is_empty() => {
                Ok(By::PartialLinkText(value.to_owned()))
            }
            _ => Err(SeleniumBaseError::InvalidSelector(
                "selector value cannot be empty".to_owned(),
            )),
        }
    }
}

/// An owned element selector, the `String` to [`Selector`]'s `&str`.
///
/// Strings convert into a `SelectorBuf` with the same rules SeleniumBase
/// uses, so `"#login"`, `"//form/button"` and `"link=Sign in"` all work. The
/// kind is decided once, here, and sent to the page already tagged.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::selectors::SelectorBuf;
///
/// assert_eq!(SelectorBuf::from("#login"), SelectorBuf::css("#login"));
/// assert_eq!(SelectorBuf::from("//form/button"), SelectorBuf::xpath("//form/button"));
/// assert_eq!(SelectorBuf::from("link=Sign in"), SelectorBuf::link_text("Sign in"));
///
/// // Name the kind to skip the guessing.
/// assert!(matches!(SelectorBuf::css("//not-really"), SelectorBuf::Css(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SelectorBuf {
    /// A CSS selector.
    Css(String),
    /// An XPath expression.
    #[serde(rename = "xpath")]
    XPath(String),
    /// A link whose text is exactly this.
    LinkText(String),
    /// A link whose text contains this.
    PartialLinkText(String),
}

impl SelectorBuf {
    /// Creates a CSS selector, taken literally with no detection.
    pub fn css(selector: impl Into<String>) -> Self {
        Self::Css(selector.into())
    }

    /// Creates an XPath selector, taken literally with no detection.
    pub fn xpath(expression: impl Into<String>) -> Self {
        Self::XPath(expression.into())
    }

    /// Creates a selector for a link with exactly this text.
    pub fn link_text(text: impl Into<String>) -> Self {
        Self::LinkText(text.into())
    }

    /// Creates a selector for a link whose text contains this.
    pub fn partial_link_text(text: impl Into<String>) -> Self {
        Self::PartialLinkText(text.into())
    }

    /// Borrows this selector as a [`Selector`].
    pub fn as_selector(&self) -> Selector<'_> {
        match self {
            Self::Css(value) => Selector::Css(value),
            Self::XPath(value) => Selector::XPath(value),
            Self::LinkText(value) => Selector::LinkText(value),
            Self::PartialLinkText(value) => Selector::PartialLinkText(value),
        }
    }
}

impl From<Selector<'_>> for SelectorBuf {
    fn from(selector: Selector<'_>) -> Self {
        match selector {
            Selector::Css(value) => Self::Css(value.to_owned()),
            Selector::XPath(value) => Self::XPath(value.to_owned()),
            Selector::LinkText(value) => Self::LinkText(value.to_owned()),
            Selector::PartialLinkText(value) => Self::PartialLinkText(value.to_owned()),
            Selector::Id(value) => Self::Css(format!("[id={}]", css_quoted(value, '"'))),
        }
    }
}

impl From<&str> for SelectorBuf {
    fn from(selector: &str) -> Self {
        Selector::auto(selector).into()
    }
}

impl From<String> for SelectorBuf {
    fn from(selector: String) -> Self {
        Self::from(selector.as_str())
    }
}

impl From<&String> for SelectorBuf {
    fn from(selector: &String) -> Self {
        Self::from(selector.as_str())
    }
}

impl std::fmt::Display for SelectorBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Css(value) | Self::XPath(value) => f.write_str(value),
            Self::LinkText(value) => write!(f, "link={value}"),
            Self::PartialLinkText(value) => write!(f, "partial_link={value}"),
        }
    }
}

/// `value` as a quoted CSS string, safe inside an attribute selector.
///
/// Backslashes, the quote character and line breaks are escaped, so no value
/// can end the string early and change the selector around it.
fn css_quoted(value: &str, quote: char) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push(quote);
    for c in value.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\a "),
            '\r' => quoted.push_str("\\d "),
            '\u{c}' => quoted.push_str("\\c "),
            '\0' => quoted.push('\u{fffd}'),
            c if c == quote => {
                quoted.push('\\');
                quoted.push(c);
            }
            c => quoted.push(c),
        }
    }
    quoted.push(quote);
    quoted
}

/// One `@name` or `@name='value'` test from an XPath predicate.
struct AttributeTest<'a> {
    name: &'a str,
    value: Option<&'a str>,
}

/// Reads one attribute test from the start of `input` and returns it with the
/// text that follows.
fn attribute_test(input: &str) -> Option<(AttributeTest<'_>, &str)> {
    let rest = input.strip_prefix('@')?;
    let name_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    let (name, rest) = rest.split_at(name_len);
    if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    let Some(rest) = rest.trim_start().strip_prefix('=') else {
        return Some((AttributeTest { name, value: None }, rest));
    };
    let rest = rest.trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    let test = AttributeTest {
        name,
        value: Some(&rest[..end]),
    };
    Some((test, &rest[end + 1..]))
}

/// Reads `[test and test][test]...` and returns every test in it.
fn attribute_tests(predicates: &str) -> Option<Vec<AttributeTest<'_>>> {
    let mut tests = Vec::new();
    let mut rest = predicates.trim();
    while !rest.is_empty() {
        rest = rest.strip_prefix('[')?.trim_start();
        loop {
            let (test, after) = attribute_test(rest)?;
            tests.push(test);
            rest = after.trim_start();
            let Some(after_and) = rest.strip_prefix("and") else {
                break;
            };
            if !after_and.starts_with(char::is_whitespace) {
                return None;
            }
            rest = after_and.trim_start();
        }
        rest = rest.strip_prefix(']')?.trim_start();
    }
    Some(tests)
}

/// Converts a simple XPath expression to the equivalent CSS selector.
///
/// Only the simplest expressions have an exact CSS equivalent, and only those
/// are converted: a tag (or `*`) found anywhere in the document, optionally
/// narrowed by attribute tests joined with `and`:
///
/// | XPath | CSS |
/// |---|---|
/// | `//div` | `div` |
/// | `//*[@id='x']` | `*[id='x']` |
/// | `//input[@type="text" and @name='q']` | `input[type='text'][name='q']` |
/// | `//a[@href][@target='_blank']` | `a[href][target='_blank']` |
///
/// A leading `.//` is accepted too. Anything else (`or`, `contains()`,
/// `text()`, positions, paths with more than one step, other axes) is
/// refused: a CSS selector that quietly left part of the XPath out would match
/// other elements than the XPath does.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidSelector`] when `xpath` is empty or is
/// not one of the forms above.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::selectors::xpath_to_css;
///
/// assert_eq!(xpath_to_css("//div[@id='x']")?, "div[id='x']");
/// assert!(xpath_to_css("//div[@id='x' or @id='y']").is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn xpath_to_css(xpath: &str) -> Result<String, SeleniumBaseError> {
    let trimmed = xpath.trim();
    if trimmed.is_empty() {
        return Err(SeleniumBaseError::InvalidSelector("empty xpath".to_owned()));
    }
    let unsupported = || SeleniumBaseError::InvalidSelector(format!("unsupported xpath: {xpath}"));
    let body = trimmed
        .strip_prefix(".//")
        .or_else(|| trimmed.strip_prefix("//"))
        .ok_or_else(unsupported)?;
    let tag_len = if body.starts_with('*') {
        1
    } else {
        body.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(body.len())
    };
    let (tag, predicates) = body.split_at(tag_len);
    if tag.is_empty() || !(tag == "*" || tag.starts_with(|c: char| c.is_ascii_alphabetic())) {
        return Err(unsupported());
    }
    let mut css = tag.to_owned();
    for test in attribute_tests(predicates).ok_or_else(unsupported)? {
        css.push('[');
        css.push_str(test.name);
        if let Some(value) = test.value {
            css.push('=');
            css.push_str(&css_quoted(value, '\''));
        }
        css.push(']');
    }
    Ok(css)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant_name(by: By) -> String {
        format!("{by:?}")
    }

    /// The kind and text `Selector::auto` finds in `selector`.
    fn classify(selector: &str) -> (&'static str, String) {
        match Selector::auto(selector) {
            Selector::Css(v) => ("css", v.to_owned()),
            Selector::XPath(v) => ("xpath", v.to_owned()),
            Selector::LinkText(v) => ("link", v.to_owned()),
            Selector::PartialLinkText(v) => ("partial", v.to_owned()),
            Selector::Id(v) => ("id", v.to_owned()),
        }
    }

    #[test]
    fn css_selector_to_by() {
        let by = Selector::Css("#id").to_by().unwrap();
        assert!(variant_name(by).contains("Css"));
    }

    #[test]
    fn xpath_selector_to_by() {
        let by = Selector::XPath("//div").to_by().unwrap();
        assert!(variant_name(by).contains("XPath"));
    }

    #[test]
    fn id_selector_to_by() {
        let by = Selector::Id("user").to_by().unwrap();
        assert!(variant_name(by).contains("Id"));
    }

    #[test]
    fn link_text_selector_to_by() {
        let by = Selector::LinkText("Home").to_by().unwrap();
        assert!(variant_name(by).contains("LinkText"));
    }

    #[test]
    fn partial_link_text_selector_to_by() {
        let by = Selector::PartialLinkText("Hom").to_by().unwrap();
        assert!(variant_name(by).contains("PartialLinkText"));
    }

    #[test]
    fn empty_selector_fails() {
        assert!(Selector::Css("  ").to_by().is_err());
    }

    #[test]
    fn every_kind_refuses_an_empty_or_blank_value() {
        for selector in [
            Selector::Css(""),
            Selector::XPath(" \t"),
            Selector::Id(""),
            Selector::LinkText(" "),
            Selector::PartialLinkText(""),
        ] {
            let error = selector.to_by().unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidSelector(_)),
                "{selector:?}"
            );
        }
        assert!(Selector::auto_by("link=").is_err());
        assert!(Selector::auto_by("p_text=  ").is_err());
    }

    /// Python SeleniumBase's rules, written as a table: input, kind, text.
    #[test]
    fn classification_follows_the_python_rules() {
        let table = [
            // XPath: a leading slash, ./ or an opening parenthesis.
            ("//div", "xpath", "//div"),
            ("/html/body", "xpath", "/html/body"),
            ("./span", "xpath", "./span"),
            (".//span", "xpath", ".//span"),
            ("(//a)[1]", "xpath", "(//a)[1]"),
            ("((//a)[1])[last()]", "xpath", "((//a)[1])[last()]"),
            ("(", "xpath", "("),
            ("  //div", "xpath", "  //div"),
            // XPath: the parent step and the axes.
            ("../div", "xpath", "../div"),
            ("..", "xpath", ".."),
            ("parent::div", "xpath", "parent::div"),
            ("ancestor-or-self::*", "xpath", "ancestor-or-self::*"),
            (
                "following-sibling::td[1]",
                "xpath",
                "following-sibling::td[1]",
            ),
            ("self::a", "xpath", "self::a"),
            // CSS that merely looks like something else.
            ("[name=q]", "css", "[name=q]"),
            ("[name=\"q\"]", "css", "[name=\"q\"]"),
            ("input[name='q']", "css", "input[name='q']"),
            ("[data-x='//y']", "css", "[data-x='//y']"),
            ("a[href^='/docs']", "css", "a[href^='/docs']"),
            (".parent", "css", ".parent"),
            ("parent", "css", "parent"),
            ("parent > child", "css", "parent > child"),
            ("div::before", "css", "div::before"),
            ("::selection", "css", "::selection"),
            ("*", "css", "*"),
            (":root", "css", ":root"),
            ("#a:not(.b) > c", "css", "#a:not(.b) > c"),
            ("a:nth-child(2n + 1)", "css", "a:nth-child(2n + 1)"),
            ("textarea", "css", "textarea"),
            ("text", "css", "text"),
            ("..parent", "css", "..parent"),
            ("", "css", ""),
            // Link text.
            ("link=Home", "link", "Home"),
            ("link_text=Home", "link", "Home"),
            ("text=Home", "link", "Home"),
            ("link=a=b", "link", "a=b"),
            ("link=", "link", ""),
            ("text=//div", "link", "//div"),
            ("link=link=x", "link", "link=x"),
            ("link= padded ", "link", " padded "),
            // Partial link text, every spelling.
            ("partial_link=Hom", "partial", "Hom"),
            ("partial_link_text=Hom", "partial", "Hom"),
            ("partial_text=Hom", "partial", "Hom"),
            ("p_link=Hom", "partial", "Hom"),
            ("p_link_text=Hom", "partial", "Hom"),
            ("p_text=Hom", "partial", "Hom"),
            // Prefixes are exact: case and leading space matter.
            ("LINK=Home", "css", "LINK=Home"),
            ("Text=Home", "css", "Text=Home"),
            (" link=Home", "css", " link=Home"),
            ("link =Home", "css", "link =Home"),
            // Unicode.
            ("link=Главная", "link", "Главная"),
            ("link=日本語", "link", "日本語"),
            ("p_text=Приват", "partial", "Приват"),
            ("ссылка=x", "css", "ссылка=x"),
            ("#日本語", "css", "#日本語"),
            ("//a[text()='日本語']", "xpath", "//a[text()='日本語']"),
        ];
        for (input, kind, text) in table {
            assert_eq!(classify(input), (kind, text.to_owned()), "input {input:?}");
        }
    }

    #[test]
    fn auto_prefers_partial_prefix_over_link_prefix() {
        // "partial_link_text=" must not be truncated by the shorter
        // "partial_link=" match, and neither may be read as link text.
        assert_eq!(
            Selector::auto("partial_link_text=Check"),
            Selector::PartialLinkText("Check")
        );
    }

    #[test]
    fn auto_by_produces_matching_locators() {
        assert!(variant_name(Selector::auto_by("//div").unwrap()).contains("XPath"));
        assert!(variant_name(Selector::auto_by("parent::div").unwrap()).contains("XPath"));
        assert!(variant_name(Selector::auto_by("#id").unwrap()).contains("Css"));
        assert!(variant_name(Selector::auto_by("link=Home").unwrap()).contains("LinkText"));
        assert!(variant_name(Selector::auto_by("p_link=Hom").unwrap()).contains("PartialLinkText"));
    }

    #[test]
    fn auto_by_rejects_empty_selector() {
        assert!(Selector::auto_by("   ").is_err());
    }

    #[test]
    fn the_predicate_functions_agree_with_auto() {
        assert!(is_xpath_selector("//a[text()='Home']"));
        assert!(!is_link_text_selector("//a[text()='Home']"));
        assert!(is_link_text_selector("text=Home"));
        assert!(!is_link_text_selector("partial_text=Home"));
        assert!(is_partial_link_text_selector("p_text=Hom"));
        assert!(!is_partial_link_text_selector("text=Hom"));
        assert!(!is_xpath_selector("[name=q]"));
        assert!(!is_xpath_selector("a/b"));
    }

    #[test]
    fn a_selector_buf_agrees_with_selector_auto() {
        let expected = [
            ("#login", SelectorBuf::css("#login")),
            ("[name=q]", SelectorBuf::css("[name=q]")),
            ("//form/button", SelectorBuf::xpath("//form/button")),
            ("(//a)[1]", SelectorBuf::xpath("(//a)[1]")),
            ("parent::div", SelectorBuf::xpath("parent::div")),
            ("link=Sign in", SelectorBuf::link_text("Sign in")),
            ("text=Sign in", SelectorBuf::link_text("Sign in")),
            ("partial_link=Sign", SelectorBuf::partial_link_text("Sign")),
            ("p_text=Sign", SelectorBuf::partial_link_text("Sign")),
            ("link=日本語", SelectorBuf::link_text("日本語")),
        ];
        for (input, buf) in expected {
            assert_eq!(SelectorBuf::from(input), buf, "{input:?}");
            assert_eq!(SelectorBuf::from(input.to_owned()), buf);
            assert_eq!(SelectorBuf::from(&input.to_owned()), buf);
            assert_eq!(buf.as_selector(), Selector::auto(input), "{input:?}");
        }
    }

    #[test]
    fn a_selector_buf_serializes_with_its_kind_for_the_page() {
        let cases = [
            (SelectorBuf::css("a"), r#"{"kind":"css","value":"a"}"#),
            (
                SelectorBuf::xpath("//a"),
                r#"{"kind":"xpath","value":"//a"}"#,
            ),
            (
                SelectorBuf::link_text("Hi"),
                r#"{"kind":"link_text","value":"Hi"}"#,
            ),
            (
                SelectorBuf::partial_link_text("Hi"),
                r#"{"kind":"partial_link_text","value":"Hi"}"#,
            ),
        ];
        for (buf, json) in cases {
            assert_eq!(serde_json::to_string(&buf).unwrap(), json);
        }
    }

    #[test]
    fn a_selector_buf_displays_the_way_it_was_written() {
        assert_eq!(SelectorBuf::css("a > b").to_string(), "a > b");
        assert_eq!(SelectorBuf::xpath("//a").to_string(), "//a");
        assert_eq!(SelectorBuf::link_text("Hi").to_string(), "link=Hi");
        assert_eq!(
            SelectorBuf::partial_link_text("Hi").to_string(),
            "partial_link=Hi"
        );
    }

    #[test]
    fn an_id_becomes_an_attribute_selector_that_no_id_can_break() {
        let id = |value| SelectorBuf::from(Selector::Id(value));

        assert_eq!(id("user"), SelectorBuf::css("[id=\"user\"]"));
        assert_eq!(id("a\"b"), SelectorBuf::css("[id=\"a\\\"b\"]"));
        // A trailing backslash must not escape the closing quote.
        assert_eq!(id("a\\"), SelectorBuf::css("[id=\"a\\\\\"]"));
        assert_eq!(id("x\ny"), SelectorBuf::css("[id=\"x\\a y\"]"));
        assert_eq!(id("日本語"), SelectorBuf::css("[id=\"日本語\"]"));
    }

    #[test]
    fn css_quoting_escapes_the_quote_it_uses_and_nothing_else() {
        assert_eq!(css_quoted("it's", '\''), "'it\\'s'");
        assert_eq!(css_quoted("it's", '"'), "\"it's\"");
        assert_eq!(css_quoted("say \"hi\"", '"'), "\"say \\\"hi\\\"\"");
        assert_eq!(css_quoted("", '\''), "''");
        assert_eq!(css_quoted("a\0b", '\''), "'a\u{fffd}b'");
    }

    #[test]
    fn xpath_to_css_converts_what_has_an_exact_equivalent() {
        let table = [
            ("//div", "div"),
            ("//*", "*"),
            (".//span", "span"),
            ("  //p  ", "p"),
            ("//div[@id='x']", "div[id='x']"),
            ("//a[@class='link']", "a[class='link']"),
            ("//a[@class=\"btn primary\"]", "a[class='btn primary']"),
            ("//*[@id='x']", "*[id='x']"),
            (
                "//input[@type='text' and @name='q']",
                "input[type='text'][name='q']",
            ),
            (
                "//input[@type = 'text'  and  @name = \"q\"]",
                "input[type='text'][name='q']",
            ),
            ("//a[@href]", "a[href]"),
            ("//a[@href][@target='_blank']", "a[href][target='_blank']"),
            ("//a[@data-id='7']", "a[data-id='7']"),
            ("//a[@title=\"it's\"]", "a[title='it\\'s']"),
            ("//a[@title='back\\slash']", "a[title='back\\\\slash']"),
            ("//a[@title='日本語']", "a[title='日本語']"),
            ("//my-element[@x='1']", "my-element[x='1']"),
            ("//h1", "h1"),
        ];
        for (xpath, css) in table {
            assert_eq!(xpath_to_css(xpath).unwrap(), css, "{xpath:?}");
        }
    }

    #[test]
    fn xpath_to_css_refuses_what_css_would_change_the_meaning_of() {
        for xpath in [
            "",
            "   ",
            "//",
            "//[@id='x']",
            "//a[text()='Home']",
            "//a[contains(@class,'x')]",
            "//div[2]",
            "//div[last()]",
            "//div[@id='a' or @id='b']",
            "//div[@id='a' and 1=1]",
            "//div[@id='a'and@x='b']",
            "//div[@id=a]",
            "//div[@id='a]",
            "//div[@id='a'",
            "//div[@id='a']junk",
            "//div[@1abc='a']",
            "//div[@a:b='c']",
            "//div/span",
            "//div//span",
            "//div | //span",
            "/div",
            "div",
            "(//div)[1]",
            "//1div",
            "//div[@id='a'] [",
            "parent::div",
            "//a[not(@href)]",
            "//a[@href!='x']",
        ] {
            let error = xpath_to_css(xpath).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidSelector(_)),
                "{xpath:?} -> {error:?}"
            );
        }
    }

    #[test]
    fn xpath_to_css_names_the_xpath_it_could_not_convert() {
        let message = xpath_to_css("//a[text()='Home']").unwrap_err().to_string();
        assert!(message.contains("unsupported xpath"), "{message}");
        assert!(message.contains("//a[text()='Home']"), "{message}");
        let empty = xpath_to_css("").unwrap_err().to_string();
        assert!(empty.contains("empty xpath"), "{empty}");
    }
}
