---
title: "PDF Parsing Guide"
---

SeleniumBase for Rust can print pages to PDF and extract text from PDF files.
This is useful for validating generated reports, invoices, receipts, and other
printable documents.

<Note>

Printing a page to PDF is always available. Reading the text of a PDF needs the
`pdf` Cargo feature, which pulls in the `pdf-extract` parser (about 34 crates):
`seleniumbase-rs = { version = "0.4", features = ["pdf"] }`. The CLI package
`seleniumbase-rs-cli` has it on by default
(`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli`).
Built with `--no-default-features`, `sbrs pdf-text` and `sbrs assert-pdf-text`
exit with status 2 and say how to get it.

</Note>

## What you will learn

- How to save a page as PDF.
- How to extract and assert text from PDF files, bytes and URLs.

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

| Symptom | Likely cause | Fix |
|---|---|---|
| PDF is blank | Page not fully loaded | Wait for `readyState === 'complete'` before printing. |
| Text extraction fails | PDF is scanned/image-only | Use OCR or ensure the PDF contains embedded text. |
| `print_to_pdf` unsupported | Driver is Firefox | Use Chromium-based browsers for CDP print-to-PDF. |
| `no method named get_pdf_text` | Built without the `pdf` feature | Add `features = ["pdf"]` to the dependency. |
