# PDF Parsing Guide

SeleniumBase for Rust can print pages to PDF and extract text from PDF files.
This is useful for validating generated reports, invoices, receipts, and other
printable documents.

## What you will learn

- How to save a page as PDF.
- How to configure PDF output options.
- How to extract and assert text from PDF files and bytes.

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

`PdfOptions` lets you control page size, margins, header/footer, landscape mode,
and other print parameters.

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

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| PDF is blank | Page not fully loaded | Wait for `readyState === 'complete'` before printing. |
| Text extraction fails | PDF is scanned/image-only | Use OCR or ensure the PDF contains embedded text. |
| `print_to_pdf` unsupported | Driver is Firefox | Use Chromium-based browsers for CDP print-to-PDF. |
