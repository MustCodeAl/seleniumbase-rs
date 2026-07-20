use crate::error::SeleniumBaseError;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Presentation {
    pub title: String,
    pub slides: Vec<String>,
}

impl Presentation {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            slides: Vec::new(),
        }
    }

    pub fn add_slide(&mut self, content: &str) {
        self.slides.push(content.to_owned());
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        let mut html =
            String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
        html.push_str(&self.title);
        html.push_str(
            "</title>\n<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.css\">\n</head>\n<body>\n<div class=\"reveal\"><div class=\"slides\">\n",
        );
        for slide in &self.slides {
            html.push_str("<section>");
            html.push_str(&markdown_to_html(slide));
            html.push_str("</section>\n");
        }
        html.push_str(
            "</div></div>\n<script src=\"https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.js\"></script>\n<script>Reveal.initialize();</script>\n</body>\n</html>",
        );

        fs::write(path.as_ref(), html).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write presentation '{}': {e}",
                path.as_ref().display()
            ))
        })?;
        Ok(())
    }
}

fn markdown_to_html(md: &str) -> String {
    md.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("# ") {
                format!(
                    "<h1>{}</h1>",
                    html_escape(trimmed.strip_prefix("# ").unwrap_or(trimmed))
                )
            } else if let Some(stripped) = trimmed.strip_prefix("## ") {
                format!("<h2>{}</h2>", html_escape(stripped))
            } else if let Some(stripped) = trimmed.strip_prefix("- ") {
                format!("<li>{}</li>", html_escape(stripped))
            } else if trimmed.is_empty() {
                String::new()
            } else {
                format!("<p>{}</p>", html_escape(trimmed))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn save_presentation_contains_slides() {
        let mut pres = Presentation::new("Demo");
        pres.add_slide("# Intro\n- point one");
        pres.add_slide("## Details");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slides.html");
        pres.save(&path).unwrap();

        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Demo"));
        assert!(html.contains("<h1>Intro</h1>"));
        assert!(html.contains("<li>point one</li>"));
        assert!(html.contains("<h2>Details</h2>"));
        assert!(html.contains("reveal.js"));
    }
}
