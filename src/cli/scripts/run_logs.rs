//! What `sbrs test` does with the folders of the last run before a new one.
//!
//! Python SeleniumBase empties `latest_logs/` at the start of a run, or moves
//! it to `archived_logs/logs_<epoch>/` with `--archive-logs`, and does the
//! same for `downloaded_files/` and `archived_files/downloads_<epoch>/` with
//! `--archive-downloads`. Plain `cargo test` never touches them: only
//! `sbrs test` does, in the folder of each package the tests run in (a test's
//! working directory is its package's), and never for `--co`.
//!
//! A folder that does not exist, is empty or is a symbolic link is left alone.
//! Emptying removes what is inside the folder and keeps the folder.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::run_logs::{reset_logs, Reset};
//!
//! let dir = tempfile::tempdir()?;
//! let logs = dir.path().join("latest_logs");
//! std::fs::create_dir(&logs)?;
//! std::fs::write(logs.join("old.txt"), "old")?;
//!
//! let done = reset_logs(dir.path(), true, 1_700_000_000)?;
//! assert_eq!(
//!     done,
//!     Reset::Archived(dir.path().join("archived_logs/logs_1700000000"))
//! );
//! assert!(dir.path().join("archived_logs/logs_1700000000/old.txt").is_file());
//! assert!(std::fs::read_dir(&logs)?.next().is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The folder the tests write their logs and failure evidence in.
pub const LATEST_LOGS: &str = "latest_logs";
/// Where `--archive-logs` keeps the old logs.
pub const ARCHIVED_LOGS: &str = "archived_logs";
/// The folder the browsers save downloads in.
pub const DOWNLOADED_FILES: &str = "downloaded_files";
/// Where `--archive-downloads` keeps the old downloads.
pub const ARCHIVED_FILES: &str = "archived_files";

/// What happened to one folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reset {
    /// There was no such folder (or it is a symbolic link): nothing was done.
    Missing,
    /// The folder had nothing in it: nothing was done.
    Empty,
    /// The contents were removed; the folder stays.
    Cleared,
    /// The folder was moved here and an empty one made in its place.
    Archived(PathBuf),
}

