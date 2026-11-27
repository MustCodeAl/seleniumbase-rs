---
title: "AI & LLM Structured Extraction"
sidebarTitle: "AI & LLM Extraction"
description: "Token-efficient HTML-to-Markdown conversion, Schema.org JSON-LD parsing, table extraction, typed form data serialization, and LLM prompt builders via seleniumbase-rs-plugin-extractor."
---

Modern AI and LLM workflows need clean, structured data from web pages instead of raw HTML filled with navigation bars, tracking scripts, stylesheets, and cookie banners.

<Note>

Since v0.10.0, structured DOM extraction, HTML-to-Markdown conversion, JSON-LD parsing, table parsing, `FormData`, and AI prompt builders live in `seleniumbase-rs-plugin-extractor` inside [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins) (see the [extractor plugin](/plugins/builtin/extractor) reference and [langchain plugin](/plugins/community/langchain)).

</Note>

`seleniumbase-rs-plugin-extractor` provides two layers:

- **In-browser plugin (`Extractor`)**: Discovers repeating cards, JSON-LD blocks, and tables on a live CDP page (`Extractor::discover`, `Extractor::extract`) or runs batch jobs via `sbrs plugin run extractor`.
- **Pure offline HTML & AI helpers (`html`, `ai`)**: Convert captured HTML into clean Markdown (`html_to_markdown`, `html_to_markdown_with`, `MarkdownOptions`), parse page metadata (`PageMetadata`), JSON-LD (`extract_json_ld`, `extract_json_ld_raw`), HTML tables (`TableData`), and typed form payloads (`FormData`), plus optional provider-agnostic LLM prompt builders (`features = ["ai"]`: `AiPromptBuilder`, `object_schema`, `parse_ai_response`, `parse_ai_records`).

<Info>

**What you will learn**

- How to convert HTML fetched from `Page::content()` or `BaseCase::page_source()` into token-efficient Markdown.
- How to configure `MarkdownOptions` to strip links, drop images, or cap output characters.
- How to extract `PageMetadata`, Schema.org JSON-LD, and HTML tables offline.
- How to serialize typed Rust structs with `FormData` and build JSON-Schema LLM prompts with `AiPromptBuilder`.

</Info>

<Visibility for="agents">

Summary for AI exports: Since v0.10.0, extraction moved out of `seleniumbase-rs` core into `seleniumbase-rs-plugin-extractor` (`MustCodeAl/seleniumbase-rs-plugins`). Fetch HTML with `page.content().await?` (pure CDP) or `sb.page_source().await?` (WebDriver), then call `seleniumbase_rs_plugin_extractor::html::{html_to_markdown, html_to_markdown_with, MarkdownOptions, PageMetadata, extract_json_ld, extract_json_ld_raw, TableData, FormData}`. Enable `features = ["ai"]` for `seleniumbase_rs_plugin_extractor::ai::{AiPromptBuilder, AiExtractionPrompt, object_schema, parse_ai_response, parse_ai_records}`.

</Visibility>

---

## Setup

Add `seleniumbase-rs-plugin-extractor` alongside `seleniumbase-rs` in `Cargo.toml`:

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", branch = "main", features = ["testing"] }
seleniumbase-rs-plugin-extractor = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins", branch = "main", features = ["ai"] }
serde = { version = "1", features = ["derive"] }
```

---

## Clean Markdown Extraction (`html_to_markdown`)

Fetch the document HTML from a pure CDP `Page` (or WebDriver `BaseCase`), then prune noise tags (`<script>`, `<style>`, `<nav>`, `<footer>`, `<aside>`, `<form>`, SVG icons) and convert semantic content into Markdown:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};
use seleniumbase_rs_plugin_extractor::html::html_to_markdown;

#[rstest]
#[tokio::test]
async fn extract_docs_for_llm(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://docs.example.com/api/getting-started").await?;
            page.locator("article").expect().to_be_visible().await?;

            let html = page.content().await?;
            let markdown = html_to_markdown(&html);
            assert!(markdown.contains("# Getting Started"));
            assert!(!markdown.contains("<script"));
            assert!(!markdown.contains("<style"));

            Ok(())
        })
        .await
}
```

