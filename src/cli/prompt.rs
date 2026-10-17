//! `sbrs prompt`: one short line about the installed browser, for a shell
//! prompt.
//!
//! The line names the installed Chrome's major version and whether a
//! chromedriver of that version is already at hand, such as
//! `chrome 155 ✓driver`. A shell runs it before every command line, so it
//! starts no browser and opens no connection. Versions are read from files
//! where the system keeps them ([`major_without_running`]: the `Info.plist` of
//! a macOS app, the version folder beside `chrome.exe`). A version that only
//! running the program tells (`chromedriver --version`, Chrome on Linux) is
//! remembered in a [`VersionCache`] for [`CACHE_TTL`], together with the
//! program's size and modification time, so a new download is noticed at once.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::prompt::{DriverStatus, Marks, PromptFormat, Segment};
//!
//! let segment = Segment {
//!     browser: "chrome".to_owned(),
//!     major: 155,
//!     driver: DriverStatus::Ready,
//! };
//! let format: PromptFormat = "{browser} {major} {driver}".parse().unwrap();
//! assert_eq!(format.render(&segment, Marks::Unicode), "chrome 155 ✓driver");
//! assert_eq!(format.render(&segment, Marks::Ascii), "chrome 155 +driver");
//! ```
//!
//! [`major_without_running`]: crate::browser::driver_version::major_without_running

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::browser::driver_version::{major_without_running, version_flag_major};
use crate::resources::assets::{DriverLookup, CHROMEDRIVER_PATH_VAR, DRIVERS_DIR};

/// The line `sbrs prompt` prints when no `--format` is given.
pub const DEFAULT_FORMAT: &str = "{browser} {major} {driver}";

/// The placeholders a format may use.
pub const PLACEHOLDERS: [&str; 5] = ["browser", "major", "driver", "mark", "driver_major"];

/// How long a version read by running a program is trusted.
pub const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// How long `--version` may take before the prompt gives up on the program.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// The name of the cache file in the user's cache folder.
const CACHE_FILE: &str = "prompt-versions.json";

/// Whether the chromedriver at hand can drive the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverStatus {
    /// A chromedriver of the browser's major version is there.
    Ready,
    /// A chromedriver is there, of another major version.
    Mismatch {
        /// The chromedriver's major version.
        driver: u32,
    },
    /// A chromedriver is there, but its version could not be read.
    Unknown,
    /// No chromedriver was found.
    Missing,
}

/// What the prompt line is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// A short name of the browser: `chrome` or `chromium`.
    pub browser: String,
    /// The browser's major version.
    pub major: u32,
    /// How the chromedriver relates to the browser.
    pub driver: DriverStatus,
}

impl Segment {
    /// The chromedriver's major version, when it is known.
    #[must_use]
    pub fn driver_major(&self) -> Option<u32> {
        match self.driver {
            DriverStatus::Ready => Some(self.major),
            DriverStatus::Mismatch { driver } => Some(driver),
            DriverStatus::Unknown | DriverStatus::Missing => None,
        }
    }
}

/// The characters that mark the driver's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Marks {
    /// `✓` ready, `✗` missing or of another version, `?` unknown.
    #[default]
    Unicode,
    /// `+` ready, `-` missing or of another version, `?` unknown, for
    /// terminals and fonts without the check marks.
    Ascii,
}

impl Marks {
    /// The mark of `status`.
    #[must_use]
    pub const fn mark(self, status: DriverStatus) -> &'static str {
        match (self, status) {
            (Self::Unicode, DriverStatus::Ready) => "✓",
            (Self::Ascii, DriverStatus::Ready) => "+",
            (Self::Unicode, DriverStatus::Mismatch { .. } | DriverStatus::Missing) => "✗",
            (Self::Ascii, DriverStatus::Mismatch { .. } | DriverStatus::Missing) => "-",
            (_, DriverStatus::Unknown) => "?",
        }
    }
}

/// A part of a [`PromptFormat`].
#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Browser,
    Major,
    Driver,
    Mark,
    DriverMajor,
}

/// A `--format` template, such as `{browser} {major} {driver}`.
///
/// The placeholders are `{browser}` (`chrome` or `chromium`), `{major}` (the
/// browser's major version), `{driver}` (the mark and the word `driver`, as in
/// `✓driver`), `{mark}` (the mark alone) and `{driver_major}` (the
/// chromedriver's major version, empty when unknown or missing). `{{` and `}}`
/// stand for literal braces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptFormat {
    pieces: Vec<Piece>,
}

