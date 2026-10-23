//! The crate carries no deprecated items.
//!
//! Breaking changes are fine here, so a renamed or replaced item is deleted, not
//! kept behind `#[deprecated]`. This test walks the sources of the library and of
//! every package under `crates/` and fails, naming `path:line`, for any line that
//! marks something deprecated or silences the lint with `allow`. Test files and
//! the change logs are not scanned.
//!
//! A deprecation that comes from a dependency is not ours to delete. Name it with
//! `expect(deprecated, reason = "..")` instead: the reason says which dependency,
//! and the compiler warns when the expectation is no longer needed.

use std::fs;
use std::path::{Path, PathBuf};

/// Spellings that mark an item deprecated.
const MARKERS: &[&str] = &["deprecated(", "#[deprecated", "#![deprecated"];

/// The attribute that silences the lint, whatever its reason and layout.
const SILENCER: &str = "allow(deprecated";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn sources() -> Vec<PathBuf> {
    let root = root();
    let mut files = Vec::new();
    rust_files_under(&root.join("src"), &mut files);
    if let Ok(packages) = fs::read_dir(root.join("crates")) {
        for package in packages.flatten() {
            rust_files_under(&package.path().join("src"), &mut files);
        }
    }
    files.sort();
    files
}

/// The lines of `text` that mark something deprecated or silence the lint.
///
/// `#[allow(` may be split over lines by rustfmt, so a line that opens it is
/// read together with the two lines after it, without their whitespace.
fn hits(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            if MARKERS.iter().any(|marker| line.contains(marker)) {
                return true;
            }
            if !line.contains("allow(") {
                return false;
            }
            let window: String = lines[*index..lines.len().min(index + 3)]
                .iter()
                .flat_map(|l| l.split_whitespace())
                .collect();
            window.contains(SILENCER)
        })
        .map(|(index, line)| (index + 1, line.trim()))
        .collect()
}

#[test]
fn no_source_marks_anything_deprecated() {
    let root = root();
    let mut found = Vec::new();
    for file in sources() {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file).unwrap_or_default();
        for (number, line) in hits(&text) {
            found.push(format!("{rel}:{number}: {line}"));
        }
    }
    assert!(
        found.is_empty(),
        "{} deprecated marker(s) in the sources; delete the item instead:\n{}",
        found.len(),
        found.join("\n")
    );
}

#[test]
fn the_scan_sees_the_library_and_the_cli_package() {
    let files = sources();
    let root = root();
    let has = |suffix: &str| files.iter().any(|f| f.starts_with(root.join(suffix)));
    assert!(has("src/api"), "src/api is not scanned");
    assert!(has("crates/cli/src"), "crates/cli/src is not scanned");
    assert!(files.len() > 100, "only {} files scanned", files.len());
}

#[test]
fn the_scan_catches_each_spelling() {
    assert_eq!(hits("#[deprecated]\nfn a() {}").len(), 1);
    assert_eq!(hits("#[deprecated(since = \"1\")]").len(), 1);
    assert_eq!(hits("#![allow(deprecated)]").len(), 1);
    assert_eq!(hits("#[allow(deprecated, reason = \"x\")]").len(), 1);
    assert_eq!(
        hits("#[allow(\n    deprecated,\n    reason = \"x\"\n)]").len(),
        1
    );
    assert_eq!(
        hits("#[expect(deprecated, reason = \"thirtyfour\")]").len(),
        0
    );
    assert_eq!(
        hits("#[allow(dead_code)]\nfn deprecated_name() {}").len(),
        0
    );
    assert_eq!(hits("fn fine() {}\n// no marker here").len(), 0);
}
