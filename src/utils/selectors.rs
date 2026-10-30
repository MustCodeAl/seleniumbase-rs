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
//! A selector that crosses shadow roots, such as `my-app::shadow button`, is
//! CSS as a whole, whatever its first piece looks like: the engines cut it at
//! each `::shadow` (see [`utils::shadow`](crate::utils::shadow)) and read the
//! first piece as any selector, and the others as CSS inside the shadow root
//! of the element before them. One that ends on `::shadow` is refused.

//!
//! [`Selector`] borrows the string, [`SelectorBuf`] owns it and is what the
//! Pure CDP engine ([`sb_cdp`](crate::sb_cdp)) sends to the page already
//! tagged with its kind. Use the explicit constructors ([`SelectorBuf::css`],
//! [`SelectorBuf::xpath`]) when a string would be misread: the guess never
//! applies to them.

use thirtyfour::By;

use crate::error::SeleniumBaseError;
use crate::utils::shadow::{is_shadow_selector, shadow_chain};

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

/// SeleniumBase's own `:contains("text")` pseudo-class. It is not CSS, so a
/// browser's `querySelector` rejects it; SeleniumBase rewrites it as XPath.
const CONTAINS_PSEUDO: &str = ":contains(";

/// The XPath for a CSS selector that uses `:contains()`, or `None` when the
/// selector is more than the converter understands (it is then left as CSS and
/// the browser reports the problem).
fn contains_to_xpath(css: &str) -> Option<String> {
    css_to_xpath(css).ok()
}

