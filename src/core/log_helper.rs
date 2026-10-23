//! The logs a test run leaves behind, named and written the way Python
//! SeleniumBase's `core/log_helper.py` does.
//!
//! - [`TestId`] says which test is running: the id that names its folder
//!   (`latest_logs/<id>/`) and the name a report shows (the display id).
//! - [`log_test_failure_data`], [`log_skipped_test_data`], [`log_page_source`]
//!   and [`log_screenshot`] write the files of one test's folder.
//! - [`log_folder_setup`], [`archive_logs_if_set`] and [`clear_empty_logs`]
//!   manage the `latest_logs` and `archived_logs` folders of a whole run.
//!
//! Nothing here deletes a file outside `latest_logs`. [`log_folder_setup`]
//! refuses to clear a `latest_logs` that is a symbolic link, and a test id is
//! always one plain file name, so a hostile test name cannot name a folder
//! outside the logs.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::core::log_helper::{test_folder_id, TestId};
//!
//! assert_eq!(test_folder_id("login::tests::valid login"), "login.tests.valid_login");
//! assert!(!test_folder_id("../../etc/passwd").contains('/'));
//!
//! let scenario = TestId::scenario("tests/features/login.feature", 12, "Successful login", ["smoke"]);
//! assert_eq!(scenario.display, "tests/features/login.feature:12 => Successful login");
//! ```

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone};

use crate::artifacts::LATEST_LOGS_DIR;
use crate::common::test_id;
use crate::core::report_helper::html_escape;

/// The folder the logs of earlier runs are moved to (`--archive-logs`).
pub const ARCHIVED_LOGS_DIR: &str = "archived_logs";
/// The file with the facts about a failed test.
pub const BASIC_INFO_FILE: &str = "basic_test_info.txt";
/// The file with the HTML of the page a test failed on.
pub const PAGE_SOURCE_FILE: &str = "page_source.html";
/// The file with the screenshot of the page.
pub const SCREENSHOT_FILE: &str = "screenshot.png";
/// The file with the reason a test was skipped.
pub const SKIP_REASON_FILE: &str = "skip_reason.txt";

/// The longest folder id; a longer one keeps its start and its end.
const MAX_ID_LEN: usize = 223;
/// How much of each end a truncated id keeps.
const KEPT_ENDS: usize = 105;
/// What replaces the middle of a truncated id.
const TRUNCATED: &str = "__TRUNCATED__";
/// What an id is when nothing is left of a name.
const FALLBACK_ID: &str = "test";
/// What a test is called when no name can be found for it.
const UNIDENTIFIED: &str = "unidentified_test";
/// The rule above and below the header of the info file.
const RULE: &str = "--------------------------------------------------------------------";

/// A Gherkin scenario, for a [`TestId`] made by [`TestId::scenario`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioInfo {
    /// The path of the feature file.
    pub feature: String,
    /// The line of the scenario in that file.
    pub line: u32,
    /// The scenario's name.
    pub name: String,
    /// The scenario's tags, without the `@`.
    pub tags: Vec<String>,
}

/// Which test a result or a log belongs to.
///
/// `id` names the test's folder in `latest_logs`. `display` is what a person
/// reads in a report: `login_chrome::tests::test_login` for a Rust test,
/// `features/login.feature:12 => Successful login` for a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestId {
    /// One plain file name: letters, digits, `.`, `-` and `_`.
    pub id: String,
    /// The name a report shows.
    pub display: String,
    /// The test binary (or feature file) the test belongs to.
    pub suite: String,
    /// The scenario, for a test that is one.
    pub scenario: Option<ScenarioInfo>,
}

