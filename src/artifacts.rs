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
    let path = logs_root().join(LATEST_LOGS_DIR);
    ensure_dir(&path)?;
    Ok(path)
}

/// The variable that names the folder a run's files go under.
pub const REPORT_ROOT_VARIABLE: &str = "SB_REPORT_ROOT";

/// The folder where manual-QA reports are made, under [`logs_root`].
pub const LATEST_REPORT_DIR: &str = "latest_report";

/// The folder where finished manual-QA reports are kept, under [`logs_root`].
pub const ARCHIVED_REPORTS_DIR: &str = "archived_reports";

/// The folder the files of a run go under: `SB_REPORT_ROOT`, or the empty
/// path (the current folder) when it is unset or blank.
///
/// `sbrs test` sets the variable to its own working directory, because cargo
/// runs the tests of each package from that package's folder.
#[must_use]
pub fn logs_root() -> PathBuf {
    std::env::var(REPORT_ROOT_VARIABLE)
        .ok()
        .filter(|root| !root.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// The folders of the logs of the tests that are being recorded, by the thread
/// that started each.
static ACTIVE_TEST_LOGS: Mutex<Vec<(std::thread::ThreadId, PathBuf)>> = Mutex::new(Vec::new());

/// Makes `dir` the folder [`ensure_test_logs_dir`] answers with on the calling
/// thread, and returns the thread to hand to [`deactivate_test_logs`].
pub(crate) fn activate_test_logs(dir: &Path) -> std::thread::ThreadId {
    let thread = std::thread::current().id();
    ACTIVE_TEST_LOGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((thread, dir.to_path_buf()));
    thread
}

/// Undoes [`activate_test_logs`]. It may run on any thread.
pub(crate) fn deactivate_test_logs(thread: std::thread::ThreadId, dir: &Path) {
    let mut active = ACTIVE_TEST_LOGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(index) = active
        .iter()
        .position(|(owner, folder)| *owner == thread && folder == dir)
    {
        active.remove(index);
    }
}

/// Creates the folder for the files of the test running on this thread and
/// returns its path: `latest_logs/<test id>` while a
/// [`TestRecorder`](crate::core::test_run::TestRecorder) is recording it, and
/// [`ensure_latest_logs_dir`] otherwise.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder cannot be created.
pub fn ensure_test_logs_dir() -> Result<PathBuf, SeleniumBaseError> {
    ensure_active_or_latest(active_test_logs(None))
}

/// Creates the folder for the files of the test called `test_id` and returns
/// its path, as [`ensure_test_logs_dir`] does for the test on this thread.
///
/// A recorder may be recording the test on another thread than the one that
/// asks: a Gherkin scenario's recorder runs on the scenario's own thread, and
/// the steps run elsewhere. So the folder of a recorded test whose id is
/// `test_id` is found on any thread (the one on this thread first); with no such
/// test, the folder of the test recorded on this thread is the answer, and
/// [`ensure_latest_logs_dir`] when there is none.
///
/// The id is the last part of the folder's path, which is what a
/// [`TestRecorder`](crate::core::test_run::TestRecorder) names it after.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder cannot be created.
pub fn ensure_test_logs_dir_for(test_id: &str) -> Result<PathBuf, SeleniumBaseError> {
    ensure_active_or_latest(active_test_logs(Some(test_id)))
}

/// The folder of the recorded test called `test_id` (on any thread, the
/// calling thread's first), else the folder of the test recorded on the calling
/// thread. `None` as `test_id` asks for the calling thread's test only.
fn active_test_logs(test_id: Option<&str>) -> Option<PathBuf> {
    let thread = std::thread::current().id();
    let active = ACTIVE_TEST_LOGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let named = |own_thread: bool| {
        let id = test_id?;
        active.iter().rev().find(|(owner, folder)| {
            (!own_thread || *owner == thread) && folder.file_name().is_some_and(|name| name == id)
        })
    };
    named(true)
        .or_else(|| named(false))
        .or_else(|| active.iter().rev().find(|(owner, _)| *owner == thread))
        .map(|(_, folder)| folder.clone())
}

/// Creates `active` and returns it; `latest_logs` when there is none.
fn ensure_active_or_latest(active: Option<PathBuf>) -> Result<PathBuf, SeleniumBaseError> {
    match active {
        Some(dir) => {
            ensure_dir(&dir)?;
            Ok(dir)
        }
        None => ensure_latest_logs_dir(),
    }
}

/// Creates the `latest_report` folder if it is missing and returns its path.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder cannot be created.
pub fn ensure_latest_report_dir() -> Result<PathBuf, SeleniumBaseError> {
    let path = logs_root().join(LATEST_REPORT_DIR);
    ensure_dir(&path)?;
    Ok(path)
}

/// Moves `latest_report` to `archived_reports/log_<unix seconds>`, makes an
/// empty `latest_report` again, and returns the folder it was moved to.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if a folder cannot be created or moved.
pub fn archive_latest_report() -> Result<PathBuf, SeleniumBaseError> {
    let latest = logs_root().join(LATEST_REPORT_DIR);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let archive = logs_root()
        .join(ARCHIVED_REPORTS_DIR)
        .join(format!("log_{seconds}"));
    if let Some(parent) = archive.parent() {
        ensure_dir(parent)?;
    }
    if latest.is_dir() {
        fs::rename(&latest, &archive).map_err(|error| {
            SeleniumBaseError::Io(io::Error::new(
                error.kind(),
                format!(
                    "failed to move {} to {}: {error}",
                    latest.display(),
                    archive.display()
                ),
            ))
        })?;
    } else {
        ensure_dir(&archive)?;
    }
    ensure_dir(&latest)?;
    Ok(archive)
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

/// The folder, in the working directory, where the cookie helpers keep their
/// files: Python's `SavedCookies.STORAGE_FOLDER`.
pub const SAVED_COOKIES_FOLDER: &str = "saved_cookies";

/// The file name the cookie helpers store the cookies called `requested`
/// under, as Python's `save_cookies(name)` works it out: only the last path
/// part is kept (`/` and `\` both separate parts), and `.txt` is added when it
/// is missing, so `../../etc/passwd` becomes `passwd.txt` and a name cannot
/// leave the folder.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if `requested` ends with a
/// separator or has no file name (`""`, `.`, `..`).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::artifacts::saved_cookies_file_name;
///
/// assert_eq!(saved_cookies_file_name("login").unwrap(), "login.txt");
/// assert_eq!(saved_cookies_file_name("a/b/login.txt").unwrap(), "login.txt");
/// assert!(saved_cookies_file_name("a/").is_err());
/// ```
pub fn saved_cookies_file_name(requested: &str) -> Result<String, SeleniumBaseError> {
    let last = requested.rsplit(['/', '\\']).next().unwrap_or_default();
    if requested.ends_with(['/', '\\']) || matches!(last, "" | "." | "..") {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "{requested:?} is not a file name for cookies"
        )));
    }
    let name = if last.ends_with(".txt") {
        last.to_owned()
    } else {
        format!("{last}.txt")
    };
    bare_file_name(&name)?;
    Ok(name)
}

