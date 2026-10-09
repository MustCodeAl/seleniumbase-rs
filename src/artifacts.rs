//! Where the files a test run produces go, and how they are named.
//!
//! Screenshots, page sources, recordings and reports are written to the
//! `latest_logs` folder in the working directory, the same place Python
//! SeleniumBase uses. Two things here keep that safe:
//!
//! - [`bare_file_name`] and [`confined_path`] turn a name a caller (or a test
//!   author, or an MCP client) supplied into a path *inside* the folder. A name
//!   with a directory part, a `..` or a drive letter is refused rather than
//!   resolved, so `../../etc/cron.d/x` can never leave the folder.
//! - [`artifact_path`] and [`write_new_file`] pick names the framework chooses
//!   itself. They never return the name of a file that exists, so two
//!   screenshots taken in the same millisecond keep both.
//!
//! A name the *caller* chose, such as `save_screenshot("login.png")`, does
//! replace an earlier file of that name. That is what Python does, and what
//! re-running a test expects.
//!
//! # Examples
//!
//! ```
//! use std::path::Path;
//! use seleniumbase_rs::artifacts::{artifact_path, confined_path};
//!
//! let dir = Path::new("latest_logs");
//! assert_eq!(confined_path(dir, "login.png")?, dir.join("login.png"));
//! assert!(confined_path(dir, "../login.png").is_err());
//!
//! // The prefix is cleaned up too.
//! let path = artifact_path(dir, "../shot", "png");
//! assert_eq!(path.parent(), Some(dir));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::SeleniumBaseError;

/// The folder artifacts are written to, relative to the working directory.
pub const LATEST_LOGS_DIR: &str = "latest_logs";

/// The most characters [`safe_stem`] keeps.
const MAX_STEM_LEN: usize = 80;

/// How many names [`write_new_file`] tries before it gives up.
const MAX_NAME_ATTEMPTS: u32 = 1000;

/// Creates the `latest_logs` folder if it is missing and returns its path.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder cannot be created, for
/// example because `latest_logs` is a file or the directory is read-only.
pub fn ensure_latest_logs_dir() -> Result<PathBuf, SeleniumBaseError> {
    let path = PathBuf::from(LATEST_LOGS_DIR);
    ensure_dir(&path)?;
    Ok(path)
}

fn ensure_dir(path: &Path) -> Result<(), SeleniumBaseError> {
    fs::create_dir_all(path).map_err(|error| {
        SeleniumBaseError::Io(io::Error::new(
            error.kind(),
            format!("failed to create the {} directory: {error}", path.display()),
        ))
    })
}

/// `name` as a path, if it is one plain file name: no directory part, no
/// `..`, not absolute, no drive prefix.
///
/// Both `/` and `\` count as separators on every platform, so a name that is
/// harmless on Unix but would climb out of the folder on Windows is refused
/// everywhere.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] naming the offending value.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::artifacts::bare_file_name;
///
/// assert!(bare_file_name("login-failure_1.png").is_ok());
/// for bad in ["", "..", "../x.png", "a/b.png", "/etc/passwd", r"..\x.png"] {
///     assert!(bare_file_name(bad).is_err(), "{bad}");
/// }
/// ```
pub fn bare_file_name(name: &str) -> Result<&Path, SeleniumBaseError> {
    let path = Path::new(name);
    let mut parts = path.components();
    let one_plain_part = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    );
    if one_plain_part && !name.contains(['/', '\\', '\0']) {
        Ok(path)
    } else {
        Err(SeleniumBaseError::InvalidConfig(format!(
            "a file name here must be a plain name with no directory part, not {name:?}"
        )))
    }
}

/// `dir` joined with `name`, if `name` is a [bare file name](bare_file_name).
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not a bare name.
pub fn confined_path(dir: &Path, name: &str) -> Result<PathBuf, SeleniumBaseError> {
    Ok(dir.join(bare_file_name(name)?))
}

