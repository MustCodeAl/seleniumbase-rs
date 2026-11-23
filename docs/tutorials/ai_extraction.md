---
title: "AI & LLM Structured Extraction"
sidebarTitle: "AI & LLM Extraction"
description: "Token-efficient HTML-to-Markdown conversion for LLMs/RAG, automated typed form filling, Schema.org JSON-LD extraction, and table parsing."
---

Modern AI and LLM agents require clean, noise-free context from the web. Raw HTML contains thousands of tokens of navigational headers, tracking scripts, style sheets, advertising banners, and cookie consent popups that consume context window budgets and degrade LLM reasoning performance.

`seleniumbase-rs` provides built-in zero-allocation and token-efficient extraction primitives specifically designed for AI agents, RAG (Retrieval-Augmented Generation) pipelines, and autonomous form execution:

- **Clean Markdown Extraction (`markdown()`)**: Converts full pages or locator-scoped DOM subtrees into structured Markdown, stripping noisy boilerplate while preserving article headings, lists, tables, and hyperlinks.
- **Automated Strongly-Typed Form Filling (`fill_form()`)**: Automatically serializes any Serde Rust struct into matched HTML form inputs (`input`, `textarea`, `select`), eliminating fragile manual input field typing.
- **Schema.org JSON-LD Parsing (`json_ld()`)**: Directly deserializes structured metadata (including nested `@graph` trees) into typed Rust models.
- **Table Data Extraction (`table()`)**: Parses HTML `<table>` elements into tabular records and typed structs.

<Info>

**What you will learn**

- How to extract clean Markdown from full web pages and specific element containers.
- How to configure `MarkdownOptions` to strip links, omit images, or cap token character budgets.
- How to automate form filling using typed Rust structs with `FormData`.
- How to combine CDP or WebDriver automation with LLM prompts in reliable tests.

</Info>

<Visibility for="agents">

Summary for AI exports: Use `sb.markdown().await?` on `BaseCase`, `page.markdown().await?` on pure CDP `Page`, or `locator.markdown().await?` on `Locator` to convert HTML into clean, token-efficient Markdown for LLMs. Scripts, styles, `<nav>`, `<footer>`, `<aside>`, and `<form>` containers are automatically stripped. For finer control or offline parsing, use `extract::html_to_markdown(html)` or `extract::html_to_markdown_with(html, &options)` with `MarkdownOptions::new().strip_links(bool).strip_images(bool).max_characters(Option<usize>)`. For form automation, use `sb.fill_form(&struct).await?` or `page.fill_form(&struct).await?`: any `Serialize` struct is mapped via `extract::FormData::from_serializable(&val)` to match `input[name]`, `textarea[name]`, and `select[name]` elements automatically.

</Visibility>

---

## Clean Markdown Extraction (`markdown()`)

The `markdown()` method is available on both the WebDriver engine (`BaseCase`) and the pure CDP engine (`Page` and `Locator`).

### Full Page Extraction

When invoked on a page or `BaseCase`, `markdown()` reads the HTML document, discards noise tags (`<script>`, `<style>`, `<nav>`, `<footer>`, `<aside>`, `<form>`, SVG icons, and ads), and formats the semantic content into Markdown:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn extract_docs_for_llm(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://docs.example.com/api/getting-started").await?;
        page.locator("article").expect().to_be_visible().await?;

        // Extract clean markdown for feeding into an LLM or vector database
        let markdown: String = page.markdown().await?;
        assert!(markdown.contains("# Getting Started"));
        assert!(!markdown.contains("<script"));
        assert!(!markdown.contains("<style"));

        Ok(())
    })
    .await
}
```

### Scoped Container Extraction (`Locator::markdown`)

Often, a page contains sidebars, header navigation, or related links that you wish to exclude entirely. You can scope Markdown extraction to a specific container using `Locator::markdown()`:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn extract_article_body_only(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://news.example.com/posts/release-notes").await?;

        // Only convert the main article body
        let article_md = page.locator("article[data-testid='main-content']").markdown().await?;
        assert!(article_md.starts_with("# "));

        Ok(())
    })
    .await
}
```

### Custom Markdown Options (`MarkdownOptions`)

For offline processing or custom token limits, call `extract::html_to_markdown_with`:

```rust
use seleniumbase_rs::extract::{html_to_markdown_with, MarkdownOptions};

let html = r#"
    <article>
        <h1>API Documentation</h1>
        <p>Visit <a href="https://example.com/pricing">Pricing</a> for details.</p>
        <img src="banner.jpg" alt="Platform Banner" />
    </article>
"#;

let options = MarkdownOptions::new()
    .strip_links(true)       // Convert [text](url) to plain text
    .strip_images(true)      // Remove images completely
    .max_characters(Some(5000)); // Cap length to fit token budget

let md = html_to_markdown_with(html, &options);
assert_eq!(md.trim(), "# API Documentation\n\nVisit Pricing for details.");
```

---

## Automated Strongly-Typed Form Filling (`fill_form()`)

Manually locating and typing into dozens of form fields is repetitive and prone to selector breakages. With `fill_form()`, you can serialize any typed Rust struct directly into web forms.

`fill_form()` maps each field of your struct to DOM elements matching:
1. `input[name="{field}"]` or `input[id="{field}"]`
2. `textarea[name="{field}"]`
3. `select[name="{field}"]`

### WebDriver Example (`BaseCase::fill_form`)

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};
use serde::Serialize;

#[derive(Serialize)]
struct UserRegistration<'a> {
    first_name: &'a str,
    last_name: &'a str,
    email: &'a str,
    company: &'a str,
    subscribe_newsletter: bool,
}

#[rstest]
#[tokio::test]
async fn automated_registration_form(sb: Sb) -> Result<()> {
    sb.run(async |mut sb| {
        sb.open("https://app.example.com/register").await?;
        sb.assert_element("#registration-form").await?;

        let form_data = UserRegistration {
            first_name: "Alice",
            last_name: "Smith",
            email: "alice@example.com",
            company: "Acme Corp",
            subscribe_newsletter: true,
        };

        // Automatically fill all matching form inputs
        sb.fill_form(&form_data).await?;

        sb.click("#submit-button").await?;
        sb.assert_text("Welcome, Alice!", "#welcome-banner").await?;

        Ok(())
    })
    .await
}
```

### Pure CDP Example (`Page::fill_form`)

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};
use serde::Serialize;

#[derive(Serialize)]
struct SettingsUpdate<'a> {
    display_name: &'a str,
    bio: &'a str,
}

#[rstest]
#[tokio::test]
async fn update_profile_settings(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://app.example.com/settings/profile").await?;
        page.locator("#profile-form").expect().to_be_visible().await?;

        let settings = SettingsUpdate {
            display_name: "RustDeveloper",
            bio: "Building autonomous AI agents with seleniumbase-rs.",
        };

        page.fill_form(&settings).await?;
        page.locator("#save-button").click().await?;
        page.locator("#toast-success").expect().to_have_text("Profile updated").await?;

        Ok(())
    })
    .await
}
```

---

## Schema.org JSON-LD Extraction (`json_ld()`)

Many content-heavy sites (e-commerce, recipe portals, news publications) publish Schema.org structured data inside `<script type="application/ld+json">` tags.

`seleniumbase-rs` extracts and parses these scripts directly into typed models, automatically flattening nested `@graph` arrays:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Product {
    name: String,
    sku: Option<String>,
    #[serde(rename = "description")]
    desc: Option<String>,
}

#[rstest]
#[tokio::test]
async fn extract_product_catalog_json_ld(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://shop.example.com/item/42").await?;
        page.locator("#product-detail").expect().to_be_visible().await?;

        // Deserializes all JSON-LD blocks on the page
        let products: Vec<Product> = page.json_ld().await?;
        assert!(!products.is_empty());
        assert_eq!(products[0].name, "Ergonomic Rust Mechanical Keyboard");

        Ok(())
    })
    .await
}
```

---

## Table Parsing (`table()`)

Extract HTML tables without writing manual DOM iteration loops:

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct BenchmarkRow {
    framework: String,
    throughput_rps: u64,
    latency_ms: f64,
}

#[rstest]
#[tokio::test]
async fn parse_benchmark_table(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://benchmarks.example.com").await?;

        let table = page.table("#benchmark-results").await?;
        assert_eq!(table.headers(), &["Framework", "Throughput (rps)", "Latency (ms)"]);

        // Automatically deserialize rows into typed structs
        let records: Vec<BenchmarkRow> = table.to_records()?;
        assert!(records.iter().any(|r| r.framework == "seleniumbase-rs"));

        Ok(())
    })
    .await
}
```