impl TestId {
    /// The test running on the calling thread.
    ///
    /// The harness names the thread that runs a test after it, and cargo names
    /// a test binary after its crate or test file, so call this on the thread
    /// the harness started, before the test spawns any task. The id is the one
    /// [`current_test_id`](crate::common::test_id::current_test_id) gives, so the
    /// `sb` fixture and the run write to the same folder. A thread with no test
    /// name (`main`, a tokio worker) is `unidentified_test`: name the test
    /// with [`TestId::named`] instead.
    #[must_use]
    pub fn current() -> Self {
        let thread = std::thread::current();
        let exe = std::env::current_exe().ok();
        let binary = exe
            .as_deref()
            .and_then(Path::file_stem)
            .and_then(OsStr::to_str);
        Self::from_thread(thread.name().unwrap_or_default(), binary)
    }

    /// The test run by the thread named `thread` in the program `binary` (its
    /// file name without the extension).
    #[must_use]
    pub fn from_thread(thread: &str, binary: Option<&str>) -> Self {
        let suite = binary.map(binary_suite_name).unwrap_or_default();
        let names_a_test =
            !thread.is_empty() && thread != "main" && !thread.starts_with("tokio-runtime-worker");
        if !names_a_test {
            return Self {
                id: UNIDENTIFIED.to_owned(),
                display: UNIDENTIFIED.to_owned(),
                suite,
                scenario: None,
            };
        }
        let display = if suite.is_empty() {
            thread.to_owned()
        } else {
            format!("{suite}::{thread}")
        };
        Self {
            id: test_id::test_id(thread, binary),
            display,
            suite,
            scenario: None,
        }
    }

    /// A test with a name of the caller's choosing, such as `shop::checkout`.
    /// The suite is the part before the first `::`.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            id: test_folder_id(name),
            display: name.to_owned(),
            suite: name
                .split_once("::")
                .map(|(suite, _)| suite.to_owned())
                .unwrap_or_default(),
            scenario: None,
        }
    }

    /// A Gherkin scenario: `features/login.feature:12 => Successful login`.
    /// The suite is the feature file.
    #[must_use]
    pub fn scenario<T: Into<String>>(
        feature: &str,
        line: u32,
        name: &str,
        tags: impl IntoIterator<Item = T>,
    ) -> Self {
        let tags: Vec<String> = tags
            .into_iter()
            .map(|tag| tag.into().trim_start_matches('@').to_owned())
            .collect();
        Self {
            id: test_folder_id(&format!("{feature}.{name}")),
            display: format!("{feature}:{line} => {name}"),
            suite: feature.to_owned(),
            scenario: Some(ScenarioInfo {
                feature: feature.to_owned(),
                line,
                name: name.to_owned(),
                tags,
            }),
        }
    }
}

/// The folder id of the test called `display`, by Python's rules: `.py::`,
/// `::`, `/` and `\` become `.`, a space becomes `_`, and anything but
/// letters, digits, `.`, `-` and `_` becomes `_`. Leading dots are dropped, so
/// the id cannot be `..`. An id longer than 223 characters becomes its first
/// 105, `__TRUNCATED__` and its last 105. `test` is what is left of nothing.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::core::log_helper::test_folder_id;
///
/// assert_eq!(test_folder_id("shop::cart::adds an item"), "shop.cart.adds_an_item");
/// assert_eq!(test_folder_id("../.."), "test");
/// assert_eq!(test_folder_id(&"x".repeat(300)).len(), 223);
/// ```
#[must_use]
pub fn test_folder_id(display: &str) -> String {
    let converted = display
        .replace(".py::", ".")
        .replace("::", ".")
        .replace(['/', '\\'], ".")
        .replace(' ', "_");
    let cleaned: String = converted
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let id = cleaned.trim_start_matches('.');
    // Only ASCII is left, so any byte index is a character boundary.
    if id.len() > MAX_ID_LEN {
        format!(
            "{}{TRUNCATED}{}",
            &id[..KEPT_ENDS],
            &id[id.len() - KEPT_ENDS..]
        )
    } else if id.is_empty() {
        FALLBACK_ID.to_owned()
    } else {
        id.to_owned()
    }
}

