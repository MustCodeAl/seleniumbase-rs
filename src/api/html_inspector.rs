//! Deterministic static checks for common HTML and accessibility defects.
//!
//! [`HtmlInspector::inspect`] parses a document and reports an
//! [`InspectionIssue`] for each defect it finds. It does not run JavaScript,
//! so it sees the markup as served, not as a script later changes it. The same
//! input always gives the same issues in the same order, which makes the
//! output safe to diff in CI.
//!
//! Each issue carries a stable `rule` identifier:
//!
//! | Rule | Severity | Reports |
//! |------|----------|---------|
//! | `document-lang` | error | `<html>` has no `lang` |
//! | `document-title` | error | the page has no `<title>`, or an empty one (an SVG `<title>` does not count) |
//! | `skip-link` | warning | no working skip link to the main content |
//! | `meta-viewport` | warning | no `width=device-width` viewport |
//! | `image-alt` | error | an `<img>` has no `alt` attribute |
//! | `interactive-name` | error | a link or button has no accessible name |
//! | `nested-interactive` | error | a `<button>` is inside a link |
//! | `form-control-label` | error | an input, select or textarea has no label |
//! | `radio-group-legend` | warning | a radio group is not in a fieldset with a legend |
//! | `duplicate-id` | error | an `id` is used more than once |
//! | `invalid-aria-reference` | error | `aria-labelledby`/`-describedby` names a missing id |
//! | `empty-heading` | warning | a heading has no text |
//! | `heading-order` | warning | a heading level is skipped |
//! | `page-h1` | warning | the page has no `<h1>`, or more than one |
//! | `table-headers` | warning | a table has no `<th>` (`role="presentation"` layout tables are skipped) |
//! | `main-landmark` | warning | no `<main>` landmark |
//! | `link-text` | warning | a link has no name, or one that says nothing out of context |
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::html_inspector::HtmlInspector;
//!
//! let html = r#"<html><head><title>Shop</title></head>
//!     <body><main><h1>Shop</h1><img src="logo.png"></main></body></html>"#;
//! let result = HtmlInspector::inspect(html).unwrap();
//!
//! assert!(result.has_errors());
//! let issue = result.issues.iter().find(|i| i.rule == "image-alt").unwrap();
//! assert_eq!(
//!     issue.selector.as_deref(),
//!     Some("html:nth-of-type(1) > body:nth-of-type(1) > main:nth-of-type(1) > img:nth-of-type(1)")
//! );
//! ```

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
    /// A CSS selector that matches the affected element in the inspected
    /// document, when the issue is about one element.
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
    ///
    /// The issues come in a fixed order: the rules in the order the module
    /// documentation lists them, each in document order.
    pub fn inspect(html: &str) -> Result<InspectionResult, SeleniumBaseError> {
        let document = Html::parse_document(html);
        let ids = collect_ids(&document);
        let mut inspection = Inspection {
            document: &document,
            sel: Selectors::new(),
            ids,
            issues: Vec::new(),
        };

        inspection.document_metadata();
        inspection.skip_link();
        inspection.viewport();
        inspection.images();
        inspection.interactive_names();
        inspection.nested_interactives();
        inspection.form_labels();
        inspection.radio_groups();
        inspection.duplicate_ids();
        inspection.invalid_references();
        inspection.headings();
        inspection.tables();
        inspection.landmarks();
        inspection.link_text();

        Ok(InspectionResult {
            issues: inspection.issues,
        })
    }
}

/// Every selector the checks use, parsed once per inspection.
struct Selectors {
    html: Selector,
    title: Selector,
    img: Selector,
    interactive: Selector,
    nested_interactive: Selector,
    label_for: Selector,
    form_controls: Selector,
    aria_references: Selector,
    radios: Selector,
    legend: Selector,
    headings: Selector,
    tables: Selector,
    table_header: Selector,
    main: Selector,
    in_page_links: Selector,
    viewport: Selector,
    links: Selector,
    labelled_descendants: Selector,
    images_with_alt: Selector,
}