/// `raw` with every character except ASCII letters, digits, `-` and `_`
/// replaced by `_`.
pub(crate) fn replace_unsafe(raw: &str) -> String {
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

/// What the tests need to play a recorder: a folder marked as the one of a
/// running test, on this thread or on another, until the guard is dropped.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;
    use std::sync::mpsc;
    use std::thread::{self, JoinHandle, ThreadId};

    /// A folder that is active for a thread as long as the guard lives.
    pub(crate) struct Active {
        /// The thread the folder is active for.
        pub(crate) thread: ThreadId,
        /// Ends the thread that holds the folder, when there is one.
        stop: Option<(mpsc::Sender<()>, JoinHandle<()>)>,
        dir: std::path::PathBuf,
    }

    impl Active {
        /// Makes `dir` the folder of the test on the calling thread.
        pub(crate) fn here(dir: &Path) -> Self {
            Self {
                thread: super::activate_test_logs(dir),
                stop: None,
                dir: dir.to_path_buf(),
            }
        }

        /// Makes `dir` the folder of a test on a thread of its own, as a
        /// scenario's recorder does.
        pub(crate) fn elsewhere(dir: &Path) -> Self {
            let (stop, stopped) = mpsc::channel::<()>();
            let (ready, started) = mpsc::channel();
            let folder = dir.to_path_buf();
            let holder = thread::spawn(move || {
                let thread = super::activate_test_logs(&folder);
                ready.send(thread).expect("the test is waiting");
                // Wait for the guard to be dropped (the sender is gone).
                let _ = stopped.recv();
                super::deactivate_test_logs(thread, &folder);
            });
            Self {
                thread: started.recv().expect("the thread activates the folder"),
                stop: Some((stop, holder)),
                dir: dir.to_path_buf(),
            }
        }
    }

    impl Drop for Active {
        fn drop(&mut self) {
            match self.stop.take() {
                Some((stop, holder)) => {
                    drop(stop);
                    let _ = holder.join();
                }
                None => super::deactivate_test_logs(self.thread, &self.dir),
            }
        }
    }
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

    /// An id no other test uses, so tests that share the registry of active
    /// folders (it is one for the whole process) cannot see each other.
    fn unique_id(name: &str) -> String {
        format!("artifacts-{name}-{:?}", std::thread::current().id())
            .replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")
    }

    #[test]
    fn the_folder_of_a_test_recorded_on_another_thread_is_found_by_its_id() {
        let root = tempfile::tempdir().unwrap();
        let id = unique_id("elsewhere");
        let folder = root.path().join(&id);
        let _recorded = test_support::Active::elsewhere(&folder);

        // The calling thread records nothing, so only the id leads to it.
        assert_eq!(active_test_logs(None), None);
        assert_eq!(active_test_logs(Some(&id)), Some(folder.clone()));
        assert_eq!(ensure_test_logs_dir_for(&id).unwrap(), folder);
        assert!(folder.is_dir(), "the folder is made when it is asked for");
    }

    #[test]
    fn the_folder_of_this_thread_answers_when_no_id_matches() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(unique_id("here"));
        let _recorded = test_support::Active::here(&folder);

        assert_eq!(active_test_logs(None), Some(folder.clone()));
        assert_eq!(
            active_test_logs(Some("an-id-nothing-records")),
            Some(folder.clone())
        );
        assert_eq!(ensure_test_logs_dir().unwrap(), folder);
    }

    #[test]
    fn a_folder_that_matches_the_id_beats_the_one_of_this_thread() {
        let root = tempfile::tempdir().unwrap();
        let own = root.path().join(unique_id("own"));
        let id = unique_id("named");
        let named = root.path().join(&id);
        let _own = test_support::Active::here(&own);
        let _named = test_support::Active::elsewhere(&named);

        assert_eq!(active_test_logs(None), Some(own));
        assert_eq!(active_test_logs(Some(&id)), Some(named));
    }

    #[test]
    fn the_folder_of_this_thread_wins_when_two_tests_share_an_id() {
        let root = tempfile::tempdir().unwrap();
        let id = unique_id("shared");
        let other = root.path().join("a").join(&id);
        let own = root.path().join("b").join(&id);
        let _other = test_support::Active::elsewhere(&other);
        let _own = test_support::Active::here(&own);

        assert_eq!(active_test_logs(Some(&id)), Some(own));
    }

    #[test]
    fn a_test_that_has_ended_leaves_no_folder_behind() {
        let root = tempfile::tempdir().unwrap();
        let id = unique_id("ended");
        let folder = root.path().join(&id);
        drop(test_support::Active::elsewhere(&folder));
        drop(test_support::Active::here(&folder));

        assert_eq!(active_test_logs(Some(&id)), None);
        assert_eq!(active_test_logs(None), None);
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