impl Default for PromptFormat {
    fn default() -> Self {
        DEFAULT_FORMAT.parse().expect("the default format is valid")
    }
}

/// Why a `--format` template was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// A placeholder that is not one of [`PLACEHOLDERS`].
    #[error(
        "unknown placeholder {{{0}}}; use {{browser}}, {{major}}, {{driver}}, {{mark}} or \
         {{driver_major}}, and {{{{ or }}}} for a brace"
    )]
    UnknownPlaceholder(String),
    /// A `{` without its `}`.
    #[error("a {{ is not closed; write {{{{ for a literal brace")]
    Unclosed,
    /// A `}` that closes nothing.
    #[error("a }} closes nothing; write }}}} for a literal brace")]
    StrayClose,
}

impl FromStr for PromptFormat {
    type Err = FormatError;

    fn from_str(template: &str) -> Result<Self, Self::Err> {
        let mut pieces = Vec::new();
        let mut text = String::new();
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' | '}' if chars.peek() == Some(&c) => {
                    chars.next();
                    text.push(c);
                }
                '{' => {
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => name.push(c),
                            None => return Err(FormatError::Unclosed),
                        }
                    }
                    let piece = match name.as_str() {
                        "browser" => Piece::Browser,
                        "major" => Piece::Major,
                        "driver" => Piece::Driver,
                        "mark" => Piece::Mark,
                        "driver_major" => Piece::DriverMajor,
                        _ => return Err(FormatError::UnknownPlaceholder(name)),
                    };
                    if !text.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut text)));
                    }
                    pieces.push(piece);
                }
                '}' => return Err(FormatError::StrayClose),
                c => text.push(c),
            }
        }
        if !text.is_empty() {
            pieces.push(Piece::Text(text));
        }
        Ok(Self { pieces })
    }
}

impl PromptFormat {
    /// The line for `segment`, with the driver marked by `marks`.
    #[must_use]
    pub fn render(&self, segment: &Segment, marks: Marks) -> String {
        let mark = marks.mark(segment.driver);
        let mut line = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Text(text) => line.push_str(text),
                Piece::Browser => line.push_str(&segment.browser),
                Piece::Major => line.push_str(&segment.major.to_string()),
                Piece::Driver => {
                    line.push_str(mark);
                    line.push_str("driver");
                }
                Piece::Mark => line.push_str(mark),
                Piece::DriverMajor => {
                    if let Some(major) = segment.driver_major() {
                        line.push_str(&major.to_string());
                    }
                }
            }
        }
        line
    }
}

/// The size and modification time of a program file. A download or an update
/// of the program changes at least one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    len: u64,
    modified_secs: u64,
    modified_nanos: u32,
}

impl FileStamp {
    /// The stamp of the file at `path` (a link is followed), or `None` when it
    /// cannot be read.
    #[must_use]
    pub fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            len: metadata.len(),
            modified_secs: modified.as_secs(),
            modified_nanos: modified.subsec_nanos(),
        })
    }
}

/// What the cache knows about one program.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheEntry {
    stamp: FileStamp,
    major: Option<u32>,
    /// When the version was read, in seconds since the Unix epoch.
    checked: u64,
}

impl CacheEntry {
    /// Whether the entry still counts at `now` (seconds since the Unix epoch).
    /// An entry from the future, after the clock was set back, does not.
    fn fresh(&self, now: u64) -> bool {
        now.checked_sub(self.checked)
            .is_some_and(|age| age < CACHE_TTL.as_secs())
    }
}