/// `raw` with every character except ASCII letters, digits, `-` and `_`
/// replaced by `_`.
fn replace_unsafe(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A file-name stem made from `raw`: letters, digits, `-` and `_` only, at
/// most 80 characters, and `fallback` if nothing is left.
///
/// A test called `../../etc/passwd` becomes `______etc_passwd`, so it cannot
/// name a file outside the folder it is written to.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::artifacts::safe_stem;
///
/// assert_eq!(safe_stem("login works!", "test"), "login_works_");
/// assert_eq!(safe_stem("", "test"), "test");
/// ```
#[must_use]
pub fn safe_stem(raw: &str, fallback: &str) -> String {
    let mut stem: String = replace_unsafe(raw).chars().take(MAX_STEM_LEN).collect();
    if stem.is_empty() {
        stem.push_str(fallback);
    }
    stem
}

/// The ASCII letters and digits of `extension`, so `.png`, `p/ng` and `png`
/// all come out harmless. May be empty.
fn safe_extension(extension: &str) -> String {
    extension
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// `stem` plus `.extension`, or just `stem` if the extension is empty.
fn file_name(stem: &str, extension: &str) -> String {
    if extension.is_empty() {
        stem.to_owned()
    } else {
        format!("{stem}.{extension}")
    }
}

/// A new path in `dir` named from `prefix` and the time, for a file the
/// framework names itself.
///
/// The path is `{prefix}_{milliseconds}.{extension}`, with a `_{n}` suffix when
/// another artifact was named in the same millisecond. It is never the path of
/// a file that already exists, and never leaves `dir`: `prefix` is reduced to
/// letters, digits, `-` and `_`, and `extension` to letters and digits.
///
/// Nothing is created. The name is unique among the calls made by this
/// process; a caller that must not lose a race with another process should
/// write with [`write_new_file`] instead.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use seleniumbase_rs::artifacts::artifact_path;
///
/// let dir = Path::new("latest_logs");
/// let first = artifact_path(dir, "screenshot", "png");
/// let second = artifact_path(dir, "screenshot", "png");
/// assert_ne!(first, second);
/// assert_eq!(first.extension().and_then(|e| e.to_str()), Some("png"));
/// ```
#[must_use]
pub fn artifact_path(dir: &Path, prefix: &str, extension: &str) -> PathBuf {
    pick_unused_path(dir, prefix, extension, next_stamp)
}

fn pick_unused_path(
    dir: &Path,
    prefix: &str,
    extension: &str,
    mut stamp: impl FnMut() -> (u128, u32),
) -> PathBuf {
    let prefix = safe_stem(prefix, "artifact");
    let extension = safe_extension(extension);
    loop {
        let (millis, sequence) = stamp();
        let stem = if sequence == 0 {
            format!("{prefix}_{millis}")
        } else {
            format!("{prefix}_{millis}_{sequence}")
        };
        let path = dir.join(file_name(&stem, &extension));
        if !path.exists() {
            return path;
        }
    }
}

/// The current time in milliseconds and a number that is different for every
/// call within the same millisecond, even if the clock steps backwards.
fn next_stamp() -> (u128, u32) {
    static LAST: Mutex<(u128, u32)> = Mutex::new((0, 0));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let mut last = LAST.lock().unwrap_or_else(PoisonError::into_inner);
    let millis = now.max(last.0);
    let sequence = if millis == last.0 { last.1 + 1 } else { 0 };
    *last = (millis, sequence);
    (millis, sequence)
}

/// Writes `bytes` to a new file in `dir` named `{stem}.{extension}` and
/// returns its path, creating `dir` if needed.
///
/// If that name is taken the file becomes `{stem}_1.{extension}`, then `_2`,
/// and so on: an existing file is never replaced, and the check and the
/// creation are one atomic step, so concurrent writers cannot clobber each
/// other. `stem` and `extension` are cleaned the way [`artifact_path`] cleans
/// its arguments.
///
/// # Errors
///
/// Returns an I/O error if `dir` or the file cannot be created or written, or
/// [`SeleniumBaseError::InvalidConfig`] if a thousand names are taken.
pub async fn write_new_file(
    dir: &Path,
    stem: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, SeleniumBaseError> {
    use tokio::io::AsyncWriteExt;

    tokio::fs::create_dir_all(dir).await?;
    let stem = replace_unsafe(stem);
    let extension = safe_extension(extension);
    for attempt in 0..MAX_NAME_ATTEMPTS {
        let name = if attempt == 0 {
            file_name(&stem, &extension)
        } else {
            file_name(&format!("{stem}_{attempt}"), &extension)
        };
        let path = dir.join(name);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(mut file) => {
                file.write_all(bytes).await?;
                file.flush().await?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(SeleniumBaseError::InvalidConfig(format!(
        "more than {MAX_NAME_ATTEMPTS} files named like {} in {}",
        file_name(&stem, &extension),
        dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn a_plain_file_name_is_accepted() {
        for ok in [
            "shot.png",
            "login-failure_1.png",
            "no_extension",
            ".hidden",
            "a b.png",
            "тест.png",
            "日本語.png",
            "with:colon.png",
        ] {
            assert_eq!(bare_file_name(ok).unwrap(), Path::new(ok), "{ok}");
        }
    }

    #[test]
    fn a_name_that_could_leave_the_folder_is_refused() {
        for bad in [
            "",
            ".",
            "..",
            "../x.png",
            "../../etc/passwd",
            "a/b.png",
            "a/",
            "/",
            "/etc/passwd",
            "./x.png",
            r"..\x.png",
            r"a\b.png",
            r"C:\Windows\x.png",
            r"\\server\share\x.png",
            "x\0y.png",
        ] {
            assert!(bare_file_name(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn a_refused_name_is_quoted_in_the_error() {
        let message = bare_file_name("../x").unwrap_err().to_string();
        assert!(message.contains(r#""../x""#), "{message}");
    }

    #[test]
    fn a_confined_path_stays_in_its_folder() {
        let dir = Path::new("logs");
        assert_eq!(confined_path(dir, "a.png").unwrap(), dir.join("a.png"));
        assert!(confined_path(dir, "../a.png").is_err());
        assert!(confined_path(dir, "/tmp/a.png").is_err());
    }

    #[test]
    fn a_stem_is_letters_digits_dash_and_underscore() {
        assert_eq!(safe_stem("login_works", "x"), "login_works");
        assert_eq!(safe_stem("tests::login works!", "x"), "tests__login_works_");
        assert_eq!(safe_stem("../../etc/passwd", "x"), "______etc_passwd");
        assert_eq!(safe_stem("", "fallback"), "fallback");
        assert_eq!(safe_stem(&"x".repeat(500), "x").len(), MAX_STEM_LEN);
        assert!(!safe_stem(r"a/b\c:d", "x").contains(['/', '\\', ':']));
    }

    #[test]
    fn an_extension_loses_everything_but_letters_and_digits() {
        assert_eq!(safe_extension("png"), "png");
        assert_eq!(safe_extension(".png"), "png");
        assert_eq!(safe_extension("p/../ng"), "png");
        assert_eq!(safe_extension(""), "");
        assert_eq!(file_name("shot", ""), "shot");
        assert_eq!(file_name("shot", "png"), "shot.png");
    }

    #[test]
    fn an_artifact_path_stays_in_its_folder_whatever_the_prefix() {
        let dir = Path::new("logs");
        for prefix in ["../../etc/x", "/abs/path", r"..\x", "", "a/b"] {
            let path = artifact_path(dir, prefix, "png");
            assert_eq!(path.parent(), Some(dir), "{prefix:?} -> {path:?}");
            assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        }
        let path = artifact_path(dir, "shot", "../../x");
        assert_eq!(path.parent(), Some(dir));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("x"));
    }

    #[test]
    fn artifact_paths_made_in_the_same_millisecond_differ() {
        let dir = Path::new("logs");
        let paths: HashSet<_> = (0..2000)
            .map(|_| artifact_path(dir, "screenshot", "png"))
            .collect();
        assert_eq!(paths.len(), 2000);
    }

    #[test]
    fn an_artifact_path_skips_a_file_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot_5.png"), "old").unwrap();
        std::fs::write(dir.path().join("shot_5_1.png"), "old").unwrap();
        let mut stamps = [(5, 0), (5, 1), (5, 2), (5, 3)].into_iter();

        let path = pick_unused_path(dir.path(), "shot", "png", || stamps.next().unwrap());

        assert_eq!(path, dir.path().join("shot_5_2.png"));
    }

    #[test]
    fn the_stamp_never_repeats_even_if_the_clock_goes_back() {
        let stamps: Vec<_> = (0..100).map(|_| next_stamp()).collect();
        let distinct: HashSet<_> = stamps.iter().collect();
        assert_eq!(distinct.len(), stamps.len());
        assert!(stamps.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[test]
    fn a_missing_folder_is_created_with_its_parents() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("a").join("b");

        ensure_dir(&nested).unwrap();
        ensure_dir(&nested).unwrap();

        assert!(nested.is_dir());
    }

    #[test]
    fn a_folder_that_cannot_be_created_says_which() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        std::fs::write(&file, "x").unwrap();

        let error = ensure_dir(&file.join("logs")).unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Io(_)), "{error:?}");
        assert!(error.to_string().contains("failed to create"), "{error}");
    }

    #[tokio::test]
    async fn written_files_never_replace_each_other() {
        let dir = tempfile::tempdir().unwrap();

        let first = write_new_file(dir.path(), "t", "png", b"one")
            .await
            .unwrap();
        let second = write_new_file(dir.path(), "t", "png", b"two")
            .await
            .unwrap();

        assert_eq!(first, dir.path().join("t.png"));
        assert_eq!(second, dir.path().join("t_1.png"));
        assert_eq!(std::fs::read(first).unwrap(), b"one");
        assert_eq!(std::fs::read(second).unwrap(), b"two");
    }

    #[tokio::test]
    async fn a_written_file_cannot_leave_its_folder() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");

        let path = write_new_file(&out, "../../escaped", "p/../ng", b"x")
            .await
            .unwrap();

        assert_eq!(path.parent(), Some(out.as_path()));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn the_folder_is_created_when_it_is_missing() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("a").join("b");

        let path = write_new_file(&nested, "t", "html", b"x").await.unwrap();

        assert!(path.starts_with(&nested));
    }
}
