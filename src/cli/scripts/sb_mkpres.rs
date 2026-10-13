//! `sbrs mkpres`: a starter reveal.js slide deck.

use crate::cli::commands::write_new_file;
use std::path::PathBuf;

/// Writes a reveal.js 4 example deck to `filename`.
///
/// Missing folders are created. A file that already exists is not replaced:
/// that is an [`std::io::ErrorKind::AlreadyExists`] error. The page loads
/// reveal.js and its `black` theme from jsDelivr, so it needs a network
/// connection to show the slides.
pub fn make_presentation(filename: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(filename);
    let content = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>SeleniumBase Presentation</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/theme/black.css">
</head>
<body>
    <div class="reveal">
        <div class="slides">
            <section><h1>SeleniumBase Rust</h1></section>
            <section><h2>Features</h2><ul><li>Stealth</li><li>CDP</li><li>CLI</li></ul></section>
        </div>
    </div>
    <script src="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.js"></script>
    <script>Reveal.initialize();</script>
</body>
</html>"#;
    write_new_file(&path, content)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deck_loads_reveal_with_a_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.html");

        let written = make_presentation(path.to_str().unwrap()).unwrap();

        let html = std::fs::read_to_string(written).unwrap();
        assert!(html.contains("reveal.js@4/dist/reveal.css"));
        assert!(
            html.contains("reveal.js@4/dist/theme/black.css"),
            "no theme: {html}"
        );
        assert!(html.contains("reveal.js@4/dist/reveal.js\""));
    }

    #[test]
    fn an_existing_deck_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.html");
        std::fs::write(&path, "<p>my slides</p>").unwrap();

        let error = make_presentation(path.to_str().unwrap()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<p>my slides</p>");
    }
}
