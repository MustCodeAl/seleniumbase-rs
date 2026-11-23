//! JSON-LD (Schema.org) structured data extraction.

use std::sync::LazyLock;

use scraper::{Html, Selector as ScraperSelector};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::SeleniumBaseError;

static JSON_LD_SEL: LazyLock<ScraperSelector> = LazyLock::new(|| {
    ScraperSelector::parse("script[type='application/ld+json']")
        .expect("valid JSON-LD script selector")
});

#[inline]
fn element_text(element: &scraper::ElementRef) -> String {
    let mut text = String::new();
    for chunk in element.text() {
        text.push_str(chunk);
    }
    text.trim().to_string()
}

/// Extracts all raw JSON-LD blocks from `<script type="application/ld+json">` elements.
///
/// Unwraps top-level arrays and flattens nested Schema.org `@graph` structures into individual items.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::extract::extract_json_ld_raw;
///
/// let html = r#"
///     <script type="application/ld+json">
///     {
///         "@context": "https://schema.org",
///         "@type": "Product",
///         "name": "Rust Mechanical Keyboard",
///         "offers": { "@type": "Offer", "price": "149.99" }
///     }
///     </script>
/// "#;
///
/// let items = extract_json_ld_raw(html);
/// assert_eq!(items.len(), 1);
/// assert_eq!(items[0]["name"], "Rust Mechanical Keyboard");
/// ```
#[must_use]
pub fn extract_json_ld_raw(html_text: &str) -> Vec<Value> {
    let document = Html::parse_document(html_text);
    let mut results = Vec::new();

    for element in document.select(&JSON_LD_SEL) {
        let trimmed = element_text(&element);
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(parsed) = serde_json::from_str::<Value>(&trimmed) {
            flatten_json_ld_item(parsed, &mut results);
        }
    }

    results
}

fn flatten_json_ld_item(value: Value, out: &mut Vec<Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                flatten_json_ld_item(item, out);
            }
        }
        Value::Object(map) => {
            // Check if this object contains an "@graph" array
            if let Some(Value::Array(graph_items)) = map.get("@graph") {
                for graph_item in graph_items {
                    out.push(graph_item.clone());
                }
            } else {
                out.push(Value::Object(map));
            }
        }
        _ => {}
    }
}

/// Extracts and deserializes JSON-LD structured data into typed Rust models `T`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Json`] if deserialization into `T` fails.
///
/// # Examples
///
/// ```
/// use serde::Deserialize;
/// use seleniumbase_rs::extract::extract_json_ld;
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct Product {
///     name: String,
///     description: String,
/// }
///
/// let html = r#"
///     <script type="application/ld+json">
///     {
///         "@context": "https://schema.org",
///         "@type": "Product",
///         "name": "Ferris Mascot",
///         "description": "Official plush crab."
///     }
///     </script>
/// "#;
///
/// let products: Vec<Product> = extract_json_ld(html).unwrap();
/// assert_eq!(products.len(), 1);
/// assert_eq!(products[0].name, "Ferris Mascot");
/// ```
pub fn extract_json_ld<T: DeserializeOwned>(html_text: &str) -> Result<Vec<T>, SeleniumBaseError> {
    let raw_items = extract_json_ld_raw(html_text);
    let mut typed_items = Vec::with_capacity(raw_items.len());

    for item in raw_items {
        // Attempt to parse into T; if T matches, collect it
        if let Ok(val) = serde_json::from_value::<T>(item) {
            typed_items.push(val);
        }
    }

    Ok(typed_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Article {
        headline: String,
        author: String,
    }

    #[test]
    fn parses_single_and_graph_json_ld() {
        let html = r#"
            <!doctype html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "NewsArticle",
                    "headline": "Rust Concurrency Revolution",
                    "author": "Alice"
                }
                </script>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@graph": [
                        {
                            "@type": "NewsArticle",
                            "headline": "Zero-Cost Browser Automation",
                            "author": "Bob"
                        }
                    ]
                }
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let raw = extract_json_ld_raw(html);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0]["headline"], "Rust Concurrency Revolution");
        assert_eq!(raw[1]["headline"], "Zero-Cost Browser Automation");

        let articles: Vec<Article> = extract_json_ld(html).unwrap();
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].author, "Alice");
        assert_eq!(articles[1].author, "Bob");
    }
}