/// `time` in whole seconds since the Unix epoch; 0 before it.
fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The cache's name for `program`: its absolute path, so the same relative
/// path in two folders is two entries.
fn cache_key(program: &Path) -> String {
    std::path::absolute(program)
        .unwrap_or_else(|_| program.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// Versions found by running programs, kept in a small JSON file in the user's
/// cache folder so the next prompt does not run them again.
///
/// An entry counts for [`CACHE_TTL`], and only while the program's
/// [`FileStamp`] is the one it was read with.
#[derive(Debug, Default)]
pub struct VersionCache {
    path: Option<PathBuf>,
    entries: BTreeMap<String, CacheEntry>,
    changed: bool,
}

impl VersionCache {
    /// `prompt-versions.json` in the `seleniumbase-rs` folder of the user's
    /// cache folder (`~/Library/Caches` on macOS, `$XDG_CACHE_HOME` or
    /// `~/.cache` on Linux, `%LOCALAPPDATA%` on Windows), when there is one.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|dir| dir.join("seleniumbase-rs").join(CACHE_FILE))
    }

    /// The cache kept in the file at `path`. A file that is missing or cannot
    /// be read is an empty cache.
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries,
            changed: false,
        }
    }

    /// A cache that remembers nothing and writes no file (`--no-cache`).
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// The major version remembered for `program` (`Some(None)` when it printed
    /// none), if it was read less than [`CACHE_TTL`] before `now` from the file
    /// `stamp` describes.
    #[must_use]
    pub fn get(&self, program: &Path, stamp: FileStamp, now: SystemTime) -> Option<Option<u32>> {
        let entry = self.entries.get(&cache_key(program))?;
        (entry.stamp == stamp && entry.fresh(unix_secs(now))).then_some(entry.major)
    }

    /// Remembers that `program`, as `stamp` describes it, gave `major` at `now`.
    pub fn put(&mut self, program: &Path, stamp: FileStamp, major: Option<u32>, now: SystemTime) {
        if self.path.is_none() {
            return;
        }
        let entry = CacheEntry {
            stamp,
            major,
            checked: unix_secs(now),
        };
        self.entries.insert(cache_key(program), entry);
        self.changed = true;
    }

    /// Writes the cache back to its file when something was added, leaving out
    /// the entries that have expired at `now`. The file is replaced in one
    /// step, so a prompt in another shell never reads half of it.
    ///
    /// # Errors
    ///
    /// When the folder cannot be created or the file cannot be written.
    pub fn save(&mut self, now: SystemTime) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !self.changed {
            return Ok(());
        }
        let now = unix_secs(now);
        self.entries.retain(|_, entry| entry.fresh(now));
        let folder = match path.parent() {
            Some(folder) if !folder.as_os_str().is_empty() => folder,
            _ => Path::new("."),
        };
        std::fs::create_dir_all(folder)?;
        let mut file = tempfile::NamedTempFile::new_in(folder)?;
        serde_json::to_writer(&mut file, &self.entries)?;
        file.persist(path).map_err(|error| error.error)?;
        self.changed = false;
        Ok(())
    }
}

/// Where the browser for the prompt was looked for, and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserLocation {
    /// A path that was given, by the option or variable `source` names.
    Given {
        /// The path, whether or not a file is there.
        path: PathBuf,
        /// `--browser-path` or `SB_CHROME_BIN`.
        source: &'static str,
    },
    /// Found where Chrome or Chromium is usually installed, or on the `PATH`.
    Found(PathBuf),
    /// None given and none found.
    NotFound,
}

/// The browser to report on: `flag` (`--browser-path`) first, then
/// `configured` (the value of `SB_CHROME_BIN`; empty counts as unset), then
/// what `search` finds. A given path is used even when no file is there, so a
/// wrong one is reported instead of silently replaced.
pub fn locate_browser(
    flag: Option<&Path>,
    configured: Option<&OsStr>,
    search: impl FnOnce() -> Option<PathBuf>,
) -> BrowserLocation {
    if let Some(path) = flag {
        return BrowserLocation::Given {
            path: path.to_path_buf(),
            source: "--browser-path",
        };
    }
    if let Some(path) = configured.filter(|value| !value.is_empty()) {
        return BrowserLocation::Given {
            path: PathBuf::from(path),
            source: "SB_CHROME_BIN",
        };
    }
    search().map_or(BrowserLocation::NotFound, BrowserLocation::Found)
}

/// `chromium` for a Chromium build, else `chrome`.
fn browser_label(binary: &Path) -> &'static str {
    let name = binary
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.contains("chromium") {
        "chromium"
    } else {
        "chrome"
    }
}

/// `155`, or `none` for a program that printed no version.
fn shown(major: Option<u32>) -> String {
    major.map_or_else(|| "none".to_owned(), |major| major.to_string())
}

