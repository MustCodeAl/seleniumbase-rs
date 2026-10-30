//! `sbrs print`: show a file's text.

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

/// `text` with its line number in front of every line, as `sbrs print -n` shows it.
///
/// The numbers start at 1 and are right-aligned to the width of the last one, then a
/// space and the line. Every line of the result ends with a newline, also the last one,
/// and `\r\n` becomes `\n`. Text with no line gives an empty string.
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_print::number_lines;
///
/// assert_eq!(number_lines("a\nb\n"), "1 a\n2 b\n");
/// let ten: String = (1..=10).map(|n| format!("line {n}\n")).collect();
/// assert!(number_lines(&ten).starts_with(" 1 line 1\n 2 line 2\n"));
/// assert!(number_lines(&ten).ends_with("10 line 10\n"));
/// ```
#[must_use]
pub fn number_lines(text: &str) -> String {
    let width = text.lines().count().to_string().len();
    let mut numbered = String::with_capacity(text.len() + text.lines().count() * (width + 1));
    for (index, line) in text.lines().enumerate() {
        numbered.push_str(&format!("{:>width$} {line}\n", index + 1));
    }
    numbered
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

    #[test]
    fn numbers_start_at_one_and_line_up_to_the_width_of_the_last() {
        assert_eq!(number_lines("a\nb\n"), "1 a\n2 b\n");
        let hundred: String = (1..=100).map(|n| format!("l{n}\n")).collect();
        let numbered = number_lines(&hundred);
        assert!(numbered.starts_with("  1 l1\n  2 l2\n"), "{numbered}");
        assert!(numbered.contains("\n 10 l10\n"), "{numbered}");
        assert!(numbered.ends_with("100 l100\n"), "{numbered}");
    }

    #[test]
    fn a_missing_last_newline_is_added_and_empty_text_has_no_lines() {
        assert_eq!(number_lines("only"), "1 only\n");
        assert_eq!(number_lines(""), "");
        assert_eq!(number_lines("a\r\n\r\nb"), "1 a\n2 \n3 b\n");
    }
}
