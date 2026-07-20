//! HTML Inspector for detecting common accessibility and markup issues.

use crate::api::html::BeautifulSoup;
use crate::error::SeleniumBaseError;
use std::collections::HashSet;

/// A single issue found by the inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlIssue {
    pub rule: String,
    pub message: String,
    pub selector: Option<String>,
}

/// Collection of inspection results.
#[derive(Clone, Debug, Default)]
pub struct HtmlInspection {
    pub issues: Vec<HtmlIssue>,
}

impl HtmlInspection {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn errors(&self) -> Vec<&HtmlIssue> {
        self.issues.iter().collect()
    }
}

/// Inspects HTML markup for common problems.
pub struct HtmlInspector;

impl HtmlInspector {
    pub fn inspect(html: &str) -> Result<HtmlInspection, SeleniumBaseError> {
        let soup = BeautifulSoup::parse(html);
        let mut issues = Vec::new();

        // Missing page title
        if soup
            .get_text("title")?
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            issues.push(HtmlIssue {
                rule: "missing-title".to_owned(),
                message: "Page is missing a non-empty <title> element.".to_owned(),
                selector: Some("title".to_owned()),
            });
        }

        // Missing html lang attribute
        if let Some(html_node) = soup.find("html")? {
            if html_node.attr("lang").unwrap_or_default().trim().is_empty() {
                issues.push(HtmlIssue {
                    rule: "missing-lang".to_owned(),
                    message: "<html> element is missing a lang attribute.".to_owned(),
                    selector: Some("html".to_owned()),
                });
            }
        }

        // Missing alt on images
        for (idx, img) in soup.find_all("img")?.iter().enumerate() {
            let src = img.attr("src").unwrap_or_default();
            let alt = img.attr("alt");
            let is_decorative = img.attr("role").as_deref() == Some("presentation")
                || img.attr("aria-hidden").as_deref() == Some("true");
            if alt.is_none() && !is_decorative {
                issues.push(HtmlIssue {
                    rule: "missing-alt".to_owned(),
                    message: format!("Image {} is missing an alt attribute.", src),
                    selector: Some(format!("img:nth-of-type({})", idx + 1)),
                });
            }
        }

        // Empty links
        for (idx, a) in soup.find_all("a")?.iter().enumerate() {
            let text = a.text.trim();
            let aria_label = a.attr("aria-label").unwrap_or_default();
            let aria_labelled_by = a.attr("aria-labelledby").unwrap_or_default();
            if text.is_empty() && aria_label.is_empty() && aria_labelled_by.is_empty() {
                issues.push(HtmlIssue {
                    rule: "empty-link".to_owned(),
                    message: "Link has no visible text or accessible name.".to_owned(),
                    selector: Some(format!("a:nth-of-type({})", idx + 1)),
                });
            }
        }

        // Missing labels on form inputs
        for (idx, input) in soup.find_all("input")?.iter().enumerate() {
            let typ = input.attr("type").unwrap_or_default();
            if matches!(
                typ.as_str(),
                "hidden" | "submit" | "button" | "image" | "reset"
            ) {
                continue;
            }
            let has_label = input.attr("id").is_some()
                || input.attr("aria-label").is_some()
                || input.attr("aria-labelledby").is_some()
                || input.attr("placeholder").is_some();
            if !has_label {
                issues.push(HtmlIssue {
                    rule: "missing-label".to_owned(),
                    message: format!("Input {} is missing an associated label.", typ),
                    selector: Some(format!("input:nth-of-type({})", idx + 1)),
                });
            }
        }

        // Duplicate IDs
        let mut seen = HashSet::new();
        let mut dupes = HashSet::new();
        // Query all elements with an id attribute via a broad selector.
        for node in soup.find_all("[id]")? {
            if let Some(id) = node.attr("id") {
                if !id.trim().is_empty() && !seen.insert(id.clone()) {
                    dupes.insert(id);
                }
            }
        }
        for id in dupes {
            issues.push(HtmlIssue {
                rule: "duplicate-id".to_owned(),
                message: format!("Duplicate id attribute: '{}'.", id),
                selector: Some(format!("#[id=\"{}\"]", id)),
            });
        }

        // Skipped heading levels
        let headings = soup.find_all("h1, h2, h3, h4, h5, h6")?;
        let mut last_level = 0usize;
        for (idx, h) in headings.iter().enumerate() {
            let level = h
                .tag_name
                .strip_prefix('h')
                .unwrap_or("0")
                .parse::<usize>()
                .unwrap_or(0);
            if level > 0 && level > last_level + 1 {
                issues.push(HtmlIssue {
                    rule: "skipped-heading".to_owned(),
                    message: format!("Heading level jumps from h{} to h{}.", last_level, level),
                    selector: Some(format!("{}:nth-of-type({})", h.tag_name, idx + 1)),
                });
            }
            last_level = level;
        }

        // Main landmark check
        let has_main = soup.find("main")?.is_some()
            || soup.find("[role='main']")?.is_some()
            || soup.find("[role=\"main\"]")?.is_some();
        if !has_main {
            issues.push(HtmlIssue {
                rule: "missing-main-landmark".to_owned(),
                message: "Page is missing a <main> element or role='main' landmark.".to_owned(),
                selector: None,
            });
        }

        Ok(HtmlInspection { issues })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_title_and_lang() {
        let html = r#"<html><body><h1>Hi</h1></body></html>"#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result.issues.iter().any(|i| i.rule == "missing-title"));
        assert!(result.issues.iter().any(|i| i.rule == "missing-lang"));
    }

    #[test]
    fn detects_missing_alt() {
        let html =
            r#"<html lang="en"><head><title>T</title></head><body><img src="a.png"></body></html>"#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result.issues.iter().any(|i| i.rule == "missing-alt"));
    }

    #[test]
    fn detects_empty_link_and_duplicate_id() {
        let html = r##"<html lang="en"><head><title>T</title></head><body>
            <a href="#" id="x"></a>
            <span id="x">dup</span>
        </body></html>"##;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result.issues.iter().any(|i| i.rule == "empty-link"));
        assert!(result.issues.iter().any(|i| i.rule == "duplicate-id"));
    }

    #[test]
    fn clean_html_passes() {
        let html = r##"<html lang="en"><head><title>Good</title></head><body>
            <main>
                <h1>Title</h1>
                <img src="a.png" alt="desc">
                <a href="#">Click me</a>
                <label for="name">Name</label><input id="name" type="text">
            </main>
        </body></html>"##;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result.is_clean(), "{:#?}", result.issues);
    }
}