/// The name of a test binary cargo built: `login_chrome` for
/// `login_chrome-0123456789abcdef`. Any other name is returned as it is, less
/// its extension.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::core::log_helper::binary_suite_name;
///
/// assert_eq!(binary_suite_name("login_chrome-0123456789abcdef"), "login_chrome");
/// assert_eq!(binary_suite_name("/tmp/deps/shop-0123456789abcdef.exe"), "shop");
/// assert_eq!(binary_suite_name("mytool"), "mytool");
/// ```
#[must_use]
pub fn binary_suite_name(exe: &str) -> String {
    let stem = Path::new(exe)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or(exe);
    match stem.rsplit_once('-') {
        Some((name, hash))
            if !name.is_empty()
                && hash.len() == 16
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
        {
            name.to_owned()
        }
        _ => stem.to_owned(),
    }
}

/// The time, in the three forms the info files print: Python's
/// `get_master_time`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterTime {
    /// Seconds since the Unix epoch.
    pub timestamp: String,
    /// `Wednesday, 22 October 2026`.
    pub date: String,
    /// `3:24:01 PM  (UTC+02:00)`. The zone is its offset: the name of a zone
    /// (`CEST`) needs a time zone database.
    pub time: String,
}

/// `now` as a [`MasterTime`].
///
/// # Examples
///
/// ```
/// use chrono::{TimeZone, Utc};
/// use seleniumbase_rs::core::log_helper::get_master_time;
///
/// let now = Utc.with_ymd_and_hms(2026, 10, 22, 15, 24, 1).unwrap();
/// let time = get_master_time(&now);
/// assert_eq!(time.date, "Thursday, 22 October 2026");
/// assert_eq!(time.time, "3:24:01 PM  (UTC+00:00)");
/// ```
#[must_use]
pub fn get_master_time<Tz>(now: &DateTime<Tz>) -> MasterTime
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    MasterTime {
        timestamp: now.timestamp().to_string(),
        date: now.format("%A, %d %B %Y").to_string(),
        time: format!("{}  (UTC{})", now.format("%-I:%M:%S %p"), now.format("%:z")),
    }
}

/// What the info file of a failed or skipped test says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FailureInfo {
    /// The test's display id: the first line of the file.
    pub test_id: String,
    /// The page the test ended on, when it could be read.
    pub last_page: Option<String>,
    /// How long the test ran.
    pub duration: Duration,
    /// The browser and its version, such as `Chrome 155.0.7000.1 / headless`.
    pub browser: Option<String>,
    /// What drove the browser, such as `chromedriver 155.0.7000.1`.
    pub driver: Option<String>,
    /// The error with each of its causes; left out of the file when `None`.
    pub traceback: Option<String>,
    /// The error's message.
    pub exception: String,
}

/// The lines of an info file above the traceback.
fn header(info: &FailureInfo, time: &MasterTime) -> Vec<String> {
    vec![
        info.test_id.clone(),
        RULE.to_owned(),
        format!(
            "Last Page: {}",
            info.last_page.as_deref().unwrap_or("(Unknown Page)")
        ),
        format!(" Duration: {:.2}s", info.duration.as_secs_f64()),
        format!(
            "  Browser: {}",
            info.browser.as_deref().unwrap_or("(Unknown Browser)")
        ),
        format!(
            "   Driver: {}",
            info.driver.as_deref().unwrap_or("(Unknown Driver)")
        ),
        format!("Timestamp: {}  (Unix Timestamp)", time.timestamp),
        format!("     Date: {}", time.date),
        format!("     Time: {}", time.time),
        RULE.to_owned(),
    ]
}

/// The text of `basic_test_info.txt`: the lines Python writes, joined with
/// `\r\n`.
#[must_use]
pub fn render_failure_info(info: &FailureInfo, time: &MasterTime) -> String {
    let mut lines = header(info, time);
    if let Some(traceback) = &info.traceback {
        lines.push("Traceback:".to_owned());
        lines.extend(traceback.lines().map(|line| format!("  {line}")));
    }
    lines.push(format!("Exception: {}", info.exception));
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    text
}

