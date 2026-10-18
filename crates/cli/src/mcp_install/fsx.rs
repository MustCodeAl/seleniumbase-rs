//! File helpers: a backup before a change, and a write that cannot leave half a file behind.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// What the backup of a changed file ends with: `mcp.json` becomes `mcp.json.bak-sbrs-mcp`.
pub const BACKUP_SUFFIX: &str = ".bak-sbrs-mcp";

/// Where the backup of `path` goes.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(BACKUP_SUFFIX);
    path.with_file_name(name)
}

/// The file a write to `path` changes: the target of a symbolic link, so that a dotfile
/// manager's link stays a link, and `path` itself otherwise.
fn target_of(path: &Path) -> PathBuf {
    let is_link = fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());
    if is_link {
        fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

/// Replaces the file at `path` with `bytes`.
///
/// A file that exists is copied to [`backup_path`] first (the backup is overwritten by the
/// next change). The new content goes to a temporary file in the same folder, is flushed to
/// disk, takes over the permissions of the file it replaces, and is renamed over it, so a
/// reader sees the old file or the new one and never a part of either. Missing folders are
/// created.
///
/// # Errors
///
/// The I/O error that stopped the change; the original file is then as it was.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let target = target_of(path);
    let dir = match target.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&dir)?;
    let existing = fs::metadata(&target).ok();
    if existing.is_some() {
        fs::copy(&target, backup_path(&target))?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if let Some(meta) = existing {
        fs::set_permissions(temp.path(), meta.permissions())?;
    }
    temp.persist(&target).map_err(|e| e.error)?;
    sync_dir(&dir);
    Ok(())
}

/// Flushes the folder entry of a renamed file, where the system lets a folder be flushed.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = fs::File::open(dir) {
        // A folder that cannot be flushed leaves the rename as durable as the system makes it.
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

/// Whether `text` has a `//` or `/* */` comment outside its strings, which makes it JSONC.
pub fn has_comments(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if in_string {
            if byte == b'\\' {
                i += 1;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'/' && matches!(bytes.get(i + 1), Some(b'/' | b'*')) {
            return true;
        }
        i += 1;
    }
    false
}

/// The indent of the first indented line of `text`: a tab or a run of spaces. Two spaces when
/// no line is indented.
pub fn detect_indent(text: &str) -> String {
    for line in text.lines() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if !trimmed.is_empty() && trimmed.len() < line.len() {
            return line[..line.len() - trimmed.len()].to_owned();
        }
    }
    "  ".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_are_found_only_outside_strings() {
        assert!(has_comments("{ // note\n}"));
        assert!(has_comments("{ /* note */ }"));
        assert!(!has_comments(r#"{ "url": "http://example.test/a//b" }"#));
        assert!(!has_comments(r#"{ "s": "quote \" // still inside" }"#));
        assert!(has_comments(
            "{\"a\": \"x\\\\\" // after an escaped backslash\n}"
        ));
        assert!(!has_comments("{}"));
    }

    #[test]
    fn indent_is_the_first_indented_lines() {
        assert_eq!(detect_indent("{\n    \"a\": 1\n}"), "    ");
        assert_eq!(detect_indent("{\n\t\"a\": 1\n}"), "\t");
        assert_eq!(detect_indent("{\"a\": 1}"), "  ");
        assert_eq!(detect_indent(""), "  ");
    }

    #[test]
    fn backup_sits_beside_the_file() {
        assert_eq!(
            backup_path(Path::new("/h/.cursor/mcp.json")),
            Path::new("/h/.cursor/mcp.json.bak-sbrs-mcp")
        );
    }

    #[test]
    fn replace_creates_folders_and_backs_up_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a").join("b").join("c.json");
        replace(&file, b"one").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"one");
        assert!(!backup_path(&file).exists(), "a new file has no backup");
        replace(&file, b"two").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"two");
        assert_eq!(fs::read(backup_path(&file)).unwrap(), b"one");
    }

    #[cfg(unix)]
    #[test]
    fn replace_keeps_the_permissions_of_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("c.json");
        fs::write(&file, "one").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        replace(&file, b"two").unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn replace_writes_through_a_symbolic_link() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        let link = dir.path().join("link.json");
        fs::write(&real, "one").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        replace(&link, b"two").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"two");
    }
}
