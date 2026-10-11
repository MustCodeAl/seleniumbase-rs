//! Slide presentations rendered with reveal.js.
//!
//! A [`Presentation`] is a title and a list of slides. Each slide is written in
//! a small Markdown subset (`# heading`, `## subheading`, `- list item`, and
//! plain paragraphs), and [`Presentation::save`] turns the deck into one HTML
//! file.
//!
//! The file is not offline-capable: it loads a pinned version of reveal.js and
//! its stylesheets from the jsDelivr CDN, so the browser that opens it needs
//! network access.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::presentation::Presentation;
//!
//! # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut deck = Presentation::new("Release notes");
//! deck.add_slide("# What's new\n- faster start\n- fewer flaky waits");
//! deck.add_slide("## Thanks");
//!
//! let dir = std::env::temp_dir();
//! deck.save(dir.join("release-notes.html"))?;
//! # Ok(())
//! # }
//! ```

use crate::error::SeleniumBaseError;
use std::fs;
use std::path::Path;

/// The reveal.js release the saved page loads.
const REVEAL_BASE_URL: &str = "https://cdn.jsdelivr.net/npm/reveal.js@4.6.1/dist";

/// A titled list of slides.
#[derive(Clone, Debug, Default)]
pub struct Presentation {
    /// Title of the deck, shown as the browser tab's title.
    pub title: String,
    /// The slides, in order, each in the Markdown subset described in the
    /// [module documentation](self).
    pub slides: Vec<String>,
}

impl Presentation {
    /// Creates an empty presentation with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            slides: Vec::new(),
        }
    }

    /// Adds a slide written in the Markdown subset described in the
    /// [module documentation](self).
    pub fn add_slide(&mut self, content: &str) {
        self.slides.push(content.to_owned());
    }

    /// Writes the presentation to `path` as an HTML page.
    ///
    /// Text in the title and the slides is escaped, so it is shown as written
    /// and never read as HTML.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the file cannot be
    /// written.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        let mut html = format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n\
             <link rel=\"stylesheet\" href=\"{REVEAL_BASE_URL}/reveal.css\">\n\
             <link rel=\"stylesheet\" href=\"{REVEAL_BASE_URL}/theme/white.css\">\n\
             </head>\n<body>\n<div class=\"reveal\"><div class=\"slides\">\n",
            html_escape(&self.title)
        );
        for slide in &self.slides {
            html.push_str("<section>");
            html.push_str(&markdown_to_html(slide));
            html.push_str("</section>\n");
        }
        html.push_str(&format!(
            "</div></div>\n<script src=\"{REVEAL_BASE_URL}/reveal.js\"></script>\n\
             <script>Reveal.initialize();</script>\n</body>\n</html>"
        ));

        fs::write(path.as_ref(), html).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write presentation '{}': {e}",
                path.as_ref().display()
            ))
        })?;
        Ok(())
    }
}

/// One slide's Markdown subset as HTML. Consecutive `- ` lines form one list.
fn markdown_to_html(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_list = false;
    for line in md.lines() {
        let trimmed = line.trim();
        let item = trimmed.strip_prefix("- ");
        // A blank line does not end a list; the next line that is not an item does.
        if item.is_none() && !trimmed.is_empty() && in_list {
            out.push("</ul>".to_owned());
            in_list = false;
        }
        if let Some(item) = item {
            if !in_list {
                out.push("<ul>".to_owned());
                in_list = true;
            }
            out.push(format!("<li>{}</li>", html_escape(item)));
        } else if let Some(heading) = trimmed.strip_prefix("# ") {
            out.push(format!("<h1>{}</h1>", html_escape(heading)));
        } else if let Some(heading) = trimmed.strip_prefix("## ") {
            out.push(format!("<h2>{}</h2>", html_escape(heading)));
        } else if !trimmed.is_empty() {
            out.push(format!("<p>{}</p>", html_escape(trimmed)));
        }
    }
    if in_list {
        out.push("</ul>".to_owned());
    }
    out.join("\n")
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn saved(pres: &Presentation) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slides.html");
        pres.save(&path).unwrap();
        fs::read_to_string(&path).unwrap()
    }

    #[test]
    fn save_presentation_contains_slides() {
        let mut pres = Presentation::new("Demo");
        pres.add_slide("# Intro\n- point one");
        pres.add_slide("## Details");

        let html = saved(&pres);
        assert!(html.contains("<title>Demo</title>"));
        assert!(html.contains("<h1>Intro</h1>"));
        assert!(html.contains("<ul>\n<li>point one</li>\n</ul>"), "{html}");
        assert!(html.contains("<h2>Details</h2>"));
        assert_eq!(html.matches("<section>").count(), 2);
        assert!(html.contains("reveal.js"));
    }

    #[test]
    fn the_title_cannot_inject_markup() {
        let html = saved(&Presentation::new("</title><script>alert(1)</script>"));
        assert!(html.contains("<title>&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;</title>"));
        assert!(!html.contains("<script>alert"));
    }

    #[test]
    fn slide_text_is_escaped_too() {
        let html = saved(&{
            let mut pres = Presentation::new("t");
            pres.add_slide("# <b>bold</b> & more\n- <i>x</i>\nsay \"hi\"");
            pres
        });
        assert!(html.contains("<h1>&lt;b&gt;bold&lt;/b&gt; &amp; more</h1>"));
        assert!(html.contains("<li>&lt;i&gt;x&lt;/i&gt;</li>"));
        assert!(html.contains("<p>say &quot;hi&quot;</p>"));
    }

    #[test]
    fn list_items_are_wrapped_in_a_list_and_a_new_list_starts_after_a_paragraph() {
        assert_eq!(
            markdown_to_html("- a\n- b\ntext\n- c"),
            "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n<p>text</p>\n<ul>\n<li>c</li>\n</ul>"
        );
        assert_eq!(
            markdown_to_html("- a\n\n- b"),
            "<ul>\n<li>a</li>\n<li>b</li>\n</ul>",
            "a blank line does not split a list"
        );
        assert_eq!(markdown_to_html(""), "");
    }

    #[test]
    fn the_saved_page_loads_one_pinned_reveal_release_with_a_theme() {
        let html = saved(&Presentation::new("t"));
        assert!(html.contains("reveal.js@4.6.1/dist/reveal.css"));
        assert!(html.contains("reveal.js@4.6.1/dist/theme/white.css"));
        assert!(html.contains("reveal.js@4.6.1/dist/reveal.js"));
        assert!(!html.contains("reveal.js@4/"), "no floating major version");
    }

    #[test]
    fn saving_to_a_missing_folder_names_the_path() {
        let error = Presentation::new("t")
            .save("/nonexistent-folder-for-test/slides.html")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("/nonexistent-folder-for-test/slides.html"),
            "{error}"
        );
    }
}
