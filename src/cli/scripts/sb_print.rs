//! `sbase print`: show a file's text.

use std::fs;

/// Reads `filename` as text to show on the terminal.
///
/// Bytes that are not valid UTF-8 (a Latin-1 source file, a log with one stray
/// byte) become `U+FFFD` instead of failing the whole file: the point is to
/// see it. Errors from opening or reading the file are returned unchanged.
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_print::print_file;
///
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("note.txt");
/// std::fs::write(&path, b"caf\xe9").unwrap();
/// assert_eq!(print_file(path.to_str().unwrap()).unwrap(), "caf\u{fffd}");
/// ```
pub fn print_file(filename: &str) -> std::io::Result<String> {
    let bytes = fs::read(filename)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_text_is_returned_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "héllo\nwörld\n").unwrap();

        assert_eq!(
            print_file(path.to_str().unwrap()).unwrap(),
            "héllo\nwörld\n"
        );
    }

    #[test]
    fn a_missing_file_is_a_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.txt");

        let error = print_file(missing.to_str().unwrap()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
