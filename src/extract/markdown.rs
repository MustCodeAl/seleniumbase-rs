//! HTML to clean Markdown extraction tailored for LLMs and RAG pipelines.
//!
//! Provides [`html_to_markdown`] and [`MarkdownOptions`] to strip noisy page elements
//! (`nav`, `footer`, `script`, `style`, ads, banners) and convert article content directly
//! into token-efficient Markdown (headings, lists, tables, code blocks, links).

use scraper::{ElementRef, Html, Node};

/// Options configuring the HTML to Markdown conversion.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownOptions {
    /// Whether to strip hyperlinks, keeping only the anchor text (`[text](url)` -> `text`).
    pub strip_links: bool,
    /// Whether to strip `<img>` tags.
    pub strip_images: bool,
    /// Optional character limit for the generated Markdown.
    pub max_characters: Option<usize>,
}

impl MarkdownOptions {
    /// Creates a new `MarkdownOptions` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to strip hyperlinks from the output.
    #[must_use]
    pub fn strip_links(mut self, strip: bool) -> Self {
        self.strip_links = strip;
        self
    }

    /// Sets whether to strip images from the output.
    #[must_use]
    pub fn strip_images(mut self, strip: bool) -> Self {
        self.strip_images = strip;
        self
    }

    /// Sets a maximum character limit for the output.
    #[must_use]
    pub fn max_characters(mut self, max: Option<usize>) -> Self {
        self.max_characters = max;
        self
    }
}

/// Converts an HTML string or fragment into clean, LLM-friendly Markdown.
///
/// Strips non-content elements (`<script>`, `<style>`, `<nav>`, `<footer>`, `<aside>`, `<form>`)
/// and maps semantic HTML elements to standard Markdown syntax.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::extract::html_to_markdown;
///
/// let html = r#"
///     <article>
///         <h1>Rust Automation</h1>
///         <p>High performance web automation with <strong>zero</strong> overhead.</p>
///         <ul>
///             <li>Fast</li>
///             <li>Type-safe</li>
///         </ul>
///     </article>
/// "#;
///
/// let md = html_to_markdown(html);
/// assert!(md.contains("# Rust Automation"));
/// assert!(md.contains("* Fast"));
/// assert!(md.contains("**zero**"));
/// ```
#[must_use]
pub fn html_to_markdown(html_text: &str) -> String {
    html_to_markdown_with(html_text, &MarkdownOptions::default())
}

/// Converts an HTML string into clean Markdown using the provided [`MarkdownOptions`].
#[must_use]
pub fn html_to_markdown_with(html_text: &str, options: &MarkdownOptions) -> String {
    let document = Html::parse_fragment(html_text);
    let mut output = String::with_capacity(html_text.len().saturating_div(2));

    for child in document.tree.root().children() {
        if let Some(element) = ElementRef::wrap(child) {
            render_element(&element, &mut output, options);
        }
    }

    let cleaned = post_process_markdown(&output);
    if let Some(max_len) = options.max_characters
        && cleaned.len() > max_len
    {
        return cleaned.chars().take(max_len).collect();
    }

    cleaned
}

fn is_ignorable_tag(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "nav" | "footer" | "header" | "aside" | "noscript" | "svg" | "iframe"
    )
}

fn render_element(element: &ElementRef, out: &mut String, options: &MarkdownOptions) {
    let tag = element.value().name();
    if is_ignorable_tag(tag) {
        return;
    }

    match tag {
        "h1" => render_heading(element, out, "# ", options),
        "h2" => render_heading(element, out, "## ", options),
        "h3" => render_heading(element, out, "### ", options),
        "h4" => render_heading(element, out, "#### ", options),
        "h5" => render_heading(element, out, "##### ", options),
        "h6" => render_heading(element, out, "###### ", options),
        "p" => {
            out.push('\n');
            render_children(element, out, options);
            out.push('\n');
        }
        "ul" | "ol" => {
            out.push('\n');
            render_children(element, out, options);
            out.push('\n');
        }
        "li" => {
            out.push_str("\n* ");
            render_children(element, out, options);
        }
        "strong" | "b" => {
            out.push_str("**");
            render_children(element, out, options);
            out.push_str("**");
        }
        "em" | "i" => {
            out.push('*');
            render_children(element, out, options);
            out.push('*');
        }
        "code" => {
            out.push('`');
            render_children(element, out, options);
            out.push('`');
        }
        "pre" => {
            out.push_str("\n```\n");
            let code_text = element.text().collect::<Vec<_>>().join("");
            out.push_str(code_text.trim());
            out.push_str("\n```\n");
        }
        "blockquote" => {
            out.push_str("\n> ");
            render_children(element, out, options);
            out.push('\n');
        }
        "a" => {
            if options.strip_links {
                render_children(element, out, options);
            } else if let Some(href) = element.value().attr("href") {
                let link_text = element
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                if !link_text.is_empty() {
                    out.push('[');
                    out.push_str(&link_text);
                    out.push_str("](");
                    out.push_str(href.trim());
                    out.push(')');
                }
            } else {
                render_children(element, out, options);
            }
        }
        "img" => {
            if !options.strip_images {
                let alt = element.value().attr("alt").unwrap_or("image");
                if let Some(src) = element.value().attr("src") {
                    out.push_str("![");
                    out.push_str(alt);
                    out.push_str("](");
                    out.push_str(src.trim());
                    out.push(')');
                }
            }
        }
        "table" => {
            render_table(element, out);
        }
        _ => {
            render_children(element, out, options);
        }
    }
}

