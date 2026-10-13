//! PDF text extraction, the Rust counterpart of SeleniumBase's `get_pdf_text`.
//!
//! Text comes out of a PDF in three ways, from simplest to most flexible:
//!
//! * [`extract_text_from_file`] and [`extract_text_from_bytes`] return the whole
//!   document's text.
//! * [`extract_text_from_url`] downloads a PDF first.
//! * [`PdfReader`] opens a document once and reads it page by page, with a
//!   password if it has one.
//!
//! PDFs often come from the web, so they are treated as untrusted input: a
//! file the extractor cannot parse is an [`Err`], never a panic, and a
//! download is capped at [`MAX_DOWNLOAD_BYTES`].
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::pdf::PdfReader;
//!
//! # fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let reader = PdfReader::open("report.pdf", None)?;
//! println!("{} pages", reader.page_count());
//! let first = reader.page_text(1)?; // pages count from 1
//! assert!(first.contains("Quarterly"));
//! # Ok(())
//! # }
//! ```

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use crate::error::SeleniumBaseError;

/// The most bytes [`extract_text_from_url`] will download.
pub const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

/// A PDF opened for reading its text.
///
/// Pages are numbered from 1, as a reader shows them.
pub struct PdfReader {
    document: pdf_extract::Document,
}

impl std::fmt::Debug for PdfReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfReader")
            .field("pages", &self.page_count())
            .finish_non_exhaustive()
    }
}

impl PdfReader {
    /// Opens the PDF at `path`.
    ///
    /// `password` is used only if the document is encrypted; an encrypted
    /// document that needs no password opens without one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be read, and
    /// [`SeleniumBaseError::Pdf`] if it is not a PDF this library can parse,
    /// or is encrypted and `password` is missing or wrong.
    pub fn open<P: AsRef<Path>>(
        path: P,
        password: Option<&str>,
    ) -> Result<Self, SeleniumBaseError> {
        Self::from_bytes(&std::fs::read(path.as_ref())?, password)
    }

    /// Opens a PDF held in memory. See [`open`](Self::open) for `password`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Pdf`] if the bytes are not a PDF this
    /// library can parse, or the document is encrypted and `password` is
    /// missing or wrong.
    pub fn from_bytes(bytes: &[u8], password: Option<&str>) -> Result<Self, SeleniumBaseError> {
        let document = guarded(|| {
            // The password has to go to the loader. Loaded without it, an
            // encrypted file comes back with its pages left out, and decrypting
            // that afterwards finds nothing to decrypt.
            let options = match password {
                Some(password) => pdf_extract::LoadOptions::with_password(password),
                None => pdf_extract::LoadOptions::default(),
            };
            let document = pdf_extract::Document::load_mem_with_options(bytes, options)?;
            if document.is_encrypted() {
                // The loader decrypts a file whose password it was given, or
                // that needs none, and leaves any other one encrypted.
                return Err(pdf_extract::Error::InvalidPassword.into());
            }
            Ok(document)
        })?;
        Ok(Self { document })
    }

    /// The number of pages.
    pub fn page_count(&self) -> usize {
        self.document.get_pages().len()
    }

    /// The text of the whole document.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Pdf`] if a page cannot be read.
    pub fn text(&self) -> Result<String, SeleniumBaseError> {
        guarded(|| {
            let mut text = String::new();
            {
                let mut output = pdf_extract::PlainTextOutput::new(&mut text);
                pdf_extract::output_doc(&self.document, &mut output)?;
            }
            Ok(expand_ligatures(&text))
        })
    }

    /// The text of one page, numbered from 1.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Pdf`] if `page` is 0 or past the last
    /// page, or the page cannot be read.
    pub fn page_text(&self, page: usize) -> Result<String, SeleniumBaseError> {
        let count = self.page_count();
        let number = u32::try_from(page)
            .ok()
            .filter(|number| (1..=count).contains(&(*number as usize)))
            .ok_or_else(|| {
                SeleniumBaseError::Pdf(format!(
                    "page {page} does not exist; pages are numbered from 1 and this document has {count}"
                ))
            })?;
        guarded(|| {
            let mut text = String::new();
            {
                let mut output = pdf_extract::PlainTextOutput::new(&mut text);
                pdf_extract::output_doc_page(&self.document, &mut output, number)?;
            }
            Ok(expand_ligatures(&text))
        })
    }

    /// The text of every page, in order.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Pdf`] if a page cannot be read.
    pub fn pages(&self) -> Result<Vec<String>, SeleniumBaseError> {
        (1..=self.page_count())
            .map(|page| self.page_text(page))
            .collect()
    }
}

