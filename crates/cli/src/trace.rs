//! `sbrs trace show`: opens the viewer of a trace folder.
//!
//! A trace is the folder `Page::start_trace` writes. Its viewer, `index.html`, is written when the
//! recording stops and holds everything it shows, so opening it needs no server: the file goes to
//! the system's default browser.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The viewer's file name in a trace folder.
const VIEWER: &str = "index.html";
/// The record a trace folder holds; the viewer is written beside it when the recording stops.
const RECORD: &str = "trace.json";

/// Why a path leads to no trace viewer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NotAViewer {
    /// Nothing is at the path.
    Missing(PathBuf),
    /// A trace folder whose recording was not stopped, so it has no viewer yet.
    NotStopped(PathBuf),
    /// A folder or file that is not a trace.
    NotATrace(PathBuf),
}

impl fmt::Display for NotAViewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "{}: no such file or folder", path.display()),
            Self::NotStopped(path) => write!(
                f,
                "{} has {RECORD} but no {VIEWER}: the recording was not stopped, and the viewer \
                 is written when it stops",
                path.display()
            ),
            Self::NotATrace(path) => write!(
                f,
                "{} is not a trace folder: it has no {VIEWER} (a trace is the folder \
                 Page::start_trace writes)",
                path.display()
            ),
        }
    }
}

/// The viewer of the trace at `target`: the folder's `index.html`, or `target` itself when it
/// names that file.
pub(crate) fn viewer_file(target: &Path) -> Result<PathBuf, NotAViewer> {
    if target.is_file() {
        return if target.file_name().is_some_and(|name| name == VIEWER) {
            Ok(target.to_path_buf())
        } else {
            Err(NotAViewer::NotATrace(target.to_path_buf()))
        };
    }
    if !target.is_dir() {
        return Err(NotAViewer::Missing(target.to_path_buf()));
    }
    let viewer = target.join(VIEWER);
    if viewer.is_file() {
        Ok(viewer)
    } else if target.join(RECORD).is_file() {
        Err(NotAViewer::NotStopped(target.to_path_buf()))
    } else {
        Err(NotAViewer::NotATrace(target.to_path_buf()))
    }
}

/// The program that hands a file to the system's default browser.
fn opener_program() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    }
}

/// Runs `sbrs trace show`: prints the viewer's path when `print_only`, else opens it. Returns the
/// exit status: 2 when `target` leads to no viewer, 1 when the browser could not be started.
pub(crate) fn show(target: &Path, print_only: bool) -> i32 {
    let viewer = match viewer_file(target) {
        Ok(viewer) => std::path::absolute(&viewer).unwrap_or(viewer),
        Err(problem) => {
            eprintln!("error: {problem}");
            return 2;
        }
    };
    if print_only {
        println!("{}", viewer.display());
        return 0;
    }
    // The path is one argument of the program, never read by a shell.
    let program = opener_program();
    match Command::new(program).arg(&viewer).status() {
        // explorer exits with 1 even when it opened the file.
        Ok(status) if status.success() || cfg!(windows) => {
            println!("Opened {}", viewer.display());
            0
        }
        Ok(status) => {
            eprintln!(
                "error: {program} {status}; open {} yourself",
                viewer.display()
            );
            1
        }
        Err(error) => {
            eprintln!(
                "error: could not run {program} ({error}); open {} yourself",
                viewer.display()
            );
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder with the files `names`, each holding a little text.
    fn folder_with(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporary folder");
        for name in names {
            std::fs::write(dir.path().join(name), "{}").expect("write a file");
        }
        dir
    }

    #[test]
    fn a_stopped_trace_folder_leads_to_its_viewer() {
        let dir = folder_with(&[RECORD, VIEWER]);
        assert_eq!(viewer_file(dir.path()), Ok(dir.path().join(VIEWER)));
    }

    #[test]
    fn the_viewer_file_itself_is_accepted() {
        let dir = folder_with(&[RECORD, VIEWER]);
        let viewer = dir.path().join(VIEWER);
        assert_eq!(viewer_file(&viewer), Ok(viewer));
    }

    #[test]
    fn a_trace_that_was_not_stopped_is_named_as_such() {
        let dir = folder_with(&[RECORD]);
        let problem = viewer_file(dir.path()).expect_err("no viewer yet");
        assert_eq!(problem, NotAViewer::NotStopped(dir.path().to_path_buf()));
        assert!(problem.to_string().contains("not stopped"), "{problem}");
    }

    #[test]
    fn a_folder_without_a_trace_is_refused() {
        let dir = folder_with(&["notes.txt"]);
        assert_eq!(
            viewer_file(dir.path()),
            Err(NotAViewer::NotATrace(dir.path().to_path_buf()))
        );
    }

    #[test]
    fn another_file_is_refused() {
        let dir = folder_with(&["notes.html"]);
        let file = dir.path().join("notes.html");
        assert_eq!(viewer_file(&file), Err(NotAViewer::NotATrace(file.clone())));
    }

    #[test]
    fn a_missing_path_is_refused() {
        let dir = folder_with(&[]);
        let gone = dir.path().join("gone");
        assert_eq!(viewer_file(&gone), Err(NotAViewer::Missing(gone.clone())));
    }

    #[test]
    fn printing_opens_nothing_and_a_bad_folder_exits_with_2() {
        let dir = folder_with(&[RECORD, VIEWER]);
        assert_eq!(show(dir.path(), true), 0);
        assert_eq!(show(&dir.path().join("gone"), true), 2);
        assert_eq!(show(&dir.path().join("gone"), false), 2);
    }

    #[test]
    fn the_opener_is_the_system_one() {
        let expected = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        assert_eq!(opener_program(), expected);
    }
}
