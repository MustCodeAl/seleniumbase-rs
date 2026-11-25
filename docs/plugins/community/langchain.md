---
title: "langchain Plugin"
sidebarTitle: "langchain"
description: "LangChain integration plugin providing SeleniumBaseLoader (Markdown + head metadata for RAG pipelines) and SeleniumBaseTool for AI agent tool calling."
---

<Badge color="purple">Community plugin</Badge>

`seleniumbase-rs-plugin-langchain` (`plugins/langchain` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) connects browser automation cleanly to LLM, RAG, and agent tool-calling workflows using `seleniumbase_rs::extract::html_to_markdown_with` and `PageMetadata`.

## Install

```sh Terminal
cargo add seleniumbase-rs-plugin-langchain --git https://github.com/MustCodeAl/seleniumbase-rs-plugins
```

## Components

1. **`SeleniumBaseLoader`**: Converts full pages or scoped CSS subtrees into LangChain-compatible `Document` records (`page_content` Markdown + `DocumentMetadata` carrying `url`, `title`, `description`, `source`), stripping `<script>`, `<style>`, `<nav>`, and boilerplate chrome and supporting `strip_links`, `strip_images`, and `max_characters` budgets.
2. **`SeleniumBaseTool`**: Structured JSON-Schema agent interface dispatching typed `ToolAction` variants (`Navigate { url }`, `Click { selector }`, `TypeText { selector, text }`, `ExtractMarkdown { selector }`).
3. **`LangChainPlugin`**: Registers as plugin `"langchain"` in `Category::Core`, declaring `Capability::PageEvaluate` and `Capability::PageNavigate`.

## Examples

### Loading documents for RAG

```rust
use seleniumbase_rs_plugin_langchain::SeleniumBaseLoader;

let html = r#"
    <!doctype html>
    <html>
    <head>
        <title>LangChain Integration</title>
        <meta name="description" content="Connecting SeleniumBase to LangChain pipelines.">
    </head>
    <body>
        <main><h1>Automation with Rust</h1><p>Fast, memory safe extraction.</p></main>
    </body>
    </html>
"#;

let loader = SeleniumBaseLoader::new();
let doc = loader.load_html(html, "https://example.com/docs");
assert!(doc.page_content.contains("# Automation with Rust"));
assert_eq!(doc.metadata.title.as_deref(), Some("LangChain Integration"));
```

### Parsing LLM tool calls

```rust
use seleniumbase_rs_plugin_langchain::{SeleniumBaseTool, ToolAction};
use serde_json::json;

let tool = SeleniumBaseTool::new();
let schema = tool.schema();
assert_eq!(schema["type"], "object");

let call = json!({ "action": "navigate", "url": "https://example.com" });
let action: ToolAction = serde_json::from_value(call)?;
assert_eq!(action, ToolAction::Navigate { url: "https://example.com".into() });
Ok::<(), serde_json::Error>(())
```
