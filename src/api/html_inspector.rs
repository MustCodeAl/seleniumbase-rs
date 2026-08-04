//! Deterministic static checks for common HTML and accessibility defects.

use std::collections::{HashMap, HashSet};

use scraper::{ElementRef, Html, Selector};

use crate::SeleniumBaseError;

/// Importance of an inspection issue.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InspectionSeverity {
    /// A problem that should not reach production.
    Warning,
    /// A problem that blocks accessibility or validity.
    Error,
}

/// A single HTML inspection issue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionIssue {
    /// Stable rule identifier suitable for CI allowlists.
    pub rule: String,
    /// Whether the issue is a warning or an error.
    pub severity: InspectionSeverity,
    /// Human-readable explanation of the problem.
    pub message: String,
    /// Best-effort selector identifying the affected element.
    pub selector: Option<String>,
}

/// Result of inspecting one HTML document.
#[derive(Clone, Debug, Default)]
pub struct InspectionResult {
    /// All issues discovered during inspection.
    pub issues: Vec<InspectionIssue>,
}

/// Backwards-compatible name for an HTML inspection result.
pub type HtmlInspection = InspectionResult;

impl InspectionResult {
    /// Returns true when no issues were found.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Returns true when at least one error-level issue exists.
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == InspectionSeverity::Error)
    }
}

/// Static HTML inspector backed by an HTML5 parser and CSS selectors.
pub struct HtmlInspector;

impl HtmlInspector {
    /// Inspects HTML without executing JavaScript.
    pub fn inspect(html: &str) -> Result<InspectionResult, SeleniumBaseError> {
        let document = Html::parse_document(html);
        let mut issues = Vec::new();
        let ids = collect_ids(&document);

        inspect_document_metadata(&document, &mut issues);
        inspect_skip_link(&document, &mut issues);
        inspect_viewport(&document, &mut issues);
        inspect_images(&document, &mut issues);
        inspect_interactive_names(&document, &ids, &mut issues);
        inspect_nested_interactives(&document, &mut issues);
        inspect_form_labels(&document, &ids, &mut issues);
        inspect_radio_groups(&document, &mut issues);
        inspect_duplicate_ids(&ids, &mut issues);
        inspect_invalid_references(&document, &ids, &mut issues);
        inspect_headings(&document, &mut issues);
        inspect_tables(&document, &mut issues);
        inspect_landmarks(&document, &mut issues);
        inspect_link_text(&document, &mut issues);

        Ok(InspectionResult { issues })
    }
}

fn inspect_document_metadata(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let html_selector = selector("html");
    if document
        .select(&html_selector)
        .next()
        .and_then(|element| element.value().attr("lang"))
        .is_none_or(|lang| lang.trim().is_empty())
    {
        issue(
            issues,
            "document-lang",
            InspectionSeverity::Error,
            "The html element needs a non-empty lang attribute.",
            Some("html".to_owned()),
        );
    }

    let title_selector = selector("title");
    if document
        .select(&title_selector)
        .next()
        .is_none_or(|element| text(&element).is_empty())
    {
        issue(
            issues,
            "document-title",
            InspectionSeverity::Error,
            "The document needs a non-empty title.",
            Some("title".to_owned()),
        );
    }
}

fn inspect_images(document: &Html, issues: &mut Vec<InspectionIssue>) {
    for (index, image) in document.select(&selector("img")).enumerate() {
        if image.value().attr("alt").is_none() {
            issue(
                issues,
                "image-alt",
                InspectionSeverity::Error,
                "Image is missing an alt attribute. Use alt=\"\" for decorative images.",
                Some(element_selector(&image, index)),
            );
        }
    }
}

fn inspect_interactive_names(
    document: &Html,
    ids: &HashMap<String, Vec<IdElement>>,
    issues: &mut Vec<InspectionIssue>,
) {
    for (index, element) in document.select(&selector("a[href], button")).enumerate() {
        if !has_accessible_name(&element, ids) {
            issue(
                issues,
                "interactive-name",
                InspectionSeverity::Error,
                "Link or button has no static accessible name.",
                Some(element_selector(&element, index)),
            );
        }
    }
}

