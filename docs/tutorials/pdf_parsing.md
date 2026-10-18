---
title: "PDF Parsing Guide"
sidebarTitle: "PDF parsing"
---

SeleniumBase for Rust can print pages to PDF and extract text from PDF files.
This is useful for validating generated reports, invoices, receipts, and other
printable documents.

<Note>

Printing a page to PDF is always available. Reading the text of a PDF needs the
`pdf` Cargo feature, which pulls in the `pdf-extract` parser (about 34 crates).
The CLI package `seleniumbase-rs-cli` has it on by default. Built with
`--no-default-features`, `sbrs pdf-text` and `sbrs assert-pdf-text` exit with
status 2 and say how to get it.

</Note>

<CodeGroup>

```toml Cargo.toml
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["pdf"] }
```

```bash Install the CLI
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli
```

</CodeGroup>

<Card title="Quick reference" icon="file-pdf">

`save_as_pdf` and `print_to_pdf` print a page to PDF (always available).

With feature `pdf`: `get_pdf_text`, `assert_pdf_text`, `pdf::extract_text_from_bytes`, `pdf::extract_text_from_url` (refuses downloads over 100 MB) and `PdfReader::open`.

</Card>

<Info>

**What you will learn**

- How to save a page as PDF.
- How to extract and assert text from PDF files, bytes and URLs.

</Info>

## Save a page as PDF

```rust
sb.save_as_pdf("page.pdf").await?;
```

`print_to_pdf` is the same call; both print with the page background and the
page's own CSS page size (CDP `Page.printToPDF`).

## Extract text from a PDF (feature `pdf`)

These are plain functions; they read the file and need no browser.

```rust
let text = sb.get_pdf_text("page.pdf")?;
println!("{text}");
```

## Assert PDF contents (feature `pdf`)

```rust
sb.assert_pdf_text("page.pdf", "SeleniumBase")?;
```

## Bytes, URLs, pages and passwords (feature `pdf`)

```rust
use seleniumbase_rs::api::pdf::{self, PdfReader};

let bytes = std::fs::read("page.pdf")?;
let text = pdf::extract_text_from_bytes(&bytes)?;
let remote = pdf::extract_text_from_url("https://example.com/report.pdf").await?;

let reader = PdfReader::open("locked.pdf", Some("secret"))?;
for (number, page) in reader.pages()?.iter().enumerate() {
    println!("page {}: {page}", number + 1);
}
```

`extract_text_from_url` refuses downloads larger than
`pdf::MAX_DOWNLOAD_BYTES` (100 MB).

## Use cases

- Validate generated reports.
- Verify invoices and receipts.
- Archive page snapshots as searchable documents.

## Troubleshooting

<AccordionGroup>

<Accordion title="PDF is blank" icon="file">

**Likely cause.** Page not fully loaded.

**Fix.** Wait for `readyState === 'complete'` before printing.

</Accordion>

<Accordion title="Text extraction fails" icon="magnifying-glass">

**Likely cause.** PDF is scanned/image-only.

**Fix.** Use <Tooltip tip="Optical character recognition: reading text out of an image.">OCR</Tooltip> or ensure the PDF contains embedded text.

</Accordion>

<Accordion title="print_to_pdf unsupported" icon="globe">

**Likely cause.** Driver is Firefox.

**Fix.** Use Chromium-based browsers for CDP print-to-PDF.

</Accordion>

<Accordion title="no method named get_pdf_text" icon="puzzle-piece">

**Likely cause.** Built without the `pdf` feature.

**Fix.** Add `features = ["pdf"]` to the dependency.

</Accordion>

</AccordionGroup>
