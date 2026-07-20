# PDF Parsing Guide

SeleniumBase for Rust can print pages to PDF and extract text from PDF files.

## Save a page as PDF

```rust
sb.save_as_pdf("page.pdf").await?;
```

## Print to PDF with options

```rust
use seleniumbase_rs::api::pdf::PdfOptions;

let options = PdfOptions::default();
sb.print_to_pdf("page.pdf", &options).await?;
```

## Extract text from a PDF

```rust
let text = sb.get_pdf_text("page.pdf").await?;
println!("{text}");
```

## Assert PDF contents

```rust
sb.assert_pdf_text("page.pdf", "SeleniumBase").await?;
```

## Extract text from PDF bytes

```rust
let bytes = std::fs::read("page.pdf")?;
let text = seleniumbase_rs::api::pdf::extract_pdf_text(&bytes)?;
```

## Use cases

- Validate generated reports.
- Verify invoices and receipts.
- Archive page snapshots as searchable documents.