/// `text` as an XPath string literal, quoted so that any content is safe.
fn xpath_string_literal(text: &str) -> String {
    if !text.contains('\'') {
        format!("'{text}'")
    } else if !text.contains('"') {
        format!("\"{text}\"")
    } else {
        let parts: Vec<String> = text.split('\'').map(|part| format!("'{part}'")).collect();
        format!("concat({})", parts.join(", \"'\", "))
    }
}

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
        // The engines cut a selector that crosses a shadow root into pieces
        // when they look the element up, and read each piece's kind then: the
        // first may be XPath, the others are CSS.
        if is_shadow_selector(selector) {
            return Self::Css(selector);
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
    /// A CSS selector that uses SeleniumBase's `:contains("text")` pseudo-class,
    /// which is not CSS and which browsers reject, becomes the equivalent XPath
    /// expression, as it does in SeleniumBase.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] when the text is empty
    /// or contains only whitespace.
    pub fn to_by(self) -> Result<By, SeleniumBaseError> {
        match self {
            // A selector that crosses a shadow root stays whole: the session
            // cuts it into pieces and finds each in its own root. A selector
            // that cannot be cut is refused here, before anything waits.
            Self::Css(value) if is_shadow_selector(value) => {
                shadow_chain(value)?;
                Ok(By::Css(value.to_owned()))
            }

            Self::Css(value) if value.contains(CONTAINS_PSEUDO) && !value.trim().is_empty() => {
                Ok(match contains_to_xpath(value) {
                    Some(xpath) => By::XPath(xpath),
                    None => By::Css(value.to_owned()),
                })
            }
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

    /// Creates a selector for elements that contain `text`, the way
    /// SeleniumBase's `find_element_by_text` finds them.
    ///
    /// With a `tag`, that is every `tag` element whose text contains `text`,
    /// which is what `tag:contains("text")` means in SeleniumBase. Without one it
    /// is the innermost elements that hold the text themselves, so a page's
    /// `<html>` and `<body>` do not match just because they contain everything.
    /// Runs of white space in `text` and in the page count as one space.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::SelectorBuf;
    ///
    /// assert_eq!(
    ///     SelectorBuf::containing_text("Sign in", Some("button")),
    ///     SelectorBuf::xpath("//button[contains(normalize-space(.), 'Sign in')]"),
    /// );
    /// ```
    pub fn containing_text(text: impl AsRef<str>, tag: Option<&str>) -> Self {
        let wanted = text
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let literal = xpath_string_literal(&wanted);
        let tag = tag
            .map(str::trim)
            .filter(|tag| {
                !tag.is_empty()
                    && tag
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '*'))
            })
            .map(str::to_owned);
        match tag {
            Some(tag) => Self::XPath(format!("//{tag}[contains(normalize-space(.), {literal})]")),
            None => Self::XPath(format!(
                "//*[text()[contains(normalize-space(.), {literal})]]"
            )),
        }
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

    /// Elements that show `text`, the way Playwright's `getByText` finds them:
    /// the innermost elements whose text contains `text`, ignoring ASCII case,
    /// or (with `exact`) whose whole text is `text`, case and all. Buttons made
    /// with `<input type="submit">` (and `button`, `reset`) match by their
    /// value. Runs of white space count as one space, and `<script>` and
    /// `<style>` never match.
    ///
    /// This is an approximation written in XPath 1.0: only ASCII letters are
    /// folded for the case-insensitive match.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::SelectorBuf;
    ///
    /// let SelectorBuf::XPath(xpath) = SelectorBuf::text("Sign in", true) else {
    ///     unreachable!()
    /// };
    /// assert!(xpath.contains("normalize-space(.)='Sign in'"));
    /// ```
    pub fn text(text: impl AsRef<str>, exact: bool) -> Self {
        let text = text.as_ref();
        let content = text_match(".", text, exact);
        let value = text_match("@value", text, exact);
        Self::XPath(format!(
            "//body/descendant-or-self::*\
             [not(self::script or self::style or self::noscript or self::template)]\
             [({content} and not(.//*[{content}])) or \
             (self::input and (@type='button' or @type='submit' or @type='reset') and {value})]"
        ))
    }

    /// The element itself when its text contains `text`, ignoring ASCII case:
    /// Playwright's `filter({ hasText })`. As a step after another selector
    /// (`page.locator("li").locator(SelectorBuf::has_text("Shoes"))`) it keeps
    /// the matches of that selector whose text contains `text`.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::SelectorBuf;
    ///
    /// assert!(matches!(SelectorBuf::has_text("Shoes"), SelectorBuf::XPath(x) if x.starts_with("self::*[")));
    /// ```
    pub fn has_text(text: impl AsRef<str>) -> Self {
        Self::XPath(format!(
            "self::*[{}]",
            text_match(".", text.as_ref(), false)
        ))
    }

    /// Form controls labelled `text`, the way Playwright's `getByLabel` finds
    /// them: by a `<label for>` that names the control's id, by a `<label>`
    /// around the control, or by its `aria-label`. The label matches as in
    /// [`text`](Self::text): containing `text` with ASCII case ignored, or the
    /// whole label when `exact`. `aria-labelledby` is not followed.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::SelectorBuf;
    ///
    /// let SelectorBuf::XPath(xpath) = SelectorBuf::label("Email", false) else {
    ///     unreachable!()
    /// };
    /// assert!(xpath.contains("//label["));
    /// ```
    pub fn label(text: impl AsRef<str>, exact: bool) -> Self {
        let text = text.as_ref();
        let by_label = text_match(".", text, exact);
        let by_aria = text_match("@aria-label", text, exact);
        Self::XPath(format!(
            "//*[(self::input and not(@type='hidden')) or self::textarea or self::select or \
             self::button or self::meter or self::output or self::progress or \
             @role='textbox' or @role='combobox' or @role='checkbox' or @role='radio' or \
             @role='switch' or @role='slider' or @role='spinbutton' or @role='listbox']\
             [(@id and @id=//label[{by_label}]/@for) or ancestor::label[{by_label}] or \
             (@aria-label and {by_aria})]"
        ))
    }

    /// Elements with the ARIA `role`, and with `name` as their accessible name
    /// when one is given: an approximation of Playwright's `getByRole` (and of
    /// Puppeteer's `aria/` selectors) in XPath, for engines without an
    /// accessibility tree query.
    ///
    /// An element has the role when its `role` attribute says so, or when it
    /// has no `role` attribute and is the native element for it: `<button>`
    /// and `<input type="submit">` for `button`, `<a href>` for `link`,
    /// `<input type="checkbox">` for `checkbox`, `<h1>` to `<h6>` for
    /// `heading`, `<select>` for `combobox`, and so on for the common roles.
    /// An empty `role` matches any element.
    ///
    /// The name matches the `aria-label`, the text, the `value`, the `title` or
    /// the `alt` of the element, or the text of its `<label>`: containing
    /// `name` with ASCII case ignored, or the whole of it when `exact`. With no
    /// role, the text match takes the innermost element, so the page's
    /// `<body>` does not match because it contains everything.
    ///
    /// Not covered: `aria-labelledby`, hidden elements being skipped, roles
    /// that depend on context (a `<header>` inside an `<article>` is not a
    /// banner), and states such as `checked` or `level`.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::SelectorBuf;
    ///
    /// let SelectorBuf::XPath(xpath) = SelectorBuf::role("button", Some("Sign in"), false) else {
    ///     unreachable!()
    /// };
    /// assert!(xpath.starts_with("//*[@role='button' or (not(@role) and (self::button"));
    /// assert!(xpath.contains("'sign in'"));
    /// ```
    pub fn role(role: &str, name: Option<&str>, exact: bool) -> Self {
        let role = role.trim();
        let mut xpath = String::from("//*");
        if !role.is_empty() {
            let explicit = format!("@role={}", xpath_string_literal(role));
            match implicit_role(role) {
                Some(native) => {
                    xpath.push_str(&format!("[{explicit} or (not(@role) and ({native}))]"));
                }
                None => xpath.push_str(&format!("[{explicit}]")),
            }
        }
        if let Some(name) = name {
            let by = |value: &str| text_match(value, name, exact);
            let content = by(".");
            let content = if role.is_empty() {
                format!("({content} and not(.//*[{content}]))")
            } else {
                content
            };
            xpath.push_str(&format!(
                "[{} or {content} or {} or {} or {} or (@id and @id=//label[{}]/@for) or \
                 ancestor::label[{}]]",
                by("@aria-label"),
                by("@value"),
                by("@title"),
                by("@alt"),
                by("."),
                by("."),
            ));
        }
        Self::XPath(xpath)
    }
}

/// The ASCII capitals and their lower-case forms, for XPath's `translate()`:
/// XPath 1.0 has no `lower-case()`, so case-insensitive matching folds ASCII
/// letters only.
const XPATH_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// See [`XPATH_UPPER`].
const XPATH_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";

/// An XPath test that the string of `value` (an XPath expression such as `.`
/// or `@title`) matches `text`, white space collapsed on both sides: equal
/// when `exact`, otherwise containing `text` with ASCII case ignored.
fn text_match(value: &str, text: &str, exact: bool) -> String {
    let wanted = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if exact {
        format!("normalize-space({value})={}", xpath_string_literal(&wanted))
    } else {
        format!(
            "contains(translate(normalize-space({value}), '{XPATH_UPPER}', '{XPATH_LOWER}'), {})",
            xpath_string_literal(&wanted.to_ascii_lowercase())
        )
    }
}

/// The XPath test, on the element itself, for the native elements that have
/// `role` without a `role` attribute, or `None` for a role no native element
/// has.
fn implicit_role(role: &str) -> Option<&'static str> {
    Some(match role {
        "article" => "self::article",
        "banner" => "self::header",
        "button" => {
            "self::button or self::summary or (self::input and (@type='button' or \
             @type='submit' or @type='reset' or @type='image'))"
        }
        "cell" => "self::td",
        "checkbox" => "self::input and @type='checkbox'",
        "columnheader" => "self::th",
        "combobox" => {
            "(self::select and not(@multiple) and not(@size > 1)) or (self::input and @list)"
        }
        "complementary" => "self::aside",
        "contentinfo" => "self::footer",
        "definition" => "self::dd",
        "dialog" => "self::dialog",
        "figure" => "self::figure",
        "form" => "self::form",
        "group" => "self::fieldset or self::details or self::optgroup",
        "heading" => "self::h1 or self::h2 or self::h3 or self::h4 or self::h5 or self::h6",
        "img" => "self::img and not(@alt='')",
        "link" => "(self::a or self::area) and @href",
        "list" => "self::ul or self::ol or self::menu",
        "listbox" => "(self::select and (@multiple or @size > 1)) or self::datalist",
        "listitem" => "self::li",
        "main" => "self::main",
        "meter" => "self::meter",
        "navigation" => "self::nav",
        "option" => "self::option",
        "progressbar" => "self::progress",
        "radio" => "self::input and @type='radio'",
        "region" => "self::section",
        "row" => "self::tr",
        "rowgroup" => "self::thead or self::tbody or self::tfoot",
        "searchbox" => "self::input and @type='search'",
        "separator" => "self::hr",
        "slider" => "self::input and @type='range'",
        "spinbutton" => "self::input and @type='number'",
        "status" => "self::output",
        "table" => "self::table",
        "term" => "self::dt or self::dfn",
        "textbox" => {
            "self::textarea or (self::input and (not(@type) or @type='' or @type='text' or \
             @type='email' or @type='tel' or @type='url' or @type='password'))"
        }
        _ => return None,
    })
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
    /// Classifies `selector` like SeleniumBase does. CSS that uses
    /// `:contains("text")` is converted to XPath, which is how SeleniumBase
    /// runs it too.
    fn from(selector: &str) -> Self {
        match Selector::auto(selector) {
            Selector::Css(css) if css.contains(CONTAINS_PSEUDO) => {
                contains_to_xpath(css).map_or_else(|| Self::Css(css.to_owned()), Self::XPath)
            }
            other => other.into(),
        }
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
pub(crate) fn css_quoted(value: &str, quote: char) -> String {
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

/// The most selectors one XPath may fan out to when it uses `or`.
const MAX_XPATH_ALTERNATIVES: usize = 16;

/// Reads the XPath forms [`xpath_to_css`] converts. Every method returns
/// `None` for a form it does not convert, and the whole conversion then fails.
struct XPathReader<'a> {
    rest: &'a str,
}

impl<'a> XPathReader<'a> {
    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    /// Consumes `token` if the text begins with it.
    fn eat(&mut self, token: &str) -> bool {
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    /// Consumes an operator word such as `and`, which must be followed by
    /// white space.
    fn eat_operator(&mut self, word: &str) -> bool {
        match self.rest.strip_prefix(word) {
            Some(rest) if rest.starts_with(char::is_whitespace) => {
                self.rest = rest.trim_start();
                true
            }
            _ => false,
        }
    }

    /// An element or attribute name.
    fn name(&mut self) -> Option<&'a str> {
        let len = self
            .rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(self.rest.len());
        let (name, rest) = self.rest.split_at(len);
        if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            return None;
        }
        self.rest = rest;
        Some(name)
    }

    /// A quoted string, without its quotes.
    fn literal(&mut self) -> Option<&'a str> {
        let quote = self
            .rest
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"'))?;
        let inner = &self.rest[1..];
        let end = inner.find(quote)?;
        self.rest = &inner[end + 1..];
        Some(&inner[..end])
    }

    /// A whole number.
    fn number(&mut self) -> Option<usize> {
        let len = self
            .rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(self.rest.len());
        let number = self.rest[..len].parse().ok()?;
        self.rest = &self.rest[len..];
        Some(number)
    }

    /// `@name`, `@name='value'`, `contains(@name, 'value')` or
    /// `starts-with(@name, 'value')`, as the CSS that tests the same.
    fn test(&mut self) -> Option<String> {
        if self.eat("@") {
            let name = self.name()?;
            let before_space = self.rest;
            self.skip_space();
            if !self.eat("=") {
                self.rest = before_space;
                return Some(format!("[{name}]"));
            }
            self.skip_space();
            return Some(format!("[{name}={}]", css_quoted(self.literal()?, '\'')));
        }
        let operator = if self.eat("contains(") {
            "*="
        } else if self.eat("starts-with(") {
            "^="
        } else {
            return None;
        };
        self.skip_space();
        if !self.eat("@") {
            return None;
        }
        let name = self.name()?;
        self.skip_space();
        if !self.eat(",") {
            return None;
        }
        self.skip_space();
        // XPath finds the empty string in every string; `[a*='']` matches
        // nothing.
        let value = self.literal().filter(|value| !value.is_empty())?;
        self.skip_space();
        if !self.eat(")") {
            return None;
        }
        Some(format!("[{name}{operator}{}]", css_quoted(value, '\'')))
    }

    /// Tests joined with `and`, as one run of CSS qualifiers.
    fn and_list(&mut self) -> Option<String> {
        let mut css = self.test()?;
        loop {
            let before_space = self.rest;
            self.skip_space();
            if !self.eat_operator("and") {
                self.rest = before_space;
                return Some(css);
            }
            css.push_str(&self.test()?);
        }
    }

    /// `and` lists joined with `or`: one CSS qualifier run for each.
    fn or_list(&mut self) -> Option<Vec<String>> {
        let mut alternatives = vec![self.and_list()?];
        loop {
            let before_space = self.rest;
            self.skip_space();
            if !self.eat_operator("or") {
                self.rest = before_space;
                return Some(alternatives);
            }
            alternatives.push(self.and_list()?);
        }
    }

    /// One step, `tag[position][tests]...`, as the compound selectors it
    /// stands for (several when a test uses `or`), with its tag.
    fn step(&mut self) -> Option<(&'a str, Vec<String>)> {
        let tag = if self.eat("*") { "*" } else { self.name()? };
        let mut position = None;
        let mut variants = vec![String::new()];
        let mut first = true;
        while self.eat("[") {
            self.skip_space();
            let before_number = self.rest;
            if let Some(number) = self.number() {
                // `tag[n]` is the nth element of the tag among its siblings,
                // which is `:nth-of-type`. After another test it would be the
                // nth of the elements that passed it, which CSS cannot say.
                if !first || number == 0 {
                    return None;
                }
                position = Some(number);
            } else {
                self.rest = before_number;
                let alternatives = self.or_list()?;
                variants = variants
                    .iter()
                    .flat_map(|variant| {
                        alternatives
                            .iter()
                            .map(move |alternative| format!("{variant}{alternative}"))
                    })
                    .collect();
                if variants.len() > MAX_XPATH_ALTERNATIVES {
                    return None;
                }
            }
            self.skip_space();
            if !self.eat("]") {
                return None;
            }
            first = false;
        }
        let pseudo = match position {
            None => String::new(),
            Some(number) if tag == "*" => format!(":nth-child({number})"),
            Some(number) => format!(":nth-of-type({number})"),
        };
        let compounds = variants
            .into_iter()
            .map(|variant| format!("{tag}{variant}{pseudo}"))
            .collect();
        Some((tag, compounds))
    }

    /// The whole expression.
    fn path(&mut self) -> Option<String> {
        // Each step with whether a `/` (child) rather than a `//` (descendant)
        // joins it to the step before.
        let mut steps: Vec<(bool, Vec<String>)> = Vec::new();
        if self.eat("id(") {
            self.skip_space();
            let id = self
                .literal()
                .filter(|id| !id.contains(char::is_whitespace))?;
            self.skip_space();
            if !self.eat(")") {
                return None;
            }
            steps.push((false, vec![format!("[id={}]", css_quoted(id, '\''))]));
        } else {
            let from_root = if self.eat(".//") || self.eat("//") {
                false
            } else if self.eat("/") {
                true
            } else {
                return None;
            };
            let (tag, compounds) = self.step()?;
            // Only `html` is certain to be the root, as CSS `html` is.
            if from_root && tag != "html" {
                return None;
            }
            steps.push((false, compounds));
        }
        while !self.rest.is_empty() {
            let child = if self.eat("//") {
                false
            } else if self.eat("/") {
                true
            } else {
                return None;
            };
            let (_, compounds) = self.step()?;
            steps.push((child, compounds));
        }
        let mut selectors = vec![String::new()];
        for (index, (child, compounds)) in steps.iter().enumerate() {
            let joint = match (index, child) {
                (0, _) => "",
                (_, true) => " > ",
                (_, false) => " ",
            };
            selectors = selectors
                .iter()
                .flat_map(|prefix| {
                    compounds
                        .iter()
                        .map(move |compound| format!("{prefix}{joint}{compound}"))
                })
                .collect();
            if selectors.len() > MAX_XPATH_ALTERNATIVES {
                return None;
            }
        }
        Some(selectors.join(", "))
    }
}

