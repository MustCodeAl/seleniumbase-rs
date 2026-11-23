//! Page metadata and SEO / `OpenGraph` / Twitter Card extraction.

use scraper::{Html, Selector as ScraperSelector};
use std::collections::BTreeMap;

/// Structured metadata extracted from HTML `<head>` tags.
///
/// Includes standard HTML metadata, `OpenGraph` attributes, Twitter Cards,
/// and custom `<meta>` name/property tags.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::extract::PageMetadata;
///
/// let html = r#"
///     <!doctype html>
///     <html>
///     <head>
///         <title>Awesome Product</title>
///         <meta name="description" content="The best product ever made.">
///         <meta property="og:title" content="Awesome Product OG">
///         <meta property="og:image" content="https://example.com/image.png">
///         <meta name="twitter:card" content="summary_large_image">
///         <link rel="canonical" href="https://example.com/product">
///     </head>
///     <body></body>
///     </html>
/// "#;
///
/// let meta = PageMetadata::from_html(html);
/// assert_eq!(meta.title.as_deref(), Some("Awesome Product"));
/// assert_eq!(meta.description.as_deref(), Some("The best product ever made."));
/// assert_eq!(meta.og_title.as_deref(), Some("Awesome Product OG"));
/// assert_eq!(meta.og_image.as_deref(), Some("https://example.com/image.png"));
/// assert_eq!(meta.canonical_url.as_deref(), Some("https://example.com/product"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageMetadata {
    /// The page `<title>` tag content.
    pub title: Option<String>,
    /// Meta description (`<meta name="description" content="...">`).
    pub description: Option<String>,
    /// Meta keywords split into a list (`<meta name="keywords" content="...">`).
    pub keywords: Vec<String>,
    /// Canonical URL (`<link rel="canonical" href="...">`).
    pub canonical_url: Option<String>,
    /// Favicon URL (`<link rel="icon" href="...">` or `shortcut icon`).
    pub favicon: Option<String>,
    /// Author (`<meta name="author" content="...">`).
    pub author: Option<String>,
    /// HTML `lang` attribute or Content-Language.
    pub language: Option<String>,
    /// Charset declaration (`<meta charset="...">`).
    pub charset: Option<String>,
    /// OpenGraph title (`og:title`).
    pub og_title: Option<String>,
    /// OpenGraph description (`og:description`).
    pub og_description: Option<String>,
    /// OpenGraph image URL (`og:image`).
    pub og_image: Option<String>,
    /// OpenGraph URL (`og:url`).
    pub og_url: Option<String>,
    /// OpenGraph type (`og:type`, e.g. "website", "article").
    pub og_type: Option<String>,
    /// OpenGraph site name (`og:site_name`).
    pub og_site_name: Option<String>,
    /// Twitter card type (`twitter:card`).
    pub twitter_card: Option<String>,
    /// Twitter title (`twitter:title`).
    pub twitter_title: Option<String>,
    /// Twitter description (`twitter:description`).
    pub twitter_description: Option<String>,
    /// Twitter image (`twitter:image`).
    pub twitter_image: Option<String>,
    /// All additional raw `<meta>` properties and names.
    pub custom: BTreeMap<String, String>,
}