/// Runs a call into the PDF parser and turns its failures into errors.
///
/// The parser has code paths that panic on malformed files (a page tree with
/// no `MediaBox`, for one). A PDF is untrusted input, so a panic there is
/// reported as an error instead of ending the caller's test run.
fn guarded<T>(
    work: impl FnOnce() -> Result<T, pdf_extract::OutputError>,
) -> Result<T, SeleniumBaseError> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(result) => result.map_err(output_error),
        Err(payload) => {
            let reason = payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "the parser failed".to_owned());
            Err(SeleniumBaseError::Pdf(format!(
                "the PDF could not be read ({reason})"
            )))
        }
    }
}

/// Maps the extractor's error to ours: a file problem stays an I/O error, a
/// bad password is named as one, and everything else is a PDF error.
fn output_error(error: pdf_extract::OutputError) -> SeleniumBaseError {
    match error {
        pdf_extract::OutputError::IoError(error) => SeleniumBaseError::Io(error),
        pdf_extract::OutputError::PdfError(
            pdf_extract::Error::InvalidPassword
            | pdf_extract::Error::Decryption(
                pdf_extract::encryption::DecryptionError::IncorrectPassword,
            ),
        ) => SeleniumBaseError::Pdf(
            "the PDF is encrypted and the password is missing or wrong".to_owned(),
        ),
        other => SeleniumBaseError::Pdf(other.to_string()),
    }
}

/// Writes the typographic ligatures a PDF often stores (`ﬁ`, `ﬂ`, `ﬀ`, `ﬃ`,
/// `ﬄ`, `ﬅ`, `ﬆ`) as the letters they join, so that text found in a page can be
/// searched for as it reads: Chrome prints "fixture" with a single `ﬁ` glyph.
fn expand_ligatures(text: &str) -> String {
    let mut expanded = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{FB00}' => expanded.push_str("ff"),
            '\u{FB01}' => expanded.push_str("fi"),
            '\u{FB02}' => expanded.push_str("fl"),
            '\u{FB03}' => expanded.push_str("ffi"),
            '\u{FB04}' => expanded.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => expanded.push_str("st"),
            other => expanded.push(other),
        }
    }
    expanded
}

/// Extracts text from a PDF file.
///
/// # Errors
///
/// See [`PdfReader::open`].
pub fn extract_text_from_file<P: AsRef<Path>>(path: P) -> Result<String, SeleniumBaseError> {
    PdfReader::open(path, None)?.text()
}

/// Extracts text from raw PDF bytes.
///
/// # Errors
///
/// See [`PdfReader::from_bytes`].
pub fn extract_text_from_bytes(bytes: &[u8]) -> Result<String, SeleniumBaseError> {
    PdfReader::from_bytes(bytes, None)?.text()
}