### Custom Markdown Options (`MarkdownOptions`)

For scoped HTML or strict token budgets, pass `MarkdownOptions` to `html_to_markdown_with`:

```rust
use seleniumbase_rs_plugin_extractor::html::{html_to_markdown_with, MarkdownOptions};

let html = r#"
    <article>
        <h1>API Documentation</h1>
        <p>Visit <a href="https://example.com/pricing">Pricing</a> for details.</p>
        <img src="banner.jpg" alt="Platform Banner" />
    </article>
"#;

let options = MarkdownOptions::new()
    .strip_links(true)
    .strip_images(true)
    .max_characters(Some(5000));

let md = html_to_markdown_with(html, &options);
assert_eq!(md.trim(), "# API Documentation\n\nVisit Pricing for details.");
```

---

## Schema.org JSON-LD, Metadata, and Tables

`seleniumbase_rs_plugin_extractor::html` parses structured metadata, `<script type="application/ld+json">` blocks (including `@graph` arrays), and HTML `<table>` elements directly from page HTML:

```rust
use seleniumbase_rs_plugin_extractor::html::{
    extract_json_ld, PageMetadata, TableData,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Product {
    name: String,
    sku: Option<String>,
}

let meta = PageMetadata::from_html(&html);
let products: Vec<Product> = extract_json_ld(&html);
let tables = TableData::extract_all(&html);
```

---

## Strongly Typed Form Serialization (`FormData`)

`FormData::from_serializable` flattens any `Serialize` Rust struct into ordered `(field_name, value)` pairs so test helpers can populate form inputs without hardcoding field-by-field boilerplate:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};
use seleniumbase_rs_plugin_extractor::html::FormData;
use serde::Serialize;

#[derive(Serialize)]
struct SettingsUpdate<'a> {
    display_name: &'a str,
    bio: &'a str,
}

#[rstest]
#[tokio::test]
async fn update_profile_settings(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://app.example.com/settings/profile").await?;
            page.locator("#profile-form").expect().to_be_visible().await?;

            let settings = SettingsUpdate {
                display_name: "RustDeveloper",
                bio: "Building browser automation with seleniumbase-rs.",
            };
            let form = FormData::from_serializable(&settings)
                .map_err(|e| seleniumbase_rs::SeleniumBaseError::Other(e.to_string()))?;

            for (name, value) in form.fields() {
                page.locator(format!("[name='{name}']")).type_text(value).await?;
            }

            page.locator("#save-button").click().await?;
            page.locator("#toast-success")
                .expect()
                .to_have_text("Profile updated")
                .await?;
            Ok(())
        })
        .await
}
```

---

## Provider-Agnostic AI Extraction Prompts (`features = ["ai"]`)

Enable the optional `ai` feature to construct structured JSON-Schema extraction prompts and parse model responses without coupling to a single LLM SDK:

```rust
use seleniumbase_rs_plugin_extractor::ai::{
    object_schema, parse_ai_response, AiPromptBuilder,
};
use seleniumbase_rs_plugin_extractor::html::html_to_markdown;

let markdown = html_to_markdown(&html);
let schema = object_schema(&[
    ("title", "string"),
    ("price", "number"),
    ("in_stock", "boolean"),
]);

let prompt = AiPromptBuilder::new()
    .json_schema(schema)
    .content(markdown)
    .instructions("Extract the primary product listing.")
    .max_content_chars(12_000)
    .build()?;

// Send `prompt.system` and `prompt.user` to your chosen LLM API, then parse the reply:
let parsed = parse_ai_response(&llm_raw_reply)?;
```

For full LangChain document loader and agent tool bindings, see the [`langchain` community plugin](/plugins/community/langchain). For live DOM discovery inside a CDP plugin host, see the [`extractor` plugin](/plugins/builtin/extractor).