fn inspect_form_labels(
    document: &Html,
    ids: &HashMap<String, Vec<IdElement>>,
    issues: &mut Vec<InspectionIssue>,
) {
    let labels_for: HashSet<String> = document
        .select(&selector("label[for]"))
        .filter_map(|label| label.value().attr("for"))
        .map(ToOwned::to_owned)
        .collect();
    let controls = selector(
        "input:not([type='hidden']):not([type='button']):not([type='submit']):not([type='reset']), select, textarea",
    );

    for (index, control) in document.select(&controls).enumerate() {
        let labelled_by_id = control
            .value()
            .attr("id")
            .is_some_and(|id| labels_for.contains(id));
        if !labelled_by_id && !has_wrapping_label(&control) && !has_accessible_name(&control, ids) {
            issue(
                issues,
                "form-control-label",
                InspectionSeverity::Error,
                "Form control needs a label, aria-label, or valid aria-labelledby reference.",
                Some(element_selector(&control, index)),
            );
        }
    }
}

fn inspect_duplicate_ids(ids: &HashMap<String, Vec<IdElement>>, issues: &mut Vec<InspectionIssue>) {
    let mut duplicates = ids
        .iter()
        .filter(|(_, elements)| elements.len() > 1)
        .collect::<Vec<_>>();
    duplicates.sort_by_key(|(id, _)| id.as_str());
    for (id, elements) in duplicates {
        issue(
            issues,
            "duplicate-id",
            InspectionSeverity::Error,
            &format!("ID '{id}' is used {} times.", elements.len()),
            Some(format!(r#"[id="{id}"]"#)),
        );
    }
}

fn inspect_headings(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let heading_selector = selector("h1, h2, h3, h4, h5, h6");
    let headings = document.select(&heading_selector);
    let mut previous = None;
    let mut h1_count = 0;

    for (index, heading) in headings.enumerate() {
        let level = heading.value().name()[1..].parse::<u8>().unwrap_or(1);
        if level == 1 {
            h1_count += 1;
        }
        if text(&heading).is_empty() {
            issue(
                issues,
                "empty-heading",
                InspectionSeverity::Warning,
                &format!("h{level} element has no accessible text."),
                Some(element_selector(&heading, index)),
            );
        }
        if previous.is_some_and(|previous_level| level > previous_level + 1) {
            issue(
                issues,
                "heading-order",
                InspectionSeverity::Warning,
                &format!(
                    "Heading level jumps from h{} to h{level}.",
                    previous.unwrap_or(1)
                ),
                Some(element_selector(&heading, index)),
            );
        }
        previous = Some(level);
    }

    match h1_count {
        0 => issue(
            issues,
            "page-h1",
            InspectionSeverity::Warning,
            "Document has no h1 page heading.",
            None,
        ),
        1 => {}
        count => issue(
            issues,
            "page-h1",
            InspectionSeverity::Warning,
            &format!("Document has {count} h1 headings; verify there is one page topic."),
            Some("h1".to_owned()),
        ),
    }
}

fn inspect_landmarks(document: &Html, issues: &mut Vec<InspectionIssue>) {
    if document
        .select(&selector("main, [role='main']"))
        .next()
        .is_none()
    {
        issue(
            issues,
            "main-landmark",
            InspectionSeverity::Warning,
            "Document has no main landmark.",
            None,
        );
    }
}

fn inspect_skip_link(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let focusable = selector("a[href^='#']");
    let has_skip = document.select(&focusable).any(|link| {
        link.value()
            .attr("href")
            .is_some_and(|href| href == "#maincontent" || href == "#content")
    });
    if !has_skip {
        issue(
            issues,
            "skip-link",
            InspectionSeverity::Warning,
            "Document lacks a skip link to the main content.",
            None,
        );
    }
}

fn inspect_viewport(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let viewport = selector(r#"meta[name="viewport"]"#);
    let valid_viewport = document.select(&viewport).any(|meta| {
        meta.value()
            .attr("content")
            .is_some_and(|content| content.contains("width=device-width"))
    });
    if !valid_viewport {
        issue(
            issues,
            "meta-viewport",
            InspectionSeverity::Warning,
            "Document needs a viewport meta tag with width=device-width.",
            None,
        );
    }
}

fn inspect_nested_interactives(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let interactive = selector("a button");
    for (index, element) in document.select(&interactive).enumerate() {
        issue(
            issues,
            "nested-interactive",
            InspectionSeverity::Error,
            "Button is nested inside a link; nested interactive elements break accessibility and input handling.",
            Some(element_selector(&element, index)),
        );
    }
}

fn inspect_invalid_references(
    document: &Html,
    ids: &HashMap<String, Vec<IdElement>>,
    issues: &mut Vec<InspectionIssue>,
) {
    let referencing = selector("[aria-labelledby], [aria-describedby]");
    for (index, element) in document.select(&referencing).enumerate() {
        for attribute in ["aria-labelledby", "aria-describedby"] {
            let Some(references) = element.value().attr(attribute) else {
                continue;
            };
            for id in references.split_whitespace() {
                if !ids.contains_key(id) {
                    issue(
                        issues,
                        "invalid-aria-reference",
                        InspectionSeverity::Error,
                        &format!("{attribute} references missing ID '{id}'."),
                        Some(element_selector(&element, index)),
                    );
                }
            }
        }
    }
}

fn inspect_radio_groups(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let radios = selector("input[type='radio']");
    let mut groups: HashMap<String, Vec<ElementRef<'_>>> = HashMap::new();
    for radio in document.select(&radios) {
        let name = radio
            .value()
            .attr("name")
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        groups.entry(name).or_default().push(radio);
    }
    for (name, elements) in groups {
        if name.is_empty() || elements.len() < 2 {
            continue;
        }
        let in_fieldset = elements.iter().any(|radio| {
            let mut parent = radio.parent();
            while let Some(node) = parent {
                if ElementRef::wrap(node).is_some_and(|ancestor| {
                    ancestor.value().name() == "fieldset"
                        && ancestor.select(&selector("legend")).next().is_some()
                }) {
                    return true;
                }
                parent = node.parent();
            }
            false
        });
        if !in_fieldset {
            issue(
                issues,
                "radio-group-legend",
                InspectionSeverity::Warning,
                &format!("Radio group '{name}' should be inside a fieldset with a legend."),
                None,
            );
        }
    }
}

fn inspect_tables(document: &Html, issues: &mut Vec<InspectionIssue>) {
    for (index, table) in document.select(&selector("table")).enumerate() {
        if table.select(&selector("th")).next().is_none() {
            issue(
                issues,
                "table-headers",
                InspectionSeverity::Warning,
                "Table has no th elements; add headers so screen-reader users can navigate it.",
                Some(element_selector(&table, index)),
            );
        }
    }
}

fn inspect_link_text(document: &Html, issues: &mut Vec<InspectionIssue>) {
    let generic = ["click here", "here", "read more", "more", "link", "details"];
    for (index, link) in document.select(&selector("a[href]")).enumerate() {
        let link_text = text(&link).to_lowercase();
        if link_text.is_empty() {
            issue(
                issues,
                "link-text",
                InspectionSeverity::Warning,
                "Link has no visible text; add descriptive text or an aria-label.",
                Some(element_selector(&link, index)),
            );
        } else if generic.iter().any(|phrase| link_text == *phrase) {
            issue(
                issues,
                "link-text",
                InspectionSeverity::Warning,
                &format!("Link text '{link_text}' is not descriptive out of context."),
                Some(element_selector(&link, index)),
            );
        }
    }
}

#[derive(Clone, Debug)]
struct IdElement {
    text: String,
}

fn collect_ids(document: &Html) -> HashMap<String, Vec<IdElement>> {
    let mut ids: HashMap<String, Vec<IdElement>> = HashMap::new();
    for element in document.select(&selector("[id]")) {
        let Some(id) = element.value().attr("id") else {
            continue;
        };
        ids.entry(id.to_owned()).or_default().push(IdElement {
            text: text(&element),
        });
    }
    ids
}

fn has_accessible_name(element: &ElementRef<'_>, ids: &HashMap<String, Vec<IdElement>>) -> bool {
    if !text(element).is_empty()
        || attribute_has_text(element, "aria-label")
        || attribute_has_text(element, "title")
        || element
            .value()
            .attr("alt")
            .is_some_and(|value| !value.trim().is_empty())
    {
        return true;
    }

    element
        .value()
        .attr("aria-labelledby")
        .is_some_and(|references| {
            references.split_whitespace().any(|id| {
                ids.get(id)
                    .is_some_and(|elements| elements.iter().any(|element| !element.text.is_empty()))
            })
        })
}

fn attribute_has_text(element: &ElementRef<'_>, attribute: &str) -> bool {
    element
        .value()
        .attr(attribute)
        .is_some_and(|value| !value.trim().is_empty())
}

fn has_wrapping_label(element: &ElementRef<'_>) -> bool {
    let mut parent = element.parent();
    while let Some(node) = parent {
        if ElementRef::wrap(node).is_some_and(|ancestor| ancestor.value().name() == "label") {
            return true;
        }
        parent = node.parent();
    }
    false
}

fn text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_selector(element: &ElementRef<'_>, index: usize) -> String {
    element.value().attr("id").map_or_else(
        || format!("{}:nth-of-type({})", element.value().name(), index + 1),
        |id| format!(r#"{}[id="{id}"]"#, element.value().name()),
    )
}

fn issue(
    issues: &mut Vec<InspectionIssue>,
    rule: &str,
    severity: InspectionSeverity,
    message: &str,
    selector: Option<String>,
) {
    issues.push(InspectionIssue {
        rule: rule.to_owned(),
        severity,
        message: message.to_owned(),
        selector,
    });
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("built-in inspector selector must compile")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_semantic_document() {
        let html = r##"
            <html lang="en"><head>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>Example</title></head><body>
            <a href="#maincontent">Skip to main content</a>
            <main id="maincontent"><h1>Example</h1>
            <label>Name <input type="text"></label>
            <button aria-label="Save changes"></button>
            <img src="decorative.png" alt="">
            </main></body></html>
        "##;
        assert!(HtmlInspector::inspect(html).unwrap().is_clean());
    }

    #[test]
    fn reports_missing_labels_and_duplicate_ids() {
        let html = r#"
            <html lang="en"><head><title>Example</title></head><body><main>
            <h1>Example</h1><input id="name" placeholder="Name">
            <div id="same"></div><span id="same"></span>
            </main></body></html>
        "#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.rule == "form-control-label"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.rule == "duplicate-id"));
    }

    #[test]
    fn validates_aria_labelledby_target_text() {
        let html = r##"
            <html lang="en"><head>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>Example</title></head><body>
            <a href="#maincontent">Skip to main content</a>
            <main id="maincontent"><h1>Example</h1><span id="name-label">Name</span>
            <input aria-labelledby="name-label">
            </main></body></html>
        "##;
        assert!(HtmlInspector::inspect(html).unwrap().is_clean());
    }

    #[test]
    fn does_not_report_first_h2_as_heading_jump() {
        let html = r#"
            <html lang="en"><head><title>Example</title></head><body><main>
            <h2>Section</h2>
            </main></body></html>
        "#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(!result
            .issues
            .iter()
            .any(|issue| issue.rule == "heading-order"));
        assert!(result.issues.iter().any(|issue| issue.rule == "page-h1"));
    }

    #[test]
    fn reports_invalid_aria_reference_and_nested_button() {
        let html = r#"
            <html lang="en"><head><title>Example</title></head><body><main>
            <h1>Example</h1>
            <input aria-labelledby="missing-label">
            <a href="/"><button>Bad</button></a>
            </main></body></html>
        "#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.rule == "invalid-aria-reference"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.rule == "nested-interactive"));
    }

    #[test]
    fn reports_generic_link_text_and_missing_table_headers() {
        let html = r#"
            <html lang="en"><head><title>Example</title></head><body><main>
            <h1>Example</h1>
            <a href="/">click here</a>
            <table><tr><td>No headers</td></tr></table>
            </main></body></html>
        "#;
        let result = HtmlInspector::inspect(html).unwrap();
        assert!(result.issues.iter().any(|issue| issue.rule == "link-text"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.rule == "table-headers"));
    }

    #[test]
    fn accepts_document_with_enhanced_checks() {
        let html = r##"
            <html lang="en"><head>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>Example</title></head><body>
            <a href="#maincontent">Skip to main content</a>
            <main id="maincontent"><h1>Example</h1>
            <a href="/about">About us</a>
            <table><tr><th>Name</th><td>Ada</td></tr></table>
            </main></body></html>
        "##;
        assert!(HtmlInspector::inspect(html).unwrap().is_clean());
    }
}