/// Downloads the PDF at `url` and extracts its text.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Download`] if the request fails, the server
/// answers with an error status, or the file is larger than
/// [`MAX_DOWNLOAD_BYTES`]; otherwise see [`PdfReader::from_bytes`].
pub async fn extract_text_from_url(url: &str) -> Result<String, SeleniumBaseError> {
    let download = |reason: String| SeleniumBaseError::Download {
        url: url.to_owned(),
        reason,
    };
    crate::install_tls_provider();
    let mut response = reqwest::get(url)
        .await
        .map_err(|error| download(error.to_string()))?;
    if !response.status().is_success() {
        return Err(download(format!(
            "the server answered {}",
            response.status()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| download(error.to_string()))?
    {
        if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
            return Err(download(format!(
                "the file is larger than {MAX_DOWNLOAD_BYTES} bytes"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    extract_text_from_bytes(&bytes)
}

/// Writes PDF bytes to disk.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the file cannot be written.
pub fn save_pdf_bytes<P: AsRef<Path>>(bytes: &[u8], path: P) -> Result<(), SeleniumBaseError> {
    std::fs::write(path.as_ref(), bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::{
        BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point,
    };

    /// A document with one page of text per entry of `pages`.
    fn pdf_bytes(pages: &[&str]) -> Vec<u8> {
        let mut doc = PdfDocument::new("Sample");
        for text in pages {
            let ops = vec![
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                    size: printpdf::Pt(12.0),
                },
                Op::SetTextCursor {
                    pos: Point::new(Mm(50.0), Mm(250.0)),
                },
                Op::ShowText {
                    items: vec![(*text).into()],
                },
                Op::EndTextSection,
            ];
            doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
        }
        let mut warnings = Vec::new();
        doc.save(&PdfSaveOptions::default(), &mut warnings)
    }

    fn write_sample_pdf<P: AsRef<Path>>(path: P) {
        std::fs::write(path, pdf_bytes(&["Hello SeleniumBase"])).unwrap();
    }

    #[test]
    fn ligatures_are_written_as_the_letters_they_join() {
        assert_eq!(
            expand_ligatures("Welcome to the \u{FB01}xture, \u{FB02}at \u{FB00} \u{FB03} \u{FB04} \u{FB05}\u{FB06}"),
            "Welcome to the fixture, flat ff ffi ffl stst"
        );
        assert_eq!(expand_ligatures("plain ✓ text"), "plain ✓ text");
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

    #[test]
    fn a_reader_gives_each_page_on_its_own_counting_from_one() {
        let reader =
            PdfReader::from_bytes(&pdf_bytes(&["alpha", "bravo", "charlie"]), None).unwrap();
        assert_eq!(reader.page_count(), 3);
        assert!(reader.page_text(1).unwrap().contains("alpha"));
        assert!(reader.page_text(2).unwrap().contains("bravo"));
        assert!(!reader.page_text(2).unwrap().contains("alpha"));
        assert!(reader.page_text(3).unwrap().contains("charlie"));

        let pages = reader.pages().unwrap();
        assert_eq!(pages.len(), 3);
        assert!(pages[0].contains("alpha") && pages[2].contains("charlie"));

        let whole = reader.text().unwrap();
        assert!(["alpha", "bravo", "charlie"]
            .iter()
            .all(|word| whole.contains(word)));
    }

    #[test]
    fn a_page_that_does_not_exist_is_an_error_that_says_how_many_there_are() {
        let reader = PdfReader::from_bytes(&pdf_bytes(&["one", "two"]), None).unwrap();
        for page in [0, 3, usize::MAX] {
            let error = reader.page_text(page).unwrap_err();
            assert!(
                matches!(&error, SeleniumBaseError::Pdf(message) if message.contains("has 2")),
                "page {page}: {error:?}"
            );
        }
    }

    #[test]
    fn a_file_that_is_not_a_pdf_is_a_pdf_error_and_a_missing_file_is_an_io_error() {
        let garbage = extract_text_from_bytes(b"this is not a pdf at all").unwrap_err();
        assert!(matches!(garbage, SeleniumBaseError::Pdf(_)), "{garbage:?}");

        let dir = tempfile::tempdir().unwrap();
        let missing = extract_text_from_file(dir.path().join("nope.pdf")).unwrap_err();
        assert!(
            matches!(&missing, SeleniumBaseError::Io(e) if e.kind() == std::io::ErrorKind::NotFound),
            "{missing:?}"
        );
    }

    /// A file with a valid structure whose page has no `MediaBox`. The
    /// extractor panics on it.
    fn pdf_without_media_box() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R >>",
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        pdf
    }

    #[test]
    fn a_pdf_that_makes_the_parser_panic_is_an_error_not_a_crash() {
        let result = extract_text_from_bytes(&pdf_without_media_box());
        assert!(
            matches!(&result, Err(SeleniumBaseError::Pdf(_))),
            "expected a PDF error, got {result:?}"
        );
    }

    /// The sample document, encrypted with a user password.
    fn encrypted_pdf_bytes(password: &str) -> Vec<u8> {
        let mut document = pdf_extract::Document::load_mem(&pdf_bytes(&["secret words"])).unwrap();
        let state = pdf_extract::EncryptionState::try_from(pdf_extract::EncryptionVersion::V2 {
            document: &document,
            owner_password: "owner",
            user_password: password,
            key_length: 128,
            permissions: pdf_extract::Permissions::all(),
        })
        .unwrap();
        document.encrypt(&state).unwrap();
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn an_encrypted_pdf_opens_with_its_password_and_says_so_without_it() {
        let bytes = encrypted_pdf_bytes("hunter2");

        let reader = PdfReader::from_bytes(&bytes, Some("hunter2")).unwrap();
        assert!(reader.text().unwrap().contains("secret words"));

        for wrong in [None, Some("wrong")] {
            let error = PdfReader::from_bytes(&bytes, wrong).unwrap_err();
            assert!(
                matches!(&error, SeleniumBaseError::Pdf(message) if message.contains("password")),
                "{wrong:?}: {error:?}"
            );
        }
    }

    #[test]
    fn a_password_given_for_a_pdf_that_needs_none_is_ignored() {
        let reader = PdfReader::from_bytes(&pdf_bytes(&["plain"]), Some("unused")).unwrap();
        assert!(reader.text().unwrap().contains("plain"));
    }

    /// Serves `body` once on a loopback port, and returns the URL.
    fn serve_once(status: &'static str, body: Vec<u8>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        format!("http://{address}/report.pdf")
    }

    #[tokio::test]
    async fn text_can_be_extracted_from_a_url() {
        let url = serve_once("200 OK", pdf_bytes(&["from the web"]));
        let text = extract_text_from_url(&url).await.unwrap();
        assert!(text.contains("from the web"));
    }

    #[tokio::test]
    async fn a_failing_download_is_a_download_error_naming_the_url() {
        let url = serve_once("404 Not Found", b"nothing".to_vec());
        let error = extract_text_from_url(&url).await.unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::Download { url: u, reason } if *u == url && reason.contains("404")),
            "{error:?}"
        );
    }
}
