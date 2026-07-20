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

impl<'a> Selector<'a> {
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
}
