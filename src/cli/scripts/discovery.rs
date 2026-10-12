//! Finding the files the terminal apps list: the discovery `sbase commander`
//! uses for tests and examples, and the Rust test files `sbase caseplans`
//! writes case plans for.

use std::io;
use std::path::{Path, PathBuf};

/// The `*.rs` files directly inside `dir`, sorted by name.
///
/// It does not descend into subfolders. The order of `read_dir` depends on
/// the file system, and a menu that reshuffles itself on every refresh is hard
/// to use, so the result is sorted. A folder that is not there has no files.
///
/// # Errors
///
/// Returns the error of reading `dir` or one of its entries.
pub fn rust_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The folders whose Rust files are the tests of the project at `root`:
/// `root/tests` and `root/examples`, those of them that exist.
///
/// A folder with neither is itself the place to look, so that
/// `sbase caseplans some/folder` works on any folder of test files.
#[must_use]
pub fn test_folders(root: &Path) -> Vec<PathBuf> {
    let project: Vec<PathBuf> = ["tests", "examples"]
        .into_iter()
        .map(|name| root.join(name))
        .filter(|folder| folder.is_dir())
        .collect();
    if project.is_empty() {
        vec![root.to_path_buf()]
    } else {
        project
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_rust_files_directly_in_the_folder_come_back_sorted() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["zeta.rs", "alpha.rs", "mid.rs", "notes.txt"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        // A folder that happens to be called x.rs, and a file below a folder.
        std::fs::create_dir(dir.path().join("sub.rs")).unwrap();
        std::fs::write(dir.path().join("sub.rs").join("inner.rs"), "").unwrap();

        let files = rust_files(dir.path()).unwrap();

        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["alpha.rs", "mid.rs", "zeta.rs"]);
    }

    #[test]
    fn a_folder_that_is_not_there_has_no_files() {
        let dir = tempfile::tempdir().unwrap();

        assert!(rust_files(&dir.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn a_project_is_searched_in_tests_and_examples_and_any_other_folder_in_itself() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(test_folders(dir.path()), [dir.path().to_path_buf()]);

        std::fs::create_dir(dir.path().join("examples")).unwrap();
        assert_eq!(test_folders(dir.path()), [dir.path().join("examples")]);

        std::fs::create_dir(dir.path().join("tests")).unwrap();
        assert_eq!(
            test_folders(dir.path()),
            [dir.path().join("tests"), dir.path().join("examples")]
        );
    }
}