/// Converts an XPath expression to the equivalent CSS selector.
///
/// Only the forms that have an exact CSS equivalent are converted:
///
/// | XPath | CSS |
/// |---|---|
/// | `//div` | `div` |
/// | `//*[@id='x']` | `*[id='x']` |
/// | `//input[@type="text" and @name='q']` | `input[type='text'][name='q']` |
/// | `//a[@href][@target='_blank']` | `a[href][target='_blank']` |
/// | `//form//input`, `//ul/li` | `form input`, `ul > li` |
/// | `/html/body/div` | `html > body > div` |
/// | `//li[2]`, `//*[2]` | `li:nth-of-type(2)`, `*:nth-child(2)` |
/// | `//a[contains(@href,'x')]` | `a[href*='x']` |
/// | `//a[starts-with(@href,'x')]` | `a[href^='x']` |
/// | `id('x')` | `[id='x']` |
/// | `//a[@x='1' or @y='2']` | `a[x='1'], a[y='2']` |
///
/// A position converts only when it is the first test of its step
/// (`li[2][@class='x']`, not `li[@class='x'][2]`: the second counts only the
/// elements that passed the first test, which CSS cannot say). An `or` becomes
/// a comma-separated list, and is refused when the list would pass
/// sixteen selectors. A leading `.//` is accepted, and a leading `/` only
/// before `html`. `starts-with()` is an addition of this crate: Python
/// SeleniumBase has no such form.
///
/// Everything else (`text()`, `contains(.,...)`, `not()`, `last()`, other
/// axes such as `parent::`, unions, a position after another test) is
/// refused: a CSS selector that quietly left part of the XPath out would
/// match other elements than the XPath does. Python SeleniumBase converts
/// some of those to a `:contains()` pseudo-class that is not CSS.
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
/// assert_eq!(xpath_to_css("//ul/li[2]")?, "ul > li:nth-of-type(2)");
/// assert!(xpath_to_css("//div[text()='x']").is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn xpath_to_css(xpath: &str) -> Result<String, SeleniumBaseError> {
    let trimmed = xpath.trim();
    if trimmed.is_empty() {
        return Err(SeleniumBaseError::InvalidSelector("empty xpath".to_owned()));
    }
    XPathReader { rest: trimmed }
        .path()
        .ok_or_else(|| SeleniumBaseError::InvalidSelector(format!("unsupported xpath: {xpath}")))
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
    fn css_with_contains_runs_as_xpath_for_the_driver() {
        let by = Selector::auto("button:contains(\"Go on\")")
            .to_by()
            .unwrap();
        assert_eq!(
            variant_name(by),
            variant_name(By::XPath("//button[contains(., 'Go on')]".to_owned()))
        );
    }

    #[test]
    fn contains_css_the_converter_cannot_read_stays_css() {
        // An unterminated string is not something it can turn into XPath.
        let by = Selector::Css("a:contains(\"open").to_by().unwrap();
        assert!(variant_name(by).contains("Css"));
        assert_eq!(
            SelectorBuf::from("a:contains(\"open"),
            SelectorBuf::css("a:contains(\"open")
        );
    }

    #[test]
    fn a_selector_that_crosses_a_shadow_root_is_css_and_stays_whole() {
        for selector in [
            "my-app::shadow button",
            "my-app ::shadow .form ::shadow input",
            // The first piece may be XPath: the pieces are read when the
            // element is looked up.
            "//my-app::shadow button",
            "button:contains(\"Go\")::shadow a",
        ] {
            assert_eq!(Selector::auto(selector), Selector::Css(selector));
            assert_eq!(SelectorBuf::from(selector), SelectorBuf::css(selector));
            let by = Selector::auto(selector).to_by().unwrap();
            assert_eq!(format!("{by}"), format!("CSS({selector})"));
        }
    }

    #[test]
    fn a_selector_that_ends_on_a_shadow_root_is_refused_by_every_method() {
        for selector in [
            "my-app ::shadow",
            "my-app::shadow",
            "a ::shadow b ::shadow  ",
        ] {
            let error = Selector::auto(selector).to_by().unwrap_err();
            assert!(
                matches!(&error, SeleniumBaseError::InvalidSelector(m)
                    if m.contains("cannot end on a shadow root")),
                "{selector:?}: {error}"
            );
        }
    }

    #[test]
    fn a_name_that_only_starts_with_shadow_is_no_shadow_selector() {
        assert_eq!(
            Selector::auto("//a/parent::shadow-host"),
            Selector::XPath("//a/parent::shadow-host")
        );
        assert_eq!(Selector::auto("a::shadowed"), Selector::Css("a::shadowed"));
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
            // Paths of several steps: `/` is a child, `//` a descendant.
            ("//div/span", "div > span"),
            ("//div//span", "div span"),
            (
                "//form[@id='f']//input[@name='q']",
                "form[id='f'] input[name='q']",
            ),
            (".//ul/li", "ul > li"),
            ("/html/body/div", "html > body > div"),
            // A position that comes before the other tests of its step.
            ("//div[2]", "div:nth-of-type(2)"),
            ("//ul/li[1]", "ul > li:nth-of-type(1)"),
            ("//*[3]", "*:nth-child(3)"),
            ("//li[2][@class='x']", "li[class='x']:nth-of-type(2)"),
            // contains() and starts-with() on an attribute.
            ("//a[contains(@href,'x')]", "a[href*='x']"),
            ("//a[contains( @class , \"it's\" )]", "a[class*='it\\'s']"),
            ("//a[starts-with(@href, '/x')]", "a[href^='/x']"),
            (
                "//a[@rel='x' and contains(@class,'b')]",
                "a[rel='x'][class*='b']",
            ),
            // id() and `or`.
            ("id('main')", "[id='main']"),
            ("id(\"main\")//a", "[id='main'] a"),
            ("//div[@id='a' or @id='b']", "div[id='a'], div[id='b']"),
            (
                "//div[@a='1' and @b='2' or @c]",
                "div[a='1'][b='2'], div[c]",
            ),
            ("//div[@a or @b]/span", "div[a] > span, div[b] > span"),
            (
                "//div[@a or @b][@c or @d]",
                "div[a][c], div[a][d], div[b][c], div[b][d]",
            ),
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
            "//a[contains(@class,'')]",
            "//a[starts-with(@class,'')]",
            "//a[contains(@class,x)]",
            "//a[contains(text(),'x')]",
            "//div[last()]",
            // A position after another test counts among the elements that
            // passed it; `:nth-of-type` counts every sibling of the tag.
            "//div[@id='a'][2]",
            "//div[2][3]",
            "//div[0]",
            "//div[@id='a' or 2]",
            "//div[@id='a' and 1=1]",
            "//div[@id='a'and@x='b']",
            "//div[@id=a]",
            "//div[@id='a]",
            "//div[@id='a'",
            "//div[@id='a']junk",
            "//div[@1abc='a']",
            "//div[@a:b='c']",
            "//div | //span",
            "/div",
            "/div/span",
            "//div/",
            "//div/@id",
            "//div/..",
            "id('a b')",
            "id('a')x",
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
    fn xpath_to_css_refuses_an_or_that_fans_out_too_far() {
        let sixteen = format!("//a[{}]", vec!["@x"; 16].join(" or "));
        assert_eq!(xpath_to_css(&sixteen).unwrap().matches(", ").count(), 15);
        let seventeen = format!("//a[{}]", vec!["@x"; 17].join(" or "));
        assert!(xpath_to_css(&seventeen).is_err());
    }

    #[test]
    fn xpath_to_css_names_the_xpath_it_could_not_convert() {
        let message = xpath_to_css("//a[text()='Home']").unwrap_err().to_string();
        assert!(message.contains("unsupported xpath"), "{message}");
        assert!(message.contains("//a[text()='Home']"), "{message}");
        let empty = xpath_to_css("").unwrap_err().to_string();
        assert!(empty.contains("empty xpath"), "{empty}");
    }

    fn xpath_of(selector: SelectorBuf) -> String {
        match selector {
            SelectorBuf::XPath(xpath) => xpath,
            other => panic!("not XPath: {other:?}"),
        }
    }

    #[test]
    fn text_matches_ignore_ascii_case_unless_exact() {
        assert_eq!(
            text_match(".", "  Sign   In ", false),
            "contains(translate(normalize-space(.), 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', \
             'abcdefghijklmnopqrstuvwxyz'), 'sign in')"
        );
        assert_eq!(
            text_match("@title", "Sign In", true),
            "normalize-space(@title)='Sign In'"
        );
    }

    #[test]
    fn a_quote_in_the_text_cannot_end_the_xpath_string() {
        let xpath = xpath_of(SelectorBuf::text(r#"it's "quoted"']"#, true));
        assert!(
            xpath.contains(r#"concat('it', "'", 's "quoted"', "'", ']')"#),
            "{xpath}"
        );
        let xpath = xpath_of(SelectorBuf::role("x'] | //script[('", Some("n"), false));
        assert!(
            xpath.starts_with(r#"//*[@role="x'] | //script[('"]["#),
            "{xpath}"
        );
    }

    #[test]
    fn text_takes_the_innermost_element_and_input_buttons() {
        let xpath = xpath_of(SelectorBuf::text("Go", false));
        assert!(xpath.starts_with("//body/descendant-or-self::*"), "{xpath}");
        assert!(xpath.contains("and not(.//*[contains("), "{xpath}");
        assert!(xpath.contains("@type='submit'"), "{xpath}");
        assert!(xpath.contains("normalize-space(@value)"), "{xpath}");
    }

    #[test]
    fn has_text_tests_the_element_itself() {
        assert_eq!(
            xpath_of(SelectorBuf::has_text("Shoes")),
            "self::*[contains(translate(normalize-space(.), 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', \
             'abcdefghijklmnopqrstuvwxyz'), 'shoes')]"
        );
        assert!(matches!(
            Selector::auto(&xpath_of(SelectorBuf::has_text("x"))),
            Selector::XPath(_)
        ));
    }

    #[test]
    fn label_follows_for_wrapping_labels_and_aria_label() {
        let xpath = xpath_of(SelectorBuf::label("Email", true));
        assert!(
            xpath.contains("@id=//label[normalize-space(.)='Email']/@for"),
            "{xpath}"
        );
        assert!(
            xpath.contains("ancestor::label[normalize-space(.)='Email']"),
            "{xpath}"
        );
        assert!(
            xpath.contains("normalize-space(@aria-label)='Email'"),
            "{xpath}"
        );
        assert!(xpath.contains("not(@type='hidden')"), "{xpath}");
    }

    #[test]
    fn role_uses_the_role_attribute_or_the_native_element() {
        let button = xpath_of(SelectorBuf::role("button", None, false));
        assert_eq!(
            button,
            "//*[@role='button' or (not(@role) and (self::button or self::summary or \
             (self::input and (@type='button' or @type='submit' or @type='reset' or \
             @type='image'))))]"
        );
        let custom = xpath_of(SelectorBuf::role("tab", None, false));
        assert_eq!(custom, "//*[@role='tab']");
        let any = xpath_of(SelectorBuf::role("", Some("Sign in"), true));
        assert!(any.starts_with("//*[normalize-space(@aria-label)='Sign in' or (normalize-space(.)='Sign in' and not(.//*["), "{any}");
    }

    #[test]
    fn role_names_match_text_label_value_title_and_alt() {
        let xpath = xpath_of(SelectorBuf::role("link", Some("About"), false));
        for source in ["@aria-label", ".", "@value", "@title", "@alt"] {
            assert!(
                xpath.contains(&format!("normalize-space({source})")),
                "{source}: {xpath}"
            );
        }
        assert!(xpath.contains("ancestor::label["), "{xpath}");
        assert!(
            xpath.contains("(self::a or self::area) and @href"),
            "{xpath}"
        );
    }

    #[test]
    fn every_role_helper_xpath_is_read_as_xpath() {
        for selector in [
            SelectorBuf::text("a", false),
            SelectorBuf::label("a", false),
            SelectorBuf::role("button", Some("a"), false),
            SelectorBuf::role("", None, false),
        ] {
            let xpath = xpath_of(selector);
            assert!(
                matches!(Selector::auto(&xpath), Selector::XPath(_)),
                "{xpath}"
            );
        }
    }
}

// -------------------------------------------------------------------------
// CSS selector -> XPath
// -------------------------------------------------------------------------

/// `s` as an XPath string literal, quoted so that any content is safe.
fn xpath_literal(s: &str) -> String {
    if !s.contains('\'') {
        format!("'{s}'")
    } else if !s.contains('"') {
        format!("\"{s}\"")
    } else {
        let parts: Vec<String> = s.split('\'').map(|part| format!("'{part}'")).collect();
        format!("concat({})", parts.join(", \"'\", "))
    }
}

/// How a CSS combinator relates a compound selector to the one before it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CssCombinator {
    Descendant,
    Child,
    NextSibling,
    LaterSibling,
}

/// One compound selector (`tag#id.class[attr]:pseudo`) as an XPath step.
struct CssStep {
    /// Element name, or `*`.
    name: String,
    /// XPath predicates the element must satisfy.
    predicates: Vec<String>,
}

impl CssStep {
    fn render(&self) -> String {
        self.predicates
            .iter()
            .fold(self.name.clone(), |step, predicate| {
                format!("{step}[{predicate}]")
            })
    }
}

/// Reads one CSS selector list from a string.
struct CssParser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    source: &'a str,
}

impl<'a> CssParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().peekable(),
            source,
        }
    }

    fn error(&self, reason: &str) -> SeleniumBaseError {
        SeleniumBaseError::InvalidSelector(format!(
            "cannot convert CSS selector '{}' to XPath: {reason}",
            self.source
        ))
    }

    fn skip_whitespace(&mut self) -> bool {
        let mut skipped = false;
        while self.chars.next_if(|c| c.is_whitespace()).is_some() {
            skipped = true;
        }
        skipped
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
    }

    /// An identifier; a backslash makes the next character literal.
    fn ident(&mut self) -> String {
        let mut ident = String::new();
        loop {
            match self.chars.peek() {
                Some('\\') => {
                    self.chars.next();
                    if let Some(c) = self.chars.next() {
                        ident.push(c);
                    }
                }
                Some(&c) if Self::is_ident_char(c) => {
                    ident.push(c);
                    self.chars.next();
                }
                _ => return ident,
            }
        }
    }

    /// A quoted string or a bare identifier or number.
    fn value(&mut self) -> Result<String, SeleniumBaseError> {
        match self.chars.peek().copied() {
            Some(quote @ ('"' | '\'')) => {
                self.chars.next();
                let mut value = String::new();
                loop {
                    match self.chars.next() {
                        Some(c) if c == quote => return Ok(value),
                        Some('\\') => value.extend(self.chars.next()),
                        Some(c) => value.push(c),
                        None => return Err(self.error("unterminated string")),
                    }
                }
            }
            _ => {
                let value = self.ident();
                if value.is_empty() {
                    Err(self.error("expected a value"))
                } else {
                    Ok(value)
                }
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), SeleniumBaseError> {
        self.skip_whitespace();
        match self.chars.next() {
            Some(c) if c == expected => Ok(()),
            _ => Err(self.error(&format!("expected '{expected}'"))),
        }
    }

    /// `[name]`, `[name=value]` and the other attribute forms.
    fn attribute(&mut self) -> Result<String, SeleniumBaseError> {
        self.skip_whitespace();
        let name = self.ident();
        if name.is_empty() {
            return Err(self.error("expected an attribute name"));
        }
        self.skip_whitespace();
        let operator = match self.chars.next() {
            Some(']') => return Ok(format!("@{name}")),
            Some('=') => "=".to_owned(),
            Some(c @ ('~' | '|' | '^' | '$' | '*')) => {
                if self.chars.next() != Some('=') {
                    return Err(self.error("expected '=' in an attribute selector"));
                }
                format!("{c}=")
            }
            _ => return Err(self.error("malformed attribute selector")),
        };
        self.skip_whitespace();
        let value = self.value()?;
        self.skip_whitespace();
        self.expect(']')?;
        let attr = format!("@{name}");
        let literal = xpath_literal(&value);
        let empty_or_blank = value.is_empty();
        Ok(match operator.as_str() {
            "=" => format!("{attr}={literal}"),
            // The value of `~=` is a whole word, so it cannot be empty or hold a space.
            "~=" if empty_or_blank || value.contains(char::is_whitespace) => "false()".to_owned(),
            "~=" => format!(
                "contains(concat(' ', normalize-space({attr}), ' '), {})",
                xpath_literal(&format!(" {value} "))
            ),
            "|=" => format!(
                "({attr}={literal} or starts-with({attr}, {}))",
                xpath_literal(&format!("{value}-"))
            ),
            // These three match nothing when the value is empty.
            "^=" | "$=" | "*=" if empty_or_blank => "false()".to_owned(),
            "^=" => format!("starts-with({attr}, {literal})"),
            "$=" => format!(
                "substring({attr}, string-length({attr}) - string-length({literal}) + 1) = {literal}"
            ),
            _ => format!("contains({attr}, {literal})"),
        })
    }

    /// `:pseudo` and `:pseudo(argument)`.
    fn pseudo(&mut self, name: &str) -> Result<Vec<String>, SeleniumBaseError> {
        let pseudo = self.ident().to_ascii_lowercase();
        let argument = if self.chars.next_if_eq(&'(').is_some() {
            self.skip_whitespace();
            let argument = if matches!(self.chars.peek(), Some('"' | '\'')) {
                self.value()?
            } else {
                let mut raw = String::new();
                while let Some(c) = self.chars.next_if(|c| *c != ')') {
                    raw.push(c);
                }
                raw.trim().to_owned()
            };
            self.expect(')')?;
            Some(argument)
        } else {
            None
        };
        let same_name = |preceding: bool| -> Result<String, SeleniumBaseError> {
            if name == "*" {
                return Err(self.error(&format!(":{pseudo} needs an element name")));
            }
            let axis = if preceding { "preceding" } else { "following" };
            Ok(format!("not({axis}-sibling::{name})"))
        };
        let position = |argument: &str, counted: &str| -> Result<String, SeleniumBaseError> {
            let before = format!("count(preceding-sibling::{counted})");
            match argument.to_ascii_lowercase().as_str() {
                "odd" => Ok(format!("{before} mod 2 = 0")),
                "even" => Ok(format!("{before} mod 2 = 1")),
                number => match number.parse::<u32>() {
                    Ok(n) if n >= 1 => Ok(format!("{before} = {}", n - 1)),
                    _ => Err(self.error(&format!("unsupported position '{argument}'"))),
                },
            }
        };
        let predicate = match (pseudo.as_str(), argument.as_deref()) {
            ("first-child", None) => "not(preceding-sibling::*)".to_owned(),
            ("last-child", None) => "not(following-sibling::*)".to_owned(),
            ("only-child", None) => {
                "not(preceding-sibling::*) and not(following-sibling::*)".to_owned()
            }
            ("first-of-type", None) => same_name(true)?,
            ("last-of-type", None) => same_name(false)?,
            ("empty", None) => "not(node())".to_owned(),
            ("nth-child", Some(n)) => position(n, "*")?,
            ("nth-of-type", Some(n)) if name != "*" => position(n, name)?,
            ("contains", Some(text)) => format!("contains(., {})", xpath_literal(text)),
            _ => return Err(self.error(&format!("unsupported pseudo-class ':{pseudo}'"))),
        };
        Ok(vec![predicate])
    }

    /// One compound selector.
    fn compound(&mut self) -> Result<CssStep, SeleniumBaseError> {
        let (name, named) = if self.chars.next_if_eq(&'*').is_some() {
            ("*".to_owned(), true)
        } else {
            let tag = self.ident().to_ascii_lowercase();
            if tag.is_empty() {
                ("*".to_owned(), false)
            } else {
                (tag, true)
            }
        };
        let mut predicates = Vec::new();
        loop {
            match self.chars.peek().copied() {
                Some('#') => {
                    self.chars.next();
                    let id = self.ident();
                    if id.is_empty() {
                        return Err(self.error("expected an id after '#'"));
                    }
                    predicates.push(format!("@id={}", xpath_literal(&id)));
                }
                Some('.') => {
                    self.chars.next();
                    let class = self.ident();
                    if class.is_empty() {
                        return Err(self.error("expected a class after '.'"));
                    }
                    predicates.push(format!(
                        "contains(concat(' ', normalize-space(@class), ' '), {})",
                        xpath_literal(&format!(" {class} "))
                    ));
                }
                Some('[') => {
                    self.chars.next();
                    predicates.push(self.attribute()?);
                }
                Some(':') => {
                    self.chars.next();
                    predicates.extend(self.pseudo(&name)?);
                }
                _ => break,
            }
        }
        if !named && predicates.is_empty() {
            return Err(self.error("expected a selector"));
        }
        Ok(CssStep { name, predicates })
    }

    /// One complex selector, as an absolute XPath path.
    fn complex(&mut self) -> Result<String, SeleniumBaseError> {
        let mut path = String::new();
        let mut combinator = CssCombinator::Descendant;
        loop {
            let step = self.compound()?;
            path.push_str(&match combinator {
                CssCombinator::Descendant => format!("//{}", step.render()),
                CssCombinator::Child => format!("/{}", step.render()),
                CssCombinator::NextSibling => {
                    let tail = CssStep {
                        name: "*".to_owned(),
                        predicates: Vec::new(),
                    };
                    let own = CssStep {
                        name: format!("self::{}", step.name),
                        predicates: step.predicates,
                    };
                    format!("/following-sibling::{}[1]/{}", tail.render(), own.render())
                }
                CssCombinator::LaterSibling => format!("/following-sibling::{}", step.render()),
            });
            let spaced = self.skip_whitespace();
            combinator = match self.chars.peek().copied() {
                None | Some(',') => return Ok(path),
                Some('>') => CssCombinator::Child,
                Some('+') => CssCombinator::NextSibling,
                Some('~') => CssCombinator::LaterSibling,
                Some(_) if spaced => CssCombinator::Descendant,
                Some(c) => return Err(self.error(&format!("unexpected '{c}'"))),
            };
            if combinator != CssCombinator::Descendant {
                self.chars.next();
                self.skip_whitespace();
            }
        }
    }

    /// A comma-separated list of complex selectors.
    fn list(&mut self) -> Result<String, SeleniumBaseError> {
        let mut paths = Vec::new();
        self.skip_whitespace();
        loop {
            paths.push(self.complex()?);
            if self.chars.next_if_eq(&',').is_none() {
                return Ok(paths.join(" | "));
            }
            self.skip_whitespace();
        }
    }
}

/// Converts the CSS selector `css` to an XPath expression.
pub(crate) fn css_to_xpath(css: &str) -> Result<String, SeleniumBaseError> {
    if css.trim().is_empty() {
        return Err(SeleniumBaseError::InvalidSelector(
            "cannot convert an empty CSS selector to XPath".to_owned(),
        ));
    }
    CssParser::new(css).list()
}

#[cfg(test)]
mod tests_css_to_xpath {
    use super::*;

    fn xpath(css: &str) -> String {
        css_to_xpath(css).unwrap_or_else(|e| panic!("{css}: {e}"))
    }

    #[test]
    fn xpath_literal_quotes_whatever_it_is_given() {
        assert_eq!(xpath_literal("plain"), "'plain'");
        assert_eq!(xpath_literal("it's"), "\"it's\"");
        assert_eq!(xpath_literal(r#"a'b"c"#), r#"concat('a', "'", 'b"c')"#);
        // Not HTML-escaped: `&` is just a character in an XPath string.
        assert_eq!(xpath_literal("a&b"), "'a&b'");
    }

    #[test]
    fn simple_selectors() {
        assert_eq!(xpath("div"), "//div");
        assert_eq!(xpath("*"), "//*");
        assert_eq!(xpath("#main"), "//*[@id='main']");
        assert_eq!(xpath("input#user"), "//input[@id='user']");
        assert_eq!(
            xpath("a.nav.big"),
            "//a[contains(concat(' ', normalize-space(@class), ' '), ' nav ')]\
             [contains(concat(' ', normalize-space(@class), ' '), ' big ')]"
        );
        assert_eq!(
            xpath("DIV"),
            "//div",
            "type selectors are case-insensitive in HTML"
        );
    }

    #[test]
    fn combinators_become_path_steps() {
        assert_eq!(xpath("div a"), "//div//a");
        assert_eq!(xpath("div > a"), "//div/a");
        assert_eq!(xpath("form>input"), "//form/input");
        assert_eq!(xpath("h1 + p"), "//h1/following-sibling::*[1]/self::p");
        assert_eq!(xpath("h1 ~ p"), "//h1/following-sibling::p");
        assert_eq!(xpath("ul, ol"), "//ul | //ol");
    }

    #[test]
    fn attribute_selectors() {
        assert_eq!(xpath("[disabled]"), "//*[@disabled]");
        assert_eq!(xpath("input[name='q']"), "//input[@name='q']");
        assert_eq!(xpath("input[name=q]"), "//input[@name='q']");
        assert_eq!(xpath(r#"a[title="it's"]"#), r#"//a[@title="it's"]"#);
        assert_eq!(xpath("a[href^='/x']"), "//a[starts-with(@href, '/x')]");
        assert_eq!(xpath("a[href*=x]"), "//a[contains(@href, 'x')]");
        assert_eq!(xpath("a[href^='']"), "//a[false()]");
        assert_eq!(
            xpath("a[rel~='nofollow']"),
            "//a[contains(concat(' ', normalize-space(@rel), ' '), ' nofollow ')]"
        );
        assert_eq!(
            xpath("a[lang|=en]"),
            "//a[(@lang='en' or starts-with(@lang, 'en-'))]"
        );
    }

    #[test]
    fn pseudo_classes() {
        assert_eq!(xpath("li:first-child"), "//li[not(preceding-sibling::*)]");
        assert_eq!(xpath("li:last-child"), "//li[not(following-sibling::*)]");
        assert_eq!(
            xpath("li:nth-child(2)"),
            "//li[count(preceding-sibling::*) = 1]"
        );
        assert_eq!(
            xpath("li:nth-of-type(odd)"),
            "//li[count(preceding-sibling::li) mod 2 = 0]"
        );
        assert_eq!(
            xpath("li:first-of-type"),
            "//li[not(preceding-sibling::li)]"
        );
        assert_eq!(
            xpath("button:contains(\"Go on\")"),
            "//button[contains(., 'Go on')]"
        );
        assert_eq!(xpath("td:empty"), "//td[not(node())]");
    }

    #[test]
    fn escaped_identifiers_are_taken_literally() {
        assert_eq!(xpath(r"#a\:b"), "//*[@id='a:b']");
    }

    #[test]
    fn what_cannot_be_converted_is_an_error_not_a_broken_xpath() {
        for css in [
            "",
            "   ",
            "a:hover",
            "a::before",
            "li:nth-child(2n+1)",
            "*:first-of-type",
            "[",
            "a[b=",
            "div >",
            "a[b!=c]",
            "#",
            ".",
            "a,",
        ] {
            let result = css_to_xpath(css);
            assert!(
                matches!(result, Err(SeleniumBaseError::InvalidSelector(_))),
                "{css:?} should be rejected, got {result:?}"
            );
        }
    }
}