impl Selectors {
    fn new() -> Self {
        Self {
            html: selector("html"),
            title: selector("title"),
            img: selector("img"),
            interactive: selector("a[href], button"),
            nested_interactive: selector("a button"),
            label_for: selector("label[for]"),
            form_controls: selector(
                "input:not([type='hidden']):not([type='button']):not([type='submit']):not([type='reset']), select, textarea",
            ),
            aria_references: selector("[aria-labelledby], [aria-describedby]"),
            radios: selector("input[type='radio']"),
            legend: selector("legend"),
            headings: selector("h1, h2, h3, h4, h5, h6"),
            tables: selector("table"),
            table_header: selector("th"),
            main: selector("main, [role='main']"),
            in_page_links: selector("a[href^='#']"),
            viewport: selector(r#"meta[name="viewport"]"#),
            links: selector("a[href]"),
            labelled_descendants: selector("[aria-label]"),
            images_with_alt: selector("img[alt]"),
        }
    }
}

/// The state shared by the checks of one inspection.
struct Inspection<'a> {
    document: &'a Html,
    sel: Selectors,
    ids: HashMap<String, Vec<IdElement>>,
    issues: Vec<InspectionIssue>,
}

/// Link texts that say nothing out of context.
const GENERIC_LINK_TEXTS: [&str; 6] =
    ["click here", "here", "read more", "more", "link", "details"];

/// Targets that name the main content, for skip links that do not say "skip".
const MAIN_CONTENT_IDS: [&str; 4] = ["maincontent", "main-content", "main", "content"];

/// The namespace of HTML elements, as the parser records it.
const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