/// The major version of `program` (`what` names it in the notes): from its
/// files, else from `cache`, else from `program --version`, which `cache` then
/// remembers. A program that cannot be started or does not answer in time
/// gives `None`, and that is not remembered, so the next prompt tries again.
async fn program_major(
    program: &Path,
    what: &str,
    cache: &mut VersionCache,
    now: SystemTime,
    notes: &mut Vec<String>,
) -> Option<u32> {
    if let Some(major) = major_without_running(program) {
        notes.push(format!("{what} version {major}: read from its files"));
        return Some(major);
    }
    let stamp = FileStamp::of(program);
    if let Some(major) = stamp.and_then(|stamp| cache.get(program, stamp, now)) {
        notes.push(format!("{what} version {}: from the cache", shown(major)));
        return major;
    }
    match version_flag_major(program, PROBE_TIMEOUT).await {
        Ok(major) => {
            if let Some(stamp) = stamp {
                cache.put(program, stamp, major, now);
            }
            notes.push(format!(
                "{what} version {}: from `{} --version`",
                shown(major),
                program.display()
            ));
            major
        }
        Err(error) => {
            notes.push(format!(
                "{what}: `{} --version` failed: {error}",
                program.display()
            ));
            None
        }
    }
}

/// What `sbrs prompt` found, with notes on how, for `--verbose`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    /// What the line is made of; `None` when no browser version was found,
    /// and the prompt then prints nothing.
    pub segment: Option<Segment>,
    /// One line per step: which programs were found and how each version was
    /// read.
    pub notes: Vec<String>,
}