/// The seconds since the Unix epoch, which name an archive.
#[must_use]
pub fn unix_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn is_real_folder(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn empty_folder(path: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        // `file_type` does not follow a link, so a link is removed, not what it names.
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// A name for the archive that no earlier archive has: `base`, else `base-1`, ...
fn free_name(parent: &Path, base: &str) -> PathBuf {
    let first = parent.join(base);
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| parent.join(format!("{base}-{n}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

fn reset(
    root: &Path,
    folder: &str,
    archive_folder: &str,
    prefix: &str,
    archive: bool,
    stamp: u64,
) -> io::Result<Reset> {
    let path = root.join(folder);
    if !is_real_folder(&path) {
        return Ok(Reset::Missing);
    }
    if std::fs::read_dir(&path)?.next().is_none() {
        return Ok(Reset::Empty);
    }
    if !archive {
        empty_folder(&path)?;
        return Ok(Reset::Cleared);
    }
    let archive_root = root.join(archive_folder);
    std::fs::create_dir_all(&archive_root)?;
    let target = free_name(&archive_root, &format!("{prefix}_{stamp}"));
    std::fs::rename(&path, &target)?;
    std::fs::create_dir(&path)?;
    Ok(Reset::Archived(target))
}

/// Archives (`archive`) or empties the `latest_logs/` folder of `root`.
///
/// # Errors
///
/// Returns the I/O error of the move or the removal.
pub fn reset_logs(root: &Path, archive: bool, stamp: u64) -> io::Result<Reset> {
    reset(root, LATEST_LOGS, ARCHIVED_LOGS, "logs", archive, stamp)
}

/// Archives (`archive`) or empties the `downloaded_files/` folder of `root`.
/// The operating system's own Downloads folder is never touched.
///
/// # Errors
///
/// Returns the I/O error of the move or the removal.
pub fn reset_downloads(root: &Path, archive: bool, stamp: u64) -> io::Result<Reset> {
    reset(
        root,
        DOWNLOADED_FILES,
        ARCHIVED_FILES,
        "downloads",
        archive,
        stamp,
    )
}

/// The folders of the packages in `metadata` (the output of
/// `cargo metadata --no-deps --format-version 1`) that `cargo test` run from
/// `cwd` selects: the package `cwd` is in, else the workspace's default
/// members. `None` when the text is not cargo's metadata.
#[must_use]
pub fn package_dirs_from_metadata(metadata: &str, cwd: &Path) -> Option<Vec<PathBuf>> {
    let value: serde_json::Value = serde_json::from_str(metadata).ok()?;
    let packages = value.get("packages")?.as_array()?;
    let dir_of = |package: &serde_json::Value| {
        let manifest = package.get("manifest_path")?.as_str()?;
        Path::new(manifest).parent().map(Path::to_path_buf)
    };
    let containing = packages
        .iter()
        .filter_map(|package| dir_of(package))
        .filter(|dir| cwd.starts_with(dir))
        .max_by_key(|dir| dir.components().count());
    if let Some(dir) = containing {
        return Some(vec![dir]);
    }
    let selected = value
        .get("workspace_default_members")
        .or_else(|| value.get("workspace_members"))?
        .as_array()?;
    let dirs: Vec<PathBuf> = packages
        .iter()
        .filter(|package| {
            package
                .get("id")
                .is_some_and(|id| selected.iter().any(|member| member == id))
        })
        .filter_map(dir_of)
        .collect();
    (!dirs.is_empty()).then_some(dirs)
}

/// The folders `sbrs test` clears, found with `cargo metadata` run in `cwd`;
/// `cwd` itself when cargo cannot say.
#[must_use]
pub fn package_dirs(cwd: &Path) -> Vec<PathBuf> {
    let output = std::process::Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(cwd)
        .stderr(std::process::Stdio::null())
        .output();
    output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| package_dirs_from_metadata(&text, cwd))
        .unwrap_or_else(|| vec![cwd.to_path_buf()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with_a_file(root: &Path, name: &str) -> PathBuf {
        let folder = root.join(name);
        std::fs::create_dir_all(folder.join("deep")).unwrap();
        std::fs::write(folder.join("a.txt"), "a").unwrap();
        std::fs::write(folder.join("deep/b.txt"), "b").unwrap();
        folder
    }

    #[test]
    fn without_archiving_the_contents_go_and_the_folder_stays() {
        let dir = tempfile::tempdir().unwrap();
        let logs = folder_with_a_file(dir.path(), LATEST_LOGS);
        assert_eq!(reset_logs(dir.path(), false, 5).unwrap(), Reset::Cleared);
        assert!(logs.is_dir());
        assert!(std::fs::read_dir(&logs).unwrap().next().is_none());
        assert!(!dir.path().join(ARCHIVED_LOGS).exists());
    }

    #[test]
    fn archiving_moves_the_folder_and_leaves_an_empty_one() {
        let dir = tempfile::tempdir().unwrap();
        let logs = folder_with_a_file(dir.path(), LATEST_LOGS);
        let done = reset_logs(dir.path(), true, 42).unwrap();
        let archive = dir.path().join("archived_logs/logs_42");
        assert_eq!(done, Reset::Archived(archive.clone()));
        assert!(archive.join("a.txt").is_file());
        assert!(archive.join("deep/b.txt").is_file());
        assert!(logs.is_dir() && std::fs::read_dir(&logs).unwrap().next().is_none());
    }

    #[test]
    fn a_second_archive_in_the_same_second_does_not_overwrite_the_first() {
        let dir = tempfile::tempdir().unwrap();
        folder_with_a_file(dir.path(), LATEST_LOGS);
        reset_logs(dir.path(), true, 7).unwrap();
        folder_with_a_file(dir.path(), LATEST_LOGS);
        let done = reset_logs(dir.path(), true, 7).unwrap();
        assert_eq!(
            done,
            Reset::Archived(dir.path().join("archived_logs/logs_7-1"))
        );
        assert!(dir.path().join("archived_logs/logs_7/a.txt").is_file());
    }

    #[test]
    fn a_missing_or_empty_folder_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(reset_logs(dir.path(), true, 1).unwrap(), Reset::Missing);
        assert!(!dir.path().join(ARCHIVED_LOGS).exists());
        std::fs::create_dir(dir.path().join(LATEST_LOGS)).unwrap();
        assert_eq!(reset_logs(dir.path(), true, 1).unwrap(), Reset::Empty);
        assert_eq!(reset_logs(dir.path(), false, 1).unwrap(), Reset::Empty);
        assert!(!dir.path().join(ARCHIVED_LOGS).exists());
    }

    #[test]
    fn downloads_have_their_own_folders() {
        let dir = tempfile::tempdir().unwrap();
        folder_with_a_file(dir.path(), DOWNLOADED_FILES);
        let done = reset_downloads(dir.path(), true, 9).unwrap();
        assert_eq!(
            done,
            Reset::Archived(dir.path().join("archived_files/downloads_9"))
        );
        folder_with_a_file(dir.path(), DOWNLOADED_FILES);
        assert_eq!(
            reset_downloads(dir.path(), false, 9).unwrap(),
            Reset::Cleared
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = folder_with_a_file(dir.path(), "elsewhere");
        std::os::unix::fs::symlink(&elsewhere, dir.path().join(LATEST_LOGS)).unwrap();
        assert_eq!(reset_logs(dir.path(), false, 1).unwrap(), Reset::Missing);
        assert!(elsewhere.join("a.txt").is_file());

        // A link inside the folder is removed, not what it names.
        let real = dir.path().join("real");
        let logs = dir.path().join("logs_dir");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("keep.txt"), "k").unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::os::unix::fs::symlink(&real, logs.join("link")).unwrap();
        empty_folder(&logs).unwrap();
        assert!(real.join("keep.txt").is_file());
        assert!(std::fs::read_dir(&logs).unwrap().next().is_none());
    }

    #[test]
    fn the_package_in_the_working_directory_is_the_one_selected() {
        let metadata = r#"{
            "packages": [
                {"id": "root 0.1.0", "manifest_path": "/w/Cargo.toml"},
                {"id": "cli 0.1.0", "manifest_path": "/w/crates/cli/Cargo.toml"}
            ],
            "workspace_members": ["root 0.1.0", "cli 0.1.0"],
            "workspace_default_members": ["root 0.1.0"]
        }"#;
        assert_eq!(
            package_dirs_from_metadata(metadata, Path::new("/w/crates/cli/src")),
            Some(vec![PathBuf::from("/w/crates/cli")])
        );
        assert_eq!(
            package_dirs_from_metadata(metadata, Path::new("/w")),
            Some(vec![PathBuf::from("/w")])
        );
        assert_eq!(
            package_dirs_from_metadata(metadata, Path::new("/elsewhere")),
            Some(vec![PathBuf::from("/w")])
        );
        assert_eq!(
            package_dirs_from_metadata("nonsense", Path::new("/w")),
            None
        );
    }
}
