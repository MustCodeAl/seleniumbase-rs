use thirtyfour::By;

use crate::error::SeleniumBaseError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selector<'a> {
    LinkText(&'a str),
    PartialLinkText(&'a str),
    Css(&'a str),
    XPath(&'a str),
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
/// a leading XPath axis such as `parent::` or `ancestor::`.
pub fn is_xpath_selector(selector: &str) -> bool {
    let trimmed = selector.trim_start();
    if trimmed.starts_with('/') || trimmed.starts_with("./") || trimmed.starts_with('(') {
        return true;
    }
    XPATH_AXES.iter().any(|axis| {
        trimmed
            .strip_prefix(axis)
            .is_some_and(|rest| rest.starts_with("::"))
    })
}

/// Returns `true` when `selector` uses a link-text prefix.
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
    /// link text, then XPath, then CSS as the fallback.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::utils::selectors::Selector;
    ///
    /// assert_eq!(Selector::auto("#submit"), Selector::Css("#submit"));
    /// assert_eq!(Selector::auto("//div[@id='x']"), Selector::XPath("//div[@id='x']"));
    /// assert_eq!(Selector::auto("parent::div"), Selector::XPath("parent::div"));
    /// assert_eq!(Selector::auto("link=Home"), Selector::LinkText("Home"));
    /// ```
    pub fn auto(selector: &'a str) -> Self {
        // Partial prefixes are checked first: "partial_link_text=" also starts
        // with no link-text prefix, but "p_text=" and friends must not be
        // mistaken for anything else.
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

/// Best-effort conversion of simple XPath expressions to CSS selectors.
pub fn xpath_to_css(xpath: &str) -> Result<String, SeleniumBaseError> {
    let trimmed = xpath.trim();
    // Strip leading //
    let body = trimmed
        .trim_start_matches('/')
        .trim_start_matches('/')
        .trim();
    if body.is_empty() {
        return Err(SeleniumBaseError::InvalidSelector("empty xpath".to_owned()));
    }
    // Split tag and predicate, e.g. div[@id='x']
    let re = regex::Regex::new(r"^([a-zA-Z0-9*]+)(?:\[(.+)\])?$").unwrap();
    let caps = re
        .captures(body)
        .ok_or_else(|| SeleniumBaseError::InvalidSelector(format!("unsupported xpath: {xpath}")))?;
    let tag = caps.get(1).map(|m| m.as_str()).unwrap_or("*");
    let mut css = tag.to_owned();
    if let Some(pred) = caps.get(2).map(|m| m.as_str()) {
        // Support @attr='value' or @attr=\"value\"
        let attr_re = regex::Regex::new(r#"@([a-zA-Z0-9_-]+)\s*=\s*['\"]([^'\"]+)['\"]"#).unwrap();
        for cap in attr_re.captures_iter(pred) {
            let attr = cap.get(1).unwrap().as_str();
            let val = cap.get(2).unwrap().as_str();
            css.push_str(&format!("[{}='{}']", attr, val));
        }
        // Support text()='value' for link text -> :contains not standard CSS, skip
        if pred.contains("text()") {
            return Err(SeleniumBaseError::InvalidSelector(
                "text() predicates are not supported in CSS".to_owned(),
            ));
        }
    }
    Ok(css)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant_name(by: By) -> String {
        format!("{:?}", by)
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
    fn xpath_to_css_basic() {
        assert_eq!(xpath_to_css("//div[@id='x']").unwrap(), "div[id='x']");
        assert_eq!(
            xpath_to_css("//a[@class='link']").unwrap(),
            "a[class='link']"
        );
    }

    #[test]
    fn xpath_to_css_text_fails() {
        assert!(xpath_to_css("//a[text()='Home']").is_err());
    }

    #[test]
    fn auto_detects_css_by_default() {
        assert_eq!(Selector::auto("#submit"), Selector::Css("#submit"));
        assert_eq!(
            Selector::auto("div.card > a"),
            Selector::Css("div.card > a")
        );
        assert_eq!(
            Selector::auto("[data-id='7']"),
            Selector::Css("[data-id='7']")
        );
    }

    #[test]
    fn auto_detects_leading_slash_xpath() {
        assert_eq!(Selector::auto("//div"), Selector::XPath("//div"));
        assert_eq!(Selector::auto("/html/body"), Selector::XPath("/html/body"));
        assert_eq!(Selector::auto("./span"), Selector::XPath("./span"));
        assert_eq!(Selector::auto("(//a)[1]"), Selector::XPath("(//a)[1]"));
    }

    #[test]
    fn auto_detects_xpath_axes() {
        // SeleniumBase 4.53.0 added support for the "parent::" axis.
        assert_eq!(
            Selector::auto("parent::div"),
            Selector::XPath("parent::div")
        );
        assert_eq!(
            Selector::auto("ancestor::form"),
            Selector::XPath("ancestor::form")
        );
        assert_eq!(
            Selector::auto("following-sibling::td"),
            Selector::XPath("following-sibling::td")
        );
        assert_eq!(
            Selector::auto("ancestor-or-self::section"),
            Selector::XPath("ancestor-or-self::section")
        );
    }

    #[test]
    fn auto_does_not_mistake_css_pseudo_elements_for_xpath() {
        // A pseudo-element is preceded by its subject, so it never begins with
        // an axis name.
        assert_eq!(Selector::auto("div::before"), Selector::Css("div::before"));
        assert_eq!(Selector::auto("::after"), Selector::Css("::after"));
        assert_eq!(
            Selector::auto("input::placeholder"),
            Selector::Css("input::placeholder")
        );
        // "parent" as a bare class or element name is still CSS.
        assert_eq!(Selector::auto(".parent"), Selector::Css(".parent"));
        assert_eq!(Selector::auto("parent"), Selector::Css("parent"));
    }

    #[test]
    fn auto_detects_link_text_prefixes() {
        assert_eq!(Selector::auto("link=Home"), Selector::LinkText("Home"));
        assert_eq!(Selector::auto("link_text=Home"), Selector::LinkText("Home"));
        assert_eq!(Selector::auto("text=Home"), Selector::LinkText("Home"));
    }

    #[test]
    fn auto_detects_partial_link_text_prefixes() {
        for prefix in [
            "partial_link=",
            "partial_link_text=",
            "partial_text=",
            "p_link=",
            "p_link_text=",
            "p_text=",
        ] {
            let selector = format!("{prefix}Hom");
            assert_eq!(
                Selector::auto(&selector),
                Selector::PartialLinkText("Hom"),
                "failed for prefix {prefix}"
            );
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
    }

    #[test]
    fn auto_by_rejects_empty_selector() {
        assert!(Selector::auto_by("   ").is_err());
    }

    #[test]
    fn xpath_predicates_are_reported_as_xpath() {
        assert!(is_xpath_selector("//a[text()='Home']"));
        assert!(!is_link_text_selector("//a[text()='Home']"));
        assert!(is_link_text_selector("text=Home"));
        assert!(is_partial_link_text_selector("p_text=Hom"));
    }
}