/// Reads the versions of the browser at `browser` and the chromedriver
/// `driver` names, from files, from `cache`, or by running `--version` (which
/// `cache` then remembers), and compares them.
pub async fn probe(
    browser: &BrowserLocation,
    driver: &DriverLookup,
    cache: &mut VersionCache,
    now: SystemTime,
) -> Probe {
    let mut notes = Vec::new();
    let binary = match browser {
        BrowserLocation::Given { path, source } if path.is_file() => {
            notes.push(format!("browser: {} (from {source})", path.display()));
            path
        }
        BrowserLocation::Given { path, source } => {
            notes.push(format!(
                "browser: {source} names {}, which is not a file; printing nothing",
                path.display()
            ));
            return Probe {
                segment: None,
                notes,
            };
        }
        BrowserLocation::Found(path) => {
            notes.push(format!("browser: {}", path.display()));
            path
        }
        BrowserLocation::NotFound => {
            notes.push(
                "browser: no Chrome or Chromium found (SB_CHROME_BIN, CHROME_BIN, \
                 CHROMIUM_BIN, the usual install folders, PATH); printing nothing"
                    .to_owned(),
            );
            return Probe {
                segment: None,
                notes,
            };
        }
    };
    let Some(major) = program_major(binary, "browser", cache, now, &mut notes).await else {
        notes.push("browser: its version could not be read; printing nothing".to_owned());
        return Probe {
            segment: None,
            notes,
        };
    };
    let driver = match driver {
        DriverLookup::Found(path) => {
            notes.push(format!("driver: {}", path.display()));
            match program_major(path, "driver", cache, now, &mut notes).await {
                Some(driver) if driver == major => DriverStatus::Ready,
                Some(driver) => {
                    notes.push(format!(
                        "driver: {driver} cannot drive browser {major}; `sbrs install` fetches the matching one"
                    ));
                    DriverStatus::Mismatch { driver }
                }
                None => DriverStatus::Unknown,
            }
        }
        DriverLookup::MissingConfigured(path) => {
            notes.push(format!(
                "driver: {CHROMEDRIVER_PATH_VAR} names {}, which is not a file",
                path.display()
            ));
            DriverStatus::Missing
        }
        DriverLookup::NotFound => {
            notes.push(format!(
                "driver: no chromedriver in {CHROMEDRIVER_PATH_VAR}, {DRIVERS_DIR}/ or on PATH"
            ));
            DriverStatus::Missing
        }
    };
    let segment = Segment {
        browser: browser_label(binary).to_owned(),
        major,
        driver,
    };
    Probe {
        segment: Some(segment),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(driver: DriverStatus) -> Segment {
        Segment {
            browser: "chrome".to_owned(),
            major: 155,
            driver,
        }
    }

    fn render(template: &str, driver: DriverStatus, marks: Marks) -> String {
        let format: PromptFormat = template.parse().expect("a valid format");
        format.render(&segment(driver), marks)
    }

    #[test]
    fn the_default_line_names_the_browser_its_major_and_the_driver() {
        let format = PromptFormat::default();
        assert_eq!(
            format.render(&segment(DriverStatus::Ready), Marks::Unicode),
            "chrome 155 ✓driver"
        );
        assert_eq!(
            format.render(&segment(DriverStatus::Missing), Marks::Unicode),
            "chrome 155 ✗driver"
        );
        assert_eq!(
            format.render(&segment(DriverStatus::Ready), Marks::Ascii),
            "chrome 155 +driver"
        );
        assert_eq!(
            format.render(&segment(DriverStatus::Missing), Marks::Ascii),
            "chrome 155 -driver"
        );
    }

    #[test]
    fn every_placeholder_is_filled_for_every_driver_state() {
        let all = "{browser}|{major}|{driver}|{mark}|{driver_major}";
        let cases = [
            (
                DriverStatus::Ready,
                "chrome|155|✓driver|✓|155",
                "chrome|155|+driver|+|155",
            ),
            (
                DriverStatus::Mismatch { driver: 151 },
                "chrome|155|✗driver|✗|151",
                "chrome|155|-driver|-|151",
            ),
            (
                DriverStatus::Unknown,
                "chrome|155|?driver|?|",
                "chrome|155|?driver|?|",
            ),
            (
                DriverStatus::Missing,
                "chrome|155|✗driver|✗|",
                "chrome|155|-driver|-|",
            ),
        ];
        for (driver, unicode, ascii) in cases {
            assert_eq!(render(all, driver, Marks::Unicode), unicode, "{driver:?}");
            assert_eq!(render(all, driver, Marks::Ascii), ascii, "{driver:?}");
        }
    }

    #[test]
    fn text_and_doubled_braces_are_kept_as_written() {
        assert_eq!(
            render(
                "[{browser}:{major}] {{x}} ",
                DriverStatus::Ready,
                Marks::Ascii
            ),
            "[chrome:155] {x} "
        );
        assert_eq!(render("", DriverStatus::Ready, Marks::Ascii), "");
        assert_eq!(
            render("{{{major}}}", DriverStatus::Ready, Marks::Ascii),
            "{155}"
        );
    }

    #[test]
    fn a_wrong_format_is_refused_with_a_reason() {
        assert_eq!(
            "{browser} {version}".parse::<PromptFormat>(),
            Err(FormatError::UnknownPlaceholder("version".to_owned()))
        );
        assert_eq!("{major".parse::<PromptFormat>(), Err(FormatError::Unclosed));
        assert_eq!(
            "major}".parse::<PromptFormat>(),
            Err(FormatError::StrayClose)
        );
        let message = FormatError::UnknownPlaceholder("version".to_owned()).to_string();
        assert!(message.contains("{version}"), "{message}");
        for name in PLACEHOLDERS {
            assert!(
                message.contains(&format!("{{{name}}}")),
                "{name}: {message}"
            );
        }
    }

    fn stamp(len: u64) -> FileStamp {
        FileStamp {
            len,
            modified_secs: 1_700_000_000,
            modified_nanos: 5,
        }
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn a_version_is_remembered_for_the_ttl_and_only_for_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = VersionCache::open(dir.path().join("cache.json"));
        let driver = Path::new("/opt/drivers/chromedriver");
        let t0 = 1_800_000_000;
        assert_eq!(cache.get(driver, stamp(10), at(t0)), None);

        cache.put(driver, stamp(10), Some(155), at(t0));
        assert_eq!(cache.get(driver, stamp(10), at(t0)), Some(Some(155)));
        let last = t0 + CACHE_TTL.as_secs() - 1;
        assert_eq!(cache.get(driver, stamp(10), at(last)), Some(Some(155)));
        // Expired, replaced by another download, or a clock set back.
        assert_eq!(cache.get(driver, stamp(10), at(last + 1)), None);
        assert_eq!(cache.get(driver, stamp(11), at(t0)), None);
        assert_eq!(cache.get(driver, stamp(10), at(t0 - 1)), None);
        // Another program is another entry.
        assert_eq!(
            cache.get(Path::new("/opt/chromedriver"), stamp(10), at(t0)),
            None
        );

        // A program that printed no version is remembered as such.
        cache.put(driver, stamp(10), None, at(t0));
        assert_eq!(cache.get(driver, stamp(10), at(t0)), Some(None));
    }

    #[test]
    fn the_cache_survives_in_its_file_without_the_expired_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deeper").join("cache.json");
        let t0 = 1_800_000_000;
        let mut cache = VersionCache::open(path.clone());
        cache.put(Path::new("/a/old"), stamp(1), Some(150), at(t0));
        cache.put(
            Path::new("/a/new"),
            stamp(2),
            Some(155),
            at(t0 + CACHE_TTL.as_secs()),
        );
        cache.save(at(t0 + CACHE_TTL.as_secs())).unwrap();

        let reopened = VersionCache::open(path.clone());
        let now = at(t0 + CACHE_TTL.as_secs());
        assert_eq!(
            reopened.get(Path::new("/a/new"), stamp(2), now),
            Some(Some(155))
        );
        assert_eq!(reopened.entries.len(), 1, "the expired entry was dropped");

        // A damaged file is an empty cache, not an error.
        std::fs::write(&path, b"{ not json").unwrap();
        let damaged = VersionCache::open(path);
        assert_eq!(damaged.get(Path::new("/a/new"), stamp(2), now), None);
    }

    #[test]
    fn nothing_is_written_without_news_or_when_the_cache_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut untouched = VersionCache::open(path.clone());
        untouched.save(at(1)).unwrap();
        assert!(!path.exists(), "a cache with nothing new writes no file");

        let mut off = VersionCache::disabled();
        off.put(Path::new("/a/driver"), stamp(1), Some(155), at(1));
        assert_eq!(off.get(Path::new("/a/driver"), stamp(1), at(1)), None);
        off.save(at(1)).unwrap();
    }

    #[test]
    fn a_files_stamp_changes_with_its_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("chromedriver");
        assert_eq!(FileStamp::of(&file), None);
        std::fs::write(&file, b"one").unwrap();
        let first = FileStamp::of(&file).expect("a stamp");
        assert_eq!(FileStamp::of(&file), Some(first));
        std::fs::write(&file, b"three").unwrap();
        assert_ne!(FileStamp::of(&file), Some(first));
    }

    #[test]
    fn the_option_beats_the_variable_and_the_variable_beats_the_search() {
        let never = || -> Option<PathBuf> { panic!("no search when a path is given") };
        assert_eq!(
            locate_browser(Some(Path::new("/opt/a")), Some(OsStr::new("/opt/b")), never),
            BrowserLocation::Given {
                path: "/opt/a".into(),
                source: "--browser-path",
            }
        );
        assert_eq!(
            locate_browser(None, Some(OsStr::new("/opt/b")), never),
            BrowserLocation::Given {
                path: "/opt/b".into(),
                source: "SB_CHROME_BIN",
            }
        );
        // An empty variable counts as unset.
        assert_eq!(
            locate_browser(None, Some(OsStr::new("")), || Some("/opt/c".into())),
            BrowserLocation::Found("/opt/c".into())
        );
        assert_eq!(
            locate_browser(None, None, || None),
            BrowserLocation::NotFound
        );
    }

    /// The start of a macOS app's `Info.plist`.
    fn plist(version: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\
             \t<key>CFBundleShortVersionString</key>\n\t<string>{version}</string>\n</dict>\n</plist>\n"
        )
    }

    /// A browser in a macOS app bundle whose program writes `ran` beside
    /// itself if it is ever started, so a test can tell it was not.
    fn bundled_browser(dir: &Path, app: &str, name: &str, version: &str) -> PathBuf {
        let contents = dir.join(app).join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        std::fs::write(contents.join("Info.plist"), plist(version)).unwrap();
        let binary = contents.join("MacOS").join(name);
        std::fs::write(&binary, "#!/bin/sh\ntouch \"$0.ran\"\n").unwrap();
        binary
    }

    #[tokio::test]
    async fn no_browser_gives_no_segment_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = VersionCache::disabled();
        let now = SystemTime::now();

        let found = probe(
            &BrowserLocation::NotFound,
            &DriverLookup::NotFound,
            &mut cache,
            now,
        )
        .await;
        assert_eq!(found.segment, None);
        assert!(
            found.notes.join("\n").contains("no Chrome"),
            "{:?}",
            found.notes
        );

        let missing = dir.path().join("no-chrome");
        let given = BrowserLocation::Given {
            path: missing.clone(),
            source: "SB_CHROME_BIN",
        };
        let found = probe(&given, &DriverLookup::NotFound, &mut cache, now).await;
        assert_eq!(found.segment, None);
        let notes = found.notes.join("\n");
        assert!(
            notes.contains("SB_CHROME_BIN") && notes.contains(&missing.display().to_string()),
            "{notes}"
        );
    }

    #[tokio::test]
    async fn a_bundled_browser_is_read_without_starting_it() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = bundled_browser(
            dir.path(),
            "Google Chrome.app",
            "Google Chrome",
            "155.0.8059.40",
        );
        let chromium = bundled_browser(dir.path(), "Chromium.app", "Chromium", "154.0.1.2");
        let mut cache = VersionCache::disabled();
        let now = SystemTime::now();

        let found = probe(
            &BrowserLocation::Found(chrome.clone()),
            &DriverLookup::NotFound,
            &mut cache,
            now,
        )
        .await;
        assert_eq!(found.segment, Some(segment(DriverStatus::Missing)));
        let found = probe(
            &BrowserLocation::Found(chromium.clone()),
            &DriverLookup::MissingConfigured(dir.path().join("nowhere")),
            &mut cache,
            now,
        )
        .await;
        assert_eq!(
            found.segment,
            Some(Segment {
                browser: "chromium".to_owned(),
                major: 154,
                driver: DriverStatus::Missing,
            })
        );
        assert!(
            found.notes.join("\n").contains("CHROMEDRIVER_PATH"),
            "{:?}",
            found.notes
        );
        for binary in [chrome, chromium] {
            let mut ran = binary.into_os_string();
            ran.push(".ran");
            assert!(!Path::new(&ran).exists(), "the browser was started");
        }
    }

    /// An executable script at `dir/name` that prints `output` for
    /// `--version`, run once so macOS's first-run check is out of the way.
    #[cfg(unix)]
    fn driver_script(dir: &Path, name: &str, output: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho '{output}'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::process::Command::new(&path).arg("--version").output();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_driver_is_ready_only_at_the_browsers_major() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = bundled_browser(
            dir.path(),
            "Google Chrome.app",
            "Google Chrome",
            "155.0.8059.40",
        );
        let browser = BrowserLocation::Found(chrome);
        let now = SystemTime::now();
        let cases = [
            (
                "match",
                "ChromeDriver 155.0.8059.40 (abc)",
                DriverStatus::Ready,
            ),
            (
                "older",
                "ChromeDriver 151.0.7922.71 (abc)",
                DriverStatus::Mismatch { driver: 151 },
            ),
            ("silent", "", DriverStatus::Unknown),
        ];
        for (name, output, expected) in cases {
            let driver = driver_script(dir.path(), name, output);
            let mut cache = VersionCache::disabled();
            let found = probe(&browser, &DriverLookup::Found(driver), &mut cache, now).await;
            assert_eq!(
                found.segment,
                Some(segment(expected)),
                "{name}: {:?}",
                found.notes
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_driver_version_read_by_running_it_is_cached_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = bundled_browser(
            dir.path(),
            "Google Chrome.app",
            "Google Chrome",
            "155.0.8059.40",
        );
        let browser = BrowserLocation::Found(chrome);
        let driver = driver_script(
            dir.path(),
            "chromedriver",
            "ChromeDriver 155.0.8059.40 (abc)",
        );
        let lookup = DriverLookup::Found(driver.clone());
        let now = SystemTime::now();
        let mut cache = VersionCache::open(dir.path().join("cache.json"));

        let found = probe(&browser, &lookup, &mut cache, now).await;
        assert_eq!(found.segment, Some(segment(DriverStatus::Ready)));
        let stamp = FileStamp::of(&driver).unwrap();
        assert_eq!(cache.get(&driver, stamp, now), Some(Some(155)));

        // What the cache says wins while the file is unchanged: an entry that
        // says 151 shows that the driver was not run again.
        cache.put(&driver, stamp, Some(151), now);
        let found = probe(&browser, &lookup, &mut cache, now).await;
        assert_eq!(
            found.segment,
            Some(segment(DriverStatus::Mismatch { driver: 151 }))
        );
        assert!(
            found.notes.join("\n").contains("cache"),
            "{:?}",
            found.notes
        );
    }
}
