//! `sbase objectify`: a starter page object.

use crate::cli::commands::write_new_file;
use std::path::{Path, PathBuf};

/// The file `sbase objectify` writes, in the current folder.
pub const FILE_NAME: &str = "page_object.rs";

/// The page object. It lives in its own file so a test can compile it against
/// the real `BaseCase` API.
const PAGE_OBJECT: &str = include_str!("templates/page_object.tpl");

/// Writes the starter page object to `page_object.rs` in the current folder.
///
/// An existing `page_object.rs` is never replaced: that is an
/// [`std::io::ErrorKind::AlreadyExists`] error.
pub fn objectify_page() -> std::io::Result<PathBuf> {
    let path = PathBuf::from(FILE_NAME);
    write_page_object(&path)?;
    Ok(path)
}

/// Writes the starter page object to `path`, which must not exist yet.
pub fn write_page_object(path: &Path) -> std::io::Result<()> {
    write_new_file(path, PAGE_OBJECT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_object_is_valid_rust_and_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);

        write_page_object(&path).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        syn::parse_file(&source).expect("the template parses as Rust");
        std::fs::write(&path, "// edited").unwrap();
        let error = write_page_object(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "// edited");
    }
}