impl Inspection<'_> {
    fn issue(
        &mut self,
        rule: &str,
        severity: InspectionSeverity,
        message: &str,
        selector: Option<String>,
    ) {
        self.issues.push(InspectionIssue {
            rule: rule.to_owned(),
            severity,
            message: message.to_owned(),
            selector,
        });
    }

    fn document_metadata(&mut self) {
        if self
            .document
            .select(&self.sel.html)
            .next()
            .and_then(|element| element.value().attr("lang"))
            .is_none_or(|lang| lang.trim().is_empty())
        {
            self.issue(
                "document-lang",
                InspectionSeverity::Error,
                "The html element needs a non-empty lang attribute.",
                Some("html".to_owned()),
            );
        }

        // The document's title is its first HTML `title` element. An SVG
        // `<title>` (the tooltip of a drawing) matches the same selector but
        // is not the page title.
        if self
            .document
            .select(&self.sel.title)
            .find(|element| &*element.value().name.ns == HTML_NAMESPACE)
            .is_none_or(|element| text(&element).is_empty())
        {
            self.issue(
                "document-title",
                InspectionSeverity::Error,
                "The document needs a non-empty title.",
                Some("title".to_owned()),
            );
        }
    }

    fn images(&mut self) {
        let missing: Vec<String> = self
            .document
            .select(&self.sel.img)
            .filter(|image| image.value().attr("alt").is_none())
            .map(|image| element_selector(&image))
            .collect();
        for selector in missing {
            self.issue(
                "image-alt",
                InspectionSeverity::Error,
                "Image is missing an alt attribute. Use alt=\"\" for decorative images.",
                Some(selector),
            );
        }
    }

    fn interactive_names(&mut self) {
        let unnamed: Vec<String> = self
            .document
            .select(&self.sel.interactive)
            .filter(|element| self.accessible_name(element).is_empty())
            .map(|element| element_selector(&element))
            .collect();
        for selector in unnamed {
            self.issue(
                "interactive-name",
                InspectionSeverity::Error,
                "Link or button has no static accessible name.",
                Some(selector),
            );
        }
    }

    fn form_labels(&mut self) {
        let labels_for: HashSet<&str> = self
            .document
            .select(&self.sel.label_for)
            .filter_map(|label| label.value().attr("for"))
            .collect();
        let unlabelled: Vec<String> = self
            .document
            .select(&self.sel.form_controls)
            .filter(|control| {
                let labelled_by_id = control
                    .value()
                    .attr("id")
                    .is_some_and(|id| labels_for.contains(id));
                !labelled_by_id
                    && !has_wrapping_label(control)
                    && self.accessible_name(control).is_empty()
            })
            .map(|control| element_selector(&control))
            .collect();
        for selector in unlabelled {
            self.issue(
                "form-control-label",
                InspectionSeverity::Error,
                "Form control needs a label, aria-label, or valid aria-labelledby reference.",
                Some(selector),
            );
        }
    }

    fn duplicate_ids(&mut self) {
        let mut duplicates: Vec<(String, usize)> = self
            .ids
            .iter()
            .filter(|(_, elements)| elements.len() > 1)
            .map(|(id, elements)| (id.clone(), elements.len()))
            .collect();
        duplicates.sort();
        for (id, count) in duplicates {
            self.issue(
                "duplicate-id",
                InspectionSeverity::Error,
                &format!("ID '{id}' is used {count} times."),
                Some(format!(r#"[id="{}"]"#, escape_attribute(&id))),
            );
        }
    }

    fn headings(&mut self) {
        let mut previous: Option<u8> = None;
        let mut h1_count = 0;
        let mut found = Vec::new();

        for heading in self.document.select(&self.sel.headings) {
            let level = heading.value().name()[1..].parse::<u8>().unwrap_or(1);
            if level == 1 {
                h1_count += 1;
            }
            if text(&heading).is_empty() {
                found.push((
                    "empty-heading",
                    format!("h{level} element has no accessible text."),
                    Some(element_selector(&heading)),
                ));
            }
            if let Some(previous_level) =
                previous.filter(|previous_level| level > previous_level + 1)
            {
                found.push((
                    "heading-order",
                    format!("Heading level jumps from h{previous_level} to h{level}."),
                    Some(element_selector(&heading)),
                ));
            }
            previous = Some(level);
        }

        for (rule, message, selector) in found {
            self.issue(rule, InspectionSeverity::Warning, &message, selector);
        }
        match h1_count {
            0 => self.issue(
                "page-h1",
                InspectionSeverity::Warning,
                "Document has no h1 page heading.",
                None,
            ),
            1 => {}
            count => self.issue(
                "page-h1",
                InspectionSeverity::Warning,
                &format!("Document has {count} h1 headings; verify there is one page topic."),
                Some("h1".to_owned()),
            ),
        }
    }

    fn landmarks(&mut self) {
        if self.document.select(&self.sel.main).next().is_none() {
            self.issue(
                "main-landmark",
                InspectionSeverity::Warning,
                "Document has no main landmark.",
                None,
            );
        }
    }

    /// A skip link is an in-page link to an element that exists, which either
    /// says it skips ("Skip to content", "Jump to main") or points at the
    /// conventional id of the main content.
    fn skip_link(&mut self) {
        let has_skip = self.document.select(&self.sel.in_page_links).any(|link| {
            let Some(target) = link
                .value()
                .attr("href")
                .and_then(|href| href.strip_prefix('#'))
                .filter(|target| !target.is_empty())
            else {
                return false;
            };
            let says_skip = {
                let name = self.accessible_name(&link).to_lowercase();
                name.contains("skip") || name.contains("jump to")
            };
            self.ids.contains_key(target) && (says_skip || MAIN_CONTENT_IDS.contains(&target))
        });
        if !has_skip {
            self.issue(
                "skip-link",
                InspectionSeverity::Warning,
                "Document lacks a skip link to the main content.",
                None,
            );
        }
    }

    fn viewport(&mut self) {
        let valid_viewport = self.document.select(&self.sel.viewport).any(|meta| {
            meta.value()
                .attr("content")
                .is_some_and(|content| content.contains("width=device-width"))
        });
        if !valid_viewport {
            self.issue(
                "meta-viewport",
                InspectionSeverity::Warning,
                "Document needs a viewport meta tag with width=device-width.",
                None,
            );
        }
    }

    fn nested_interactives(&mut self) {
        let nested: Vec<String> = self
            .document
            .select(&self.sel.nested_interactive)
            .map(|element| element_selector(&element))
            .collect();
        for selector in nested {
            self.issue(
                "nested-interactive",
                InspectionSeverity::Error,
                "Button is nested inside a link; nested interactive elements break accessibility and input handling.",
                Some(selector),
            );
        }
    }

    fn invalid_references(&mut self) {
        let mut missing = Vec::new();
        for element in self.document.select(&self.sel.aria_references) {
            for attribute in ["aria-labelledby", "aria-describedby"] {
                let Some(references) = element.value().attr(attribute) else {
                    continue;
                };
                for id in references.split_whitespace() {
                    if !self.ids.contains_key(id) {
                        missing.push((
                            format!("{attribute} references missing ID '{id}'."),
                            element_selector(&element),
                        ));
                    }
                }
            }
        }
        for (message, selector) in missing {
            self.issue(
                "invalid-aria-reference",
                InspectionSeverity::Error,
                &message,
                Some(selector),
            );
        }
    }

    /// Radio buttons that share a `name` form a group, which should sit in a
    /// fieldset with a legend. Groups are reported in the order their first
    /// radio appears in the document.
    fn radio_groups(&mut self) {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<ElementRef<'_>>> = HashMap::new();
        for radio in self.document.select(&self.sel.radios) {
            let name = radio.value().attr("name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            groups
                .entry(name.to_owned())
                .or_insert_with(|| {
                    order.push(name.to_owned());
                    Vec::new()
                })
                .push(radio);
        }

        let mut found = Vec::new();
        for name in order {
            let radios = &groups[&name];
            if radios.len() < 2 {
                continue;
            }
            let in_fieldset = radios
                .iter()
                .any(|radio| self.in_fieldset_with_legend(radio));
            if !in_fieldset {
                found.push((name, element_selector(&radios[0])));
            }
        }
        for (name, selector) in found {
            self.issue(
                "radio-group-legend",
                InspectionSeverity::Warning,
                &format!("Radio group '{name}' should be inside a fieldset with a legend."),
                Some(selector),
            );
        }
    }

    fn in_fieldset_with_legend(&self, element: &ElementRef<'_>) -> bool {
        let mut parent = element.parent();
        while let Some(node) = parent {
            if ElementRef::wrap(node).is_some_and(|ancestor| {
                ancestor.value().name() == "fieldset"
                    && ancestor.select(&self.sel.legend).next().is_some()
            }) {
                return true;
            }
            parent = node.parent();
        }
        false
    }

    fn tables(&mut self) {
        let headerless: Vec<String> = self
            .document
            .select(&self.sel.tables)
            .filter(|table| !is_layout_table(table))
            .filter(|table| table.select(&self.sel.table_header).next().is_none())
            .map(|table| element_selector(&table))
            .collect();
        for selector in headerless {
            self.issue(
                "table-headers",
                InspectionSeverity::Warning,
                "Table has no th elements; add headers so screen-reader users can navigate it.",
                Some(selector),
            );
        }
    }

    fn link_text(&mut self) {
        let mut found = Vec::new();
        for link in self.document.select(&self.sel.links) {
            let name = self.accessible_name(&link).to_lowercase();
            if name.is_empty() {
                found.push((
                    "Link has no visible text; add descriptive text or an aria-label.".to_owned(),
                    element_selector(&link),
                ));
            } else if GENERIC_LINK_TEXTS.contains(&name.as_str()) {
                found.push((
                    format!("Link text '{name}' is not descriptive out of context."),
                    element_selector(&link),
                ));
            }
        }
        for (message, selector) in found {
            self.issue(
                "link-text",
                InspectionSeverity::Warning,
                &message,
                Some(selector),
            );
        }
    }

    /// The name a screen reader would announce for `element`, or an empty
    /// string if it has none.
    ///
    /// In order: the text of the elements `aria-labelledby` points at, the
    /// `aria-label`, the element's own text (not for form controls, whose
    /// inner text is their options or default value, not a label), the `alt`
    /// of images and the `aria-label` of elements inside it, and the `alt`
    /// and `title` attributes.
    fn accessible_name(&self, element: &ElementRef<'_>) -> String {
        let referenced: Vec<&str> = element
            .value()
            .attr("aria-labelledby")
            .into_iter()
            .flat_map(str::split_whitespace)
            .filter_map(|id| self.ids.get(id))
            .flat_map(|elements| elements.iter().map(|e| e.text.as_str()))
            .filter(|text| !text.is_empty())
            .collect();
        if !referenced.is_empty() {
            return referenced.join(" ");
        }
        if let Some(label) = non_blank_attribute(element, "aria-label") {
            return label;
        }
        if !is_form_control(element) {
            let own = text(element);
            if !own.is_empty() {
                return own;
            }
            let from_inside: Vec<String> = element
                .select(&self.sel.images_with_alt)
                .filter_map(|image| non_blank_attribute(&image, "alt"))
                .chain(
                    element
                        .select(&self.sel.labelled_descendants)
                        .filter_map(|inner| non_blank_attribute(&inner, "aria-label")),
                )
                .collect();
            if !from_inside.is_empty() {
                return from_inside.join(" ");
            }
        }
        non_blank_attribute(element, "alt")
            .or_else(|| non_blank_attribute(element, "title"))
            .unwrap_or_default()
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

/// Whether the author marked a table as layout only (`role="presentation"`
/// or `role="none"`), which has no headers to announce.
fn is_layout_table(table: &ElementRef<'_>) -> bool {
    table.value().attr("role").is_some_and(|role| {
        matches!(
            role.trim().to_ascii_lowercase().as_str(),
            "presentation" | "none"
        )
    })
}

fn is_form_control(element: &ElementRef<'_>) -> bool {
    matches!(element.value().name(), "input" | "select" | "textarea")
}

fn non_blank_attribute(element: &ElementRef<'_>, attribute: &str) -> Option<String> {
    element
        .value()
        .attr(attribute)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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

/// `value` escaped for use inside a double-quoted CSS attribute value.
fn escape_attribute(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A CSS selector that matches `element` and no other element of its document.
///
/// An element with an `id` is named by it. Any other element is named by its
/// path from the nearest ancestor that has one, or from the root: each step is
/// the tag and its position among the siblings of the same tag, which is how
/// `:nth-of-type` counts. (The id is only unique if the document has no
/// duplicate ids, which the `duplicate-id` rule reports.)
fn element_selector(element: &ElementRef<'_>) -> String {
    let mut steps = Vec::new();
    let mut current = Some(*element);
    while let Some(element) = current {
        let name = element.value().name();
        if let Some(id) = element.value().attr("id") {
            steps.push(format!(r#"{name}[id="{}"]"#, escape_attribute(id)));
            break;
        }
        let position = 1 + element
            .prev_siblings()
            .filter_map(ElementRef::wrap)
            .filter(|sibling| sibling.value().name() == name)
            .count();
        steps.push(format!("{name}:nth-of-type({position})"));
        current = element.parent().and_then(ElementRef::wrap);
    }
    steps.reverse();
    steps.join(" > ")
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("built-in inspector selector must compile")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> String {
        format!(
            r##"<html lang="en"><head><title>Example</title>
            <meta name="viewport" content="width=device-width, initial-scale=1"></head><body>
            <a href="#maincontent">Skip to main content</a>
            <main id="maincontent"><h1>Example</h1>{body}</main></body></html>"##
        )
    }

    fn rules(html: &str) -> Vec<String> {
        HtmlInspector::inspect(html)
            .unwrap()
            .issues
            .into_iter()
            .map(|issue| issue.rule)
            .collect()
    }

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

    #[test]
    fn every_reported_selector_matches_exactly_the_element_it_is_about() {
        // Many same-tag elements in different parents: the old `img:nth-of-type(index)`
        // matched all of them, or the wrong one.
        let html = page(
            r#"<div><img src="a"></div><div><img src="b"><p>x</p><img src="c"></div>
               <section><div><img src="d"></div></section>
               <ul><li><a href="/">here</a></li><li><a href="/two">here</a></li></ul>
               <table><tr><td>1</td></tr></table><table><tr><td>2</td></tr></table>"#,
        );
        let result = HtmlInspector::inspect(&html).unwrap();
        let document = Html::parse_document(&html);
        let mut checked = 0;
        for issue in &result.issues {
            let Some(selector) = &issue.selector else {
                continue;
            };
            if selector == "html" || selector == "title" || selector == "h1" {
                continue;
            }
            let parsed = Selector::parse(selector).unwrap_or_else(|e| panic!("{selector}: {e:?}"));
            let matches: Vec<_> = document.select(&parsed).collect();
            assert_eq!(
                matches.len(),
                1,
                "{selector} ({}) must name one element",
                issue.rule
            );
            checked += 1;
        }
        assert!(
            checked >= 8,
            "only {checked} selectors were checked: {:?}",
            result.issues
        );

        // And they name the right ones: the image in the second div is the third image in the document.
        let image_selectors: Vec<&str> = result
            .issues
            .iter()
            .filter(|i| i.rule == "image-alt")
            .filter_map(|i| i.selector.as_deref())
            .collect();
        let sources: Vec<String> = image_selectors
            .iter()
            .map(|s| {
                let parsed = Selector::parse(s).unwrap();
                document
                    .select(&parsed)
                    .next()
                    .unwrap()
                    .value()
                    .attr("src")
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(sources, ["a", "b", "c", "d"]);
    }

    #[test]
    fn an_element_with_an_id_is_named_by_it_and_a_path_stops_at_the_nearest_id() {
        let html = r#"<html lang="en"><head><title>t</title></head><body>
            <div id="box"><span><img src="x"></span></div><img id="logo" src="y"></body></html>"#;
        let result = HtmlInspector::inspect(html).unwrap();
        let selectors: Vec<&str> = result
            .issues
            .iter()
            .filter(|i| i.rule == "image-alt")
            .filter_map(|i| i.selector.as_deref())
            .collect();
        assert_eq!(
            selectors,
            [
                r#"div[id="box"] > span:nth-of-type(1) > img:nth-of-type(1)"#,
                r#"img[id="logo"]"#
            ]
        );
    }

    #[test]
    fn a_selector_for_an_id_with_quotes_stays_valid_css() {
        let html = page(r#"<div id='say "hi"'></div><div id='say "hi"'></div>"#);
        let issue = HtmlInspector::inspect(&html)
            .unwrap()
            .issues
            .into_iter()
            .find(|i| i.rule == "duplicate-id")
            .unwrap();
        let selector = issue.selector.unwrap();
        assert_eq!(selector, r#"[id="say \"hi\""]"#);
        assert_eq!(
            Html::parse_document(&html)
                .select(&Selector::parse(&selector).unwrap())
                .count(),
            2
        );
    }

    #[test]
    fn radio_groups_are_reported_in_document_order_every_time() {
        let radios: String = [5, 2, 9, 0, 7, 1, 8, 3, 6, 4]
            .iter()
            .map(|g| format!(r#"<input type="radio" name="g{g}" id="a{g}"><input type="radio" name="g{g}" id="b{g}">"#))
            .collect();
        let html = page(&radios);
        let first = HtmlInspector::inspect(&html).unwrap().issues;
        let groups: Vec<&str> = first
            .iter()
            .filter(|i| i.rule == "radio-group-legend")
            .map(|i| i.message.as_str())
            .collect();
        let expected: Vec<String> = [5, 2, 9, 0, 7, 1, 8, 3, 6, 4]
            .iter()
            .map(|g| format!("Radio group 'g{g}' should be inside a fieldset with a legend."))
            .collect();
        assert_eq!(groups, expected);
        for _ in 0..20 {
            assert_eq!(HtmlInspector::inspect(&html).unwrap().issues, first);
        }
    }

    #[test]
    fn a_radio_group_in_a_fieldset_with_a_legend_is_fine_and_a_lone_radio_is_ignored() {
        let grouped = page(
            r#"<fieldset><legend>Size</legend>
               <label><input type="radio" name="size"> S</label><label><input type="radio" name="size"> M</label>
               </fieldset><label><input type="radio" name="solo"> only</label>"#,
        );
        assert!(!rules(&grouped).contains(&"radio-group-legend".to_owned()));
        let bare = page(
            r#"<label><input type="radio" name="size"> S</label><label><input type="radio" name="size"> M</label>"#,
        );
        let issue = HtmlInspector::inspect(&bare)
            .unwrap()
            .issues
            .into_iter()
            .find(|i| i.rule == "radio-group-legend")
            .unwrap();
        assert!(
            issue.selector.is_some(),
            "the group points at its first radio"
        );
    }

    #[test]
    fn a_select_or_textarea_is_not_labelled_by_its_own_content() {
        for control in [
            "<select><option>Red</option><option>Green</option></select>",
            "<textarea>default text</textarea>",
        ] {
            assert!(
                rules(&page(control)).contains(&"form-control-label".to_owned()),
                "{control} has no label"
            );
        }
        for control in [
            r#"<select aria-label="Colour"><option>Red</option></select>"#,
            r#"<label for="c">Colour</label><select id="c"><option>Red</option></select>"#,
            r#"<label>Notes <textarea></textarea></label>"#,
        ] {
            assert!(
                !rules(&page(control)).contains(&"form-control-label".to_owned()),
                "{control} is labelled"
            );
        }
    }

    #[test]
    fn a_link_or_button_is_named_by_what_is_inside_it() {
        for named in [
            r#"<a href="/"><img src="h.png" alt="Home"></a>"#,
            r#"<button><img src="s.png" alt="Save"></button>"#,
            r#"<a href="/" aria-label="Home"></a>"#,
            r#"<a href="/"><svg aria-label="Home"></svg></a>"#,
            r#"<a href="/" title="Home"></a>"#,
            r#"<span id="n">Home</span><a href="/" aria-labelledby="n"></a>"#,
        ] {
            let found = rules(&page(named));
            assert!(
                !found.contains(&"interactive-name".to_owned()),
                "{named}: {found:?}"
            );
            assert!(
                !found.contains(&"link-text".to_owned()),
                "{named}: {found:?}"
            );
        }
        for unnamed in [
            r#"<a href="/"></a>"#,
            r#"<a href="/"><img src="h.png" alt=""></a>"#,
            r#"<button></button>"#,
        ] {
            assert!(
                rules(&page(unnamed)).contains(&"interactive-name".to_owned()),
                "{unnamed}"
            );
        }
    }

    #[test]
    fn a_link_with_only_an_aria_label_is_not_told_to_add_an_aria_label() {
        let found =
            HtmlInspector::inspect(&page(r#"<a href="/cart" aria-label="Shopping cart"></a>"#))
                .unwrap();
        assert!(found.is_clean(), "{:?}", found.issues);
        // A generic accessible name is still generic.
        assert!(rules(&page(r#"<a href="/x" aria-label="read more"></a>"#))
            .contains(&"link-text".to_owned()));
    }

    #[test]
    fn a_skip_link_may_point_at_any_element_that_exists() {
        let html = |link: &str| {
            format!(
                r##"<html lang="en"><head><title>t</title>
                <meta name="viewport" content="width=device-width"></head><body>
                {link}<nav>menu</nav><main id="primary"><h1>x</h1><section id="s2">two</section></main></body></html>"##
            )
        };
        for accepted in [
            r##"<a href="#primary">Skip to content</a>"##,
            r##"<a href="#s2">Jump to the results</a>"##,
            r##"<a href="#primary" aria-label="Skip navigation"><img src="x" alt=""></a>"##,
        ] {
            assert!(
                !rules(&html(accepted)).contains(&"skip-link".to_owned()),
                "{accepted}"
            );
        }
        for rejected in [
            "",
            r##"<a href="#s2">Section two</a>"##,
            r##"<a href="#nowhere">Skip to content</a>"##,
            r##"<a href="#">Skip to content</a>"##,
            r##"<a href="/other#primary">Skip to content</a>"##,
        ] {
            assert!(
                rules(&html(rejected)).contains(&"skip-link".to_owned()),
                "{rejected}"
            );
        }
    }

    #[test]
    fn the_conventional_main_content_ids_still_count_without_the_word_skip() {
        let html = r##"<html lang="en"><head><title>t</title>
            <meta name="viewport" content="width=device-width"></head><body>
            <a href="#content">Main</a><main id="content"><h1>x</h1></main></body></html>"##;
        assert!(!rules(html).contains(&"skip-link".to_owned()));
    }

    #[test]
    fn issues_follow_the_documented_rule_order() {
        let html = r#"<html><head></head><body><img src="a"><h3></h3><table><tr><td>x</td></tr></table></body></html>"#;
        let found = rules(html);
        let position = |rule: &str| {
            found
                .iter()
                .position(|r| r == rule)
                .unwrap_or_else(|| panic!("{rule} in {found:?}"))
        };
        assert!(position("document-lang") < position("document-title"));
        assert!(position("document-title") < position("skip-link"));
        assert!(position("skip-link") < position("meta-viewport"));
        assert!(position("meta-viewport") < position("image-alt"));
        assert!(position("image-alt") < position("empty-heading"));
        assert!(position("empty-heading") < position("table-headers"));
        assert!(position("table-headers") < position("main-landmark"));
    }

    #[test]
    fn severity_is_ordered_and_the_result_helpers_agree_with_it() {
        assert!(InspectionSeverity::Warning < InspectionSeverity::Error);
        let warnings_only = InspectionResult {
            issues: vec![InspectionIssue {
                rule: "x".to_owned(),
                severity: InspectionSeverity::Warning,
                message: String::new(),
                selector: None,
            }],
        };
        assert!(!warnings_only.is_clean() && !warnings_only.has_errors());
        assert!(InspectionResult::default().is_clean());
    }

    #[test]
    fn an_svg_title_is_not_the_page_title() {
        let html = r#"<html lang="en"><head></head><body><main><h1>x</h1>
            <svg width="10" height="10"><title>A chart</title></svg></main></body></html>"#;
        assert!(rules(html).contains(&"document-title".to_owned()));

        // The page's own title still counts, wherever the SVG one sits.
        let both = r#"<html lang="en"><head><title>Real</title></head><body><main><h1>x</h1>
            <svg><title>Tooltip</title></svg></main></body></html>"#;
        assert!(!rules(both).contains(&"document-title".to_owned()));
    }

    #[test]
    fn a_layout_table_is_not_asked_for_headers() {
        for role in ["presentation", "none", " PRESENTATION "] {
            let html = page(&format!(
                r#"<table role="{role}"><tr><td>layout</td></tr></table>"#
            ));
            assert!(
                !rules(&html).contains(&"table-headers".to_owned()),
                "role={role:?}"
            );
        }
        let data = page(r#"<table role="grid"><tr><td>x</td></tr></table>"#);
        assert!(rules(&data).contains(&"table-headers".to_owned()));
    }

    #[test]
    fn markup_that_is_not_a_document_is_still_inspected() {
        // The parser supplies the missing html/head/body, and the checks run on that.
        let found = rules("just text");
        assert!(found.contains(&"document-lang".to_owned()));
        assert!(found.contains(&"document-title".to_owned()));
        assert!(!rules("").is_empty());
    }
}