/// Writes `basic_test_info.txt` into `dir`, which is created first, and
/// returns its path.
///
/// # Errors
///
/// Returns the I/O error if the folder or the file cannot be written.
pub fn log_test_failure_data(dir: &Path, info: &FailureInfo) -> io::Result<PathBuf> {
    let text = render_failure_info(info, &get_master_time(&Local::now()));
    write_file(dir, BASIC_INFO_FILE, text.as_bytes())
}

/// Writes `skip_reason.txt` into `dir`: the header of the info file and the
/// reason the test was skipped.
///
/// # Errors
///
/// Returns the I/O error if the folder or the file cannot be written.
pub fn log_skipped_test_data(dir: &Path, info: &FailureInfo, reason: &str) -> io::Result<PathBuf> {
    let mut lines = header(info, &get_master_time(&Local::now()));
    lines.push(format!("Skip Reason: {reason}"));
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    write_file(dir, SKIP_REASON_FILE, text.as_bytes())
}

/// Writes `page_source.html` into `dir`: `source` with a charset and a
/// `<base href>` for `last_page` added, so the saved page finds its
/// stylesheets and images when it is opened from the logs folder.
///
/// # Errors
///
/// Returns the I/O error if the folder or the file cannot be written.
pub fn log_page_source(dir: &Path, last_page: Option<&str>, source: &str) -> io::Result<PathBuf> {
    write_file(
        dir,
        PAGE_SOURCE_FILE,
        with_base(source, last_page).as_bytes(),
    )
}

/// Writes `screenshot.png` into `dir` and returns its path.
///
/// # Errors
///
/// Returns the I/O error if the folder or the file cannot be written.
pub fn log_screenshot(dir: &Path, png: &[u8]) -> io::Result<PathBuf> {
    write_file(dir, SCREENSHOT_FILE, png)
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    fs::write(&path, bytes)?;
    Ok(path)
}

/// `source` with the charset and base elements added just after `<head>`, or
/// in front when there is no head.
fn with_base(source: &str, last_page: Option<&str>) -> String {
    let mut added = String::from("<meta charset=\"utf-8\">");
    if let Some(page) = last_page.filter(|page| !page.is_empty()) {
        let _ = write!(added, "<base href=\"{}\">", html_escape(page));
    }
    added.push('\n');
    let head_end = source
        .to_ascii_lowercase()
        .find("<head")
        .and_then(|start| source[start..].find('>').map(|end| start + end + 1));
    match head_end {
        Some(end) => format!("{}\n{added}{}", &source[..end], &source[end..]),
        None => format!("{added}{source}"),
    }
}

/// Prepares the `latest_logs` folder of `root` for a new run and returns the
/// folder the old logs were moved to, if they were.
///
/// With `archive_logs` the old logs are first copied to
/// `archived_logs/logs_<unix seconds>`. Then the folder is emptied and made
/// again. Nothing outside `latest_logs` is touched.
///
/// # Errors
///
/// Returns an error, and deletes nothing, if `latest_logs` is a symbolic link
/// or a file. Returns the I/O error if the folder cannot be archived, emptied
/// or created.
pub fn log_folder_setup(root: &Path, archive_logs: bool) -> io::Result<Option<PathBuf>> {
    let latest = root.join(LATEST_LOGS_DIR);
    let mut archived = None;
    match fs::symlink_metadata(&latest) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is a symbolic link, so it is not cleared; remove the link",
                    latest.display()
                ),
            ));
        }
        Ok(meta) if meta.is_dir() => {
            archived = archive_logs_if_set(root, archive_logs)?;
            fs::remove_dir_all(&latest)?;
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a folder", latest.display()),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(&latest)?;
    Ok(archived)
}

