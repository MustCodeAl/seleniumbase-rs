//! HTML parsing helpers (BeautifulSoup-style convenience wrappers).

use crate::error::SeleniumBaseError;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;
use std::rc::Rc;

/// Wrapper around `scraper::Html` providing simplified query helpers.
#[derive(Clone, Debug)]
pub struct BeautifulSoup {
    document: Rc<Html>,
}

impl BeautifulSoup {
    /// Parses HTML from a string.
    pub fn parse(html: &str) -> Self {
        Self {
            document: Rc::new(Html::parse_document(html)),
        }
    }

    /// Parses HTML from a fragment.
    pub fn parse_fragment(html: &str) -> Self {
        Self {
            document: Rc::new(Html::parse_fragment(html)),
        }
    }

    /// Finds the first element matching a CSS selector.
    pub fn find(&self, selector: &str) -> Result<Option<SoupNode>, SeleniumBaseError> {
        let sel = Selector::parse(selector).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Bad selector '{selector}': {e:?}"))
        })?;
        Ok(self
            .document
            .select(&sel)
            .next()
            .map(SoupNode::from_element))
    }

    /// Finds all elements matching a CSS selector.
    pub fn find_all(&self, selector: &str) -> Result<Vec<SoupNode>, SeleniumBaseError> {
        let sel = Selector::parse(selector).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Bad selector '{selector}': {e:?}"))
        })?;
        Ok(self
            .document
            .select(&sel)
            .map(SoupNode::from_element)
            .collect())
    }

    /// Returns the inner text of the first element matching `selector`.
    pub fn get_text(&self, selector: &str) -> Result<Option<String>, SeleniumBaseError> {
        Ok(self.find(selector)?.map(|e| e.text))
    }

    /// Returns the value of an attribute on the first matching element.
    pub fn get_attribute(
        &self,
        selector: &str,
        attr: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        Ok(self
            .find(selector)?
            .and_then(|e| e.attributes.get(attr).cloned()))
    }
}

/// A snapshot of a parsed element with owned data.
#[derive(Clone, Debug, Default)]
pub struct SoupNode {
    pub tag_name: String,
    pub text: String,
    pub attributes: HashMap<String, String>,
    pub outer_html: String,
    pub inner_html: String,
}

impl SoupNode {
    fn from_element(element: ElementRef<'_>) -> Self {
        Self {
            tag_name: element.value().name.local.as_ref().to_owned(),
            text: element.text().collect::<Vec<_>>().join(""),
            attributes: element
                .value()
                .attrs()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            outer_html: element.html(),
            inner_html: element.inner_html(),
        }
    }

    /// Returns the value of an attribute, if present.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attributes.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_elements_and_text() {
        let html = r#"<html><body><h1>Title</h1><ul><li class="item">A</li><li class="item">B</li></ul></body></html>"#;
        let soup = BeautifulSoup::parse(html);
        assert_eq!(soup.get_text("h1").unwrap(), Some("Title".to_owned()));
        let items = soup.find_all(".item").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "A");
        assert_eq!(items[1].text, "B");
    }

    #[test]
    fn get_attribute() {
        let html = r#"<a href="https://example.com" id="link">Go</a>"#;
        let soup = BeautifulSoup::parse(html);
        assert_eq!(
            soup.get_attribute("a", "href").unwrap(),
            Some("https://example.com".to_owned())
        );
        assert_eq!(soup.get_attribute("a", "missing").unwrap(), None);
    }
}
