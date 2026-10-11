//! HTML parsing helpers (BeautifulSoup-style convenience wrappers).
//!
//! [`BeautifulSoup`] parses a page's source and answers CSS-selector queries
//! on it without a browser, the way `sb.get_beautiful_soup()` does in Python
//! SeleniumBase. Matches come back as [`SoupNode`]s: owned snapshots of an
//! element's tag, attributes, text and markup.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::html::BeautifulSoup;
//!
//! # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let soup = BeautifulSoup::parse(
//!     r#"<ul><li class="item first">A</li><li class="item">B</li></ul>"#,
//! );
//!
//! let items = soup.find_all("li.item")?;
//! assert_eq!(items.len(), 2);
//! assert_eq!(items[1].text, "B");
//! assert!(items[0].has_class("first"));
//! assert_eq!(soup.get_text("li")?, Some("A".to_owned()));
//! # Ok(())
//! # }
//! ```

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
    ///
    /// Like a browser, the parser supplies the `html`, `head` and `body`
    /// elements a snippet leaves out. Use [`parse_fragment`](Self::parse_fragment)
    /// to parse a snippet as it is.
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
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if `selector` is not a
    /// valid CSS selector.
    pub fn find(&self, selector: &str) -> Result<Option<SoupNode>, SeleniumBaseError> {
        let sel = parse_selector(selector)?;
        Ok(self
            .document
            .select(&sel)
            .next()
            .map(SoupNode::from_element))
    }

    /// Finds all elements matching a CSS selector, in document order.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if `selector` is not a
    /// valid CSS selector.
    pub fn find_all(&self, selector: &str) -> Result<Vec<SoupNode>, SeleniumBaseError> {
        let sel = parse_selector(selector)?;
        Ok(self
            .document
            .select(&sel)
            .map(SoupNode::from_element)
            .collect())
    }

    /// Returns the inner text of the first element matching `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if `selector` is not a
    /// valid CSS selector.
    pub fn get_text(&self, selector: &str) -> Result<Option<String>, SeleniumBaseError> {
        Ok(self.find(selector)?.map(|e| e.text))
    }

    /// Returns the value of an attribute on the first matching element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] if `selector` is not a
    /// valid CSS selector.
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

/// Parses a CSS selector, naming it in the error.
fn parse_selector(selector: &str) -> Result<Selector, SeleniumBaseError> {
    Selector::parse(selector)
        .map_err(|e| SeleniumBaseError::InvalidSelector(format!("'{selector}': {e}")))
}

/// A snapshot of a parsed element with owned data.
#[derive(Clone, Debug, Default)]
pub struct SoupNode {
    /// The tag name, in lower case, such as `"li"`.
    pub tag_name: String,
    /// The element's text, with its descendants' text joined and untrimmed.
    pub text: String,
    /// The element's attributes. `class` is the attribute as written, such as
    /// `"item first"`; see [`classes`](Self::classes).
    pub attributes: HashMap<String, String>,
    /// The element's markup, including its own tag.
    pub outer_html: String,
    /// The element's markup, without its own tag.
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

    /// The element's classes, in the order they were written.
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.attributes
            .get("class")
            .map(String::as_str)
            .unwrap_or_default()
            .split_whitespace()
    }

    /// Whether the element has the class `name`.
    pub fn has_class(&self, name: &str) -> bool {
        self.classes().any(|class| class == name)
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

    #[test]
    fn a_selector_that_is_not_css_is_an_invalid_selector_that_names_it() {
        let soup = BeautifulSoup::parse("<p>x</p>");
        for result in [
            soup.find("p[").map(|_| ()),
            soup.find_all("p[").map(|_| ()),
            soup.get_text("p[").map(|_| ()),
            soup.get_attribute("p[", "id").map(|_| ()),
        ] {
            let error = result.unwrap_err();
            assert!(
                matches!(&error, SeleniumBaseError::InvalidSelector(message) if message.contains("'p['")),
                "{error:?}"
            );
        }
    }

    #[test]
    fn the_error_reads_as_a_sentence_not_as_a_debug_dump() {
        let error = BeautifulSoup::parse("<p>x</p>").find("p[").unwrap_err();
        let message = error.to_string();
        assert!(message.starts_with("invalid selector: 'p['"), "{message}");
        assert!(!message.contains("Kind"), "{message}");
    }

    #[test]
    fn a_selector_that_matches_nothing_is_not_an_error() {
        let soup = BeautifulSoup::parse("<p>x</p>");
        assert!(soup.find("h6").unwrap().is_none());
        assert!(soup.find_all("h6").unwrap().is_empty());
        assert_eq!(soup.get_text("h6").unwrap(), None);
    }

    #[test]
    fn a_node_carries_its_tag_markup_and_classes() {
        let soup = BeautifulSoup::parse(r#"<div id="d"><b class="x  y">bold</b> tail</div>"#);
        let div = soup.find("#d").unwrap().unwrap();
        assert_eq!(div.tag_name, "div");
        assert_eq!(div.text, "bold tail");
        assert_eq!(div.inner_html, r#"<b class="x  y">bold</b> tail"#);
        assert!(div.outer_html.starts_with(r#"<div id="d">"#));
        assert_eq!(div.attr("id").as_deref(), Some("d"));
        assert_eq!(div.classes().count(), 0);
        assert!(!div.has_class("x"));

        let bold = soup.find("b").unwrap().unwrap();
        assert_eq!(bold.classes().collect::<Vec<_>>(), ["x", "y"]);
        assert!(bold.has_class("y"));
        assert!(!bold.has_class("z"));
        assert!(!bold.has_class("x y"), "a class is one word");
    }

    #[test]
    fn a_fragment_is_parsed_as_given_and_a_document_gets_its_missing_parts() {
        let snippet = "<li>one</li>";
        assert_eq!(
            BeautifulSoup::parse_fragment(snippet)
                .find("li")
                .unwrap()
                .unwrap()
                .text,
            "one"
        );
        // Parsed as a document, the snippet is wrapped in html and body.
        assert!(BeautifulSoup::parse(snippet)
            .find("body")
            .unwrap()
            .is_some());
        assert!(BeautifulSoup::parse_fragment(snippet)
            .find("body")
            .unwrap()
            .is_none());
    }
}