/// With `archive_logs`, copies the `latest_logs` folder of `root` to
/// `archived_logs/logs_<unix seconds>` and returns the new folder. Without it,
/// or when there is nothing to keep, does nothing.
///
/// # Errors
///
/// Returns the I/O error if the copy fails.
pub fn archive_logs_if_set(root: &Path, archive_logs: bool) -> io::Result<Option<PathBuf>> {
    let latest = root.join(LATEST_LOGS_DIR);
    let has_logs = fs::read_dir(&latest).is_ok_and(|mut entries| entries.next().is_some());
    if !archive_logs || !has_logs {
        return Ok(None);
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let archive = root.join(ARCHIVED_LOGS_DIR);
    let mut target = archive.join(format!("logs_{seconds}"));
    let mut n = 0_u32;
    while target.exists() {
        n += 1;
        target = archive.join(format!("logs_{seconds}_{n}"));
    }
    copy_dir(&latest, &target)?;
    Ok(Some(target))
}

/// Removes the empty folders directly inside `latest_logs` of `root`.
///
/// # Errors
///
/// Returns the I/O error if the folder cannot be read; a folder that cannot be
/// removed is left.
pub fn clear_empty_logs(root: &Path) -> io::Result<()> {
    let latest = root.join(LATEST_LOGS_DIR);
    let entries = match fs::read_dir(&latest) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries.flatten() {
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        // `remove_dir` refuses a folder that has anything in it.
        if is_folder {
            let _ = fs::remove_dir(entry.path());
        }
    }
    Ok(())
}

/// Copies the folder `from` to `to`. A symbolic link inside is skipped, not
/// followed.
pub(crate) fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The link from a page in `from_dir` to `to`, such as
/// `../latest_logs/x/screenshot.png`: `/` separated, and percent-encoded where
/// a URL needs it. Both paths are read as they are, without asking the file
/// system, so give both relative to the same root or both absolute.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use seleniumbase_rs::core::log_helper::relative_link;
///
/// let root = Path::new("/run");
/// assert_eq!(
///     relative_link(&root.join("reports"), &root.join("latest_logs/a/shot.png")),
///     "../latest_logs/a/shot.png"
/// );
/// assert_eq!(relative_link(root, &root.join("latest_logs/my test")), "latest_logs/my%20test");
/// ```
#[must_use]
pub fn relative_link(from_dir: &Path, to: &Path) -> String {
    let parts = |path: &Path| -> Vec<String> {
        path.components()
            .filter(|part| !matches!(part, Component::CurDir))
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let (from, to) = (parts(from_dir), parts(to));
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut link: Vec<String> = vec!["..".to_owned(); from.len() - shared];
    link.extend(to[shared..].iter().map(|part| percent_encode(part)));
    if link.is_empty() {
        ".".to_owned()
    } else {
        link.join("/")
    }
}

/// `part` with every byte a URL path segment cannot hold written as `%XX`.
fn percent_encode(part: &str) -> String {
    let mut encoded = String::with_capacity(part.len());
    for byte in part.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn info() -> FailureInfo {
        FailureInfo {
            test_id: "login_chrome::tests::test_login".to_owned(),
            last_page: Some("https://example.com/login".to_owned()),
            duration: Duration::from_millis(3210),
            browser: Some("Chrome 155.0.7000.1 / headless".to_owned()),
            driver: Some("chromedriver 155.0.7000.1".to_owned()),
            traceback: Some("assertion failed: title\ncaused by: boom".to_owned()),
            exception: "assertion failed: title".to_owned(),
        }
    }

    fn fixed_time() -> MasterTime {
        get_master_time(&Utc.with_ymd_and_hms(2026, 10, 22, 15, 24, 1).unwrap())
    }

    #[test]
    fn a_folder_id_follows_pythons_rules() {
        assert_eq!(test_folder_id("a::b::c"), "a.b.c");
        assert_eq!(
            test_folder_id("tests/test_login.py::Class::method"),
            "tests.test_login.Class.method"
        );
        assert_eq!(test_folder_id(r"dir\file name"), "dir.file_name");
        assert_eq!(test_folder_id("Mixed-Case_ok.1"), "Mixed-Case_ok.1");
        assert_eq!(test_folder_id("ünï/<b>"), "_n_._b_");
    }

    #[test]
    fn a_hostile_name_stays_inside_the_logs() {
        for hostile in [
            "../../etc/passwd",
            "..",
            "../..",
            "a/../../b",
            r"..\..\x",
            "/abs/olute",
            "",
        ] {
            let id = test_folder_id(hostile);
            assert!(
                crate::artifacts::bare_file_name(&id).is_ok(),
                "{hostile:?} gave {id:?}"
            );
            assert!(!id.starts_with('.'), "{id:?}");
        }
        assert_eq!(test_folder_id("../../etc/passwd"), "etc.passwd");
        assert_eq!(test_folder_id(".."), "test");
    }

    #[test]
    fn a_long_name_keeps_its_two_ends() {
        let name = format!("{}{}", "a".repeat(150), "z".repeat(150));
        let id = test_folder_id(&name);
        assert_eq!(id.len(), 223);
        assert!(id.starts_with(&"a".repeat(105)));
        assert!(id.ends_with(&"z".repeat(105)));
        assert_eq!(&id[105..118], "__TRUNCATED__");
        let exact = "q".repeat(223);
        assert_eq!(test_folder_id(&exact), exact, "223 characters fit");
    }

    #[test]
    fn the_hash_of_a_test_binary_is_dropped() {
        assert_eq!(
            binary_suite_name("login_chrome-0123456789abcdef"),
            "login_chrome"
        );
        assert_eq!(binary_suite_name("/x/deps/a-b-0123456789ABCDEF.exe"), "a-b");
        assert_eq!(binary_suite_name("tool-v1"), "tool-v1");
        assert_eq!(binary_suite_name("-0123456789abcdef"), "-0123456789abcdef");
    }

    #[test]
    fn a_thread_name_becomes_a_display_and_a_folder_id() {
        let id = TestId::from_thread("tests::test_login", Some("login_chrome-0123456789abcdef"));
        assert_eq!(id.display, "login_chrome::tests::test_login");
        assert_eq!(id.id, "login_chrome.tests.test_login");
        assert_eq!(id.suite, "login_chrome");
        let case = TestId::from_thread("login::case_1_firefox", Some("sb-0123456789abcdef"));
        assert_eq!(case.display, "sb::login::case_1_firefox");
        assert_eq!(case.id, "sb.login.case_1_firefox");
    }

    #[test]
    fn a_thread_that_is_no_test_is_unidentified() {
        for thread in ["main", "tokio-runtime-worker", "tokio-runtime-worker-3", ""] {
            let id = TestId::from_thread(thread, Some("x-0123456789abcdef"));
            assert_eq!(id.id, "unidentified_test", "{thread:?}");
            assert_eq!(id.display, "unidentified_test");
        }
        assert_eq!(TestId::named("shop::cart").suite, "shop");
        assert_eq!(TestId::named("shop::cart").id, "shop.cart");
    }

    #[test]
    fn the_running_test_is_found_from_its_thread() {
        let id = TestId::current();
        assert!(
            id.display
                .ends_with("log_helper::tests::the_running_test_is_found_from_its_thread"),
            "{}",
            id.display
        );
        assert_eq!(id.id, crate::common::test_id::current_test_id());
    }

    #[test]
    fn a_scenario_is_named_by_its_feature_line_and_title() {
        let id = TestId::scenario(
            "tests/features/login.feature",
            12,
            "Successful login",
            ["@smoke", "ui"],
        );
        assert_eq!(
            id.display,
            "tests/features/login.feature:12 => Successful login"
        );
        assert_eq!(id.id, "tests.features.login.feature.Successful_login");
        assert_eq!(id.suite, "tests/features/login.feature");
        let scenario = id.scenario.unwrap();
        assert_eq!(
            (scenario.line, scenario.tags),
            (12, vec!["smoke".to_owned(), "ui".to_owned()])
        );
    }

    #[test]
    fn the_master_time_has_python_s_three_forms() {
        let time = fixed_time();
        assert_eq!(time.timestamp, "1792682641");
        assert_eq!(time.date, "Thursday, 22 October 2026");
        assert_eq!(time.time, "3:24:01 PM  (UTC+00:00)");
    }

    #[test]
    fn the_info_file_has_pythons_lines_joined_with_crlf() {
        let text = render_failure_info(&info(), &fixed_time());
        let lines: Vec<&str> = text.split("\r\n").collect();
        assert_eq!(lines[0], "login_chrome::tests::test_login");
        assert_eq!(lines[1], RULE);
        assert_eq!(lines[2], "Last Page: https://example.com/login");
        assert_eq!(lines[3], " Duration: 3.21s");
        assert_eq!(lines[4], "  Browser: Chrome 155.0.7000.1 / headless");
        assert_eq!(lines[5], "   Driver: chromedriver 155.0.7000.1");
        assert_eq!(lines[6], "Timestamp: 1792682641  (Unix Timestamp)");
        assert_eq!(lines[7], "     Date: Thursday, 22 October 2026");
        assert_eq!(lines[8], "     Time: 3:24:01 PM  (UTC+00:00)");
        assert_eq!(lines[9], RULE);
        assert_eq!(lines[10], "Traceback:");
        assert_eq!(lines[11], "  assertion failed: title");
        assert_eq!(lines[12], "  caused by: boom");
        assert_eq!(lines[13], "Exception: assertion failed: title");
        assert_eq!(lines[14], "", "the file ends with a line end");
        assert!(
            !text.replace("\r\n", "").contains('\n'),
            "no bare line feed"
        );
    }

    #[test]
    fn what_is_unknown_is_said_so() {
        let bare = FailureInfo {
            test_id: "t".to_owned(),
            ..FailureInfo::default()
        };
        let text = render_failure_info(&bare, &fixed_time());
        assert!(text.contains("Last Page: (Unknown Page)"));
        assert!(text.contains("  Browser: (Unknown Browser)"));
        assert!(text.contains("   Driver: (Unknown Driver)"));
        assert!(!text.contains("Traceback:"));
    }

    #[test]
    fn the_log_writers_make_the_folder_and_the_files() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("latest_logs").join("a.b");
        let info_path = log_test_failure_data(&dir, &info()).unwrap();
        assert_eq!(info_path, dir.join("basic_test_info.txt"));
        assert!(fs::read_to_string(&info_path)
            .unwrap()
            .starts_with("login_chrome::tests::test_login\r\n"));
        let skip = log_skipped_test_data(&dir, &info(), "no browser here").unwrap();
        assert!(fs::read_to_string(skip)
            .unwrap()
            .ends_with("Skip Reason: no browser here\r\n"));
        let shot = log_screenshot(&dir, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(fs::read(shot).unwrap(), b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn the_page_source_gets_a_charset_and_a_base() {
        let dir = tempfile::tempdir().unwrap();
        let path = log_page_source(
            dir.path(),
            Some("https://a.test/x?a=1&b=\"2\""),
            "<html><head><title>t</title></head></html>",
        )
        .unwrap();
        let saved = fs::read_to_string(path).unwrap();
        assert!(
            saved.contains("<base href=\"https://a.test/x?a=1&amp;b=&quot;2&quot;\">"),
            "{saved}"
        );
        assert!(saved.contains("<meta charset=\"utf-8\">"));
        assert!(
            saved.find("<head>").unwrap() < saved.find("<base").unwrap(),
            "inside the head"
        );
        assert!(saved.contains("<title>t</title>"));
        let headless = with_base("<p>x</p>", Some("https://a.test/"));
        assert!(headless.starts_with("<meta charset"), "{headless}");
        assert!(!with_base("<p>x</p>", None).contains("<base"));
    }

    fn logs_with_a_file(root: &Path, test: &str) {
        let dir = root.join(LATEST_LOGS_DIR).join(test);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("screenshot.png"), b"x").unwrap();
    }

    #[test]
    fn a_new_run_clears_the_old_logs() {
        let root = tempfile::tempdir().unwrap();
        logs_with_a_file(root.path(), "old");
        assert_eq!(log_folder_setup(root.path(), false).unwrap(), None);
        assert!(root.path().join(LATEST_LOGS_DIR).is_dir());
        assert_eq!(
            fs::read_dir(root.path().join(LATEST_LOGS_DIR))
                .unwrap()
                .count(),
            0
        );
        assert!(!root.path().join(ARCHIVED_LOGS_DIR).exists());
    }

    #[test]
    fn archiving_keeps_the_old_logs_in_a_folder_named_by_the_time() {
        let root = tempfile::tempdir().unwrap();
        logs_with_a_file(root.path(), "old");
        let archived = log_folder_setup(root.path(), true).unwrap().unwrap();
        assert!(archived.parent().unwrap().ends_with(ARCHIVED_LOGS_DIR));
        assert!(archived
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("logs_"));
        assert!(archived.join("old/screenshot.png").is_file());
        assert_eq!(
            fs::read_dir(root.path().join(LATEST_LOGS_DIR))
                .unwrap()
                .count(),
            0
        );
        // A second archive in the same second does not replace the first.
        logs_with_a_file(root.path(), "newer");
        let second = archive_logs_if_set(root.path(), true).unwrap().unwrap();
        assert_ne!(second, archived);
        assert!(archived.join("old/screenshot.png").is_file());
    }

    #[test]
    fn nothing_is_archived_when_there_is_nothing_or_it_is_off() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(archive_logs_if_set(root.path(), true).unwrap(), None);
        logs_with_a_file(root.path(), "t");
        assert_eq!(archive_logs_if_set(root.path(), false).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_latest_logs_that_is_a_symbolic_link_is_refused_and_nothing_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("precious"), b"keep").unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), root.path().join(LATEST_LOGS_DIR)).unwrap();
        let error = log_folder_setup(root.path(), false).unwrap_err();
        assert!(error.to_string().contains("symbolic link"), "{error}");
        assert!(elsewhere.path().join("precious").is_file());
    }

    #[test]
    fn a_latest_logs_that_is_a_file_is_refused() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(LATEST_LOGS_DIR), b"x").unwrap();
        assert!(log_folder_setup(root.path(), false).is_err());
        assert!(root.path().join(LATEST_LOGS_DIR).is_file());
    }

    #[test]
    fn only_the_empty_folders_are_removed() {
        let root = tempfile::tempdir().unwrap();
        logs_with_a_file(root.path(), "full");
        fs::create_dir_all(root.path().join(LATEST_LOGS_DIR).join("empty")).unwrap();
        clear_empty_logs(root.path()).unwrap();
        assert!(root.path().join(LATEST_LOGS_DIR).join("full").is_dir());
        assert!(!root.path().join(LATEST_LOGS_DIR).join("empty").exists());
        let nowhere = tempfile::tempdir().unwrap();
        clear_empty_logs(nowhere.path()).unwrap();
    }

    #[test]
    fn a_link_climbs_out_of_the_page_folder_and_encodes_what_a_url_needs() {
        let root = Path::new("/run");
        let file = root.join("latest_logs/a b/screenshot.png");
        assert_eq!(
            relative_link(root, &file),
            "latest_logs/a%20b/screenshot.png"
        );
        assert_eq!(
            relative_link(&root.join("out/deep"), &file),
            "../../latest_logs/a%20b/screenshot.png"
        );
        assert_eq!(relative_link(root, root), ".");
        assert_eq!(
            relative_link(Path::new("."), Path::new("latest_logs/x")),
            "latest_logs/x"
        );
    }
}