impl PageMetadata {
    /// Extracts metadata from raw HTML text.
    #[must_use]
    pub fn from_html(html_text: &str) -> Self {
        let document = Html::parse_document(html_text);
        let mut meta = Self::default();

        // Title
        if let Ok(title_sel) = ScraperSelector::parse("title")
            && let Some(title_elem) = document.select(&title_sel).next()
        {
            let text = title_elem
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();
            if !text.is_empty() {
                meta.title = Some(text);
            }
        }

        // HTML lang attribute
        if let Ok(html_sel) = ScraperSelector::parse("html[lang]")
            && let Some(html_elem) = document.select(&html_sel).next()
            && let Some(lang) = html_elem.value().attr("lang")
        {
            meta.language = Some(lang.trim().to_string());
        }

        // Canonical link
        if let Ok(canon_sel) = ScraperSelector::parse("link[rel='canonical']")
            && let Some(canon_elem) = document.select(&canon_sel).next()
            && let Some(href) = canon_elem.value().attr("href")
        {
            meta.canonical_url = Some(href.trim().to_string());
        }

        // Favicon link
        if let Ok(icon_sel) = ScraperSelector::parse("link[rel~='icon']")
            && let Some(icon_elem) = document.select(&icon_sel).next()
            && let Some(href) = icon_elem.value().attr("href")
        {
            meta.favicon = Some(href.trim().to_string());
        }

        // Meta tags
        if let Ok(meta_sel) = ScraperSelector::parse("meta") {
            for element in document.select(&meta_sel) {
                let val = element.value();

                if let Some(cs) = val.attr("charset") {
                    meta.charset = Some(cs.trim().to_string());
                }

                let key = val
                    .attr("name")
                    .or_else(|| val.attr("property"))
                    .or_else(|| val.attr("http-equiv"));
                let content = val.attr("content");

                if let (Some(k), Some(c)) = (key, content) {
                    let k_lower = k.trim().to_ascii_lowercase();
                    let content_trimmed = c.trim().to_string();

                    match k_lower.as_str() {
                        "description" => meta.description = Some(content_trimmed.clone()),
                        "keywords" => {
                            meta.keywords = content_trimmed
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "author" => meta.author = Some(content_trimmed.clone()),
                        "content-language" if meta.language.is_none() => {
                            meta.language = Some(content_trimmed.clone());
                        }
                        "og:title" => meta.og_title = Some(content_trimmed.clone()),
                        "og:description" => meta.og_description = Some(content_trimmed.clone()),
                        "og:image" => meta.og_image = Some(content_trimmed.clone()),
                        "og:url" => meta.og_url = Some(content_trimmed.clone()),
                        "og:type" => meta.og_type = Some(content_trimmed.clone()),
                        "og:site_name" => meta.og_site_name = Some(content_trimmed.clone()),
                        "twitter:card" => meta.twitter_card = Some(content_trimmed.clone()),
                        "twitter:title" => meta.twitter_title = Some(content_trimmed.clone()),
                        "twitter:description" => {
                            meta.twitter_description = Some(content_trimmed.clone())
                        }
                        "twitter:image" => meta.twitter_image = Some(content_trimmed.clone()),
                        _ => {}
                    }

                    meta.custom.insert(k.trim().to_string(), content_trimmed);
                }
            }
        }

        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rich_metadata() {
        let html = r#"
            <!doctype html>
            <html lang="en">
            <head>
                <meta charset="utf-8">
                <title>Test Title</title>
                <meta name="description" content="Test description for SEO.">
                <meta name="keywords" content="rust, selenium, automation, testing">
                <meta name="author" content="MustCodeAl">
                <link rel="canonical" href="https://example.com/canonical">
                <link rel="shortcut icon" href="/favicon.ico">
                <meta property="og:title" content="Social Title">
                <meta property="og:description" content="Social Description">
                <meta property="og:image" content="https://example.com/banner.jpg">
                <meta property="og:type" content="article">
                <meta name="twitter:card" content="summary">
                <meta name="custom-metric" content="xyz123">
            </head>
            <body></body>
            </html>
        "#;

        let meta = PageMetadata::from_html(html);
        assert_eq!(meta.title.as_deref(), Some("Test Title"));
        assert_eq!(meta.language.as_deref(), Some("en"));
        assert_eq!(meta.charset.as_deref(), Some("utf-8"));
        assert_eq!(
            meta.description.as_deref(),
            Some("Test description for SEO.")
        );
        assert_eq!(
            meta.keywords,
            vec!["rust", "selenium", "automation", "testing"]
        );
        assert_eq!(meta.author.as_deref(), Some("MustCodeAl"));
        assert_eq!(
            meta.canonical_url.as_deref(),
            Some("https://example.com/canonical")
        );
        assert_eq!(meta.favicon.as_deref(), Some("/favicon.ico"));
        assert_eq!(meta.og_title.as_deref(), Some("Social Title"));
        assert_eq!(meta.og_description.as_deref(), Some("Social Description"));
        assert_eq!(
            meta.og_image.as_deref(),
            Some("https://example.com/banner.jpg")
        );
        assert_eq!(meta.og_type.as_deref(), Some("article"));
        assert_eq!(meta.twitter_card.as_deref(), Some("summary"));
        assert_eq!(
            meta.custom.get("custom-metric").map(String::as_str),
            Some("xyz123")
        );
    }
}
