//! PDF text extraction and CDP-based print-to-PDF helpers.

use crate::error::SeleniumBaseError;
use std::fs;
use std::path::Path;

/// Extracts text from a PDF file.
pub fn extract_text_from_file<P: AsRef<Path>>(path: P) -> Result<String, SeleniumBaseError> {
    let path = path.as_ref();
    pdf_extract::extract_text(path).map_err(|e| SeleniumBaseError::Io(std::io::Error::other(e)))
}

/// Extracts text from raw PDF bytes.
pub fn extract_text_from_bytes(bytes: &[u8]) -> Result<String, SeleniumBaseError> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| SeleniumBaseError::Io(std::io::Error::other(e)))
}

/// Writes PDF bytes to disk.
pub fn save_pdf_bytes<P: AsRef<Path>>(bytes: &[u8], path: P) -> Result<(), SeleniumBaseError> {
    fs::write(path.as_ref(), bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::{BuiltinFont, Mm, PdfDocument};
    use std::fs::File;
    use std::io::BufWriter;

    fn write_sample_pdf<P: AsRef<Path>>(path: P) {
        let (doc, page, layer) = PdfDocument::new("Sample", Mm(210.0), Mm(297.0), "Layer 1");
        let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
        doc.get_page(page).get_layer(layer).use_text(
            "Hello SeleniumBase",
            12.0,
            Mm(50.0),
            Mm(250.0),
            &font,
        );
        doc.save(&mut BufWriter::new(File::create(path).unwrap()))
            .unwrap();
    }

    #[test]
    fn extracts_text_from_pdf_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");
        write_sample_pdf(&path);

        let bytes = std::fs::read(&path).unwrap();
        let text = extract_text_from_bytes(&bytes).unwrap();
        assert!(text.contains("Hello SeleniumBase"));
    }

    #[test]
    fn saves_and_extracts_pdf_text() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("sample.pdf");
        let dst = dir.path().join("copy.pdf");
        write_sample_pdf(&src);

        let bytes = std::fs::read(&src).unwrap();
        save_pdf_bytes(&bytes, &dst).unwrap();
        let text = extract_text_from_file(&dst).unwrap();
        assert!(text.contains("Hello SeleniumBase"));
    }
}