fn render_heading(element: &ElementRef, out: &mut String, prefix: &str, options: &MarkdownOptions) {
    out.push('\n');
    out.push_str(prefix);
    render_children(element, out, options);
    out.push('\n');
}

fn render_children(element: &ElementRef, out: &mut String, options: &MarkdownOptions) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                let text_str = text.text.as_ref();
                if !text_str.trim().is_empty() {
                    out.push_str(text_str);
                } else if text_str.contains(' ') && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    render_element(&child_el, out, options);
                }
            }
            _ => {}
        }
    }
}

fn render_table(table: &ElementRef, out: &mut String) {
    out.push('\n');
    let mut has_header = false;

    // Search for <th> in the table
    let th_sel = scraper::Selector::parse("th").ok();
    let tr_sel = scraper::Selector::parse("tr").ok();
    let td_sel = scraper::Selector::parse("td").ok();

    if let (Some(th_s), Some(tr_s), Some(td_s)) = (th_sel, tr_sel, td_sel) {
        let headers: Vec<String> = table
            .select(&th_s)
            .map(|th| th.text().collect::<Vec<_>>().join("").trim().to_string())
            .collect();

        if !headers.is_empty() {
            has_header = true;
            out.push('|');
            for h in &headers {
                out.push(' ');
                out.push_str(h);
                out.push_str(" |");
            }
            out.push('\n');

            out.push('|');
            for _ in &headers {
                out.push_str(" --- |");
            }
            out.push('\n');
        }

        for tr in table.select(&tr_s) {
            let cells: Vec<String> = tr
                .select(&td_s)
                .map(|td| td.text().collect::<Vec<_>>().join("").trim().to_string())
                .collect();

            if !cells.is_empty() {
                if !has_header {
                    // Generate dummy header for valid markdown table
                    out.push('|');
                    for (i, _) in cells.iter().enumerate() {
                        out.push_str(&format!(" Col {} |", i + 1));
                    }
                    out.push_str("\n|");
                    for _ in &cells {
                        out.push_str(" --- |");
                    }
                    out.push('\n');
                    has_header = true;
                }

                out.push('|');
                for c in &cells {
                    out.push(' ');
                    out.push_str(c);
                    out.push_str(" |");
                }
                out.push('\n');
            }
        }
    }
    out.push('\n');
}

fn post_process_markdown(raw: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    let mut consecutive_newlines = 0;

    for line in raw.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                cleaned.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            cleaned.push_str(trimmed_line);
            cleaned.push('\n');
        }
    }

    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_headings_and_paragraphs() {
        let html = "<h1>Title</h1><p>First paragraph with <strong>bold</strong> text.</p>";
        let md = html_to_markdown(html);
        assert_eq!(md, "# Title\n\nFirst paragraph with **bold** text.");
    }

    #[test]
    fn converts_lists_and_code() {
        let html = "<ul><li>Item 1</li><li>Item 2 with <code>code</code></li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("* Item 1"));
        assert!(md.contains("* Item 2 with `code`"));
    }

    #[test]
    fn strips_script_and_nav() {
        let html = "<nav><a href='/home'>Home</a></nav><script>alert(1);</script><p>Content</p>";
        let md = html_to_markdown(html);
        assert_eq!(md, "Content");
    }

    #[test]
    fn converts_tables() {
        let html = "<table><tr><th>Name</th><th>Score</th></tr><tr><td>Alice</td><td>100</td></tr></table>";
        let md = html_to_markdown(html);
        assert!(md.contains("| Name | Score |"));
        assert!(md.contains("| Alice | 100 |"));
    }
}
