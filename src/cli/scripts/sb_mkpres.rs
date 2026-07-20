use std::path::PathBuf;

pub fn make_presentation(filename: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(filename);
    let content = r#"<!DOCTYPE html>
<html>
<head>
    <title>SeleniumBase Presentation</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.css">
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
    std::fs::write(&path, content)?;
    Ok(path)
}
